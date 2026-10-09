/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ReplaceCssNames.java.

//! Port of `ReplaceCssNames.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::ast_factory::AstFactory;
use crate::compiler_pass::CompilerPass;
use crate::css_renaming_map::{CssRenamingMap, Style};
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{
    AbstractPostOrderCallback, AbstractPostOrderCallbackInterface, Callback, NodeTraversal,
};
use crate::process_closure_primitives::ProcessClosurePrimitives;
use closure_rhino::java_lang::string::split_units;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::{check_not_null, check_state};
use indexmap::{IndexMap, IndexSet};
use std::sync::Arc;

/// `GET_CSS_NAME_FUNCTION = IR.getprop(IR.name("goog"), "getCssName")`, used only for qualified
/// name comparison (`Node#matchesQualifiedName(Node)` against it matches the same nodes as
/// `matchesQualifiedName("goog.getCssName")`).
// port: ReplaceCssNames#GET_CSS_NAME_FUNCTION
pub const GET_CSS_NAME_FUNCTION: &str = "goog.getCssName";

// port: ReplaceCssNames#INVALID_NUM_ARGUMENTS_ERROR
pub static INVALID_NUM_ARGUMENTS_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_GETCSSNAME_NUM_ARGS",
    "goog.getCssName called with \"{0}\" arguments, expected 1 or 2.",
);

// port: ReplaceCssNames#STRING_LITERAL_EXPECTED_ERROR
pub static STRING_LITERAL_EXPECTED_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_GETCSSNAME_STRING_LITERAL_EXPECTED",
    "goog.getCssName called with invalid argument, string literal expected.  Was \"{0}\".",
);

// port: ReplaceCssNames#UNEXPECTED_STRING_LITERAL_ERROR
pub static UNEXPECTED_STRING_LITERAL_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_GETCSSNAME_UNEXPECTED_STRING_LITERAL",
    "goog.getCssName called with invalid arguments, string literal passed as first of two arguments.  Did you mean goog.getCssName(\"{0}-{1}\")?",
);

// port: ReplaceCssNames#NESTED_CALL_ERROR
pub static NESTED_CALL_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_GETCSSNAME_NESTED_CALL",
    "goog.getCssName: nested call is not allowed.",
);

// port: ReplaceCssNames#UNKNOWN_SYMBOL_WARNING
pub static UNKNOWN_SYMBOL_WARNING: DiagnosticType = DiagnosticType::warning(
    "JSC_GETCSSNAME_UNKNOWN_CSS_SYMBOL",
    "goog.getCssName called with unrecognized symbol \"{0}\" in class \"{1}\".",
);

// port: ReplaceCssNames#UNEXPECTED_SASS_GENERATED_CSS_TS_ERROR
pub static UNEXPECTED_SASS_GENERATED_CSS_TS_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_UNEXPECTED_SASS_GENERATED_CSS_TS",
    "@sass_generated_css_ts JSDoc annotation is only allowed on .css.closure.js files.",
);

// port: ReplaceCssNames#UNKNOWN_SYMBOL_ERROR
pub static UNKNOWN_SYMBOL_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_UNKNOWN_CSS_SYMBOL_IN_CLASSES_OBJECT",
    "Symbol was not defined \"{0}\" in classes object.",
);

// port: ReplaceCssNames#INVALID_USE_OF_CLASSES_OBJECT_ERROR
pub static INVALID_USE_OF_CLASSES_OBJECT_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_USE_OF_CLASSES_OBJECT",
    "invalid use of generated classes object. Only accessing its members is allowed.",
);

/// `GOOG_SET_CSS_NAME_MAPPING = IR.getprop(IR.name("goog"), "setCssNameMapping")`, used only for
/// qualified name comparison.
// port: ReplaceCssNames#GOOG_SET_CSS_NAME_MAPPING
const GOOG_SET_CSS_NAME_MAPPING: &str = "goog.setCssNameMapping";

