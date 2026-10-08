/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2014 The Closure Compiler Authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/Es6RewriteRestParameters.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `Es6RewriteRestParameters.java`: converts ES6 rest parameters to ES3 code that copies
//! the rest of `arguments` into an array.

use crate::{
    AbstractCompiler,
    ast_factory::AstFactory,
    compiler_pass::CompilerPass,
    js::runtime_js_lib_manager::JsLibField,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    transpilation_namespace::TranspilationNamespace,
    transpilation_passes::TranspilationPasses,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{node::NodeId, token::Token};
use std::sync::{Arc, LazyLock};

// port: Es6RewriteRestParameters#TRANSPILED_FEATURES
static TRANSPILED_FEATURES: LazyLock<FeatureSet> =
    LazyLock::new(|| FeatureSet::BARE_MINIMUM.with(Feature::REST_PARAMETERS));

pub struct Es6RewriteRestParameters {
    ast_factory: AstFactory,
    namespace: TranspilationNamespace,
    get_rest_arguments: Arc<dyn JsLibField>,
}

impl Es6RewriteRestParameters {
    // port: Es6RewriteRestParameters#Es6RewriteRestParameters
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        let ast_factory = compiler.create_ast_factory();
        let namespace = TranspilationNamespace::get(compiler);
        let get_rest_arguments = compiler
            .get_runtime_js_lib_manager()
            .lock()
            .unwrap()
            .get_js_lib_field("$jscomp.getRestArguments");
        Self {
            ast_factory,
            namespace,
            get_rest_arguments,
        }
    }

    /// Processes a rest parameter
    // port: Es6RewriteRestParameters#visitRestParam
    fn visit_rest_param(&self, t: &mut NodeTraversal<'_>, rest_param: NodeId, param_list: NodeId) {
        let function_body = param_list.get_next(t).unwrap();
        let rest_index = param_list.get_index_of_child(t, rest_param);
        let name_node = rest_param.get_only_child(t);
        let param_name = name_node.get_string(t);

        // Remove the existing param from the list, as it will be replaced with a declaration with
        // the same name.
        rest_param.detach(t);

        if !function_body.has_children(t) {
            // If function has no body, we are done!
            t.report_code_change();
            return;
        }

        // Now that the restParam is deleted, create a let declaration by making a new NAME node of
        // the same name `paramName`
        let compiler = t.get_compiler();
        let af = &self.ast_factory;
        let qname =
            af.create_qname_for_field(compiler, &self.namespace, self.get_rest_arguments.as_ref());
        let callee = af.create_get_prop_with_unknown_type(compiler, qname, "apply");
        let index = af.create_number(compiler, f64::from(rest_index));
        let arguments = af.create_arguments_reference(compiler);
        let call = af.create_call(
            compiler,
            callee,
            AstFactory::type_node(name_node),
            &[index, arguments],
        );
        let let_ = af
            .create_single_let_name_declaration_with_value(
                compiler, param_name, // creates a new NAME node with name `paramName`
                call,
            )
            .srcref_tree_if_missing(compiler, function_body);
        let insert_before_point =
            NodeUtil::get_insertion_point_after_all_inner_function_declarations(
                compiler,
                function_body,
            );
        if let Some(insert_before_point) = insert_before_point {
            let_.insert_before(compiler, insert_before_point);
        } else {
            // functionBody only contains hoisted function declarations
            function_body.add_child_to_back(compiler, let_);
        }
        let script = t.get_current_script().unwrap();
        NodeUtil::add_feature_to_script(t.get_compiler(), script, Feature::LET_DECLARATIONS);
        t.report_code_change();
    }
}

impl CompilerPass for Es6RewriteRestParameters {
    // port: Es6RewriteRestParameters#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        TranspilationPasses::process_transpile(compiler, root, *TRANSPILED_FEATURES, &mut [self]);
        TranspilationPasses::maybe_mark_features_as_transpiled_away(
            compiler,
            root,
            *TRANSPILED_FEATURES,
        );
    }
}

impl Callback for Es6RewriteRestParameters {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: Es6RewriteRestParameters#visit
    fn visit(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        current: NodeId,
        parent: Option<NodeId>,
    ) {
        if current.get_token(traversal) == Token::ITER_REST {
            self.visit_rest_param(traversal, current, parent.unwrap());
        }
    }
}
