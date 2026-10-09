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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/TypeInferenceTest.java.

//! Port of TypeInferenceTest.
//!
//! These unit tests don't test the full type inference that happens in a normal compilation. This
//! is because a normal compilation creates many instances of the TypeInference class. More
//! precisely, one instance is created for every scope that is a valid root of a control-flow graph
//! (as defined by NodeUtil#isValidCfgRoot(Node) such as, for example, functions.
//!
//! These unit tests only ever create a single TypeInference instance. That means: these unit tests
//! ignore any code nested within a function.
//!
//! The fixture keeps Java's fields in RefCells: Java helpers nest (`assuming("x",
//! createNullableType(createRecordType(..)))`) and all mutate the same compiler.
#![allow(clippy::too_many_lines)]
use closure_jscomp::{
    closure_coding_convention::ClosureCodingConvention,
    coding_convention::{AssertionFunctionLookup, CodingConvention},
    compiler::Compiler,
    compiler_options::{CompilerOptions, LanguageMode},
    compiler_pass::CompilerPass,
    control_flow_analysis::ControlFlowAnalysis,
    data_flow_analysis::{DataFlowAnalysis, LinearFlowState},
    deps::module_loader::ResolutionMode,
    flow_scope::FlowScope,
    gather_module_metadata::GatherModuleMetadata,
    graph::annotatable::Annotatable,
    js_chunk::JSChunk,
    modules::module_map_creator::ModuleMapCreator,
    node_traversal::{AbstractScopedCallback, NodeTraversal},
    type_inference::{BigIntPresence, FlowScopeLattice, TypeInference},
    type_inference_pass::SharedTypedScopeCreator,
    typed_scope::TypedScope,
    typed_scope_creator::TypedScopeCreator,
};
use closure_jstype::{
    TypeId, enum_type, enum_type::EnumType, function_type, function_type::FunctionType,
    js_type::JSType, js_type_native::JSTypeNative, js_type_native::JSTypeNative::*,
    js_type_registry::PropDefinitionKind, js_type_resolver::Closer,
    known_symbol_type::KnownSymbolType, object_type::ObjectType, property::PropertyKey,
    property::SymbolKey, testing::type_subject::TypeSubject,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{
    closure_primitive::ClosurePrimitive, ir::IR, js_string::JsString, node::NodeId, token::Token,
};
use closure_testing::testing::scope_subject::{ScopeSubject, assert_scope};
use std::cell::{RefCell, RefMut};
use std::sync::Arc;

type FS = Arc<dyn FlowScope>;

/// Java's overloads taking either a `JSType` or a `JSTypeNative`.
trait AsType {
    fn ty(self, t: &TypeInferenceTest) -> TypeId;
}
impl AsType for TypeId {
    fn ty(self, _t: &TypeInferenceTest) -> TypeId {
        self
    }
}
impl AsType for JSTypeNative {
    fn ty(self, t: &TypeInferenceTest) -> TypeId {
        t.get_native_type(self)
    }
}

// port: TypeInferenceTest.LabeledStatement
/// Stores information about a labeled statement and allows making assertions on it.
#[derive(Clone, Copy)]
struct LabeledStatement {
    statement_node: NodeId,
    enclosing_scope: TypedScope,
}

struct TypeInferenceTest {
    compiler: RefCell<Compiler>,
    closer: RefCell<Option<Closer>>,
    /// Java's `HashMap<String, JSType>`; declared in insertion order.
    assumptions: RefCell<IndexMap<JsString, TypeId>>,
    assumed_this_type: RefCell<Option<TypeId>>,
    return_scope: RefCell<Option<FS>>,
    assertion_function_map: AssertionFunctionLookup,
    /// Maps a label name to information about the labeled statement.
    ///
    /// This map is recreated each time parseAndRunTypeInference() is executed.
    labeled_statement_map: RefCell<IndexMap<JsString, LabeledStatement>>,
}

/// A TypeSubject bound to the fixture's registry (Java's TypeSubject reaches the registry
/// through the type).
struct TS<'a> {
    t: &'a TypeInferenceTest,
    s: TypeSubject,
}

impl TS<'_> {
    fn is_equal_to(&self, other: impl AsType) {
        let other = other.ty(self.t);
        let mut compiler = self.t.compiler.borrow_mut();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        self.s.is_equal_to(reg, ast, other);
    }
    fn is_not_equal_to(&self, other: impl AsType) {
        let other = other.ty(self.t);
        let mut compiler = self.t.compiler.borrow_mut();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        self.s.is_not_equal_to(reg, ast, other);
    }
    fn to_string_is_equal_to(&self, expected: &str) {
        let mut compiler = self.t.compiler.borrow_mut();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        self.s.to_string_is_equal_to(reg, ast, expected);
    }
    fn is_number(&self) {
        let mut compiler = self.t.compiler.borrow_mut();
        self.s.is_number(compiler.get_type_registry());
    }
    fn is_string(&self) {
        let mut compiler = self.t.compiler.borrow_mut();
        self.s.is_string(compiler.get_type_registry());
    }
    fn is_unknown(&self) {
        let mut compiler = self.t.compiler.borrow_mut();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        self.s.is_unknown(reg, ast);
    }
    fn is_subtype_of(&self, other: impl AsType) {
        let other = other.ty(self.t);
        let mut compiler = self.t.compiler.borrow_mut();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        self.s.is_subtype_of(reg, ast, other);
    }
}

impl TypeInferenceTest {
    // port: TypeInferenceTest#setUp
    fn set_up() -> Self {
        let mut compiler = Compiler::new();
        let mut options = CompilerOptions::new();
        options.set_closure_pass(true);
        options.set_language_in(LanguageMode::UNSUPPORTED);
        compiler.init_options(options);
        let closer = compiler
            .get_type_registry()
            .get_resolver()
            .open_for_definition();
        Self {
            compiler: RefCell::new(compiler),
            closer: RefCell::new(Some(closer)),
            assumptions: RefCell::new(IndexMap::<_, _>::default()),
            assumed_this_type: RefCell::new(None),
            return_scope: RefCell::new(None),
            assertion_function_map: AssertionFunctionLookup::of(
                ClosureCodingConvention::new().get_assertion_functions(),
            ),
            labeled_statement_map: RefCell::new(IndexMap::<_, _>::default()),
        }
    }

    fn c(&self) -> RefMut<'_, Compiler> {
        self.compiler.borrow_mut()
    }

    // port: TypeInferenceTest#assumingThisType
    fn assuming_this_type(&self, type_: TypeId) {
        *self.assumed_this_type.borrow_mut() = Some(type_);
    }

    // port: TypeInferenceTest#assuming
    /// Declares a name with a given type in the parent scope of the test case code.
    fn assuming(&self, name: &str, type_: impl AsType) {
        let type_ = type_.ty(self);
        self.assumptions
            .borrow_mut()
            .insert(JsString::from(name), type_);
    }

    // port: TypeInferenceTest#inFunction
    /// Runs an instance of TypeInference over the given code after wrapping it in a function.
    ///
    /// Does not visit any nested functions.
    fn in_function(&self, js: &str) {
        // Parse the body of the function.
        let assumed_this_type = *self.assumed_this_type.borrow();
        let this_block = match assumed_this_type {
            None => String::new(),
            Some(type_) => format!("/** @this {{{}}} */", self.to_string(type_)),
        };
        self.parse_and_run_type_inference(&format!("({this_block} function() {{{js}}});"));
    }

    // port: TypeInferenceTest#inScript
    /// Runs an instance of TypeInference over the given code.
    ///
    /// Does not visit any nested functions.
    fn in_script(&self, js: &str) {
        let root = {
            let mut compiler = self.c();
            let compiler = &mut *compiler;
            let script = compiler.parse_test_code(js);
            assert_eq!(
                compiler.get_error_count(),
                0,
                "parsing error: {}",
                Self::errors(compiler)
            );
            let externs = IR::root(compiler, &[]);
            let main = IR::root(compiler, &[script]);
            let root = IR::root(compiler, &[externs, main]);
            Self::gather_module_metadata_and_map(compiler, externs, main);
            root
        };
        self.parse_and_run_type_inference_at(root, root);
    }

    // port: TypeInferenceTest#inGenerator
    /// Runs an instance of TypeInference over the given code after wrapping it in a generator.
    ///
    /// Does not visit any nested functions.
    fn in_generator(&self, js: &str) {
        assert!(self.assumed_this_type.borrow().is_none());
        self.parse_and_run_type_inference(&format!("(function *() {{{js}}});"));
    }

    // port: TypeInferenceTest#withModules
    fn with_modules(&self, js: &[&str]) {
        let root = {
            let mut compiler = self.c();
            let compiler = &mut *compiler;
            let script =
                compiler.parse_test_code_many(js.iter().map(|s| JsString::from(*s)).collect());
            let chunk = JSChunk::new("entry");
            let inputs: Vec<_> = compiler
                .get_inputs_by_id()
                .values()
                .map(|input| input.get_source_file_arc())
                .collect();
            for source_file in inputs {
                chunk.add_source_file(source_file);
            }
            let options = compiler.get_options().clone();
            compiler.init_chunks(&[], vec![chunk], options);
            compiler.initialize_module_loader();
            assert_eq!(
                compiler.get_error_count(),
                0,
                "parsing error: {}",
                Self::errors(compiler)
            );
            let externs = IR::root(compiler, &[]);
            let main = IR::root(compiler, &[script]);
            let root = IR::root(compiler, &[externs, main]);
            Self::gather_module_metadata_and_map(compiler, externs, main);
            root
        };
        self.parse_and_run_type_inference_at(root, root);
    }

    /// `new GatherModuleMetadata(..).process(..)` and `new ModuleMapCreator(..).process(..)`.
    fn gather_module_metadata_and_map(compiler: &mut Compiler, externs: NodeId, main: NodeId) {
        GatherModuleMetadata::new(
            /* processCommonJsModules= */ false,
            ResolutionMode::BROWSER,
        )
        .process(compiler, externs, main);
        let module_metadata_map = compiler.get_module_metadata_map().cloned().unwrap();
        ModuleMapCreator::new(module_metadata_map).process(compiler, externs, main);
    }

    /// `Joiner.on(", ").join(compiler.getErrors())`.
    fn errors(compiler: &Compiler) -> String {
        compiler
            .get_errors()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    }

    // port: TypeInferenceTest#parseAndRunTypeInference(String)
    fn parse_and_run_type_inference(&self, js: &str) {
        let (root, function) = {
            let mut compiler = self.c();
            let compiler = &mut *compiler;
            let script = compiler.parse_test_code(js);
            let externs = IR::root(compiler, &[]);
            let main = IR::root(compiler, &[script]);
            let root = IR::root(compiler, &[externs, main]);
            assert_eq!(
                compiler.get_error_count(),
                0,
                "parsing error: {}",
                Self::errors(compiler)
            );

            // SCRIPT -> EXPR_RESULT -> FUNCTION
            // `(function() { TEST CODE HERE });`
            let function = script.get_first_first_child(compiler).unwrap();
            (root, function)
        };
        self.parse_and_run_type_inference_at(root, function);
    }

    // port: TypeInferenceTest#parseAndRunTypeInference(Node,Node)
    fn parse_and_run_type_inference_at(&self, root: NodeId, cfg_root: NodeId) {
        let mut compiler = self.c();
        let compiler = &mut *compiler;
        {
            let mut closer = self.closer.borrow_mut().take().unwrap();
            let (reg, ast) = compiler.get_type_registry_and_ast();
            closer.close(reg, ast);
        }

        let scope_creator = RefCell::new(TypedScopeCreator::new(compiler));
        let assumed_scope;
        {
            let mut closer = compiler
                .get_type_registry()
                .get_resolver()
                .open_for_definition();
            // Create the scope with the assumptions.
            // Also populate a map allowing us to look up labeled statements later.
            let mut labeled_statement_map: IndexMap<JsString, LabeledStatement> =
                IndexMap::<_, _>::default();
            {
                let mut creator = SharedTypedScopeCreator(&scope_creator);
                let mut callback = AbstractScopedCallback::with_scope_callbacks(
                    |t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>| {
                        let scope = t.get_typed_scope();
                        let compiler = t.get_compiler();
                        if let Some(parent) = parent
                            && parent.is_label(compiler)
                            && !n.is_label_name(compiler)
                        {
                            // First child of a LABEL is a LABEL_NAME, n is the second child.
                            let label_name_node = n.get_previous(compiler).unwrap();
                            assert!(label_name_node.is_label_name(compiler));
                            let label_name = label_name_node.get_string(compiler).clone();
                            assert!(
                                !labeled_statement_map.contains_key(&label_name),
                                "Duplicate label name: {label_name}"
                            );
                            labeled_statement_map.insert(
                                label_name,
                                LabeledStatement {
                                    statement_node: n,
                                    enclosing_scope: scope,
                                },
                            );
                        }
                    },
                    |t: &mut NodeTraversal<'_>| {
                        t.get_typed_scope();
                    },
                    |_t: &mut NodeTraversal<'_>| {},
                );
                NodeTraversal::builder()
                    .set_compiler(compiler)
                    .set_callback(&mut callback)
                    .set_scope_creator(&mut creator)
                    .traverse(root);
            }
            *self.labeled_statement_map.borrow_mut() = labeled_statement_map;
            assumed_scope = scope_creator
                .borrow_mut()
                .create_scope_for_node(compiler, cfg_root);
            for (name, type_) in self.assumptions.borrow().iter() {
                assumed_scope.declare(compiler, name.clone(), None, Some(*type_), None, false);
            }
            scope_creator
                .borrow()
                .resolve_weak_imports_pre_resolution(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            closer.close(reg, ast);
        }
        scope_creator.borrow_mut().finish_and_freeze(compiler);
        // Create the control graph.
        let cfg = ControlFlowAnalysis::builder()
            .set_compiler(compiler)
            .set_cfg_root(cfg_root)
            .set_traverse_functions(true)
            .set_include_edge_annotations(true)
            .compute_cfg(compiler);
        // Create a simple reverse abstract interpreter.
        let rai = compiler.get_reverse_abstract_interpreter();
        // Do the type inference by data-flow analysis.
        let mut scope_creator = scope_creator.into_inner();
        let rtn_in: FS = {
            let mut dfa = TypeInference::new(
                compiler,
                cfg,
                rai,
                assumed_scope,
                &mut scope_creator,
                &self.assertion_function_map,
            );
            dfa.analyze(compiler);
            // Get the scope of the implicit return.
            let cfg = dfa.get_cfg();
            let rtn_state = cfg
                .get_implicit_return()
                .get_annotation_as::<LinearFlowState<FlowScopeLattice>>(cfg)
                .unwrap();
            rtn_state.get_in().clone()
        };
        let return_scope = if cfg_root.is_function(compiler) {
            // Reset the flow scope's syntactic scope to the function block, rather than the
            // function node itself.  This allows pulling out local vars from the function by name
            // to verify their types.
            let block = cfg_root.get_last_child(compiler).unwrap();
            let block_scope = scope_creator.create_scope_for_node(compiler, block);
            rtn_in.with_syntactic_scope(compiler, block_scope)
        } else {
            rtn_in
        };
        *self.return_scope.borrow_mut() = Some(return_scope);

        *self.closer.borrow_mut() = Some(
            compiler
                .get_type_registry()
                .get_resolver()
                .open_for_definition(),
        );
    }

    // port: TypeInferenceTest#getLabeledStatement
    fn get_labeled_statement(&self, label: &str) -> LabeledStatement {
        let map = self.labeled_statement_map.borrow();
        *map.get(&JsString::from(label))
            .unwrap_or_else(|| panic!("No statement found for label: {label}"))
    }

    // port: TypeInferenceTest#assertScopeEnclosing
    /// Returns a ScopeSubject for the scope containing the labeled statement.
    ///
    /// Asserts that a statement with the given label existed in the code last passed to
    /// parseAndRunTypeInference().
    fn assert_scope_enclosing<'a>(
        &self,
        compiler: &'a mut Compiler,
        label: &str,
    ) -> ScopeSubject<'a, TypedScope> {
        assert_scope(compiler, self.get_labeled_statement(label).enclosing_scope)
    }

    // port: TypeInferenceTest#assertTypeOfExpression
    /// Returns a TypeSubject for the JSType of the expression with the given label.
    ///
    /// Asserts that a statement with the given label existed in the code last passed to
    /// parseAndRunTypeInference(). Also asserts that the statement is an EXPR_RESULT whose
    /// expression has a non-null JSType.
    fn assert_type_of_expression(&self, label: &str) -> TS<'_> {
        let statement_node = self.get_labeled_statement(label).statement_node;
        let compiler = self.c();
        assert!(
            statement_node.is_expr_result(&compiler),
            "Not an expression statement."
        );
        let js_type = statement_node
            .get_only_child(&compiler)
            .get_jstype(&compiler);
        assert!(js_type.is_some(), "Expression type is null");
        self.assert_type(js_type)
    }

    /// `assertType(type)` bound to the fixture's registry.
    fn assert_type(&self, type_: Option<TypeId>) -> TS<'_> {
        TS {
            t: self,
            s: TypeSubject::assert_type(type_),
        }
    }

    // port: TypeInferenceTest#getType
    fn get_type(&self, name: &str) -> TypeId {
        let return_scope = self.return_scope.borrow().clone();
        let return_scope = return_scope.expect("The return scope should not be null.");
        let mut compiler = self.c();
        let var = return_scope
            .get_slot(&mut compiler, &JsString::from(name))
            .unwrap_or_else(|| panic!("The variable {name} is missing from the scope."));
        var.get_type(&compiler).unwrap()
    }

    // port: TypeInferenceTest#getParamNameNode
    /// Returns the NAME node `name` from the PARAM_LIST of the top level of type inference.
    fn get_param_name_node(&self, name: &str) -> NodeId {
        let return_scope = self.return_scope.borrow().clone().unwrap();
        let mut compiler = self.c();
        let compiler = &mut *compiler;
        let static_scope = return_scope.get_declaration_scope(compiler);
        let slot = static_scope.get_slot(compiler, name).unwrap();
        let declaration = slot.get_declaration(compiler).unwrap();
        let node = declaration.get_node(compiler).unwrap();

        assert_eq!(node.get_token(compiler), Token::NAME);
        let mut has_param_list_ancestor = false;
        let mut ancestor = node.get_parent(compiler);
        while let Some(a) = ancestor {
            if a.is_param_list(compiler) {
                has_param_list_ancestor = true;
            }
            ancestor = a.get_parent(compiler);
        }
        assert!(has_param_list_ancestor);

        node
    }

    // port: TypeInferenceTest#verify
    fn verify(&self, name: &str, type_: impl AsType) {
        let type_ = type_.ty(self);
        let actual = self.get_type(name);
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let equal = actual.equals(reg, ast, type_);
        if !equal {
            let actual_string = actual.to_string(reg, ast);
            let expected_string = type_.to_string(reg, ast);
            panic!("Mismatch for {name}: expected {expected_string} but was {actual_string}");
        }
    }

    // port: TypeInferenceTest#verifyUnequal
    fn verify_unequal(&self, name: &str, type_: impl AsType) {
        let type_ = type_.ty(self);
        let actual = self.get_type(name);
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        assert!(
            !actual.equals(reg, ast, type_),
            "Mismatch for {name}: expected not to be {}",
            type_.to_string(reg, ast)
        );
    }

    // port: TypeInferenceTest#verifySubtypeOf
    fn verify_subtype_of(&self, name: &str, type_: impl AsType) {
        let type_ = type_.ty(self);
        let var_type = self.get_type(name);
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        assert!(
            var_type.is_subtype_of(reg, ast, type_),
            "The type {} of variable {name} is not a subtype of {}.",
            var_type.to_string(reg, ast),
            type_.to_string(reg, ast)
        );
    }

    // port: TypeInferenceTest#createEnumType
    fn create_enum_type(&self, name: &str, elem_type: impl AsType) -> TypeId {
        let elem_type = elem_type.ty(self);
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        enum_type::builder()
            .set_name(name)
            .set_element_type(elem_type)
            .build(reg, ast)
    }

    // port: TypeInferenceTest#createUndefinableType
    fn create_undefinable_type(&self, type_: JSTypeNative) -> TypeId {
        self.registry_create_union_type(&[
            self.get_native_type(type_),
            self.get_native_type(VOID_TYPE),
        ])
    }

    // port: TypeInferenceTest#createNullableType
    fn create_nullable_type(&self, type_: impl AsType) -> TypeId {
        let type_ = type_.ty(self);
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        reg.create_nullable_type(ast, type_)
    }

    // port: TypeInferenceTest#createUnionType
    fn create_union_type(&self, type1: JSTypeNative, type2: JSTypeNative) -> TypeId {
        self.registry_create_union_type(&[self.get_native_type(type1), self.get_native_type(type2)])
    }

    // port: TypeInferenceTest#createRecordType
    /// Returns a record type with a field `fieldName` and JSType specified by `fieldType`.
    fn create_record_type(&self, field_name: &str, field_type: impl AsType) -> TypeId {
        let field_type = field_type.ty(self);
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        reg.create_record_type(ast, [(JsString::from(field_name), field_type)])
    }

    // port: TypeInferenceTest#createMultiParamUnionType
    fn create_multi_param_union_type(&self, variants: &[JSTypeNative]) -> TypeId {
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        reg.create_union_type_from_native(ast, variants)
    }

    // port: TypeInferenceTest#testForAllBigInt
    fn test_for_all_big_int(&self, type_: TypeId) {
        assert_eq!(
            TypeInference::get_big_int_presence(&mut self.c(), type_),
            BigIntPresence::ALL_BIGINT
        );
    }

    // port: TypeInferenceTest#testForNoBigInt
    fn test_for_no_big_int(&self, type_: TypeId) {
        assert_eq!(
            TypeInference::get_big_int_presence(&mut self.c(), type_),
            BigIntPresence::NO_BIGINT
        );
    }

    // port: TypeInferenceTest#testForBigIntOrNumber
    fn test_for_big_int_or_number(&self, type_: TypeId) {
        assert_eq!(
            TypeInference::get_big_int_presence(&mut self.c(), type_),
            BigIntPresence::BIGINT_OR_NUMBER
        );
    }

    // port: TypeInferenceTest#testForBigIntOrOther
    fn test_for_big_int_or_other(&self, type_: TypeId) {
        assert_eq!(
            TypeInference::get_big_int_presence(&mut self.c(), type_),
            BigIntPresence::BIGINT_OR_OTHER
        );
    }

    // port: TypeInferenceTest#getNativeObjectType
    fn get_native_object_type(&self, t: JSTypeNative) -> TypeId {
        self.c().get_type_registry().get_native_object_type(t)
    }

    // port: TypeInferenceTest#getNativeType
    fn get_native_type(&self, t: JSTypeNative) -> TypeId {
        self.c().get_type_registry().get_native_type(t)
    }

    // port: TypeInferenceTest#templatize
    fn templatize(&self, obj_type: TypeId, t: &[TypeId]) -> TypeId {
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        reg.create_templatized_type(ast, obj_type, t)
    }

    // port: TypeInferenceTest#includeGoogAssertionFn
    /// Adds a goog.asserts.assert[name] function to the scope that asserts the given returnType
    fn include_goog_assertion_fn(&self, fn_name: &str, return_type: TypeId) {
        let full_name = format!("goog.asserts.{fn_name}");
        let unknown = self.get_native_type(UNKNOWN_TYPE);
        let fn_type = {
            let mut compiler = self.c();
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let parameters = reg.create_parameters(&[unknown]);
            function_type::builder()
                .with_return_type(return_type)
                .with_parameters(parameters)
                .with_name(full_name.as_str())
                .build(reg, ast)
        };
        self.assuming(&full_name, fn_type);
    }

    // port: TypeInferenceTest#includePrimitiveTruthyAssertionFunction
    /// Adds a function with ClosurePrimitive#ASSERTS_TRUTHY and the given name
    fn include_primitive_truthy_assertion_function(&self, fn_name: &str) {
        let assert_type = {
            let mut compiler = self.c();
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let t = reg.create_template_type(ast, "T");
            let parameters = reg.create_parameters(&[t]);
            function_type::builder()
                .with_name(fn_name)
                .with_closure_primitive_id(Some(ClosurePrimitive::ASSERTS_TRUTHY))
                .with_return_type(t)
                .with_parameters(parameters)
                .with_template_keys(reg, ast, vec![t])
                .build(reg, ast)
        };
        self.assuming(fn_name, assert_type);
    }

    // port: TypeInferenceTest#includePrimitiveAssertionFn
    /// Adds a function with ClosurePrimitive#ASSERTS_MATCHES_RETURN that asserts the given
    /// returnType
    fn include_primitive_assertion_fn(&self, full_name: &str, return_type: TypeId) {
        let unknown = self.get_native_type(UNKNOWN_TYPE);
        let fn_type = {
            let mut compiler = self.c();
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let parameters = reg.create_parameters(&[unknown]);
            function_type::builder()
                .with_return_type(return_type)
                .with_parameters(parameters)
                .with_name(full_name)
                .with_closure_primitive_id(Some(ClosurePrimitive::ASSERTS_MATCHES_RETURN))
                .build(reg, ast)
        };
        self.assuming(full_name, fn_type);
    }

    // port: TypeInferenceTest#includeAssertInstanceof
    /// Adds goog.asserts.assertInstanceof to the scope, to do fine-grained assertion testing
    fn include_assert_instanceof(&self) {
        let full_name = "goog.asserts.assertInstanceof";
        let unknown = self.get_native_type(UNKNOWN_TYPE);
        let fn_type = {
            let mut compiler = self.c();
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let template_type = reg.create_template_type(ast, "T");
            // Create the function type `function(new:T)`
            let template_type_ctor = function_type::builder()
                .for_constructor()
                .with_type_of_this(template_type)
                .build(reg, ast);
            // Create the function type `function(?, function(new:T)): T`
            // This matches the JSDoc for goog.asserts.assertInstanceof:
            //   /**
            //    * @param {?} value The value to check
            //    * @param {function(new:T)) type A user-defined ctor
            //    * @return {T}
            //    * @template T
            //    */
            let parameters = reg.create_parameters(&[unknown, template_type_ctor]);
            function_type::builder()
                .with_parameters(parameters)
                .with_template_keys(reg, ast, vec![template_type])
                .with_return_type(template_type)
                .with_name(full_name)
                .build(reg, ast)
        };
        self.assuming(full_name, fn_type);
    }

    // ---- Rust-only wrappers for the `registry.*` and `JSType#*` calls of the test bodies ----

    fn to_string(&self, type_: TypeId) -> String {
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        type_.to_string(reg, ast)
    }

    /// `registry.createUnionType(JSType...)`.
    fn registry_create_union_type(&self, variants: &[TypeId]) -> TypeId {
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        reg.create_union_type(ast, variants)
    }

    /// `registry.createFunctionType(JSType returnType, JSType... parameterTypes)`.
    fn registry_create_function_type(&self, return_type: TypeId, params: &[TypeId]) -> TypeId {
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        reg.create_function_type(ast, return_type, params)
    }

    /// `registry.createAnonymousObjectType(null)`.
    fn registry_create_anonymous_object_type(&self) -> TypeId {
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        reg.create_anonymous_object_type(ast, None)
    }

    /// `registry.createObjectType(name, implicitPrototype)`.
    fn registry_create_object_type(&self, name: &str, implicit_prototype: TypeId) -> TypeId {
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        reg.create_object_type(ast, name, Some(implicit_prototype))
    }

    /// `registry.createRecordType(ImmutableMap.of(..))`.
    fn registry_create_record_type(&self, props: &[(&str, TypeId)]) -> TypeId {
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        reg.create_record_type(ast, props.iter().map(|(k, v)| (JsString::from(*k), *v)))
    }

    /// Binds a TypeSubject (from ScopeSubject#withTypeThat) to the fixture's registry.
    fn bind(&self, s: TypeSubject) -> TS<'_> {
        TS { t: self, s }
    }

    /// `registry.createTemplateType(name)`.
    fn registry_create_template_type(&self, name: &str) -> TypeId {
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        reg.create_template_type(ast, name)
    }

    /// `new KnownSymbolType(registry, name)`.
    fn new_known_symbol_type(&self, name: &str) -> TypeId {
        let mut compiler = self.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        KnownSymbolType::new(reg, ast, name)
    }

    /// `assertNode(node).hasJSTypeThat()`.
    fn assert_node_js_type(&self, node: NodeId) -> TS<'_> {
        let js_type = node.get_jstype(&self.c());
        self.assert_type(js_type)
    }
}