/// Called for each class name seen when replacing.
// port: ReplaceCssNames.CssNameCollector
pub type CssNameCollector<'a> = Box<dyn FnMut(&JsString) + 'a>;

/// ReplaceCssNames replaces occurrences of goog.getCssName('foo') with a shorter version from the
/// passed in renaming map. There are two styles of operation: for 'BY_WHOLE' we look up the whole
/// string in the renaming map. For 'BY_PART', all the class name's components, separated by '-',
/// are renamed individually and then recombined.
///
/// In addition, the CSS names before replacement can optionally be gathered.
pub struct ReplaceCssNames<'a> {
    ast_factory: AstFactory,

    css_name_collector: CssNameCollector<'a>,

    css_names_by_symbol: IndexMap<JsString, JsString>,

    classes_objects_qualified_names: IndexSet<JsString>,

    symbol_map: Option<Arc<dyn CssRenamingMap>>,

    skiplist: Option<IndexSet<String>>,
}

impl<'a> ReplaceCssNames<'a> {
    // port: ReplaceCssNames#ReplaceCssNames
    pub fn new(
        compiler: &mut AbstractCompiler,
        symbol_map: Option<Arc<dyn CssRenamingMap>>,
        css_name_collector: CssNameCollector<'a>,
        skiplist: Option<IndexSet<String>>,
    ) -> Self {
        Self {
            ast_factory: compiler.create_ast_factory(),
            symbol_map,
            css_name_collector,
            skiplist,
            css_names_by_symbol: IndexMap::new(),
            classes_objects_qualified_names: IndexSet::new(),
        }
    }

    /// Create a list of objects representing all the `goog.getCssName()` calls in the AST.
    ///
    /// The elements in this list will be in depth-first left-to-right order. So, if you perform
    /// actions on them in this order you'll affect nested calls before the containing calls.
    // port: ReplaceCssNames#gatherGetCssNameInstances
    fn gather_get_css_name_instances(
        compiler: &mut AbstractCompiler,
        root: NodeId,
    ) -> Vec<GetCssNameInstance> {
        let mut gather_css_names_traversal = GatherCssNamesTraversal {
            list_of_css_name_instances: Vec::new(),
            traversal_state: TraversalState::default(),
        };
        NodeTraversal::traverse(compiler, root, &mut gather_css_names_traversal);
        gather_css_names_traversal.list_of_css_name_instances
    }

    /// Processes a string argument to goog.getCssName(). The string will be renamed based off the
    /// symbol map. If there is no map or any part of the name can't be renamed, a warning is
    /// reported to the compiler and the node is left unchanged.
    ///
    /// If the type is unexpected then an error is reported to the compiler.
    // port: ReplaceCssNames#processStringNode
    fn process_string_node(&self, compiler: &mut AbstractCompiler, n: NodeId) {
        let name = n.get_string(compiler);
        if self
            .skiplist
            .as_ref()
            .is_some_and(|skiplist| skiplist.contains(&name.to_string()))
        {
            // We apply the skiplist before splitting on dashes, and not after.
            // External substitution maps should do the same.
            return;
        }
        let parts = split_units(&name, "-");
        if let Some(symbol_map) = &self.symbol_map {
            let replacement: JsString;

            if name.starts_with("--") {
                // Force BY_WHOLE style for CSS variables.
                let Some(r) = symbol_map.get(&name) else {
                    let name = name.to_string();
                    compiler.report(JSError::make(
                        compiler,
                        n,
                        &UNKNOWN_SYMBOL_WARNING,
                        &[&name, &name],
                    ));
                    return;
                };
                replacement = r;
            } else {
                match symbol_map.get_style() {
                    Style::BY_WHOLE => {
                        let Some(r) = symbol_map.get(&name) else {
                            let name = name.to_string();
                            compiler.report(JSError::make(
                                compiler,
                                n,
                                &UNKNOWN_SYMBOL_WARNING,
                                &[&name, &name],
                            ));
                            return;
                        };
                        replacement = r;
                    }
                    Style::BY_PART => {
                        let mut replaced: Vec<JsString> = Vec::with_capacity(parts.len());
                        for part_name in &parts {
                            let part = symbol_map.get(part_name);
                            let Some(part) = part else {
                                // If we can't encode all parts, don't encode any of it.
                                let part_name = part_name.to_string();
                                let name = name.to_string();
                                compiler.report(JSError::make(
                                    compiler,
                                    n,
                                    &UNKNOWN_SYMBOL_WARNING,
                                    &[&part_name, &name],
                                ));
                                return;
                            };
                            replaced.push(part);
                        }
                        // Joiner.on("-").join(replaced)
                        let mut joined: Vec<u16> = Vec::new();
                        for (i, part) in replaced.iter().enumerate() {
                            if i > 0 {
                                joined.push(u16::from(b'-'));
                            }
                            joined.extend_from_slice(part.as_units());
                        }
                        replacement = JsString::from_units(joined);
                    }
                }
            }
            n.set_string(compiler, replacement);
        }
    }

