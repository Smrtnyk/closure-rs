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
//   src/com/google/javascript/jscomp/Es6RewriteGenerators.java.

//! Port of `Es6RewriteGenerators.java`.
//!
//! Converts ES6 generator functions to valid ES3 code. This pass runs after all ES6 features
//! except for yield and generators have been transpiled.
//!
//! Genertor transpilation pass uses two sets of node properties:
//!
//! - generatorMarker property - to indicate that subtee contains YIELD nodes;
//! - generatorSafe property - the node is known to require no further modifications to work in
//!   the transpiled form of the generator body.
//!
//! The conversion is done in the following steps:
//!
//! - Find a generator function: `function *() {}`
//! - Replace its original body with a template
//! - Mark all nodes in original body that contain any YIELD nodes
//! - Transpile every statement of the original body into replaced template
//!   - unmarked nodes may be copied into the template with a trivial transpilation of "this",
//!     "break", "continue", "return" and "arguments" keywords.
//!   - marked nodes must be broken up into multiple states to support the yields they contain.
//!
//! `Es6RewriteGenerators` depends on `InjectTranspilationRuntimeLibraries` to inject
//! `generator_engine.js` template.
//!
//! Rust-only layout: Java's inner classes read their enclosing instances' fields. Here
//! `SingleGeneratorFunctionTranspiler` borrows the pass, and the methods of its inner class
//! `TranspilationContext` are implemented on `SingleGeneratorFunctionTranspiler` (they read
//! `uniqueId`, `originalGeneratorBody` and `newGeneratorHoistBlock`), with the context's fields in
//! `self.context`. `Case` objects live in an arena in the context (`CaseId` is Java's object
//! identity); the `Case` methods are `case_*` methods taking the `CaseId` receiver.

use crate::{
    AbstractCompiler,
    ast_factory::{AstFactory, Type},
    compiler_pass::CompilerPass,
    expression_decomposer::{DecompositionType, ExpressionDecomposer},
    js::runtime_js_lib_manager::JsLibField,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    transpilation_namespace::TranspilationNamespace,
    transpilation_passes::TranspilationPasses,
    transpilation_util,
};
use closure_jstype::js_type_native::JSTypeNative;
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::{
    check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    jscomp_colors::{Color, standard_colors},
    node::{NodeId, Prop},
    token::Token,
};
use indexmap::{IndexMap, IndexSet};
use std::cell::OnceCell;
use std::collections::VecDeque;
use std::sync::Arc;

// port: Es6RewriteGenerators#GENERATOR_FUNCTION
const GENERATOR_FUNCTION: &str = "$jscomp$generator$function";
// port: Es6RewriteGenerators#GENERATOR_CONTEXT
const GENERATOR_CONTEXT: &str = "$jscomp$generator$context";
// port: Es6RewriteGenerators#GENERATOR_ARGUMENTS
const GENERATOR_ARGUMENTS: &str = "$jscomp$generator$arguments";
// port: Es6RewriteGenerators#GENERATOR_THIS
const GENERATOR_THIS: &str = "$jscomp$generator$this";
// port: Es6RewriteGenerators#GENERATOR_FORIN_PREFIX
const GENERATOR_FORIN_PREFIX: &str = "$jscomp$generator$forin";

// port: Es6RewriteGenerators#transpiledFeatures
fn transpiled_features() -> FeatureSet {
    FeatureSet::BARE_MINIMUM.with(Feature::GENERATORS)
}

pub struct Es6RewriteGenerators {
    jscomp_async_execute_function: Arc<dyn JsLibField>,
    jscomp_async_execute_program: Arc<dyn JsLibField>,
    generator_context: Arc<dyn JsLibField>,
    jscomp_create_generator: Arc<dyn JsLibField>,

    namespace: TranspilationNamespace,
    ast_factory: AstFactory,

    nullable_string_type: Option<Color>,
    generator_context_type: OnceCell<Type>,
    property_iterator_type: Type,
}

impl Es6RewriteGenerators {
    // port: Es6RewriteGenerators#Es6RewriteGenerators
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        let ast_factory = compiler.create_ast_factory();
        let namespace = TranspilationNamespace::get(compiler);

        let nullable_string_type = if compiler.has_optimization_colors() {
            // typechecking has run, so we must preserve and propagate type information
            Some(Color::create_union(&IndexSet::from([
                standard_colors::NULL_OR_VOID.clone(),
                standard_colors::STRING.clone(),
            ])))
        } else {
            None
        };

        let runtime_js_lib_manager = compiler.get_runtime_js_lib_manager();
        let mut runtime_js_lib_manager = runtime_js_lib_manager.lock().unwrap();
        let generator_context =
            runtime_js_lib_manager.get_js_lib_field("$jscomp.generator.Context");
        let jscomp_async_execute_function =
            runtime_js_lib_manager.get_js_lib_field("$jscomp.asyncExecutePromiseGeneratorFunction");
        let jscomp_async_execute_program =
            runtime_js_lib_manager.get_js_lib_field("$jscomp.asyncExecutePromiseGeneratorProgram");
        let jscomp_create_generator =
            runtime_js_lib_manager.get_js_lib_field("$jscomp.generator.createGenerator");
        drop(runtime_js_lib_manager);

        // Java's Suppliers.memoize(..) of generatorContextType is the OnceCell below.
        let property_iterator_type = AstFactory::type_native_and_color(
            JSTypeNative::FUNCTION_TYPE,
            standard_colors::TOP_OBJECT.clone(),
        );
        // Java also caches compiler.getUniqueIdSupplier(); the compiler owns it (DESIGN §6).
        Self {
            jscomp_async_execute_function,
            jscomp_async_execute_program,
            generator_context,
            jscomp_create_generator,
            namespace,
            ast_factory,
            nullable_string_type,
            generator_context_type: OnceCell::new(),
            property_iterator_type,
        }
    }

    // port: Es6RewriteGenerators#generatorContextType (the memoized Supplier)
    fn generator_context_type(&self, compiler: &mut AbstractCompiler) -> Type {
        self.generator_context_type
            .get_or_init(|| {
                let qname = self.ast_factory.create_qname_for_field(
                    compiler,
                    &self.namespace,
                    self.generator_context.as_ref(),
                );
                AstFactory::type_node(self.ast_factory.create_new_node(compiler, qname, &[]))
            })
            .clone()
    }
}

impl CompilerPass for Es6RewriteGenerators {
    // port: Es6RewriteGenerators#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        check_state!(compiler.get_life_cycle_stage().is_normalized());
        let mut transpiler = GeneratorFunctionsTranspiler { pass: self };
        TranspilationPasses::process_transpile(
            compiler,
            root,
            transpiled_features(),
            &mut [&mut transpiler],
        );
        TranspilationPasses::maybe_mark_features_as_transpiled_away(
            compiler,
            root,
            transpiled_features(),
        );
    }
}

/// Exposes expression with yield inside to an equivalent expression in which yield is of the
/// form:
///
/// ```text
/// var name = yield expr;
/// ```
///
/// For example, changes the following code:
///
/// ```text
/// { return x || yield y; }
/// ```
///
/// into:
///
/// ```text
/// {
///   var temp$$0;
///   if (temp$$0 = x); else temp$$0 = yield y;
///   return temp$$0;
/// }
/// ```
///
/// Expression should always be inside a block, so that other statements could be added at need.
///
/// Uses the `ExpressionDecomposer` class.
struct YieldExposer {
    decomposer: ExpressionDecomposer,
}

impl YieldExposer {
    // port: Es6RewriteGenerators.YieldExposer#YieldExposer
    fn new(compiler: &mut AbstractCompiler) -> Self {
        Self {
            decomposer: compiler.create_default_expression_decomposer(),
        }
    }

    // port: Es6RewriteGenerators.YieldExposer#visitYield
    fn visit_yield(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        if n.get_parent(compiler).unwrap().is_expr_result(compiler) {
            return;
        }
        if self.decomposer.can_expose_expression(compiler, n) != DecompositionType::UNDECOMPOSABLE {
            self.decomposer.maybe_expose_expression(compiler, n);
        } else {
            let link = String::from("https://github.com/google/closure-compiler/wiki/FAQ")
                + "#i-get-an-undecomposable-expression-error-for-my-yield-or-await-expression"
                + "-what-do-i-do";
            let suggestion = "Please rewrite the yield or await as a separate statement.";
            let message = format!("Undecomposable expression: {suggestion}\nSee {link}");
            compiler.report(JSError::make(
                compiler,
                n,
                &transpilation_util::CANNOT_CONVERT,
                &[&message],
            ));
        }
    }
}

impl Callback for YieldExposer {
    // port: Es6RewriteGenerators.YieldExposer#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        n.set_generator_marker(t, false);
        if n.is_function(t) {
            return false;
        }
        if n.is_yield(t) {
            self.visit_yield(t.get_compiler(), n);
            return false;
        }
        true
    }

    // AbstractPreOrderCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}

/// Finds generator functions and performs ES6 -> ES3 trnspilation
struct GeneratorFunctionsTranspiler<'a> {
    pass: &'a Es6RewriteGenerators,
}

impl Callback for GeneratorFunctionsTranspiler<'_> {
    // AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: Es6RewriteGenerators.GeneratorFunctionsTranspiler#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_generator_function(t) {
            let input = t
                .get_input()
                .cloned()
                .expect("java.lang.NullPointerException");
            let unique_id = t
                .get_compiler()
                .get_unique_id_supplier()
                .get_unique_id(&input);
            let compiler = t.get_compiler();
            SingleGeneratorFunctionTranspiler::new(self.pass, compiler, n, unique_id)
                .transpile(compiler);
        }
    }
}

/// A `Case` handle: Java's object identity of a `TranspilationContext.Case`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct CaseId(usize);

/// A case section in a switch block of generator program.
struct Case {
    id: i32,
    case_block: NodeId,

    /// Records number of times the section was referenced.
    ///
    /// It's used to drop unreferenced sections.
    references: Vec<NodeId>,

    /// Indicates that this case is a simple jump or a fall-though case. Points to the target
    /// case.
    jump_to: Option<CaseId>,

    /// Indicates that the body of this case could potentially be embedded into another block
    /// node.
    ///
    /// Usually "`if (a) {b();} else { c(); }`" is transpiled into:
    ///
    /// ```text
    ///   if (a) { goto labelIf; }
    ///   c();
    ///   goto labelEnd;
    /// labelIf:
    ///   b();
    /// labelEnd:
    /// ```
    ///
    /// but "`labelIf: b();`" can be inlined to get shorter output:
    ///
    /// ```text
    ///   if (a) { b(); goto labelEnd; }
    ///   c();
    /// labelEnd:
    /// ```
    ///
    /// In this example "labelIf" case can be embedded into "`{ goto labelIf; }`" block.
    embed_into: Option<NodeId>,

    /// Tells whether this case might fall-through.
    may_fall_through: bool,
}

/// Reprasents a catch case that is used by try/catch transpilation
struct CatchCase {
    catch_case: CaseId,

    /// Number of finally blocks that should be executed before exception can be handled by this
    /// catch case.
    finally_blocks: i32,
}

impl CatchCase {
    // port: Es6RewriteGenerators.TranspilationContext.CatchCase#CatchCase
    fn new(catch_case: CaseId) -> Self {
        Self {
            catch_case,
            finally_blocks: 0,
        }
    }
}

/// Stores "break" and "continue" case sections assosiated with a label.
#[derive(Clone, Copy)]
struct LabelCases {
    break_case: CaseId,
    continue_case: Option<CaseId>,
}

impl LabelCases {
    // port: Es6RewriteGenerators.TranspilationContext.LabelCases#LabelCases
    fn new(break_case: CaseId, continue_case: Option<CaseId>) -> Self {
        Self {
            break_case,
            continue_case,
        }
    }
}

/// State machine context that is used during generator function transpilation.
struct TranspilationContext {
    named_labels: IndexMap<JsString, LabelCases>,
    break_cases: VecDeque<CaseId>,
    continue_cases: VecDeque<CaseId>,

    catch_cases: VecDeque<CatchCase>,
    finally_cases: VecDeque<CaseId>,
    catch_names: IndexSet<JsString>,

    /// All "case" sections that will be added to generator program.
    all_cases: Vec<CaseId>,

    /// All "break" nodes that exit from the generator primary switch statement
    switch_breaks: Vec<NodeId>,

    /// A virtual case that indicates end of program
    program_end_case: CaseId,

    /// Most recently assigned id.
    case_id_counter: i32,

    /// Points to the switch case that is being populated with transpiled instructions from the
    /// original generator function that is being transpiled.
    current_case: CaseId,