/// The `JSType` methods the test bodies call, with the fixture's registry.
#[allow(clippy::wrong_self_convention)]
trait TypeExt {
    fn to_string_t(self, t: &TypeInferenceTest) -> String;
    fn get_elements_type_t(self, t: &TypeInferenceTest) -> TypeId;
    fn is_record_type_t(self, t: &TypeInferenceTest) -> bool;
    fn is_number_t(self, t: &TypeInferenceTest) -> bool;
    fn define_declared_property_t(
        self,
        t: &TypeInferenceTest,
        name: impl Into<PropertyKey>,
        type_: TypeId,
        node: Option<NodeId>,
    ) -> bool;
}

impl TypeExt for TypeId {
    fn to_string_t(self, t: &TypeInferenceTest) -> String {
        t.to_string(self)
    }
    fn get_elements_type_t(self, t: &TypeInferenceTest) -> TypeId {
        self.get_elements_type(t.c().get_type_registry())
    }
    fn is_record_type_t(self, t: &TypeInferenceTest) -> bool {
        self.is_record_type(t.c().get_type_registry())
    }
    fn is_number_t(self, t: &TypeInferenceTest) -> bool {
        let mut compiler = t.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        self.is_number(reg, ast)
    }
    fn define_declared_property_t(
        self,
        t: &TypeInferenceTest,
        name: impl Into<PropertyKey>,
        type_: TypeId,
        node: Option<NodeId>,
    ) -> bool {
        let mut compiler = t.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        self.define_declared_property(reg, ast, name, type_, node)
    }
}

/// `registry.createUnionType(a, b, ..)` with JSType or JSTypeNative arguments.
macro_rules! union {
    ($t:expr, $($x:expr),+ $(,)?) => {
        $t.registry_create_union_type(&[$(AsType::ty($x, &$t)),+])
    };
}

/// `registry.createFunctionType(returnType, params..)`.
macro_rules! fn_type {
    ($t:expr, $ret:expr $(, $x:expr)* $(,)?) => {
        $t.registry_create_function_type(AsType::ty($ret, &$t), &[$(AsType::ty($x, &$t)),*])
    };
}

// ---- Tests ----

// port: TypeInferenceTest#testAssumption
#[test]
fn test_assumption() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("");
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testVar
#[test]
fn test_var() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var x = 1;");
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testMissingTypeAnnotationsInfersUnknownReturnInArrow
#[test]
fn test_missing_type_annotations_infers_unknown_return_in_arrow() {
    let t = TypeInferenceTest::set_up();
    t.in_function("const getNum = () => 2; const a = getNum();");
    t.verify_unequal("a", NUMBER_TYPE);
    t.verify("a", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testMissingTypeAnnotationsInfersUnknownReturn
#[test]
fn test_missing_type_annotations_infers_unknown_return() {
    let t = TypeInferenceTest::set_up();
    t.in_function("const getNum = function() {return 2; }; const a = getNum();");
    t.verify_unequal("a", NUMBER_TYPE);
    t.verify("a", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testEmptyVar
#[test]
fn test_empty_var() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var x;");
    t.verify("x", VOID_TYPE);
}

// port: TypeInferenceTest#testAssignment
#[test]
fn test_assignment() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", OBJECT_TYPE);
    t.in_function("x = 1;");
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testExprWithinCast
#[test]
fn test_expr_within_cast() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", OBJECT_TYPE);
    t.in_function("/** @type {string} */ (x = 1);");
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testGetProp
#[test]
fn test_get_prop() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("x.y();");
    t.verify("x", OBJECT_TYPE);
}

// port: TypeInferenceTest#testOptChainGetProp_nullObject
#[test]
fn test_opt_chain_get_prop_null_object() {
    let t = TypeInferenceTest::set_up();
    t.in_function("let x = null; let a = x?.y;");
    t.verify("a", VOID_TYPE);
}

// port: TypeInferenceTest#testOptChainGetElem_accessedByName
#[test]
fn test_opt_chain_get_elem_accessed_by_name() {
    let t = TypeInferenceTest::set_up();
    t.in_function("let x = { y : 5}; let a = x?.[y];");
    t.verify("a", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testOptChainGetElem_accessedByString
#[test]
fn test_opt_chain_get_elem_accessed_by_string() {
    let t = TypeInferenceTest::set_up();
    t.in_function("let x = { y : 5}; let a = x?.['y'];");
    t.verify("a", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testNormalGetElem_accessedByString
#[test]
fn test_normal_get_elem_accessed_by_string() {
    let t = TypeInferenceTest::set_up();
    t.in_function("let x = { y : 5}; let a = x['y'];");
    t.verify("a", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testSimpleGetProp_missingPropAccessedOnRecordType
#[test]
fn test_simple_get_prop_missing_prop_accessed_on_record_type() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_record_type("y", STRING_TYPE));
    t.in_function("a = x.z;");
    t.verify("a", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testSimpleGetProp_missingPropAccessedOnStringType
#[test]
fn test_simple_get_prop_missing_prop_accessed_on_string_type() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", STRING_TYPE);
    t.in_function("a = x.z;");
    t.verify("a", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testOptChainGetProp_nullObj
#[test]
fn test_opt_chain_get_prop_null_obj() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NULL_TYPE);
    t.in_function("let a = x?.y;");
    t.verify("a", VOID_TYPE);
}

// port: TypeInferenceTest#testOptChain_accessingInexistentPropOnUnknownType
#[test]
fn test_opt_chain_accessing_inexistent_prop_on_unknown_type() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", UNKNOWN_TYPE);
    t.in_function("a = x?.z");
    t.verify("a", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testOptChainGetProp_stringProp
#[test]
fn test_opt_chain_get_prop_string_prop() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_record_type("y", STRING_TYPE));
    t.in_function("let a = x?.y;");
    t.verify("a", STRING_TYPE);
}

// port: TypeInferenceTest#testOptChainGetProp_nullableStringProp
#[test]
fn test_opt_chain_get_prop_nullable_string_prop() {
    let t = TypeInferenceTest::set_up();
    t.assuming(
        "x",
        t.create_record_type("y", t.create_union_type(NULL_TYPE, STRING_TYPE)),
    );
    t.in_function("let a = x?.y;");
    t.verify("a", t.create_union_type(NULL_TYPE, STRING_TYPE));
}

// port: TypeInferenceTest#testOptChainGetProp_functionProp
#[test]
fn test_opt_chain_get_prop_function_prop() {
    let t = TypeInferenceTest::set_up();
    let func_type = fn_type!(t, t.get_native_type(NUMBER_TYPE));
    let lhs_type = t.create_record_type("y", func_type);
    t.assuming("x", lhs_type);
    t.in_function("let a = x?.y;");
    t.verify("x", lhs_type);
    t.verify("a", func_type); // not the function `y`'s return type
}

// port: TypeInferenceTest#testOptChainGetProp_functionProp_nullableObj
#[test]
fn test_opt_chain_get_prop_function_prop_nullable_obj() {
    let t = TypeInferenceTest::set_up();
    t.assuming(
        "x",
        t.create_nullable_type(t.create_record_type("y", FUNCTION_TYPE)),
    );
    t.in_function("let a = x?.y;");
    t.verify("a", t.create_union_type(VOID_TYPE, FUNCTION_TYPE));
}

// port: TypeInferenceTest#testOptChainGetProp_stringProp_nullableObj
#[test]
fn test_opt_chain_get_prop_string_prop_nullable_obj() {
    let t = TypeInferenceTest::set_up();
    t.assuming(
        "x",
        t.create_nullable_type(t.create_record_type("y", STRING_TYPE)),
    );
    t.in_function("let a = x?.y;");
    t.verify("a", t.create_union_type(STRING_TYPE, VOID_TYPE));
}

// port: TypeInferenceTest#testOptChainGetProp_numberProp
#[test]
fn test_opt_chain_get_prop_number_prop() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_record_type("y", NUMBER_TYPE));
    t.in_function("let a = x?.y;");
    t.verify("a", NUMBER_TYPE);
}

// port: TypeInferenceTest#testOptChainGetProp_numberProp_nullableObj
#[test]
fn test_opt_chain_get_prop_number_prop_nullable_obj() {
    let t = TypeInferenceTest::set_up();
    t.assuming(
        "x",
        t.create_nullable_type(t.create_record_type("y", NUMBER_TYPE)),
    );
    t.in_function("let a = x?.y;");
    t.verify("a", t.create_union_type(VOID_TYPE, NUMBER_TYPE));
}

// port: TypeInferenceTest#testOptChainGetProp_nullProp
#[test]
fn test_opt_chain_get_prop_null_prop() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_record_type("y", NULL_TYPE));
    t.in_function("let a = x?.y;");
    t.verify("a", NULL_TYPE);
}

// port: TypeInferenceTest#testOptChainGetProp_inexistentProp
#[test]
fn test_opt_chain_get_prop_inexistent_prop() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_record_type("y", STRING_TYPE));
    t.in_function("let a = x?.z;");
    t.verify("a", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testSimpleOptChain_withUnsetType_trailingGetProp
#[test]
fn test_simple_opt_chain_with_unset_type_trailing_get_prop() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", UNKNOWN_TYPE);
    t.in_function("let a = (x?.y?.z).q;");
    // NOTE: The parentheses breaks the optional chain,  so the `.q`  is not optional.
    // We should issue a warning about this  in TypeCheck, but that isn't TypeInference's job,
    // so we'll just assume `.q` is UNKNOWN_TYPE.
    t.verify("a", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testGetProp_withOptionalChainObject_voidable
#[test]
fn test_get_prop_with_optional_chain_object_voidable() {
    let t = TypeInferenceTest::set_up();
    let record_type = t.create_record_type("y", t.create_record_type("z", STRING_TYPE));
    let lhs_type = union!(t, t.get_native_type(VOID_TYPE), record_type);
    t.assuming("x", lhs_type);
    t.in_function("let a = (x?.y).z");
    t.verify("x", record_type); // Dereferencing non-optionally (`.z`) tightens `x` here
    t.verify("a", STRING_TYPE); // deliberate in TypeInf, must report in TypeChecking.
}

// port: TypeInferenceTest#testOptChainGetProp_unconditionalAssignmentToObjInInnerNodes
#[test]
fn test_opt_chain_get_prop_unconditional_assignment_to_obj_in_inner_nodes() {
    let t = TypeInferenceTest::set_up();
    t.assuming(
        "x",
        t.create_record_type("y", fn_type!(t, t.get_native_type(NUMBER_TYPE))),
    );
    t.in_function("x?.y(x=5);");
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testOptChainGetProp_conditionalAssignmentToObjInInnerNodes
#[test]
fn test_opt_chain_get_prop_conditional_assignment_to_obj_in_inner_nodes() {
    let t = TypeInferenceTest::set_up();
    let nullable_record_type = t.create_nullable_type(
        t.create_record_type("y", fn_type!(t, t.get_native_type(NUMBER_TYPE))),
    );
    t.assuming("x", nullable_record_type);
    t.in_function("x?.y(x=5);");
    t.verify(
        "x",
        union!(
            t,
            t.get_native_type(NUMBER_TYPE),
            t.get_native_type(NULL_TYPE)
        ),
    );
}

// port: TypeInferenceTest#testOptChainGetProp_typeAnnotationOnObjInInnerNodes_unconditionalChain
#[test]
fn test_opt_chain_get_prop_type_annotation_on_obj_in_inner_nodes_unconditional_chain() {
    let t = TypeInferenceTest::set_up();
    let record_type = t.create_record_type("y", fn_type!(t, t.get_native_type(NUMBER_TYPE)));
    t.assuming("x", record_type);
    t.in_function("x?.y(/** @type {number} */ (x)); ");
    t.verify("x", record_type);
}

// port: TypeInferenceTest#testGetProp_typeAnnotationOnObjInInnerNodes_unconditionalChain
#[test]
fn test_get_prop_type_annotation_on_obj_in_inner_nodes_unconditional_chain() {
    let t = TypeInferenceTest::set_up();
    let record_type = t.create_record_type("y", fn_type!(t, t.get_native_type(NUMBER_TYPE)));
    t.assuming("x", record_type);
    t.in_function("x.y(/** @type {number} */ (x)); "); // regular GET_PROP
    t.verify("x", record_type);
}

// port: TypeInferenceTest#testOptChainGetProp_typeAnnotationOnObjInInnerNodes_conditionalChain
#[test]
fn test_opt_chain_get_prop_type_annotation_on_obj_in_inner_nodes_conditional_chain() {
    let t = TypeInferenceTest::set_up();
    let nullable_record_type = t.create_nullable_type(
        t.create_record_type("y", fn_type!(t, t.get_native_type(NUMBER_TYPE))),
    );
    t.assuming("x", nullable_record_type);
    t.in_function("x?.y(/** @type {number} */ (x)); ");
    t.verify("x", nullable_record_type);
}

// port: TypeInferenceTest#testOptChainGetProp_conditionalChangeToOuterVariable
#[test]
fn test_opt_chain_get_prop_conditional_change_to_outer_variable() {
    let t = TypeInferenceTest::set_up();
    let lhs_nullable_record_type = t.create_nullable_type(
        t.create_record_type("b", fn_type!(t, t.get_native_type(NUMBER_TYPE))),
    );
    t.assuming("a", lhs_nullable_record_type);
    t.in_function("let x = 'x';\na?.b(x=5);\na;\nx;\n");
    t.verify("a", lhs_nullable_record_type);
    t.verify("x", t.create_union_type(NUMBER_TYPE, STRING_TYPE));
}

// port: TypeInferenceTest#testOptChainGetProp_unconditionalChangeToOuterVariable_inNullableReceiver
#[test]
fn test_opt_chain_get_prop_unconditional_change_to_outer_variable_in_nullable_receiver() {
    let t = TypeInferenceTest::set_up();
    let record_type = t.create_record_type("b", t.get_native_type(NUMBER_TYPE));
    let nullable_record_type = t.create_nullable_type(record_type);
    let lhs_type = fn_type!(t, nullable_record_type);

    t.assuming("a", lhs_type);
    t.in_function("let x = 'x';\nlet res = a(x = 'some')?.b\na;\nx;\n");
    t.verify("a", lhs_type);
    t.verify("x", STRING_TYPE);
    t.verify("res", t.create_union_type(VOID_TYPE, NUMBER_TYPE));
}

// port: TypeInferenceTest#testRegularGetProp_unconditionalChangeToOuterVariable_inNullableReceiver
#[test]
fn test_regular_get_prop_unconditional_change_to_outer_variable_in_nullable_receiver() {
    let t = TypeInferenceTest::set_up();
    let func_type = fn_type!(t, t.get_native_type(NUMBER_TYPE));
    let record_type = t.create_record_type("b", func_type);
    let nullable_record_type = t.create_nullable_type(record_type);
    let lhs_type = fn_type!(t, nullable_record_type);

    t.assuming("a", lhs_type);
    t.in_function("let x = 'x';\nlet res = a(x = 1).b\na;\nx;\n");
    t.verify("a", lhs_type);
    t.verify("x", NUMBER_TYPE);
    t.verify("res", func_type);
}

// port: TypeInferenceTest#testOptChainGetProp_unconditionalChangeToOuterVariable_inNonNullableReceiver
#[test]
fn test_opt_chain_get_prop_unconditional_change_to_outer_variable_in_non_nullable_receiver() {
    let t = TypeInferenceTest::set_up();
    let record_type = t.create_record_type("b", t.get_native_type(NUMBER_TYPE));
    let lhs_type = fn_type!(t, record_type);

    t.assuming("a", lhs_type);
    t.in_function("let x = 'x';\nlet res = a(x = 'some')?.b\na;\nx;\n");
    t.verify("a", lhs_type);
    t.verify("x", STRING_TYPE);
    t.verify("res", NUMBER_TYPE);
}

// port: TypeInferenceTest#testRegularGetProp_changeToOuterVariable_inNonNullableReceiver_andOptNodes
#[test]
fn test_regular_get_prop_change_to_outer_variable_in_non_nullable_receiver_and_opt_nodes() {
    let t = TypeInferenceTest::set_up();
    let func_type = fn_type!(t, t.get_native_type(NUMBER_TYPE));
    let record_type = t.create_record_type("b", func_type);
    let lhs_type = fn_type!(t, record_type);
    t.assuming("a", lhs_type);
    t.in_function("let x = 'x';\nlet res = a(x = 1).b(x = 'x')\na;\nx;\n");
    t.verify("a", lhs_type);
    t.verify("x", STRING_TYPE);
    t.verify("res", NUMBER_TYPE);
}

// port: TypeInferenceTest#testOptChainGetProp_changeToOuterVariable_inNonNullableReceiver_andOptNodes
#[test]
fn test_opt_chain_get_prop_change_to_outer_variable_in_non_nullable_receiver_and_opt_nodes() {
    let t = TypeInferenceTest::set_up();
    let func_type = fn_type!(t, t.get_native_type(NUMBER_TYPE));
    let record_type = t.create_record_type("b", func_type);
    let lhs_type = fn_type!(t, record_type);
    t.assuming("a", lhs_type);
    t.in_function("let x = 'x';\nlet res = a(x = 1)?.b(x = 'x')\na;\nx; res;\n");
    t.verify("a", lhs_type);
    t.verify("x", STRING_TYPE);
    t.verify("res", NUMBER_TYPE);
}

// port: TypeInferenceTest#testRegularGetProp_changeToOuterVariable_inNullableReceiver_andOptNodes
#[test]
fn test_regular_get_prop_change_to_outer_variable_in_nullable_receiver_and_opt_nodes() {
    let t = TypeInferenceTest::set_up();
    let func_type = fn_type!(t, t.get_native_type(NUMBER_TYPE));
    let record_type = t.create_record_type("b", func_type);
    let nullable_record_type = t.create_nullable_type(record_type);
    let lhs_type = fn_type!(t, nullable_record_type);
    t.assuming("a", lhs_type);
    t.in_function("let x = 'x';\nlet res = a(x = 1).b(x = 'x')\na;\nx;\n");
    t.verify("a", lhs_type);
    t.verify("x", union!(t, STRING_TYPE));
    t.verify("res", NUMBER_TYPE);
}

// port: TypeInferenceTest#testOptChainGetProp_changeToOuterVariable_inNullableReceiver_andOptNodes
#[test]
fn test_opt_chain_get_prop_change_to_outer_variable_in_nullable_receiver_and_opt_nodes() {
    let t = TypeInferenceTest::set_up();
    let func_type = fn_type!(t, t.get_native_type(NUMBER_TYPE));
    let record_type = t.create_record_type("b", func_type);
    let nullable_record_type = t.create_nullable_type(record_type);
    let lhs_type = fn_type!(t, nullable_record_type);
    t.assuming("a", lhs_type);
    t.in_function("let x = 'x';\nlet res = a(x = 1)?.b(x = 'x')\na;\nx;\n");
    t.verify("a", lhs_type);
    t.verify("x", union!(t, NUMBER_TYPE, STRING_TYPE));
    t.verify("res", t.create_union_type(VOID_TYPE, NUMBER_TYPE));
}

// port: TypeInferenceTest#testOptChainGetProp_unconditionalChangeToOuterVariable
#[test]
fn test_opt_chain_get_prop_unconditional_change_to_outer_variable() {
    let t = TypeInferenceTest::set_up();
    let lhs_type = t.create_record_type("b", fn_type!(t, t.get_native_type(NUMBER_TYPE)));
    t.assuming("a", lhs_type);
    t.in_function("let x = 'x';\na?.b(x=5);\na;\nx;\n");
    t.verify("a", lhs_type);
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testRegularGetProp_unconditionalChangeToOuterVariable
#[test]
fn test_regular_get_prop_unconditional_change_to_outer_variable() {
    let t = TypeInferenceTest::set_up();
    let lhs_type = t.create_record_type("b", fn_type!(t, t.get_native_type(NUMBER_TYPE)));
    t.assuming("a", lhs_type);
    t.in_function("let x = 'x';\na.b(x=5);\na;\nx;\n");
    t.verify("a", lhs_type);
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testOptChainGetProp_invalidChangeToOuterVariable
#[test]
fn test_opt_chain_get_prop_invalid_change_to_outer_variable() {
    let t = TypeInferenceTest::set_up();
    let lhs_type = t.create_record_type("b", fn_type!(t, t.get_native_type(NUMBER_TYPE)));
    t.assuming("a", lhs_type);
    t.in_function(
        "let x = 'x';\na?.c(x=5); // even though prop `c` is inexistent, x=5 will run, and typeOf(x) will\n// change.\na;\nx;\n");
    t.verify("a", lhs_type);
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testOptChainGetProp_multipleChains
#[test]
fn test_opt_chain_get_prop_multiple_chains() {
    let t = TypeInferenceTest::set_up();
    let lhs_type = t.create_record_type("b", fn_type!(t, t.get_native_type(NUMBER_TYPE)));
    t.assuming("a", lhs_type);
    t.in_function("let res = a?.b(x?.y.z);");
    t.verify("res", NUMBER_TYPE);
}

// port: TypeInferenceTest#testOptChainGetProp_multipleChains_nullableReceiver
#[test]
fn test_opt_chain_get_prop_multiple_chains_nullable_receiver() {
    let t = TypeInferenceTest::set_up();
    let lhs_type = t.create_nullable_type(
        t.create_record_type("b", fn_type!(t, t.get_native_type(NUMBER_TYPE))),
    );
    t.assuming("a", lhs_type);
    t.in_function("let res = a?.b(x?.y.z);");
    t.verify("res", union!(t, VOID_TYPE, NUMBER_TYPE));
}

// port: TypeInferenceTest#testGetElemDereference
#[test]
fn test_get_elem_dereference() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_undefinable_type(OBJECT_TYPE));
    t.in_function("x['z'] = 3;");
    t.verify("x", OBJECT_TYPE);
}

// port: TypeInferenceTest#testIf1
#[test]
fn test_if1() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("var y = {}; if (x) { y = x; }");
    t.verify_subtype_of("y", OBJECT_TYPE);
}

// port: TypeInferenceTest#testIf1a
#[test]
fn test_if1a() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("var y = {}; if (x != null) { y = x; }");
    t.verify_subtype_of("y", OBJECT_TYPE);
}

// port: TypeInferenceTest#testNullishCoalesceNullableObject
#[test]
fn test_nullish_coalesce_nullable_object() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("let z = x ?? {}");
    t.verify("z", OBJECT_TYPE);
}

// port: TypeInferenceTest#testNullishCoalesceNullableUnion
#[test]
fn test_nullish_coalesce_nullable_union() {
    let t = TypeInferenceTest::set_up();
    t.assuming(
        "x",
        t.create_nullable_type(t.create_union_type(OBJECT_TYPE, STRING_TYPE)),
    );
    t.in_function("let z = x ?? {}");
    t.verify("z", t.create_union_type(STRING_TYPE, OBJECT_TYPE));
}

// port: TypeInferenceTest#testIf2
#[test]
fn test_if2() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("var y = x; if (x) { y = x; } else { y = {}; }");
    t.verify_subtype_of("y", OBJECT_TYPE);
}

// port: TypeInferenceTest#testIf3
#[test]
fn test_if3() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("var y = 1; if (x) { y = x; }");
    t.verify("y", t.create_union_type(OBJECT_TYPE, NUMBER_TYPE));
}

