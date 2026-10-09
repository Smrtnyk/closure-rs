/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Reference.java.

//! Port of `Reference.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::basic_block::BasicBlock;
use crate::compiler_input::CompilerInput;
use crate::node_traversal::NodeTraversal;
use crate::node_util::NodeUtil;
use crate::scope::ScopeId;
use crate::var::VarId;
use closure_rhino::input_id::InputId;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::static_source_file::StaticSourceFile;
use closure_rhino::token::Token;
use std::sync::Arc;

/// Represents a single declaration or reference to a variable. Note that references can only be
/// used with untyped scopes and traversals.
///
/// Java's `StaticRef` is not implemented as a trait (DESIGN: Scope/Var handles do not implement
/// the rhino static traits either); the same methods exist with the same names.
#[derive(Debug, Clone)]
pub struct Reference {
    name_node: NodeId,
    basic_block: Option<Arc<BasicBlock>>,
    scope: Option<ScopeId>,
    input_id: Arc<InputId>,
}

// Favor EnumSet over ImmutableSet for performance
const DECLARATION_PARENTS: [Token; 7] = [
    Token::VAR,
    Token::LET,
    Token::CONST,
    Token::PARAM_LIST,
    Token::FUNCTION,
    Token::CLASS,
    Token::CATCH,
];

impl Reference {
    // port: Reference#Reference(Node,NodeTraversal,BasicBlock)
    pub fn new(name_node: NodeId, t: &mut NodeTraversal<'_>, basic_block: Arc<BasicBlock>) -> Self {
        let scope = t.get_scope();
        // Java: t.getInput().getInputId(); the id is shared instead of copied per reference.
        let input_id = t.get_input_id_of_input().expect("NodeTraversal#getInput");
        Self::new_private(name_node, Some(basic_block), Some(scope), input_id)
    }

    // port: Reference#Reference(Node,BasicBlock,Scope,InputId)
    fn new_private(
        name_node: NodeId,
        basic_block: Option<Arc<BasicBlock>>,
        scope: Option<ScopeId>,
        input_id: Arc<InputId>,
    ) -> Self {
        Self {
            name_node,
            basic_block,
            scope,
            input_id,
        }
    }

    // port: Reference#toString
    pub fn to_string(&self, ast: &Ast) -> String {
        self.name_node.to_string(ast)
    }

    /// Creates a variable reference in a given script file name, used in tests.
    // port: Reference#createRefForTest
    pub fn create_ref_for_test(ast: &mut Ast, input: &CompilerInput) -> Reference {
        Reference::new_private(
            ast.new_node(Token::NAME),
            None,
            None,
            Arc::new(input.get_input_id().clone()),
        )
    }

    /// Makes a copy of the current reference using a new Scope instance.
    // port: Reference#cloneWithNewScope
    pub fn clone_with_new_scope(&self, new_scope: ScopeId) -> Reference {
        Reference::new_private(
            self.name_node,
            self.basic_block.clone(),
            Some(new_scope),
            self.input_id.clone(),
        )
    }

    // port: Reference#getSymbol
    pub fn get_symbol(&self, compiler: &mut AbstractCompiler) -> Option<VarId> {
        let name = self.name_node.get_string(compiler);
        self.scope
            .expect("Reference#getSymbol")
            .get_var(compiler, name)
    }

    // port: Reference#getNode
    pub fn get_node(&self) -> NodeId {
        self.name_node
    }

    // port: Reference#getInputId
    pub fn get_input_id(&self) -> &Arc<InputId> {
        &self.input_id
    }

    // port: Reference#getSourceFile
    pub fn get_source_file(&self, ast: &Ast) -> Option<Arc<dyn StaticSourceFile>> {
        self.name_node.get_static_source_file(ast)
    }

    // port: Reference#isDeclaration
    pub fn is_declaration(&self, ast: &Ast) -> bool {
        Self::is_declaration_helper(ast, self.name_node)
    }

    // port: Reference#isDeclarationHelper
    fn is_declaration_helper(ast: &Ast, node: NodeId) -> bool {
        let parent = node.get_parent(ast).unwrap();

        // Special case for class B extends A, A is not a declaration.
        if parent.is_class(ast) && !node.is_first_child_of(ast, Some(parent)) {
            return false;
        }

        // This condition can be true during InlineVariables.
        if parent.get_parent(ast).is_none() {
            return false;
        }

        if NodeUtil::is_name_declaration(ast, parent.get_parent(ast))
            && Some(node) == parent.get_second_child(ast)
        {
            // This is the RHS of a var/let/const and thus not a declaration.
            return false;
        }

        // Special cases for destructuring patterns.
        if parent.is_destructuring_lhs(ast)
            || parent.is_destructuring_pattern(ast)
            || parent.is_rest(ast)
            || (parent.is_string_key(ast) && parent.get_parent(ast).unwrap().is_object_pattern(ast))
            || (parent.is_computed_prop(ast)
                && parent.get_parent(ast).unwrap().is_object_pattern(ast)
                && Some(node) == parent.get_last_child(ast))
            || (parent.is_default_value(ast) && Some(node) == parent.get_first_child(ast))
        {
            return Self::is_declaration_helper(ast, parent);
        }

        if parent.is_import(ast) {
            return true;
        }

        if parent.is_import_spec(ast) && Some(node) == parent.get_last_child(ast) {
            return true;
        }

        // Special case for arrow function
        if parent.is_arrow_function(ast) {
            return Some(node) == parent.get_first_child(ast);
        }

        DECLARATION_PARENTS.contains(&parent.get_token(ast))
    }