    /// `GetCssNameInstance#replaceWithExpression`: replace this `goog.getCssName()` call in the
    /// AST with a string typed expression.
    // port: ReplaceCssNames.GetCssNameInstance#replaceWithExpression
    fn replace_with_expression(&self, compiler: &mut AbstractCompiler, inst: &GetCssNameInstance) {
        let call_node = inst.call_node;
        check_state!(
            inst.is_valid(),
            "not a valid goog.getCssName() call: %s",
            call_node.to_string(compiler)
        );
        check_not_null!(
            call_node.get_parent(compiler),
            "already replaced: %s",
            call_node.to_string(compiler)
        );
        let child_count = call_node.get_child_count(compiler);
        match child_count {
            2 => self.replace_with_converted_single_arg(compiler, call_node),
            3 => self.replace_with_concatenated_args(compiler, call_node),
            _ => panic!(
                "IllegalStateException: {}",
                format_args!(
                    "invalid number of children: {} for: {}",
                    child_count,
                    call_node.to_string(compiler)
                )
            ),
        }
    }

    // port: ReplaceCssNames.GetCssNameInstance#replaceWithConvertedSingleArg
    fn replace_with_converted_single_arg(
        &self,
        compiler: &mut AbstractCompiler,
        call_node: NodeId,
    ) {
        // `goog.getCssName('some-literal')` -> `'mapped-literal'`
        let string_lit_arg = check_not_null!(
            call_node.get_second_child(compiler),
            "%s",
            call_node.to_string(compiler)
        );
        check_state!(
            string_lit_arg.is_string_lit(compiler),
            "not a string literal: %s",
            string_lit_arg.to_string(compiler)
        );
        string_lit_arg.detach(compiler);
        self.process_string_node(compiler, string_lit_arg);
        call_node.replace_with(compiler, string_lit_arg);
        compiler.report_change_to_enclosing_scope(string_lit_arg);
    }

    // port: ReplaceCssNames.GetCssNameInstance#replaceWithConcatenatedArgs
    fn replace_with_concatenated_args(&self, compiler: &mut AbstractCompiler, call_node: NodeId) {
        // `goog.getCssName(someExpr, 'some-literal')` -> `someExpr + '-mapped-literal'`
        let first_arg = check_not_null!(
            call_node.get_second_child(compiler),
            "%s",
            call_node.to_string(compiler)
        );
        let second_arg = check_not_null!(
            first_arg.get_next(compiler),
            "%s",
            first_arg.to_string(compiler)
        );
        first_arg.detach(compiler);
        second_arg.detach(compiler);
        self.process_string_node(compiler, second_arg);
        let dashed = JsString::from("-").concat(&second_arg.get_string(compiler));
        second_arg.set_string(compiler, dashed);
        let replacement = self
            .ast_factory
            .create_add(compiler, first_arg, second_arg)
            .srcref(compiler, call_node);
        call_node.replace_with(compiler, replacement);
        compiler.report_change_to_enclosing_scope(replacement);
    }