// port: TypeInferenceTest#testAssert1
#[test]
fn test_assert1() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.assuming("x", start_type);
    t.in_function("out1 = x; goog.asserts.assert(x); out2 = x;");
    t.verify("out1", start_type);
    t.verify("out2", OBJECT_TYPE);
}

// port: TypeInferenceTest#testAssert1a
#[test]
fn test_assert1a() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.assuming("x", start_type);
    t.in_function("out1 = x; goog.asserts.assert(x !== null); out2 = x;");
    t.verify("out1", start_type);
    t.verify("out2", OBJECT_TYPE);
}

// port: TypeInferenceTest#testAssert2
#[test]
fn test_assert2() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.assuming("x", start_type);
    t.in_function("goog.asserts.assert(1, x); out1 = x;");
    t.verify("out1", start_type);
}

// port: TypeInferenceTest#testAssert3
#[test]
fn test_assert3() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.assuming("x", start_type);
    t.assuming("y", start_type);
    t.in_function("out1 = x; goog.asserts.assert(x && y); out2 = x; out3 = y;");
    t.verify("out1", start_type);
    t.verify("out2", OBJECT_TYPE);
    t.verify("out3", OBJECT_TYPE);
}

// port: TypeInferenceTest#testAssert4
#[test]
fn test_assert4() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.assuming("x", start_type);
    t.assuming("y", start_type);
    t.in_function("out1 = x; goog.asserts.assert(x && !y); out2 = x; out3 = y;");
    t.verify("out1", start_type);
    t.verify("out2", OBJECT_TYPE);
    t.verify("out3", NULL_TYPE);
}

// port: TypeInferenceTest#testAssert5
#[test]
fn test_assert5() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.assuming("x", start_type);
    t.assuming("y", start_type);
    t.in_function("goog.asserts.assert(x || y); out1 = x; out2 = y;");
    t.verify("out1", start_type);
    t.verify("out2", start_type);
}

// port: TypeInferenceTest#testAssert5NullishCoalesce
#[test]
fn test_assert5_nullish_coalesce() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.assuming("x", start_type);
    t.assuming("y", start_type);
    t.in_function("goog.asserts.assert(x ?? y); out1 = x; out2 = y;");
    t.verify("out1", start_type);
    t.verify("out2", start_type);
}

// port: TypeInferenceTest#testAssert6
#[test]
fn test_assert6() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.assuming("x", t.get_native_type(UNKNOWN_TYPE)); // Only global qname roots can be undeclared
    t.assuming("x.y", start_type);
    t.in_function("out1 = x.y; goog.asserts.assert(x.y); out2 = x.y;");
    t.verify("out1", start_type);
    t.verify("out2", OBJECT_TYPE);
}

// port: TypeInferenceTest#testAssert7
#[test]
fn test_assert7() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.assuming("x", start_type);
    t.in_function("out1 = x; out2 = goog.asserts.assert(x);");
    t.verify("out1", start_type);
    t.verify("out2", OBJECT_TYPE);
}

// port: TypeInferenceTest#testAssert8
#[test]
fn test_assert8() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.assuming("x", start_type);
    t.in_function("out1 = x; out2 = goog.asserts.assert(x != null);");
    t.verify("out1", start_type);
    t.verify("out2", BOOLEAN_TYPE);
}

// port: TypeInferenceTest#testAssert9
#[test]
fn test_assert9() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(NUMBER_TYPE);
    t.assuming("x", start_type);
    t.in_function("out1 = x; out2 = goog.asserts.assert(y = x);");
    t.verify("out1", start_type);
    t.verify("out2", NUMBER_TYPE);
}

// port: TypeInferenceTest#testAssert11
#[test]
fn test_assert11() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.assuming("x", start_type);
    t.assuming("y", start_type);
    t.in_function("var z = goog.asserts.assert(x || y);");
    t.verify("x", start_type);
    t.verify("y", start_type);
}

// port: TypeInferenceTest#testPrimitiveAssertTruthy_narrowsNullableObjectToObject
#[test]
fn test_primitive_assert_truthy_narrows_nullable_object_to_object() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.include_primitive_truthy_assertion_function("assertTruthy");
    t.assuming("x", start_type);

    t.in_function("out1 = x; assertTruthy(x); out2 = x;");

    t.verify("out1", start_type);
    t.verify("out2", OBJECT_TYPE);
}

// port: TypeInferenceTest#testPrimitiveAssertTruthy_narrowsNullableObjectInNeqNullToObject
#[test]
fn test_primitive_assert_truthy_narrows_nullable_object_in_neq_null_to_object() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.include_primitive_truthy_assertion_function("assertTruthy");
    t.assuming("x", start_type);

    t.in_function("out1 = x; assertTruthy(x !== null); out2 = x;");

    t.verify("out1", start_type);
    t.verify("out2", OBJECT_TYPE);
}

// port: TypeInferenceTest#testPrimitiveAssertTruthy_ignoresSecondArgumentEvenIfNullable
#[test]
fn test_primitive_assert_truthy_ignores_second_argument_even_if_nullable() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.include_primitive_truthy_assertion_function("assertTruthy");
    t.assuming("x", start_type);

    t.in_function("assertTruthy(1, x); out1 = x;");

    t.verify("out1", start_type);
}

// port: TypeInferenceTest#testAssertNumber_narrowsAllTypeToNumber
#[test]
fn test_assert_number_narrows_all_type_to_number() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(ALL_TYPE);
    t.include_goog_assertion_fn("assertNumber", t.get_native_type(NUMBER_TYPE));
    t.assuming("x", start_type);

    t.in_function("out1 = x; goog.asserts.assertNumber(x); out2 = x;");

    t.verify("out1", start_type);
    t.verify("out2", NUMBER_TYPE);
}

// port: TypeInferenceTest#testAssertNumber_doesNotNarrowNamesInExpression
#[test]
fn test_assert_number_does_not_narrow_names_in_expression() {
    let t = TypeInferenceTest::set_up();
    // Make sure it ignores expressions.
    let start_type = t.create_nullable_type(ALL_TYPE);
    t.include_goog_assertion_fn("assertNumber", t.get_native_type(NUMBER_TYPE));
    t.assuming("x", start_type);

    t.in_function("goog.asserts.assertNumber(x + x); out1 = x;");

    t.verify("out1", start_type);
}

// port: TypeInferenceTest#testAssertNumber_returnsNumberGivenExpression
#[test]
fn test_assert_number_returns_number_given_expression() {
    let t = TypeInferenceTest::set_up();
    // Make sure it ignores expressions.
    let start_type = t.create_nullable_type(ALL_TYPE);
    t.include_goog_assertion_fn("assertNumber", t.get_native_type(NUMBER_TYPE));
    t.assuming("x", start_type);

    t.in_function("out1 = x; out2 = goog.asserts.assertNumber(x + x);");

    t.verify("out1", start_type);
    t.verify("out2", NUMBER_TYPE);
}

// port: TypeInferenceTest#testPrimitiveAssertNumber_narrowsAllTypeToNumber
#[test]
fn test_primitive_assert_number_narrows_all_type_to_number() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(ALL_TYPE);
    t.include_primitive_assertion_fn("assertNumber", t.get_native_type(NUMBER_TYPE));
    t.assuming("x", start_type);

    t.in_function("out1 = x; assertNumber(x); out2 = x;");

    t.verify("out1", start_type);
    t.verify("out2", NUMBER_TYPE);
}

// port: TypeInferenceTest#testPrimitiveAssertNumber_doesNotNarrowNamesInExpression
#[test]
fn test_primitive_assert_number_does_not_narrow_names_in_expression() {
    let t = TypeInferenceTest::set_up();
    // Make sure it ignores expressions.
    let start_type = t.create_nullable_type(ALL_TYPE);
    t.include_primitive_assertion_fn("assertNumber", t.get_native_type(NUMBER_TYPE));
    t.assuming("x", start_type);

    t.in_function("assertNumber(x + x); out1 = x;");

    t.verify("out1", start_type);
}

// port: TypeInferenceTest#testPrimitiveAssertNumber_returnsNumberGivenExpression
#[test]
fn test_primitive_assert_number_returns_number_given_expression() {
    let t = TypeInferenceTest::set_up();
    // Make sure it ignores expressions.
    let start_type = t.create_nullable_type(ALL_TYPE);
    t.include_primitive_assertion_fn("assertNumber", t.get_native_type(NUMBER_TYPE));
    t.assuming("x", start_type);

    t.in_function("out1 = x; out2 = assertNumber(x + x);");

    t.verify("out1", start_type);
    t.verify("out2", NUMBER_TYPE);
}

// port: TypeInferenceTest#testBigIntTypeAssignment
#[test]
fn test_big_int_type_assignment() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", UNKNOWN_TYPE);
    t.assuming("y", UNKNOWN_TYPE);

    t.in_function("x = 1n; y = BigInt(1);");

    t.verify("x", BIGINT_TYPE);
    t.verify("y", BIGINT_TYPE);
}

// port: TypeInferenceTest#testAssertBigInt_narrowsAllTypeToBigInt
#[test]
fn test_assert_big_int_narrows_all_type_to_big_int() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(ALL_TYPE);
    t.include_primitive_assertion_fn("assertBigInt", t.get_native_type(BIGINT_TYPE));
    t.assuming("x", start_type);

    t.in_function("out1 = x; assertBigInt(x); out2 = x;");

    t.verify("out1", start_type);
    t.verify("out2", BIGINT_TYPE);
}

// port: TypeInferenceTest#testBigIntWithUnaryPlus
#[test]
fn test_big_int_with_unary_plus() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", BIGINT_TYPE);
    t.assuming("y", BIGINT_OBJECT_TYPE);
    t.assuming("z", BIGINT_NUMBER);

    t.in_function("valueType = +x; objectType = +y; unionType = +z;");

    // Unary plus throws an exception when applied to a BigInt, so there is no valid type for its
    // result.
    t.verify("valueType", NO_TYPE);
    t.verify("objectType", NO_TYPE);
    t.verify("unionType", NO_TYPE);
}

// port: TypeInferenceTest#testBigIntWithLogicalNOT
#[test]
fn test_big_int_with_logical_not() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", BIGINT_TYPE);
    t.assuming("y", BIGINT_OBJECT_TYPE);
    t.assuming("z", BIGINT_NUMBER);

    t.in_function("valueType = !x; objectType = !y; unionType = !z;");

    t.verify("valueType", BOOLEAN_TYPE);
    t.verify("objectType", BOOLEAN_TYPE);
    t.verify("unionType", BOOLEAN_TYPE);
}

// port: TypeInferenceTest#testBigIntWithTypeOfOperation
#[test]
fn test_big_int_with_type_of_operation() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", BIGINT_TYPE);
    t.assuming("y", BIGINT_OBJECT_TYPE);
    t.assuming("z", t.create_union_type(BIGINT_TYPE, BIGINT_OBJECT_TYPE));

    t.in_function("valueType = typeof x; objectType = typeof y; unionType = typeof z;");

    t.verify("valueType", STRING_TYPE);
    t.verify("objectType", STRING_TYPE);
    t.verify("unionType", STRING_TYPE);
}

// port: TypeInferenceTest#testBigIntWithDeleteOperation
#[test]
fn test_big_int_with_delete_operation() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", BIGINT_TYPE);
    t.assuming("y", BIGINT_OBJECT_TYPE);
    t.assuming("z", t.create_union_type(BIGINT_TYPE, BIGINT_OBJECT_TYPE));

    t.in_function("valueType = delete x; objectType = delete y; unionType = delete z;");

    t.verify("valueType", BOOLEAN_TYPE);
    t.verify("objectType", BOOLEAN_TYPE);
    t.verify("unionType", BOOLEAN_TYPE);
}

// port: TypeInferenceTest#testBigIntWithVoidOperation
#[test]
fn test_big_int_with_void_operation() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", BIGINT_TYPE);
    t.assuming("y", BIGINT_OBJECT_TYPE);
    t.assuming("z", t.create_union_type(BIGINT_TYPE, BIGINT_OBJECT_TYPE));

    t.in_function("valueType = void x; objectType = void y; unionType = void z;");

    t.verify("valueType", VOID_TYPE);
    t.verify("objectType", VOID_TYPE);
    t.verify("unionType", VOID_TYPE);
}

// port: TypeInferenceTest#testBigIntWithUnaryMinus
#[test]
fn test_big_int_with_unary_minus() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", BIGINT_TYPE);
    t.assuming("y", BIGINT_OBJECT_TYPE);
    t.assuming("u", UNKNOWN_TYPE);
    t.assuming("a", ALL_TYPE);
    t.assuming("z1", BIGINT_NUMBER);
    // testing for a union between bigint and anything but number
    t.assuming("z2", t.create_union_type(BIGINT_TYPE, STRING_TYPE));

    t.in_function(
        "valueType = -x; objectType = -y; unknownType = -u; allType = -a; bigintNum = -z1;\n bigintOther = -z2;\n");

    t.verify("valueType", BIGINT_TYPE);
    t.verify("objectType", BIGINT_TYPE);
    t.verify("unknownType", NUMBER_TYPE);
    t.verify("allType", NUMBER_TYPE);
    t.verify("bigintNum", BIGINT_NUMBER);
    t.verify("bigintOther", BIGINT_NUMBER);
}

// port: TypeInferenceTest#testBigIntWithBitwiseNOT
#[test]
fn test_big_int_with_bitwise_not() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", BIGINT_TYPE);
    t.assuming("y", BIGINT_OBJECT_TYPE);
    t.assuming("u", UNKNOWN_TYPE);
    t.assuming("a", ALL_TYPE);
    t.assuming("z1", BIGINT_NUMBER);
    // testing for a union between bigint and anything but number
    t.assuming("z2", t.create_union_type(BIGINT_TYPE, STRING_TYPE));

    t.in_function(
        "valueType = ~x; objectType = ~y; unknownType = ~u; allType = ~a; bigintOrNumber = ~z1;\n bigintOrOther = ~z2;\n");

    t.verify("valueType", BIGINT_TYPE);
    t.verify("objectType", BIGINT_TYPE);
    t.verify("unknownType", NUMBER_TYPE);
    t.verify("allType", NUMBER_TYPE);
    t.verify("bigintOrNumber", BIGINT_NUMBER);
    t.verify("bigintOrOther", BIGINT_NUMBER);
}

// port: TypeInferenceTest#testIncrementOnBigInt
#[test]
fn test_increment_on_big_int() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", BIGINT_TYPE);
    t.assuming("y", BIGINT_OBJECT_TYPE);
    t.assuming("z", BIGINT_NUMBER);
    t.assuming("a", t.create_union_type(BIGINT_TYPE, STRING_TYPE));

    t.in_function("valueType = x++; objectType = y++; bigintNumber = z++; bigintOther = a++;");

    t.verify("valueType", BIGINT_TYPE);
    t.verify("objectType", BIGINT_TYPE);
    t.verify("bigintNumber", BIGINT_NUMBER);
    t.verify("bigintOther", BIGINT_NUMBER);
}

// port: TypeInferenceTest#testDecrementOnBigInt
#[test]
fn test_decrement_on_big_int() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", BIGINT_TYPE);
    t.assuming("y", BIGINT_OBJECT_TYPE);
    t.assuming("z", BIGINT_NUMBER);
    t.assuming("a", t.create_union_type(BIGINT_TYPE, STRING_TYPE));

    t.in_function("valueType = x--; objectType = y--; bigintNumber = z++; bigintOther = a++;");

    t.verify("valueType", BIGINT_TYPE);
    t.verify("objectType", BIGINT_TYPE);
    t.verify("bigintNumber", BIGINT_NUMBER);
    t.verify("bigintOther", BIGINT_NUMBER);
}

// port: TypeInferenceTest#testAdditionWithBigInt
#[test]
fn test_addition_with_big_int() {
    let t = TypeInferenceTest::set_up();
    t.assuming("b", BIGINT_TYPE);
    t.assuming("B", BIGINT_OBJECT_TYPE);
    t.assuming("n", NUMBER_TYPE);
    t.assuming("bn", BIGINT_NUMBER);
    t.assuming("bs", t.create_union_type(BIGINT_TYPE, STRING_TYPE));
    t.assuming("s", STRING_TYPE);
    t.assuming("u", UNKNOWN_TYPE);
    t.assuming("ns", NUMBER_STRING);

    t.in_function(
        "valueTypePlusSelf = b + b;\nobjectTypePlusSelf = B + B;\nvaluePlusObject = b + B;\nbigintPlusNumber = b + n;\nbigintNumberPlusSelf = bn + bn;\nbigintStringConcat = b + s;\nbigintNumberStringConcat = bn + s\nbigintOtherStringConcat = bs + s\nbigintStringConcatWithSelf = bs + bs\nbigintPlusUnknown = b + u;\nbigintPlusNumberString = b + ns;\n");

    t.verify("valueTypePlusSelf", BIGINT_TYPE);
    t.verify("objectTypePlusSelf", BIGINT_TYPE);
    t.verify("valuePlusObject", BIGINT_TYPE);
    t.verify("bigintPlusNumber", NO_TYPE);
    t.verify("bigintNumberPlusSelf", BIGINT_NUMBER);
    t.verify("bigintStringConcat", STRING_TYPE);
    t.verify("bigintNumberStringConcat", STRING_TYPE);
    t.verify("bigintOtherStringConcat", STRING_TYPE);
    // In reality if you use '+' on 2 bigint|string operands, then the result will be bigint|string.
    // However, code that does that is almost certainly wrong and we should complain about it.
    // It also keeps the TypeInference logic simpler if we pretend this operation is an error.
    t.verify("bigintStringConcatWithSelf", NO_TYPE);
    t.verify("bigintPlusUnknown", NO_TYPE);
    t.verify("bigintPlusNumberString", NO_TYPE);
}

