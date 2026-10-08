/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CrossChunkReferenceCollector.java.

//! Port of CrossChunkReferenceCollector.java: collects global variable references for use by
//! CrossChunkCodeMotion.

use crate::abstract_compiler::AbstractCompiler;
use crate::basic_block::BasicBlock;
use crate::compiler_pass::CompilerPass;
use crate::cross_chunk_method_motion::CrossChunkMethodMotion;
use crate::js_chunk::JSChunk;
use crate::node_traversal::{Callback, NodeTraversal, ScopedCallback};
use crate::node_util::NodeUtil;
use crate::reference::Reference;
use crate::reference_collection::ReferenceCollection;
use crate::scope::ScopeId;
use crate::scope_creator::ScopeCreator;
use crate::var::VarId;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use closure_rhino::{check_not_null, check_state};
use indexmap::IndexMap;
use std::sync::Arc;

/// Collects global variable references for use by CrossChunkCodeMotion.
pub struct CrossChunkReferenceCollector<'a> {
    /// Maps global variable name to the corresponding Var object.
    vars_by_name: IndexMap<JsString, VarId>,

    /// Maps a given variable to a collection of references to that name. Note that Var objects
    /// are not stable across multiple traversals (unlike scope root or name).
    reference_map: IndexMap<VarId, ReferenceCollection>,

    /// The stack of basic blocks and scopes the current traversal is in.
    block_stack: Vec<Arc<BasicBlock>>,

    /// List of all top-level statements in the order they appear in the AST.
    top_level_statements: Vec<TopLevelStatement>,

    /// Taken out of its slot while the traversal borrows this callback (Rust-only).
    scope_creator: Option<Box<dyn ScopeCreator + 'a>>,

    statement_counter: i32,
    top_level_statement_draft: Option<TopLevelStatementDraft>,
}

impl<'a> CrossChunkReferenceCollector<'a> {
    /// Constructor initializes block stack.
    // port: CrossChunkReferenceCollector#CrossChunkReferenceCollector
    pub fn new(creator: Box<dyn ScopeCreator + 'a>) -> Self {
        Self {
            vars_by_name: IndexMap::new(),
            reference_map: IndexMap::new(),
            block_stack: Vec::new(),
            top_level_statements: Vec::new(),
            scope_creator: Some(creator),
            statement_counter: 0,
            top_level_statement_draft: None,
        }
    }

    // port: CrossChunkReferenceCollector#process(Node)
    pub fn process_root(&mut self, compiler: &mut AbstractCompiler, root: NodeId) {
        check_state!(
            self.top_level_statements.is_empty(),
            "process() called more than once"
        );
        let mut scope_creator = self.scope_creator.take().unwrap();
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(self)
            .set_scope_creator(&mut *scope_creator)
            .traverse(root);
        self.scope_creator = Some(scope_creator);
    }