    // A counter for the number of finally blocks we are currently inside.
    // This value is used for two purposes:
    // 1. At COMPILE-TIME, to determine if a break/continue is inside a finally block.
    // 2. At RUNTIME, its value is emitted into the generated code to manage the
    //    exception-handling stack.
    nested_finally_block_count: i32,
    this_reference_found: bool,
    arguments_reference_found: bool,

    /// Rust-only: the arena of every `Case` this context created, indexed by `CaseId`.
    cases: Vec<Case>,
}

/// Transpiles a single generator function into a state machine program.
struct SingleGeneratorFunctionTranspiler<'a> {
    /// Rust-only: the enclosing pass instance.
    pass: &'a Es6RewriteGenerators,

    unique_id: String,

    /// The transpilation context for the state machine program.
    context: TranspilationContext,

    /// The body of original generator function that should be transpiled
    original_generator_body: NodeId,

    /// Counter to generate unique variable names for for-in loops.
    ///
    /// If there are multiple for-in loops, we'll need a separate variable for each of them.
    for_in_counter: i32,

    /// The body of a replacement function.
    ///
    /// It's a block node that hoists local variables of a generator program and returns an
    /// actual generator object created from that program:
    ///
    /// ```text
    /// {
    ///   var a;
    ///   var b;
    ///   ...
    ///   return createGenerator(function ($jscomp$generator$context) { ... });
    /// }
    /// ```
    ///
    /// The assumption is that the hoist block always ends with a return statement, and all local
    /// variables are added before this "return" statement.
    new_generator_hoist_block: Option<NodeId>,
}

/// Rust: `type(...)` shorthands of `AstFactory.type`.
fn type_color(color: &Color) -> Type {
    AstFactory::type_(color.clone())
}

impl<'a> SingleGeneratorFunctionTranspiler<'a> {
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#SingleGeneratorFunctionTranspiler
    fn new(
        pass: &'a Es6RewriteGenerators,
        compiler: &mut AbstractCompiler,
        gen_func: NodeId,
        unique_id: String,
    ) -> Self {
        let original_generator_body = gen_func.get_last_child(compiler).unwrap();
        let mut this = Self {
            pass,
            unique_id,
            context: TranspilationContext {
                named_labels: IndexMap::new(),
                break_cases: VecDeque::new(),
                continue_cases: VecDeque::new(),
                catch_cases: VecDeque::new(),
                finally_cases: VecDeque::new(),
                catch_names: IndexSet::new(),
                all_cases: Vec::new(),
                switch_breaks: Vec::new(),
                program_end_case: CaseId(0),
                case_id_counter: 0,
                current_case: CaseId(0),
                nested_finally_block_count: 0,
                this_reference_found: false,
                arguments_reference_found: false,
                cases: Vec::new(),
            },
            original_generator_body,
            for_in_counter: 0,
            new_generator_hoist_block: None,
        };
        this.transpilation_context_init(compiler);
        this
    }