    // port: ReplaceCssNames#isNotInCssClosureFile
    fn is_not_in_css_closure_file(ast: &Ast, node: NodeId) -> bool {
        let source_file_name = node.get_source_file_name(ast);
        source_file_name.is_none_or(|name| !name.ends_with(".css.closure.js"))
    }

    // port: ReplaceCssNames#isGetCssNameCall
    fn is_get_css_name_call(ast: &Ast, node: NodeId) -> bool {
        node.is_call(ast)
            && node
                .get_first_child(ast)
                .unwrap()
                .matches_qualified_name(ast, GET_CSS_NAME_FUNCTION)
    }

    // port: ReplaceCssNames#createGetCssNameInstance
    fn create_get_css_name_instance(
        ast: &Ast,
        n: NodeId,
        css_closure_classes_qualified_name: Option<JsString>,
    ) -> GetCssNameInstance {
        let validation_error = Self::validate_get_css_name_call(ast, n);
        GetCssNameInstance {
            call_node: n,
            validation_error,
            css_closure_classes_qualified_name,
        }
    }

    // port: ReplaceCssNames#validateGetCssNameCall
    fn validate_get_css_name_call(ast: &Ast, call_node: NodeId) -> Option<JSError> {
        let child_count = call_node.get_child_count(ast);
        match child_count {
            2 => Self::validate_single_arg_get_css_name_call(ast, call_node),
            3 => Self::validate_two_arg_get_css_name_call(ast, call_node),
            _ => Some(JSError::make(
                ast,
                call_node,
                &INVALID_NUM_ARGUMENTS_ERROR,
                &[&child_count.to_string()],
            )),
        }
    }

    // port: ReplaceCssNames#validateSingleArgGetCssNameCall
    fn validate_single_arg_get_css_name_call(ast: &Ast, call_node: NodeId) -> Option<JSError> {
        // `goog.getCssName('css-name')`
        let string_literal_arg = check_not_null!(call_node.get_last_child(ast));
        if !string_literal_arg.is_string_lit(ast) {
            return Some(JSError::make(
                ast,
                call_node,
                &STRING_LITERAL_EXPECTED_ERROR,
                &[&string_literal_arg.get_token(ast).to_string()],
            ));
        }
        None
    }

    // port: ReplaceCssNames#validateTwoArgGetCssNameCall
    fn validate_two_arg_get_css_name_call(ast: &Ast, call_node: NodeId) -> Option<JSError> {
        // `goog.getCssName(BASE_CSS_NAME, 'css-suffix')`
        // The first child is the callee, so the first argument is the second child
        let first_arg = check_not_null!(call_node.get_second_child(ast));
        let second_arg = check_not_null!(first_arg.get_next(ast));

        if !second_arg.is_string_lit(ast) {
            return Some(JSError::make(
                ast,
                call_node,
                &STRING_LITERAL_EXPECTED_ERROR,
                &[&second_arg.get_token(ast).to_string()],
            ));
        }
        if first_arg.is_string_lit(ast) {
            return Some(JSError::make(
                ast,
                call_node,
                &UNEXPECTED_STRING_LITERAL_ERROR,
                &[
                    &first_arg.get_string(ast).to_string(),
                    &second_arg.get_string(ast).to_string(),
                ],
            ));
        }
        if Self::is_get_css_name_call(ast, first_arg) {
            // Disallow `goog.getCssName(goog.getCssName('n1'), 'n2')`.
            // Disallowing this is a bit arbitrary and is done for historical reasons.
            // The 2-argument form results in more JS code shipped to the browser and
            // forces users to have camelCase CSS names, which contravenes normal CSS style.
            // Since we'd like to stop supporting the 2-argument form, we don't want to
            // start allowing any form of it that was previously an error.
            return Some(JSError::make(ast, first_arg, &NESTED_CALL_ERROR, &[]));
        }

        None
    }
}

