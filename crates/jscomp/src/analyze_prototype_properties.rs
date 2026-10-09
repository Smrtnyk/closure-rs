/*
 * Copyright 2006 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/AnalyzePrototypeProperties.java.

//! Port of AnalyzePrototypeProperties.java.
//!
//! Analyzes properties on prototypes.
//!
//! Uses a reference graph to analyze prototype properties. Each unique property name is
//! represented by a node in this graph. An edge from property A to property B means that there's
//! a GETPROP access of a property B on some object inside of a method named A.
//!
//! Global functions are also represented by nodes in this graph, with similar semantics.
//!
//! Rust shape: Java's NameInfo objects are graph node values compared by identity; they live in
//! the `name_infos` arena and the graph holds their `NameInfoId`. Java's `chunkGraph` field is
//! `compiler.getChunkGraph()` (its only caller, CrossChunkMethodMotion, passes that); the pass
//! reads it from the compiler after construction.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::graph::fixed_point_graph_traversal::{EdgeCallback, FixedPointGraphTraversal};
use crate::graph::graph::Graph;
use crate::graph::linked_directed_graph::LinkedDirectedGraph;
use crate::js_chunk::JSChunk;
use crate::js_chunk_graph::JSChunkGraph;
use crate::node_traversal::{
    AbstractPostOrderCallback, AbstractPostOrderCallbackInterface, Callback, NodeTraversal,
    ScopedCallback,
};
use crate::node_util::NodeUtil;
use crate::scope::ScopeId;
use crate::var::VarId;
use closure_rhino::check_state;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use std::collections::VecDeque;

/// Handle of a Java NameInfo object (identity).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NameInfoId(usize);

pub struct AnalyzePrototypeProperties {
    can_modify_externs: bool,
    anchor_unused_vars: bool,
    root_scope_uses_are_global: bool,

    first_chunk: Option<JSChunk>,

    // A graph where the nodes are property names or variable names,
    // and the edges signify the chunks where the property is referenced.
    // For example, if we had the code:
    //
    // Foo.prototype.bar = function(x) { x.baz(); }; // in chunk 2.;
    //
    // then this would be represented in the graph by a node representing
    // "bar", a node representing "baz", and an edge between them representing
    // chunk #2.
    //
    // Similarly, if we had:
    //
    // var scotch = function(f) { return f.age(); };
    //
    // then there would be a node for "scotch", a node for "age", and an edge
    // from scotch to age.
    symbol_graph: LinkedDirectedGraph<NameInfoId, Option<JSChunk>>,

    /// Owns every NameInfo (Rust-only).
    name_infos: Vec<NameInfo>,

    // A dummy node for representing global references.
    global_node: NameInfoId,

    // A dummy node for representing extern references.
    extern_node: NameInfoId,

    // A dummy node for representing all anonymous functions with no names.
    anonymous_node: NameInfoId,

    // All the real NameInfo for prototype properties, hashed by the name
    // of the property that they represent.
    property_name_info: IndexMap<JsString, NameInfoId>,

    // All the NameInfo for global functions, hashed by the name of the
    // global variable that it's assigned to.
    var_name_info: IndexMap<JsString, NameInfoId>,
}

impl AnalyzePrototypeProperties {
    // Constants for symbol types, for easier readability.
    // port: AnalyzePrototypeProperties#PROPERTY
    const PROPERTY: SymbolType = SymbolType::PROPERTY;
    // port: AnalyzePrototypeProperties#VAR
    const VAR: SymbolType = SymbolType::VAR;

    // Properties that are implicitly used as part of the JS language.
    // port: AnalyzePrototypeProperties#IMPLICITLY_USED_PROPERTIES
    const IMPLICITLY_USED_PROPERTIES: [&'static str; 3] = ["length", "toString", "valueOf"];

    /// Creates a new pass for analyzing prototype properties.
    ///
    /// @param chunk_graph The graph for resolving chunk dependencies.
    /// @param can_modify_externs If true, then we can move prototype properties that are declared
    ///     in the externs file.
    /// @param anchor_unused_vars If true, then we must mark all vars as referenced, even if they
    ///     are never used.
    /// @param root_scope_uses_are_global If true, all uses in root level scope are treated as
    ///     references from '[global]', even if they are assignments to a property.
    // port: AnalyzePrototypeProperties#AnalyzePrototypeProperties
    pub fn new(
        chunk_graph: Option<&JSChunkGraph>,
        can_modify_externs: bool,
        anchor_unused_vars: bool,
        root_scope_uses_are_global: bool,
    ) -> Self {
        let first_chunk = if chunk_graph.unwrap().get_chunk_count() > 1 {
            Some(chunk_graph.unwrap().get_root_chunk())
        } else {
            None
        };

        let mut this = Self {
            can_modify_externs,
            anchor_unused_vars,
            root_scope_uses_are_global,
            first_chunk,
            // LinkedDirectedGraph.createWithoutAnnotations(); the graph holds NameInfo handles, so
            // Java's String.valueOf(NameInfo) is not available to it (debug output only).
            symbol_graph: LinkedDirectedGraph::new_with_value_to_strings(
                false,
                false,
                |n| format!("NameInfo#{}", n.0),
                |e| {
                    e.as_ref()
                        .map_or_else(|| "null".to_string(), ToString::to_string)
                },
            ),
            name_infos: Vec::new(),
            global_node: NameInfoId(0),
            extern_node: NameInfoId(1),
            anonymous_node: NameInfoId(2),
            property_name_info: IndexMap::<_, _>::default(),
            var_name_info: IndexMap::<_, _>::default(),
        };
        this.global_node = this.new_name_info("[global]");
        this.extern_node = this.new_name_info("[extern]");
        this.anonymous_node = this.new_name_info("[anonymous]");

        let global_node = this.global_node;
        let extern_node = this.extern_node;
        this.name_infos[global_node.0].mark_reference(chunk_graph, None);
        this.name_infos[extern_node.0].mark_reference(chunk_graph, None);
        this.symbol_graph.create_node(global_node);
        this.symbol_graph.create_node(extern_node);

        for property in Self::IMPLICITLY_USED_PROPERTIES {
            let name_info = this.get_name_info_for_name(&JsString::from(property), Self::PROPERTY);
            match chunk_graph {
                None => {
                    this.symbol_graph.connect(extern_node, None, name_info);
                }
                Some(chunk_graph) => {
                    for chunk in chunk_graph.get_all_chunks() {
                        this.symbol_graph
                            .connect(extern_node, Some(chunk.clone()), name_info);
                    }
                }
            }
        }
        this
    }

    fn new_name_info(&mut self, name: &str) -> NameInfoId {
        self.name_infos.push(NameInfo::new(JsString::from(name)));
        NameInfoId(self.name_infos.len() - 1)
    }

    /// Returns the NameInfo object behind a handle (Rust-only accessor).
    pub fn get_name_info(&self, id: NameInfoId) -> &NameInfo {
        &self.name_infos[id.0]
    }

    /// Returns information on all prototype properties.
    // port: AnalyzePrototypeProperties#getAllNameInfo
    pub fn get_all_name_info(&self) -> Vec<NameInfoId> {
        let mut result: Vec<NameInfoId> = self.property_name_info.values().copied().collect();
        result.extend(self.var_name_info.values().copied());
        result
    }

    /// Gets the name info for the property or variable of a given name, and creates a new one if
    /// necessary.
    ///
    /// @param name The name of the symbol.
    /// @param type The type of symbol.
    // port: AnalyzePrototypeProperties#getNameInfoForName
    fn get_name_info_for_name(&mut self, name: &JsString, r#type: SymbolType) -> NameInfoId {
        let map = if r#type == Self::PROPERTY {
            &self.property_name_info
        } else {
            &self.var_name_info
        };
        if let Some(&name_info) = map.get(name) {
            name_info
        } else {
            self.name_infos.push(NameInfo::new(name.clone()));
            let name_info = NameInfoId(self.name_infos.len() - 1);
            let map = if r#type == Self::PROPERTY {
                &mut self.property_name_info
            } else {
                &mut self.var_name_info
            };
            map.insert(name.clone(), name_info);
            self.symbol_graph.create_node(name_info);
            name_info
        }
    }
}

impl CompilerPass for AnalyzePrototypeProperties {
    // port: AnalyzePrototypeProperties#process
    fn process(&mut self, compiler: &mut AbstractCompiler, extern_root: NodeId, root: NodeId) {
        check_state!(compiler.get_life_cycle_stage().is_normalized());
        if !self.can_modify_externs {
            NodeTraversal::traverse(
                compiler,
                extern_root,
                &mut AbstractPostOrderCallback::new(ProcessExternProperties { outer: self }),
            );
        }

        let mut process_properties = ProcessProperties {
            outer: self,
            symbol_stack: VecDeque::new(),
        };
        NodeTraversal::traverse(compiler, root, &mut process_properties);

        let entry = [self.extern_node, self.global_node];
        let mut t = FixedPointGraphTraversal::new_traversal(PropagateReferences {
            chunk_graph: compiler.get_chunk_graph(),
            name_infos: &mut self.name_infos,
        });
        t.compute_fixed_point_from_set(&mut self.symbol_graph, entry);
    }
}

struct ProcessProperties<'a> {
    outer: &'a mut AnalyzePrototypeProperties,
    // There are two types of context information on this stack:
    // 1) Every scope has a NameContext corresponding to its scope.
    //    Variables are given VAR contexts.
    //    Prototype properties are given PROPERTY contexts.
    //    The global scope is given the special [global] context.
    //    And function expressions that we aren't able to give a reasonable
    //    name are given a special [anonymous] context.
    // 2) Every assignment of a prototype property of a non-function is
    //    given a name context. These contexts do not have scopes.
    symbol_stack: VecDeque<NameContext>,
}

impl ScopedCallback for ProcessProperties<'_> {
    // port: AnalyzePrototypeProperties.ProcessProperties#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        let n = t.get_current_node().unwrap();
        let scope = t.get_scope();
        let root = scope.get_root_node(t.get_compiler());
        if root.is_function(t) {
            let prop_name = Self::get_prototype_property_name_from_r_value(t, n);
            if let Some(prop_name) = prop_name {
                let name = self
                    .outer
                    .get_name_info_for_name(&prop_name, AnalyzePrototypeProperties::PROPERTY);
                self.symbol_stack
                    .push_front(NameContext::new(name, Some(scope)));
            } else if self.is_global_function_declaration(t, n) {
                let parent = n.get_parent(t).unwrap();
                let name = if parent.is_name(t) {
                    parent.get_string(t) /* VAR */
                } else {
                    n.get_first_child(t).unwrap().get_string(t) /* named function */
                };
                let name_info = self
                    .outer
                    .get_name_info_for_name(&name, AnalyzePrototypeProperties::VAR);
                let hoist_scope = scope.get_closest_hoist_scope(t.get_compiler());
                self.symbol_stack
                    .push_front(NameContext::new(name_info, hoist_scope));
            } else {
                // NOTE(nicksantos): We use the same anonymous node for all
                // functions that do not have reasonable names. I can't remember
                // at the moment why we do this. I think it's because anonymous
                // nodes can never have in-edges. They're just there as a placeholder
                // for scope information, and do not matter in the edge propagation.
                self.symbol_stack
                    .push_front(NameContext::new(self.outer.anonymous_node, Some(scope)));
            }
        } else if t.in_global_scope() {
            self.symbol_stack
                .push_front(NameContext::new(self.outer.global_node, Some(scope)));
        } else {
            // TODO(moz): It's not yet clear if we need another kind of NameContext for block
            // scopes in ES6, use anonymous node for now and investigate later.
            // TODO(b/189993301): Test effects of pushing anonymousNode for member and computed
            // field defs in CrossChunkMethodMotion (and test class fields there as well)
            check_state!(
                NodeUtil::creates_block_scope(t, root)
                    || root.is_module_body(t)
                    || root.is_computed_field_def(t)
                    || root.is_member_field_def(t),
                "%s",
                scope.to_string(t.get_compiler())
            );
            self.symbol_stack
                .push_front(NameContext::new(self.outer.anonymous_node, Some(scope)));
        }
    }

    // port: AnalyzePrototypeProperties.ProcessProperties#exitScope
    fn exit_scope(&mut self, _t: &mut NodeTraversal<'_>) {
        self.symbol_stack.pop_front();
    }
}