    fn ast_factory(&self) -> &'a AstFactory {
        &self.pass.ast_factory
    }

    fn hoist_block(&self) -> NodeId {
        check_not_null!(self.new_generator_hoist_block)
    }

    /// Hoists a var node inside `newGeneratorHoistBlock`.
    ///
    /// The last statement in the block is expected to be the call to the generator execution
    /// function.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#hoistVarNode
    fn hoist_var_node(&self, compiler: &mut AbstractCompiler, node: NodeId) {
        check_state!(node.is_var(compiler), &node.to_string(compiler));
        let last = self.hoist_block().get_last_child(compiler).unwrap();
        node.insert_before(compiler, last);
    }

    /// Hoists a function declaration statement inside `newGeneratorHoistBlock`.
    ///
    /// In order to maintain the requirements of normalization, function declaration statements
    /// must always be at the beginning of the function body.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#hoistFunctionDeclarationNode
    fn hoist_function_declaration_node(&self, compiler: &mut AbstractCompiler, node: NodeId) {
        check_state!(node.is_function(compiler), &node.to_string(compiler));
        self.hoist_block().add_child_to_front(compiler, node);
    }

    /// Detects whether the generator function was generated by async function transpilation:
    ///
    /// ```text
    ///   function() {
    ///     ...
    ///     return $jscomp.asyncExecutePromiseGeneratorFunction(function* genFunc() {...});
    ///   }
    /// ```
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#isTranspiledAsyncFunction
    fn is_transpiled_async_function(
        &self,
        compiler: &mut AbstractCompiler,
        generator_function: NodeId,
    ) -> bool {
        if generator_function
            .get_parent(compiler)
            .unwrap()
            .is_call(compiler)
            && generator_function.get_previous(compiler).is_some()
        {
            let call_target = generator_function
                .get_parent(compiler)
                .unwrap()
                .get_first_child(compiler)
                .unwrap();
            if generator_function.get_previous(compiler) == Some(call_target)
                && generator_function.get_next(compiler).is_none()
                && self
                    .pass
                    .jscomp_async_execute_function
                    .matches(compiler, call_target)
            {
                check_state!(
                    generator_function
                        .get_grandparent(compiler)
                        .unwrap()
                        .is_return(compiler)
                );
                check_state!(
                    generator_function
                        .get_grandparent(compiler)
                        .unwrap()
                        .get_next(compiler)
                        .is_none()
                );
                return true;
            }
        }
        false
    }

    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpile
    fn transpile(&mut self, compiler: &mut AbstractCompiler) {
        let af = self.ast_factory();
        let generator_function = self.original_generator_body.get_parent(compiler).unwrap();
        check_state!(generator_function.is_generator_function(compiler));
        generator_function.put_boolean_prop(compiler, Prop::GENERATOR_FN, false);

        // A "program" function:
        //   function ($jscomp$generator$context) {
        //   }
        let program: NodeId;
        // function(!Context<YIELD_TYPE>): (void|{value: YIELD_TYPE})
        let program_type = standard_colors::TOP_OBJECT.clone();
        let generator_body = af.create_block(compiler, &[]);

        let change_scope_node: NodeId;
        if self.is_transpiled_async_function(compiler, generator_function) {
            // Our generatorFunction is a transpiled async function

            // $jscomp.asyncExecutePromiseGeneratorFunction
            let call_target = generator_function.get_previous(compiler).unwrap();
            check_state!(
                call_target.is_get_prop(compiler) || call_target.is_name(compiler),
                &call_target.to_string(compiler)
            );

            // Use original async function as a hoist block for local generator variables:
            // generator function -> call -> return -> async function body
            let hoist_block = generator_function
                .get_grandparent(compiler)
                .unwrap()
                .get_parent(compiler)
                .unwrap();
            self.new_generator_hoist_block = Some(hoist_block);
            check_state!(
                hoist_block.is_block(compiler),
                &hoist_block.to_string(compiler)
            );
            change_scope_node = NodeUtil::get_enclosing_function(compiler, hoist_block)
                .expect("java.lang.NullPointerException");
            check_state!(
                change_scope_node.is_function(compiler),
                &change_scope_node.to_string(compiler)
            );

            // asyncExecutePromiseGeneratorFunction   =>   asyncExecutePromiseGeneratorProgram
            let program_qname = af.create_qname_for_field(
                compiler,
                &self.pass.namespace,
                self.pass.jscomp_async_execute_program.as_ref(),
            );
            let program_qname = program_qname.srcref_tree(compiler, call_target);
            call_target.replace_with(compiler, program_qname);

            program = self.original_generator_body.get_parent(compiler).unwrap();
            // function *() {...}   =>   function *(context) {}
            let context_name =
                self.get_js_context_name_node(compiler, self.original_generator_body);
            self.original_generator_body
                .get_previous(compiler)
                .unwrap()
                .add_child_to_back(compiler, context_name);
            self.original_generator_body
                .replace_with(compiler, generator_body);
        } else {
            change_scope_node = generator_function;
            let gen_func_name = generator_function.get_first_child(compiler).unwrap();
            check_state!(gen_func_name.is_name(compiler));
            // The transpiled function needs to be able to refer to itself, so make sure it has a
            // name.
            if gen_func_name.get_string_ref(compiler).is_empty() {
                let name = self.get_scoped_name(GENERATOR_FUNCTION);
                gen_func_name.set_string(compiler, name);
                if af.is_adding_colors() {
                    // The name of the function is a variable with the same type as the function
                    // expression itself.
                    let color = generator_function.get_color(compiler);
                    gen_func_name.set_color(compiler, color);
                }
                // Function expression name nodes are always marked constant.
                gen_func_name.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
            }

            // Prepare a "program" function:
            //   function ($jscomp$generator$context) {
            //   }
            let context_name =
                self.get_js_context_name_node(compiler, self.original_generator_body);
            let param_list = IR::param_list(compiler, &[context_name]);
            program = af.create_function(
                compiler,
                "",
                param_list,
                generator_body,
                type_color(&program_type),
            );

            // $jscomp.generator.createGenerator
            let create_generator = af.create_qname_for_field(
                compiler,
                &self.pass.namespace,
                self.pass.jscomp_create_generator.as_ref(),
            );
            // Replace original generator function body with:
            // return $jscomp.generator.createGenerator(<origGenerator>, <program function>);
            let gen_func_name_clone = gen_func_name.clone_node(compiler);
            // TODO(b/142881197): we can't give a more accurate type right now.
            let call = af.create_call_with_unknown_type(
                compiler,
                create_generator,
                &[gen_func_name_clone, program],
            );
            let ret = af
                .create_return(compiler, call)
                .srcref_tree(compiler, self.original_generator_body);
            let hoist_block = af.create_block(compiler, &[ret]);
            self.new_generator_hoist_block = Some(hoist_block);
            self.original_generator_body
                .replace_with(compiler, hoist_block);
        }

        // New scopes and any changes to scopes should be reported individually.
        compiler.report_change_to_change_scope(program);

        NodeTraversal::traverse(compiler, self.original_generator_body, &mut YieldNodeMarker);

        // Test if end of generator function is reachable
        let should_add_final_jump =
            !Self::is_end_of_block_unreachable(compiler, self.original_generator_body);

        // Transpile statements from original generator function
        while self.original_generator_body.has_children(compiler) {
            let statement = self
                .original_generator_body
                .remove_first_child(compiler)
                .unwrap();
            self.transpile_statement(compiler, statement);
        }

        // Ensure that the state machine program ends
        let final_block = af.create_block(compiler, &[]);
        if should_add_final_jump {
            let jump_to_end = self.call_context_method_result(
                compiler,
                self.original_generator_body,
                "jumpToEnd",
                type_color(&standard_colors::NULL_OR_VOID),
                &[],
            );
            final_block.add_child_to_back(compiler, jump_to_end);
        }
        let current_case = self.context.current_case;
        let program_end_case = self.context.program_end_case;
        self.case_jump_to(compiler, current_case, program_end_case, final_block);
        self.case_mut(self.context.current_case).may_fall_through = true;

        self.finalize_transformation(compiler, generator_body);
        self.check_state_is_empty();

        compiler.report_change_to_change_scope(change_scope_node);
    }

    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileStatement(Node)
    fn transpile_statement(&mut self, compiler: &mut AbstractCompiler, statement: NodeId) {
        self.transpile_statement_with_cases(compiler, statement, None, None);
    }

    /// Transpiles a detached node and adds transpiled version of it to the `currentCase` of the
    /// `context`.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileStatement(Node,Case,Case)
    fn transpile_statement_with_cases(
        &mut self,
        compiler: &mut AbstractCompiler,
        statement: NodeId,
        break_case: Option<CaseId>,
        continue_case: Option<CaseId>,
    ) {
        check_state!(IR::may_be_statement(compiler, statement));
        check_state!(statement.get_parent(compiler).is_none());

        if !statement.is_generator_marker(compiler) {
            self.transpile_unmarked_node(compiler, statement);
            return;
        }
        match statement.get_token(compiler) {
            Token::LABEL => self.transpile_label(compiler, statement),
            Token::BLOCK => self.transpile_block(compiler, statement),
            Token::EXPR_RESULT => self.transpile_expression_result(compiler, statement),
            Token::VAR => self.transpile_var(compiler, statement),
            Token::RETURN => self.transpile_return(compiler, statement),
            Token::THROW => self.transpile_throw(compiler, statement),
            Token::IF => self.transpile_if(compiler, statement, break_case),
            Token::FOR => self.transpile_for(compiler, statement, break_case, continue_case),
            Token::FOR_IN => self.transpile_for_in(compiler, statement, break_case, continue_case),
            Token::DO => self.transpile_do(compiler, statement, break_case, continue_case),
            Token::TRY => self.transpile_try(compiler, statement, break_case),
            Token::SWITCH => self.transpile_switch(compiler, statement, break_case),
            // NOTE: There is no WHILE case, becasue this pass runs after normalization,
            // which converts all while loops to for loops.
            token => panic!("Unsupported token: {token}"),
        }
    }

    /// Transpiles code that doesn't contain `yield`s.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileUnmarkedNode
    fn transpile_unmarked_node(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        check_state!(!n.is_generator_marker(compiler));
        if n.is_function(compiler) {
            // An inner function should be created only once, when the generator function is
            // first called, not every time the inner callback we create is called, so we will
            // hoist function declaration statements into the outer scope along with the other
            // variable declarations.
            // TODO(bradfordcsmith): Ideally we should probably also hoist out function
            // expressions, but there's no strong need to do this now and it would be tricky to
            // get right.
            let function_name = n.get_first_child(compiler).unwrap().get_string(compiler);
            // Make sure there are no "function (...) {...}" statements (note that
            // "function *(...) {...}" becomes "function $jscomp$generator$function(...) {...}"
            // as inner generator functions are transpiled first).
            check_state!(
                !function_name.is_empty()
                    && !function_name.starts_with(&JsString::from(GENERATOR_FUNCTION))
            );
            self.hoist_function_declaration_node(compiler, n);
            return;
        }
        let block = if n.is_block(compiler) || n.is_added_block(compiler) {
            n
        } else {
            IR::block_with_child(compiler, n)
        };
        self.transpile_unmarked_block(compiler, block);
    }

    /// Transpiles a label with marked statement.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileLabel
    fn transpile_label(&mut self, compiler: &mut AbstractCompiler, mut n: NodeId) {
        // Collect all labels names in "a: b: c: {}" statement
        let mut label_names: Vec<NodeId> = Vec::new();
        while n.is_label(compiler) {
            label_names.push(n.remove_first_child(compiler).unwrap());
            n = n.remove_first_child(compiler).unwrap();
        }

        // Push label names and continue transpilation
        let continue_case = if NodeUtil::is_loop_structure(compiler, n) {
            Some(self.create_case(compiler))
        } else {
            None
        };
        let break_case = self.create_case(compiler);
        self.push_labels(compiler, &label_names, break_case, continue_case);
        self.transpile_statement_with_cases(compiler, n, Some(break_case), continue_case);
        self.pop_labels(compiler, &label_names);

        // Switch to endCase if it's not yet active.
        if break_case != self.context.current_case {
            self.switch_case_to(compiler, break_case);
        }
    }

    /// Transpiles a block.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileBlock
    fn transpile_block(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        while n.has_children(compiler) {
            let statement = n.remove_first_child(compiler).unwrap();
            self.transpile_statement(compiler, statement);
        }
    }

    /// Transpiles marked expression result statement.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileExpressionResult
    fn transpile_expression_result(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let first = n.remove_first_child(compiler).unwrap();
        let exposed_expression = self.expose_yield_and_transpile_rest(compiler, first);
        let decomposed = self.transpile_yields(compiler, exposed_expression);

        // Tanspile "a = yield;" into "a = $context.yieldResult;"
        // But don't transpile "yield;" into "$context.yieldResult;"
        // As it influences the collapsing of empty case sections.
        if !exposed_expression.is_yield(compiler) {
            let prepared = self
                .prepare_node_for_write(compiler, Some(decomposed))
                .unwrap();
            n.add_child_to_front(compiler, prepared);
            n.set_generator_marker(compiler, false);
            self.write_generated_node(compiler, n);
        }
    }

    /// Transpiles marked "var" statement.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileVar
    fn transpile_var(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        n.set_generator_marker(compiler, false);
        let mut new_vars = n.clone_node(compiler);
        while n.has_children(compiler) {
            let mut var: Option<NodeId>;
            // Just collect all unmarked vars and transpile them together.
            loop {
                var = n.remove_first_child(compiler);
                match var {
                    Some(v) if !v.is_generator_marker(compiler) => {
                        new_vars.add_child_to_back(compiler, v);
                    }
                    _ => break,
                }
            }
            if new_vars.has_children(compiler) {
                self.transpile_unmarked_node(compiler, new_vars);
                new_vars = n.clone_node(compiler);
            }

            // Transpile marked var
            if let Some(var) = var {
                check_state!(var.is_generator_marker(compiler));
                let first = var.remove_first_child(compiler);
                let decomposed = self.maybe_decompose_expression(compiler, first).unwrap();
                var.add_child_to_front(compiler, decomposed);
                var.set_generator_marker(compiler, false);
                new_vars.add_child_to_back(compiler, var);
            }
        }

        // Flush the vars if not empty
        if new_vars.has_children(compiler) {
            self.transpile_unmarked_node(compiler, new_vars);
        }
    }

    /// Transpiles marked "return" statement.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileReturn
    fn transpile_return(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let first = n.remove_first_child(compiler);
        let decomposed = self.maybe_decompose_expression(compiler, first);
        let prepared = self.prepare_node_for_write(compiler, decomposed);
        let expression = self.return_expression(compiler, n, prepared);
        n.add_child_to_front(compiler, expression);
        self.write_generated_node(compiler, n);
        self.case_mut(self.context.current_case).may_fall_through = false;
    }

    /// Transpiles marked "throw" statement.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileThrow
    fn transpile_throw(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let first = n.remove_first_child(compiler);
        let decomposed = self.maybe_decompose_expression(compiler, first);
        let prepared = self.prepare_node_for_write(compiler, decomposed).unwrap();
        n.add_child_to_front(compiler, prepared);
        self.write_generated_node(compiler, n);
        self.case_mut(self.context.current_case).may_fall_through = false;
    }

    /// Exposes YIELD operator so it's free of side effects transpiling some code on the way.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#exposeYieldAndTranspileRest
    fn expose_yield_and_transpile_rest(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
    ) -> NodeId {
        let af = self.ast_factory();
        check_state!(n.is_generator_marker(compiler));
        if n.is_yield(compiler) {
            return n;
        }

        // Assuming the initial node is "a + (a = b) + (b = yield) + a".

        // YieldExposer may break n up into multiple statements.
        // Place n into a temporary block to hold those statements:
        // {
        //   var JSCompiler_temp_const$jscomp$0 = a + (a = b);
        //   return JSCompiler_temp_const$jscomp$0 + (b = yield) + a;
        // }
        // Need to put expression nodes into return node so that they always stay expression
        // nodes. If expression put into expression result YieldExposer may turn it into an "if"
        // statement.
        let is_expression = IR::may_be_expression(compiler, n);
        let statement = if is_expression {
            af.create_return(compiler, n)
        } else {
            n
        };
        let block = af.create_block(compiler, &[statement]);
        let mut exposer = YieldExposer::new(compiler);
        NodeTraversal::traverse(compiler, n, &mut exposer);
        // Make sure newly created statements are correctly marked for recursive
        // transpileStatement() calls.
        NodeTraversal::traverse(compiler, block, &mut YieldNodeMarker);

        // The last child of decomposed block free of side effects.
        let decomposed = block.get_last_child(compiler).unwrap().detach(compiler);
        self.transpile_statement(compiler, block);
        if is_expression {
            decomposed.remove_first_child(compiler).unwrap()
        } else {
            decomposed
        }
    }

    /// Converts an expression node containing YIELD into an unmarked analogue.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#maybeDecomposeExpression
    fn maybe_decompose_expression(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: Option<NodeId>,
    ) -> Option<NodeId> {
        let Some(n) = n else {
            return n;
        };
        if !n.is_generator_marker(compiler) {
            return Some(n);
        }
        let exposed = self.expose_yield_and_transpile_rest(compiler, n);
        Some(self.transpile_yields(compiler, exposed))
    }

    /// Makes unmarked node containing arbitrary code suitable to write using
    /// `TranspilationContext#writeGeneratedNode` method.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#prepareNodeForWrite
    fn prepare_node_for_write(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: Option<NodeId>,
    ) -> Option<NodeId> {
        let af = self.ast_factory();
        let n = n?;

        // Need to wrap a node so, it can be replaced in the tree with some other node if
        // necessary.
        let wrapper = if IR::may_be_statement(compiler, n) {
            af.create_block(compiler, &[n])
        } else {
            af.expr_result(compiler, n)
        };
        let mut transpiler = UnmarkedNodeTranspiler::new(self);
        NodeTraversal::traverse(compiler, wrapper, &mut transpiler);
        check_state!(wrapper.has_one_child(compiler));
        wrapper.remove_first_child(compiler)
    }

    /// Converts node with YIELD into $jscomp$generator$context.yieldResult.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileYields
    fn transpile_yields(&mut self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        if !n.is_generator_marker(compiler) {
            // In some cases exposing yield causes it to disapear from the resulting statement.
            // I.e. the following node: "0 || yield;" becomes:
            // {
            //   var JSCompiler_temp$jscomp$0;
            //   if (JSCompiler_temp$jscomp$0 = 0); else JSCompiler_temp$jscomp$0 = yield;
            // }
            // JSCompiler_temp$jscomp$0; // This is our resulting statement.
            return n;
        }
        let jump_to_section = self.create_case(compiler);
        let yield_node = self.find_yield(compiler, n);
        let first = yield_node.remove_first_child(compiler);
        let decomposed = self.maybe_decompose_expression(compiler, first);
        let yield_expression = self.prepare_node_for_write(compiler, decomposed);
        if yield_node.is_yield_all(compiler) {
            self.yield_all(
                compiler,
                yield_expression.expect("java.lang.NullPointerException"),
                jump_to_section,
                yield_node,
            );
        } else {
            self.yield_value(compiler, yield_expression, jump_to_section, yield_node);
        }
        self.switch_case_to(compiler, jump_to_section);
        let mut yield_result = self.yield_result(compiler, yield_node);
        if yield_node == n {
            return yield_result;
        }
        // Replace YIELD with $context.yeildResult
        yield_node.replace_with(compiler, yield_result);
        // Remove generator markings from subtree
        while yield_result != n {
            yield_result = yield_result.get_parent(compiler).unwrap();
            yield_result.set_generator_marker(compiler, false);
        }
        n
    }
}