impl CompilerPass for ReplaceCssNames<'_> {
    // port: ReplaceCssNames#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(
            compiler,
            root,
            &mut AbstractPostOrderCallback::new(FindSetCssNameTraversal { outer: self }),
        );
        let get_css_name_instances = Self::gather_get_css_name_instances(compiler, root);
        for get_css_name_instance in &get_css_name_instances {
            let css_class_name = get_css_name_instance.get_css_class_name(compiler);
            if get_css_name_instance.is_css_closure_file_classes_member() {
                let member_name =
                    get_css_name_instance.get_css_closure_classes_member_name(compiler);
                self.css_names_by_symbol.insert(member_name, css_class_name);
                self.classes_objects_qualified_names
                    .insert(get_css_name_instance.get_css_closure_classes_qualified_name(compiler));
            } else {
                (self.css_name_collector)(&css_class_name);
            }
            self.replace_with_expression(compiler, get_css_name_instance);
        }
        NodeTraversal::traverse(compiler, root, &mut CountCssNamesBySymbol { outer: self });
    }
}

struct FindSetCssNameTraversal<'p, 'a> {
    outer: &'p mut ReplaceCssNames<'a>,
}

impl AbstractPostOrderCallbackInterface for FindSetCssNameTraversal<'_, '_> {
    // port: ReplaceCssNames.FindSetCssNameTraversal#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let Some(parent) = parent else {
            return;
        };
        if !n.is_call(t) || !parent.is_expr_result(t) {
            return;
        }
        let callee = n.get_first_child(t).unwrap();
        if !callee.matches_qualified_name(t, GOOG_SET_CSS_NAME_MAPPING) {
            return;
        }
        let compiler = t.get_compiler();
        let css_renaming_map =
            ProcessClosurePrimitives::process_set_css_name_mapping(compiler, n, parent);
        self.outer.symbol_map = css_renaming_map;
        compiler.report_change_to_enclosing_scope(parent);
        parent.detach(compiler);
    }
}

struct GatherCssNamesTraversal {
    list_of_css_name_instances: Vec<GetCssNameInstance>,
    traversal_state: TraversalState,
}

impl Callback for GatherCssNamesTraversal {
    // port: ReplaceCssNames.GatherCssNamesTraversal#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        let sass_generated_css_ts_expert = Self::create_sass_generated_css_ts_expert(t, n);
        if let Some(error) = sass_generated_css_ts_expert.sass_generated_css_ts_validation_error {
            t.get_compiler().report(error);
            // Skip all nodes that are descendants of this one, since we found an invalid
            // @sassGeneratedCssTs JSDoc annotation, and are in an invalid state.
            return false;
        }
        if n.is_script(t) {
            self.traversal_state.in_sass_generated_css_ts_script =
                sass_generated_css_ts_expert.has_sass_generated_css_ts_js_doc;
            self.traversal_state.css_closure_classes_qualified_name = None;
        } else if self.traversal_state.in_sass_generated_css_ts_script
            && sass_generated_css_ts_expert.is_css_closure_classes_assignment
        {
            self.traversal_state.css_closure_classes_qualified_name =
                sass_generated_css_ts_expert.css_closure_classes_qualified_name;
        }
        true
    }

    // port: ReplaceCssNames.GatherCssNamesTraversal#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if ReplaceCssNames::is_get_css_name_call(t, n) {
            let get_css_name_instance = ReplaceCssNames::create_get_css_name_instance(
                t,
                n,
                self.traversal_state
                    .css_closure_classes_qualified_name
                    .clone(),
            );
            if get_css_name_instance.is_valid() {
                self.list_of_css_name_instances.push(get_css_name_instance);
            } else {
                let error = get_css_name_instance.get_validation_error(t);
                t.get_compiler().report(error);
            }
        }
    }
}