impl Callback for ProcessProperties<'_> {
    // port: AnalyzePrototypeProperties.ProcessProperties#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if !self.outer.root_scope_uses_are_global {
            // Process assignment of a non-function expression to a prototype property.
            let prop_name = Self::process_non_function_prototype_assign(t, n, parent);
            if let Some(prop_name) = prop_name {
                let name = self
                    .outer
                    .get_name_info_for_name(&prop_name, AnalyzePrototypeProperties::PROPERTY);
                self.symbol_stack.push_front(NameContext::new(name, None));
            }
        }
        true
    }

    // port: AnalyzePrototypeProperties.ProcessProperties#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        let mut fall_through_to_object_pattern = false;
        match n.get_token(t) {
            Token::SUPER => {
                // Example:
                // class X extends Y {
                //   method() {
                //     return () => super.x;
                //   }
                // }
                // Names associated with the arrow function, the method body, and the method
                // itself should be marked as referencing super, but not the class definition or
                // anything containing it.
                for i in 0..self.symbol_stack.len() {
                    let context = self.symbol_stack[i];
                    self.outer.name_infos[context.name.0].references_super = true;
                    let scope_root = context.scope.unwrap().get_root_node(t.get_compiler());
                    if NodeUtil::is_method_declaration(t, scope_root) {
                        break;
                    }
                }
            }

            Token::OPTCHAIN_GETPROP => {
                let chunk = t.get_chunk();
                self.add_symbol_use(
                    &n.get_string(t),
                    chunk,
                    AnalyzePrototypeProperties::PROPERTY,
                );
            }

            Token::GETPROP => {
                let prop_name = n.get_string(t);

                if n.is_qualified_name(t) {
                    if prop_name == "prototype" {
                        if self.handle_possible_assignment_to_prototype(t, n) {
                            // The reference is being assigned to not read from, so don't record
                            // this as a reference.
                            return;
                        }
                    } else if t
                        .get_compiler()
                        .get_coding_convention()
                        .is_exported(&prop_name, /* local= */ false)
                    {
                        // TODO(bradfordcsmith): We don't seem to have any tests that cover this
                        // case. This class has no unit tests of its own and it is only used by
                        // CrossChunkMethodMotion.
                        let chunk = t.get_chunk();
                        self.add_global_use_of_symbol(
                            &prop_name,
                            chunk,
                            AnalyzePrototypeProperties::PROPERTY,
                        );
                        return;
                    } else {
                        let parent = parent.unwrap();
                        if parent.is_assign(t) && Some(n) == parent.get_first_child(t) {
                            let r_value_name = Self::get_prototype_property_name_from_r_value(t, n);
                            if r_value_name.is_some() {
                                // e.g. `Foo.prototype.bar = something`
                                // We record the declaration of `bar` when we look at the
                                // `Foo.prototype` Node via the call to
                                // handlePossibleAssignmentToPrototype() above.
                                // Now that we're looking at the whole `Foo.prototype.bar` node,
                                // we just need to make sure we don't record it as a read
                                // reference.
                                return;
                            }
                        }
                    }
                }

                let chunk = t.get_chunk();
                self.add_symbol_use(&prop_name, chunk, AnalyzePrototypeProperties::PROPERTY);
            }

            Token::OBJECTLIT => {
                // Make sure that we're not handling object literals being
                // assigned to a prototype, as in:
                // Foo.prototype = {bar: 3, baz: 5};
                let l_value_name =
                    NodeUtil::get_best_l_value_name(t, NodeUtil::get_best_l_value(t, n));
                if l_value_name
                    .as_ref()
                    .is_some_and(|name| name.ends_with(".prototype"))
                {
                    return;
                }

                // Fall through.
                fall_through_to_object_pattern = true;
            }

            Token::OBJECT_PATTERN => {
                fall_through_to_object_pattern = true;
            }

            Token::CLASS => {
                let class_members = n.get_last_child(t).unwrap();
                let mut child = class_members.get_first_child(t);
                while let Some(c) = child {
                    if c.is_member_function_def(t) || c.is_setter_def(t) || c.is_getter_def(t) {
                        self.process_member_def(t, c);
                    }
                    child = c.get_next(t);
                }
            }

            Token::NAME => {
                let name = n.get_string(t);

                let scope = t.get_scope();
                let var = scope.get_var(t.get_compiler(), name.clone());
                if let Some(var) = var {
                    // Only process global functions.
                    if var.is_global(t.get_compiler()) {
                        let initial_value = var.get_initial_value(t.get_compiler());
                        if initial_value.is_some_and(|value| value.is_function(t)) {
                            if t.in_global_hoist_scope() {
                                if !self.process_global_function_declaration(t, n, var) {
                                    let chunk = t.get_chunk();
                                    self.add_global_use_of_symbol(
                                        &name,
                                        chunk,
                                        AnalyzePrototypeProperties::VAR,
                                    );
                                }
                            } else {
                                let chunk = t.get_chunk();
                                self.add_symbol_use(&name, chunk, AnalyzePrototypeProperties::VAR);
                            }
                        }

                        // If it is not a global, it might be accessing a local of the outer
                        // scope. If that's the case the functions between the variable's
                        // declaring scope and the variable reference scope cannot be moved.
                    } else if var.get_scope(t.get_compiler()) != t.get_scope() {
                        let var_scope = var.get_scope(t.get_compiler());
                        for i in 0..self.symbol_stack.len() {
                            let context = self.symbol_stack[i];
                            if context.scope == Some(var_scope) {
                                break;
                            }

                            self.outer.name_infos[context.name.0].read_closure_variables = true;
                        }
                    }
                }
            }

            _ => {}
        }
        if fall_through_to_object_pattern {
            // `var x = {a: 1, b: 2}` and `var {a: x, b: y} = obj;`
            // should count as a use of property a and b.
            let mut prop_node = n.get_first_child(t);
            while let Some(p) = prop_node {
                match p.get_token(t) {
                    Token::COMPUTED_PROP
                    | Token::ITER_REST
                    | Token::OBJECT_REST
                    | Token::ITER_SPREAD
                    | Token::OBJECT_SPREAD => {}

                    Token::STRING_KEY
                    | Token::GETTER_DEF
                    | Token::SETTER_DEF
                    | Token::MEMBER_FUNCTION_DEF => {
                        if !p.is_quoted_string_key(t) {
                            // May be STRING, GET, or SET, but NUMBER isn't interesting.
                            let chunk = t.get_chunk();
                            self.add_symbol_use(
                                &p.get_string(t),
                                chunk,
                                AnalyzePrototypeProperties::PROPERTY,
                            );
                        }
                    }

                    _ => {
                        panic!(
                            "Unexpected child of {}: {}",
                            n.get_token(t),
                            p.to_string_tree(t)
                        );
                    }
                }
                prop_node = p.get_next(t);
            }
        }
        // Process prototype assignments to non-functions.
        if !self.outer.root_scope_uses_are_global
            && Self::process_non_function_prototype_assign(t, n, parent).is_some()
        {
            self.symbol_stack.pop_front();
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ProcessProperties<'_> {
    // port: AnalyzePrototypeProperties.ProcessProperties#addSymbolUse
    fn add_symbol_use(&mut self, name: &JsString, chunk: Option<JSChunk>, r#type: SymbolType) {
        let info = self.outer.get_name_info_for_name(name, r#type);
        let mut def = None;
        // Skip all anonymous nodes. We care only about symbols with names.
        for context in &self.symbol_stack {
            def = Some(context.name);
            if context.name != self.outer.anonymous_node {
                break;
            }
        }
        let def = def.unwrap();
        if def != info {
            self.outer.symbol_graph.connect(def, chunk, info);
        }
    }

    /// If this is a non-function prototype assign, return the prop name. Otherwise, return null.
    // port: AnalyzePrototypeProperties.ProcessProperties#processNonFunctionPrototypeAssign
    fn process_non_function_prototype_assign(
        ast: &Ast,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> Option<JsString> {
        if Self::is_assign_r_value(ast, n, parent) && !n.is_function(ast) {
            return Self::get_prototype_property_name_from_r_value(ast, n);
        }
        None
    }

    /// Determines whether `n` is the FUNCTION node in a global function declaration.
    // port: AnalyzePrototypeProperties.ProcessProperties#isGlobalFunctionDeclaration
    fn is_global_function_declaration(&self, t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        // Make sure we're not in a function scope, or if we are then the function we're looking
        // at is defined in the global scope.
        if !(t.in_global_hoist_scope()
            || (n.is_function(t) && t.get_scope_root() == Some(n) && {
                let scope = t.get_scope();
                let compiler = t.get_compiler();
                scope
                    .get_parent(compiler)
                    .unwrap()
                    .get_closest_hoist_scope(compiler)
                    .unwrap()
                    .is_global(compiler)
            }))
        {
            return false;
        }

        NodeUtil::is_function_declaration(t, n)
            || (n.is_function(t) && n.get_parent(t).unwrap().is_name(t))
    }

    /// Returns true if this is the r-value of an assignment.
    // port: AnalyzePrototypeProperties.ProcessProperties#isAssignRValue
    fn is_assign_r_value(ast: &Ast, n: NodeId, parent: Option<NodeId>) -> bool {
        parent.is_some_and(|parent| parent.is_assign(ast) && parent.get_first_child(ast) != Some(n))
    }

    /// Returns the name of a prototype property being assigned to this r-value.
    ///
    /// Returns null if this is not the R-value of a prototype property, or if the R-value is used
    /// in multiple expressions (i.e., if there's a prototype property assignment in a more complex
    /// expression).
    // port: AnalyzePrototypeProperties.ProcessProperties#getPrototypePropertyNameFromRValue
    fn get_prototype_property_name_from_r_value(ast: &Ast, r_value: NodeId) -> Option<JsString> {
        let l_value = NodeUtil::get_best_l_value(ast, r_value);
        let l_value = l_value?;
        if !((NodeUtil::may_be_object_lit_key(ast, l_value) && !l_value.is_quoted_string_key(ast))
            || NodeUtil::is_expr_assign(ast, l_value.get_grandparent(ast).unwrap()))
        {
            return None;
        }

        let l_value_name = NodeUtil::get_best_l_value_name(ast, Some(l_value))?;

        let last_dot = l_value_name.last_index_of_char(u16::from(b'.'));
        if last_dot == -1 {
            return None;
        }
        let last_dot = last_dot as usize;

        let first_part = l_value_name.substring(0, last_dot);
        if !first_part.ends_with(".prototype") {
            return None;
        }

        Some(l_value_name.substring(last_dot + 1, l_value_name.length()))
    }

    /// Processes a NAME node to see if it's a global function declaration. If it is, record it
    /// and return true. Otherwise, return false.
    // port: AnalyzePrototypeProperties.ProcessProperties#processGlobalFunctionDeclaration
    fn process_global_function_declaration(
        &mut self,
        t: &mut NodeTraversal<'_>,
        name_node: NodeId,
        v: VarId,
    ) -> bool {
        let first_child = name_node.get_first_child(t);
        let parent = name_node.get_parent(t).unwrap();

        if
        // Check for a named FUNCTION.
        self.is_global_function_declaration(t, parent)
            // Check for a VAR declaration.
            || first_child.is_some_and(|first_child| self.is_global_function_declaration(t, first_child))
        {
            let name = name_node.get_string(t);
            let chunk = t.get_chunk();
            let symbol =
                Symbol::GlobalFunction(GlobalFunction::new(t.get_compiler(), name_node, v, chunk));
            let name_info = self
                .outer
                .get_name_info_for_name(&name, AnalyzePrototypeProperties::VAR);
            self.outer.name_infos[name_info.0]
                .get_declarations_mut()
                .push_back(symbol);

            // If the function name is exported, we should create an edge here
            // so that it's never removed.
            if t.get_compiler()
                .get_coding_convention()
                .is_exported(&name, /* local= */ false)
                || self.outer.anchor_unused_vars
            {
                let chunk = t.get_chunk();
                self.add_global_use_of_symbol(&name, chunk, AnalyzePrototypeProperties::VAR);
            }

            return true;
        }
        false
    }

    /// Examines a qualified name ending in `.prototype`.
    ///
    /// If it is part of an assignment like `foo.prototype = {}` or `foo.prototype.bar = x`,
    /// record this reference as the definition of one or more prototype properties and return
    /// `true`.
    ///
    /// @param ref A reference to some qualified name that ends with `.prototype`
    /// @return True if a declaration was added.
    // port: AnalyzePrototypeProperties.ProcessProperties#handlePossibleAssignmentToPrototype
    fn handle_possible_assignment_to_prototype(
        &mut self,
        t: &mut NodeTraversal<'_>,
        r#ref: NodeId,
    ) -> bool {
        let root = NodeUtil::get_root_of_qualified_name(t, r#ref);

        let n = r#ref.get_parent(t).unwrap();
        match n.get_token(t) {
            Token::GETPROP => {
                // Foo.prototype.getBar = function() { ... }
                let parent = n.get_parent(t).unwrap();
                let grand_parent = parent.get_parent(t).unwrap();

                if NodeUtil::is_expr_assign(t, grand_parent)
                    && NodeUtil::is_name_decl_or_simple_assign_lhs(t, n, parent)
                {
                    let root_var = Self::maybe_get_var(t, root);
                    let chunk = t.get_chunk();
                    let prop = Symbol::AssignmentPrototypeProperty(
                        AssignmentPrototypeProperty::new(grand_parent, root_var, chunk),
                    );
                    let name_info = self.outer.get_name_info_for_name(
                        &n.get_string(t),
                        AnalyzePrototypeProperties::PROPERTY,
                    );
                    self.outer.name_infos[name_info.0]
                        .get_declarations_mut()
                        .push_back(prop);
                    return true;
                }
            }
            Token::ASSIGN => {
                // Foo.prototype = { "getBar" : function() { ... } }
                let map = n.get_second_child(t).unwrap();
                if map.is_object_lit(t) {
                    let mut key = map.get_first_child(t);
                    while let Some(k) = key {
                        if !k.is_quoted_string_key(t) && !k.is_computed_prop(t) {
                            // We won't consider quoted or computed properties for any kind of
                            // modification, so key may be STRING_KEY, GETTER_DEF, SETTER_DEF, or
                            // MEMBER_FUNCTION_DEF
                            let name = k.get_string(t);
                            let root_var = Self::maybe_get_var(t, root);
                            let chunk = t.get_chunk();
                            let prop =
                                Symbol::LiteralPrototypeProperty(LiteralPrototypeProperty::new(
                                    k.get_first_child(t).unwrap(),
                                    n,
                                    root_var,
                                    chunk,
                                ));
                            let name_info = self.outer.get_name_info_for_name(
                                &name,
                                AnalyzePrototypeProperties::PROPERTY,
                            );
                            self.outer.name_infos[name_info.0]
                                .get_declarations_mut()
                                .push_back(prop);
                        }
                        key = k.get_next(t);
                    }
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    // port: AnalyzePrototypeProperties.ProcessProperties#processMemberDef
    fn process_member_def(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        check_state!(n.is_member_function_def(t) || n.is_getter_def(t) || n.is_setter_def(t));
        let name = n.get_string(t);
        // Don't want to add a declaration for constructors and static members
        // so they aren't removed
        if NodeUtil::is_es6_constructor_member_function_def(t, n) || n.is_static_member(t) {
            return;
        }

        let class_name_node = NodeUtil::get_name_node(t, n.get_grandparent(t).unwrap());
        let var = match class_name_node {
            Some(class_name_node) if class_name_node.is_name(t) => {
                let scope = t.get_scope();
                let class_name = class_name_node.get_string(t);
                scope.get_var(t.get_compiler(), class_name)
            }
            _ => None,
        };
        let chunk = t.get_chunk();
        let symbol = Symbol::ClassMemberFunction(ClassMemberFunction::new(t, n, var, chunk));
        let name_info = self
            .outer
            .get_name_info_for_name(&name, AnalyzePrototypeProperties::PROPERTY);
        self.outer.name_infos[name_info.0]
            .get_declarations_mut()
            .push_back(symbol);
    }

    // port: AnalyzePrototypeProperties.ProcessProperties#maybeGetVar
    fn maybe_get_var(t: &mut NodeTraversal<'_>, maybe_name: NodeId) -> Option<VarId> {
        if maybe_name.is_name(t) {
            let scope = t.get_scope();
            let name = maybe_name.get_string(t);
            scope.get_var(t.get_compiler(), name)
        } else {
            None
        }
    }

    // port: AnalyzePrototypeProperties.ProcessProperties#addGlobalUseOfSymbol
    fn add_global_use_of_symbol(
        &mut self,
        name: &JsString,
        chunk: Option<JSChunk>,
        r#type: SymbolType,
    ) {
        let global_node = self.outer.global_node;
        let name_info = self.outer.get_name_info_for_name(name, r#type);
        self.outer
            .symbol_graph
            .connect(global_node, chunk, name_info);
    }
}

struct ProcessExternProperties<'a> {
    outer: &'a mut AnalyzePrototypeProperties,
}

impl AbstractPostOrderCallbackInterface for ProcessExternProperties<'_> {
    // port: AnalyzePrototypeProperties.ProcessExternProperties#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_get_prop(t) {
            let extern_node = self.outer.extern_node;
            let first_chunk = self.outer.first_chunk.clone();
            let name_info = self
                .outer
                .get_name_info_for_name(&n.get_string(t), AnalyzePrototypeProperties::PROPERTY);
            self.outer
                .symbol_graph
                .connect(extern_node, first_chunk, name_info);
        } else if n.is_member_function_def(t) || n.is_getter_def(t) || n.is_setter_def(t) {
            // As of 2019-08-29 the only user of this class is CrossChunkMethodMotion, which never
            // moves static methods, but that could change. So, we're intentionally including
            // static methods, static getters, and static setters here, because there are cases
            // where they could act like prototype properties.
            //
            // e.g.
            // // externs.js
            // class Foo {
            //   static foo() {}
            // }
            //
            // // src.js
            // /** @record */
            // class ObjWithFooMethod {
            //   foo() {}
            // }
            // /** @type {!ObjWithFooMethod} */
            // let objWithFooMethod = Foo; // yes, this is valid
            //
            let extern_node = self.outer.extern_node;
            let first_chunk = self.outer.first_chunk.clone();
            let name_info = self
                .outer
                .get_name_info_for_name(&n.get_string(t), AnalyzePrototypeProperties::PROPERTY);
            self.outer
                .symbol_graph
                .connect(extern_node, first_chunk, name_info);
        }
    }
}

struct PropagateReferences<'a> {
    chunk_graph: Option<&'a JSChunkGraph>,
    name_infos: &'a mut Vec<NameInfo>,
}

impl<G> EdgeCallback<NameInfoId, Option<JSChunk>, G> for PropagateReferences<'_> {
    // port: AnalyzePrototypeProperties.PropagateReferences#traverseEdge
    fn traverse_edge(
        &mut self,
        _graph: &mut G,
        start: NameInfoId,
        edge: Option<JSChunk>,
        dest: NameInfoId,
    ) -> bool {
        if self.name_infos[start.0].is_referenced() {
            let start_chunk = self.name_infos[start.0].get_deepest_common_chunk_ref();
            if let Some(start_chunk) = start_chunk.filter(|start_chunk| {
                self.chunk_graph
                    .unwrap()
                    .depends_on(start_chunk, edge.as_ref().unwrap())
            }) {
                self.name_infos[dest.0].mark_reference(self.chunk_graph, Some(start_chunk))
            } else {
                self.name_infos[dest.0].mark_reference(self.chunk_graph, edge)
            }
        } else {
            false
        }
    }
}

/// The declaration of an abstract symbol (Java interfaces Symbol, Property and PrototypeProperty
/// with their four implementations).
pub enum Symbol {
    GlobalFunction(GlobalFunction),
    ClassMemberFunction(ClassMemberFunction),
    AssignmentPrototypeProperty(AssignmentPrototypeProperty),
    LiteralPrototypeProperty(LiteralPrototypeProperty),
}

impl Symbol {
    /// The variable for the root of this symbol.
    // port: AnalyzePrototypeProperties.Symbol#getRootVar
    pub fn get_root_var(&self) -> Option<VarId> {
        match self {
            Symbol::GlobalFunction(s) => s.get_root_var(),
            Symbol::ClassMemberFunction(s) => s.get_root_var(),
            Symbol::AssignmentPrototypeProperty(s) => s.get_root_var(),
            Symbol::LiteralPrototypeProperty(s) => s.get_root_var(),
        }
    }

    /// Returns the chunk where this appears.
    // port: AnalyzePrototypeProperties.Symbol#getChunk
    pub fn get_chunk(&self) -> Option<&JSChunk> {
        match self {
            Symbol::GlobalFunction(s) => s.get_chunk(),
            Symbol::ClassMemberFunction(s) => s.get_chunk(),
            Symbol::AssignmentPrototypeProperty(s) => s.get_chunk(),
            Symbol::LiteralPrototypeProperty(s) => s.get_chunk(),
        }
    }

    /// Java `symbol instanceof Property`.
    pub fn is_property(&self) -> bool {
        !matches!(self, Symbol::GlobalFunction(_))
    }

    /// Java `symbol instanceof PrototypeProperty`.
    pub fn is_prototype_property(&self) -> bool {
        matches!(
            self,
            Symbol::AssignmentPrototypeProperty(_) | Symbol::LiteralPrototypeProperty(_)
        )
    }

    /// Returns the GETPROP node that refers to the prototype.
    // port: AnalyzePrototypeProperties.PrototypeProperty#getPrototype
    pub fn get_prototype(&self, ast: &Ast) -> NodeId {
        match self {
            Symbol::AssignmentPrototypeProperty(s) => s.get_prototype(ast),
            Symbol::LiteralPrototypeProperty(s) => s.get_prototype(ast),
            _ => panic!("not a PrototypeProperty"),
        }
    }

    /// Returns the value of this property.
    // port: AnalyzePrototypeProperties.PrototypeProperty#getValue
    pub fn get_value(&self, ast: &Ast) -> NodeId {
        match self {
            Symbol::AssignmentPrototypeProperty(s) => s.get_value(ast),
            Symbol::LiteralPrototypeProperty(s) => s.get_value(),
            _ => panic!("not a PrototypeProperty"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SymbolType {
    PROPERTY,
    VAR,
}

/// A function initialized as a VAR statement or LET AND CONST and global or a function
/// declaration.
pub struct GlobalFunction {
    var: VarId,
    chunk: Option<JSChunk>,
}

impl GlobalFunction {
    // port: AnalyzePrototypeProperties.GlobalFunction#GlobalFunction
    fn new(
        compiler: &AbstractCompiler,
        name_node: NodeId,
        var: VarId,
        chunk: Option<JSChunk>,
    ) -> Self {
        let parent = name_node.get_parent(compiler);
        check_state!(
            (NodeUtil::is_name_declaration(compiler, parent) && var.is_global(compiler))
                || NodeUtil::is_function_declaration(compiler, parent.unwrap())
        );
        Self { var, chunk }
    }

    // port: AnalyzePrototypeProperties.GlobalFunction#getRootVar
    pub fn get_root_var(&self) -> Option<VarId> {
        Some(self.var)
    }

    // port: AnalyzePrototypeProperties.GlobalFunction#getChunk
    pub fn get_chunk(&self) -> Option<&JSChunk> {
        self.chunk.as_ref()
    }
}

pub struct ClassMemberFunction {
    node: NodeId,
    var: Option<VarId>,
    chunk: Option<JSChunk>,
}

impl ClassMemberFunction {
    // port: AnalyzePrototypeProperties.ClassMemberFunction#ClassMemberFunction
    fn new(ast: &Ast, node: NodeId, var: Option<VarId>, chunk: Option<JSChunk>) -> Self {
        check_state!(node.get_parent(ast).unwrap().is_class_members(ast));
        check_state!(
            node.is_member_function_def(ast) || node.is_setter_def(ast) || node.is_getter_def(ast)
        );
        Self { node, var, chunk }
    }

    // port: AnalyzePrototypeProperties.ClassMemberFunction#getRootVar
    pub fn get_root_var(&self) -> Option<VarId> {
        self.var
    }

    // port: AnalyzePrototypeProperties.ClassMemberFunction#getChunk
    pub fn get_chunk(&self) -> Option<&JSChunk> {
        self.chunk.as_ref()
    }

    /// Returns the function node within the definition.
    // port: AnalyzePrototypeProperties.ClassMemberFunction#getFunctionNode
    pub fn get_function_node(&self, ast: &Ast) -> NodeId {
        self.node.get_only_child(ast)
    }

    /// Returns the MEMBER_FUNCTION_DEF, GETTER_DEF, or SETTER_DEF node that defines the property.
    // port: AnalyzePrototypeProperties.ClassMemberFunction#getDefinitionNode
    pub fn get_definition_node(&self) -> NodeId {
        self.node
    }
}

/// Properties created via EXPR assignment:
///
/// ```text
/// function Foo() { ... };
/// Foo.prototype.bar = function() { ... };
/// ```
pub struct AssignmentPrototypeProperty {
    expr_node: NodeId,
    root_var: Option<VarId>,
    chunk: Option<JSChunk>,
}

impl AssignmentPrototypeProperty {
    /// @param node An EXPR node.
    // port: AnalyzePrototypeProperties.AssignmentPrototypeProperty#AssignmentPrototypeProperty
    fn new(node: NodeId, root_var: Option<VarId>, chunk: Option<JSChunk>) -> Self {
        Self {
            expr_node: node,
            root_var,
            chunk,
        }
    }

    // port: AnalyzePrototypeProperties.AssignmentPrototypeProperty#getRootVar
    pub fn get_root_var(&self) -> Option<VarId> {
        self.root_var
    }

    // port: AnalyzePrototypeProperties.AssignmentPrototypeProperty#getPrototype
    pub fn get_prototype(&self, ast: &Ast) -> NodeId {
        self.get_assign_node(ast)
            .get_first_first_child(ast)
            .unwrap()
    }

    // port: AnalyzePrototypeProperties.AssignmentPrototypeProperty#getValue
    pub fn get_value(&self, ast: &Ast) -> NodeId {
        self.get_assign_node(ast).get_last_child(ast).unwrap()
    }

    // port: AnalyzePrototypeProperties.AssignmentPrototypeProperty#getAssignNode
    fn get_assign_node(&self, ast: &Ast) -> NodeId {
        self.expr_node.get_first_child(ast).unwrap()
    }

    // port: AnalyzePrototypeProperties.AssignmentPrototypeProperty#getChunk
    pub fn get_chunk(&self) -> Option<&JSChunk> {
        self.chunk.as_ref()
    }
}

/// Properties created via object literals:
///
/// ```text
/// function Foo() { ... };
/// Foo.prototype = {bar: function() { ... };
/// ```
pub struct LiteralPrototypeProperty {
    value: NodeId,
    assign: NodeId,
    root_var: Option<VarId>,
    chunk: Option<JSChunk>,
}

impl LiteralPrototypeProperty {
    // port: AnalyzePrototypeProperties.LiteralPrototypeProperty#LiteralPrototypeProperty
    fn new(value: NodeId, assign: NodeId, root_var: Option<VarId>, chunk: Option<JSChunk>) -> Self {
        Self {
            value,
            assign,
            root_var,
            chunk,
        }
    }

    // port: AnalyzePrototypeProperties.LiteralPrototypeProperty#getRootVar
    pub fn get_root_var(&self) -> Option<VarId> {
        self.root_var
    }

    // port: AnalyzePrototypeProperties.LiteralPrototypeProperty#getPrototype
    pub fn get_prototype(&self, ast: &Ast) -> NodeId {
        self.assign.get_first_child(ast).unwrap()
    }

    // port: AnalyzePrototypeProperties.LiteralPrototypeProperty#getValue
    pub fn get_value(&self) -> NodeId {
        self.value
    }

    // port: AnalyzePrototypeProperties.LiteralPrototypeProperty#getChunk
    pub fn get_chunk(&self) -> Option<&JSChunk> {
        self.chunk.as_ref()
    }
}

/// The context of the current name. This includes the NameInfo and the scope if it is a scope
/// defining name (function).
#[derive(Clone, Copy)]
struct NameContext {
    name: NameInfoId,

    // If this is a function context, then scope will be the scope of the
    // corresponding function. Otherwise, it will be null.
    scope: Option<ScopeId>,
}

impl NameContext {
    // port: AnalyzePrototypeProperties.NameContext#NameContext
    fn new(name: NameInfoId, scope: Option<ScopeId>) -> Self {
        Self { name, scope }
    }
}

/// Information on all properties or global variables of a given name.
pub struct NameInfo {
    pub name: JsString,

    referenced: bool,
    declarations: VecDeque<Symbol>,
    deepest_common_chunk_ref: Option<JSChunk>,

    // True if this property is a function that reads a variable from an
    // outer scope which isn't the global scope.
    read_closure_variables: bool,

    // does the definition refer to `super`?
    // We cannot move references to `super` outside of the class body.
    references_super: bool,
}

impl NameInfo {
    /// Constructs a new NameInfo.
    ///
    /// @param name The name of the property that this represents. May be null to signify dummy
    ///     nodes in the property graph.
    // port: AnalyzePrototypeProperties.NameInfo#NameInfo
    fn new(name: JsString) -> Self {
        Self {
            name,
            referenced: false,
            declarations: VecDeque::new(),
            deepest_common_chunk_ref: None,
            read_closure_variables: false,
            references_super: false,
        }
    }

    /// Determines whether we've marked a reference to this property name.
    // port: AnalyzePrototypeProperties.NameInfo#isReferenced
    pub fn is_referenced(&self) -> bool {
        self.referenced
    }

    /// Determines whether it reads a closure variable.
    // port: AnalyzePrototypeProperties.NameInfo#readsClosureVariables
    pub fn reads_closure_variables(&self) -> bool {
        self.read_closure_variables
    }

    /// Does the definition refer to `super`?
    // port: AnalyzePrototypeProperties.NameInfo#referencesSuper
    pub fn references_super(&self) -> bool {
        self.references_super
    }

    /// Mark a reference in a given chunk to this property name, and record the deepest common
    /// chunk reference.
    ///
    /// @param chunk The chunk where it was referenced.
    /// @return Whether the name info has changed.
    // port: AnalyzePrototypeProperties.NameInfo#markReference
    fn mark_reference(
        &mut self,
        chunk_graph: Option<&JSChunkGraph>,
        chunk: Option<JSChunk>,
    ) -> bool {
        let mut has_changed = false;
        if !self.referenced {
            self.referenced = true;
            has_changed = true;
        }

        let original_deepest_common = self.deepest_common_chunk_ref.clone();

        if let Some(deepest_common_chunk_ref) = &self.deepest_common_chunk_ref {
            self.deepest_common_chunk_ref = chunk_graph
                .unwrap()
                .get_deepest_common_dependency_inclusive(deepest_common_chunk_ref, &chunk.unwrap());
        } else {
            self.deepest_common_chunk_ref = chunk;
        }

        if original_deepest_common != self.deepest_common_chunk_ref {
            has_changed = true;
        }

        has_changed
    }

    /// Returns the deepest common chunk of all the references to this property.
    // port: AnalyzePrototypeProperties.NameInfo#getDeepestCommonChunkRef
    pub fn get_deepest_common_chunk_ref(&self) -> Option<JSChunk> {
        self.deepest_common_chunk_ref.clone()
    }

    /// Returns a mutable collection of all the prototype property declarations of this property
    /// name.
    // port: AnalyzePrototypeProperties.NameInfo#getDeclarations
    pub fn get_declarations(&self) -> &VecDeque<Symbol> {
        &self.declarations
    }

    fn get_declarations_mut(&mut self) -> &mut VecDeque<Symbol> {
        &mut self.declarations
    }
}

impl std::fmt::Display for NameInfo {
    // port: AnalyzePrototypeProperties.NameInfo#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name.to_string_lossy())
    }
}
