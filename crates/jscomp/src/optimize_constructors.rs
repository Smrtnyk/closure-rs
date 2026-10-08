/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/OptimizeConstructors.java.

use crate::{
    abstract_compiler::{AbstractCompiler, LifeCycleStage},
    ast_analyzer::AstAnalyzer,
    compiler_pass::CompilerPass,
    node_util::NodeUtil,
    optimize_calls::{CallGraphCompilerPass, OptimizeCalls, ReferenceMap},
};
use closure_rhino::{
    check_state,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};

/// Optimize class declarations by removing explicit constructor declarations if the implicit
/// constructor is sufficient.
///
/// The constructor can be removed, if
///
/// (0) the class in question defined using ES class syntax
///
/// (2) the constructor is empty save for the call to super
///
/// (3) either:
///
/// - all constructor arguments are forward to the super constructor (via rest or arguments)
///
/// - super constructor parameters match the number, order, and default values of constructor to
///   be removed and the constructor doesn't look for additional parameters via references to
///   'arguments'
///
/// When inspecting super class constructors, we don't need to worry about (1) reassignment of
/// subclasses or (2) escapes of the superclass. We only need to make sure the superclass isn't
/// reassigned. For that we can simply check for direct assignments and whether the name is
/// defined in the externs as references. The two other reassignment cases that are
/// hard/impossible to detect: "globalThis" properties and assignment through eval are only only
/// supported with externs.
///
/// Note that an alternative exists where the super class could be ignored, if all constructor
/// references to the class were known (including direct calls, super calls, implicit
/// constructor calls, etc) to validate the number of parameters passed. This would require
/// backing off on various escapes and is believed it would be both more complicated and less
/// effective.
pub struct OptimizeConstructors {
    ast_analyzer: AstAnalyzer,

    // All constructor definition nodes that are to be removed.
    pub removable_constructors: Vec<NodeId>,
}

impl OptimizeConstructors {
    // port: OptimizeConstructors#OptimizeConstructors
    pub fn new(compiler: &AbstractCompiler) -> Self {
        Self {
            ast_analyzer: compiler.get_ast_analyzer(),
            removable_constructors: Vec::new(),
        }
    }

    /// Iterate over all the references to a symbol. There are several interesting references:
    ///
    /// -a class definition
    ///
    /// -a subclass definition
    ///
    /// -class redefinition
    ///
    /// -allowed reference (anything not an assignment)
    // port: OptimizeConstructors#addConstructorsToBeRemoved
    fn add_constructors_to_be_removed(
        &mut self,
        compiler: &mut AbstractCompiler,
        name: &JsString,
        refs: &[NodeId],
    ) {
        if !OptimizeCalls::may_be_optimizable_name(compiler, name) {
            return;
        }

        let mut candidate_class_definition: Option<NodeId> = None;
        let mut candidate_constructor: Option<NodeId> = None;

        // Lazily init as most symbols aren't class, and most classes don't have subclasses
        let mut subclass_constructors: Option<Vec<NodeId>> = None;

        for &n in refs {
            let definition = Self::get_class_definition_or_function(compiler, n);
            if let Some(definition) = definition {
                if candidate_class_definition.is_some() {
                    // As a simplification only allow one definition.
                    return;
                }

                // Be lazy about constructor analysis as it is expected that most classes
                // don't have subclasses.
                candidate_class_definition = Some(definition);

                if definition.is_class(compiler) {
                    // There are two special cases that we can handle just by looking at the
                    // extend clause

                    // handle no super class
                    //    - if no extends clause
                    //    - and the constructor body is empty
                    //    - and the constructor parameters are side-effect free (no
                    // destructuring or side-effect defaults)
                    let extends_expr = definition.get_second_child(compiler).unwrap();
                    if extends_expr.is_empty(compiler) {
                        let constructor = NodeUtil::get_es6_class_constructor_member_function_def(
                            compiler, definition,
                        );
                        if let Some(constructor) = constructor {
                            let r#fn = constructor.get_last_child(compiler).unwrap();
                            let body = r#fn.get_last_child(compiler).unwrap();
                            if !body.has_children(compiler)
                                && self.has_removable_parameter_list(compiler, constructor)
                            {
                                candidate_constructor = Some(constructor);
                            }
                        }
                    } else {
                        // TODO: handle "Object" super class
                        //    - just a call to super and no-side-effects in the formal
                        // parameters, or super call arguments
                    }
                }
            } else {
                // If this is a reference to the class in an extend clause, then this is a class
                // definition whose constructor should considered for removal.
                if Self::is_class_extends_expression(compiler, n) {
                    let subclass_literal = n.get_parent(compiler).unwrap();
                    let subclass_constructor =
                        NodeUtil::get_es6_class_constructor_member_function_def(
                            compiler,
                            subclass_literal,
                        );
                    if let Some(subclass_constructor) = subclass_constructor
                        && Self::constructor_has_removable_definition(
                            compiler,
                            subclass_constructor,
                        )
                    {
                        subclass_constructors
                            .get_or_insert_with(Vec::new)
                            .push(subclass_constructor);
                    }
                } else {
                    // Anything assignment that isn't a class definition covered above, is an
                    // invalidating assignment.
                    if Self::is_assigning_reference(compiler, n) {
                        return;
                    }
                }
            }
        }

        // There is no known class definition
        let Some(candidate_class_definition) = candidate_class_definition else {
            return;
        };

        if let Some(candidate_constructor) = candidate_constructor {
            self.removable_constructors.push(candidate_constructor);
        }

        // Nothing that invalidated the superclass definition was found, so now check if the
        // candidate subclasses are equivalent to the superclass.

        if let Some(subclass_constructors) = subclass_constructors {
            let summary = ClassConstructorSummary::build(compiler, candidate_class_definition);
            for n in subclass_constructors {
                if summary.is_equivalent_constructor_definition(compiler, n) {
                    self.removable_constructors.push(n);
                }
            }
        }
    }