impl GatherCssNamesTraversal {
    // port: ReplaceCssNames.GatherCssNamesTraversal#createSassGeneratedCssTsExpert
    fn create_sass_generated_css_ts_expert(ast: &Ast, n: NodeId) -> SassGeneratedCssTsExpert {
        let has_sass_generated_css_ts_js_doc = n
            .get_jsdoc_info(ast)
            .is_some_and(|info| info.is_sass_generated_css_ts());

        let mut sass_generated_css_ts_validation_error = None;
        let is_invalid_sass_generated_css_ts_js_doc =
            !n.is_script(ast) || ReplaceCssNames::is_not_in_css_closure_file(ast, n);
        if has_sass_generated_css_ts_js_doc && is_invalid_sass_generated_css_ts_js_doc {
            sass_generated_css_ts_validation_error = Some(JSError::make(
                ast,
                n,
                &UNEXPECTED_SASS_GENERATED_CSS_TS_ERROR,
                &[],
            ));
        }

        let assignment_target = n.get_first_child(ast);
        let is_css_closure_classes_assignment = n.is_assign(ast)
            && assignment_target.is_some_and(|target| {
                target.is_get_prop(ast) && target.get_string_ref(ast) == "classes"
            });

        let mut css_closure_classes_qualified_name = None;
        if is_css_closure_classes_assignment {
            css_closure_classes_qualified_name = assignment_target.unwrap().get_qualified_name(ast);
        }

        SassGeneratedCssTsExpert {
            has_sass_generated_css_ts_js_doc,
            sass_generated_css_ts_validation_error,
            is_css_closure_classes_assignment,
            css_closure_classes_qualified_name,
        }
    }
}

// port: ReplaceCssNames.GatherCssNamesTraversal.SassGeneratedCssTsExpert
struct SassGeneratedCssTsExpert {
    has_sass_generated_css_ts_js_doc: bool,
    sass_generated_css_ts_validation_error: Option<JSError>,
    is_css_closure_classes_assignment: bool,
    css_closure_classes_qualified_name: Option<JsString>,
}

// port: ReplaceCssNames.GatherCssNamesTraversal.TraversalState
#[derive(Default)]
struct TraversalState {
    in_sass_generated_css_ts_script: bool,
    css_closure_classes_qualified_name: Option<JsString>,
}

/// Represents a `goog.getCssName()` call.
// port: ReplaceCssNames.GetCssNameInstance
struct GetCssNameInstance {
    /// The CALL node representing `goog.getCssName()`.
    ///
    /// We shouldn't store final pointers to any of the children of this node, because they might
    /// be changed between creation of this object and performance of other methods.
    call_node: NodeId,

    /// Non-null if the shape of the function call was invalid, when this object was created.
    validation_error: Option<JSError>,

    /// The qualified name for the classes object if we're in a Sass-generated .css.ts file, or
    /// null otherwise. For example "module$exports$foo.classes".
    css_closure_classes_qualified_name: Option<JsString>,
}

impl GetCssNameInstance {
    // port: ReplaceCssNames.GetCssNameInstance#isValid
    fn is_valid(&self) -> bool {
        self.validation_error.is_none()
    }

    // port: ReplaceCssNames.GetCssNameInstance#getValidationError
    fn get_validation_error(&self, ast: &Ast) -> JSError {
        check_not_null!(
            self.validation_error.clone(),
            "No validation error found: %s",
            self.call_node.to_string(ast)
        )
    }

    // port: ReplaceCssNames.GetCssNameInstance#isCssClosureFileClassesMember
    fn is_css_closure_file_classes_member(&self) -> bool {
        self.css_closure_classes_qualified_name.is_some()
    }

    // port: ReplaceCssNames.GetCssNameInstance#getCssClosureClassesQualifiedName
    fn get_css_closure_classes_qualified_name(&self, ast: &Ast) -> JsString {
        check_not_null!(
            self.css_closure_classes_qualified_name.clone(),
            "Not a css closure file classes object: %s",
            self.call_node.to_string(ast)
        )
    }

