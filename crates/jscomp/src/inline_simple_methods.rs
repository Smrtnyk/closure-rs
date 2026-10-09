/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2007 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/InlineSimpleMethods.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    ast_analyzer::AstAnalyzer,
    compiler_pass::CompilerPass,
    invocations_callback::InvocationsCallback,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_not_null, check_state, ir::IR, js_string::JsString, node::NodeId, token::Token,
};

/// Inlines methods that take no arguments and have only a return statement returning a property.
/// Because it works on method names rather than type inference, a method with multiple
/// definitions will be inlined if each definition is identical.
///
/// ```text
/// A.prototype.foo = function() { return this.b; }
/// B.prototype.foo = function() { return this.b; }
/// ```
///
/// will inline foo, but
///
/// ```text
/// A.prototype.foo = function() { return this.b; }
/// B.prototype.foo = function() { return this.c; }
/// ```
///
/// will not.
///
/// Declarations are not removed because we do not find all possible call sites. For examples,
/// calls of the form foo["bar"] are not detected.
///
/// This pass is not on by default because it is not safe in simple mode. If the prototype method
/// is mutated and we don't detect that, inlining it is unsafe. We enable it whenever function
/// inlining is enabled.
pub struct InlineSimpleMethods {
    // List of property names that we know are not safe to inline
    // This includes:
    //   - extern properties (we can't see the actual method definition for externs)
    //   - non-method properties
    //   - methods with @noinline
    //   - methods with multiple, non-equivalent definitions
    non_inlineable_properties: IndexSet<JsString>,

    // Use a linked map here to keep the output deterministic.  Otherwise,
    // the choice of method bodies is random when multiple identical definitions
    // are found which causes problems in the source maps.
    // (Java: LinkedHashMultimap<String, Node>)
    method_definitions: IndexMap<JsString, IndexSet<NodeId>>,

    ast_analyzer: AstAnalyzer,
}

impl InlineSimpleMethods {
    // port: InlineSimpleMethods#InlineSimpleMethods
    pub fn new(compiler: &AbstractCompiler) -> Self {
        let ast_analyzer = compiler.get_ast_analyzer();
        let mut non_inlineable_properties = IndexSet::<_>::default();
        non_inlineable_properties.extend(
            check_not_null!(compiler.get_extern_properties())
                .iter()
                .map(|s| JsString::from(s.as_str())),
        );
        Self {
            non_inlineable_properties,
            method_definitions: IndexMap::<_, _>::default(),
            ast_analyzer,
        }
    }

    /// Returns true if the provided node is a getprop for which the left child is this or a valid
    /// property tree and for which the right side is a string.
    // port: InlineSimpleMethods#isPropertyTree
    fn is_property_tree(ast: &closure_rhino::node::Ast, expected_getprop: NodeId) -> bool {
        if !expected_getprop.is_get_prop(ast) {
            return false;
        }

        let getprop_lhs = expected_getprop.get_first_child(ast).unwrap();
        getprop_lhs.is_this(ast) || Self::is_property_tree(ast, getprop_lhs)
    }

    /// Finds the occurrence of "this" in the provided property tree and replaces it with
    /// replacement
    // port: InlineSimpleMethods#replaceThis
    fn replace_this(
        ast: &mut closure_rhino::node::Ast,
        expected_getprop: NodeId,
        replacement: NodeId,
    ) {
        let left_child = expected_getprop.get_first_child(ast).unwrap();
        if left_child.is_this(ast) {
            left_child.replace_with(ast, replacement);
        } else {
            Self::replace_this(ast, left_child, replacement);
        }
    }