// port: TypeInferenceTest#testBigIntCompatibleBinaryOperator
#[test]
fn test_big_int_compatible_binary_operator() {
    let t = TypeInferenceTest::set_up();
    t.assuming("b", BIGINT_TYPE);
    t.assuming("B", BIGINT_OBJECT_TYPE);
    t.assuming("n", NUMBER_TYPE);
    t.assuming("bn", BIGINT_NUMBER);
    t.assuming("s", STRING_TYPE);
    t.assuming("u", UNKNOWN_TYPE);
    t.assuming("ns", NUMBER_STRING);

    t.in_function(
        "valueTypeWithSelf = b * b;\nobjectTypeWithSelf = B * B;\nvalueWithObject = b * B;\nbigintWithNumber = b * n;\nbigintNumberWithSelf = bn * bn;\nbigintWithOther = b * s;\nbigintWithUnknown = b * u;\nbigintWithNumberString = b * ns;\n");

    t.verify("valueTypeWithSelf", BIGINT_TYPE);
    t.verify("objectTypeWithSelf", BIGINT_TYPE);
    t.verify("valueWithObject", BIGINT_TYPE);
    t.verify("bigintWithNumber", NO_TYPE);
    t.verify("bigintNumberWithSelf", BIGINT_NUMBER);
    t.verify("bigintWithOther", NO_TYPE);
    t.verify("bigintWithUnknown", NO_TYPE);
    t.verify("bigintWithNumberString", NO_TYPE);
}

// port: TypeInferenceTest#testAssignOpWithBigInt
#[test]
fn test_assign_op_with_big_int() {
    let t = TypeInferenceTest::set_up();
    t.assuming("b", BIGINT_TYPE);
    t.assuming("n", NUMBER_TYPE);
    t.assuming("s", STRING_TYPE);
    t.assuming("u", UNKNOWN_TYPE);
    t.assuming("bn", BIGINT_NUMBER);
    t.assuming("bigintWithSelf", BIGINT_TYPE);
    t.assuming("bigintWithNumber", BIGINT_TYPE);
    t.assuming("bigintWithOther", BIGINT_TYPE);
    t.assuming("bigintConcatString", BIGINT_TYPE);
    t.assuming("stringConcatBigInt", STRING_TYPE);
    t.assuming("bigintWithUnknown", BIGINT_TYPE);
    t.assuming("bigintNumberWithSelf", BIGINT_NUMBER);
    t.assuming("bigintNumberWithBigInt", BIGINT_NUMBER);
    t.assuming("bigintNumberWithNumber", BIGINT_NUMBER);

    t.in_function(
        "bigintWithSelf *= b;\nbigintWithNumber *= n;\nbigintWithOther *= s;\nbigintConcatString += s\nstringConcatBigInt += b\nbigintWithUnknown *= u;\nbigintNumberWithSelf *= bn;\nbigintNumberWithBigInt *= b\nbigintNumberWithNumber *= n\n");

    t.verify("bigintWithSelf", BIGINT_TYPE);
    t.verify("bigintWithNumber", NO_TYPE);
    t.verify("bigintWithOther", NO_TYPE);
    t.verify("bigintConcatString", STRING_TYPE);
    t.verify("stringConcatBigInt", STRING_TYPE);
    t.verify("bigintWithUnknown", NO_TYPE);
    t.verify("bigintNumberWithSelf", BIGINT_NUMBER);
    t.verify("bigintNumberWithBigInt", NO_TYPE);
    t.verify("bigintNumberWithNumber", NO_TYPE);
}

// port: TypeInferenceTest#testUnsignedRightShiftWithBigInt
#[test]
fn test_unsigned_right_shift_with_big_int() {
    let t = TypeInferenceTest::set_up();
    t.assuming("b", BIGINT_TYPE);
    t.assuming("n", NUMBER_TYPE);
    t.assuming("assignBigIntOnLeft", BIGINT_TYPE);
    t.assuming("assignBigIntOnRight", NUMBER_TYPE);
    t.assuming("assignBigIntOnBothSides", BIGINT_TYPE);

    t.in_function(
        "bigintOnLeft = b >>> n;\nbigintOnRight = n >>> b;\nbigintOnBothSides = b >>> b;\nassignBigIntOnLeft >>>= n\nassignBigIntOnRight >>>= b\nassignBigIntOnBothSides >>>= b\n");

    t.verify("bigintOnLeft", NO_TYPE);
    t.verify("bigintOnRight", NO_TYPE);
    t.verify("bigintOnBothSides", NO_TYPE);
    t.verify("assignBigIntOnLeft", NO_TYPE);
    t.verify("assignBigIntOnRight", NO_TYPE);
    t.verify("assignBigIntOnBothSides", NO_TYPE);
}

// port: TypeInferenceTest#testBigIntComparison
#[test]
fn test_big_int_comparison() {
    let t = TypeInferenceTest::set_up();
    t.assuming("b", BIGINT_TYPE);
    t.assuming("n", NUMBER_TYPE);

    t.in_function("bigintOnly = b > b; bigintAndOther = b > n");

    t.verify("bigintOnly", BOOLEAN_TYPE);
    t.verify("bigintAndOther", BOOLEAN_TYPE);
}

// port: TypeInferenceTest#testLogicalBinaryOperatorsWithBigInt
#[test]
fn test_logical_binary_operators_with_big_int() {
    let t = TypeInferenceTest::set_up();
    t.assuming("b", BIGINT_TYPE);
    t.assuming("B", BIGINT_OBJECT_TYPE);
    t.assuming("n", NUMBER_TYPE);
    t.assuming("bn", BIGINT_NUMBER);
    t.assuming("s", STRING_TYPE);
    t.assuming("u", UNKNOWN_TYPE);
    t.assuming("ns", NUMBER_STRING);

    t.in_function(
        "valueTypeWithSelf = b && b;\nobjectTypeWithSelf = B && B;\nvalueWithObject = b && B;\nbigintWithNumber = b && n;\nbigintNumberWithSelf = bn && bn;\nbigintWithOther = b && s;\nbigintWithUnknown = b && u;\nbigintWithNumberString = b && ns;\n");

    t.verify("valueTypeWithSelf", BIGINT_TYPE);
    t.verify("objectTypeWithSelf", BIGINT_OBJECT_TYPE);
    t.verify(
        "valueWithObject",
        t.create_union_type(BIGINT_TYPE, BIGINT_OBJECT_TYPE),
    );
    t.verify("bigintWithNumber", BIGINT_NUMBER);
    t.verify("bigintNumberWithSelf", BIGINT_NUMBER);
    t.verify(
        "bigintWithOther",
        t.create_union_type(BIGINT_TYPE, STRING_TYPE),
    );
    t.verify(
        "bigintWithUnknown",
        t.create_union_type(BIGINT_TYPE, UNKNOWN_TYPE),
    );
    t.verify("bigintWithNumberString", BIGINT_NUMBER_STRING);
}

// port: TypeInferenceTest#testTernaryOperatorWithBigInt
#[test]
fn test_ternary_operator_with_big_int() {
    let t = TypeInferenceTest::set_up();
    t.assuming("b", BIGINT_TYPE);
    t.assuming("B", BIGINT_OBJECT_TYPE);
    t.assuming("n", NUMBER_TYPE);
    t.assuming("bn", BIGINT_NUMBER);
    t.assuming("s", STRING_TYPE);
    t.assuming("u", UNKNOWN_TYPE);
    t.assuming("v", VOID_TYPE);
    t.assuming("ns", NUMBER_STRING);

    t.in_function(
        "valueTypeWithSelf = v ? b : b;\nobjectTypeWithSelf = v ? B : B;\nvalueWithObject = v ? b : B;\nbigintWithNumber = v ? b : n;\nbigintNumberWithSelf = v ? bn : bn;\nbigintWithOther = v ? b : s;\nbigintWithUnknown = v ? b : u;\nbigintWithNumberString = v ? b : ns;\n");

    t.verify("valueTypeWithSelf", BIGINT_TYPE);
    t.verify("objectTypeWithSelf", BIGINT_OBJECT_TYPE);
    t.verify(
        "valueWithObject",
        t.create_union_type(BIGINT_TYPE, BIGINT_OBJECT_TYPE),
    );
    t.verify("bigintWithNumber", BIGINT_NUMBER);
    t.verify("bigintNumberWithSelf", BIGINT_NUMBER);
    t.verify(
        "bigintWithOther",
        t.create_union_type(BIGINT_TYPE, STRING_TYPE),
    );
    t.verify(
        "bigintWithUnknown",
        t.create_union_type(BIGINT_TYPE, UNKNOWN_TYPE),
    );
    t.verify("bigintWithNumberString", BIGINT_NUMBER_STRING);
}

// port: TypeInferenceTest#testAssertBoolean_narrowsAllTypeToBoolean
#[test]
fn test_assert_boolean_narrows_all_type_to_boolean() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(ALL_TYPE);
    t.include_goog_assertion_fn("assertBoolean", t.get_native_type(BOOLEAN_TYPE));
    t.assuming("x", start_type);

    t.in_function("out1 = x; goog.asserts.assertBoolean(x); out2 = x;");

    t.verify("out1", start_type);
    t.verify("out2", BOOLEAN_TYPE);
}

// port: TypeInferenceTest#testAssertString_narrowsAllTypeToString
#[test]
fn test_assert_string_narrows_all_type_to_string() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(ALL_TYPE);
    t.include_goog_assertion_fn("assertString", t.get_native_type(STRING_TYPE));
    t.assuming("x", start_type);

    t.in_function("out1 = x; goog.asserts.assertString(x); out2 = x;");

    t.verify("out1", start_type);
    t.verify("out2", STRING_TYPE);
}

// port: TypeInferenceTest#testAssertFunction_narrowsAllTypeToFunction
#[test]
fn test_assert_function_narrows_all_type_to_function() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(ALL_TYPE);
    t.include_goog_assertion_fn("assertFunction", t.get_native_type(FUNCTION_TYPE));
    t.assuming("x", start_type);

    t.in_function("out1 = x; goog.asserts.assertFunction(x); out2 = x;");

    t.verify("out1", start_type);
    t.verify_subtype_of("out2", FUNCTION_TYPE);
}

// port: TypeInferenceTest#testAssertObject_narrowsNullableArrayToArray
#[test]
fn test_assert_object_narrows_nullable_array_to_array() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(ARRAY_TYPE);
    t.include_goog_assertion_fn("assertObject", t.get_native_type(OBJECT_TYPE));
    t.assuming("x", start_type);

    t.in_function("out1 = x; goog.asserts.assertObject(x); out2 = x;");

    t.verify("out1", start_type);
    t.verify("out2", ARRAY_TYPE);
}

// port: TypeInferenceTest#testAssertObject_narrowsNullableObjectToObject
#[test]
fn test_assert_object_narrows_nullable_object_to_object() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.include_goog_assertion_fn("assertObject", t.get_native_type(OBJECT_TYPE));
    t.assuming("x", start_type);

    t.in_function("out1 = x; goog.asserts.assertObject(x); out2 = x;");

    t.verify("out1", start_type);
    t.verify("out2", OBJECT_TYPE);
}

// port: TypeInferenceTest#testAssertObject_narrowsQualifiedNameArgument
#[test]
fn test_assert_object_narrows_qualified_name_argument() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(OBJECT_TYPE);
    t.include_goog_assertion_fn("assertObject", t.get_native_type(OBJECT_TYPE));
    t.assuming("x", t.get_native_type(UNKNOWN_TYPE)); // Only global qname roots can be undeclared
    t.assuming("x.y", start_type);

    // test a property "x.y" instead of a simple name
    t.in_function("out1 = x.y; goog.asserts.assertObject(x.y); out2 = x.y;");

    t.verify("out1", start_type);
    t.verify("out2", OBJECT_TYPE);
}

// port: TypeInferenceTest#testAssertObject_inCastToArray_returnsArray
#[test]
fn test_assert_object_in_cast_to_array_returns_array() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(ALL_TYPE);
    t.include_goog_assertion_fn("assertObject", t.get_native_type(OBJECT_TYPE));
    t.assuming("x", start_type);

    t.in_function("out1 = x;\nout2 = /** @type {!Array} */ (goog.asserts.assertObject(x));\n");

    t.verify("out1", start_type);
    t.verify("out2", ARRAY_TYPE);
}

// port: TypeInferenceTest#testAssertArray_narrowsNullableAllTypeToArray
#[test]
fn test_assert_array_narrows_nullable_all_type_to_array() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(ALL_TYPE);
    t.include_goog_assertion_fn("assertArray", t.get_native_type(ARRAY_TYPE));
    t.assuming("x", start_type);

    t.in_function("out1 = x; goog.asserts.assertArray(x); out2 = x;");

    t.verify("out1", start_type);
    t.verify_subtype_of("out2", ARRAY_TYPE);
}

// port: TypeInferenceTest#testAssertArray_narrowsObjectTypeToArray
#[test]
fn test_assert_array_narrows_object_type_to_array() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.get_native_type(OBJECT_TYPE);
    t.include_goog_assertion_fn("assertArray", t.get_native_type(ARRAY_TYPE));
    t.assuming("x", start_type);

    t.in_function("out1 = x; goog.asserts.assertArray(x); out2 = x;");

    t.verify("out1", start_type);
    t.verify_subtype_of("out2", ARRAY_TYPE);
}

// port: TypeInferenceTest#testAssertInstanceof_invalidCall_setsArgToUnknownType
#[test]
fn test_assert_instanceof_invalid_call_sets_arg_to_unknown_type() {
    let t = TypeInferenceTest::set_up();
    // Test invalid assert (2 params are required)
    let start_type = t.create_nullable_type(ALL_TYPE);
    t.include_assert_instanceof();
    t.assuming("x", start_type);

    t.in_function("out1 = x; goog.asserts.assertInstanceof(x); out2 = x;");

    t.verify("out1", start_type);
    t.verify("out2", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testAssertInstanceof_stringCtor_narrowsAllTypeToString
#[test]
fn test_assert_instanceof_string_ctor_narrows_all_type_to_string() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_nullable_type(ALL_TYPE);
    t.include_assert_instanceof();
    t.assuming("x", start_type);

    t.in_function("out1 = x; goog.asserts.assertInstanceof(x, String); out2 = x;");

    t.verify("out1", start_type);
    t.verify("out2", STRING_OBJECT_TYPE);
}

// port: TypeInferenceTest#testAssertInstanceof_unknownCtor_setsStringToUnknown
#[test]
fn test_assert_instanceof_unknown_ctor_sets_string_to_unknown() {
    let t = TypeInferenceTest::set_up();
    t.include_assert_instanceof();
    t.assuming("x", STRING_TYPE);
    t.assuming("Foo", UNKNOWN_TYPE);

    t.in_function("out1 = x; goog.asserts.assertInstanceof(x, Foo); out2 = x;");

    t.verify("out1", STRING_TYPE);
    t.verify("out2", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testAssertInstanceof_stringCtor_narrowsUnknownToString
#[test]
fn test_assert_instanceof_string_ctor_narrows_unknown_to_string() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.get_native_type(UNKNOWN_TYPE);
    t.include_assert_instanceof();
    t.assuming("x", start_type);

    t.in_function("out1 = x; goog.asserts.assertInstanceof(x, String); out2 = x;");

    t.verify("out1", start_type);
    t.verify("out2", STRING_OBJECT_TYPE);
}

// port: TypeInferenceTest#testAssertInstanceof_objectCtor_doesNotChangeStringType
#[test]
fn test_assert_instanceof_object_ctor_does_not_change_string_type() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.get_native_type(STRING_OBJECT_TYPE);
    t.include_assert_instanceof();
    t.assuming("x", start_type);

    t.in_function("out1 = x; goog.asserts.assertInstanceof(x, Object); out2 = x;");

    t.verify("out1", start_type);
    t.verify("out2", STRING_OBJECT_TYPE);
}

// port: TypeInferenceTest#testAssertInstanceof_stringCtor_narrowsObjOrVoidToString
#[test]
fn test_assert_instanceof_string_ctor_narrows_obj_or_void_to_string() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_union_type(OBJECT_TYPE, VOID_TYPE);
    t.include_assert_instanceof();
    t.assuming("x", start_type);

    t.in_function("out1 = x; goog.asserts.assertInstanceof(x, String); var r = x;");

    t.verify("out1", start_type);
    t.verify("x", STRING_OBJECT_TYPE);
}

// port: TypeInferenceTest#testAssertInstanceof_stringCtor_returnsStringFromObjOrVoid
#[test]
fn test_assert_instanceof_string_ctor_returns_string_from_obj_or_void() {
    let t = TypeInferenceTest::set_up();
    let start_type = t.create_union_type(OBJECT_TYPE, VOID_TYPE);
    t.include_assert_instanceof();
    t.assuming("x", start_type);

    t.in_function("out1 = x; var y = goog.asserts.assertInstanceof(x, String);");

    t.verify("out1", start_type);
    t.verify("y", STRING_OBJECT_TYPE);
}

// port: TypeInferenceTest#testTypeInferenceOccursInConstObjectProperties
#[test]
fn test_type_inference_occurs_in_const_object_properties() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @return {string} */\nfunction foo() { return ''; }\n\nconst obj = {\n   prop: foo(),\n}\nLABEL: obj.prop;\n");

    t.assert_type_of_expression("LABEL")
        .to_string_is_equal_to("string");
    t.assert_type_of_expression("LABEL")
        .is_not_equal_to(UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testReturn1
#[test]
fn test_return1() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("if (x) { return x; }\nx = {};\nreturn x;");
    t.verify("x", OBJECT_TYPE);
}

// port: TypeInferenceTest#testReturn2
#[test]
fn test_return2() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(NUMBER_TYPE));
    t.in_function("if (!x) { x = 0; }\nreturn x;");
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testWhile1
#[test]
fn test_while1() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(NUMBER_TYPE));
    t.in_function("while (!x) { if (x == null) { x = 0; } else { x = 1; } }");
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testWhile2
#[test]
fn test_while2() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(NUMBER_TYPE));
    t.in_function("while (!x) { x = {}; }");
    t.verify_subtype_of("x", t.create_union_type(OBJECT_TYPE, NUMBER_TYPE));
}

// port: TypeInferenceTest#testDo
#[test]
fn test_do() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("do { x = 1; } while (!x);");
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testFor1
#[test]
fn test_for1() {
    let t = TypeInferenceTest::set_up();
    t.assuming("y", NUMBER_TYPE);
    t.in_function("var x = null; var i = null; for (i=y; !i; i=1) { x = 1; }");
    t.verify("x", t.create_nullable_type(NUMBER_TYPE));
    t.verify("i", NUMBER_TYPE);
}

// port: TypeInferenceTest#testFor4
#[test]
fn test_for4() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("var y = {};\nif (x) { for (var i = 0; i < 10; i++) { break; } y = x; }\n");
    t.verify_subtype_of("y", OBJECT_TYPE);
}

// port: TypeInferenceTest#testFor5
#[test]
fn test_for5() {
    let t = TypeInferenceTest::set_up();
    t.assuming(
        "y",
        t.templatize(
            t.get_native_object_type(ARRAY_TYPE),
            &[t.get_native_type(NUMBER_TYPE)],
        ),
    );
    t.in_function("var x = null; for (var i = 0; i < y.length; i++) { x = y[i]; }");
    t.verify("x", t.create_nullable_type(NUMBER_TYPE));
    t.verify("i", NUMBER_TYPE);
}

// port: TypeInferenceTest#testFor6
#[test]
fn test_for6() {
    let t = TypeInferenceTest::set_up();
    t.assuming("y", t.get_native_object_type(ARRAY_TYPE));
    t.in_function(
        "var x = null;\nfor (var i = 0; i < y.length; i++) {\n if (y[i] == 'z') { x = y[i]; }\n}\n",
    );
    t.verify("x", t.get_native_type(UNKNOWN_TYPE));
    t.verify("i", NUMBER_TYPE);
}

// port: TypeInferenceTest#testSwitch1
#[test]
fn test_switch1() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function(
        "var y = null; switch(x) {\ncase 1: y = 1; break;\ncase 2: y = {};\ncase 3: y = {};\ndefault: y = 0;}\n");
    t.verify("y", NUMBER_TYPE);
}

// port: TypeInferenceTest#testSwitch2
#[test]
fn test_switch2() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", ALL_TYPE);
    t.in_function(
        "var y = null; switch (typeof x) {\ncase 'string':\n  y = x;\n  return;\ndefault:\n  y = 'a';\n}\n");
    t.verify("y", STRING_TYPE);
}

// port: TypeInferenceTest#testSwitch3
#[test]
fn test_switch3() {
    let t = TypeInferenceTest::set_up();
    t.assuming(
        "x",
        t.create_nullable_type(t.create_union_type(NUMBER_TYPE, STRING_TYPE)),
    );
    t.in_function(
        "var y; var z; switch (typeof x) {\ncase 'string':\n  y = 1; z = null;\n  return;\ncase 'number':\n  y = x; z = null;\n  return;\ndefault:\n  y = 1; z = x;\n}\n");
    t.verify("y", NUMBER_TYPE);
    t.verify("z", NULL_TYPE);
}

// port: TypeInferenceTest#testSwitch4
#[test]
fn test_switch4() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", ALL_TYPE);
    t.in_function(
        "var y = null; switch (typeof x) {\ncase 'string':\ncase 'number':\n  y = x;\n  return;\ndefault:\n  y = 1;\n}\n");
    t.verify("y", t.create_union_type(NUMBER_TYPE, STRING_TYPE));
}

// port: TypeInferenceTest#testCall1
#[test]
fn test_call1() {
    let t = TypeInferenceTest::set_up();
    t.assuming(
        "x",
        t.create_nullable_type(fn_type!(t, t.get_native_type(NUMBER_TYPE))),
    );
    t.in_function("var y = x();");
    t.verify("y", NUMBER_TYPE);
}

// port: TypeInferenceTest#testInnerFunction1
#[test]
fn test_inner_function1() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var x = 1; function f() { x = null; };");
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testInnerFunction2
#[test]
fn test_inner_function2() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var x = 1; var f = function() { x = null; };");
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testHook
#[test]
fn test_hook() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("var y = x ? x : {};");
    t.verify_subtype_of("y", OBJECT_TYPE);
}