impl SingleGeneratorFunctionTranspiler<'_> {
    /// Transpiles marked "if" stetement.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileIf
    fn transpile_if(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        break_case: Option<CaseId>,
    ) {
        let af = self.ast_factory();
        // Decompose condition first
        let first = n.remove_first_child(compiler);
        let mut condition = self.maybe_decompose_expression(compiler, first).unwrap();
        let mut if_block = n.get_first_child(compiler).unwrap();
        let mut else_block = if_block.get_next(compiler);

        // No transpilation is needed
        if !if_block.is_generator_marker(compiler)
            && else_block.is_none_or(|else_block| !else_block.is_generator_marker(compiler))
        {
            n.add_child_to_front(compiler, condition);
            n.set_generator_marker(compiler, false);
            self.transpile_unmarked_node(compiler, n);
            return;
        }

        if_block.detach(compiler);
        let mut else_block = match else_block.take() {
            // No "else" block, just create an empty one as will need it anyway.
            None => af.create_block(compiler, &[]).srcref(compiler, n),
            Some(else_block) => else_block.detach(compiler),
        };

        // Only "else" block is unmarked, swap "if" and "else" blocks and negate the condition.
        if if_block.is_generator_marker(compiler) && !else_block.is_generator_marker(compiler) {
            condition = af
                .create_not(compiler, condition)
                .srcref(compiler, condition);
            std::mem::swap(&mut if_block, &mut else_block);
        }

        // Unmarked "if" block (marked "else")
        if !if_block.is_generator_marker(compiler) {
            let end_case = self.maybe_create_case(compiler, break_case);
            let jump_to_block = self.create_jump_to_block(
                compiler, end_case, /* allowEmbedding= */ false, if_block,
            );
            while jump_to_block.has_children(compiler) {
                let jump_to_node = jump_to_block.remove_first_child(compiler).unwrap();
                jump_to_node.set_generator_safe(compiler, true);
                if_block.add_child_to_back(compiler, jump_to_node);
            }
            let if_node = af
                .create_if(compiler, condition, if_block)
                .srcref(compiler, n);
            self.transpile_unmarked_node(compiler, if_node);
            self.transpile_statement(compiler, else_block);
            self.switch_case_to(compiler, end_case);
            return;
        }

        let if_case = self.create_case(compiler);
        let end_case = self.maybe_create_case(compiler, break_case);

        // "if" and "else" blocks marked
        let condition = self.prepare_node_for_write(compiler, Some(condition));
        let new_if_block =
            self.create_jump_to_block(compiler, if_case, /* allowEmbedding= */ true, n);
        let condition = self.prepare_node_for_write(compiler, condition).unwrap();
        let if_node = af
            .create_if(compiler, condition, new_if_block)
            .srcref(compiler, n);
        self.write_generated_node(compiler, if_node);
        self.transpile_statement(compiler, else_block);
        self.write_jump_to(compiler, end_case, else_block);
        self.switch_case_to(compiler, if_case);
        self.transpile_statement(compiler, if_block);
        self.switch_case_to(compiler, end_case);
    }

    /// Transpiles marked "for" statement.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileFor
    fn transpile_for(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        break_case: Option<CaseId>,
        continue_case: Option<CaseId>,
    ) {
        let af = self.ast_factory();
        // Decompose init first
        let first = n.remove_first_child(compiler);
        let mut init = self.maybe_decompose_expression(compiler, first).unwrap();
        let mut condition = n.get_first_child(compiler).unwrap();
        let mut increment = condition.get_next(compiler).unwrap();
        let body = increment.get_next(compiler).unwrap();

        // No transpilation is needed
        if !condition.is_generator_marker(compiler)
            && !increment.is_generator_marker(compiler)
            && !body.is_generator_marker(compiler)
        {
            n.add_child_to_front(compiler, init);
            n.set_generator_marker(compiler, false);
            self.transpile_unmarked_node(compiler, n);
            return;
        }

        // Move init expression out of for loop.
        if !init.is_empty(compiler) {
            if IR::may_be_expression(compiler, init) {
                // Convert expression into expression result.
                init = af.expr_result(compiler, init).srcref(compiler, init);
            }
            self.transpile_unmarked_node(compiler, init);
        }

        let start_case = self.create_case(compiler);
        let increment_case = self.maybe_create_case(compiler, continue_case);
        let end_case = self.maybe_create_case(compiler, break_case);

        self.switch_case_to(compiler, start_case);

        // Transpile condition expression
        if !condition.is_empty(compiler) {
            let detached = condition.detach(compiler);
            let decomposed = self.maybe_decompose_expression(compiler, Some(detached));
            condition = self.prepare_node_for_write(compiler, decomposed).unwrap();
            let not = af
                .create_not(compiler, condition)
                .srcref(compiler, condition);
            let jump_block =
                self.create_jump_to_block(compiler, end_case, /* allowEmbedding= */ true, n);
            let if_node = af.create_if(compiler, not, jump_block).srcref(compiler, n);
            self.write_generated_node(compiler, if_node);
        }

        // Transpile "for" body
        self.push_break_continue_context(end_case, increment_case);
        let body = body.detach(compiler);
        self.transpile_statement(compiler, body);
        self.pop_break_continue_context();

        // Transpile increment expression
        self.switch_case_to(compiler, increment_case);
        if !increment.is_empty(compiler) {
            let detached = increment.detach(compiler);
            increment = self
                .maybe_decompose_expression(compiler, Some(detached))
                .unwrap();
            let expr_result = af
                .expr_result(compiler, increment)
                .srcref(compiler, increment);
            self.transpile_unmarked_node(compiler, expr_result);
        }
        self.write_jump_to(compiler, start_case, n);

        self.switch_case_to(compiler, end_case);
    }

    /// Transpile "for in" statement by converting it into "for".
    ///
    /// `for (var i in expr) {}` will be converted into
    /// `for (var i, $for$in = $context.forIn(expr); i = $for$in.getNext(); ) {}`
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileForIn
    fn transpile_for_in(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        break_case: Option<CaseId>,
        continue_case: Option<CaseId>,
    ) {
        let af = self.ast_factory();
        // Decompose condition first
        let second = n.get_second_child(compiler).unwrap().detach(compiler);
        let detached_expr = self
            .maybe_decompose_expression(compiler, Some(second))
            .unwrap();
        let mut target = n.get_first_child(compiler).unwrap();
        let body = n.get_second_child(compiler).unwrap();

        // No transpilation is needed
        if !target.is_generator_marker(compiler) && !body.is_generator_marker(compiler) {
            detached_expr.insert_after(compiler, target);
            n.set_generator_marker(compiler, false);
            self.transpile_unmarked_node(compiler, n);
            return;
        }

        // Prepare a new init statement
        let init: NodeId;
        if target.detach(compiler).is_var(compiler) {
            // "var i in x"    =>   "var i"
            check_state!(!target.is_generator_marker(compiler));
            init = target;
            check_state!(
                !init
                    .get_first_child(compiler)
                    .unwrap()
                    .has_children(compiler)
            );
            target = init.get_first_child(compiler).unwrap().clone_node(compiler);
        } else {
            // "i in x"   =>    "var"
            init = compiler.new_node(Token::VAR).srcref(compiler, target);
        }

        // "$for$in"
        let child = self.call_context_method(
            compiler,
            target,
            "forIn",
            self.pass.property_iterator_type.clone(),
            &[detached_expr],
        );
        let for_in_name = format!(
            "{}${}",
            self.get_scoped_name(GENERATOR_FORIN_PREFIX),
            self.for_in_counter
        );
        self.for_in_counter += 1;
        let for_in = af
            .create_name(compiler, for_in_name, AstFactory::type_node(child))
            .srcref(compiler, target);
        // "$context.forIn(x)"
        for_in.add_child_to_front(compiler, child);
        // "var ..., $for$in = $context.forIn(expr)"
        init.add_child_to_back(compiler, for_in);

        // "$for$in.getNext()"
        let for_in_clone = for_in.clone_node(compiler);
        let for_in_get_next = af
            .create_get_prop(
                compiler,
                for_in_clone,
                "getNext",
                type_color(&standard_colors::TOP_OBJECT),
            )
            .srcref(compiler, detached_expr);

        // "(i = $for$in.getNext()) != null"
        let nullable_string_type = self.pass.nullable_string_type.clone();
        let target_with_color = target.set_color(compiler, nullable_string_type.clone());
        let call = af
            .create_call(
                compiler,
                for_in_get_next,
                AstFactory::type_jstype_and_color(None, nullable_string_type),
                &[],
            )
            .srcref(compiler, detached_expr);
        let assign = af
            .create_assign(compiler, target_with_color, call)
            .srcref(compiler, detached_expr);
        let null = af.create_null(compiler).srcref(compiler, for_in);
        let for_cond = af
            .create_ne(compiler, assign, null)
            .srcref(compiler, detached_expr);
        let target_marker = target.is_generator_marker(compiler);
        for_cond.set_generator_marker(compiler, target_marker);

        // Prepare "for" statement.
        // "for (var i, $for$in = $context.forIn(expr); (i = $for$in.getNext()) != null; ) {}"
        let empty = IR::empty(compiler).srcref(compiler, n);
        let body = body.detach(compiler);
        let for_node = IR::for_node(compiler, init, for_cond, empty, body).srcref(compiler, n);

        // Transpile "for" instead of "for in".
        self.transpile_for(compiler, for_node, break_case, continue_case);
    }

    /// Transpiles "do while" statement.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileDo
    fn transpile_do(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        break_case: Option<CaseId>,
        continue_case: Option<CaseId>,
    ) {
        let af = self.ast_factory();
        let start_case = self.create_case(compiler);
        let break_case = self.maybe_create_case(compiler, break_case);
        let continue_case = self.maybe_create_case(compiler, continue_case);

        self.switch_case_to(compiler, start_case);

        // Transpile body
        let body = n.remove_first_child(compiler).unwrap();
        self.push_break_continue_context(break_case, continue_case);
        self.transpile_statement(compiler, body);
        self.pop_break_continue_context();

        // Transpile condition
        self.switch_case_to(compiler, continue_case);
        let first = n.remove_first_child(compiler);
        let decomposed = self.maybe_decompose_expression(compiler, first);
        let condition = self.prepare_node_for_write(compiler, decomposed).unwrap();
        let jump_block =
            self.create_jump_to_block(compiler, start_case, /* allowEmbedding= */ false, n);
        let if_node = af
            .create_if(compiler, condition, jump_block)
            .srcref(compiler, n);
        self.write_generated_node(compiler, if_node);
        self.switch_case_to(compiler, break_case);
    }

    /// Transpiles "try" statement
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileTry
    fn transpile_try(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        break_case: Option<CaseId>,
    ) {
        let try_block = n.remove_first_child(compiler).unwrap();
        let catch_block = n.remove_first_child(compiler).unwrap();
        let finally_block = n.remove_first_child(compiler);

        let catch_case = if catch_block.has_children(compiler) {
            Some(self.create_case(compiler))
        } else {
            None
        };
        let finally_case = if finally_block.is_none() {
            None
        } else {
            Some(self.create_case(compiler))
        };
        let end_case = self.maybe_create_case(compiler, break_case);

        // Transpile "try" block
        self.enter_try_block(compiler, catch_case, finally_case, try_block);
        self.transpile_statement(compiler, try_block);

        match finally_block {
            None => self.leave_try_block(compiler, catch_case, end_case, try_block),
            Some(finally_block) => {
                // Transpile "finally" block
                self.switch_case_to(compiler, finally_case.unwrap());
                self.enter_finally_block(compiler, catch_case, finally_case, finally_block);
                self.transpile_statement(compiler, finally_block);
                self.leave_finally_block(compiler, end_case, finally_block);
            }
        }

        // Transpile "catch" block
        if catch_block.has_children(compiler) {
            check_state!(
                catch_block
                    .get_first_child(compiler)
                    .unwrap()
                    .is_catch(compiler)
            );

            self.switch_case_to(compiler, catch_case.unwrap());
            let exception_name = catch_block
                .get_first_first_child(compiler)
                .unwrap()
                .detach(compiler);
            self.enter_catch_block(compiler, finally_case, exception_name);

            let catch_body = catch_block
                .get_first_first_child(compiler)
                .unwrap()
                .detach(compiler);
            check_state!(catch_body.is_block(compiler));
            self.transpile_statement(compiler, catch_body);
            self.leave_catch_block(compiler, finally_case, catch_body);
        }

        self.switch_case_to(compiler, end_case);
    }

    // Transpiles "switch" statement.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#transpileSwitch
    fn transpile_switch(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        break_case: Option<CaseId>,
    ) {
        // Transpile condition first
        let first = n.remove_first_child(compiler);
        let decomposed = self.maybe_decompose_expression(compiler, first).unwrap();
        n.add_child_to_front(compiler, decomposed);

        // Are all "switch" cases unmarked?
        let mut has_generator_marker = false;
        let switch_body = n.get_second_child(compiler).unwrap();
        let mut case_section = switch_body.get_first_child(compiler);
        while let Some(section) = case_section {
            if section.is_generator_marker(compiler) {
                has_generator_marker = true;
                break;
            }
            case_section = section.get_next(compiler);
        }
        // No transpilation is needed
        if !has_generator_marker {
            n.set_generator_marker(compiler, false);
            self.transpile_unmarked_node(compiler, n);
            return;
        }

        /* Stores a detached body of a case statement and a case section assosiated with it. */
        struct SwitchCase {
            generated_case: CaseId,
            body: NodeId,
        }

        // TODO(skill): Don't move all case sections.
        let mut detached_cases: Vec<SwitchCase> = Vec::new();

        // We don't have to transpile unmarked cases at the beginning of "switch".
        let mut can_skip_unmarked_cases = true;
        let mut case_section = switch_body.get_first_child(compiler);
        while let Some(section) = case_section {
            if !section.is_default_case(compiler)
                && section
                    .get_first_child(compiler)
                    .unwrap()
                    .is_generator_marker(compiler)
            {
                // Following example is possible to transpile, but it's not trivial.
                // switch (cond) {
                //   case yield "test": break;
                //   case 5 + yield:  break;
                // }
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &transpilation_util::CANNOT_CONVERT_YET,
                    &["Case statements that contain yields"],
                ));
                return;
            }
            let body = section.get_last_child(compiler).unwrap();

            if !body.has_children(compiler)
                || (can_skip_unmarked_cases && !body.is_generator_marker(compiler))
            {
                // Can skip empty or unmarked case.
                case_section = section.get_next(compiler);
                continue;
            }

            // Check whether we can start skipping unmarked cases again
            can_skip_unmarked_cases = Self::is_end_of_block_unreachable(compiler, body);

            // Move case's body under a global switch statement...

            // Allocate a new case
            let generated_case = self.create_case(compiler);
            let case_block = self.case_ref(generated_case).case_block;
            case_block.srcref(compiler, body);

            // Replace old body with a jump instruction.
            let jump = self.create_jump_to_node(compiler, generated_case, body);
            let new_body = IR::block_with_child(compiler, jump);
            new_body.set_is_added_block(compiler, true);
            // make sure we don't transpile generated "jump" instruction
            new_body.set_generator_safe(compiler, true);
            body.replace_with(compiler, new_body);

            // Remember the body and the case under which the body will be moved.
            // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.SwitchCase#SwitchCase
            detached_cases.push(SwitchCase {
                generated_case,
                body,
            });

            section.set_generator_marker(compiler, false);
            case_section = section.get_next(compiler);
        }

        let end_case = self.maybe_create_case(compiler, break_case);

        // Transpile the barebone of original "switch" statement
        n.set_generator_marker(compiler, false);
        switch_body.set_generator_marker(compiler, false);
        self.transpile_unmarked_node(compiler, n);
        self.write_jump_to(compiler, end_case, n); // TODO(skill): do not always add this.

        // Transpile all detached case bodies
        self.push_break_context(end_case);
        for detached_case in detached_cases {
            let generated_case = detached_case.generated_case;
            self.switch_case_to(compiler, generated_case);
            self.transpile_statement(compiler, detached_case.body);
        }
        self.pop_break_context();

        self.switch_case_to(compiler, end_case);
    }

    /// Finds the only YIELD node in a tree.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#findYield
    fn find_yield(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        let mut yield_finder = YieldFinder { yield_node: None };
        NodeTraversal::traverse(compiler, n, &mut yield_finder);
        yield_finder.get_yield_node()
    }

    /// Returns whether any statements added to the end of the block would be unreachable.
    ///
    /// It's OK for this method to return false-negatives.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler#isEndOfBlockUnreachable
    fn is_end_of_block_unreachable(compiler: &AbstractCompiler, block: NodeId) -> bool {
        check_state!(block.is_block(compiler));
        if !block.has_children(compiler) {
            return false;
        }
        let last_child = block.get_last_child(compiler).unwrap();
        match last_child.get_token(compiler) {
            Token::BLOCK => Self::is_end_of_block_unreachable(compiler, last_child),
            Token::RETURN | Token::THROW | Token::CONTINUE | Token::BREAK => true,
            _ => false,
        }
    }
}