    /// Gets the variables that were referenced in this callback.
    // port: CrossChunkReferenceCollector#getAllSymbols
    pub fn get_all_symbols(&self) -> impl Iterator<Item = VarId> + '_ {
        self.reference_map.keys().copied()
    }

    /// Gets the reference collection for the given variable.
    // port: CrossChunkReferenceCollector#getReferences
    pub fn get_references(&self, v: VarId) -> Option<&ReferenceCollection> {
        self.reference_map.get(&v)
    }

    // port: CrossChunkReferenceCollector#getGlobalVariableNamesMap
    pub fn get_global_variable_names_map(&self) -> IndexMap<JsString, VarId> {
        self.vars_by_name.clone()
    }

    // port: CrossChunkReferenceCollector#initializeDraftStatement
    fn initialize_draft_statement(
        &mut self,
        compiler: &AbstractCompiler,
        chunk: Option<JSChunk>,
        statement_node: NodeId,
    ) -> TopLevelStatementDraft {
        let original_order = self.statement_counter;
        self.statement_counter += 1;
        let mut draft = TopLevelStatementDraft::new(original_order, chunk, statement_node);
        // Determine whether this statement declares a name or not.
        // If so, save its name node and value node, if any.
        if NodeUtil::is_name_declaration(compiler, Some(statement_node)) {
            // variable declaration
            draft.declared_name_node = statement_node.get_first_child(compiler);
            draft.declared_value_node = statement_node.get_first_first_child(compiler);
        } else if statement_node.is_class(compiler) {
            draft.declared_name_node = statement_node.get_first_child(compiler);
            draft.declared_value_node = Some(statement_node);
        } else if statement_node.is_function(compiler) {
            // function declaration
            draft.declared_name_node = statement_node.get_first_child(compiler);
            draft.declared_value_node = Some(statement_node);
        } else if statement_node.is_expr_result(compiler) {
            let expr = check_not_null!(statement_node.get_first_child(compiler));
            if expr.is_assign(compiler) {
                let lhs = check_not_null!(expr.get_first_child(compiler));
                let rhs = check_not_null!(expr.get_second_child(compiler));
                if lhs.is_name(compiler) {
                    // `varName = value;`
                    draft.declared_name_node = Some(lhs);
                    draft.declared_value_node = Some(rhs);
                } else if lhs.is_get_prop(compiler)
                    || Self::is_potentially_moveable_get_elem(compiler, lhs)
                {
                    let mut name_node = check_not_null!(lhs.get_first_child(compiler));
                    while name_node.is_get_prop(compiler) {
                        name_node = check_not_null!(name_node.get_first_child(compiler));
                    }
                    if name_node.is_name(compiler) {
                        // `varName.some.property = value;`
                        // OR
                        // `varName[someProperty] = value;`
                        draft.declared_name_node = Some(name_node);
                        draft.declared_value_node = Some(rhs);
                    }
                }
            } else if expr.is_call(compiler) {
                let mut name_node = None;
                let mut value_node = None;
                let relationship = compiler
                    .get_coding_convention()
                    .get_classes_defined_by_call(compiler, expr);
                if let Some(relationship) = relationship {
                    // Check for $jscomp.inherits(SubC, SuperC), goog.inherits(Sub, SuperC), etc.
                    let declared_name = relationship.subclass_name.clone();
                    let mut call_arg = expr.get_second_child(compiler);
                    while let Some(arg) = call_arg {
                        // We're assuming that the child class must be an argument to the function
                        // that establishes its inheritance, which is true for `goog.inherits()`
                        // and `$jscomp.inherits()`
                        // TODO(bradfordcsmith): handle cases like
                        // `goog.inherits(x.ChildClass, SuperClass)`
                        if arg.is_name(compiler) && declared_name == arg.get_string(compiler) {
                            name_node = Some(arg);
                            break;
                        }
                        call_arg = arg.get_next(compiler);
                    }
                } else if NodeUtil::is_object_define_properties_definition(compiler, expr) {
                    // Check for $jscomp$global.Object.defineProperties.
                    let target_object = expr.get_second_child(compiler).unwrap();

                    // Get the global var being referenced in the first parameter of
                    // Object.defineProperties. Can be 'Foo' or 'Foo.prototype'.
                    name_node = if target_object.is_name(compiler) {
                        Some(target_object)
                    } else {
                        target_object.get_first_child(compiler)
                    };

                    // Second parameter of defineProperties is the value node.
                    value_node = target_object.get_next(compiler);
                }

                if name_node.is_some() {
                    draft.declared_name_node = name_node;
                    draft.declared_value_node = value_node;
                }
            }
        }
        draft
    }

    // port: CrossChunkReferenceCollector#isPotentiallyMoveableGetElem
    fn is_potentially_moveable_get_elem(ast: &Ast, maybe_get_elem: NodeId) -> bool {
        if !maybe_get_elem.is_get_elem(ast) {
            return false;
        }

        // We'll only consider a GETELEM as potentially moveable if the property being indexed is
        // on `Symbol` (e.g. `Symbol.iterator`). We don't generally want to consider GETELEM nodes
        // as moveable as registries (ex. `GlobalRegistry['Foo'] = Foo`) will create a reference
        // from `Foo` to `GlobalRegistry`. As other pieces of code reference `GlobalRegistry` this
        // will ultimately cause a large cluster of symbols that would have to be considered as
        // moving together, but likely will no to be able to.
        // Therefore, we we'll only consider a subset of cases, like `Symbol.iterator`, as
        // moveable as these large reference clusters are unlikely in this case. This should allow
        // classes that define these properties to potentialy be moved to later chunks.
        let property = maybe_get_elem.get_last_child(ast).unwrap();
        if !property.is_get_prop(ast) {
            return false;
        }
        let name = property.get_first_child(ast).unwrap();
        name.is_name(ast) && name.get_string(ast) == "Symbol"
    }

    // port: CrossChunkReferenceCollector#pop
    fn pop<T>(list: &mut Vec<T>) -> T {
        list.pop().unwrap()
    }

    // port: CrossChunkReferenceCollector#peek
    fn peek<T: Clone>(list: &[T]) -> T {
        list.last().unwrap().clone()
    }

    /// @return true if this node marks the start of a new basic block
    // port: CrossChunkReferenceCollector#isBlockBoundary
    fn is_block_boundary(ast: &Ast, n: NodeId, parent: Option<NodeId>) -> bool {
        if let Some(parent) = parent {
            match parent.get_token(ast) {
                Token::DO
                | Token::FOR
                | Token::FOR_IN
                | Token::FOR_OF
                | Token::FOR_AWAIT_OF
                | Token::TRY
                | Token::WHILE
                | Token::WITH
                | Token::CLASS => {
                    // NOTE: TRY has up to 3 child blocks:
                    // TRY
                    //   BLOCK
                    //   BLOCK
                    //     CATCH
                    //   BLOCK
                    // Note that there is an explicit CATCH token but no explicit
                    // FINALLY token. For simplicity, we consider each BLOCK
                    // a separate basic BLOCK.
                    return true;
                }
                Token::AND
                | Token::HOOK
                | Token::IF
                | Token::OR
                | Token::SWITCH
                | Token::COALESCE => {
                    // The first child of a conditional is not a boundary,
                    // but all the rest of the children are.
                    return Some(n) != parent.get_first_child(ast);
                }
                _ => {}
            }
        }

        n.is_case(ast)
    }

    // port: CrossChunkReferenceCollector#addReferenceToCollection
    fn add_reference_to_collection(&mut self, v: VarId, reference: Reference) {
        // Create collection if none already
        let reference_info = self.reference_map.entry(v).or_default();

        // Add this particular reference
        reference_info.add(reference);
    }

    // port: CrossChunkReferenceCollector#getTopLevelStatements
    pub fn get_top_level_statements(&self) -> &[TopLevelStatement] {
        &self.top_level_statements
    }

    /// Rust-only read of the private `blockStack` field (the unit replay dumps the test's
    /// `testedCollector` field by field, as Java reflection does).
    pub fn get_block_stack(&self) -> &[Arc<BasicBlock>] {
        &self.block_stack
    }

    /// Rust-only read of the private `statementCounter` field (unit replay field dump).
    pub fn get_statement_counter(&self) -> i32 {
        self.statement_counter
    }

    /// Rust-only: whether the private `topLevelStatementDraft` field is set (unit replay field
    /// dump).
    pub fn has_top_level_statement_draft(&self) -> bool {
        self.top_level_statement_draft.is_some()
    }

    /// Determines whether the given value is eligible to be moved across chunks.
    // port: CrossChunkReferenceCollector#canMoveValue
    fn can_move_value(
        &self,
        compiler: &mut AbstractCompiler,
        scope: ScopeId,
        value_node: Option<NodeId>,
    ) -> bool {
        // the value is only movable if it's
        // - nothing,
        // - a constant literal,
        // - a function
        // - class without computed properties, or whose computed properties expressions are
        //   moveable
        // - an array/object literal of movable values
        // - a template literal with movable substitutions
        // - a function stub generated by CrossChunkMethodMotion.
        let Some(value_node) = value_node else {
            return true;
        };
        if NodeUtil::is_literal_value(compiler, value_node, true)
            || value_node.is_function(compiler)
        {
            return true;
        }

        // Consider the node eligible for move if the current statement or expression is annotated
        // with @pureOrBreakMyCode.
        let js_doc_info = NodeUtil::get_best_jsdoc_info(compiler, value_node);
        if js_doc_info.is_some_and(|info| info.is_pure_or_break_my_code()) {
            return true;
        }

        match value_node.get_token(compiler) {
            Token::CLASS => {
                // NOTE: Why aren't class extends expressions checked here?
                let class_members = value_node.get_last_child(compiler).unwrap();
                let mut member = class_members.get_first_child(compiler);
                while let Some(m) = member {
                    if m.is_computed_prop(compiler) {
                        let key_expr = m.get_first_child(compiler);
                        let method = m.get_last_child(compiler).unwrap();
                        check_state!(
                            method.is_function(compiler),
                            "%s",
                            method.to_string(compiler)
                        );
                        if !self.can_move_value(compiler, scope, key_expr) {
                            return false;
                        }
                    } else if m.is_computed_field_def(compiler) {
                        let key_expr = m.get_first_child(compiler);
                        if !self.can_move_value(compiler, scope, key_expr) {
                            return false;
                        }
                    } else if m.is_member_field_def(compiler) {
                        if m.is_static_member(compiler) {
                            let first = m.get_first_child(compiler);
                            if !self.can_move_value(compiler, scope, first) {
                                // Unlike non-static fields,the RHS of a static field is executed
                                // at class definition time. So, we must check canMoveValue for
                                // static fields.
                                return false;
                            }
                        }
                    } else if m.is_block(compiler) {
                        // TODO(bradfordcsmith): Ideally could move these in some cases, fix later
                        return false;
                    } else {
                        check_state!(
                            m.is_member_function_def(compiler)
                                || NodeUtil::is_get_or_set_key(compiler, m),
                            "%s",
                            m.to_string(compiler)
                        );
                    }
                    member = m.get_next(compiler);
                }
                return true;
            }
            Token::CALL => {
                // In general it is not safe to move function calls, but we carve out an exception
                // for the special stub method calls used for CrossChunkMethodMotion.
                // Case: `JSCompiler_stubMethod(x)`
                let function_name = check_not_null!(value_node.get_first_child(compiler));
                return function_name.is_name(compiler)
                    && function_name.get_string(compiler)
                        == CrossChunkMethodMotion::STUB_METHOD_NAME;
            }
            Token::ARRAYLIT => {
                // Movable if all of the array values are movable.
                let mut child = value_node.get_first_child(compiler);
                while let Some(c) = child {
                    if !self.can_move_value(compiler, scope, Some(c)) {
                        return false;
                    }
                    child = c.get_next(compiler);
                }

                return true;
            }
            Token::OBJECTLIT => {
                // Movable if all of the keys and values are movable.
                let mut child = value_node.get_first_child(compiler);
                while let Some(c) = child {
                    match c.get_token(compiler) {
                        Token::MEMBER_FUNCTION_DEF | Token::GETTER_DEF | Token::SETTER_DEF => {}

                        Token::COMPUTED_PROP => {
                            let first = c.get_first_child(compiler);
                            let last = c.get_last_child(compiler);
                            if !self.can_move_value(compiler, scope, first)
                                || !self.can_move_value(compiler, scope, last)
                            {
                                return false;
                            }
                        }

                        Token::STRING_KEY | Token::OBJECT_SPREAD => {
                            let only = c.get_only_child(compiler);
                            if !self.can_move_value(compiler, scope, Some(only)) {
                                return false;
                            }
                        }

                        _ => {
                            panic!(
                                "Unexpected child of OBJECTLIT: {}",
                                c.to_string_tree(compiler)
                            );
                        }
                    }
                    child = c.get_next(compiler);
                }

                return true;
            }
            Token::NAME => {
                // If the value is guaranteed to never be changed after
                // this reference, then we can move it.
                let name = value_node.get_string(compiler);
                let v = scope.get_var(compiler, name);
                if let Some(v) = v
                    && v.is_global(compiler)
                {
                    let ref_collection = self.get_references(v);
                    if let Some(ref_collection) = ref_collection
                        && ref_collection.is_well_defined(compiler)
                        && ref_collection.is_assigned_once_in_lifetime(compiler)
                    {
                        return true;
                    }
                }
            }
            Token::TEMPLATELIT => {
                // A template literal is movable if all of the substitutions it contains are
                // movable.
                let mut child = value_node.get_first_child(compiler);
                while let Some(c) = child {
                    if c.is_template_lit_sub(compiler) {
                        let first = c.get_first_child(compiler);
                        if !self.can_move_value(compiler, scope, first) {
                            return false;
                        }
                    } else {
                        check_state!(
                            c.is_template_lit_string(compiler),
                            "%s",
                            c.to_string(compiler)
                        );
                    }
                    child = c.get_next(compiler);
                }
                return true;
            }
            Token::GETPROP => {
                // `Symbol` is a built-in JavaScript object that is guaranteed to be unique.
                // Therefore it is movable. Any properties on `Symbol` are also movable (e.g
                // `Symbol.iterator`).
                let child = value_node.get_first_child(compiler);
                if let Some(child) = child
                    && child.is_name(compiler)
                {
                    let symbol = child.get_string(compiler);
                    if symbol == "Symbol" {
                        return true;
                    }
                }
                return false;
            }
            _ => {}
        }

        false
    }
}