// port: TypeInferenceTest#testThrow
#[test]
fn test_throw() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(NUMBER_TYPE));
    t.in_function("var y = 1;\nif (x == null) { throw new Error('x is null') }\ny = x;\n");
    t.verify("y", NUMBER_TYPE);
}

// port: TypeInferenceTest#testTry1
#[test]
fn test_try1() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("var y = null; try { y = null; } finally { y = x; }");
    t.verify("y", NUMBER_TYPE);
}

// port: TypeInferenceTest#testTry2
#[test]
fn test_try2() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("var y = null;\ntry {  } catch (e) { y = null; } finally { y = x; }\n");
    t.verify("y", NUMBER_TYPE);
}

// port: TypeInferenceTest#testTry3
#[test]
fn test_try3() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("var y = null; try { y = x; } catch (e) { }");
    t.verify("y", NUMBER_TYPE);
}

// port: TypeInferenceTest#testCatch1
#[test]
fn test_catch1() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var y = null; try { foo(); } catch (e) { y = e; }");
    t.verify("y", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testCatch2
#[test]
fn test_catch2() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var y = null; var e = 3; try { foo(); } catch (e) { y = e; }");
    t.verify("y", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testUnknownType1
#[test]
fn test_unknown_type1() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var y = 3; y = x;");
    t.verify("y", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testUnknownType2
#[test]
fn test_unknown_type2() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", ARRAY_TYPE);
    t.in_function("var y = 5; y = x[0];");
    t.verify("y", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testInfiniteLoop1
#[test]
fn test_infinite_loop1() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("x = {}; while(x != null) { x = {}; }");
}

// port: TypeInferenceTest#testInfiniteLoop2
#[test]
fn test_infinite_loop2() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("x = {}; do { x = null; } while (x == null);");
}

// port: TypeInferenceTest#testJoin1
#[test]
fn test_join1() {
    let t = TypeInferenceTest::set_up();
    let unknown_or_null = t.create_union_type(NULL_TYPE, UNKNOWN_TYPE);
    t.assuming("x", BOOLEAN_TYPE);
    t.assuming("unknownOrNull", unknown_or_null);
    t.in_function("var y; if (x) y = unknownOrNull; else y = null;");
    t.verify("y", unknown_or_null);
}

// port: TypeInferenceTest#testJoin2
#[test]
fn test_join2() {
    let t = TypeInferenceTest::set_up();
    let unknown_or_null = t.create_union_type(NULL_TYPE, UNKNOWN_TYPE);
    t.assuming("x", BOOLEAN_TYPE);
    t.assuming("unknownOrNull", unknown_or_null);
    t.in_function("var y; if (x) y = null; else y = unknownOrNull;");
    t.verify("y", unknown_or_null);
}

// port: TypeInferenceTest#testArrayLit
#[test]
fn test_array_lit() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("var y = 3; if (x) { x = [y = x]; }");
    t.verify("x", t.create_union_type(NULL_TYPE, ARRAY_TYPE));
    t.verify("y", t.create_union_type(NUMBER_TYPE, OBJECT_TYPE));
}

// port: TypeInferenceTest#testGetElem
#[test]
fn test_get_elem() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("var y = 3; if (x) { x = x[y = x]; }");
    t.verify("x", UNKNOWN_TYPE);
    t.verify("y", t.create_union_type(NUMBER_TYPE, OBJECT_TYPE));
}

// port: TypeInferenceTest#testShortCircuitingAnd
#[test]
fn test_short_circuiting_and() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("var y = null; if (x && (y = 3)) { }");
    t.verify("y", t.create_nullable_type(NUMBER_TYPE));
}

// port: TypeInferenceTest#testShortCircuitingAnd2
#[test]
fn test_short_circuiting_and2() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("var y = null; var z = 4; if (x && (y = 3)) { z = y; }");
    t.verify("z", NUMBER_TYPE);
}

// port: TypeInferenceTest#testShortCircuitingOr
#[test]
fn test_short_circuiting_or() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("var y = null; if (x || (y = 3)) { }");
    t.verify("y", t.create_nullable_type(NUMBER_TYPE));
}

// port: TypeInferenceTest#testShortCircuitingNullishCoalesce
#[test]
fn test_short_circuiting_nullish_coalesce() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("var y = null; if (x ?? (y = 3)) { }");
    t.verify("y", NULL_TYPE);
}

// port: TypeInferenceTest#testShortCircuitingNullishCoalesceIf
#[test]
fn test_short_circuiting_nullish_coalesce_if() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("var y = null; var z = 5; if (x ?? (y = 3)) { z = y }");
    t.verify("y", NULL_TYPE);
}

// port: TypeInferenceTest#testShortCircuitingOr2
#[test]
fn test_short_circuiting_or2() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("var y = null; var z = 4; if (x || (y = 3)) { z = y; }");
    t.verify("z", t.create_nullable_type(NUMBER_TYPE));
}

// port: TypeInferenceTest#testShortCircuitingNullishCoalseceNumber
#[test]
fn test_short_circuiting_nullish_coalsece_number() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("var z = x ?? null");
    t.verify("z", NUMBER_TYPE);
}

// port: TypeInferenceTest#testNullishCoalesce
#[test]
fn test_nullish_coalesce() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("var y = 1; x ?? (y = x);");
    t.verify("y", t.create_nullable_type(NUMBER_TYPE));
}

// port: TypeInferenceTest#nullishCoalesceWithHook
#[test]
fn nullish_coalesce_with_hook() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("var z = (x) ?? (x ? 'hi' : false)");
    // Looks like x should be (object|boolean) but hook always traverses both branches
    t.verify(
        "z",
        t.create_multi_param_union_type(&[OBJECT_TYPE, BOOLEAN_TYPE, STRING_TYPE]),
    );
}

// port: TypeInferenceTest#nullishCoalesceRemoveNull
#[test]
fn nullish_coalesce_remove_null() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(NUMBER_TYPE));
    t.in_function("x = x ?? 3");
    t.verify("x", NUMBER_TYPE); // nullability removed by ?? operation
}

// port: TypeInferenceTest#nullishCoalesceZeroIsValid
#[test]
fn nullish_coalesce_zero_is_valid() {
    let t = TypeInferenceTest::set_up();
    // Making sure that ?? does not execute RHS (even if x is 0)
    t.assuming("x", t.create_nullable_type(NUMBER_TYPE));
    t.in_function("var y = ''; if (x ?? (y = x)) { }");
    t.verify("y", t.create_nullable_type(STRING_TYPE));
}

// port: TypeInferenceTest#nullishCoalesceFalseIsValid
#[test]
fn nullish_coalesce_false_is_valid() {
    let t = TypeInferenceTest::set_up();
    // Making sure that ?? does not execute RHS (even if x is false)
    t.assuming("x", t.create_nullable_type(BOOLEAN_TYPE));
    t.in_function("var y = ''; x ?? (y = x)");
    t.verify("y", t.create_nullable_type(STRING_TYPE));
}

// port: TypeInferenceTest#testAssignInCondition
#[test]
fn test_assign_in_condition() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(NUMBER_TYPE));
    t.in_function("var y; if (!(y = x)) { y = 3; }");
    t.verify("y", NUMBER_TYPE);
}

// port: TypeInferenceTest#testInstanceOf1
#[test]
fn test_instance_of1() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", OBJECT_TYPE);
    t.in_function("var y = null; if (x instanceof String) y = x;");
    t.verify("y", t.create_nullable_type(STRING_OBJECT_TYPE));
}

// port: TypeInferenceTest#testInstanceOf2
#[test]
fn test_instance_of2() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("var y = 1; if (x instanceof String) y = x;");
    t.verify("y", t.create_union_type(STRING_OBJECT_TYPE, NUMBER_TYPE));
}

// port: TypeInferenceTest#testInstanceOf3
#[test]
fn test_instance_of3() {
    let t = TypeInferenceTest::set_up();
    t.assuming(
        "x",
        t.create_union_type(STRING_OBJECT_TYPE, NUMBER_OBJECT_TYPE),
    );
    t.in_function("var y = null; if (x instanceof String) y = x;");
    t.verify("y", t.create_nullable_type(STRING_OBJECT_TYPE));
}

// port: TypeInferenceTest#testInstanceOf4
#[test]
fn test_instance_of4() {
    let t = TypeInferenceTest::set_up();
    t.assuming(
        "x",
        t.create_union_type(STRING_OBJECT_TYPE, NUMBER_OBJECT_TYPE),
    );
    t.in_function("var y = null; if (x instanceof String); else y = x;");
    t.verify("y", t.create_nullable_type(NUMBER_OBJECT_TYPE));
}

// port: TypeInferenceTest#testInstanceOf5
#[test]
fn test_instance_of5() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", OBJECT_TYPE);
    t.in_function("var y = null; if (x instanceof String); else y = x;");
    t.verify("y", t.create_nullable_type(OBJECT_TYPE));
}

// port: TypeInferenceTest#testInstanceOf6
#[test]
fn test_instance_of6() {
    let t = TypeInferenceTest::set_up();
    // Here we are using "instanceof" to restrict the unknown type to
    // the type of the instance.  This has the following problems:
    //   1) The type may actually be any sub-type
    //   2) The type may implement any interface
    // After the instanceof we will require casts for methods that require
    // sub-type or unrelated interfaces which would not have been required
    // before.
    let start_type = t.get_native_type(UNKNOWN_TYPE);
    t.assuming("x", start_type);
    t.in_function("out1 = x; if (x instanceof String) out2 = x;");
    t.verify("out1", start_type);
    t.verify("out2", STRING_OBJECT_TYPE);
}

// port: TypeInferenceTest#testUnary
#[test]
fn test_unary() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("var y = +x;");
    t.verify("y", NUMBER_TYPE);
    t.in_function("var z = -x;");
    t.verify("z", NUMBER_TYPE);
}

// port: TypeInferenceTest#testAdd1
#[test]
fn test_add1() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("var y = x + 5;");
    t.verify("y", NUMBER_TYPE);
}

// port: TypeInferenceTest#testAdd2
#[test]
fn test_add2() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("var y = x + '5';");
    t.verify("y", STRING_TYPE);
}

// port: TypeInferenceTest#testAdd3
#[test]
fn test_add3() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("var y = '5' + x;");
    t.verify("y", STRING_TYPE);
}

// port: TypeInferenceTest#testAssignAdd
#[test]
fn test_assign_add() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);
    t.in_function("x += '5';");
    t.verify("x", STRING_TYPE);
}

// port: TypeInferenceTest#testAssignOrToNumeric
#[test]
fn test_assign_or_to_numeric() {
    let t = TypeInferenceTest::set_up();
    // This in line with the existing type-inferencing logic for
    // or/and, since any boolean type is treated as {true, false},
    // but there can be improvement in precision here
    t.in_function("var y = false; y ||= 20");
    t.verify("y", t.create_union_type(NUMBER_TYPE, BOOLEAN_TYPE));
}

// port: TypeInferenceTest#testFunction
#[test]
fn test_function() {
    let t = TypeInferenceTest::set_up();
    // should verify y as string, but due to function-rooted CFG
    // being detached from larger, root CFG, verifies y as void
    t.in_script("let y;\nfunction foo() {\n  y = 'hi';\n}\nfoo();\n");
    t.verify("y", VOID_TYPE);
}

// port: TypeInferenceTest#testClassStaticBlock
#[test]
fn test_class_static_block() {
    let t = TypeInferenceTest::set_up();
    // should verify y as string, but due to static block-rooted CFG
    // being detached from larger, root CFG, verifies y as void
    t.in_script("let y;\nclass Foo {\n  static {\n    y = 'hi';\n  }\n}\n");
    t.verify("y", VOID_TYPE);
}

// port: TypeInferenceTest#testAssignOrNoAssign
#[test]
fn test_assign_or_no_assign() {
    let t = TypeInferenceTest::set_up();
    // The two examples below show imprecision of || operator
    // The resulting type of Node n is (boolean|string), when it can be
    // more precise by verifying `x` as a string
    t.in_function("var x; var y; y = false; x = (y || 'foo');");
    t.verify("x", t.create_union_type(BOOLEAN_TYPE, STRING_TYPE));

    // Short-circuiting should occur, as `a` is a truthy value,
    // `c` would be assigned true, and `b` would remain undefined.
    // To be more precise, `c` may be verified as a BOOLEAN_TYPE,
    // `a` a BOOLEAN_TYPE, and `b` a VOID_TYPE.
    // The actual behavior considers `a` (a BOOLEAN_TYPE) to be {true, false},
    // and states that `c` can be (boolean|string).
    t.in_function("var a; var b; var c; a = true; c = (a || (b = 'foo'));");
    t.verify("c", t.create_union_type(BOOLEAN_TYPE, STRING_TYPE));
    t.verify("a", BOOLEAN_TYPE);
    t.verify("b", t.create_union_type(VOID_TYPE, STRING_TYPE));

    // This test should not assign the string to `y` and
    // should verify `y` as BOOLEAN_TYPE (true).
    t.in_function("var y; y = true; y ||= 'foo';");
    t.verify("y", t.create_union_type(BOOLEAN_TYPE, STRING_TYPE));
}

// port: TypeInferenceTest#testAssignOrToBooleanEitherAbsoluteFalseOrTrue
#[test]
fn test_assign_or_to_boolean_either_absolute_false_or_true() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NULL_TYPE);
    t.in_function("x ||= 'foo';");
    t.verify("x", STRING_TYPE);

    t.assuming("y", OBJECT_TYPE);
    t.in_function("y ||= 'foo';");
    t.verify("y", OBJECT_TYPE);
}

// port: TypeInferenceTest#testAssignOrLHSFalsyRHSTruthy
#[test]
fn test_assign_or_lhs_falsy_rhs_truthy() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NULL_TYPE);
    t.assuming("obj", OBJECT_TYPE);
    t.in_function("x ||= obj;");
    t.verify("x", OBJECT_TYPE);
}

// port: TypeInferenceTest#testAssignAndToNumeric
#[test]
fn test_assign_and_to_numeric() {
    let t = TypeInferenceTest::set_up();
    // This in line with the existing type-inferencing logic for
    // or/and, since any boolean type is treated as {true, false},
    // but there can be improvement in precision here
    t.in_function("var y = true; y &&= 20");
    t.verify("y", t.create_union_type(NUMBER_TYPE, BOOLEAN_TYPE));
}

// port: TypeInferenceTest#testAssignAndNoAssign
#[test]
fn test_assign_and_no_assign() {
    let t = TypeInferenceTest::set_up();
    // This example below show imprecision of && operator
    // The resulting type of Node n is (boolean|string), when it can be
    // more precise by verifying `x` as a boolean type (true).
    t.in_function("var x = true && 'foo';");
    t.verify("x", t.create_union_type(BOOLEAN_TYPE, STRING_TYPE));

    // This test should not assign the string to `y` and
    // should verify `y` as BOOLEAN_TYPE (false).
    t.in_function("var y = false; y &&= 'foo';");
    t.verify("y", t.create_union_type(BOOLEAN_TYPE, STRING_TYPE));
}

// port: TypeInferenceTest#testAssignAndToBooleanEitherAbsoluteFalseOrTrue
#[test]
fn test_assign_and_to_boolean_either_absolute_false_or_true() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NULL_TYPE);
    t.in_function("x &&= 'foo';");
    t.verify("x", NULL_TYPE);

    t.assuming("y", OBJECT_TYPE);
    t.in_function("y &&= 'foo';");
    t.verify("y", STRING_TYPE);
}

// port: TypeInferenceTest#testAssignAndLHSTruthyRHSFalsy
#[test]
fn test_assign_and_lhs_truthy_rhs_falsy() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", OBJECT_TYPE);
    t.assuming("null", NULL_TYPE);
    t.in_function("x &&= null;");
    t.verify("x", NULL_TYPE);
}

// port: TypeInferenceTest#testAssignCoalesceToNumeric
#[test]
fn test_assign_coalesce_to_numeric() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NULL_TYPE);
    t.in_function("x ??= 10;");
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testAssignCoalesceNoAssign
#[test]
fn test_assign_coalesce_no_assign() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", STRING_TYPE);
    t.in_function("x ??= 10;");
    t.verify("x", STRING_TYPE);
}

// port: TypeInferenceTest#testAssignCoalesceRHSAssignmentScope
#[test]
fn test_assign_coalesce_rhs_assignment_scope() {
    let t = TypeInferenceTest::set_up();
    // since lhs is null, ??= executes rhs;
    // precision can be improved in the future for `y` to expect a number and not undefined,
    // but this is currently in accordance with the AST and not an oversight.
    t.in_function("var y; var x = null; x ??= (y = 6)");
    t.verify("y", t.create_union_type(VOID_TYPE, NUMBER_TYPE));
    t.verify("x", NUMBER_TYPE);

    // ??= does not execute rhs
    t.in_function("var y; var x = true; x ??= (y = 'a' + 6)");
    t.verify("y", VOID_TYPE);
    t.verify("x", BOOLEAN_TYPE);
}

// port: TypeInferenceTest#testAssignCoalesceJoin
#[test]
fn test_assign_coalesce_join() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(NUMBER_TYPE));
    t.in_function("var y = ''; x ??= (y = x, 1)");
    t.verify("y", t.create_nullable_type(STRING_TYPE));
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testComparison
#[test]
fn test_comparison() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var x = 'foo'; var y = (x = 3) < 4;");
    t.verify("x", NUMBER_TYPE);
    t.in_function("var x = 'foo'; var y = (x = 3) > 4;");
    t.verify("x", NUMBER_TYPE);
    t.in_function("var x = 'foo'; var y = (x = 3) <= 4;");
    t.verify("x", NUMBER_TYPE);
    t.in_function("var x = 'foo'; var y = (x = 3) >= 4;");
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testComparisonWithBigInt
#[test]
fn test_comparison_with_big_int() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var x = 'foo'; var y = (x = 3n) < 4;");
    t.verify("x", BIGINT_TYPE);
    t.in_function("var x = 'foo'; var y = (x = 3n) > 4;");
    t.verify("x", BIGINT_TYPE);
    t.in_function("var x = 'foo'; var y = (x = 3n) <= 4;");
    t.verify("x", BIGINT_TYPE);
    t.in_function("var x = 'foo'; var y = (x = 3n) >= 4;");
    t.verify("x", BIGINT_TYPE);
}

// port: TypeInferenceTest#testThrownExpression
#[test]
fn test_thrown_expression() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var x = 'foo';\ntry { throw new Error(x = 3); } catch (ex) {}\n");
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testObjectLit
#[test]
fn test_object_lit() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var x = {}; var out = x.a;");
    t.verify("out", UNKNOWN_TYPE); // Shouldn't this be 'undefined'?

    t.in_function("var x = {a:1}; var out = x.a;");
    t.verify("out", NUMBER_TYPE);

    t.in_function("var x = {a:1}; var out = x.a; x.a = 'string'; var out2 = x.a;");
    t.verify("out", NUMBER_TYPE);
    t.verify("out2", STRING_TYPE);

    t.in_function("var x = { get a() {return 1} }; var out = x.a;");
    t.verify("out", UNKNOWN_TYPE);

    t.in_function("var x = {\n  /** @return {number} */ get a() {return 1}\n};\nvar out = x.a;\n");
    t.verify("out", NUMBER_TYPE);

    t.in_function("var x = { set a(b) {} }; var out = x.a;");
    t.verify("out", UNKNOWN_TYPE);

    t.in_function("var x = {\n/** @param {number} b */ set a(b) {} };\nvar out = x.a;\n");
    t.verify("out", NUMBER_TYPE);
}

// port: TypeInferenceTest#testCast1
#[test]
fn test_cast1() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var x = /** @type {Object} */ (this);");
    t.verify("x", t.create_nullable_type(OBJECT_TYPE));
}

// port: TypeInferenceTest#testNoThisInference
#[test]
fn test_no_this_inference() {
    let t = TypeInferenceTest::set_up();
    let this_type = t.create_nullable_type(OBJECT_TYPE);
    t.assuming_this_type(this_type);
    t.in_function("var out = 3; if (goog.isNull(this)) out = this;");
    t.verify("out", t.create_union_type(OBJECT_TYPE, NUMBER_TYPE));
}

// port: TypeInferenceTest#testFunctionTemplateType_literalParam
#[test]
fn test_function_template_type_literal_param() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @template T\n * @param {T} a\n * @return {T}\n */\nfunction f(a){}\n\nvar result = f(10);\n");
    t.verify("result", NUMBER_TYPE);
}

// port: TypeInferenceTest#testFunctionTemplateType_unionsPossibilities
#[test]
fn test_function_template_type_unions_possibilities() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @template T\n * @param {T} a\n * @param {T} b\n * @return {T}\n */\nfunction f(a, b){}\n\nvar result = f(10, 'x');\n");
    t.verify("result", union!(t, NUMBER_TYPE, STRING_TYPE));
}

// port: TypeInferenceTest#testFunctionTemplateType_willUseUnknown
#[test]
fn test_function_template_type_will_use_unknown() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @template T\n * @param {T} a\n * @return {T}\n */\nfunction f(a){}\n\nvar result = f(/** @type {?} */ ({}));\n");
    t.verify("result", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testFunctionTemplateType_willUseUnknown_butPrefersTighterTypes
#[test]
fn test_function_template_type_will_use_unknown_but_prefers_tighter_types() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @template T\n * @param {T} a\n * @param {T} b\n * @param {T} c\n * @return {T}\n */\nfunction f(a, b, c){}\n\n// Make sure `?` is dispreferred before *and* after a known type.\nvar result = f('x', /** @type {?} */ ({}), 5);\n");
    t.verify("result", union!(t, NUMBER_TYPE, STRING_TYPE));
}

// port: TypeInferenceTest#testFunctionTemplateType_recursesIntoFunctionParams
#[test]
fn test_function_template_type_recurses_into_function_params() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @template T\n * @param {function(T)} a\n * @return {T}\n */\nfunction f(a){}\n\nvar result = f(function(/** number */ a) { });\n");
    t.verify("result", NUMBER_TYPE);
}

// port: TypeInferenceTest#testFunctionTemplateType_recursesIntoFunctionParams_throughUnknown
#[test]
fn test_function_template_type_recurses_into_function_params_through_unknown() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @template T\n * @param {function(T)=} a\n * @return {T}\n */\nfunction f(a){}\n\nvar result = f(/** @type {?} */ ({}));\n");
    t.verify("result", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testFunctionTemplateType_unpacksUnions_fromParamType
#[test]
fn test_function_template_type_unpacks_unions_from_param_type() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @template T\n * @param {!Iterable<T>|number} a\n * @return {T}\n */\nfunction f(a){}\n\nvar result = f(/** @type {!Iterable<number>} */ ({}));\n");
    t.verify("result", NUMBER_TYPE);
}