    /// Return the node that represents the expression returned by the method, given a FUNCTION
    /// node.
    // port: InlineSimpleMethods#returnedExpression
    fn returned_expression(ast: &closure_rhino::node::Ast, r#fn: NodeId) -> Option<NodeId> {
        let expected_block = NodeUtil::get_function_body(ast, r#fn);
        if !expected_block.has_one_child(ast) {
            return None;
        }

        let expected_return = expected_block.get_first_child(ast).unwrap();
        if !expected_return.is_return(ast) {
            return None;
        }

        if !expected_return.has_one_child(ast) {
            return None;
        }

        Some(expected_return.get_only_child(ast))
    }

    /// Return whether the given FUNCTION node is an empty method definition.
    ///
    /// Must be private, or moved to NodeUtil.
    // port: InlineSimpleMethods#isEmptyMethod
    fn is_empty_method(ast: &closure_rhino::node::Ast, r#fn: NodeId) -> bool {
        NodeUtil::is_empty_block(ast, NodeUtil::get_function_body(ast, r#fn))
    }

    /// Given a set of method definitions, verify they are the same.
    // port: InlineSimpleMethods#allDefinitionsEquivalent
    fn all_definitions_equivalent(
        &self,
        compiler: &AbstractCompiler,
        definitions: &IndexSet<NodeId>,
    ) -> bool {
        let mut first: Option<NodeId> = None;
        for &n in definitions {
            match first {
                None => first = Some(n),
                Some(first) => {
                    if !compiler.are_nodes_equal_for_inlining(first, n) {
                        return false;
                    } // else continue
                }
            }
        }
        true
    }

    // port: InlineSimpleMethods#anyDefinitionsNotInlineable
    fn any_definitions_not_inlineable(
        &self,
        compiler: &mut AbstractCompiler,
        definitions: &IndexSet<NodeId>,
    ) -> bool {
        for &n in definitions {
            if n.is_async_function(compiler) || n.is_generator_function(compiler) {
                // async and generator functions cannot be trivially inlined because they return a
                // Promise or generator object, not the return value directly. For now just don't
                // inline them - it's unclear it's worth the complexity.
                return true;
            }
            if NodeUtil::function_parameters_may_have_side_effects(compiler, n, &self.ast_analyzer)
            {
                // Methods with parameters that may have side effects or throw on destructuring are
                // not inlined to prevent altering runtime behavior.
                return true;
            }
            let js_doc_info =
                NodeUtil::get_best_jsdoc_info(compiler, n.get_parent(compiler).unwrap());
            if js_doc_info.is_some_and(|info| info.is_no_inline()) {
                return true;
            }
        }
        false
    }

    /// Replace the provided method call with the tree specified in returnedValue
    ///
    /// Parse tree of a call is name call getprop obj string
    // port: InlineSimpleMethods#inlinePropertyReturn
    fn inline_property_return(
        &mut self,
        compiler: &mut AbstractCompiler,
        call: NodeId,
        returned_value: NodeId,
    ) {
        let get_prop = returned_value.clone_tree(compiler);
        let replacement = call
            .get_first_child(compiler)
            .unwrap()
            .remove_first_child(compiler)
            .unwrap();
        Self::replace_this(compiler, get_prop, replacement);
        call.replace_with(compiler, get_prop);
        compiler.report_change_to_enclosing_scope(get_prop);
    }

    /// Replace the provided object and its method call with the tree specified in returnedValue.
    /// Should be called only if the object reference has no side effects.
    // port: InlineSimpleMethods#inlineConstReturn
    fn inline_const_return(
        &mut self,
        compiler: &mut AbstractCompiler,
        call: NodeId,
        returned_value: NodeId,
    ) {
        let ret_value = returned_value.clone_tree(compiler);
        call.replace_with(compiler, ret_value);
        compiler.report_change_to_enclosing_scope(ret_value);
    }

    /// Remove the provided object and its method call.
    // port: InlineSimpleMethods#inlineEmptyMethod
    fn inline_empty_method(&mut self, t: &mut NodeTraversal<'_>, parent: NodeId, call: NodeId) {
        // If the return value of the method call is read,
        // replace it with "void 0". Otherwise, remove the call entirely.

        if NodeUtil::is_expr_call(t, parent) {
            let empty = IR::empty(t);
            parent.replace_with(t, empty);
            NodeUtil::mark_functions_deleted(t.get_compiler(), parent);
        } else {
            let src_location = call;
            let undefined = NodeUtil::new_undefined_node(t, Some(src_location));
            call.replace_with(t, undefined);
            NodeUtil::mark_functions_deleted(t.get_compiler(), call);
        }
        t.report_code_change();
    }

    /// Check whether the given method call's arguments have side effects. `call` is the call node
    /// of a method invocation.
    // port: InlineSimpleMethods#argsMayHaveSideEffects
    fn args_may_have_side_effects(&self, compiler: &mut AbstractCompiler, call: NodeId) -> bool {
        let mut current_child = call.get_second_child(compiler);
        while let Some(child) = current_child {
            if self.ast_analyzer.may_have_side_effects(compiler, child) {
                return true;
            }
            current_child = child.get_next(compiler);
        }

        false
    }

    /// Adds a node that may represent a function signature (if it's a function itself or the name
    /// of a function).
    // port: InlineSimpleMethods#addPossibleSignature
    fn add_possible_signature(
        &mut self,
        ast: &closure_rhino::node::Ast,
        name: JsString,
        node: Option<NodeId>,
    ) {
        if let Some(node) = node.filter(|node| node.is_function(ast)) {
            // The node we're looking at is a function, so we can add it directly
            self.add_signature(name, node);
        } else {
            self.non_inlineable_properties.insert(name);
        }
    }

    // port: InlineSimpleMethods#addSignature
    fn add_signature(&mut self, name: JsString, function: NodeId) {
        if self.non_inlineable_properties.contains(&name) {
            return;
        }

        self.method_definitions
            .entry(name)
            .or_default()
            .insert(function);
    }

    // port: InlineSimpleMethods#getPropName
    fn get_prop_name(ast: &closure_rhino::node::Ast, get_prop_elem: NodeId) -> Option<JsString> {
        if get_prop_elem.is_get_prop(ast) {
            Some(get_prop_elem.get_string(ast))
        } else if get_prop_elem
            .get_second_child(ast)
            .unwrap()
            .is_string_lit(ast)
        {
            Some(get_prop_elem.get_second_child(ast).unwrap().get_string(ast))
        } else {
            None
        }
    }
}

impl CompilerPass for InlineSimpleMethods {
    // port: InlineSimpleMethods#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let stage = compiler.get_life_cycle_stage();
        check_state!(stage.is_normalized(), "%s", format!("{stage:?}"));
        check_state!(self.method_definitions.is_empty());
        // checkState(externs != null): a NodeId is never null.

        NodeTraversal::traverse_roots(
            compiler,
            &mut GatherSignatures { outer: self },
            externs,
            root,
        );
        NodeTraversal::traverse(compiler, root, &mut InlineTrivialAccessors { outer: self });
    }
}

/// For each method call, see if it is a candidate for inlining, and do the inlining if so.
struct InlineTrivialAccessors<'a> {
    outer: &'a mut InlineSimpleMethods,
}

impl Callback for InlineTrivialAccessors<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        InvocationsCallback::visit(self, t, n, parent);
    }
}