impl CompilerPass for CrossChunkReferenceCollector<'_> {
    /// Convenience method for running this pass over a tree with this class as a callback.
    // port: CrossChunkReferenceCollector#process(Node,Node)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        check_state!(
            self.top_level_statements.is_empty(),
            "process() called more than once"
        );
        let mut scope_creator = self.scope_creator.take().unwrap();
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(self)
            .set_scope_creator(&mut *scope_creator)
            .traverse_roots(externs, root);
        self.scope_creator = Some(scope_creator);
    }
}

impl Callback for CrossChunkReferenceCollector<'_> {
    // port: CrossChunkReferenceCollector#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if let Some(parent) = parent
            && NodeUtil::is_top_level(t, parent)
        {
            check_state!(
                self.top_level_statement_draft.is_none(),
                "%s",
                n.to_string(t)
            );
            let chunk = t.get_chunk();
            let draft = self.initialize_draft_statement(t.get_compiler(), chunk, n);
            self.top_level_statement_draft = Some(draft);
        }
        // If node is a new basic block, put on basic block stack
        if Self::is_block_boundary(t, n, parent) {
            let block = BasicBlock::new(t, Some(Self::peek(&self.block_stack)), n);
            self.block_stack.push(block);
        }
        true
    }

    /// For each node, update the block stack and reference collection as appropriate.
    // port: CrossChunkReferenceCollector#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if let Some(draft) = &self.top_level_statement_draft {
            if n == draft.statement_node {
                let draft = self.top_level_statement_draft.take().unwrap();
                self.top_level_statements
                    .push(TopLevelStatement::new(draft));
            } else if n.is_name(t) {
                let var_name = n.get_string(t);
                let scope = t.get_scope();
                let v = scope.get_var(t.get_compiler(), var_name.clone());

                if let Some(v) = v {
                    let compiler = t.get_compiler();
                    // Only global, non-exported names can be moved
                    if v.is_global(compiler)
                        && !compiler
                            .get_coding_convention()
                            .is_exported(&v.get_name(compiler), /* local= */ false)
                    {
                        if let Some(existing) = self.vars_by_name.get(&var_name) {
                            check_state!(existing.equals(t.get_compiler(), v));
                        } else {
                            self.vars_by_name.insert(var_name, v);
                        }
                        let reference = Reference::new(n, t, Self::peek(&self.block_stack));
                        let draft = self.top_level_statement_draft.as_mut().unwrap();
                        if Some(reference.get_node()) == draft.declared_name_node {
                            draft.declared_name_reference = Some(reference.clone());
                        } else {
                            draft.non_declaration_references.push(reference.clone());
                        }
                        self.add_reference_to_collection(v, reference);
                    }
                }
            }
        }
        if Self::is_block_boundary(t, n, parent) {
            Self::pop(&mut self.block_stack);
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for CrossChunkReferenceCollector<'_> {
    /// Updates block stack and invokes any additional behavior.
    // port: CrossChunkReferenceCollector#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        let n = t.get_scope_root().unwrap();
        let parent = if self.block_stack.is_empty() {
            None
        } else {
            Some(Self::peek(&self.block_stack))
        };
        // Don't add all ES6 scope roots to blockStack, only those that are also scopes according
        // to the ES5 scoping rules. Other nodes that ought to be considered the root of a
        // BasicBlock are added in shouldTraverse() and removed in visit().
        if t.is_hoist_scope() {
            let block = BasicBlock::new(t, parent, n);
            self.block_stack.push(block);
        }
    }

    /// Updates block stack and invokes any additional behavior.
    // port: CrossChunkReferenceCollector#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if t.is_hoist_scope() {
            Self::pop(&mut self.block_stack);
        }
    }
}