// port: TypeInferenceTest#testFunctionTemplateType_unpacksUnions_fromArgType
#[test]
fn test_function_template_type_unpacks_unions_from_arg_type() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @template T\n * @param {!Iterable<T>} a\n * @return {T}\n */\nfunction f(a){}\n\n// The arg type is illegal, but the inference should still work.\nvar result = f(/** @type {!Iterable<number>|number} */ ({}));\n");
    t.verify("result", NUMBER_TYPE);
}

// port: TypeInferenceTest#testFunctionTemplateType_unpacksUnions_fromArgType_acrossSubtypes
#[test]
fn test_function_template_type_unpacks_unions_from_arg_type_across_subtypes() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @template T\n * @param {!Iterable<T>} a\n * @return {T}\n */\nfunction f(a){}\n\nvar result = f(/** @type {!Array<number>|!Generator<string>} */ ({}));\n");
    t.verify("result", union!(t, NUMBER_TYPE, STRING_TYPE));
}

// port: TypeInferenceTest#testTypeTransformationTypePredicate
#[test]
fn test_type_transformation_type_predicate() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @return {R}\n * @template R := 'number' =:\n */\nfunction f(a){}\nvar result = f(10);\n");
    t.verify("result", NUMBER_TYPE);
}

// port: TypeInferenceTest#testTypeTransformationConditional
#[test]
fn test_type_transformation_conditional() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @param {T} a\n * @param {N} b\n * @return {R}\n * @template T, N\n * @template R := cond( eq(T, N), 'string', 'boolean' ) =:\n */\nfunction f(a, b){}\nvar result = f(1, 2);\nvar result2 = f(1, 'a');\n");
    t.verify("result", STRING_TYPE);
    t.verify("result2", BOOLEAN_TYPE);
}

// port: TypeInferenceTest#testTypeTransformationUnionType
#[test]
fn test_type_transformation_union_type() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @param {S} a\n * @param {N} b\n * @return {R}\n * @template S, N\n * @template R := union(S, N) =:\n */\nfunction f(a, b) {}\nvar result = f(1, 'a');\n");
    t.verify("result", t.create_union_type(STRING_TYPE, NUMBER_TYPE));
}

// port: TypeInferenceTest#testTypeTransformationMapunion
#[test]
fn test_type_transformation_mapunion() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @param {U} a\n * @return {R}\n * @template U\n * @template R :=\n * mapunion(U, (x) => cond(eq(x, 'string'), 'boolean', 'null'))\n * =:\n */\nfunction f(a) {}\n/** @type {string|number} */ var x;\nvar result = f(x);\n");
    t.verify("result", t.create_union_type(BOOLEAN_TYPE, NULL_TYPE));
}

// port: TypeInferenceTest#testTypeTransformationObjectUseCase2
#[test]
fn test_type_transformation_object_use_case2() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** \n * @param {T} a\n * @return {R}\n * @template T \n * @template R := \n * mapunion(T, (x) => \n *      cond(eq(x, 'string'), 'String',\n *      cond(eq(x, 'number'), 'Number',\n *      cond(eq(x, 'boolean'), 'Boolean',\n *      cond(eq(x, 'null'), 'Object', \n *      cond(eq(x, 'undefined'), 'Object',\n *      x)))))) \n * =:\n */\nfunction fn(a) {}\n/** @type {(string|null|undefined)} */\nvar o;\nvar r = fn(o);\n");
    t.verify("r", OBJECT_TYPE);
}

// port: TypeInferenceTest#testTypeTransformationObjectUseCase3
#[test]
fn test_type_transformation_object_use_case3() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** \n * @param {T} a\n * @return {R}\n * @template T \n * @template R := \n * mapunion(T, (x) => \n *      cond(eq(x, 'string'), 'String',\n *      cond(eq(x, 'number'), 'Number',\n *      cond(eq(x, 'boolean'), 'Boolean',\n *      cond(eq(x, 'null'), 'Object', \n *      cond(eq(x, 'undefined'), 'Object',\n *      x)))))) \n * =:\n */\nfunction fn(a) {}\n/** @type {(Array|undefined)} */\nvar o;\nvar r = fn(o);\n");
    t.verify("r", OBJECT_TYPE);
}

// port: TypeInferenceTest#testTypeTransformationTypeOfVarWithInstanceOfConstructor
#[test]
fn test_type_transformation_type_of_var_with_instance_of_constructor() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @constructor */\nfunction Bar() {}\nvar b = new Bar();\n/** \n * @return {R}\n * @template R := typeOfVar('b') =:\n */\nfunction f(){}\nvar r = f();\n");
    t.verify("r", t.get_type("b"));
}

// port: TypeInferenceTest#testTypeTransformationTypeOfVarWithConstructor
#[test]
fn test_type_transformation_type_of_var_with_constructor() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @constructor */\nfunction Bar() {}\n/** \n * @return {R}\n * @template R := typeOfVar('Bar') =:\n */\nfunction f(){}\nvar r = f();\n");
    t.verify("r", t.get_type("Bar"));
}

// port: TypeInferenceTest#testTypeTransformationTypeOfVarWithTypedef
#[test]
fn test_type_transformation_type_of_var_with_typedef() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @typedef {(string|number)} */\nvar NumberLike;\n/** @type {!NumberLike} */\nvar x;\n/**\n * @return {R}\n * @template R := typeOfVar('x') =:\n */\nfunction f(){}\nvar r = f();\n");
    t.verify("r", t.get_type("x"));
}

// port: TypeInferenceTest#testTypeTransformationWithTypeFromConstructor
#[test]
fn test_type_transformation_with_type_from_constructor() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @constructor */\nfunction Bar(){}\nvar x = new Bar();\n/** \n * @return {R}\n * @template R := 'Bar' =:\n */\nfunction f(){}\nvar r = f();\n");
    t.verify("r", t.get_type("x"));
}

// port: TypeInferenceTest#testTypeTransformationWithTypeFromTypedef
#[test]
fn test_type_transformation_with_type_from_typedef() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @typedef {(string|number)} */\nvar NumberLike;\n/** @type {!NumberLike} */\nvar x;\n/**\n * @return {R}\n * @template R := 'NumberLike' =:\n */\nfunction f(){}\nvar r = f();\n");
    t.verify("r", t.create_union_type(STRING_TYPE, NUMBER_TYPE));
}

// port: TypeInferenceTest#testTypeTransformationWithTypeFromNamespace
#[test]
fn test_type_transformation_with_type_from_namespace() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "var wiz\n/** @constructor */\nwiz.async.Response = function() {};\n/**\n * @return {R}\n * @template R := typeOfVar('wiz.async.Response') =:\n */\nfunction f(){}\nvar r = f();\n");
    t.verify("r", t.get_type("wiz.async.Response"));
}

// port: TypeInferenceTest#testTypeTransformationWithNativeTypeExpressionFunction
#[test]
fn test_type_transformation_with_native_type_expression_function() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @type {function(string, boolean)} */\nvar x;\n/**\n * @return {R}\n * @template R := typeExpr('function(string, boolean)') =:\n */\nfunction f(){}\nvar r = f();\n");
    t.verify("r", t.get_type("x"));
}

// port: TypeInferenceTest#testTypeTransformationWithNativeTypeExpressionFunctionReturn
#[test]
fn test_type_transformation_with_native_type_expression_function_return() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @type {function(): number} */\nvar x;\n/**\n * @return {R}\n * @template R := typeExpr('function(): number') =:\n */\nfunction f(){}\nvar r = f();\n");
    t.verify("r", t.get_type("x"));
}

// port: TypeInferenceTest#testTypeTransformationWithNativeTypeExpressionFunctionThis
#[test]
fn test_type_transformation_with_native_type_expression_function_this() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @type {function(this:boolean, string)} */\nvar x;\n/**\n * @return {R}\n * @template R := typeExpr('function(this:boolean, string)') =:\n */\nfunction f(){}\nvar r = f();\n");
    t.verify("r", t.get_type("x"));
}

// port: TypeInferenceTest#testTypeTransformationWithNativeTypeExpressionFunctionVarargs
#[test]
fn test_type_transformation_with_native_type_expression_function_varargs() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @type {function(string, ...number): number} */\nvar x;\n/**\n * @return {R}\n * @template R := typeExpr('function(string, ...number): number') =:\n */\nfunction f(){}\nvar r = f();\n");
    t.verify("r", t.get_type("x"));
}

// port: TypeInferenceTest#testTypeTransformationWithNativeTypeExpressionFunctionOptional
#[test]
fn test_type_transformation_with_native_type_expression_function_optional() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @type {function(?string=, number=)} */\nvar x;\n/**\n * @return {R}\n * @template R := typeExpr('function(?string=, number=)') =:\n */\nfunction f(){}\nvar r = f();\n");
    t.verify("r", t.get_type("x"));
}

// port: TypeInferenceTest#testAssertTypeofProp
#[test]
fn test_assert_typeof_prop() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("goog.asserts.assert(typeof x.prop != 'undefined');\nout = x.prop;\n");
    t.verify("out", CHECKED_UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testIsArray
#[test]
fn test_is_array() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_nullable_type(OBJECT_TYPE));
    t.in_function("goog.asserts.assert(Array.isArray(x));");
    t.verify("x", ARRAY_TYPE);
}

// port: TypeInferenceTest#testNotIsArray
#[test]
fn test_not_is_array() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", t.create_union_type(ARRAY_TYPE, NUMBER_TYPE));
    t.in_function("goog.asserts.assert(!Array.isArray(x));");
    t.verify("x", NUMBER_TYPE);
}

// port: TypeInferenceTest#testYield1
#[test]
fn test_yield1() {
    let t = TypeInferenceTest::set_up();
    t.in_generator("var x = yield 3;");
    t.verify("x", t.get_native_type(UNKNOWN_TYPE));
}

// port: TypeInferenceTest#testYield2
#[test]
fn test_yield2() {
    let t = TypeInferenceTest::set_up();
    // test that type inference happens inside the yield expression
    t.in_generator("var obj;\nyield (obj = {a: 3, b: '4'});\nvar a = obj.a;\nvar b = obj.b;\n");

    t.verify("a", t.get_native_type(NUMBER_TYPE));
    t.verify("b", t.get_native_type(STRING_TYPE));
}

// port: TypeInferenceTest#testTemplateLiteral1
#[test]
fn test_template_literal1() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var x = `foobar`; X: x;");
    t.assert_type_of_expression("X").is_string();
}

// port: TypeInferenceTest#testSpreadExpression
#[test]
fn test_spread_expression() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "let x = 1; // x is initially a number\nlet y = [...[x = 'hi', 'there']]; // reassign x a string in the spread\nX: x;\n");
    t.assert_type_of_expression("X")
        .to_string_is_equal_to("string");
}

// port: TypeInferenceTest#testTaggedTemplateLiteral1
#[test]
fn test_tagged_template_literal1() {
    let t = TypeInferenceTest::set_up();
    t.assuming("getNumber", fn_type!(t, t.get_native_type(NUMBER_TYPE)));
    t.in_function("var num = getNumber``; NUM: num;");

    t.assert_type_of_expression("NUM").is_number();
}

// port: TypeInferenceTest#testObjectDestructuringDeclarationInferenceWithDefaultValue
#[test]
fn test_object_destructuring_declaration_inference_with_default_value() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "var /** {x: (?string|undefined)} */ obj;\nlet {x = 3} = obj;  // preserve newline\nX: x;\n");
    t.assert_type_of_expression("X")
        .to_string_is_equal_to("(null|number|string)");
}

// port: TypeInferenceTest#testObjectDestructuringDeclarationInferenceWithUnnecessaryDefaultValue
#[test]
fn test_object_destructuring_declaration_inference_with_unnecessary_default_value() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "var /** {x: string} */ obj;\nlet {x = 3} = obj;  // we ignore the default value's type\nX: x;\n");
    // TODO(b/77597706): should this just be `string`?
    // the legacy behavior (typechecking transpiled code) produces (number|string), but we should
    // possibly realize that the default value will never be evaluated.
    t.assert_type_of_expression("X")
        .to_string_is_equal_to("(number|string)");
}

// port: TypeInferenceTest#testObjectDestructuringDeclarationInference_unknownRhsAndKnownDefaultValue
#[test]
fn test_object_destructuring_declaration_inference_unknown_rhs_and_known_default_value() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var /** ? */ obj;\nlet {x = 3} = obj;  // preserve newline\nX: x;\n");
    t.assert_type_of_expression("X").to_string_is_equal_to("?");
}

// port: TypeInferenceTest#testObjectDestructuringDeclarationInference_knownRhsAndUnknownDefaultValue
#[test]
fn test_object_destructuring_declaration_inference_known_rhs_and_unknown_default_value() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "var /** {x: (string|undefined)} */ obj;\nlet {x = someUnknown} = obj;  // preserve newline\nX: x;\n");
    t.assert_type_of_expression("X").to_string_is_equal_to("?");
}

// port: TypeInferenceTest#testObjectDestructuringDeclaration_defaultValueEvaluatedAfterComputedProperty
#[test]
fn test_object_destructuring_declaration_default_value_evaluated_after_computed_property() {
    let t = TypeInferenceTest::set_up();
    // contrived example to verify that we traverse the computed property before the default value.

    t.in_function(
        "var /** !Object<string, (number|undefined)> */ obj = {};\nvar a = 1;\nconst {[a = 'string']: b = a} = obj\nA: a\nB: b\n");

    t.assert_type_of_expression("A")
        .to_string_is_equal_to("string");
    t.assert_type_of_expression("B")
        .to_string_is_equal_to("(number|string)");
}

// port: TypeInferenceTest#testObjectDestructuringDoesInferenceWithinComputedProp
#[test]
fn test_object_destructuring_does_inference_within_computed_prop() {
    let t = TypeInferenceTest::set_up();
    t.in_function("let y = 'foobar';  // preserve newline\nlet {[y = 3]: z} = {};\nY: y\nZ: z\n");

    t.assert_type_of_expression("Y")
        .to_string_is_equal_to("number");
    t.assert_type_of_expression("Z").to_string_is_equal_to("?");
}

// port: TypeInferenceTest#testObjectDestructuringUsesIObjectTypeForComputedProp
#[test]
fn test_object_destructuring_uses_i_object_type_for_computed_prop() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "let /** !IObject<string, number> */ myObj = {['foo']: 3};  // preserve newline\nlet {[42]: x} = myObj;\nX: x\n");

    t.assert_type_of_expression("X")
        .to_string_is_equal_to("number");
}

// port: TypeInferenceTest#testObjectDestructuringDeclarationWithNestedPattern
#[test]
fn test_object_destructuring_declaration_with_nested_pattern() {
    let t = TypeInferenceTest::set_up();
    t.in_function("let /** {a: {b: number}} */ obj = {a: {b: 3}};\nlet {a: {b: x}} = obj;\nX: x\n");

    t.assert_type_of_expression("X")
        .to_string_is_equal_to("number");
}

// port: TypeInferenceTest#testObjectDestructuringAssignmentToQualifiedName
#[test]
fn test_object_destructuring_assignment_to_qualified_name() {
    let t = TypeInferenceTest::set_up();
    t.in_function("const ns = {};\n({x: ns.x} = {x: 3});\nX: ns.x;\n");

    t.assert_type_of_expression("X")
        .to_string_is_equal_to("number");
}

// port: TypeInferenceTest#testObjectDestructuringDeclarationInForOf
#[test]
fn test_object_destructuring_declaration_in_for_of() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "const /** !Iterable<{x: number}> */ data = [{x: 3}];\nfor (let {x} of data) {\n  X: x;\n}\n");

    t.assert_type_of_expression("X")
        .to_string_is_equal_to("number");
}

// port: TypeInferenceTest#testObjectDestructuringAssignInForOf
#[test]
fn test_object_destructuring_assign_in_for_of() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "const /** !Iterable<{x: number}> */ data = [{x: 3}];\nvar x;\nfor ({x} of data) {\n  X: x;\n}\n");

    t.assert_type_of_expression("X")
        .to_string_is_equal_to("number");
}

// port: TypeInferenceTest#testObjectRest_inferredGivenObjectLiteralType
#[test]
fn test_object_rest_inferred_given_object_literal_type() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var obj = {a: 1, b: 2, c: 3}; const {a, ...rest} = obj;  A: a; REST: rest;");

    t.assert_type_of_expression("A")
        .to_string_is_equal_to("number");
    t.assert_type_of_expression("REST")
        .is_equal_to(t.get_native_type(OBJECT_TYPE));
}

// port: TypeInferenceTest#testArrayDestructuringDeclaration
#[test]
fn test_array_destructuring_declaration() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "const /** !Iterable<number> */ numbers = [1, 2, 3];\nlet [x, y] = numbers;\nX: x\nY: y\n",
    );

    t.assert_type_of_expression("X")
        .to_string_is_equal_to("number");
    t.assert_type_of_expression("Y")
        .to_string_is_equal_to("number");
}

// port: TypeInferenceTest#testArrayDestructuringDeclarationWithDefaultValue
#[test]
fn test_array_destructuring_declaration_with_default_value() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "const /** !Iterable<(number|undefined)> */ numbers = [1, 2, 3];\nlet [x = 'x', y = 'y'] = numbers;\nX: x\nY: y\n");

    t.assert_type_of_expression("X")
        .to_string_is_equal_to("(number|string)");
    t.assert_type_of_expression("Y")
        .to_string_is_equal_to("(number|string)");
}

// port: TypeInferenceTest#testArrayDestructuringDeclarationWithDefaultValueForNestedPattern
#[test]
fn test_array_destructuring_declaration_with_default_value_for_nested_pattern() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "const /** !Iterable<({x: number}|undefined)> */ xNumberObjs = [];\nlet [{x = 'foo'} = {}] = xNumberObjs;\nX: x\nY: y\n");

    t.assert_type_of_expression("X")
        .to_string_is_equal_to("(number|string)");
}

// port: TypeInferenceTest#testArrayDestructuringDeclarationWithRest
#[test]
fn test_array_destructuring_declaration_with_rest() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "const /** !Iterable<number> */ numbers = [1, 2, 3];\nlet [x, ...y] = numbers;\nX: x\nY: y\n");

    t.assert_type_of_expression("X")
        .to_string_is_equal_to("number");
    t.assert_type_of_expression("Y")
        .to_string_is_equal_to("Array<number>");
}

// port: TypeInferenceTest#testArrayDestructuringDeclarationWithNestedArrayPattern
#[test]
fn test_array_destructuring_declaration_with_nested_array_pattern() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "const /** !Iterable<!Iterable<number>> */ numbers = [[1, 2, 3]];\nlet [[x], y] = numbers;\nX: x\nY: y\n");

    t.assert_type_of_expression("X")
        .to_string_is_equal_to("number");
    t.assert_type_of_expression("Y")
        .to_string_is_equal_to("Iterable<number,?,?>");
}

// port: TypeInferenceTest#testArrayDestructuringDeclarationWithNestedObjectPattern
#[test]
fn test_array_destructuring_declaration_with_nested_object_pattern() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "const /** !Iterable<{x: number}> */ numbers = [{x: 3}, {x: 4}];\nlet [{x}, {x: y}] = numbers;\nX: x\nY: y\n");

    t.assert_type_of_expression("X")
        .to_string_is_equal_to("number");
    t.assert_type_of_expression("Y")
        .to_string_is_equal_to("number");
}

// port: TypeInferenceTest#testArrayDestructuringDeclarationWithNonIterableRhs
#[test]
fn test_array_destructuring_declaration_with_non_iterable_rhs() {
    let t = TypeInferenceTest::set_up();
    // TODO(lharker): make sure TypeCheck warns on this
    t.in_function("let [x] = 3; X: x;");

    t.assert_type_of_expression("X").to_string_is_equal_to("?");
}

// port: TypeInferenceTest#testArrayDestructuringAssignWithGetProp
#[test]
fn test_array_destructuring_assign_with_get_prop() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "const ns = {};\nconst /** !Iterable<number> */ numbers = [1, 2, 3];\n[ns.x] = numbers;\nNSX: ns.x;\n");

    t.assert_type_of_expression("NSX")
        .to_string_is_equal_to("number");
}

// port: TypeInferenceTest#testArrayDestructuringAssignWithGetElem
#[test]
fn test_array_destructuring_assign_with_get_elem() {
    let t = TypeInferenceTest::set_up();
    // we don't update the scope on an assignment to a getelem, so this test just verifies that
    // a) type inference doesn't crash and b) type info validation passes.
    t.in_function(
        "const arr = [];\nconst /** !Iterable<number> */ numbers = [1, 2, 3];\n[arr[1]] = numbers;\nARR1: arr[1];\n");

    t.assert_type_of_expression("ARR1")
        .to_string_is_equal_to("?");
}

// port: TypeInferenceTest#testDeclarationDoesntOverrideInferredTypeInDestructuringPattern
#[test]
fn test_declaration_doesnt_override_inferred_type_in_destructuring_pattern() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var [/** number */ x] = /** @type {?} */ ([null]); X: x");

    t.assert_type_of_expression("X")
        .to_string_is_equal_to("number");
}

// port: TypeInferenceTest#testDeclarationDoesntOverrideInferredTypeInForOfLoop
#[test]
fn test_declaration_doesnt_override_inferred_type_in_for_of_loop() {
    let t = TypeInferenceTest::set_up();
    t.in_function("for (var /** number */ x of /** @type {?} */ [null]) { X: x; }");

    t.assert_type_of_expression("X")
        .to_string_is_equal_to("number");
}

// port: TypeInferenceTest#testTypeInferenceOccursInDestructuringCatch
#[test]
fn test_type_inference_occurs_in_destructuring_catch() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);

    t.in_function(
        "try {\n  throw {err: 3};\n} catch ({[x = 'err']: /** number */ err}) {\n  ERR: err;\n  X: x;\n}\n");

    t.assert_type_of_expression("ERR")
        .to_string_is_equal_to("number");
    // verify we do inference on the assignment to `x` inside the computed property
    t.assert_type_of_expression("X")
        .to_string_is_equal_to("string");
}

// port: TypeInferenceTest#testTypeInferenceOccursInDestructuringForIn
#[test]
fn test_type_inference_occurs_in_destructuring_for_in() {
    let t = TypeInferenceTest::set_up();
    t.assuming("x", NUMBER_TYPE);

    t.in_function(
        "/** @type {number} */\nString.prototype.length;\n\nvar obj = {};\nfor ({length: obj.length} in {'1': 1, '22': 22}) {\n  LENGTH: obj.length; // set to '1'.length and '22'.length\n}\n");

    t.assert_type_of_expression("LENGTH")
        .to_string_is_equal_to("number");
}

// port: TypeInferenceTest#testTypeInferenceOccursInsideVoidOperator
#[test]
fn test_type_inference_occurs_inside_void_operator() {
    let t = TypeInferenceTest::set_up();
    t.in_function("var x; var y = void (x = 3); X: x; Y: y");

    t.assert_type_of_expression("X")
        .to_string_is_equal_to("number");
    t.assert_type_of_expression("Y")
        .to_string_is_equal_to("undefined");
}