impl InvocationsCallback for InlineTrivialAccessors<'_> {
    // port: InlineSimpleMethods.InlineTrivialAccessors#visit
    fn visit_with_call_name(
        &mut self,
        t: &mut NodeTraversal<'_>,
        call_node: NodeId,
        parent: Option<NodeId>,
        call_name: JsString,
    ) {
        let outer = &mut *self.outer;
        if outer.non_inlineable_properties.contains(&call_name) {
            return;
        }

        let definitions = match outer.method_definitions.get(&call_name) {
            Some(definitions) if !definitions.is_empty() => definitions.clone(),
            _ => return,
        };

        // Exit early if all method definitions are not equivalent, and mark this method as not
        // inlineable.
        // NOTE: we could also cache the 'good' result of this method having all equivalent
        // definitions and avoid recalculating later, but profile data suggests that's not too
        // useful
        if definitions.len() > 1
            && !outer.all_definitions_equivalent(t.get_compiler(), &definitions)
        {
            outer.non_inlineable_properties.insert(call_name);
            return;
        }
        // Exit early if any definitions are incompatible with the InlineSimpleMethods inlining
        // NOTE: we could also cache the 'good' result of these methods being inlineable to
        // avoid recalculating later, but profile data suggests that's not too useful.
        if outer.any_definitions_not_inlineable(t.get_compiler(), &definitions) {
            outer.non_inlineable_properties.insert(call_name);
            return;
        }

        // Do check of arity, complexity, and consistency in what we think is
        // the order from least to most complex
        let first_definition = *definitions.iter().next().unwrap();

        // Do not inline if the callsite is a derived class calling a base method using `super.`
        if !outer.args_may_have_side_effects(t.get_compiler(), call_node)
            && !NodeUtil::references_super(t, call_node)
        {
            // Verify this is a trivial return
            let returned = InlineSimpleMethods::returned_expression(t, first_definition);
            if let Some(returned) = returned {
                if InlineSimpleMethods::is_property_tree(t, returned)
                    && !first_definition.is_arrow_function(t)
                {
                    outer.inline_property_return(t.get_compiler(), call_node, returned);
                } else if NodeUtil::is_literal_value(t, returned, false) && {
                    let callee = call_node.get_first_child(t).unwrap();
                    !outer
                        .ast_analyzer
                        .may_have_side_effects(t.get_compiler(), callee)
                } {
                    outer.inline_const_return(t.get_compiler(), call_node, returned);
                }
            } else if InlineSimpleMethods::is_empty_method(t, first_definition) && {
                let callee = call_node.get_first_child(t).unwrap();
                !outer
                    .ast_analyzer
                    .may_have_side_effects(t.get_compiler(), callee)
            } {
                outer.inline_empty_method(t, parent.unwrap(), call_node);
            }
        }
    }
}

