/*
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
//   src/com/google/javascript/jscomp/LateEs6ToEs3Converter.java.

//! Port of `LateEs6ToEs3Converter.java`.
use crate::{
    AbstractCompiler,
    ast_factory::AstFactory,
    compiler_pass::CompilerPass,
    es6_template_literals::Es6TemplateLiterals,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    report_untranspilable_features,
    transpilation_passes::TranspilationPasses,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    jscomp_colors::standard_colors,
    node::{Ast, NodeId, Prop},
    qualified_name::QualifiedName,
    token::Token,
};

// port: LateEs6ToEs3Converter#transpiledFeatures
fn transpiled_features() -> FeatureSet {
    FeatureSet::BARE_MINIMUM.with_features(&[
        Feature::COMPUTED_PROPERTIES,
        Feature::MEMBER_DECLARATIONS,
        Feature::TEMPLATE_LITERALS,
    ])
}

const FRESH_COMP_PROP_VAR: &str = "$jscomp$compprop";

/// Converts ES6 code to valid ES5 code. This class does most of the transpilation, and
/// https://github.com/google/closure-compiler/wiki/ECMAScript6 lists which ES6 features are
/// supported. Other classes that start with "Es6" do other parts of the transpilation.
///
/// In most cases, the output is valid as ES3 (hence the class name) but in some cases, if the
/// output language is set to ES5, we rely on ES5 features such as getters, setters, and
/// Object.defineProperties.
pub struct LateEs6ToEs3Converter {
    ast_factory: AstFactory,
    template_literal_converter: Es6TemplateLiterals,
    // We want to insert the call to `var tagFnFirstArg = $jscomp.createTemplateTagFirstArg...`
    // just before this node. For the first script, this node is right after the runtime injected
    // function definition as injecting to the top of the script causes runtime errors
    // https://github.com/google/closure-compiler/issues/3589. For the subsequent script(s), the
    // call is injected to the top of that script.
    template_lit_insertion_point: Option<NodeId>,
}

impl LateEs6ToEs3Converter {
    // port: LateEs6ToEs3Converter#LateEs6ToEs3Converter
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        Self {
            ast_factory: compiler.create_ast_factory(),
            template_literal_converter: Es6TemplateLiterals::new(compiler),
            template_lit_insertion_point: None,
        }
    }

    // port: LateEs6ToEs3Converter#findTemplateLitInsertionPoint
    fn find_template_lit_insertion_point(ast: &Ast, script: NodeId) -> Option<NodeId> {
        if script.get_is_in_closure_unaware_subtree(ast) {
            // For all closure-unaware scripts, initialize the injection point within the closure
            // aware function.
            let closure_unaware_block =
                check_not_null!(NodeUtil::find_closure_unaware_script_root(ast, script));
            return NodeUtil::get_insertion_point_after_all_inner_function_declarations(
                ast,
                closure_unaware_block,
            );
        }
        // For all other scripts, initialize the injection point to be top of the script. The spec
        // requires a single unique array per template literal, so for a template literal in a
        // function e.g. it would be incorrect to initialize a new array per every function call.
        script.get_first_child(ast)
    }

    /// Converts a member definition in an object literal to an ES3 key/value pair. Member
    /// definitions in classes are handled in `Es6RewriteClass`.
    // port: LateEs6ToEs3Converter#visitMemberFunctionDefInObjectLit
    fn visit_member_function_def_in_object_lit(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        let name = n.get_string(compiler);
        let name_node = n.get_first_first_child(compiler).unwrap();
        let value = check_not_null!(n.remove_first_child(compiler));
        let string_key = self.ast_factory.create_string_key(compiler, name, value);
        let info = n.get_jsdoc_info(compiler);
        string_key.set_jsdoc_info(compiler, info);
        n.replace_with(compiler, string_key);
        string_key.srcref(compiler, name_node);
        compiler.report_change_to_enclosing_scope(string_key);
    }

    // port: LateEs6ToEs3Converter#visitObject
    fn visit_object(&self, t: &mut NodeTraversal<'_>, obj: NodeId) {
        let mut child = obj.get_first_child(t);
        while let Some(c) = child {
            if c.is_computed_prop(t) {
                self.visit_object_with_computed_property(t, obj);
                return;
            }
            child = c.get_next(t);
        }
    }

    /// Transpiles an object node with computed property, and add type information to the new
    /// nodes if this pass ran after type checking. For example,
    ///
    /// ```text
    /// var obj = {a: 1, [i++]: 2}
    /// is transpiled to
    /// var $jscomp$compprop0 = {};
    /// var obj = ($jscomp$compprop0.a = 1, ($jscomp$compprop0[i++] = 2, $jscomp$compprop0));
    /// ```
    ///
    /// Note that when adding type information to the nodes, the NAME node $jscomp$compprop0 would
    /// always be assigned the type of the entire object (in the above example {a: number}). This
    /// is because we do not have sufficient type information during transpilation to know, for
    /// example, $jscomp$compprop0 has type Object{} in the expression $jscomp$compprop0.a = 1
    // port: LateEs6ToEs3Converter#visitObjectWithComputedProperty
    fn visit_object_with_computed_property(&self, t: &mut NodeTraversal<'_>, obj: NodeId) {
        check_argument!(obj.is_object_lit(t));
        let mut props: Vec<NodeId> = Vec::new();
        let mut curr_element = obj.get_first_child(t);
        let object_type = AstFactory::type_node(obj);

        while let Some(curr) = curr_element {
            if curr.get_boolean_prop(t, Prop::COMPUTED_PROP_GETTER)
                || curr.get_boolean_prop(t, Prop::COMPUTED_PROP_SETTER)
            {
                let error = JSError::make(
                    t,
                    curr,
                    &report_untranspilable_features::UNTRANSPILABLE_FEATURE_PRESENT,
                    &["computed getter/setter in an object literal", "ES2015", ""],
                );
                t.get_compiler().report(error);
                return;
            } else if curr.is_getter_def(t) || curr.is_setter_def(t) {
                curr_element = curr.get_next(t);
            } else {
                let next_node = curr.get_next(t);
                curr.detach(t);
                props.push(curr);
                curr_element = next_node;
            }
        }

        let input = check_not_null!(t.get_input().cloned());
        let unique_id = t
            .get_compiler()
            .get_unique_id_supplier()
            .get_unique_id(&input);
        let obj_name = format!("{FRESH_COMP_PROP_VAR}{unique_id}");

        props.reverse();
        let af = &self.ast_factory;
        let mut result = af.create_name(t.get_compiler(), obj_name.as_str(), object_type.clone());
        for propdef in props {
            if propdef.is_computed_prop(t) {
                let property_expression = check_not_null!(propdef.remove_first_child(t));
                let value = check_not_null!(propdef.remove_first_child(t));
                let name = af.create_name(t.get_compiler(), obj_name.as_str(), object_type.clone());
                let get_elem = af.create_get_elem(t.get_compiler(), name, property_expression);
                let assign = af.create_assign(t.get_compiler(), get_elem, value);
                result = af.create_comma(t.get_compiler(), assign, result);
            } else {
                let val = check_not_null!(propdef.remove_first_child(t));
                let is_quoted_access = propdef.is_quoted_string_key(t);

                propdef.set_token(t, Token::STRINGLIT);
                propdef.set_color(t, Some(standard_colors::STRING.clone()));
                propdef.put_boolean_prop(t, Prop::QUOTED, false);

                let obj_name_node =
                    af.create_name(t.get_compiler(), obj_name.as_str(), object_type.clone());
                let access = if is_quoted_access {
                    af.create_get_elem(t.get_compiler(), obj_name_node, propdef)
                } else {
                    let prop_name = propdef.get_string(t);
                    af.create_get_prop(
                        t.get_compiler(),
                        obj_name_node,
                        prop_name,
                        AstFactory::type_node(propdef),
                    )
                };
                let assign = af.create_assign(t.get_compiler(), access, val);
                result = af.create_comma(t.get_compiler(), assign, result);
            }
        }

        let mut statement = obj;
        while !NodeUtil::is_statement(t, statement) {
            statement = statement.get_parent(t).unwrap();
        }

        let is_inside_loop_header = NodeUtil::is_loop_structure(t, statement);

        if is_inside_loop_header {
            let placeholder = IR::empty(t);
            obj.replace_with(t, placeholder);

            let name = af.create_name(t.get_compiler(), obj_name.as_str(), object_type.clone());
            let assign_empty_obj = af.create_assign(t.get_compiler(), name, obj);
            result = af.create_comma(t.get_compiler(), assign_empty_obj, result);
            result.srcref_tree_if_missing(t, obj);

            placeholder.replace_with(t, result);

            let name = af.create_name(t.get_compiler(), obj_name.as_str(), object_type);
            let var = IR::var(t, name);
            var.srcref_tree_if_missing(t, statement);
            var.insert_before(t, statement);
            t.get_compiler().report_change_to_enclosing_scope(var);
        } else {
            result.srcref_tree_if_missing(t, obj);
            obj.replace_with(t, result);

            let name = af.create_name(t.get_compiler(), obj_name.as_str(), object_type);
            let var = IR::var_with_value(t, name, obj);
            var.srcref_tree_if_missing(t, statement);
            var.insert_before(t, statement);
            t.get_compiler().report_change_to_enclosing_scope(var);
        }
    }
}

impl CompilerPass for LateEs6ToEs3Converter {
    // port: LateEs6ToEs3Converter#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        TranspilationPasses::process_transpile(compiler, root, transpiled_features(), &mut [self]);
        TranspilationPasses::maybe_mark_features_as_transpiled_away(
            compiler,
            root,
            transpiled_features(),
        );
    }
}

impl Callback for LateEs6ToEs3Converter {
    // port: LateEs6ToEs3Converter#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(t) {
            self.template_lit_insertion_point = Self::find_template_lit_insertion_point(t, n);
        }
        true
    }

    // port: LateEs6ToEs3Converter#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::ASSIGN => {
                // Find whether this script contains the `$jscomp.createTemplateTagFirstArgWithRaw
                // = function(..) {..}` node. If yes, update the templateLitInsertionPoint.
                let lhs = n.get_first_child(t).unwrap();
                let rhs = n.get_second_child(t).unwrap();
                if lhs.is_get_prop(t)
                    && rhs.is_function(t)
                    && lhs.get_first_child(t).unwrap().is_name(t)
                {
                    let q_name = QualifiedName::of("$jscomp.createTemplateTagFirstArgWithRaw");
                    if q_name.matches(t, lhs) {
                        let n_parent = n.get_parent(t);
                        check_not_null!(n_parent, "%s", n.to_string(t));
                        check_state!(n_parent.unwrap().is_expr_result(t), "%s", n.to_string(t));
                        self.template_lit_insertion_point = n_parent.unwrap().get_next(t);
                    }
                }
            }
            Token::OBJECTLIT => self.visit_object(t, n),
            Token::MEMBER_FUNCTION_DEF if parent.unwrap().is_object_lit(t) => {
                self.visit_member_function_def_in_object_lit(t.get_compiler(), n);
            }
            Token::TAGGED_TEMPLATELIT => self
                .template_literal_converter
                .visit_tagged_template_literal(t, n, self.template_lit_insertion_point),
            Token::TEMPLATELIT if !parent.unwrap().is_tagged_template_lit(t) => {
                self.template_literal_converter.visit_template_literal(t, n);
            }
            _ => {}
        }
    }
}