    // port: ReplaceCssNames.GetCssNameInstance#getCssClosureClassesMemberName
    fn get_css_closure_classes_member_name(&self, ast: &Ast) -> JsString {
        let css_closure_classes_qualified_name = check_not_null!(
            self.css_closure_classes_qualified_name.clone(),
            "Not a css closure file classes object: %s",
            self.call_node.to_string(ast)
        );
        // We support 2 structures getCssName() calls in Sass-generated files:
        // 1. { Foo: goog.getCssName('foo') }
        // 2. { Foo: expression ? … : goog.getCssName('foo') }
        // Traversing our ancestry allows us to find the `Foo` string key in both cases.
        for ancestor in self.call_node.get_ancestors(ast) {
            if ancestor.is_string_key(ast) {
                return css_closure_classes_qualified_name
                    .concat(&JsString::from("."))
                    .concat(&ancestor.get_string(ast));
            }
        }
        panic!(
            "IllegalStateException: {}",
            format_args!(
                "No css closure classes member name found: {}",
                self.call_node.to_string(ast)
            )
        );
    }

    // port: ReplaceCssNames.GetCssNameInstance#getCssClassName
    fn get_css_class_name(&self, ast: &Ast) -> JsString {
        let call_node = self.call_node;
        check_state!(
            self.is_valid(),
            "not a valid goog.getCssName() call: %s",
            call_node.to_string(ast)
        );
        check_not_null!(
            call_node.get_parent(ast),
            "already replaced: %s",
            call_node.to_string(ast)
        );
        let child_count = call_node.get_child_count(ast);
        let first_arg = check_not_null!(
            call_node.get_second_child(ast),
            "%s",
            call_node.to_string(ast)
        );
        match child_count {
            2 => {
                check_state!(
                    first_arg.is_string_lit(ast),
                    "not a string literal: %s",
                    first_arg.to_string(ast)
                );
                first_arg.get_string(ast)
            }
            3 => {
                let second_arg =
                    check_not_null!(first_arg.get_next(ast), "%s", first_arg.to_string(ast));
                check_state!(
                    second_arg.is_string_lit(ast),
                    "not a string literal: %s",
                    second_arg.to_string(ast)
                );
                second_arg.get_string(ast)
            }
            _ => panic!(
                "IllegalStateException: {}",
                format_args!(
                    "invalid number of children: {} for: {}",
                    child_count,
                    call_node.to_string(ast)
                )
            ),
        }
    }
}

struct CountCssNamesBySymbol<'p, 'a> {
    outer: &'p mut ReplaceCssNames<'a>,
}

impl Callback for CountCssNamesBySymbol<'_, '_> {
    // port: ReplaceCssNames.CountCssNamesBySymbol#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(t) {
            // Only descend into source files NOT named `*.css.closure.js`
            ReplaceCssNames::is_not_in_css_closure_file(t, n)
        } else {
            true // descend into every other node
        }
    }

    // port: ReplaceCssNames.CountCssNamesBySymbol#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let classes_object_qualified_name = n.get_qualified_name(t);
        let Some(classes_object_qualified_name) = classes_object_qualified_name else {
            return;
        };
        if !self
            .outer
            .classes_objects_qualified_names
            .contains(&classes_object_qualified_name)
        {
            return;
        }
        let class_name_node = n.get_parent(t).unwrap();
        if !n.is_get_prop(t) || !class_name_node.is_get_prop(t) {
            let compiler = t.get_compiler();
            compiler.report(JSError::make(
                compiler,
                n,
                &INVALID_USE_OF_CLASSES_OBJECT_ERROR,
                &[],
            ));
            return;
        }
        let class_qualified_name = class_name_node.get_qualified_name(t);
        let Some(class_name) = class_qualified_name
            .as_ref()
            .and_then(|name| self.outer.css_names_by_symbol.get(name))
            .cloned()
        else {
            let compiler = t.get_compiler();
            let symbol = class_name_node.get_string(compiler).to_string();
            compiler.report(JSError::make(
                compiler,
                n,
                &UNKNOWN_SYMBOL_ERROR,
                &[&symbol],
            ));
            return;
        };
        (self.outer.css_name_collector)(&class_name);
    }
}