// port: TypeInferenceTest#testSideEffectsInEsExportDefaultInferred
#[test]
fn test_side_effects_in_es_export_default_inferred() {
    let t = TypeInferenceTest::set_up();
    t.assuming("foo", NUMBER_TYPE);
    t.assuming("bar", UNKNOWN_TYPE);

    t.in_script("export default (bar = foo, foo = 'not a number');");

    t.assert_type(Some(t.get_type("bar"))).is_number();
    t.assert_type(Some(t.get_type("foo"))).is_string();
}

// port: TypeInferenceTest#testShneTightensUnknownOperandOnLhs
#[test]
fn test_shne_tightens_unknown_operand_on_lhs() {
    let t = TypeInferenceTest::set_up();
    t.assuming("foo", NUMBER_TYPE);
    t.assuming("bar", UNKNOWN_TYPE);

    t.in_function("if (bar === foo) { FOO: foo; BAR: bar; }");

    t.assert_type_of_expression("FOO").is_number();
    t.assert_type_of_expression("BAR").is_number();
}

// port: TypeInferenceTest#testShneTightensUnknownOperandOnRhs
#[test]
fn test_shne_tightens_unknown_operand_on_rhs() {
    let t = TypeInferenceTest::set_up();
    t.assuming("foo", NUMBER_TYPE);
    t.assuming("bar", UNKNOWN_TYPE);

    t.in_function("if (foo === bar) { FOO: foo; BAR: bar; }");

    t.assert_type_of_expression("FOO").is_number();
    t.assert_type_of_expression("BAR").is_number();
}

// port: TypeInferenceTest#testDynamicImport2
#[test]
fn test_dynamic_import2() {
    let t = TypeInferenceTest::set_up();
    t.with_modules(&[
        "export default 1; export /** @return {string} */ function Bar() { return 'bar'; };",
        // modules are named of the format `testcode#` based off their index.
        // testcode0 refers the first module.
        "const foo = import('./testcode0');",
    ]);

    t.assert_type(Some(t.get_type("foo")))
        .to_string_is_equal_to("Promise<{\n  Bar: function(): string,\n  default: number\n}>");
}

// port: TypeInferenceTest#testRequireDynamic
#[test]
fn test_require_dynamic() {
    let t = TypeInferenceTest::set_up();
    t.with_modules(&[
        "goog.module('foo'); exports.barFunc = function Bar() { return 'bar'; };",
        "const f = goog.requireDynamic('foo');",
        "const e = goog.require('foo');",
    ]);

    t.assert_type(Some(t.get_type("f")))
        .to_string_is_equal_to("IThenable<{barFunc: function(): ?}>");
    t.assert_type(Some(t.get_type("e")))
        .to_string_is_equal_to("{barFunc: function(): ?}");
}

// port: TypeInferenceTest#testDynamicImportAfterModuleRewriting
#[test]
fn test_dynamic_import_after_module_rewriting() {
    let t = TypeInferenceTest::set_up();
    t.with_modules(
        &[
            "const module$testcode0 = {};\n/** @const */ module$testcode0.default = 1;\n/** @return {string} */ function Bar() { return 'bar'; };\n/** @const */ module$testcode0.Bar = Bar;\n",
            // modules are named of the format `testcode#` based off their index.
            // testcode0 refers the first module.
            "const foo = import('./testcode0');"]);

    t.assert_type(Some(t.get_type("foo")))
        .to_string_is_equal_to("Promise<{\n  Bar: function(): string,\n  default: number\n}>");
}

// port: TypeInferenceTest#testGetElemDereference_knownSymbol_onAllType
#[test]
fn test_get_elem_dereference_known_symbol_on_all_type() {
    let t = TypeInferenceTest::set_up();
    let sym = t.new_known_symbol_type("sym");
    t.assuming("o", ALL_TYPE);
    t.assuming("sym", sym);

    t.in_function("const x = o[sym];");
    t.verify("x", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testGetElemDereference_knownSymbol_onUnknownType
#[test]
fn test_get_elem_dereference_known_symbol_on_unknown_type() {
    let t = TypeInferenceTest::set_up();
    let sym = t.new_known_symbol_type("sym");
    t.assuming("o", UNKNOWN_TYPE);
    t.assuming("sym", sym);

    t.in_function("const x = o[sym];");
    t.verify("x", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testPropertyInference1
#[test]
fn test_property_inference1() {
    let t = TypeInferenceTest::set_up();
    let this_type = t.registry_create_anonymous_object_type();
    this_type.define_declared_property_t(&t, "foo", t.create_undefinable_type(STRING_TYPE), None);
    t.assuming_this_type(this_type);
    t.in_function("var y = 1; if (this.foo) { y = this.foo; }");
    t.verify("y", t.create_union_type(NUMBER_TYPE, STRING_TYPE));
}

// port: TypeInferenceTest#testPropertyInference2
#[test]
fn test_property_inference2() {
    let t = TypeInferenceTest::set_up();
    let this_type = t.registry_create_anonymous_object_type();
    this_type.define_declared_property_t(&t, "foo", t.create_undefinable_type(STRING_TYPE), None);
    t.assuming_this_type(this_type);
    t.in_function("var y = 1; this.foo = 'x'; y = this.foo;");
    t.verify("y", STRING_TYPE);
}

// port: TypeInferenceTest#testPropertyInference3
#[test]
fn test_property_inference3() {
    let t = TypeInferenceTest::set_up();
    let this_type = t.registry_create_anonymous_object_type();
    this_type.define_declared_property_t(&t, "foo", t.create_undefinable_type(STRING_TYPE), None);
    t.assuming_this_type(this_type);
    t.in_function("var y = 1; this.foo = x; y = this.foo;");
    t.verify("y", t.create_undefinable_type(STRING_TYPE));
}

// port: TypeInferenceTest#testBigIntPresence
#[test]
fn test_big_int_presence() {
    let t = TypeInferenceTest::set_up();
    // Standard types
    t.test_for_all_big_int(t.get_native_type(BIGINT_TYPE));
    t.test_for_all_big_int(t.get_native_type(BIGINT_OBJECT_TYPE));
    t.test_for_no_big_int(t.get_native_type(NUMBER_TYPE));
    t.test_for_no_big_int(t.get_native_type(STRING_TYPE));
    t.test_for_no_big_int(t.get_native_type(ALL_TYPE));
    t.test_for_no_big_int(t.get_native_type(UNKNOWN_TYPE));
    t.test_for_no_big_int(t.get_native_type(NO_TYPE));

    // Unions
    t.test_for_all_big_int(t.create_union_type(BIGINT_TYPE, BIGINT_OBJECT_TYPE));
    t.test_for_big_int_or_number(t.get_native_type(BIGINT_NUMBER));
    t.test_for_big_int_or_other(t.create_union_type(BIGINT_TYPE, STRING_TYPE));
    t.test_for_no_big_int(t.get_native_type(NUMBER_STRING));

    // Union within union
    t.test_for_big_int_or_number(t.create_union_type(NUMBER_OBJECT_TYPE, BIGINT_NUMBER));
    t.test_for_big_int_or_number(t.create_union_type(BIGINT_OBJECT_TYPE, BIGINT_NUMBER));
    t.test_for_big_int_or_number(union!(
        t,
        t.get_native_type(NUMBER_TYPE),
        t.create_union_type(BIGINT_TYPE, BIGINT_OBJECT_TYPE)
    ));
    t.test_for_big_int_or_number(union!(
        t,
        t.get_native_type(BIGINT_TYPE),
        t.create_union_type(NUMBER_TYPE, NUMBER_OBJECT_TYPE)
    ));
    t.test_for_big_int_or_other(union!(
        t,
        t.get_native_type(BIGINT_TYPE),
        t.create_union_type(STRING_TYPE, STRING_OBJECT_TYPE)
    ));

    // Enum within union
    t.test_for_all_big_int(union!(
        t,
        t.get_native_type(BIGINT_OBJECT_TYPE),
        t.create_enum_type("Enum", BIGINT_TYPE)
            .get_elements_type_t(&t)
    ));
    t.test_for_big_int_or_number(union!(
        t,
        t.get_native_type(BIGINT_TYPE),
        t.create_enum_type("Enum", NUMBER_TYPE)
            .get_elements_type_t(&t)
    ));
    t.test_for_big_int_or_other(union!(
        t,
        t.get_native_type(BIGINT_TYPE),
        t.create_enum_type("Enum", STRING_TYPE)
            .get_elements_type_t(&t)
    ));

    // Standard enum
    t.test_for_all_big_int(
        t.create_enum_type("Enum", BIGINT_TYPE)
            .get_elements_type_t(&t),
    );
    t.test_for_no_big_int(
        t.create_enum_type("Enum", NUMBER_TYPE)
            .get_elements_type_t(&t),
    );

    // Enum within enum
    t.test_for_all_big_int(
        t.create_enum_type(
            "Enum",
            t.create_enum_type("Enum", BIGINT_TYPE)
                .get_elements_type_t(&t),
        )
        .get_elements_type_t(&t),
    );
    t.test_for_no_big_int(
        t.create_enum_type(
            "Enum",
            t.create_enum_type("Enum", NUMBER_TYPE)
                .get_elements_type_t(&t),
        )
        .get_elements_type_t(&t),
    );

    // Union within enum
    t.test_for_all_big_int(
        t.create_enum_type("Enum", t.create_union_type(BIGINT_TYPE, BIGINT_OBJECT_TYPE))
            .get_elements_type_t(&t),
    );
    t.test_for_big_int_or_number(
        t.create_enum_type("Enum", BIGINT_NUMBER)
            .get_elements_type_t(&t),
    );
    t.test_for_big_int_or_other(
        t.create_enum_type("Enum", t.create_union_type(BIGINT_TYPE, STRING_TYPE))
            .get_elements_type_t(&t),
    );
}

// port: TypeInferenceTest#testBigIntEnumWithUnaryPlus
#[test]
fn test_big_int_enum_with_unary_plus() {
    let t = TypeInferenceTest::set_up();
    let enum_element_big_int_type = t
        .create_enum_type("MyEnum", BIGINT_TYPE)
        .get_elements_type_t(&t);
    let enum_element_union_type = t
        .create_enum_type("MyEnum", BIGINT_NUMBER)
        .get_elements_type_t(&t);
    t.assuming("x", enum_element_big_int_type);
    t.assuming(
        "y",
        union!(t, enum_element_big_int_type, t.get_native_type(NUMBER_TYPE)),
    );
    t.assuming("z", enum_element_union_type);

    t.in_function("enumElementBigIntType = +x; unionEnumType = +y; enumElementUnionType = +z;");

    // Unary plus throws an exception when applied to a BigInt, so there is no valid type for its
    // result.
    t.verify("enumElementBigIntType", NO_TYPE);
    t.verify("unionEnumType", NO_TYPE);
    t.verify("enumElementUnionType", NO_TYPE);
}

// port: TypeInferenceTest#testAssertElement_doesNotChangeElementType
#[test]
fn test_assert_element_does_not_change_element_type() {
    let t = TypeInferenceTest::set_up();
    let element_type =
        t.registry_create_object_type("Element", t.get_native_object_type(OBJECT_TYPE));
    t.include_goog_assertion_fn("assertElement", element_type);
    t.assuming("x", element_type);

    t.in_function("out1 = x; goog.asserts.assertElement(x); out2 = x;");

    t.verify("out1", element_type);
    t.verify("out2", element_type);
}

// port: TypeInferenceTest#testForInWithExistingVar
#[test]
fn test_for_in_with_existing_var() {
    let t = TypeInferenceTest::set_up();
    t.assuming("y", OBJECT_TYPE);
    t.in_function(
        "var x = null;\nvar i = null;\nfor (i in y) {\n  I_INSIDE_LOOP: i;\n  X_AT_LOOP_START: x;\n  x = 1;\n  X_AT_LOOP_END: x;\n}\nX_AFTER_LOOP: x;\nI_AFTER_LOOP: i;\n");
    t.assert_scope_enclosing(&mut t.c(), "I_INSIDE_LOOP")
        .declares("i")
        .on_closest_hoist_scope();
    t.assert_scope_enclosing(&mut t.c(), "I_INSIDE_LOOP")
        .declares("x")
        .on_closest_hoist_scope();

    t.assert_type_of_expression("I_INSIDE_LOOP")
        .to_string_is_equal_to("string");
    t.assert_type_of_expression("I_AFTER_LOOP")
        .to_string_is_equal_to("(null|string)");

    t.assert_type_of_expression("X_AT_LOOP_START")
        .to_string_is_equal_to("(null|number)");
    t.assert_type_of_expression("X_AT_LOOP_END")
        .to_string_is_equal_to("number");
    t.assert_type_of_expression("X_AFTER_LOOP")
        .to_string_is_equal_to("(null|number)");
}

// port: TypeInferenceTest#testForInWithRedeclaredVar
#[test]
fn test_for_in_with_redeclared_var() {
    let t = TypeInferenceTest::set_up();
    t.assuming("y", OBJECT_TYPE);
    t.in_function(
        "var i = null;\nfor (var i in y) { // i redeclared here, but really the same variable\n  I_INSIDE_LOOP: i;\n}\nI_AFTER_LOOP: i;\n");
    t.assert_scope_enclosing(&mut t.c(), "I_INSIDE_LOOP")
        .declares("i")
        .on_closest_hoist_scope();
    t.assert_type_of_expression("I_INSIDE_LOOP")
        .to_string_is_equal_to("string");

    t.assert_scope_enclosing(&mut t.c(), "I_AFTER_LOOP")
        .declares("i")
        .directly();
    t.assert_type_of_expression("I_AFTER_LOOP")
        .to_string_is_equal_to("(null|string)");
}

// port: TypeInferenceTest#testForInWithLet
#[test]
fn test_for_in_with_let() {
    let t = TypeInferenceTest::set_up();
    t.assuming("y", OBJECT_TYPE);
    t.in_function(
        "FOR_IN_LOOP: for (let i in y) { // preserve newlines\n  I_INSIDE_LOOP: i;\n}\nAFTER_LOOP: 1;\n");
    t.assert_scope_enclosing(&mut t.c(), "I_INSIDE_LOOP")
        .declares("i")
        .on_scope_labeled("FOR_IN_LOOP");
    t.assert_type_of_expression("I_INSIDE_LOOP")
        .to_string_is_equal_to("string");

    t.assert_scope_enclosing(&mut t.c(), "AFTER_LOOP")
        .does_not_declare("i");
}

// port: TypeInferenceTest#testForInWithConst
#[test]
fn test_for_in_with_const() {
    let t = TypeInferenceTest::set_up();
    t.assuming("y", OBJECT_TYPE);
    t.in_function(
        "FOR_IN_LOOP: for (const i in y) { // preserve newlines\n  I_INSIDE_LOOP: i;\n}\nAFTER_LOOP: 1;\n");
    t.assert_scope_enclosing(&mut t.c(), "I_INSIDE_LOOP")
        .declares("i")
        .on_scope_labeled("FOR_IN_LOOP");
    t.assert_type_of_expression("I_INSIDE_LOOP")
        .to_string_is_equal_to("string");

    t.assert_scope_enclosing(&mut t.c(), "AFTER_LOOP")
        .does_not_declare("i");
}

// port: TypeInferenceTest#testNew1
#[test]
fn test_new1() {
    let t = TypeInferenceTest::set_up();
    t.assuming(
        "x",
        t.create_nullable_type(t.get_native_type(FUNCTION_TYPE)),
    );
    t.in_function("var y = new x();");
    t.verify("y", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testNew2
#[test]
fn test_new2() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @constructor\n * @param {T} x\n * @template T\n */\nfunction F(x) {}\nvar x = /** @type {!Array<number>} */ ([]);\nvar result = new F(x);\n");

    assert_eq!(t.get_type("result").to_string_t(&t), "F<Array<number>>");
}

// port: TypeInferenceTest#testNew3
#[test]
fn test_new3() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @constructor\n * @param {Array<T>} x\n * @param {T} y\n * @param {S} z\n * @template T,S\n */\nfunction F(x,y,z) {}\nvar x = /** @type {!Array<number>} */ ([]);\nvar y = /** @type {string} */ ('foo');\nvar z = /** @type {boolean} */ (true);\nvar result = new F(x,y,z);\n");

    assert_eq!(
        t.get_type("result").to_string_t(&t),
        "F<(number|string),boolean>"
    );
}

// port: TypeInferenceTest#testNew4
#[test]
fn test_new4() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @constructor\n * @param {!Array<T>} x\n * @param {T} y\n * @param {S} z\n * @param {U} m\n * @template T,S,U\n */\nfunction F(x,y,z,m) {}\nvar /** !Array<number> */ x = [];\nvar y = 'foo';\nvar z = true;\nvar m = 9;\nvar result = new F(x,y,z,m);\n");

    assert_eq!(
        t.get_type("result").to_string_t(&t),
        "F<(number|string),boolean,number>"
    );
}

// port: TypeInferenceTest#testNew_onCtor_instantiatingTemplatizedType_withNoTemplateInformation
#[test]
fn test_new_on_ctor_instantiating_templatized_type_with_no_template_information() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @constructor\n * @template T\n */\nfunction Foo() {}\n\nvar result = new Foo();\n",
    );

    assert_eq!(t.get_type("result").to_string_t(&t), "Foo<?>");
}

// port: TypeInferenceTest#testNew_onCtor_instantiatingTemplatizedType_specializedOnSecondaryTemplate
#[test]
fn test_new_on_ctor_instantiating_templatized_type_specialized_on_secondary_template() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @constructor\n * @template T\n */\nfunction Foo() {}\n\n/**\n * @template U\n * @param {function(new:Foo<U>)} ctor\n * @param {U} arg\n * @return {!Foo<U>}\n */\nfunction create(ctor, arg) {\n  return new ctor(arg);\n}\n\nvar result = create(Foo, 0);\n");

    assert_eq!(t.get_type("result").to_string_t(&t), "Foo<number>");
}

// port: TypeInferenceTest#testNewRest
#[test]
fn test_new_rest() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @constructor\n * @param {Array<T>} x\n * @param {T} y\n * @param {...S} rest\n * @template T,S\n */\nfunction F(x, y, ...rest) {}\nvar x = /** @type {!Array<number>} */ ([]);\nvar y = /** @type {string} */ ('foo');\nvar z = /** @type {boolean} */ (true);\nvar result = new F(x,y,z);\n");

    assert_eq!(
        t.get_type("result").to_string_t(&t),
        "F<(number|string),boolean>"
    );
}

// port: TypeInferenceTest#testParamNodeType_simpleName
#[test]
fn test_param_node_type_simple_name() {
    let t = TypeInferenceTest::set_up();
    t.parse_and_run_type_inference("(/** @param {number} i */ function(i) {})");

    t.assert_node_js_type(t.get_param_name_node("i"))
        .is_number();
}

// port: TypeInferenceTest#testParamNodeType_rest
#[test]
fn test_param_node_type_rest() {
    let t = TypeInferenceTest::set_up();
    t.parse_and_run_type_inference("(/** @param {...number} i */ function(...i) {})");

    t.assert_node_js_type(t.get_param_name_node("i"))
        .to_string_is_equal_to("Array<number>");
}

// port: TypeInferenceTest#testParamNodeType_arrayDestructuring
#[test]
fn test_param_node_type_array_destructuring() {
    let t = TypeInferenceTest::set_up();
    t.parse_and_run_type_inference("(/** @param {!Iterable<number>} i */ function([i]) {})");

    // TODO(nickreid): Also check the types of the other nodes in the PARAM_LIST tree.
    t.assert_node_js_type(t.get_param_name_node("i"))
        .is_number();
}

// port: TypeInferenceTest#testParamNodeType_objectDestructuring
#[test]
fn test_param_node_type_object_destructuring() {
    let t = TypeInferenceTest::set_up();
    t.parse_and_run_type_inference("(/** @param {{a: number}} i */ function({a: i}) {})");

    // TODO(nickreid): Also check the types of the other nodes in the PARAM_LIST tree.
    t.assert_node_js_type(t.get_param_name_node("i"))
        .is_number();
}

// port: TypeInferenceTest#testParamNodeType_simpleName_withDefault
#[test]
fn test_param_node_type_simple_name_with_default() {
    let t = TypeInferenceTest::set_up();
    t.parse_and_run_type_inference("(/** @param {number=} i */ function(i = 9) {})");

    // TODO(nickreid): Also check the types of the other nodes in the PARAM_LIST tree.
    t.assert_node_js_type(t.get_param_name_node("i"))
        .is_number();
}

// port: TypeInferenceTest#testParamNodeType_arrayDestructuring_withDefault
#[test]
fn test_param_node_type_array_destructuring_with_default() {
    let t = TypeInferenceTest::set_up();
    t.parse_and_run_type_inference(
        "(/** @param {!Iterable<number>=} unused */\nfunction([i] = /** @type ({!Array<number>} */ ([])) {})\n");

    // TODO(nickreid): Also check the types of the other nodes in the PARAM_LIST tree.
    // TODO(b/122904530): `i` should be `number`.
    t.assert_node_js_type(t.get_param_name_node("i"))
        .is_unknown();
}

// port: TypeInferenceTest#testParamNodeType_objectDestructuring_withDefault
#[test]
fn test_param_node_type_object_destructuring_with_default() {
    let t = TypeInferenceTest::set_up();
    t.parse_and_run_type_inference("(/** @param {{a: number}=} i */ function({a: i} = {a: 9}) {})");

    // TODO(nickreid): Also check the types of the other nodes in the PARAM_LIST tree.
    t.assert_node_js_type(t.get_param_name_node("i"))
        .is_number();
}

// port: TypeInferenceTest#testParamNodeType_arrayDestructuring_withDefault_nestedInPattern
#[test]
fn test_param_node_type_array_destructuring_with_default_nested_in_pattern() {
    let t = TypeInferenceTest::set_up();
    t.parse_and_run_type_inference("(/** @param {!Iterable<number>} i */ function([i = 9]) {})");

    // TODO(nickreid): Also check the types of the other nodes in the PARAM_LIST tree.
    t.assert_node_js_type(t.get_param_name_node("i"))
        .is_number();
}

// port: TypeInferenceTest#testParamNodeType_objectDestructuring_withDefault_nestedInPattern
#[test]
fn test_param_node_type_object_destructuring_with_default_nested_in_pattern() {
    let t = TypeInferenceTest::set_up();
    t.parse_and_run_type_inference("(/** @param {{a: number}} i */ function({a: i = 9}) {})");

    // TODO(nickreid): Also check the types of the other nodes in the PARAM_LIST tree.
    t.assert_node_js_type(t.get_param_name_node("i"))
        .is_number();
}