/// Represents a top-level statement and the references to global names it contains.
pub struct TopLevelStatement {
    /// 0-based index indicating original order of this statement in the source.
    original_order: i32,

    chunk: Option<JSChunk>,
    statement_node: NodeId,
    non_declaration_references: Vec<Reference>,
    declared_name_reference: Option<Reference>,
    declared_value_node: Option<NodeId>,
}

impl TopLevelStatement {
    // port: CrossChunkReferenceCollector.TopLevelStatement#TopLevelStatement
    fn new(draft: TopLevelStatementDraft) -> Self {
        Self {
            original_order: draft.original_order,
            chunk: draft.chunk,
            statement_node: draft.statement_node,
            non_declaration_references: draft.non_declaration_references,
            declared_name_reference: draft.declared_name_reference,
            declared_value_node: draft.declared_value_node,
        }
    }

    // port: CrossChunkReferenceCollector.TopLevelStatement#getOriginalOrder
    pub fn get_original_order(&self) -> i32 {
        self.original_order
    }

    // port: CrossChunkReferenceCollector.TopLevelStatement#getChunk
    pub fn get_chunk(&self) -> Option<&JSChunk> {
        self.chunk.as_ref()
    }

    // port: CrossChunkReferenceCollector.TopLevelStatement#getStatementNode
    pub fn get_statement_node(&self) -> NodeId {
        self.statement_node
    }