/// Finds the only YIELD node in a tree.
struct YieldFinder {
    yield_node: Option<NodeId>,
}

impl YieldFinder {
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.YieldFinder#getYieldNode
    fn get_yield_node(&self) -> NodeId {
        check_not_null!(self.yield_node)
    }
}

impl Callback for YieldFinder {
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.YieldFinder#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_function(t) {
            return false;
        }
        if n.is_yield(t) {
            check_state!(self.yield_node.is_none());
            self.yield_node = Some(n);
            return false;
        }
        true
    }

    // AbstractPreOrderCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}

/// The methods of `TranspilationContext` (see the module comment).
impl SingleGeneratorFunctionTranspiler<'_> {
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#TranspilationContext
    fn transpilation_context_init(&mut self, compiler: &mut AbstractCompiler) {
        self.context.program_end_case = self.new_case(compiler);
        check_state!(self.case_ref(self.context.program_end_case).id == 0);
        self.context.current_case = self.new_case(compiler);
        check_state!(self.case_ref(self.context.current_case).id == 1);
        self.context.all_cases.push(self.context.current_case);
    }

    fn case_ref(&self, case: CaseId) -> &Case {
        &self.context.cases[case.0]
    }

    fn case_mut(&mut self, case: CaseId) -> &mut Case {
        &mut self.context.cases[case.0]
    }

    /// Removes unnecessary cases.
    ///
    /// This optimization is needed to reduce number of switch cases, which is used then to
    /// generate even shorter state machine programs.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#optimizeCaseIds
    fn optimize_case_ids(&mut self, compiler: &mut AbstractCompiler) {
        // Java passes allCases as the message: an empty list prints "[]".
        check_state!(!self.context.all_cases.is_empty(), "[]");

        // Shortcut jump chains:
        //   case 100:
        //     $context.yieldValue("something", 101);
        //     break;
        //   case 101:
        //     $context.jumpTo(102);
        //     break;
        //   case 102:
        //     $context.jumpTo(200);
        //     break;
        // becomes:
        //   case 100:
        //     $context.yieldValue("something", 200);
        //     break;
        //   case 101:
        //     $context.jumpTo(102);
        //     break;
        //   case 102:
        //     $context.jumpTo(200);
        //     break;
        for i in 0..self.context.all_cases.len() {
            let current_case = self.context.all_cases[i];
            if let Some(mut jump_to) = self.case_ref(current_case).jump_to {
                // Flatten jumps chains:
                // 1 -> 2
                // 2 -> 8
                // 8 -> 300
                // to:
                // 1 -> 300
                // 2 -> 300
                // 8 -> 300
                while let Some(next) = self.case_ref(jump_to).jump_to {
                    jump_to = next;
                    self.case_mut(current_case).jump_to = Some(jump_to);
                }

                if self.case_ref(current_case).embed_into.is_some()
                    && self.case_ref(current_case).references.len() == 1
                {
                    self.case_mut(jump_to).embed_into = self.case_ref(current_case).embed_into;
                }
                self.case_mut(current_case).embed_into = None;

                // Update references to jump to the final case in the chain
                let jump_to_id = self.case_ref(jump_to).id;
                for reference in self.case_ref(current_case).references.clone() {
                    reference.set_double(compiler, f64::from(jump_to_id));
                }
                let references = std::mem::take(&mut self.case_mut(current_case).references);
                self.case_mut(jump_to).references.extend(references);
            }
        }

        // Merge cases without any references with the previous case:
        //   case 100:
        //     doSomething();
        //   case 101:
        //     doSomethingElse();
        //     break;
        //   case 102:
        //     doEvenMore();
        // becomes:
        //   case 100:
        //     doSomething();
        //     doSomethingElse();
        //     break;
        // Java's Iterator over allCases with it.remove(): `i` is the index of the next element.
        let mut prev_case = self.context.all_cases[0];
        let mut i = 1;
        check_state!(self.case_ref(prev_case).id == 1);
        while i < self.context.all_cases.len() {
            let current_case = self.context.all_cases[i];
            if self.case_ref(current_case).references.is_empty() {
                // No jump references, just append the body to a previous case if needed.
                check_state!(self.case_ref(current_case).embed_into.is_none());
                if self.case_ref(prev_case).may_fall_through {
                    let children = self
                        .case_ref(current_case)
                        .case_block
                        .remove_children(compiler);
                    self.case_ref(prev_case)
                        .case_block
                        .add_children_to_back(compiler, children);
                    self.case_mut(prev_case).may_fall_through =
                        self.case_ref(current_case).may_fall_through;
                }
                self.context.all_cases.remove(i);
                continue;
            }
            if let Some(embed_into) = self.case_ref(current_case).embed_into {
                check_state!(self.case_ref(current_case).jump_to.is_none());
                // Cases can be embedded only if they are referenced once and don't fall through.
                if self.case_ref(current_case).references.len() == 1
                    && !self.case_ref(current_case).may_fall_through
                {
                    let case_block = self.case_ref(current_case).case_block;
                    embed_into.replace_with(compiler, case_block);
                    self.context.all_cases.remove(i);
                    continue;
                }
            }
            if self.case_ref(prev_case).jump_to == Some(current_case) {
                // Merging "case 1:" with the following case. The standard merging cannot be used
                // as "case 1:" is an entry point and it cannot be renamed.
                //   case 1:
                //   case 2:
                //     doSomethingElse();
                //     break;
                //   case 102:
                //     $context.jumpTo(2);
                //     break;
                // becomes:
                //   case 1:
                //     doSomethingElse();
                //     break;
                //   case 102:
                //     $context.jumpTo(1);
                //     break;
                check_state!(self.case_ref(prev_case).may_fall_through);
                check_state!(!self.case_ref(prev_case).case_block.has_children(compiler));
                check_state!(self.case_ref(current_case).jump_to.is_none());

                let children = self
                    .case_ref(current_case)
                    .case_block
                    .remove_children(compiler);
                self.case_ref(prev_case)
                    .case_block
                    .add_children_to_back(compiler, children);
                self.case_mut(prev_case).may_fall_through =
                    self.case_ref(current_case).may_fall_through;
                let prev_id = self.case_ref(prev_case).id;
                for reference in self.case_ref(current_case).references.clone() {
                    reference.set_double(compiler, f64::from(prev_id));
                }
                self.case_mut(prev_case).jump_to = self.case_ref(current_case).jump_to;
                let references = self.case_ref(current_case).references.clone();
                self.case_mut(prev_case).references.extend(references);
                self.context.all_cases.remove(i);
                continue;
            }
            prev_case = current_case;
            i += 1;
        }
    }

    /// Replaces "...; break;" with "return ...;".
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#eliminateSwitchBreaks
    fn eliminate_switch_breaks(&mut self, compiler: &mut AbstractCompiler) {
        for break_node in std::mem::take(&mut self.context.switch_breaks) {
            let prev_statement = break_node.get_previous(compiler);
            check_state!(prev_statement.is_some());
            let prev_statement = prev_statement.unwrap();
            check_state!(prev_statement.is_expr_result(compiler));
            let expr = prev_statement.remove_first_child(compiler).unwrap();
            let return_node = IR::return_node_with_expression(compiler, expr);
            prev_statement.replace_with(compiler, return_node);
            break_node.detach(compiler);
        }
        // Java's switchBreaks.clear() is the take above.
    }

    /// Finalizes transpilation by dumping all generated "case" nodes.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#finalizeTransformation
    fn finalize_transformation(&mut self, compiler: &mut AbstractCompiler, generator_body: NodeId) {
        let af = self.ast_factory();
        self.optimize_case_ids(compiler);

        // If number of cases is small we render them without using "switch"
        //   switch ($context.getNextAddressJsc()) {
        //     case 1: a();
        //     case 2: b();
        //     case 3: c();
        //   }
        // are rendered as:
        //   if ($context.getNextAddressJsc() == 1) a();
        //   if ($context.getNextAddressJsc() != 3) b();
        //   c();
        if self.context.all_cases.len() == 2 || self.context.all_cases.len() == 3 {
            let call = self.call_context_method(
                compiler,
                generator_body,
                "getNextAddressJsc",
                AstFactory::type_native_and_color(
                    JSTypeNative::NUMBER_TYPE,
                    standard_colors::NUMBER.clone(),
                ),
                &[],
            );
            let one = af.create_number(compiler, 1.0);
            let eq = af.create_eq(compiler, call, one);
            let removed = self.context.all_cases.remove(0);
            let case_block = self.case_ref(removed).case_block;
            let if_node = af
                .create_if(compiler, eq, case_block)
                .srcref_tree_if_missing(compiler, generator_body);
            generator_body.add_child_to_back(compiler, if_node);
        }

        // If number of cases is small we render them without using "switch"
        //   switch ($context.getNextAddressJsc()) {
        //     case 1: a();
        //     case 2: b();
        //   }
        // are rendered as:
        //   if ($context.getNextAddressJsc() == 1) a();
        //   b();
        if self.context.all_cases.len() == 2 {
            let call = self.call_context_method(
                compiler,
                generator_body,
                "getNextAddressJsc",
                AstFactory::type_native_and_color(
                    JSTypeNative::NUMBER_TYPE,
                    standard_colors::NUMBER.clone(),
                ),
                &[],
            );
            let second_id = self.case_ref(self.context.all_cases[1]).id;
            let number = af.create_number(compiler, f64::from(second_id));
            let ne = af.create_ne(compiler, call, number);
            let removed = self.context.all_cases.remove(0);
            let case_block = self.case_ref(removed).case_block;
            let if_node = af
                .create_if(compiler, ne, case_block)
                .srcref_tree_if_missing(compiler, generator_body);
            generator_body.add_child_to_back(compiler, if_node);
        }

        // If number of cases is small we render them without using "switch"
        //   switch ($context.getNextAddressJsc()) {
        //     case 1: a();
        //   }
        // are rendered as:
        //   a();
        if self.context.all_cases.len() == 1 {
            let removed = self.context.all_cases.remove(0);
            let children = self.case_ref(removed).case_block.remove_children(compiler);
            generator_body.add_children_to_back(compiler, children);
            self.eliminate_switch_breaks(compiler);
            return;
        }

        //  switch ($jscomp$generator$context.getNextAddressJsc()) {}
        let call = self.call_context_method(
            compiler,
            generator_body,
            "getNextAddressJsc",
            AstFactory::type_native_and_color(
                JSTypeNative::NUMBER_TYPE,
                standard_colors::NUMBER.clone(),
            ),
            &[],
        );
        let switch_node = IR::switch_node(compiler, call, &[]).srcref(compiler, generator_body);
        generator_body.add_child_to_back(compiler, switch_node);
        let switch_body = switch_node
            .get_second_child(compiler)
            .unwrap()
            .srcref(compiler, generator_body);

        // Populate "switch" statement with "case"s.
        for current_case in self.context.all_cases.clone() {
            let case_node = self.case_create_case_node(compiler, current_case);
            switch_body.add_child_to_back(compiler, case_node);
        }
        self.context.all_cases.clear();
    }

    /// Ensures that the context has an empty state.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#checkStateIsEmpty
    fn check_state_is_empty(&self) {
        check_state!(self.context.named_labels.is_empty());
        check_state!(self.context.break_cases.is_empty());
        check_state!(self.context.continue_cases.is_empty());
        check_state!(self.context.catch_cases.is_empty());
        check_state!(self.context.finally_cases.is_empty());
        check_state!(self.context.nested_finally_block_count == 0);
        check_state!(self.context.all_cases.is_empty());
    }

    /// Adds a block of original code to the end of the current case.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#transpileUnmarkedBlock
    fn transpile_unmarked_block(&mut self, compiler: &mut AbstractCompiler, block: NodeId) {
        if block.has_children(compiler) {
            let mut transpiler = UnmarkedNodeTranspiler::new(self);
            NodeTraversal::traverse(compiler, block, &mut transpiler);
            while block.has_children(compiler) {
                let n = block.remove_first_child(compiler).unwrap();
                self.write_generated_node(compiler, n);
            }
        }
    }

    /// Adds a new generated node to the end of the current case.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#writeGeneratedNode
    fn write_generated_node(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let current_case = self.context.current_case;
        self.case_add_node(compiler, current_case, n);
    }

    /// Adds a new generated node to the end of the current case and finializes it.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#writeGeneratedNodeAndBreak
    fn write_generated_node_and_break(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        self.write_generated_node(compiler, n);
        let break_node = self.create_break_node_for(compiler, n);
        self.write_generated_node(compiler, break_node);
        self.case_mut(self.context.current_case).may_fall_through = false;
    }

    /// Creates a new detached case statement.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#createCase
    fn create_case(&mut self, compiler: &mut AbstractCompiler) -> CaseId {
        self.new_case(compiler)
    }

    /// Returns a passed case object or creates a new one if it's null.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#maybeCreateCase
    fn maybe_create_case(
        &mut self,
        compiler: &mut AbstractCompiler,
        other: Option<CaseId>,
    ) -> CaseId {
        if let Some(other) = other {
            return other;
        }
        self.create_case(compiler)
    }

    /// Returns the name node of context parameter passed to the program.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#getJsContextNameNode
    fn get_js_context_name_node(
        &self,
        compiler: &mut AbstractCompiler,
        source_node: NodeId,
    ) -> NodeId {
        let type_ = self.pass.generator_context_type(compiler);
        self.ast_factory()
            .create_name(compiler, self.get_scoped_name(GENERATOR_CONTEXT), type_)
            .srcref(compiler, source_node)
    }

    /// Returns unique name in the current context.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#getScopedName
    fn get_scoped_name(&self, name: &str) -> String {
        format!("{name}${}", self.unique_id)
    }

    /// Creates node that access a specified field of the current context.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#getContextField
    fn get_context_field(
        &self,
        compiler: &mut AbstractCompiler,
        source_node: NodeId,
        field_name: &str,
    ) -> NodeId {
        let name = self.get_js_context_name_node(compiler, source_node);
        self.ast_factory()
            .create_get_prop_with_unknown_type(compiler, name, field_name)
            .srcref(compiler, source_node)
    }

    /// Creates node that make a call to a context function.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#callContextMethod
    fn call_context_method(
        &self,
        compiler: &mut AbstractCompiler,
        source_node: NodeId,
        method_name: &str,
        type_: Type,
        args: &[NodeId],
    ) -> NodeId {
        let context_field = self.get_context_field(compiler, source_node, method_name);
        self.ast_factory()
            .create_call(compiler, context_field, type_, args)
            .srcref(compiler, source_node)
    }

    /// Creates node that make a call to a context function.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#callContextMethodResult
    fn call_context_method_result(
        &self,
        compiler: &mut AbstractCompiler,
        source_node: NodeId,
        method_name: &str,
        type_: Type,
        args: &[NodeId],
    ) -> NodeId {
        let call = self.call_context_method(compiler, source_node, method_name, type_, args);
        self.ast_factory()
            .expr_result(compiler, call)
            .srcref(compiler, source_node)
    }

    /// Creates node that returns the result of a call to a context function.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#returnContextMethod
    fn return_context_method(
        &self,
        compiler: &mut AbstractCompiler,
        source_node: NodeId,
        method_name: &str,
        type_: Type,
        args: &[NodeId],
    ) -> NodeId {
        let call = self.call_context_method(compiler, source_node, method_name, type_, args);
        self.ast_factory()
            .create_return(compiler, call)
            .srcref(compiler, source_node)
    }

    /// Creates a "break;" statement that will follow `preBreak` node.
    ///
    /// This is used to be able to generatate a state machine program outside of "swtich"
    /// statement so:
    ///
    /// ```text
    ///   $context.jumpTo(5);
    ///   break;
    /// ```
    ///
    /// could be converted into:
    ///
    /// ```text
    ///   return $context.jumpTo(5);
    /// ```
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#createBreakNodeFor
    fn create_break_node_for(
        &mut self,
        compiler: &mut AbstractCompiler,
        pre_break: NodeId,
    ) -> NodeId {
        let break_node = IR::break_node(compiler).srcref(compiler, pre_break);
        self.context.switch_breaks.push(break_node);
        break_node
    }

    /// Returns a node that instructs a state machine program to jump to a selected case section.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#createJumpToNode
    fn create_jump_to_node(
        &mut self,
        compiler: &mut AbstractCompiler,
        section: CaseId,
        source_node: NodeId,
    ) -> NodeId {
        let number = self.case_get_number(compiler, section, source_node);
        self.return_context_method(
            compiler,
            source_node,
            "jumpTo",
            type_color(&standard_colors::NULL_OR_VOID),
            &[number],
        )
    }

    /// Instructs a state machine program to jump to a selected case section.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#writeJumpTo
    fn write_jump_to(
        &mut self,
        compiler: &mut AbstractCompiler,
        section: CaseId,
        source_node: NodeId,
    ) {
        let jump_block = self.create_jump_to_block(
            compiler,
            section,
            /* allowEmbedding= */ false,
            source_node,
        );
        let current_case = self.context.current_case;
        self.case_jump_to(compiler, current_case, section, jump_block);
    }

    /// Creates a block node that contains a jump instruction.
    ///
    /// `allow_embedding`: Whether the code from the target section can be embedded into jump
    /// block.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#createJumpToBlock
    fn create_jump_to_block(
        &mut self,
        compiler: &mut AbstractCompiler,
        section: CaseId,
        allow_embedding: bool,
        source_node: NodeId,
    ) -> NodeId {
        check_state!(self.case_ref(section).embed_into.is_none());
        let number = self.case_get_number(compiler, section, source_node);
        let jump = self.call_context_method_result(
            compiler,
            source_node,
            "jumpTo",
            type_color(&standard_colors::NULL_OR_VOID),
            &[number],
        );
        let break_node = self.create_break_node_for(compiler, source_node);
        let jump_block = self
            .ast_factory()
            .create_block(compiler, &[jump, break_node])
            .srcref(compiler, source_node);
        if allow_embedding {
            self.case_mut(section).embed_into = Some(jump_block);
        }
        jump_block
    }

    /// Converts "break" and "continue" statements into state machine jumps.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#replaceBreakContinueWithJump
    fn replace_break_continue_with_jump(
        &mut self,
        compiler: &mut AbstractCompiler,
        source_node: NodeId,
        section: CaseId,
        break_suppressors: i32,
    ) {
        let jump_method: &str;
        if self.context.nested_finally_block_count > 0 {
            // If we are in a finally block, we need to use jumpThroughFinallyBlocks to ensure
            // that the finally block is correctly exited.
            jump_method = "jumpThroughFinallyBlocks";
        } else if self.context.finally_cases.is_empty()
            || self.case_ref(self.context.finally_cases[0]).id < self.case_ref(section).id
        {
            // There are no finally blocks that should be exectuted pior to jumping
            jump_method = "jumpTo";
        } else {
            // There are some finally blocks that should be exectuted before we can
            // break/continue.
            check_state!(
                self.case_ref(self.context.finally_cases[0]).id != self.case_ref(section).id
            );
            jump_method = "jumpThroughFinallyBlocks";
        }
        if break_suppressors == 0 {
            // continue;  =>  $context.jumpTo(x); break;
            let number = self.case_get_number(compiler, section, source_node);
            let jump = self.call_context_method_result(
                compiler,
                source_node,
                jump_method,
                type_color(&standard_colors::NULL_OR_VOID),
                &[number],
            );
            jump.insert_before(compiler, source_node);
            let break_node = self.create_break_node_for(compiler, source_node);
            source_node.replace_with(compiler, break_node);
        } else {
            // "break;" inside a loop or swtich statement:
            // for (...) {
            //   break l1;
            // }
            // becomes:
            // for (...) {                  // loop doesn't allow to use "break" to advance to the
            //   return $context.jumpTo(x); // next address, so "return" is used instead.
            // }
            let number = self.case_get_number(compiler, section, source_node);
            let ret = self.return_context_method(
                compiler,
                source_node,
                jump_method,
                type_color(&standard_colors::NULL_OR_VOID),
                &[number],
            );
            source_node.replace_with(compiler, ret);
        }
    }

    /// Instructs a state machine program to yield a value and then jump to a selected case
    /// section.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#yieldValue
    fn yield_value(
        &mut self,
        compiler: &mut AbstractCompiler,
        expression: Option<NodeId>,
        jump_to_section: CaseId,
        source_node: NodeId,
    ) {
        let first_arg = match expression {
            None => self
                .ast_factory()
                .create_undefined_value(compiler)
                .srcref_tree(compiler, source_node),
            Some(expression) => expression,
        };
        let args: Vec<NodeId> = vec![
            first_arg,
            self.case_get_number(compiler, jump_to_section, source_node),
        ];
        let ret = self.return_context_method(
            compiler,
            source_node,
            "yield",
            type_color(&standard_colors::UNKNOWN),
            &args,
        );
        self.write_generated_node(compiler, ret);
        self.case_mut(self.context.current_case).may_fall_through = false;
    }

    /// Instructs a state machine program to yield all values and then jump to a selected case
    /// section.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#yieldAll
    fn yield_all(
        &mut self,
        compiler: &mut AbstractCompiler,
        expression: NodeId,
        jump_to_section: CaseId,
        source_node: NodeId,
    ) {
        let number = self.case_get_number(compiler, jump_to_section, source_node);
        let ret = self.return_context_method(
            compiler,
            source_node,
            "yieldAll",
            type_color(&standard_colors::UNKNOWN),
            &[expression, number],
        );
        self.write_generated_node(compiler, ret);
        self.case_mut(self.context.current_case).may_fall_through = false;
    }

    /// Instructs a state machine program to return a given expression.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#returnExpression
    fn return_expression(
        &self,
        compiler: &mut AbstractCompiler,
        source_node: NodeId,
        expression: Option<NodeId>,
    ) -> NodeId {
        let Some(expression) = expression else {
            return self.call_context_method(
                compiler,
                source_node,
                "return",
                type_color(&standard_colors::NULL_OR_VOID),
                &[],
            );
        };
        self.call_context_method(
            compiler,
            source_node,
            "return",
            type_color(&standard_colors::NULL_OR_VOID),
            &[expression],
        )
    }

    /// Instructs a state machine program to consume a yield result after yielding.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#yieldResult
    fn yield_result(&self, compiler: &mut AbstractCompiler, source_node: NodeId) -> NodeId {
        self.call_context_method(
            compiler,
            source_node,
            "getYieldResultJsc",
            type_color(&standard_colors::UNKNOWN),
            &[],
        )
    }

    /// Adds references to catch and finally blocks to the transpilation context.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#addCatchFinallyCases
    fn add_catch_finally_cases(
        &mut self,
        catch_case: Option<CaseId>,
        finally_case: Option<CaseId>,
    ) {
        if let Some(finally_case) = finally_case {
            if let Some(first) = self.context.catch_cases.front_mut() {
                first.finally_blocks += 1;
            }
            self.context.finally_cases.push_front(finally_case);
        }
        if let Some(catch_case) = catch_case {
            self.context
                .catch_cases
                .push_front(CatchCase::new(catch_case));
        }
    }

    /// Returns the case section of the next catch block that is not hidden by finally blocks.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#getNextCatchCase
    fn get_next_catch_case(&self) -> Option<CaseId> {
        // Java loops over catchCases and breaks after the first element.
        if let Some(catch_case) = self.context.catch_cases.front()
            && catch_case.finally_blocks == 0
        {
            return Some(catch_case.catch_case);
        }
        None
    }

    /// Returns the case section of the next finally block.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#getNextFinallyCase
    fn get_next_finally_case(&self) -> Option<CaseId> {
        self.context.finally_cases.front().copied()
    }

    /// Removes references to catch and finally blocks from the transpilation context.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#removeCatchFinallyCases
    fn remove_catch_finally_cases(
        &mut self,
        catch_case: Option<CaseId>,
        finally_case: Option<CaseId>,
    ) {
        if let Some(catch_case) = catch_case {
            let last_catch = self
                .context
                .catch_cases
                .pop_front()
                .expect("java.util.NoSuchElementException");
            check_state!(last_catch.finally_blocks == 0);
            check_state!(last_catch.catch_case == catch_case);
        }
        if let Some(finally_case) = finally_case {
            if let Some(first) = self.context.catch_cases.front_mut() {
                first.finally_blocks -= 1;
                let finally_blocks = first.finally_blocks;
                check_state!(finally_blocks >= 0);
            }
            let last_finally = self
                .context
                .finally_cases
                .pop_front()
                .expect("java.util.NoSuchElementException");
            check_state!(last_finally == finally_case);
        }
    }

    /// Writes a statement Node that should be placed at the beginning of try block.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#enterTryBlock
    fn enter_try_block(
        &mut self,
        compiler: &mut AbstractCompiler,
        catch_case: Option<CaseId>,
        finally_case: Option<CaseId>,
        source_node: NodeId,
    ) {
        self.add_catch_finally_cases(catch_case, finally_case);

        let method_name: &str;
        let mut args: Vec<NodeId> = Vec::new();
        match catch_case {
            None => {
                method_name = "setFinallyBlock";
                let finally_case = finally_case.expect("java.lang.NullPointerException");
                args.push(self.case_get_number(compiler, finally_case, source_node));
            }
            Some(catch_case) => {
                method_name = "setCatchFinallyBlocks";
                args.push(self.case_get_number(compiler, catch_case, source_node));
                if let Some(finally_case) = finally_case {
                    args.push(self.case_get_number(compiler, finally_case, source_node));
                }
            }
        }
        let call = self.call_context_method_result(
            compiler,
            source_node,
            method_name,
            type_color(&standard_colors::NULL_OR_VOID),
            &args,
        );
        self.write_generated_node(compiler, call);
    }

    /// Writes a statements that should be placed at the end of try block if finally block is not
    /// present.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#leaveTryBlock
    fn leave_try_block(
        &mut self,
        compiler: &mut AbstractCompiler,
        catch_case: Option<CaseId>,
        end_case: CaseId,
        source_node: NodeId,
    ) {
        self.remove_catch_finally_cases(catch_case, None);
        let mut args: Vec<NodeId> = Vec::new();
        args.push(self.case_get_number(compiler, end_case, source_node));
        // Find the next catch block that is not hidden by any finally blocks.
        let next_catch_case = self.get_next_catch_case();
        if let Some(next_catch_case) = next_catch_case {
            args.push(self.case_get_number(compiler, next_catch_case, source_node));
        }
        let call = self.call_context_method_result(
            compiler,
            source_node,
            "leaveTryBlock",
            type_color(&standard_colors::NULL_OR_VOID),
            &args,
        );
        self.write_generated_node_and_break(compiler, call);
    }

    /// Writes a statement Node that should be placed at the beginning of catch block.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#enterCatchBlock
    fn enter_catch_block(
        &mut self,
        compiler: &mut AbstractCompiler,
        finally_case: Option<CaseId>,
        exception_name: NodeId,
    ) {
        let af = self.ast_factory();
        check_state!(exception_name.is_name(compiler));
        self.add_catch_finally_cases(None, finally_case);

        // Find the next catch block that is not hidden by any finally blocks.
        let next_catch_case = self.get_next_catch_case();

        if self
            .context
            .catch_names
            .insert(exception_name.get_string(compiler))
        {
            let name = exception_name.clone_node(compiler);
            let var = IR::var(compiler, name).srcref(compiler, exception_name);
            self.hoist_var_node(compiler, var);
        }

        let mut args: Vec<NodeId> = Vec::new();
        if let Some(next_catch_case) = next_catch_case {
            args.push(self.case_get_number(compiler, next_catch_case, exception_name));
        }

        let enter_catch_block_call = self.call_context_method(
            compiler,
            exception_name,
            "enterCatchBlock",
            type_color(&standard_colors::UNKNOWN),
            &args,
        );
        let color = enter_catch_block_call.get_color(compiler);
        exception_name.set_color(compiler, color);
        let assign = af
            .create_assign(compiler, exception_name, enter_catch_block_call)
            .srcref(compiler, exception_name);
        let expr_result = af
            .expr_result(compiler, assign)
            .srcref(compiler, exception_name);
        self.write_generated_node(compiler, expr_result);
    }

    /// Writes a statement to jump to the finally block if it's present.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#leaveCatchBlock
    fn leave_catch_block(
        &mut self,
        compiler: &mut AbstractCompiler,
        finally_case: Option<CaseId>,
        source_node: NodeId,
    ) {
        if let Some(finally_case) = finally_case {
            self.remove_catch_finally_cases(None, Some(finally_case));
            self.write_jump_to(compiler, finally_case, source_node);
        }
    }

    /// Writes a Node that should be placed at the beginning of finally block.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#enterFinallyBlock
    fn enter_finally_block(
        &mut self,
        compiler: &mut AbstractCompiler,
        catch_case: Option<CaseId>,
        finally_case: Option<CaseId>,
        source_node: NodeId,
    ) {
        let af = self.ast_factory();
        self.remove_catch_finally_cases(catch_case, finally_case);

        let next_catch_case = self.get_next_catch_case();
        let next_finally_case = self.get_next_finally_case();

        let mut args: Vec<NodeId> = Vec::new();
        if self.context.nested_finally_block_count == 0 {
            if next_catch_case.is_some() || next_finally_case.is_some() {
                args.push(match next_catch_case {
                    None => af
                        .create_number(compiler, 0.0)
                        .srcref(compiler, source_node),
                    Some(next_catch_case) => {
                        self.case_get_number(compiler, next_catch_case, source_node)
                    }
                });
                if let Some(next_finally_case) = next_finally_case {
                    args.push(self.case_get_number(compiler, next_finally_case, source_node));
                }
            }
        } else {
            args.push(match next_catch_case {
                None => af
                    .create_number(compiler, 0.0)
                    .srcref(compiler, source_node),
                Some(next_catch_case) => {
                    self.case_get_number(compiler, next_catch_case, source_node)
                }
            });
            args.push(match next_finally_case {
                None => af
                    .create_number(compiler, 0.0)
                    .srcref(compiler, source_node),
                Some(next_finally_case) => {
                    self.case_get_number(compiler, next_finally_case, source_node)
                }
            });
            args.push(
                af.create_number(compiler, f64::from(self.context.nested_finally_block_count))
                    .srcref(compiler, source_node),
            );
        }

        let call = self.call_context_method_result(
            compiler,
            source_node,
            "enterFinallyBlock",
            type_color(&standard_colors::NULL_OR_VOID),
            &args,
        );
        self.write_generated_node(compiler, call);

        self.context.nested_finally_block_count += 1;
    }

    /// Writes a Node that should be placed at the end of finally block.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#leaveFinallyBlock
    fn leave_finally_block(
        &mut self,
        compiler: &mut AbstractCompiler,
        end_case: CaseId,
        source_node: NodeId,
    ) {
        let mut args: Vec<NodeId> = Vec::new();
        args.push(self.case_get_number(compiler, end_case, source_node));
        self.context.nested_finally_block_count -= 1;
        if self.context.nested_finally_block_count != 0 {
            args.push(
                self.ast_factory()
                    .create_number(compiler, f64::from(self.context.nested_finally_block_count))
                    .srcref(compiler, source_node),
            );
        }

        let call = self.call_context_method_result(
            compiler,
            source_node,
            "leaveFinallyBlock",
            type_color(&standard_colors::NULL_OR_VOID),
            &args,
        );
        self.write_generated_node_and_break(compiler, call);
    }

    /// Changes the `currentCase` to a new one.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#switchCaseTo
    fn switch_case_to(&mut self, compiler: &mut AbstractCompiler, case_section: CaseId) {
        let current_case = self.context.current_case;
        self.case_will_follow_by(compiler, current_case, case_section);
        self.context.all_cases.push(case_section);
        self.context.current_case = case_section;
    }

    /// Adds a named labels to the context.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#pushLabels
    fn push_labels(
        &mut self,
        compiler: &AbstractCompiler,
        label_names: &[NodeId],
        break_case: CaseId,
        continue_case: Option<CaseId>,
    ) {
        for label_name in label_names {
            check_state!(label_name.is_label_name(compiler));
            self.context.named_labels.insert(
                label_name.get_string(compiler),
                LabelCases::new(break_case, continue_case),
            );
        }
    }

    /// Removes the named labels from the context.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#popLabels
    fn pop_labels(&mut self, compiler: &AbstractCompiler, label_names: &[NodeId]) {
        for label_name in label_names {
            check_state!(label_name.is_label_name(compiler));
            // LinkedHashMap#remove keeps the order of the remaining entries.
            self.context
                .named_labels
                .shift_remove(&label_name.get_string(compiler));
        }
    }

    /// Adds "break" jump point to the context
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#pushBreakContext
    fn push_break_context(&mut self, break_case: CaseId) {
        self.context.break_cases.push_front(break_case);
    }

    /// Adds "break" and "continue" jump points to the context
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#pushBreakContinueContext
    fn push_break_continue_context(&mut self, break_case: CaseId, continue_case: CaseId) {
        self.push_break_context(break_case);
        self.context.continue_cases.push_front(continue_case);
    }

    /// Removes "break" jump point from the context, restoring the previous one
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#popBreakContext
    fn pop_break_context(&mut self) {
        self.context
            .break_cases
            .pop_front()
            .expect("java.util.NoSuchElementException");
    }

    /// Removes "break" and "continue" jump points from the context, restoring the previous ones.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext#popBreakContinueContext
    fn pop_break_continue_context(&mut self) {
        self.pop_break_context();
        self.context
            .continue_cases
            .pop_front()
            .expect("java.util.NoSuchElementException");
    }
}