// port: TypeInferenceTest#testFunctionDeclarationHasBlockScope
#[test]
fn test_function_declaration_has_block_scope() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "BLOCK_SCOPE: {\n  BEFORE_DEFINITION: f;\n  function f() {}\n  AFTER_DEFINITION: f;\n}\nAFTER_BLOCK: f;\n");
    // A block-scoped function declaration is hoisted to the beginning of its block, so it is always
    // defined within the block.
    t.assert_scope_enclosing(&mut t.c(), "BEFORE_DEFINITION")
        .declares("f")
        .on_scope_labeled("BLOCK_SCOPE");
    t.assert_type_of_expression("BEFORE_DEFINITION")
        .to_string_is_equal_to("function(): undefined");
    t.assert_type_of_expression("AFTER_DEFINITION")
        .to_string_is_equal_to("function(): undefined");
    t.assert_scope_enclosing(&mut t.c(), "AFTER_BLOCK")
        .does_not_declare("f");
}

// port: TypeInferenceTest#testEnumRAI1
#[test]
fn test_enum_rai1() {
    let t = TypeInferenceTest::set_up();
    let enum_type = t
        .create_enum_type("MyEnum", ARRAY_TYPE)
        .get_elements_type_t(&t);
    t.assuming("x", enum_type);
    t.in_function("var y = null; if (x) y = x;");
    t.verify("y", t.create_nullable_type(enum_type));
}

// port: TypeInferenceTest#testEnumRAI2
#[test]
fn test_enum_rai2() {
    let t = TypeInferenceTest::set_up();
    let enum_type = t
        .create_enum_type("MyEnum", NUMBER_TYPE)
        .get_elements_type_t(&t);
    t.assuming("x", enum_type);
    t.in_function("var y = null; if (typeof x == 'number') y = x;");
    t.verify("y", t.create_nullable_type(enum_type));
}

// port: TypeInferenceTest#testEnumRAI3
#[test]
fn test_enum_rai3() {
    let t = TypeInferenceTest::set_up();
    let enum_type = t
        .create_enum_type("MyEnum", NUMBER_TYPE)
        .get_elements_type_t(&t);
    t.assuming("x", enum_type);
    t.in_function("var y = null; if (x && typeof x == 'number') y = x;");
    t.verify("y", t.create_nullable_type(enum_type));
}

// port: TypeInferenceTest#testEnumRAI4
#[test]
fn test_enum_rai4() {
    let t = TypeInferenceTest::set_up();
    let enum_type = t
        .create_enum_type("MyEnum", t.create_union_type(STRING_TYPE, NUMBER_TYPE))
        .get_elements_type_t(&t);
    t.assuming("x", enum_type);
    t.in_function("var y = null; if (typeof x == 'number') y = x;");
    t.verify("y", t.create_nullable_type(NUMBER_TYPE));
}

// port: TypeInferenceTest#testBackwardsInferenceCall
#[test]
fn test_backwards_inference_call() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @param {{foo: (number|undefined)}} x */\nfunction f(x) {}\nvar y = {};\nf(y);\n",
    );

    assert_eq!(t.get_type("y").to_string_t(&t), "{foo: (number|undefined)}");
}

// port: TypeInferenceTest#testBackwardsInferenceCallRestParameter
#[test]
fn test_backwards_inference_call_rest_parameter() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @param {...{foo: (number|undefined)}} rest */\nfunction f(...rest) {}\nvar y = {};\nf(y);\n");

    assert_eq!(t.get_type("y").to_string_t(&t), "{foo: (number|undefined)}");
}

// port: TypeInferenceTest#testBackwardsInferenceNew
#[test]
fn test_backwards_inference_new() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @constructor\n * @param {{foo: (number|undefined)}} x\n */\nfunction F(x) {}\nvar y = {};\nnew F(y);\n");

    assert_eq!(t.get_type("y").to_string_t(&t), "{foo: (number|undefined)}");
}

// port: TypeInferenceTest#testRecordInference
#[test]
fn test_record_inference() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @param {{a: boolean}|{b: string}} x */\nfunction f(x) {}\nvar out = {};\nf(out);\n",
    );
    assert_eq!(
        t.get_type("out").to_string_t(&t),
        "{\n  a: (boolean|undefined),\n  b: (string|undefined)\n}"
    );
}

// port: TypeInferenceTest#testIssue785
#[test]
fn test_issue785() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @param {string|{prop: (string|undefined)}} x */\nfunction f(x) {}\nvar out = {};\nf(out);\n");
    assert_eq!(
        t.get_type("out").to_string_t(&t),
        "{prop: (string|undefined)}"
    );
}

// port: TypeInferenceTest#testTypeTransformationNoneType
#[test]
fn test_type_transformation_none_type() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @return {R}\n * @template R := none() =:\n */\nfunction f(){}\nvar result = f(10);\n");
    t.verify("result", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testTypeTransformationObjectUseCase
#[test]
fn test_type_transformation_object_use_case() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** \n * @param {T} a\n * @return {R}\n * @template T \n * @template R := \n * mapunion(T, (x) => \n *      cond(eq(x, 'string'), 'String',\n *      cond(eq(x, 'number'), 'Number',\n *      cond(eq(x, 'boolean'), 'Boolean',\n *      cond(eq(x, 'null'), 'Object', \n *      cond(eq(x, 'undefined'), 'Object',\n *      x)))))) \n * =:\n */\nfunction Object(a) {}\n/** @type {(string|number|boolean)} */\nvar o;\nvar r = Object(o);\n");
    t.verify(
        "r",
        t.create_multi_param_union_type(&[
            STRING_OBJECT_TYPE,
            NUMBER_OBJECT_TYPE,
            BOOLEAN_OBJECT_TYPE,
        ]),
    );
}

// port: TypeInferenceTest#testTypeTransformationRecordFromObject
#[test]
fn test_type_transformation_record_from_object() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** \n * @param {T} a\n * @return {R}\n * @template T \n * @template R := record(T) =:\n */\nfunction f(a) {}\n/** @type {{foo:?}} */\nvar e;\n/** @type {?} */\nvar bar;\nvar r = f({foo:bar});\n");
    assert!(t.get_type("r").is_record_type_t(&t));
    t.verify("r", t.get_type("e"));
}

// port: TypeInferenceTest#testTypeTransformationRecordFromObjectNested
#[test]
fn test_type_transformation_record_from_object_nested() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** \n * @param {T} a\n * @return {R}\n * @template T \n * @template R :=\n * maprecord(record(T), (k, v) => record({[k]:record(v)})) =:\n */\nfunction f(a) {}\n/** @type {{foo:!Object, bar:!Object}} */\nvar e;\nvar r = f({foo:{}, bar:{}});\n");
    assert!(t.get_type("r").is_record_type_t(&t));
    t.verify("r", t.get_type("e"));
}

// port: TypeInferenceTest#testTypeTransformationRecordFromObjectWithTemplatizedType
#[test]
fn test_type_transformation_record_from_object_with_templatized_type() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** \n * @param {T} a\n * @return {R}\n * @template T \n * @template R := record(T) =:\n */\nfunction f(a) {}\n/** @type {{foo:!Array<number>}} */\nvar e;\n/** @type {!Array<number>} */\nvar something;\nvar r = f({foo:something});\n");
    assert!(t.get_type("r").is_record_type_t(&t));
    t.verify("r", t.get_type("e"));
}

// port: TypeInferenceTest#testTypeTransformationIsTemplatizedPartially
#[test]
fn test_type_transformation_is_templatized_partially() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @constructor\n * @template T, U\n */\nfunction Foo() {}\n/**\n * @template T := cond(isTemplatized(type('Foo', 'number')), 'number', 'string') =:\n * @return {T}\n */\nfunction f() { return 123; }\nvar x = f();\n");
    assert!(t.get_type("x").is_number_t(&t));
}

// port: TypeInferenceTest#testRestParamType
#[test]
fn test_rest_param_type() {
    let t = TypeInferenceTest::set_up();
    t.parse_and_run_type_inference(
        "(\n/** // preserve newlines\n * @param {...number} nums\n */\nfunction(str, ...nums) {\n  NUMS: nums;\n  let n = null;\n  N_START: n;\n  if (nums.length > 0) {\n    n = nums[0];\n    N_IF_TRUE: n;\n  } else {\n    N_IF_FALSE: n;\n  }\n  N_FINAL: n;\n}\n);\n");
    t.assert_type_of_expression("N_START")
        .to_string_is_equal_to("null");
    t.assert_type_of_expression("N_IF_TRUE")
        .to_string_is_equal_to("number");
    t.assert_type_of_expression("N_IF_FALSE")
        .to_string_is_equal_to("null");
    t.assert_type_of_expression("N_FINAL")
        .to_string_is_equal_to("(null|number)");
}

// port: TypeInferenceTest#testObjectDestructuringParameterWithDefaults
#[test]
fn test_object_destructuring_parameter_with_defaults() {
    let t = TypeInferenceTest::set_up();
    t.parse_and_run_type_inference(
        "(/** @param {{x: (number|undefined)}} data */ function f({x = 3}) { X: x; });",
    );

    t.assert_type_of_expression("X")
        .to_string_is_equal_to("number");
}

// port: TypeInferenceTest#testDynamicImport
#[test]
fn test_dynamic_import() {
    let t = TypeInferenceTest::set_up();
    t.in_script("const foo = import('foo');");

    let promise_of_unknown_type = t.templatize(
        t.get_native_object_type(PROMISE_TYPE),
        &[t.get_native_type(UNKNOWN_TYPE)],
    );
    t.assert_type(Some(t.get_type("foo")))
        .is_subtype_of(promise_of_unknown_type);
}
// port: TypeInferenceTest#testGetElemDereference_knownSymbol
#[test]
fn test_get_elem_dereference_known_symbol() {
    let t = TypeInferenceTest::set_up();
    let array = t.get_native_object_type(ARRAY_TYPE);
    let sym = t.new_known_symbol_type("sym");
    array.define_declared_property_t(
        &t,
        SymbolKey::new(sym),
        t.get_native_type(STRING_TYPE),
        None,
    );
    t.assuming("o", array);
    t.assuming("sym", sym);

    t.in_function("const x = o[sym];");
    t.verify("x", STRING_TYPE);
}

// port: TypeInferenceTest#testGetElemDereference_knownSymbol_nullable
#[test]
fn test_get_elem_dereference_known_symbol_nullable() {
    let t = TypeInferenceTest::set_up();
    let array = t.get_native_object_type(ARRAY_TYPE);
    let sym = t.new_known_symbol_type("sym");
    array.define_declared_property_t(
        &t,
        SymbolKey::new(sym),
        t.get_native_type(STRING_TYPE),
        None,
    );
    t.assuming("o", t.create_nullable_type(array));
    t.assuming("sym", sym);

    t.in_function("const x = o[sym];");
    t.verify("x", STRING_TYPE);
}

// port: TypeInferenceTest#testGetElemDereference_knownSymbol_union
#[test]
fn test_get_elem_dereference_known_symbol_union() {
    let t = TypeInferenceTest::set_up();
    let array = t.get_native_object_type(ARRAY_TYPE);
    let promise = t.get_native_object_type(PROMISE_TYPE);
    let sym = t.new_known_symbol_type("sym");
    array.define_declared_property_t(
        &t,
        SymbolKey::new(sym),
        t.get_native_type(STRING_TYPE),
        None,
    );
    promise.define_declared_property_t(
        &t,
        SymbolKey::new(sym),
        t.get_native_type(NUMBER_TYPE),
        None,
    );
    t.assuming("o", union!(t, array, promise));
    t.assuming("sym", sym);

    t.in_function("const x = o[sym];");
    t.verify(
        "x",
        union!(
            t,
            t.get_native_type(STRING_TYPE),
            t.get_native_type(NUMBER_TYPE)
        ),
    );
}

// port: TypeInferenceTest#testGetElemDereference_knownSymbol_union_notOnAllAlternates
#[test]
fn test_get_elem_dereference_known_symbol_union_not_on_all_alternates() {
    let t = TypeInferenceTest::set_up();
    let array = t.get_native_object_type(ARRAY_TYPE);
    let promise = t.get_native_object_type(PROMISE_TYPE);
    let sym = t.new_known_symbol_type("sym");
    array.define_declared_property_t(
        &t,
        SymbolKey::new(sym),
        t.get_native_type(STRING_TYPE),
        None,
    );
    // don't define sym on promise
    t.assuming("o", union!(t, array, promise));
    t.assuming("sym", sym);

    t.in_function("const x = o[sym];");
    t.verify("x", UNKNOWN_TYPE);
}

// port: TypeInferenceTest#testObjectSpread_isInferredToBeObject
#[test]
fn test_object_spread_is_inferred_to_be_object() {
    let t = TypeInferenceTest::set_up();
    // Given
    let record_type = t.registry_create_record_type(&[
        ("x", t.get_native_type(STRING_TYPE)),
        ("y", t.get_native_type(NUMBER_TYPE)),
    ]);
    t.assuming("obj", record_type);

    t.assuming("before", BOOLEAN_TYPE);
    t.assuming("after", NULL_TYPE);

    // When
    t.in_function("let spread = {before, ...obj, after};");

    // Then

    // TODO(b/128355893): Do smarter inferrence. There are a lot of potential issues with
    // inference on object-rest, so for now we just give up and say `Object`. In theory we could
    // infer something like `{after: null, before: boolean, x: string, y: number}`.
    t.verify("spread", OBJECT_TYPE);
}

// port: TypeInferenceTest#testCast2
#[test]
fn test_cast2() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/** @return {boolean} */\nObject.prototype.method = function() { return true; };\nvar x = /** @type {Object} */ (this).method;\n",
    );
    let object_type = t.get_native_object_type(OBJECT_TYPE);
    let boolean_type = t.get_native_type(BOOLEAN_TYPE);
    let expected = {
        let mut compiler = t.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        reg.create_function_type_with_instance_type(
            ast,
            object_type,
            boolean_type,
            &[], /* params */
        )
    };
    t.verify("x", expected);
}

// port: TypeInferenceTest#testLotsOfBranchesGettingMerged
#[test]
fn test_lots_of_branches_getting_merged() {
    let t = TypeInferenceTest::set_up();
    let mut code = String::from("var a = -1;\n");
    code += "switch(foo()) { \n";
    for i in 0..100 {
        code += &format!("case {i}: a = {i}; break; \n");
    }
    code += "default: a = undefined; break;\n";
    code += "}\n";
    t.in_function(&code);
    assert_eq!(t.get_type("a").to_string_t(&t), "(number|undefined)");
}

// port: TypeInferenceTest#testFunctionTemplateType_specializedFunctionType_copiesColorIdCompnents
#[test]
fn test_function_template_type_specialized_function_type_copies_color_id_compnents() {
    let t = TypeInferenceTest::set_up();
    t.in_function(
        "/**\n * @template T\n * @param {T} a\n * @return {T}\n */\nfunction f(a) {}\nTEMPLATE: f;\n\nSPECIALIZED: f(10);\n",
    );

    let template_node = t.get_labeled_statement("TEMPLATE").statement_node;
    let specialized_node = t.get_labeled_statement("SPECIALIZED").statement_node;
    let mut compiler = t.c();
    let template_type = template_node
        .get_first_child(&compiler)
        .unwrap()
        .get_jstype(&compiler)
        .unwrap();
    let specialized_type = specialized_node
        .get_first_first_child(&compiler)
        .unwrap()
        .get_jstype(&compiler)
        .unwrap();
    let reg = compiler.get_type_registry();
    let template_fn = template_type.to_maybe_function_type(reg).unwrap();
    let specialized_fn = specialized_type.to_maybe_function_type(reg).unwrap();

    assert_ne!(specialized_fn, template_fn);
    assert_eq!(
        ObjectType::get_reference_name(specialized_fn, reg),
        ObjectType::get_reference_name(template_fn, reg)
    );
    assert_eq!(
        FunctionType::get_source(specialized_fn, reg),
        FunctionType::get_source(template_fn, reg)
    );
    assert_eq!(
        FunctionType::get_goog_module_id(specialized_fn, reg),
        FunctionType::get_goog_module_id(template_fn, reg)
    );
}

// port: TypeInferenceTest#testObjectDestructuringDeclarationInference
#[test]
fn test_object_destructuring_declaration_inference() {
    let t = TypeInferenceTest::set_up();
    let record_type = t.registry_create_record_type(&[
        ("x", t.get_native_type(STRING_TYPE)),
        ("y", t.get_native_type(NUMBER_TYPE)),
    ]);
    t.assuming("obj", record_type);

    t.in_function("let {x, y} = obj;  // preserve newline\nX: x;\nY: y;\n");
    t.assert_type_of_expression("X")
        .to_string_is_equal_to("string");
    t.assert_type_of_expression("Y")
        .to_string_is_equal_to("number");

    let type_subject = t
        .assert_scope_enclosing(&mut t.c(), "X")
        .declares("x")
        .with_type_that();
    t.bind(type_subject).to_string_is_equal_to("string");
}

// port: TypeInferenceTest#testObjectDestructuringDeclarationInferenceWithUnknownProperty
#[test]
fn test_object_destructuring_declaration_inference_with_unknown_property() {
    let t = TypeInferenceTest::set_up();
    let record_type = t.registry_create_record_type(&[]);
    t.assuming("obj", record_type);

    t.in_function("let {x} = obj;  // preserve newline\nX: x;\n");
    t.assert_type_of_expression("X").to_string_is_equal_to("?");
}

// port: TypeInferenceTest#testObjectLiteralNoSideEffect
#[test]
fn test_object_literal_no_side_effect() {
    let t = TypeInferenceTest::set_up();
    // Repro for b/260837012.
    t.in_function(
        "  for (let x = 0; x < 3; x++) {\n    obj = {\n      data: {tipsMetadata: ''},\n      ...{}\n    };\n  }\n",
    );
    let object_type = t.get_native_object_type(OBJECT_TYPE);
    {
        let mut compiler = t.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        assert!(
            !object_type.has_own_property(reg, ast, "data"),
            "Type inference must not alter OBJECT_TYPE."
        );
    }
    // Check that the inferred type of 'obj' has a property 'data' associated with it.
    let obj_type = t.get_type("obj"); // asserts non-null: "Cannot resolve type of 'obj'"
    let mut compiler = t.c();
    let (reg, ast) = compiler.get_type_registry_and_ast();
    assert_ne!(
        reg.can_property_be_defined(ast, obj_type, "data"),
        PropDefinitionKind::UNKNOWN,
        "Expect property 'data' to be defined on 'obj'"
    );
}

// port: TypeInferenceTest#testInferringTypeInObjectPattern_fromTemplatizedProperty
#[test]
fn test_inferring_type_in_object_pattern_from_templatized_property() {
    let t = TypeInferenceTest::set_up();
    // create type Foo with one property templatized with type T
    let template_key = t.registry_create_template_type("T");
    let foo_ctor = {
        let mut compiler = t.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let parameters = reg.create_parameters(&[]);
        reg.create_constructor_type(
            ast,
            Some(JsString::from("Foo")),
            None,
            Some(parameters),
            None,
            Some(vec![template_key]),
            false,
        )
    };
    let foo_instance_type =
        FunctionType::get_instance_type(foo_ctor, t.c().get_type_registry()).unwrap();
    foo_instance_type.define_declared_property_t(&t, "data", template_key, None);

    // create a variable obj with type Foo<number>
    let foo_of_number = t.templatize(foo_instance_type, &[t.get_native_type(NUMBER_TYPE)]);
    t.assuming("obj", foo_of_number);
    t.in_function("const {data} = obj;\nOBJ: obj;\nDATA: data\n");

    t.assert_type_of_expression("OBJ")
        .to_string_is_equal_to("Foo<number>");
    t.assert_type_of_expression("DATA")
        .to_string_is_equal_to("number");
}

// port: TypeInferenceTest#constDeclarationWithReturnJSDoc_ignoresUnknownRhsType
#[test]
fn const_declaration_with_return_js_doc_ignores_unknown_rhs_type() {
    let t = TypeInferenceTest::set_up();
    t.assuming("foo", UNKNOWN_TYPE);

    t.in_function("/** @return {number} */ const fn = foo;");

    let foo_with_interface_type = t.get_type("fn");
    let mut compiler = t.c();
    let reg = compiler.get_type_registry();
    TypeSubject::assert_type(foo_with_interface_type)
        .is_function_type_that(reg)
        .has_return_type_that(reg)
        .is_number(reg);
}

// port: TypeInferenceTest#constDeclarationWithCtorJSDoc_ignoresKnownMixinReturnType
#[test]
fn const_declaration_with_ctor_js_doc_ignores_known_mixin_return_type() {
    let t = TypeInferenceTest::set_up();
    // Create a function always returning a constructor for 'Foo'
    let foo_type = {
        let mut compiler = t.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        function_type::builder()
            .for_constructor()
            .with_name("Foo")
            .build(reg, ast)
    };
    t.assuming("Foo", foo_type);
    let mixin_type = {
        let mut compiler = t.c();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        function_type::builder()
            .with_return_type(foo_type)
            .build(reg, ast)
    };
    t.assuming("mixin", mixin_type);

    // The @constructor JSDoc should declare a new type, and FooExtended should refer to that
    // type instead of the constructor for Foo
    t.in_function("/** @constructor @extends {Foo} */ const FooExtended = mixin();");

    let foo_with_interface_type = t.get_type("FooExtended");
    t.assert_type(Some(foo_with_interface_type))
        .is_not_equal_to(foo_type);
    t.assert_type(Some(foo_with_interface_type))
        .to_string_is_equal_to("function(new:FooExtended): ?");
}
// port: TypeInferenceTest#testThisTypeAfterMemberField
#[test]
fn test_this_type_after_member_field() {
    let t = TypeInferenceTest::set_up();
    let this_type = t.create_record_type("x", STRING_TYPE);
    t.assuming_this_type(this_type);
    t.in_function(
        "let thisDotX;\n(class C {\n  /** @type {number} */\n  static x = 0;\n}, thisDotX = this.x);\n");
    t.verify("thisDotX", STRING_TYPE);
}

// port: TypeInferenceTest#testSuper
#[test]
fn test_super() {
    let t = TypeInferenceTest::set_up();
    // does not infer super
    t.in_script(
        "class Foo {\n  static str;\n}\nclass Bar extends Foo {\n  static {\n    super.str = 'hi';\n  }\n}\nlet x = Bar.str;\n");
    t.verify("x", ALL_TYPE);
}

// port: TypeInferenceTest#testComputedClassFieldsInControlFlow
#[test]
fn test_computed_class_fields_in_control_flow() {
    let t = TypeInferenceTest::set_up();
    // Based on the class semantics, the static RHS expressions only execute after all of the
    // computed properties, so `y` will get the string value rather than the boolean here.
    t.in_function(
        "let y;\nclass Foo {\n  static [y = null] = (y = '');\n  [y = false] = [y = null];\n}\n",
    );
    t.verify("y", STRING_TYPE);
}

// port: TypeInferenceTest#testClassFieldsInControlFlow
#[test]
fn test_class_fields_in_control_flow() {
    let t = TypeInferenceTest::set_up();
    t.in_function("let y;\nclass Foo {\n  static y = (y = '');\n  z = [y = null];\n}\n");
    t.verify("y", STRING_TYPE);
}