/// Gather signatures from the source to be compiled.
struct GatherSignatures<'a> {
    outer: &'a mut InlineSimpleMethods,
}

impl GatherSignatures<'_> {
    /// Processes the parent of a GETPROP prototype, which can either be another GETPROP (in the
    /// case of Foo.prototype.bar), or can be an assignment (in the case of Foo.prototype = ...).
    // port: InlineSimpleMethods.GatherSignatures#processPrototypeParent
    fn process_prototype_parent(&mut self, ast: &closure_rhino::node::Ast, n: NodeId) {
        match n.get_token(ast) {
            Token::GETPROP | Token::GETELEM => {
                // Foo.prototype.getBar = function() { ... } or
                // Foo.prototype.getBar = getBaz (where getBaz is a function)
                // parse tree looks like:
                // assign                          <- parent
                //     getprop                     <- n
                //         getprop
                //             name Foo
                //             string prototype
                //         string getBar
                //     function or name            <- assignee
                let grandparent = n.get_grandparent(ast).unwrap();
                let name = InlineSimpleMethods::get_prop_name(ast, n);
                if let Some(name) = name
                    && grandparent.is_assign(ast)
                {
                    let assignee = grandparent.get_second_child(ast);
                    self.outer.add_possible_signature(ast, name, assignee);
                }
            }
            _ => {}
        }
    }
}

impl Callback for GatherSignatures<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: InlineSimpleMethods.GatherSignatures#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::GETPROP | Token::GETELEM => {
                // TODO(b/251573710): Handle ES2015+ language features that can reference
                // properties
                let Some(name) = InlineSimpleMethods::get_prop_name(t, n) else {
                    return;
                };
                let parent = parent.unwrap();

                if name == "prototype" {
                    self.process_prototype_parent(t, parent);
                } else {
                    // Static methods of the form Foo.bar = function() {} or
                    // Static methods of the form Foo.bar = baz (where baz is a
                    // function name). Parse tree looks like:
                    // assign                 <- parent
                    //      getprop           <- n
                    //          name Foo
                    //          string bar
                    //      function or name  <- n.getNext()
                    if parent.is_assign(t) && n.is_first_child_of(t, Some(parent)) {
                        let next = n.get_next(t);
                        self.outer.add_possible_signature(t, name, next);
                    }
                }
            }
            Token::OBJECTLIT | Token::CLASS_MEMBERS => {
                let mut key = n.get_first_child(t);
                while let Some(k) = key {
                    match k.get_token(t) {
                        Token::MEMBER_FUNCTION_DEF
                        | Token::MEMBER_FIELD_DEF
                        | Token::STRING_KEY => {
                            let name = k.get_string(t);
                            let first = k.get_first_child(t);
                            self.outer.add_possible_signature(t, name, first);
                        }
                        Token::SETTER_DEF | Token::GETTER_DEF => {
                            self.outer.non_inlineable_properties.insert(k.get_string(t));
                        }
                        Token::COMPUTED_PROP // complicated
                        | Token::OBJECT_SPREAD
                        | Token::COMPUTED_FIELD_DEF
                        | Token::BLOCK => {} // ES2022 static initialization blocks are never methods.
                        _ => panic!(
                            "Unexpected {} key: {}",
                            n.get_token(t),
                            k.to_string(t)
                        ),
                    }
                    key = k.get_next(t);
                }
            }
            Token::CALL => {
                // If a goog.reflect.objectProperty is used for a method's name, we can't assume
                // that the method can be safely inlined.
                let compiler: &AbstractCompiler = t.get_compiler();
                if compiler
                    .get_coding_convention()
                    .is_property_rename_function(compiler, n.get_first_child(compiler).unwrap())
                {
                    // Other code guarantees that getSecondChild() is a STRINGLIT
                    let name = n.get_second_child(compiler).unwrap().get_string(compiler);
                    self.outer.non_inlineable_properties.insert(name);
                }
            }
            _ => {}
        }
    }
}