/// The methods of `TranspilationContext.Case`; `case` is the receiver.
impl SingleGeneratorFunctionTranspiler<'_> {
    /// Creates a new empty case section and assings a new id.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.Case#Case
    fn new_case(&mut self, compiler: &mut AbstractCompiler) -> CaseId {
        let id = self.context.case_id_counter;
        self.context.case_id_counter += 1;
        let case_block = self
            .ast_factory()
            .create_block(compiler, &[])
            .srcref(compiler, self.original_generator_body);
        let case = CaseId(self.context.cases.len());
        self.context.cases.push(Case {
            id,
            case_block,
            references: Vec::new(),
            jump_to: None,
            embed_into: None,
            may_fall_through: true,
        });
        case
    }

    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.Case#createCaseNode
    fn case_create_case_node(&self, compiler: &mut AbstractCompiler, case: CaseId) -> NodeId {
        let case_block = self.case_ref(case).case_block;
        let number = self
            .ast_factory()
            .create_number(compiler, f64::from(self.case_ref(case).id))
            .srcref(compiler, case_block);
        IR::case_node(compiler, number, case_block).srcref(compiler, case_block)
    }

    /// Returns the number node of the case section and increments a reference counter.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.Case#getNumber
    fn case_get_number(
        &mut self,
        compiler: &mut AbstractCompiler,
        case: CaseId,
        source_node: NodeId,
    ) -> NodeId {
        if let Some(jump_to) = self.case_ref(case).jump_to {
            return self.case_get_number(compiler, jump_to, source_node);
        }
        let node = self
            .ast_factory()
            .create_number(compiler, f64::from(self.case_ref(case).id))
            .srcref(compiler, source_node);
        self.case_mut(case).references.push(node);
        node
    }

    /// Finalizes the case section with a jump instruction.
    ///
    /// `addNode` cannot be invoked after this method is called.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.Case#jumpTo
    fn case_jump_to(
        &mut self,
        compiler: &mut AbstractCompiler,
        case: CaseId,
        other: CaseId,
        jump_block: NodeId,
    ) {
        check_state!(jump_block.is_block(compiler));
        check_state!(self.case_ref(case).jump_to.is_none());
        self.case_will_follow_by(compiler, case, other);
        let children = jump_block.remove_children(compiler);
        self.case_ref(case)
            .case_block
            .add_children_to_back(compiler, children);
        self.case_mut(case).may_fall_through = false;
    }

    /// Informs which other case will be executed after this one.
    ///
    /// It's used to detect and then eliminate case statements that are used as simple jump
    /// hops:
    ///
    /// ```text
    ///  case 100:
    ///    $context.jumpTo(200);
    ///    break;
    /// ```
    ///
    /// or
    ///
    /// ```text
    ///  case 300:
    /// ```
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.Case#willFollowBy
    fn case_will_follow_by(&mut self, compiler: &AbstractCompiler, case: CaseId, other: CaseId) {
        if self.case_ref(case).jump_to.is_none()
            && !self.case_ref(case).case_block.has_children(compiler)
        {
            check_state!(self.case_ref(other).jump_to.is_none());
            self.case_mut(case).jump_to = Some(other);
        }
    }

    /// Adds a new node to the end of the case block.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.Case#addNode
    fn case_add_node(&mut self, compiler: &mut AbstractCompiler, case: CaseId, n: NodeId) {
        check_state!(self.case_ref(case).jump_to.is_none());
        check_state!(IR::may_be_statement(compiler, n));
        self.case_ref(case)
            .case_block
            .add_child_to_back(compiler, n);
    }
}

