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
//   src/com/google/javascript/jscomp/Es6ConvertSuper.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `Es6ConvertSuper.java`.
//!
//! Converts `super.method()` calls and adds constructors to any classes that lack them.
//!
//! This has to run before the main `Es6RewriteClass` pass. The super() constructor calls are not
//! converted here, but rather in `Es6ConvertSuperConstructorCalls`, which runs later.

use crate::{
    AbstractCompiler,
    ast_factory::AstFactory,
    compiler_pass::CompilerPass,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    synthesize_explicit_constructors::SynthesizeExplicitConstructors,
    transpilation_passes::TranspilationPasses,
    transpilation_util::{CANNOT_CONVERT, CANNOT_CONVERT_YET},
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{
    check_state,
    ir::IR,
    jscomp_colors::standard_colors,
    jsdoc_info,
    node::{Ast, NodeId, Prop},
    token::Token,
};
use std::sync::LazyLock;

// TODO(b/329447979): Run this pass only for scripts containing Feature.SUPER and not for scripts
// containing just Feature.CLASSES?
// port: Es6ConvertSuper#FEATURES_TO_RUN_FOR
static FEATURES_TO_RUN_FOR: LazyLock<FeatureSet> =
    LazyLock::new(|| FeatureSet::BARE_MINIMUM.with_features(&[Feature::SUPER, Feature::CLASSES]));

pub struct Es6ConvertSuper {
    ast_factory: AstFactory,
    constructor_synthesizer: SynthesizeExplicitConstructors,
}

impl Es6ConvertSuper {
    // port: Es6ConvertSuper#Es6ConvertSuper
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        let ast_factory = compiler.create_ast_factory();
        let constructor_synthesizer = SynthesizeExplicitConstructors::new(compiler);
        Self {
            ast_factory,
            constructor_synthesizer,
        }
    }

    // port: Es6ConvertSuper#isInterface
    fn is_interface(ast: &Ast, class_node: NodeId) -> bool {
        let class_js_doc_info = NodeUtil::get_best_jsdoc_info(ast, class_node);
        class_js_doc_info.is_some_and(|info| info.is_interface())
    }

    // port: Es6ConvertSuper#visitSuper
    fn visit_super(&self, t: &mut NodeTraversal<'_>, node: NodeId, parent: NodeId) {
        check_state!(node.is_super(t));
        let mut expr_root = node;

        let expr_root_parent = expr_root.get_parent(t).unwrap();
        if expr_root_parent.is_get_elem(t) || expr_root_parent.is_get_prop(t) {
            expr_root = expr_root_parent;
        }

        let enclosing_member_def = NodeUtil::get_enclosing_node(t, expr_root, &|ast, n| {
            matches!(
                n.get_token(ast),
                Token::MEMBER_FUNCTION_DEF
                    | Token::GETTER_DEF
                    | Token::SETTER_DEF
                    | Token::COMPUTED_PROP
            )
        });

        let enclosing_member_def = match enclosing_member_def {
            Some(def) if !def.get_parent(t).unwrap().is_object_lit(t) => def,
            _ => {
                let error = JSError::make(
                    t,
                    node,
                    &CANNOT_CONVERT,
                    &["super is not supported in object literal methods."],
                );
                t.get_compiler().report(error);
                return;
            }
        };

        if parent.is_call(t) {
            // super(...)
            self.visit_super_call(t, node, parent, enclosing_member_def);
        } else if parent.is_get_prop(t) || parent.is_get_elem(t) {
            if parent.get_first_child(t) == Some(node) {
                if parent.get_parent(t).unwrap().is_call(t)
                    && NodeUtil::is_invocation_target(t, parent)
                {
                    // super.something(...) or super['something'](..)
                    self.visit_super_property_call(t, node, parent, enclosing_member_def);
                } else {
                    // super.something or super['something']
                    self.visit_super_property_access(t, node, parent, enclosing_member_def);
                }
            } else {
                // super.something used in some other way
                let error = JSError::make(
                    t,
                    node,
                    &CANNOT_CONVERT_YET,
                    &["Only calls to super or to a method of super are supported."],
                );
                t.get_compiler().report(error);
            }
        } else if parent.is_new(t) {
            panic!("This should never happen. Did Es6SuperCheck fail to run?");
        } else {
            // some other use of super we don't support yet
            let error = JSError::make(
                t,
                node,
                &CANNOT_CONVERT_YET,
                &["Only calls to super or to a method of super are supported."],
            );
            t.get_compiler().report(error);
        }
    }

    // port: Es6ConvertSuper#visitSuperCall
    fn visit_super_call(
        &self,
        t: &mut NodeTraversal<'_>,
        node: NodeId,
        parent: NodeId,
        enclosing_member_def: NodeId,
    ) {
        check_state!(parent.is_call(t), "%s", parent.to_string(t));
        check_state!(node.is_super(t), "%s", node.to_string(t));

        let clazz = NodeUtil::get_enclosing_class(t, node).unwrap();
        let super_name = clazz.get_second_child(t).unwrap();
        if !super_name.is_qualified_name(t) {
            // This will be reported as an error in Es6ToEs3Converter.
            return;
        }

        if NodeUtil::is_es6_constructor_member_function_def(t, enclosing_member_def) {
            // Calls to super() constructors will be transpiled by Es6ConvertSuperConstructorCalls
            // later.
            if node.is_from_externs(t) || Self::is_interface(t, clazz) {
                // If a class is defined in an externs file or as an interface, it's only a stub,
                // not an implementation that should be instantiated.
                // A call to super() shouldn't actually exist for these cases and is problematic to
                // transpile, so just drop it.
                let enclosing_statement = NodeUtil::get_enclosing_statement(t, node).unwrap();
                let enclosing_statement_parent = enclosing_statement.get_parent(t).unwrap();
                enclosing_statement.detach(t);
                t.get_compiler()
                    .report_change_to_enclosing_scope(enclosing_statement_parent);
            }
            // Calls to super() constructors will be transpiled by Es6ConvertSuperConstructorCalls
            // later.
        } else {
            // super can only be directly called in a constructor
            panic!("This should never happen. Did Es6SuperCheck fail to run?");
        }
    }

    // port: Es6ConvertSuper#visitSuperPropertyCall
    fn visit_super_property_call(
        &self,
        t: &mut NodeTraversal<'_>,
        node: NodeId,
        parent: NodeId,
        enclosing_member_def: NodeId,
    ) {
        check_state!(
            parent.is_get_prop(t) || parent.is_get_elem(t),
            "%s",
            parent.to_string(t)
        );
        check_state!(node.is_super(t), "%s", node.to_string(t));
        let grandparent = parent.get_parent(t).unwrap();
        check_state!(grandparent.is_call(t));

        let clazz = NodeUtil::get_enclosing_class(t, node).unwrap();
        let super_name = clazz.get_second_child(t).unwrap();
        if !super_name.is_qualified_name(t) {
            // This will be reported as an error in Es6ToEs3Converter.
            return;
        }

        let compiler = t.get_compiler();
        let af = &self.ast_factory;
        let mut call_target = parent;
        if enclosing_member_def.is_static_member(compiler) {
            let expanded_super = super_name.clone_tree(compiler).srcref_tree(compiler, node);
            expanded_super.set_original_name(compiler, Some("super".into()));
            node.replace_with(compiler, expanded_super);
            call_target.detach(compiler);
            call_target = af.create_get_prop_with_unknown_type(compiler, call_target, "call");
            grandparent.add_child_to_front(compiler, call_target);
            let this_node = af.create_this(compiler, AstFactory::type_node(clazz));
            this_node.make_non_indexable(compiler); // no direct correlation with original source
            this_node.insert_after(compiler, call_target);
            grandparent.srcref_tree_if_missing(compiler, parent);

            // We added a `this` reference to the call. We need to make sure this member is not
            // collapsed.
            Self::ensure_no_collapse_on_static_member_def(compiler, enclosing_member_def);
        } else {
            // Replace super node to give
            // super.method(...) -> SuperClass.prototype.method(...)
            let super_name_clone = super_name.clone_tree(compiler);
            let expanded_super = af
                .create_prototype_access(compiler, super_name_clone)
                .srcref_tree(compiler, node);
            expanded_super.set_original_name(compiler, Some("super".into()));
            node.replace_with(compiler, expanded_super);
            // Set the 'this' object correctly for the call
            // SuperClass.prototype.method(...) -> SuperClass.prototype.method.call(this, ...)
            call_target.detach(compiler);
            call_target = af.create_get_prop_with_unknown_type(compiler, call_target, "call");
            grandparent.add_child_to_front(compiler, call_target);
            let this_node = af.create_this_for_es6_class(compiler, clazz);
            this_node.make_non_indexable(compiler); // no direct correlation with original source
            this_node.insert_after(compiler, call_target);
            grandparent.put_boolean_prop(compiler, Prop::FREE_CALL, false);
            grandparent.srcref_tree_if_missing(compiler, parent);
        }
        compiler.report_change_to_enclosing_scope(grandparent);
    }

    // port: Es6ConvertSuper#visitSuperPropertyAccess
    fn visit_super_property_access(
        &self,
        t: &mut NodeTraversal<'_>,
        node: NodeId,
        parent: NodeId,
        enclosing_member_def: NodeId,
    ) {
        check_state!(
            parent.is_get_prop(t) || parent.is_get_elem(t),
            "%s",
            parent.to_string(t)
        );
        check_state!(node.is_super(t), "%s", node.to_string(t));
        let grandparent = parent.get_parent(t).unwrap();

        if NodeUtil::is_l_value(t, parent) {
            // We don't support assigning to a super property
            let error = JSError::make(
                t,
                parent,
                &CANNOT_CONVERT_YET,
                &["assigning to a super property"],
            );
            t.get_compiler().report(error);
            return;
        }

        let clazz = NodeUtil::get_enclosing_class(t, node).unwrap();
        let super_name = clazz.get_second_child(t).unwrap();
        if !super_name.is_qualified_name(t) {
            // This will be reported as an error in Es6ToEs3Converter.
            return;
        }

        let compiler = t.get_compiler();
        let af = &self.ast_factory;
        let new_prop = if enclosing_member_def.is_static_member(compiler) {
            // super.prop -> SuperClass.prop
            super_name.clone_tree(compiler).srcref_tree(compiler, node)
        } else {
            // super.prop -> SuperClass.prototype.prop
            let super_name_clone = super_name.clone_tree(compiler);
            af.create_prototype_access(compiler, super_name_clone)
                .srcref_tree(compiler, node)
        };

        if parent.is_get_prop(compiler)
            && compiler
                .get_accessor_summary()
                .expect("Compiler#getAccessorSummary")
                .get_kind(&parent.get_string(compiler))
                .has_getter()
        {
            // Rewrites
            //   super.x
            // as
            //   Reflect.get(super, JSCompiler_renameProperty('x', ParentClass), this)
            let super_placeholder = IR::name(compiler, "sp"); // No need for type info as this will be replaced.

            let reflect = af.create_constant_name(
                compiler,
                "Reflect",
                AstFactory::type_(standard_colors::UNKNOWN.clone()),
            );
            let reflect_get = af.create_get_prop_with_unknown_type(compiler, reflect, "get");
            let property_arg = if parent.is_get_prop(compiler) {
                let rename_fn = af.create_name(
                    compiler,
                    NodeUtil::JSC_PROPERTY_NAME_FN,
                    AstFactory::type_(standard_colors::UNKNOWN.clone()),
                );
                let prop_name = parent.get_string(compiler);
                let prop_string = af.create_string(compiler, prop_name);
                let super_name_clone = super_name.clone_tree(compiler).srcref_tree(compiler, node);
                af.create_call(
                    compiler,
                    rename_fn,
                    AstFactory::type_(standard_colors::STRING.clone()),
                    &[prop_string, super_name_clone],
                )
            } else {
                // For getElem, we just use the accessor value in the Reflect.get call.
                let accessor = node.get_next(compiler).unwrap();
                accessor.detach(compiler);
                accessor
            };
            let this_node = af.create_this(compiler, AstFactory::type_node(clazz));
            let reflect_get_call = af
                .create_call(
                    compiler,
                    reflect_get,
                    AstFactory::type_node(parent),
                    &[super_placeholder, property_arg, this_node],
                )
                .srcref_tree_if_missing(compiler, parent);

            // Replace the getProp/getElem with the Reflect.get call.
            parent.replace_with(compiler, reflect_get_call);
            // Swap in the `n` (super) where it goes in the Reflect.get call.
            node.detach(compiler);
            super_placeholder.replace_with(compiler, node);

            // We added a `this` reference to the call. We need to make sure this member is not
            // collapsed.
            Self::ensure_no_collapse_on_static_member_def(compiler, enclosing_member_def);
        }

        node.replace_with(compiler, new_prop);
        new_prop.set_original_name(compiler, Some("super".into()));

        compiler.report_change_to_enclosing_scope(grandparent);
    }

    // port: Es6ConvertSuper#ensureNoCollapseOnStaticMemberDef
    fn ensure_no_collapse_on_static_member_def(
        compiler: &mut AbstractCompiler,
        member_def: NodeId,
    ) {
        if !member_def.is_static_member(compiler) {
            return;
        }
        let info = member_def.get_jsdoc_info(compiler);
        let mut builder = jsdoc_info::Builder::maybe_copy_from(info.as_deref());
        if builder.record_no_collapse() {
            let built = builder.build();
            member_def.set_jsdoc_info(compiler, built);
        }
    }
}

impl Callback for Es6ConvertSuper {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: Es6ConvertSuper#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_class(t) {
            self.constructor_synthesizer
                .synthesize_class_constructor_if_missing(t, n);
        } else if n.is_super(t) {
            self.visit_super(t, n, parent.unwrap());
        }
    }
}

impl CompilerPass for Es6ConvertSuper {
    // port: Es6ConvertSuper#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        // Might need to synthesize constructors for ambient classes in .d.ts externs
        TranspilationPasses::process_transpile(
            compiler,
            externs,
            *FEATURES_TO_RUN_FOR,
            &mut [self],
        );
        TranspilationPasses::process_transpile(compiler, root, *FEATURES_TO_RUN_FOR, &mut [self]);
    }
}