    // port: Reference#isVarDeclaration
    pub fn is_var_declaration(&self, ast: &Ast) -> bool {
        self.get_parent(ast).is_var(ast)
    }

    // port: Reference#isLetDeclaration
    pub fn is_let_declaration(&self, ast: &Ast) -> bool {
        self.get_parent(ast).is_let(ast)
    }

    // port: Reference#isDotPropertyAccess
    pub fn is_dot_property_access(&self, ast: &Ast) -> bool {
        self.get_parent(ast).is_get_prop(ast)
    }

    // port: Reference#isAssignedToObjectDestructuringPattern
    pub fn is_assigned_to_object_destructuring_pattern(&self, ast: &Ast) -> bool {
        let parent = self.get_parent(ast);
        // `let { propName: varName } = ref;`
        // or
        // `({ propName: varName } = ref);`
        (parent.is_destructuring_lhs(ast) || parent.is_assign(ast))
            && parent.get_first_child(ast).unwrap().is_object_pattern(ast)
    }

    // port: Reference#isConstDeclaration
    pub fn is_const_declaration(&self, ast: &Ast) -> bool {
        self.get_parent(ast).is_const(ast)
    }

    // port: Reference#isHoistedFunction
    pub fn is_hoisted_function(&self, ast: &Ast) -> bool {
        NodeUtil::is_hoisted_function_declaration(ast, self.get_parent(ast))
    }

    /// Determines whether the variable is initialized at the declaration.
    // port: Reference#isInitializingDeclaration
    pub fn is_initializing_declaration(&self, ast: &Ast) -> bool {
        // VAR and LET are the only types of variable declarations that may not initialize
        // their variables. Catch blocks, named functions, and parameters all do.
        (self.is_declaration(ast)
            && !self.get_parent(ast).is_var(ast)
            && !self.get_parent(ast).is_let(ast))
            || self.name_node.has_children(ast)
    }

    /// For an assignment, variable declaration, or function declaration return the assigned
    /// value, otherwise null.
    // port: Reference#getAssignedValue
    pub fn get_assigned_value(&self, ast: &Ast) -> Option<NodeId> {
        NodeUtil::get_r_value_of_l_value(ast, self.name_node)
    }

    // port: Reference#getBasicBlock
    pub fn get_basic_block(&self) -> Option<&Arc<BasicBlock>> {
        self.basic_block.as_ref()
    }

    // port: Reference#getParent
    pub fn get_parent(&self, ast: &Ast) -> NodeId {
        self.get_node().get_parent(ast).unwrap()
    }

    // port: Reference#getGrandparent
    pub fn get_grandparent(&self, ast: &Ast) -> Option<NodeId> {
        self.get_node().get_grandparent(ast)
    }

    // port: Reference#isLhsOfEnhancedForExpression
    fn is_lhs_of_enhanced_for_expression(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast).unwrap();
        NodeUtil::is_enhanced_for(ast, parent) && parent.get_first_child(ast) == Some(n)
    }

    // port: Reference#isSimpleAssignmentToName
    pub fn is_simple_assignment_to_name(&self, ast: &Ast) -> bool {
        let parent = self.get_parent(ast);
        parent.is_assign(ast) && parent.get_first_child(ast) == Some(self.name_node)
    }

    /// Returns whether the name node for this reference is an lvalue. TODO(tbreisacher): This
    /// method disagrees with NodeUtil#isLValue for "var x;" and "let x;". Consider updating it to
    /// match.
    // port: Reference#isLvalue
    pub fn is_lvalue(&self, ast: &Ast) -> bool {
        let parent = self.get_parent(ast);
        let parent_type = parent.get_token(ast);
        match parent_type {
            Token::VAR | Token::LET | Token::CONST => {
                self.name_node.has_children(ast)
                    || Self::is_lhs_of_enhanced_for_expression(ast, self.name_node)
            }
            Token::DEFAULT_VALUE => parent.get_first_child(ast) == Some(self.name_node),
            Token::INC
            | Token::DEC
            | Token::CATCH
            | Token::ITER_REST
            | Token::OBJECT_REST
            | Token::PARAM_LIST => true,
            Token::FOR | Token::FOR_IN | Token::FOR_OF | Token::FOR_AWAIT_OF => {
                NodeUtil::is_enhanced_for(ast, parent)
                    && parent.get_first_child(ast) == Some(self.name_node)
            }
            Token::ARRAY_PATTERN | Token::STRING_KEY | Token::COMPUTED_PROP => {
                NodeUtil::is_lhs_by_destructuring(ast, self.name_node)
            }
            _ => {
                NodeUtil::is_assignment_op(ast, parent)
                    && parent.get_first_child(ast) == Some(self.name_node)
            }
        }
    }

    // port: Reference#getScope
    pub fn get_scope(&self) -> Option<ScopeId> {
        self.scope
    }
}