/// Adjust YIELD-free nodes to run correctly inside a state machine program.
///
/// The following transformations are performed:
///
/// - moving `var` into hoist scope;
/// - transpiling `return` statements;
/// - transpiling `break` and `continue` statements;
/// - transpiling references to `this` and `arguments`.
struct UnmarkedNodeTranspiler<'b, 'a> {
    /// Rust-only: the enclosing `SingleGeneratorFunctionTranspiler` (Java's `context.new ..`).
    owner: &'b mut SingleGeneratorFunctionTranspiler<'a>,

    // Count the number of enclosing statements that a bare break could address.
    // A value > 0 means that a bare break statement we encounter can be left unmodified,
    // since it addresses a statement within the node we are transpiling.
    break_suppressors: i32,
    // Same as breakSuppressors, but for bare continue statements.
    continue_suppressors: i32,
}

impl<'b, 'a> UnmarkedNodeTranspiler<'b, 'a> {
    fn new(owner: &'b mut SingleGeneratorFunctionTranspiler<'a>) -> Self {
        Self {
            owner,
            break_suppressors: 0,
            continue_suppressors: 0,
        }
    }

    /// Adjust return statements.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.UnmarkedNodeTranspiler#visitReturn
    fn visit_return(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        // return ...;   =>   return $context.return(...);
        let first = n.remove_first_child(compiler);
        let expression = self.owner.return_expression(compiler, n, first);
        n.add_child_to_front(compiler, expression);
    }