    /// Is the node in a position in the AST so that it might be assigned a value?
    // port: OptimizeConstructors#isAssigningReference
    pub fn is_assigning_reference(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast).unwrap();
        let gparent = parent.get_parent(ast);
        match parent.get_token(ast) {
            Token::LET | Token::CONST | Token::VAR => n.has_children(ast), // value assigned
            Token::STRING_KEY => gparent.unwrap().is_object_pattern(ast),
            Token::COMPUTED_PROP => {
                parent.get_last_child(ast) == Some(n) && gparent.unwrap().is_object_pattern(ast)
            }
            Token::ARRAY_PATTERN
            | Token::DEFAULT_VALUE // object or array or function parameter
            | Token::PARAM_LIST
            | Token::OBJECT_REST
            | Token::ITER_REST
            | Token::INC
            | Token::DEC => true,
            Token::FUNCTION | Token::CLASS | Token::CATCH => parent.get_first_child(ast) == Some(n),
            Token::ASSIGN
            | Token::ASSIGN_BITOR
            | Token::ASSIGN_BITXOR
            | Token::ASSIGN_BITAND
            | Token::ASSIGN_LSH
            | Token::ASSIGN_RSH
            | Token::ASSIGN_URSH
            | Token::ASSIGN_ADD
            | Token::ASSIGN_SUB
            | Token::ASSIGN_MUL
            | Token::ASSIGN_DIV
            | Token::ASSIGN_MOD
            | Token::ASSIGN_EXPONENT => {
                // If n is the target of the assignment (the first child/LHS of the assignment
                // node), then it is an assigning reference.
                parent.get_first_child(ast) == Some(n)
            }
            _ => false,
        }
    }

    // port: OptimizeConstructors#isClassExtendsExpression
    fn is_class_extends_expression(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast).unwrap();
        parent.is_class(ast) && parent.get_second_child(ast) == Some(n)
    }

    // port: OptimizeConstructors#getClassDefinitionOrFunction
    #[allow(clippy::if_same_then_else)] // Retain Java control flow.
    fn get_class_definition_or_function(compiler: &AbstractCompiler, n: NodeId) -> Option<NodeId> {
        let ast: &Ast = compiler;
        let parent = n.get_parent(ast).unwrap();

        let mut expr;
        if ReferenceMap::is_simple_assignment_target(compiler, n) {
            expr = parent.get_last_child(ast).unwrap();
        } else if n.is_name(ast) && n.has_children(ast) {
            expr = n.get_first_child(ast).unwrap();
        } else if parent.is_function(ast) && n.is_first_child_of(ast, Some(parent)) {
            expr = parent;
        } else if parent.is_class(ast) && n.is_first_child_of(ast, Some(parent)) {
            expr = parent;
        } else {
            return None; // Couldn't find a function.
        }

        expr = Self::unwrap(ast, expr);
        if Self::is_definition_class_literal_or_function(ast, expr) {
            Some(expr)
        } else {
            None
        }
    }

    // port: OptimizeConstructors#unwrap
    fn unwrap(ast: &Ast, mut expr: NodeId) -> NodeId {
        while expr.is_cast(ast) || expr.is_comma(ast) {
            expr = expr.get_last_child(ast).unwrap();
        }
        expr
    }

    // port: OptimizeConstructors#isDefinitionClassLiteralOrFunction
    fn is_definition_class_literal_or_function(ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::FUNCTION => {
                // TODO(b/176208718): ideally this is only return true for normal functions, but
                // it is harmless to include other function types and checking for "normal"
                // function is currently
                // non-trivial.
                true
            }
            Token::CLASS => {
                // `class NameNode {`
                // find the constructor
                let constructor_member_function_def =
                    NodeUtil::get_es6_class_constructor_member_function_def(ast, n);
                constructor_member_function_def.is_some()
            }
            _ => false,
        }
    }

    // port: OptimizeConstructors#functionHasRest
    pub fn function_has_rest(ast: &Ast, r#fn: NodeId) -> bool {
        check_state!(r#fn.is_function(ast));
        let params = NodeUtil::get_function_parameters(ast, r#fn);
        let last_param = params.get_last_child(ast);
        last_param.is_some_and(|last_param| last_param.is_rest(ast))
    }

    /// Validate that nothing about the constructor definition itself prevents its removal: - the
    /// body is simply a super call - it passes on exactly what it receives. These are necessary
    /// but not sufficient for the constructor to be removable.
    // port: OptimizeConstructors#constructorHasRemovableDefinition
    fn constructor_has_removable_definition(ast: &Ast, member: NodeId) -> bool {
        let r#fn = member.get_first_child(ast).unwrap();
        let Some(super_call) = Self::get_only_super_call(ast, r#fn) else {
            return false;
        };

        let param_list = NodeUtil::get_function_parameters(ast, r#fn);
        if param_list.get_child_count(ast) != super_call.get_child_count(ast) - 1 {
            return false;
        }

        // TODO(johnlenz): broaden the recognized patterns: default parameters
        let mut param = param_list.get_first_child(ast);
        let mut arg = super_call.get_second_child(ast);
        while let Some(p) = param {
            let a = arg.unwrap();
            if p.is_rest(ast) {
                if !a.is_spread(ast)
                    || !p
                        .get_first_child(ast)
                        .unwrap()
                        .matches_name_node(ast, a.get_first_child(ast).unwrap())
                {
                    return false;
                }
            } else if !p.matches_name_node(ast, a) {
                // not a simple parameter list or matching call args
                return false;
            }
            param = p.get_next(ast);
            arg = a.get_next(ast);
        }

        true
    }

    /// True if the parameter list can't cause side-effects
    // port: OptimizeConstructors#hasRemovableParameterList
    fn has_removable_parameter_list(
        &self,
        compiler: &mut AbstractCompiler,
        member: NodeId,
    ) -> bool {
        let r#fn = member.get_first_child(compiler).unwrap();
        let param_list = NodeUtil::get_function_parameters(compiler, r#fn);

        let mut param = param_list.get_first_child(compiler);
        while let Some(p) = param {
            param = p.get_next(compiler);
            if p.is_name(compiler) {
                // a simple name
                continue;
            }
            if p.is_rest(compiler) && p.get_first_child(compiler).unwrap().is_name(compiler) {
                // a simple rest expression
                continue;
            }

            if p.is_default_value(compiler)
                && p.get_first_child(compiler).unwrap().is_name(compiler)
                && !self
                    .ast_analyzer
                    .may_have_side_effects(compiler, p.get_last_child(compiler).unwrap())
            {
                // a default parameter whose value is determined to be side-effect free
                continue;
            }

            // not a simple parameter (destructuring can throw, etc)
            return false;
        }

        true
    }

    /// If the body contains only a call to super, return it, otherwise null.
    // port: OptimizeConstructors#getOnlySuperCall
    fn get_only_super_call(ast: &Ast, r#fn: NodeId) -> Option<NodeId> {
        let body = r#fn.get_last_child(ast).unwrap();
        if body.is_block(ast) && body.has_one_child(ast) {
            let stmt = body.get_first_child(ast).unwrap();
            if stmt.is_expr_result(ast) {
                let call = stmt.get_first_child(ast).unwrap();
                if call.is_call(ast) && call.get_first_child(ast).unwrap().is_super(ast) {
                    return Some(call);
                }
            }
        }
        None
    }

    /// Removes any candidate constructor if the callers are consistent with the definition.
    // port: OptimizeConstructors#removeConstructorMethod
    fn remove_constructor_method(compiler: &mut AbstractCompiler, member: NodeId) {
        check_state!(member.is_member_function_def(compiler));

        let r#fn = member.get_first_child(compiler).unwrap();
        compiler.report_function_deleted(r#fn);
        compiler.report_change_to_enclosing_scope(member);
        member.detach(compiler);

        // NOTE: As this pass is expected to run by itself, so don't spend the time to update the
        // reference map.
    }
}

/// An abstraction of a super class constructor definition. The goal of this class is to avoid
/// repeated analysis of the super class constructor when checking whether a subclass constructor
/// is removable.
// port: OptimizeConstructors.ClassConstructorSummary
struct ClassConstructorSummary {
    // The number of explicit parameters
    formal_parameter_count: i32,
    // "var args" means any number of parameters are allowed
    is_var_args: bool,
}

impl ClassConstructorSummary {
    // port: OptimizeConstructors.ClassConstructorSummary#ClassConstructorSummary
    fn new(is_var_args: bool, formal_parameter_count: i32) -> Self {
        Self {
            is_var_args,
            formal_parameter_count,
        }
    }

    // port: OptimizeConstructors.ClassConstructorSummary#build
    fn build(ast: &Ast, class_definition: NodeId) -> Self {
        check_state!(class_definition.is_class(ast) || class_definition.is_function(ast));
        let r#fn = if class_definition.is_class(ast) {
            // NOTE: it would be possible to handle implicit superclass definitions by caching
            // the super class constructor summaries.
            let member =
                NodeUtil::get_es6_class_constructor_member_function_def(ast, class_definition)
                    .unwrap();
            member.get_first_child(ast).unwrap()
        } else {
            class_definition
        };

        let arguments_reference = NodeUtil::does_function_reference_own_arguments_object(ast, r#fn);
        let has_var_args =
            arguments_reference || OptimizeConstructors::function_has_rest(ast, r#fn);
        Self::new(
            has_var_args,
            NodeUtil::get_function_parameters(ast, r#fn).get_child_count(ast),
        )
    }

    // port: OptimizeConstructors.ClassConstructorSummary#isEquivalentConstructorDefinition
    fn is_equivalent_constructor_definition(&self, ast: &Ast, constructor_member: NodeId) -> bool {
        // `constructorHasRemovableDefinition` has already checked the subclass definition. We
        // know that it has a trivial constructor and that the parameters are passed to the super
        // call in the same order as they are declared in the parameter list, etc.

        // As a result we can simply check the parameters of the constructor and
        // validate they are sufficient for the constructor of the super class

        let r#fn = constructor_member.get_first_child(ast).unwrap();
        let param_list = NodeUtil::get_function_parameters(ast, r#fn);

        let has_rest = OptimizeConstructors::function_has_rest(ast, r#fn);
        if has_rest {
            return true;
        }

        if self.is_var_args {
            // The count of parameters may matter, and the function may not pass them all on.
            return false;
        }

        if self.formal_parameter_count != param_list.get_child_count(ast) {
            return false;
        }

        // same count
        true
    }
}

impl CompilerPass for OptimizeConstructors {
    // port: OptimizeConstructors#process(Node,Node)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        check_state!(compiler.get_life_cycle_stage() == LifeCycleStage::NORMALIZED);

        // Java passes `this`; the builder owns its passes here. Each factory call creates a new
        // OptimizeConstructors and runs it once, so the pass given to the builder is a new
        // instance built from the same compiler (removableConstructors starts empty).
        OptimizeCalls::builder()
            .set_compiler(compiler)
            .set_consider_externs(false)
            .add_pass(Box::new(OptimizeConstructors::new(compiler)))
            .build()
            .process(compiler, externs, root);
    }
}

impl CallGraphCompilerPass for OptimizeConstructors {
    // port: OptimizeConstructors#process(Node,Node,ReferenceMap)
    fn process(
        &mut self,
        compiler: &mut AbstractCompiler,
        _externs: NodeId,
        _root: NodeId,
        ref_map: &mut ReferenceMap,
    ) {
        for (key, value) in ref_map.get_name_references() {
            self.add_constructors_to_be_removed(compiler, key, value);
        }

        // NOTE: Normally, the ReferenceMap must be kept in a consistent state (removing
        // references as they are removed from the AST) so the next pass can reuse the
        // ReferenceMap. However, currently this pass runs by itself so that work is avoided.
        //

        for i in 0..self.removable_constructors.len() {
            let r = self.removable_constructors[i];
            Self::remove_constructor_method(compiler, r);
        }
    }
}