    // port: CrossChunkReferenceCollector.TopLevelStatement#getNonDeclarationReferences
    pub fn get_non_declaration_references(&self) -> &[Reference] {
        &self.non_declaration_references
    }

    // port: CrossChunkReferenceCollector.TopLevelStatement#isDeclarationStatement
    pub fn is_declaration_statement(&self) -> bool {
        self.declared_name_reference.is_some()
    }

    // port: CrossChunkReferenceCollector.TopLevelStatement#getDeclaredNameReference
    pub fn get_declared_name_reference(&self) -> &Reference {
        check_not_null!(self.declared_name_reference.as_ref())
    }

    // port: CrossChunkReferenceCollector.TopLevelStatement#getDeclaredValueNode
    pub fn get_declared_value_node(&self) -> Option<NodeId> {
        self.declared_value_node
    }

    /// Java's inner class reads the enclosing collector; Rust passes it explicitly.
    // port: CrossChunkReferenceCollector.TopLevelStatement#isMovableDeclaration
    pub fn is_movable_declaration(
        &self,
        compiler: &mut AbstractCompiler,
        collector: &CrossChunkReferenceCollector<'_>,
    ) -> bool {
        self.is_declaration_statement()
            && collector.can_move_value(
                compiler,
                self.declared_name_reference
                    .as_ref()
                    .unwrap()
                    .get_scope()
                    .unwrap(),
                self.declared_value_node,
            )
    }
}

/// Holds statement info temporarily while the statement is being traversed.
struct TopLevelStatementDraft {
    /// 0-based index indicating original order of this statement in the source.
    original_order: i32,

    chunk: Option<JSChunk>,
    statement_node: NodeId,
    non_declaration_references: Vec<Reference>,
    declared_value_node: Option<NodeId>,
    declared_name_node: Option<NodeId>,
    declared_name_reference: Option<Reference>,
}

impl TopLevelStatementDraft {
    // port: CrossChunkReferenceCollector.TopLevelStatementDraft#TopLevelStatementDraft
    fn new(original_order: i32, chunk: Option<JSChunk>, statement_node: NodeId) -> Self {
        Self {
            original_order,
            chunk,
            statement_node,
            non_declaration_references: Vec::new(),
            declared_value_node: None,
            declared_name_node: None,
            declared_name_reference: None,
        }
    }
}