    /// Converts labeled `break` or `continue` statement into a jump.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.UnmarkedNodeTranspiler#visitNamedBreakContinue
    fn visit_named_break_continue(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let first = n.get_first_child(compiler).unwrap();
        check_state!(first.is_label_name(compiler));
        let cases = self
            .owner
            .context
            .named_labels
            .get(&first.get_string(compiler))
            .copied();
        if let Some(cases) = cases {
            let case_section = if n.is_break(compiler) {
                Some(cases.break_case)
            } else {
                cases.continue_case
            };
            self.owner.replace_break_continue_with_jump(
                compiler,
                n,
                case_section.expect("java.lang.NullPointerException"),
                self.break_suppressors,
            );
        }
    }

    /// Converts `break` or `continue` statement into a jump.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.UnmarkedNodeTranspiler#visitBreakContinue
    fn visit_break_continue(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let mut case_section: Option<CaseId> = None;
        if n.is_break(compiler) && self.break_suppressors == 0 {
            case_section = Some(
                *self
                    .owner
                    .context
                    .break_cases
                    .front()
                    .expect("java.util.NoSuchElementException"),
            );
        }
        if n.is_continue(compiler) && self.continue_suppressors == 0 {
            case_section = Some(
                *self
                    .owner
                    .context
                    .continue_cases
                    .front()
                    .expect("java.util.NoSuchElementException"),
            );
        }
        if let Some(case_section) = case_section {
            self.owner.replace_break_continue_with_jump(
                compiler,
                n,
                case_section,
                self.break_suppressors,
            );
        }
    }

    /// Replaces reference to `this` with `$jscomp$generator$this`.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.UnmarkedNodeTranspiler#visitThis
    fn visit_this(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let name = self.owner.get_scoped_name(GENERATOR_THIS);
        let new_this =
            self.owner
                .ast_factory()
                .create_name(compiler, name, AstFactory::type_node(n));
        n.replace_with(compiler, new_this);
        if !self.owner.context.this_reference_found {
            let lhs = new_this.clone_node(compiler).srcref(compiler, n);
            let hoist_block = self.owner.hoist_block();
            let var = IR::var_with_value(compiler, lhs, n).srcref(compiler, hoist_block);
            self.owner.hoist_var_node(compiler, var);
            self.owner.context.this_reference_found = true;
        }
    }

    /// Replaces reference to `arguments` with `$jscomp$generator$arguments`.
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.UnmarkedNodeTranspiler#visitArguments
    fn visit_arguments(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let name = self.owner.get_scoped_name(GENERATOR_ARGUMENTS);
        let new_arguments = self
            .owner
            .ast_factory()
            .create_name(compiler, name, AstFactory::type_node(n))
            .srcref(compiler, n);
        n.replace_with(compiler, new_arguments);
        if !self.owner.context.arguments_reference_found {
            let lhs = new_arguments.clone_node(compiler);
            let hoist_block = self.owner.hoist_block();
            let var = IR::var_with_value(compiler, lhs, n).srcref(compiler, hoist_block);
            self.owner.hoist_var_node(compiler, var);
            self.owner.context.arguments_reference_found = true;
        }
    }

    /// Hoists `var` statements into the closure containing the generator to preserve their
    /// state across multiple invocation of state machine program.
    ///
    /// ```text
    /// var a = "test", b = i + 5;
    /// ```
    ///
    /// is transpiled to:
    ///
    /// ```text
    /// var a, b;
    /// a = "test", b = i + 5;
    /// ```
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.UnmarkedNodeTranspiler#visitVarStatement
    fn visit_var_statement(&mut self, compiler: &mut AbstractCompiler, var_statement: NodeId) {
        let comma_expression =
            self.extract_assignments_to_comma_expression(compiler, var_statement);
        match comma_expression {
            None => {
                var_statement.detach(compiler);
            }
            Some(comma_expression) => {
                let expr_result = self
                    .owner
                    .ast_factory()
                    .expr_result(compiler, comma_expression);
                var_statement.replace_with(compiler, expr_result);
            }
        }
        // Move declaration without initial values to just before the program method definition.
        self.owner.hoist_var_node(compiler, var_statement);
    }

    /// Hoists `var` declarations in vanilla for loops into the closure containing the generator
    /// to preserve their state across multiple invocation of state machine program.
    ///
    /// ```text
    /// for (var a = "test", b = i + 5; ... ; )
    /// ```
    ///
    /// is transpiled to:
    ///
    /// ```text
    /// var a, b;
    /// for (a = "test", b = i + 5; ...; )
    /// ```
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.UnmarkedNodeTranspiler#visitVanillaForLoopVar
    fn visit_vanilla_for_loop_var(
        &mut self,
        compiler: &mut AbstractCompiler,
        var_declaration: NodeId,
    ) {
        let comma_expression =
            self.extract_assignments_to_comma_expression(compiler, var_declaration);
        match comma_expression {
            None => {
                // `for (var x; ` becomes `for (; `
                let empty = IR::empty(compiler);
                var_declaration.replace_with(compiler, empty);
            }
            Some(comma_expression) => {
                // `for (var i = 0, j = 0; `... becomes `for (i = 0, j = 0; `...
                var_declaration.replace_with(compiler, comma_expression);
            }
        }
        // Move declaration without initial values to just before the program method definition.
        self.owner.hoist_var_node(compiler, var_declaration);
    }

    /// Hoists `var` declarations in for-in loops into the closure containing the generator to
    /// preserve their state across multiple invocation of state machine program.
    ///
    /// ```text
    /// for (var a in obj)
    /// ```
    ///
    /// is transpiled to:
    ///
    /// ```text
    /// var a;
    /// for (a in obj))
    /// ```
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.UnmarkedNodeTranspiler#visitForInLoopVar
    fn visit_for_in_loop_var(&mut self, compiler: &mut AbstractCompiler, var_declaration: NodeId) {
        // `for (var varName in ` ...
        let var_name = var_declaration.get_only_child(compiler);
        check_state!(
            !var_name.has_children(compiler),
            &var_name.to_string(compiler)
        );
        let cloned_var_name = var_name.clone_node(compiler).set_jsdoc_info(compiler, None);
        // becomes `for (varName in ` ...
        var_declaration.replace_with(compiler, cloned_var_name);
        // Move declaration without initial values to just before the program method definition.
        self.owner.hoist_var_node(compiler, var_declaration);
    }

    /// Removes all initializers from a var declaration and returns them as a single expression
    /// of comma-separated assignments or null if there aren't any initializers.
    ///
    /// `var_declaration`: VAR node. Returns null or expression node (e.g.
    /// `varName1 = 1, varName2 = y`)
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.UnmarkedNodeTranspiler#extractAssignmentsToCommaExpression
    fn extract_assignments_to_comma_expression(
        &mut self,
        compiler: &mut AbstractCompiler,
        var_declaration: NodeId,
    ) -> Option<NodeId> {
        let af = self.owner.ast_factory();
        let mut assignments: Vec<NodeId> = Vec::new();
        let mut var_name = var_declaration.get_first_child(compiler);
        while let Some(name) = var_name {
            if name.has_children(compiler) {
                let copied_var_name = name.clone_node(compiler).set_jsdoc_info(compiler, None);
                let value = name.remove_first_child(compiler).unwrap();
                let assign = af
                    .create_assign(compiler, copied_var_name, value)
                    .srcref(compiler, name);
                assignments.push(assign);
            }
            var_name = name.get_next(compiler);
        }
        let mut comma_expression: Option<NodeId> = None;
        for assignment in assignments {
            comma_expression = Some(match comma_expression {
                None => assignment,
                Some(comma_expression) => af
                    .create_comma(compiler, comma_expression, assignment)
                    .srcref(compiler, assignment),
            });
        }
        comma_expression
    }
}

impl Callback for UnmarkedNodeTranspiler<'_, '_> {
    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.UnmarkedNodeTranspiler#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if n.is_generator_safe(t) {
            // Skip nodes that were generated by the compiler.
            n.set_generator_safe(t, false);
            return false;
        }

        check_state!(!n.is_generator_marker(t));
        check_state!(!n.is_super(t), "Reference to SUPER is not supported");

        if NodeUtil::is_loop_structure(t, n) {
            self.continue_suppressors += 1;
            self.break_suppressors += 1;
        } else if n.is_switch(t) {
            self.break_suppressors += 1;
        }

        if n.is_break(t) || n.is_continue(t) {
            if n.has_children(t) {
                self.visit_named_break_continue(t.get_compiler(), n);
            } else {
                self.visit_break_continue(t.get_compiler(), n);
            }
            return false;
        }

        !n.is_function(t)
    }

    // port: Es6RewriteGenerators.SingleGeneratorFunctionTranspiler.TranspilationContext.UnmarkedNodeTranspiler#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if NodeUtil::is_loop_structure(t, n) {
            self.continue_suppressors -= 1;
            self.break_suppressors -= 1;
        } else if n.is_switch(t) {
            self.break_suppressors -= 1;
        } else if n.is_this(t) {
            self.visit_this(t.get_compiler(), n);
        } else if n.is_return(t) {
            self.visit_return(t.get_compiler(), n);
        } else if n.is_name(t) && n.get_string_ref(t) == "arguments" {
            self.visit_arguments(t.get_compiler(), n);
        } else if n.is_var(t) {
            let parent = parent.expect("java.lang.NullPointerException");
            if parent.is_vanilla_for(t) {
                self.visit_vanilla_for_loop_var(t.get_compiler(), n);
            } else if parent.is_for_in(t) {
                self.visit_for_in_loop_var(t.get_compiler(), n);
            } else {
                // NOTE: for-of loops are transpiled away before this pass
                self.visit_var_statement(t.get_compiler(), n);
            }
        } // else no changes need to be made
    }
}

/// Marks "yield" nodes and propagates this information up through the tree
struct YieldNodeMarker;

impl Callback for YieldNodeMarker {
    // port: Es6RewriteGenerators.YieldNodeMarker#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        !n.is_function(t)
    }

    // port: Es6RewriteGenerators.YieldNodeMarker#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_yield(t) {
            n.set_generator_marker(t, true);
        }

        // This class is used on a tree that is detached from the main AST, so this will not end
        // up marking the parent of the node used to start the traversal.
        if let Some(parent) = parent
            && n.is_generator_marker(t)
        {
            parent.set_generator_marker(t, true);
        }
    }
}
