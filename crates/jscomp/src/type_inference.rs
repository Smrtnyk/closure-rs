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
//   src/com/google/javascript/jscomp/LinkedFlowScope.java,
//   src/com/google/javascript/jscomp/TypeInference.java.

//! Type inference within a script node or a function body, using the data-flow analysis framework.
//!
//! Java's `TypeInference extends DataFlowAnalysis<Node, FlowScope>`. The Java fields `compiler`
//! and `registry` are not stored (DESIGN §6): every method that used them takes the compiler,
//! which owns the registry. The lattice is `Arc<dyn FlowScope>` (Java's `FlowScope`), compared by
//! identity where Java compares with `==`.
use crate::{
    abstract_compiler::AbstractCompiler,
    closure_coding_convention::ClosureCodingConvention,
    coding_convention::{AssertionFunctionLookup, AssertionKind, CodingConvention},
    control_flow_graph::{Branch, ControlFlowGraph},
    data_flow_analysis::{
        DataFlowAnalysis, DataFlowAnalysisState, FlowBrancher, FlowJoiner, LatticeEquals,
    },
    destructured_target::DestructuredTarget,
    diagnostic_type::DiagnosticType,
    flow_scope::{FlowScope, FlowSlot},
    graph::{annotation::Annotation, graph_node::GraphNode},
    invocation_template_type_matcher::InvocationTemplateTypeMatcher,
    js_error::JSError,
    js_iterables::JsIterables,
    linked_flow_scope::{FlowScopeJoinOp, LinkedFlowScope},
    module_import_resolver::ModuleImportResolver,
    modules::{export::Export, module::Module, module_metadata_map::ModuleType},
    node_util::NodeUtil,
    promises::Promises,
    reverse_abstract_interpreter::ReverseAbstractInterpreter,
    type_check::TypeCheck,
    type_transformation::TypeTransformation,
    typed_scope::TypedScope,
    typed_scope_creator::TypedScopeCreator,
    typed_var::TypedVar,
};
use closure_jstype::{
    JSTypeNative, JSTypeRegistry, TypeId,
    boolean_literal_set::BooleanLiteralSet,
    js_type_class::JSTypeClass,
    object_type,
    prelude::{EnumElementType, FunctionType, JSType, ObjectType, TemplateType, UnionType},
    property::PropertyKey,
    rhino::js_type_expression::JSTypeExpressionExt,
    template_type_replacer::TemplateTypeReplacer,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    js_string::JsString,
    node::{Ast, NodeId, Prop},
    outcome::Outcome,
    qualified_name::QualifiedName,
    token::Token,
};
use std::{
    any::Any,
    collections::VecDeque,
    sync::{Arc, LazyLock},
};

/// Java's `FlowScope` value.
type FS = Arc<dyn FlowScope>;
/// The lattice element of TypeInference (Java's `FlowScope`).
pub type FlowScopeLattice = FS;

// port: TypeInference#REASSIGN_CLASS_PROTOTYPE
pub static REASSIGN_CLASS_PROTOTYPE: LazyLock<DiagnosticType> = LazyLock::new(|| {
    DiagnosticType::error(
        "JSC_REASSIGN_CLASS_PROTOTYPE",
        "Reassigning a class prototype is not allowed",
    )
});

// port: TypeInference#GOOG_REQUIREDYNAMIC_NAME
const GOOG_REQUIREDYNAMIC_NAME: &str = "goog.requireDynamic";

/// Rust-only: Java's `==` on FlowScope objects (data pointer identity).
fn same_scope(a: &FS, b: &FS) -> bool {
    std::ptr::eq(
        Arc::as_ptr(a) as *const () as *const u8,
        Arc::as_ptr(b) as *const () as *const u8,
    )
}

/// Rust-only: `LinkedFlowScope#equals` for the DataFlowAnalysis lattice comparison.
impl LatticeEquals for FS {
    fn lattice_equals(&self, compiler: &mut AbstractCompiler, other: &Self) -> bool {
        let linked: Arc<LinkedFlowScope> = Arc::clone(self)
            .as_any_arc()
            .downcast::<LinkedFlowScope>()
            .unwrap_or_else(|_| panic!("ClassCastException"));
        linked.equals(compiler, other.as_ref())
    }
}

/// Rust-only: the lattice stored as a CFG annotation.
impl Annotation for FS {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    // port: LinkedFlowScope#hashCode
    fn hash_code(&self) -> i32 {
        panic!("UnsupportedOperationException")
    }
    fn to_string(&self) -> String {
        format!("com.google.javascript.jscomp.LinkedFlowScope@{:x}", 0)
    }
}

/// Rust-only: `LinkedFlowScope.FlowScopeJoinOp` as cfg's `FlowJoiner`.
impl FlowJoiner<FS> for FlowScopeJoinOp {
    fn join_flow(&mut self, compiler: &mut AbstractCompiler, input: FS) {
        FlowScopeJoinOp::join_flow(self, compiler, input)
    }
    fn finish(self: Box<Self>) -> FS {
        check_not_null!(FlowScopeJoinOp::finish(*self))
    }
}

// port: TypeInference.AssignmentType
/// Either a combined declaration/initialization or a regular assignment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssignmentType {
    DECLARATION, // var x = 3;
    ASSIGN,      // `a.b.c = d;` or `x = 4;`
}

// port: TypeInference.TypeDeclaringCallback
/// Abstracts logic for declaring an lvalue in a particular scope
trait TypeDeclaringCallback {
    // port: TypeInference.TypeDeclaringCallback#declareTypeInScope
    /// Updates the given scope upon seeing an assignment or declaration
    ///
    /// `scope` the scope we are in, `lvalue` the value being updated, a NAME, GETPROP, GETELEM, or
    /// CAST, `type_` the type we've inferred for the lvalue. Returns the updated flow scope.
    fn declare_type_in_scope(
        &mut self,
        ti: &mut TypeInference<'_>,
        compiler: &mut AbstractCompiler,
        scope: FS,
        lvalue: NodeId,
        type_: Option<TypeId>,
    ) -> FS;
}
impl<F> TypeDeclaringCallback for F
where
    F: FnMut(&mut TypeInference<'_>, &mut AbstractCompiler, FS, NodeId, Option<TypeId>) -> FS,
{
    fn declare_type_in_scope(
        &mut self,
        ti: &mut TypeInference<'_>,
        compiler: &mut AbstractCompiler,
        scope: FS,
        lvalue: NodeId,
        type_: Option<TypeId>,
    ) -> FS {
        self(ti, compiler, scope, lvalue, type_)
    }
}

pub struct TypeInference<'a> {
    state: DataFlowAnalysisState<NodeId>,
    reverse_interpreter: Arc<dyn ReverseAbstractInterpreter>,
    bottom_scope: FS,
    container_scope: TypedScope, // global scope, function scope, or static block scope
    scope_creator: &'a mut TypedScopeCreator,
    assertion_function_lookup: &'a AssertionFunctionLookup,
    module_import_resolver: ModuleImportResolver,
    // A record is pushed onto this stack during the traversal of each optional chain.
    // Inference of the type at the end of the optional chain requires scope information
    // from traversing the start of the chain. This stack serves as a communication channel
    // for that information.
    opt_chain_array_deque: VecDeque<OptChainInfo>,

    // Scopes that have had their unbound untyped vars inferred as undefined.
    inferred_unbound_vars: IndexSet<TypedScope>,

    // For convenience
    unknown_type: TypeId,
    number_addition_supertype: TypeId,
}

impl<'a> TypeInference<'a> {
    // port: TypeInference#TypeInference
    pub fn new(
        compiler: &mut AbstractCompiler,
        cfg: ControlFlowGraph<NodeId>,
        reverse_interpreter: Arc<dyn ReverseAbstractInterpreter>,
        syntactic_scope: TypedScope,
        scope_creator: &'a mut TypedScopeCreator,
        assertion_function_lookup: &'a AssertionFunctionLookup,
    ) -> Self {
        let entry_value = *cfg.get_entry().get_value(&cfg);
        check_argument!(
            Some(syntactic_scope.get_root_node(compiler)) == entry_value,
            "Expected syntactic scope to be rooted at CFG root node of %s, but instead got syntactic scope root node of %s",
            cfg.format_node_value(compiler, entry_value.as_ref()),
            syntactic_scope.get_root_node(compiler).to_string(compiler)
        );
        let state = DataFlowAnalysisState::new(cfg, true, true);
        let module_import_resolver = ModuleImportResolver::new(compiler.get_module_map().cloned());
        let (reg, _) = compiler.get_type_registry_and_ast();
        let unknown_type = reg.get_native_object_type(JSTypeNative::UNKNOWN_TYPE);
        let number_addition_supertype =
            reg.get_native_type(JSTypeNative::NUMBER_ADDITION_SUPERTYPE);

        let root = syntactic_scope.get_root_node(compiler);
        let bottom = TypedScope::create_lattice_bottom(compiler, root);
        let bottom_scope: FS = LinkedFlowScope::create_entry_lattice(bottom);
        Self {
            state,
            reverse_interpreter,
            bottom_scope,
            container_scope: syntactic_scope,
            scope_creator,
            assertion_function_lookup,
            module_import_resolver,
            opt_chain_array_deque: VecDeque::new(),
            inferred_unbound_vars: IndexSet::<_>::default(),
            unknown_type,
            number_addition_supertype,
        }
    }

    // port: TypeInference#inferDeclarativelyUnboundVarsWithoutTypes
    fn infer_declaratively_unbound_vars_without_types(
        &mut self,
        compiler: &mut AbstractCompiler,
        mut flow: FS,
    ) -> FS {
        let scope = flow.get_declaration_scope(compiler);
        if !self.inferred_unbound_vars.insert(scope) {
            return flow;
        }
        // For each local variable declared with the VAR keyword, the entry
        // type is VOID.
        for var in scope.get_declaratively_unbound_vars_without_types(compiler) {
            if self.is_unflowable(compiler, var) {
                continue;
            }

            let name = var.get_name(compiler);
            let void_type = self.get_native_type(compiler, JSTypeNative::VOID_TYPE);
            flow = flow.infer_slot_type(compiler, &name, Some(void_type));
        }
        flow
    }

    // port: TypeInference#inferParameters
    /// Infers all of a function's parameters if their types aren't declared.
    fn infer_parameters(
        &mut self,
        compiler: &mut AbstractCompiler,
        mut entry_flow_scope: FS,
    ) -> FS {
        let function_node = self.container_scope.get_root_node(compiler);
        if !function_node.is_function(compiler) {
            return entry_flow_scope; // we're in the global scope
        } else if NodeUtil::is_bundled_goog_module_call(
            compiler,
            function_node.get_parent(compiler).unwrap(),
        ) {
            // Pretend the function literal in `goog.loadModule(function(exports) {` does not exist.
            return entry_flow_scope;
        }
        let ast_parameters = function_node.get_second_child(compiler).unwrap();
        let mut iife_argument_node: Option<NodeId> = None;

        if NodeUtil::is_invocation_target(compiler, function_node) {
            iife_argument_node = function_node.get_next(compiler);
        }

        let function_type = {
            let fn_jstype = function_node.get_jstype(compiler);
            let reg = compiler.get_type_registry();
            closure_jstype::js_type::to_maybe_function_type(reg, fn_jstype)
        };
        let parameter_types = {
            let reg = compiler.get_type_registry();
            function_type.unwrap().get_parameters(reg)
        };
        let mut parameter_types = parameter_types.into_iter();
        let mut parameter = parameter_types.next();

        // This really iterates over three different things at once:
        //   - the actual AST parameter nodes (which may be REST, DEFAULT_VALUE, etc.)
        //   - the argument nodes in an IIFE
        //   - the parameter type nodes from the FunctionType on the FUNCTION node
        // Always visit every AST parameter once, regardless of how many IIFE arguments or
        // FunctionType param nodes there are.
        let mut ast_param_itr = ast_parameters.get_first_child(compiler);
        while let Some(ast_param_cur) = ast_param_itr {
            let mut ast_param = ast_param_cur;
            if iife_argument_node.is_some_and(|n| n.is_spread(compiler)) {
                // block inference on all parameters that might possibly be set by a spread, e.g.
                // `z` in (function f(x, y, z = 1))(...[1, 2], 'foo')
                iife_argument_node = None;
            }

            // Running variable for the type of the param within the body of the function. We use
            // the existing type on the param node as the default, and then transform it according
            // to the declaration syntax.
            let mut inferred_type = self.get_js_type(compiler, ast_param);

            if let Some(iife_argument) = iife_argument_node {
                if let Some(t) = iife_argument.get_jstype(compiler) {
                    inferred_type = t;
                }
            } else if let Some(parameter) = &parameter {
                inferred_type = parameter.get_jstype();
            }

            let mut default_value: Option<NodeId> = None;
            if ast_param.is_default_value(compiler) {
                let dv = ast_param.get_second_child(compiler).unwrap();
                default_value = Some(dv);
                // must call `traverse` to correctly type the default value
                entry_flow_scope = self.traverse(compiler, dv, entry_flow_scope);
                ast_param = ast_param.get_first_child(compiler).unwrap();
            } else if ast_param.is_rest(compiler) {
                // e.g. `function f(p1, ...restParamName) {}`
                // set astParam = restParamName
                ast_param = ast_param.get_only_child(compiler);
                // convert 'number' into 'Array<number>' for rest parameters
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let array_type = reg.get_native_object_type(JSTypeNative::ARRAY_TYPE);
                inferred_type = reg.create_templatized_type(ast, array_type, &[inferred_type]);
            }

            if let Some(dv) = default_value {
                // The param could possibly be the default type, and `undefined` args won't
                // propagate in.
                let default_type = self.get_js_type(compiler, dv);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let not_undefined = inferred_type.restrict_by_not_undefined(reg, ast);
                inferred_type = reg.create_union_type(ast, &[not_undefined, default_type]);
            }

            if ast_param.is_destructuring_pattern(compiler) {
                // even if the inferredType is null, we still need to type all the nodes inside the
                // destructuring pattern. (e.g. in computed properties or default value
                // expressions)
                entry_flow_scope = self.update_destructuring_parameter(
                    compiler,
                    ast_param,
                    Some(inferred_type),
                    entry_flow_scope,
                );
            } else {
                // for simple named parameters, we only need to update the scope/AST if we have a
                // new inferred type.
                entry_flow_scope = self.update_named_parameter(
                    compiler,
                    ast_param,
                    default_value.is_some(),
                    Some(inferred_type),
                    entry_flow_scope,
                );
            }

            parameter = parameter_types.next();
            iife_argument_node = iife_argument_node.and_then(|n| n.get_next(compiler));
            ast_param_itr = ast_param_cur.get_next(compiler);
        }

        entry_flow_scope
    }

    // port: TypeInference#updateDestructuringParameter
    /// Sets the types of a un-named/destructuring function parameter to an inferred type.
    ///
    /// This method is responsible for typing the scope slot and the pattern nodes.
    fn update_destructuring_parameter(
        &mut self,
        compiler: &mut AbstractCompiler,
        pattern: NodeId,
        inferred_type: Option<TypeId>,
        mut entry_flow_scope: FS,
    ) -> FS {
        // look through all expressions and lvalues in the pattern.
        // given an lvalue, change its type if either a) it's inferred (not declared in
        // TypedScopeCreator) or b) it has a default value
        entry_flow_scope = self.traverse_destructuring_pattern_helper(
            compiler,
            pattern,
            entry_flow_scope,
            inferred_type,
            &mut |ti: &mut TypeInference<'_>,
                  compiler: &mut AbstractCompiler,
                  mut scope: FS,
                  lvalue: NodeId,
                  type_: Option<TypeId>| {
                let name = lvalue.get_string(compiler);
                let var = ti.container_scope.get_var(compiler, name);
                let var = check_not_null!(var);
                // This condition will trigger on cases like
                //   (function f({x}) {})({x: 3})
                // where `x` is of unknown type during the typed scope creation phase, but
                // here we can infer that it is of type `number`
                if var.is_type_inferred(compiler) {
                    var.set_type(compiler, type_);
                    lvalue.set_jstype(compiler, type_);
                }
                if lvalue
                    .get_parent(compiler)
                    .unwrap()
                    .is_default_value(compiler)
                {
                    // Given
                    //   /** @param {{age: (number|undefined)}} data */
                    //   function f({age = 99}) {}
                    // infer that `age` is now a `number` and not `number|undefined`
                    // treat this similarly to if there was an assignment inside the function body
                    // TODO(b/117162687): allow people to narrow the declared type to
                    // exclude 'undefined' inside the function body.
                    scope = ti.update_scope_for_assignment(
                        compiler,
                        scope,
                        lvalue,
                        type_,
                        AssignmentType::ASSIGN,
                    );
                }
                scope
            },
        );

        entry_flow_scope
    }

    // port: TypeInference#updateNamedParameter
    /// Sets the types of a named/non-destructuring function parameter to an inferred type.
    ///
    /// This method is responsible for typing the scope slot and the param node.
    fn update_named_parameter(
        &mut self,
        compiler: &mut AbstractCompiler,
        param_name: NodeId,
        has_default_value: bool,
        inferred_type: Option<TypeId>,
        mut entry_flow_scope: FS,
    ) -> FS {
        let name = param_name.get_string(compiler);
        let var = self.container_scope.get_var(compiler, name);
        let var = check_not_null!(
            var,
            "Missing var for parameter %s",
            param_name.to_string(compiler)
        );

        param_name.set_jstype(compiler, inferred_type);

        if var.is_type_inferred(compiler) {
            var.set_type(compiler, inferred_type);
        } else if has_default_value {
            // If this is a declared type with a default value, update the LinkedFlowScope slots
            // but not the actual TypedVar. This is similar to what would happen if the default
            // value was moved into an assignment in the fn body
            entry_flow_scope =
                self.redeclare_simple_var(compiler, entry_flow_scope, param_name, inferred_type);
        }
        entry_flow_scope
    }

    // port: TypeInference#updateModuleScope
    /// Updates the given scope after running inference over a goog.module or ES module
    fn update_module_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        module: Option<Arc<Module>>,
        syntactic_block_scope: TypedScope,
    ) {
        let Some(module) = module else {
            return;
        };
        match module.metadata().module_type() {
            ModuleType::ES6_MODULE => {
                // This call only affects exports with an inferred, not declared, type. Declared
                // exports were already added to the namespace object type in TypedScopeCreator.
                let namespace_var = syntactic_block_scope
                    .get_var(compiler, Export::NAMESPACE)
                    .unwrap();
                let namespace_type = namespace_var.get_type(compiler).unwrap();
                let namespace = {
                    let reg = compiler.get_type_registry();
                    namespace_type.to_object_type(reg).unwrap()
                };
                let scope_creator: &TypedScopeCreator = self.scope_creator;
                let mapper = scope_creator.get_node_to_scope_mapper();
                self.module_import_resolver.update_es_module_namespace_type(
                    compiler,
                    &mapper,
                    namespace,
                    &module,
                    syntactic_block_scope,
                );
            }
            ModuleType::GOOG_MODULE | ModuleType::LEGACY_GOOG_MODULE => {
                let exports_var = syntactic_block_scope.get_var(compiler, "exports");
                let exports_var = check_not_null!(
                    exports_var,
                    "Missing exports var for %s",
                    // Java's AutoValue toString; only reached on a crash.
                    format!("{:?}", module.metadata())
                );
                let exports_type = exports_var.get_type(compiler).unwrap_or(self.unknown_type);
                // Store the type of the namespace on the AST for the convenience of later passes
                // that want to access it.
                let root_node = syntactic_block_scope.get_root_node(compiler);
                if root_node.is_module_body(compiler) {
                    root_node.set_jstype(compiler, Some(exports_type));
                } else {
                    // For goog.loadModule, give the `exports` parameter the correct type.
                    check_state!(
                        root_node.is_block(compiler),
                        "%s",
                        root_node.to_string(compiler)
                    );
                    let param_list = NodeUtil::get_function_parameters(
                        compiler,
                        root_node.get_parent(compiler).unwrap(),
                    );
                    param_list
                        .get_only_child(compiler)
                        .set_jstype(compiler, Some(exports_type));
                }
                if !module.metadata().is_legacy_goog_module()
                    || exports_var.get_type(compiler).is_none()
                {
                    return;
                }

                // Update the global scope for the implicit assignment "legacy.module.id =
                // exports;" created by `goog.module.declareLegacyNamespace();`
                let module_id = module.closure_namespace().unwrap().clone();
                let global_scope = syntactic_block_scope.get_global_scope(compiler);
                let global_var = global_scope.get_var(compiler, module_id.clone()).unwrap();
                if global_var.is_type_inferred(compiler) {
                    global_var.set_type(compiler, Some(exports_type));
                }
                // Update the property slot on the parent namespace.
                let module_qname = QualifiedName::of(&module_id);
                if !module_qname.is_simple(compiler) {
                    let owner = module_qname.get_owner(compiler).unwrap();
                    let view = global_scope.as_static_typed_scope(compiler);
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    let parent_type = view.lookup_qualified_name(reg, ast, &owner);
                    let parent_object_type = parent_type.and_then(|t| t.to_maybe_object_type(reg));
                    let component = module_qname.get_component(ast);
                    if let Some(parent_object_type) = parent_object_type
                        && !parent_object_type.is_property_type_declared(
                            reg,
                            ast,
                            component.clone(),
                        )
                    {
                        let exports_node = exports_var.get_node(compiler);
                        let (reg, ast) = compiler.get_type_registry_and_ast();
                        parent_object_type.define_inferred_property(
                            reg,
                            ast,
                            component,
                            exports_type,
                            exports_node,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    // port: TypeInference#traverse
    fn traverse(&mut self, compiler: &mut AbstractCompiler, n: NodeId, mut scope: FS) -> FS {
        let mut is_typeable = true;
        match n.get_token(compiler) {
            Token::ASSIGN => scope = self.traverse_assign(compiler, n, scope),
            Token::NAME => scope = self.traverse_name(compiler, n, scope),
            Token::OPTCHAIN_GETPROP | Token::OPTCHAIN_CALL | Token::OPTCHAIN_GETELEM => {
                scope = self.traverse_opt_chain(compiler, n, scope)
            }
            Token::GETPROP => scope = self.traverse_get_prop(compiler, n, scope),
            Token::CLASS => scope = self.traverse_class(compiler, n, scope),
            Token::ASSIGN_AND | Token::ASSIGN_OR => {
                scope = self.traverse_short_circuiting_bin_op_assignment(compiler, n, scope)
            }
            Token::AND => {
                scope = self
                    .traverse_and(compiler, n, scope)
                    .get_joined_flow_scope(self, compiler)
            }
            Token::OR => {
                scope = self
                    .traverse_or(compiler, n, scope)
                    .get_joined_flow_scope(self, compiler)
            }
            Token::ASSIGN_COALESCE | Token::COALESCE => {
                scope = self.traverse_nullish_coalesce(compiler, n, scope)
            }
            Token::HOOK => scope = self.traverse_hook(compiler, n, scope),
            Token::OBJECTLIT => scope = self.traverse_object_literal(compiler, n, scope),
            Token::CALL => scope = self.traverse_call(compiler, n, scope),
            Token::NEW => scope = self.traverse_new(compiler, n, scope),
            Token::NEW_TARGET => self.traverse_new_target(compiler, n),
            Token::ASSIGN_ADD | Token::ADD => scope = self.traverse_add(compiler, n, scope),
            Token::POS => scope = self.traverse_unary_plus(compiler, n, scope),
            Token::NEG | Token::BITNOT | Token::DEC | Token::INC => {
                scope = self.traverse_big_int_compatible_unary_operator(compiler, n, scope)
            }
            Token::ARRAYLIT => scope = self.traverse_array_literal(compiler, n, scope),
            Token::THIS => {
                let t = scope.get_type_of_this(compiler);
                n.set_jstype(compiler, t);
            }
            Token::ASSIGN_LSH
            | Token::ASSIGN_RSH
            | Token::ASSIGN_DIV
            | Token::ASSIGN_MOD
            | Token::ASSIGN_BITAND
            | Token::ASSIGN_BITXOR
            | Token::ASSIGN_BITOR
            | Token::ASSIGN_MUL
            | Token::ASSIGN_SUB
            | Token::ASSIGN_EXPONENT => scope = self.traverse_assign_op(compiler, n, scope),
            Token::ASSIGN_URSH => {
                // >>> is not compatible with BigInt
                scope = self.traverse_assign_unsigned_right_shift(compiler, n, scope)
            }
            Token::BITAND
            | Token::BITXOR
            | Token::BITOR
            | Token::LSH
            | Token::RSH
            | Token::SUB
            | Token::MUL
            | Token::DIV
            | Token::MOD
            | Token::EXPONENT => {
                scope = self.traverse_big_int_compatible_binary_operator(compiler, n, scope)
            }
            Token::URSH => {
                // >>> is not compatible with BigInt
                scope = self.traverse_unsigned_right_shift(compiler, n, scope)
            }
            Token::COMMA => {
                scope = self.traverse_children(compiler, n, scope);
                let last = n.get_last_child(compiler).unwrap();
                let t = self.get_js_type(compiler, last);
                n.set_jstype(compiler, Some(t));
            }
            Token::TEMPLATELIT | Token::TYPEOF => {
                scope = self.traverse_children(compiler, n, scope);
                let t = self.get_native_type(compiler, JSTypeNative::STRING_TYPE);
                n.set_jstype(compiler, Some(t));
            }
            Token::TEMPLATELIT_SUB
            | Token::THROW
            | Token::ITER_SPREAD
            | Token::OBJECT_SPREAD
            | Token::IMPORT
            | Token::IMPORT_SPECS
            | Token::IMPORT_STAR => {
                // these nodes are untyped but have children that may affect the flow scope and
                // need to be typed.
                scope = self.traverse_children(compiler, n, scope);
                is_typeable = false;
            }
            Token::IMPORT_SPEC => {
                // these nodes are untyped but have children that need to be typed.
                let _ = self.traverse_import_spec(compiler, scope.clone(), n);
                is_typeable = false;
            }
            Token::TAGGED_TEMPLATELIT => {
                scope = self.traverse_tagged_template_lit(compiler, n, scope)
            }
            Token::DELPROP
            | Token::LT
            | Token::LE
            | Token::GT
            | Token::GE
            | Token::NOT
            | Token::EQ
            | Token::NE
            | Token::SHEQ
            | Token::SHNE
            | Token::INSTANCEOF
            | Token::IN => {
                scope = self.traverse_children(compiler, n, scope);
                let t = self.get_native_type(compiler, JSTypeNative::BOOLEAN_TYPE);
                n.set_jstype(compiler, Some(t));
            }
            Token::GETELEM => scope = self.traverse_get_elem(compiler, n, scope),
            Token::EXPR_RESULT => {
                scope = self.traverse_children(compiler, n, scope);
                let first = n.get_first_child(compiler).unwrap();
                if first.is_get_prop(compiler) {
                    let getprop = first;
                    let obj = getprop.get_first_child(compiler).unwrap();
                    let obj_type = self.get_js_type(compiler, obj);
                    let owner_type = {
                        let (reg, ast) = compiler.get_type_registry_and_ast();
                        let restricted = obj_type.restrict_by_not_null_or_undefined(reg, ast);
                        object_type::cast(reg, Some(restricted))
                    };
                    if let Some(owner_type) = owner_type {
                        self.ensure_property_declared_helper(compiler, getprop, owner_type, &scope);
                    }
                }
                is_typeable = false;
            }
            Token::SWITCH => {
                let first = n.get_first_child(compiler).unwrap();
                scope = self.traverse(compiler, first, scope);
                is_typeable = false;
            }
            Token::RETURN => {
                scope = self.traverse_return(compiler, n, scope);
                is_typeable = false;
            }
            Token::YIELD => {
                if n.is_yield_all(compiler) {
                    scope = self.traverse_yield_all(compiler, n, scope);
                } else {
                    scope = self.traverse_children(compiler, n, scope);
                    let t = self.get_native_type(compiler, JSTypeNative::UNKNOWN_TYPE);
                    n.set_jstype(compiler, Some(t));
                }
            }
            Token::VAR | Token::LET | Token::CONST => {
                scope = self.traverse_declaration(compiler, n, scope);
                is_typeable = false;
            }
            Token::CATCH => {
                scope = self.traverse_catch(compiler, n, scope);
                is_typeable = false;
            }
            Token::CAST => {
                scope = self.traverse_children(compiler, n, scope);
                let info = n.get_jsdoc_info(compiler);
                // TODO(b/123955687): also check that info.hasType() is true
                let info = check_not_null!(info, "CAST node should always have JSDocInfo");
                if info.has_type() {
                    // NOTE(lharker) - I tried moving CAST type evaluation into the typed scope
                    // creation phase.
                    // Since it caused a few new, seemingly spurious, 'Bad type annotation' and
                    // 'unknown property type' warnings, and having it in TypeInference seems to
                    // work, we just do the lookup + resolution here.
                    let declaration_scope = scope.get_declaration_scope(compiler);
                    let static_scope = declaration_scope.as_static_typed_scope_arc(compiler);
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    let evaluated = info
                        .get_type()
                        .unwrap()
                        .evaluate(reg, ast, Some(static_scope));
                    let resolved = evaluated.resolve(reg, ast);
                    n.set_jstype(compiler, Some(resolved));
                } else {
                    n.set_jstype(compiler, Some(self.unknown_type));
                }
            }
            Token::SUPER => self.traverse_super(compiler, n, &scope),
            Token::AWAIT => scope = self.traverse_await(compiler, n, scope),
            Token::VOID => {
                let t = self.get_native_type(compiler, JSTypeNative::VOID_TYPE);
                n.set_jstype(compiler, Some(t));
                scope = self.traverse_children(compiler, n, scope);
            }
            Token::EXPORT => {
                scope = self.traverse_children(compiler, n, scope);
                if n.get_boolean_prop(compiler, Prop::EXPORT_DEFAULT) {
                    // TypedScopeCreator declared a dummy variable *default* to store this type.
                    // Update the variable with the inferred type.
                    let default_export =
                        Self::get_declared_var(compiler, &scope, Export::DEFAULT_EXPORT_NAME)
                            .unwrap();
                    if default_export.is_type_inferred(compiler) {
                        let only = n.get_only_child(compiler);
                        let t = self.get_js_type(compiler, only);
                        default_export.set_type(compiler, Some(t));
                    }
                }
                is_typeable = false;
            }
            Token::IMPORT_META => {
                // TODO(b/137797083): Set an appropriate type.
                n.set_jstype(compiler, Some(self.unknown_type));
            }
            Token::ROOT
            | Token::SCRIPT
            | Token::MODULE_BODY
            | Token::SWITCH_BODY
            | Token::FUNCTION
            | Token::PARAM_LIST
            | Token::BLOCK
            | Token::EMPTY
            | Token::IF
            | Token::WHILE
            | Token::DO
            | Token::FOR
            | Token::FOR_IN
            | Token::FOR_OF
            | Token::FOR_AWAIT_OF
            | Token::BREAK
            | Token::CONTINUE
            | Token::TRY
            | Token::CASE
            | Token::DEFAULT_CASE
            | Token::WITH
            | Token::DEBUGGER
            | Token::EXPORT_SPECS
            | Token::LABEL => {
                // These don't need to be typed here, since they only affect control flow.
                is_typeable = false;
            }
            Token::DYNAMIC_IMPORT => {
                let _ = self.traverse_dynamic_import(compiler, n, scope.clone());
            }
            Token::TRUE
            | Token::FALSE
            | Token::STRINGLIT
            | Token::NUMBER
            | Token::BIGINT
            | Token::NULL
            | Token::REGEXP
            | Token::TEMPLATELIT_STRING => {
                // Primitives are typed in TypedScopeCreator.AbstractScopeBuilder#attachLiteralTypes
            }
            token => panic!("Type inference doesn't know to handle token {}", token),
        }

        if is_typeable
            && n.get_jstype(compiler).is_none()
            && !TOKENS_ALLOWING_NULL_TYPES.contains(&n.get_token(compiler))
        {
            panic!("Failed to infer JSType for {}", n.to_string(compiler));
        }
        scope
    }

    // port: TypeInference#initializeEnhancedForScope
    fn initialize_enhanced_for_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        source: NodeId,
        output: FS,
    ) -> FS {
        let mut item = source.get_first_child(compiler).unwrap();
        let obj = item.get_next(compiler).unwrap();

        let mut informed = self.traverse(compiler, obj, output);

        let assignment_type = if NodeUtil::is_name_declaration(compiler, Some(item)) {
            item = item.get_first_child(compiler).unwrap();
            AssignmentType::DECLARATION
        } else {
            AssignmentType::ASSIGN
        };
        if item.is_destructuring_lhs(compiler) {
            item = item.get_first_child(compiler).unwrap();
        }

        let new_type = match source.get_token(compiler) {
            Token::FOR_IN => {
                // item is assigned a property name, so its type should be string
                let mut iter_key_type = self.get_native_type(compiler, JSTypeNative::STRING_TYPE);
                let obj_jstype = self.get_js_type(compiler, obj);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let obj_type = obj_jstype.autobox(reg, ast);
                let object_index_key = reg.get_object_index_key();
                let obj_index_type = obj_type
                    .get_template_type_map(reg)
                    .get_resolved_template_type(reg, ast, object_index_key);
                if !obj_index_type.is_unknown_type(reg, ast) {
                    let narrowed_key_type =
                        iter_key_type.get_greatest_subtype(reg, ast, obj_index_type);
                    if !narrowed_key_type.is_empty_type(reg) {
                        iter_key_type = narrowed_key_type;
                    }
                }

                iter_key_type
            }
            Token::FOR_OF => {
                // for/of. The type of `item` is the type parameter of the Iterable type.
                let obj_jstype = self.get_js_type(compiler, obj);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                JsIterables::maybe_box_iterable_or_async_iterable(obj_jstype, reg, ast)
                    .or_else(self.unknown_type)
            }
            Token::FOR_AWAIT_OF => {
                // for/await/of. the iterated object is either of the Iterable or AsyncIterable
                // type. the type of `item` is the Promise.resolve() type of the object's type
                // parameter.
                let obj_jstype = self.get_js_type(compiler, obj);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let iterable_type =
                    JsIterables::maybe_box_iterable_or_async_iterable(obj_jstype, reg, ast)
                        .or_else(self.unknown_type);

                Promises::get_resolved_type(reg, ast, iterable_type)
            }
            _ => panic!("Unexpected source node {}", source.to_string(compiler)),
        };

        // Note that `item` can be an arbitrary LHS expression we need to check.
        if item.is_destructuring_pattern(compiler) {
            // for (const {x, y} of data) {
            informed = self.traverse_destructuring_pattern(
                compiler,
                item,
                informed,
                Some(new_type),
                assignment_type,
            );
        } else {
            informed = self.traverse(compiler, item, informed);
            informed = self.update_scope_for_assignment(
                compiler,
                informed,
                item,
                Some(new_type),
                assignment_type,
            );
        }
        informed
    }

    // port: TypeInference#traverseUnsignedRightShift
    fn traverse_unsigned_right_shift(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> FS {
        scope = self.traverse_children(compiler, n, scope);
        let first = n.get_first_child(compiler).unwrap();
        let last = n.get_last_child(compiler).unwrap();
        let first_type = self.get_js_type(compiler, first);
        let last_type = self.get_js_type(compiler, last);
        if Self::get_big_int_presence(compiler, first_type) != BigIntPresence::NO_BIGINT
            || Self::get_big_int_presence(compiler, last_type) != BigIntPresence::NO_BIGINT
        {
            // The spec does not allow for BigInts in an unsigned right shift, so we will report an
            // error
            let t = self.get_native_type(compiler, JSTypeNative::NO_TYPE);
            n.set_jstype(compiler, Some(t));
        } else {
            let t = self.get_native_type(compiler, JSTypeNative::NUMBER_TYPE);
            n.set_jstype(compiler, Some(t));
        }
        scope
    }

    // port: TypeInference#traverseAssignUnsignedRightShift
    fn traverse_assign_unsigned_right_shift(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> FS {
        let left = n.get_first_child(compiler).unwrap();
        scope = self.traverse_unsigned_right_shift(compiler, n, scope);
        let t = self.get_js_type(compiler, n);
        self.update_scope_for_assignment_with_update_node(
            compiler,
            scope,
            left,
            Some(t),
            /* updateNode= */ None,
            AssignmentType::ASSIGN,
        )
    }

    // port: TypeInference#traverseCall
    fn traverse_call(
        &mut self,
        compiler: &mut AbstractCompiler,
        call_node: NodeId,
        original_scope: FS,
    ) -> FS {
        check_argument!(
            call_node.is_call(compiler) || call_node.is_opt_chain_call(compiler),
            "%s",
            call_node.to_string(compiler)
        );
        let scope_after_children = self.traverse_children(compiler, call_node, original_scope);
        let scope_after_execution = self.set_call_node_type_after_children_traversed(
            compiler,
            call_node,
            scope_after_children,
        );
        // e.g. after `goog.assertString(x);` we can infer `x` is a string.
        self.tighten_types_after_assertions(compiler, scope_after_execution, call_node)
    }

    // port: TypeInference#traverseTaggedTemplateLit
    fn traverse_tagged_template_lit(
        &mut self,
        compiler: &mut AbstractCompiler,
        tag_temp_lit_node: NodeId,
        original_scope: FS,
    ) -> FS {
        check_argument!(
            tag_temp_lit_node.is_tagged_template_lit(compiler),
            "%s",
            tag_temp_lit_node.to_string(compiler)
        );
        let scope_after_children =
            self.traverse_children(compiler, tag_temp_lit_node, original_scope);
        // A tagged template literal is really a special kind of function call.
        let scope_after_execution = self.set_call_node_type_after_children_traversed(
            compiler,
            tag_temp_lit_node,
            scope_after_children,
        );
        if tag_temp_lit_node.get_jstype(compiler).is_none() {
            tag_temp_lit_node.set_jstype(compiler, Some(self.unknown_type));
        }
        scope_after_execution
    }

    // port: TypeInference#traverseSuper
    fn traverse_super(
        &mut self,
        compiler: &mut AbstractCompiler,
        super_node: NodeId,
        current_scope: &FS,
    ) {
        let mut super_node_type: Option<TypeId> = None;

        let parent = super_node.get_parent(compiler).unwrap();
        match parent.get_token(compiler) {
            Token::CALL => {
                // Find the closest non-arrow function (TODO(sdh): this could be an AbstractScope
                // method).
                let mut scope = Some(self.container_scope);
                while let Some(s) = scope
                    && !NodeUtil::is_non_arrow_function(compiler, s.get_root_node(compiler))
                {
                    scope = s.get_parent(compiler);
                }
                let Some(scope) = scope else {
                    super_node.set_jstype(compiler, Some(self.unknown_type));
                    return;
                };
                let root_type = scope.get_root_node(compiler).get_jstype(compiler);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let enclosing_function_type =
                    closure_jstype::js_type::to_maybe_function_type(reg, root_type);
                // Inside a constructor, `super` may have two different types. Calls to `super()`
                // use the super-ctor type, while property accesses use the super-instance type.
                // `Scopes` are only aware of the latter case.
                if let Some(enclosing_function_type) = enclosing_function_type
                    && enclosing_function_type.is_constructor(reg)
                {
                    super_node_type = closure_jstype::function_type::get_super_class_constructor(
                        enclosing_function_type,
                        reg,
                        ast,
                    );
                }
            }
            Token::GETELEM | Token::GETPROP => {
                let current_syntactic_scope = current_scope.get_declaration_scope(compiler);
                let slot = current_syntactic_scope.get_slot(compiler, "super").unwrap();
                let slot_type = slot.get_type(compiler);
                let reg = compiler.get_type_registry();
                super_node_type = object_type::cast(reg, slot_type);
            }
            _ => panic!(
                "Unexpected parent of SUPER: {}",
                parent.to_string_tree(compiler)
            ),
        }

        super_node.set_jstype(compiler, Some(super_node_type.unwrap_or(self.unknown_type)));
    }

    // port: TypeInference#traverseNewTarget
    fn traverse_new_target(&mut self, compiler: &mut AbstractCompiler, new_target_node: NodeId) {
        // new.target is (undefined|!Function) within a vanilla function and !Function within an
        // ES6 constructor.
        // Find the closest non-arrow function (TODO(sdh): this could be an AbstractScope method).
        let mut scope = Some(self.container_scope);
        while let Some(s) = scope
            && !NodeUtil::is_non_arrow_function(compiler, s.get_root_node(compiler))
        {
            scope = s.get_parent(compiler);
        }
        let Some(scope) = scope else {
            // NOTE: we already have a parse error for new.target outside a function.  The only
            // other case where this might happen is a top-level arrow function, which is a parse
            // error in the VM, but allowed by our parser.
            new_target_node.set_jstype(compiler, Some(self.unknown_type));
            return;
        };
        let root = scope.get_root_node(compiler);
        let parent = root.get_parent(compiler).unwrap();
        if parent.get_grandparent(compiler).unwrap().is_class(compiler) {
            // In an ES6 constuctor, new.target may not be undefined.  In any other method, it must
            // be undefined, since methods are not constructable.
            let type_ = if NodeUtil::is_es6_constructor_member_function_def(compiler, parent) {
                JSTypeNative::FUNCTION_TYPE
            } else {
                JSTypeNative::VOID_TYPE
            };
            let t = compiler.get_type_registry().get_native_type(type_);
            new_target_node.set_jstype(compiler, Some(t));
        } else {
            // Other functions also include undefined, in case they are not called with 'new'.
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let function_type = reg.get_native_type(JSTypeNative::FUNCTION_TYPE);
            let void_type = reg.get_native_type(JSTypeNative::VOID_TYPE);
            let t = reg.create_union_type(ast, &[function_type, void_type]);
            new_target_node.set_jstype(compiler, Some(t));
        }
    }

    // port: TypeInference#traverseReturn
    /// Traverse a return value.
    fn traverse_return(&mut self, compiler: &mut AbstractCompiler, n: NodeId, mut scope: FS) -> FS {
        scope = self.traverse_children(compiler, n, scope);

        let ret_value = n.get_first_child(compiler);
        if let Some(ret_value) = ret_value {
            let type_ = self
                .container_scope
                .get_root_node(compiler)
                .get_jstype(compiler);
            if let Some(type_) = type_ {
                let fn_type = type_.to_maybe_function_type(compiler.get_type_registry());
                if let Some(fn_type) = fn_type {
                    let ret_type = ret_value.get_jstype(compiler);
                    let constraint = fn_type.get_return_type(compiler.get_type_registry());
                    Self::infer_property_types_to_match_constraint(
                        compiler,
                        ret_type,
                        Some(constraint),
                    );
                }
            }
        }
        scope
    }

    // port: TypeInference#traverseYieldAll
    fn traverse_yield_all(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> FS {
        // A yield* expression will first yield all the elements of the given iterable, and then
        // evaluate to whatever the iterable returns when done.
        // The yielded type and done return type are not necessarily the same. Here, we look for
        // the done type - TReturn in Iterable<T, TReturn, TNext>.
        scope = self.traverse_children(compiler, n, scope);
        let first = n.get_first_child(compiler).unwrap();
        let inner_type = self.get_js_type(compiler, first);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let yield_all_result = JsIterables::get_return_element_type(inner_type, reg, ast);
        n.set_jstype(compiler, Some(yield_all_result));
        scope
    }

    // port: TypeInference#traverseCatch
    /// Any value can be thrown, so it's really impossible to determine the type of a CATCH param.
    /// Treat it as the UNKNOWN type.
    fn traverse_catch(
        &mut self,
        compiler: &mut AbstractCompiler,
        catch_node: NodeId,
        scope: FS,
    ) -> FS {
        let catch_target = catch_node.get_first_child(compiler).unwrap();
        if catch_target.is_name(compiler) {
            let name = catch_node.get_first_child(compiler).unwrap();
            let mut type_ = name.get_jstype(compiler);
            // If the catch expression name was declared in the catch in TypedScopeCreator use that
            // type. Otherwise use "unknown".
            if type_.is_none() {
                type_ = Some(self.unknown_type);
                name.set_jstype(compiler, Some(self.unknown_type));
            }
            self.redeclare_simple_var(compiler, scope, name, type_)
        } else if catch_target.is_destructuring_pattern(compiler) {
            let pattern = catch_node.get_first_child(compiler).unwrap();
            let unknown = self.unknown_type;
            self.traverse_destructuring_pattern(
                compiler,
                pattern,
                scope,
                Some(unknown),
                AssignmentType::DECLARATION,
            )
        } else {
            check_state!(
                catch_target.is_empty(compiler),
                "%s",
                catch_target.to_string(compiler)
            );
            // ES2019 allows `try {} catch {}` with no catch expression
            scope
        }
    }

    // port: TypeInference#traverseAssign
    fn traverse_assign(&mut self, compiler: &mut AbstractCompiler, n: NodeId, mut scope: FS) -> FS {
        let target = n.get_first_child(compiler).unwrap();
        let value = n.get_last_child(compiler).unwrap();
        if target.is_destructuring_pattern(compiler) {
            scope = self.traverse(compiler, value, scope);
            let value_type = self.get_js_type(compiler, value);
            n.set_jstype(compiler, Some(value_type));
            self.traverse_destructuring_pattern(
                compiler,
                target,
                scope,
                Some(value_type),
                AssignmentType::ASSIGN,
            )
        } else {
            scope = self.traverse_children(compiler, n, scope);

            let value_type = self.get_js_type(compiler, value);
            n.set_jstype(compiler, Some(value_type));

            self.update_scope_for_assignment(
                compiler,
                scope,
                target,
                Some(value_type),
                AssignmentType::ASSIGN,
            )
        }
    }

    // port: TypeInference#traverseAssignOp
    fn traverse_assign_op(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> FS {
        let left = n.get_first_child(compiler).unwrap();
        scope = self.traverse_big_int_compatible_binary_operator(compiler, n, scope);

        // The lhs is both an input and an output, so don't update the input type here.
        let t = self.get_js_type(compiler, n);
        self.update_scope_for_assignment_with_update_node(
            compiler,
            scope,
            left,
            Some(t),
            /* updateNode= */ None,
            AssignmentType::ASSIGN,
        )
    }

    // port: TypeInference#isInExternFile
    fn is_in_extern_file(ast: &Ast, n: NodeId) -> bool {
        NodeUtil::get_source_file(ast, Some(n)).unwrap().is_extern()
    }

    // port: TypeInference#isPossibleMixinApplication
    fn is_possible_mixin_application(ast: &Ast, lvalue: NodeId, rvalue: Option<NodeId>) -> bool {
        if Self::is_in_extern_file(ast, lvalue) {
            return true;
        }

        let jsdoc = NodeUtil::get_best_jsdoc_info(ast, lvalue);
        jsdoc.is_some_and(|jsdoc| {
            jsdoc.is_constructor()
                && jsdoc.get_implemented_interface_count() > 0
                && lvalue.is_qualified_name(ast)
                && rvalue.is_some_and(|rvalue| rvalue.is_call(ast))
        })
    }

    // port: TypeInference#addMissingInterfaceProperties
    /// `constructor`: A constructor function defined by a call, which may be a mixin application.
    /// The constructor implements at least one interface. If the constructor is missing some
    /// properties of the inherited interfaces, this method declares these properties.
    fn add_missing_interface_properties(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        constructor: Option<TypeId>,
    ) {
        let Some(constructor) = constructor else {
            return;
        };
        if !constructor.is_constructor(reg) {
            return;
        }
        let f = constructor.to_maybe_function_type(reg).unwrap();
        let proto = f.get_prototype(reg, ast);
        for interf in f.get_implemented_interfaces(reg, ast) {
            for pname in interf.get_property_names(reg, ast) {
                if !proto.has_property(reg, ast, pname.clone()) {
                    let ptype = interf.get_property_type(reg, ast, pname.clone());
                    proto.define_declared_property(reg, ast, pname, ptype, None);
                }
            }
        }
    }

    // port: TypeInference#updateScopeForAssignment(FlowScope,Node,JSType,AssignmentType)
    fn update_scope_for_assignment(
        &mut self,
        compiler: &mut AbstractCompiler,
        scope: FS,
        target: NodeId,
        result_type: Option<TypeId>,
        type_: AssignmentType,
    ) -> FS {
        self.update_scope_for_assignment_with_update_node(
            compiler,
            scope,
            target,
            result_type,
            Some(target),
            type_,
        )
    }

    // port: TypeInference#joinBooleanOutcomes
    fn join_boolean_outcomes(
        is_and: bool,
        left: BooleanLiteralSet,
        right: BooleanLiteralSet,
    ) -> BooleanLiteralSet {
        // A truthy value on the lhs of an {@code &&} can never make it to the
        // result. Same for a falsy value on the lhs of an {@code ||}.
        // Hence the intersection.
        right.union(left.intersection(BooleanLiteralSet::get(!is_and)))
    }

    // port: TypeInference#newBooleanOutcomePair
    fn new_boolean_outcome_pair(
        &self,
        compiler: &mut AbstractCompiler,
        js_type: Option<TypeId>,
        flow_scope: FS,
    ) -> BooleanOutcomePair {
        let Some(js_type) = js_type else {
            return BooleanOutcomePair::new(
                BooleanLiteralSet::BOTH,
                BooleanLiteralSet::BOTH,
                flow_scope.clone(),
                flow_scope,
            );
        };
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let to_boolean_outcomes = js_type.get_possible_to_boolean_outcomes(reg);
        let boolean_type = reg.get_native_type(JSTypeNative::BOOLEAN_TYPE);
        let boolean_values = if boolean_type.is_subtype_of(reg, ast, js_type) {
            BooleanLiteralSet::BOTH
        } else {
            BooleanLiteralSet::EMPTY
        };
        BooleanOutcomePair::new(
            to_boolean_outcomes,
            boolean_values,
            flow_scope.clone(),
            flow_scope,
        )
    }

    // port: TypeInference#redeclareSimpleVar
    fn redeclare_simple_var(
        &mut self,
        compiler: &mut AbstractCompiler,
        scope: FS,
        name_node: NodeId,
        var_type: Option<TypeId>,
    ) -> FS {
        check_state!(
            name_node.is_name(compiler),
            "%s",
            name_node.to_string(compiler)
        );
        let var_name = name_node.get_string(compiler);
        let var_type = match var_type {
            Some(t) => t,
            None => self.get_native_type(compiler, JSTypeNative::UNKNOWN_TYPE),
        };
        let declared = Self::get_declared_var(compiler, &scope, var_name.clone());
        if self.is_unflowable_opt(compiler, declared) {
            return scope;
        }
        scope.infer_slot_type(compiler, &var_name, Some(var_type))
    }

    // port: TypeInference#isUnflowable
    fn is_unflowable(&self, compiler: &AbstractCompiler, v: TypedVar) -> bool {
        self.is_unflowable_opt(compiler, Some(v))
    }

    /// Rust-only: `isUnflowable` with Java's nullable argument.
    fn is_unflowable_opt(&self, compiler: &AbstractCompiler, v: Option<TypedVar>) -> bool {
        v.is_some_and(|v| {
            v.is_local(compiler)
                && v.is_marked_escaped(compiler)
                // It's OK to flow a variable in the scope where it's escaped.
                && v.get_scope(compiler).get_closest_container_scope(compiler)
                    == self.container_scope
        })
    }

    // port: TypeInference#getJSType
    /// This method gets the JSType from the Node argument and verifies that it is present.
    fn get_js_type(&self, compiler: &AbstractCompiler, n: NodeId) -> TypeId {
        let js_type = n.get_jstype(compiler);
        match js_type {
            // TODO(nicksantos): This branch indicates a compiler bug, not worthy of
            // halting the compilation but we should log this and analyze to track
            // down why it happens. This is not critical and will be resolved over
            // time as the type checker is extended.
            None => self.unknown_type,
            Some(js_type) => js_type,
        }
    }

    // port: TypeInference#getNativeType
    fn get_native_type(&self, compiler: &mut AbstractCompiler, type_id: JSTypeNative) -> TypeId {
        compiler.get_type_registry().get_native_type(type_id)
    }

    // port: TypeInference#getDeclaredVar
    fn get_declared_var(
        compiler: &mut AbstractCompiler,
        scope: &FS,
        name: impl Into<JsString>,
    ) -> Option<TypedVar> {
        scope
            .get_declaration_scope(compiler)
            .get_var(compiler, name)
    }
}

impl DataFlowAnalysis<NodeId, FS> for TypeInference<'_> {
    fn state(&self) -> &DataFlowAnalysisState<NodeId> {
        &self.state
    }
    fn state_mut(&mut self) -> &mut DataFlowAnalysisState<NodeId> {
        &mut self.state
    }

    // port: TypeInference#createInitialEstimateLattice
    fn create_initial_estimate_lattice(&self) -> FS {
        self.bottom_scope.clone()
    }

    // port: TypeInference#createEntryLattice
    fn create_entry_lattice(&mut self, compiler: &mut AbstractCompiler) -> FS {
        // only ever called once so we don't need to cache this computation
        let entry: FS = LinkedFlowScope::create_entry_lattice(self.container_scope);
        let entry_scope = self.infer_declaratively_unbound_vars_without_types(compiler, entry);

        self.infer_parameters(compiler, entry_scope)
    }

    // port: TypeInference#flowThrough
    fn flow_through(&mut self, compiler: &mut AbstractCompiler, n: NodeId, input: FS) -> FS {
        // If we have not walked a path from <entry> to <n>, then we don't
        // want to infer anything about this scope.
        if same_scope(&input, &self.bottom_scope) {
            return input;
        }

        // This method also does some logic for ES modules right before and after entering/exiting
        // the scope rooted at the module. The reasoning for separating out this logic is that we
        // can just ignore the actual AST nodes for IMPORT/EXPORT, in most cases, because we have
        // already created an abstraction of imports and exports.
        let root = NodeUtil::get_enclosing_scope_root(compiler, n).unwrap();
        // Inferred types of ES module imports/exports aren't knowable until after TypeInference
        // runs. First update the type of all imports in the scope, then do flow-sensitive
        // inference, then update the implicit '*exports*' object.
        let module = {
            let module_map = compiler.get_module_map().cloned();
            ModuleImportResolver::get_module_from_scope_root(module_map.as_deref(), compiler, root)
        };
        let syntactic_block_scope = self.scope_creator.create_scope_for_node(compiler, root);
        if let Some(module) = &module
            && module.metadata().is_es6_module()
        {
            let input_id = NodeUtil::get_input_id(compiler, n);
            let module_input = input_id.and_then(|id| compiler.get_input(&id).cloned());
            let scope_creator: &TypedScopeCreator = self.scope_creator;
            let mapper = scope_creator.get_node_to_scope_mapper();
            self.module_import_resolver.declare_es_module_imports(
                compiler,
                &mapper,
                module,
                syntactic_block_scope,
                module_input,
            );
        }

        // This logic is not specific to ES modules.
        let mut output = input.with_syntactic_scope(compiler, syntactic_block_scope);
        output = self.infer_declaratively_unbound_vars_without_types(compiler, output);
        output = self.traverse(compiler, n, output);

        self.update_module_scope(compiler, module, syntactic_block_scope);
        output
    }

    // port: TypeInference#isForward
    fn is_forward(&self) -> bool {
        true
    }

    // port: TypeInference#isBranched
    fn is_branched(&self) -> bool {
        true
    }

    // port: TypeInference#createFlowJoiner
    fn create_flow_joiner(&self) -> Box<dyn FlowJoiner<FS>> {
        Box::new(FlowScopeJoinOp::new())
    }

    // port: TypeInference#createFlowBrancher
    fn create_flow_brancher(
        &mut self,
        _compiler: &mut AbstractCompiler,
        source: NodeId,
        output: FS,
    ) -> Box<dyn FlowBrancher<FS, Self>> {
        Box::new(TypeInferenceFlowBrancher {
            source,
            output,
            condition: None,
            condition_flow_scope: None,
            condition_outcomes: None,
        })
    }
}

/// Java's anonymous `FlowBrancher<FlowScope>` of TypeInference#createFlowBrancher.
// NOTE(nicksantos): Right now, we just treat ON_EX edges like UNCOND
// edges. If we wanted to be perfect, we'd actually JOIN all the out
// lattices of this flow with the in lattice, and then make that the out
// lattice for the ON_EX edge. But it's probably too expensive to be
// worthwhile.
struct TypeInferenceFlowBrancher {
    source: NodeId,
    output: FS,
    condition: Option<NodeId>,
    condition_flow_scope: Option<FS>,
    condition_outcomes: Option<BooleanOutcomePair>,
}

impl<'a> FlowBrancher<FS, TypeInference<'a>> for TypeInferenceFlowBrancher {
    // port: TypeInference#createFlowBrancher.branchFlow
    fn branch_flow(
        &mut self,
        ti: &mut TypeInference<'a>,
        compiler: &mut AbstractCompiler,
        branch: Branch,
    ) -> FS {
        let source = self.source;
        match branch {
            Branch::ON_TRUE | Branch::ON_FALSE => {
                if branch == Branch::ON_TRUE && NodeUtil::is_enhanced_for(compiler, source) {
                    return ti.initialize_enhanced_for_scope(compiler, source, self.output.clone());
                }
                // FALL THROUGH

                if self.condition.is_none() {
                    if source.is_case(compiler) {
                        self.condition = Some(source);
                        let first = source.get_first_child(compiler).unwrap();
                        self.condition_flow_scope =
                            Some(ti.traverse(compiler, first, self.output.clone()));
                    } else {
                        self.condition = NodeUtil::get_condition_expression(compiler, source);
                        if self.condition.is_none() {
                            return self.output.clone();
                        }
                    }
                }
                let condition = self.condition.unwrap();

                if condition.is_and(compiler) || condition.is_or(compiler) {
                    // When handling the short-circuiting binary operators,
                    // the outcome scope on true can be different than the outcome
                    // scope on false.
                    //
                    // TODO(nicksantos): The "right" way to do this is to
                    // carry the known outcome all the way through the
                    // recursive traversal, so that we can construct a
                    // different flow scope based on the outcome. However,
                    // this would require a bunch of code and a bunch of
                    // extra computation for an edge case. This seems to be
                    // a "good enough" approximation.

                    // conditionOutcomes is cached from previous calls to the brancher
                    if self.condition_outcomes.is_none() {
                        self.condition_outcomes = Some(if condition.is_and(compiler) {
                            ti.traverse_and(compiler, condition, self.output.clone())
                        } else {
                            ti.traverse_or(compiler, condition, self.output.clone())
                        });
                    }
                    let token = condition.get_token(compiler);
                    let outcome_scope = self
                        .condition_outcomes
                        .as_mut()
                        .unwrap()
                        .get_outcome_flow_scope(ti, compiler, token, branch == Branch::ON_TRUE);
                    let reverse_interpreter = Arc::clone(&ti.reverse_interpreter);
                    return reverse_interpreter.get_preciser_scope_knowing_condition_outcome(
                        compiler,
                        condition,
                        outcome_scope,
                        Outcome::for_boolean(branch == Branch::ON_TRUE),
                    );
                }

                // conditionFlowScope is cached from previous calls to the brancher
                if self.condition_flow_scope.is_none() {
                    self.condition_flow_scope =
                        Some(ti.traverse(compiler, condition, self.output.clone()));
                }
                let reverse_interpreter = Arc::clone(&ti.reverse_interpreter);
                reverse_interpreter.get_preciser_scope_knowing_condition_outcome(
                    compiler,
                    condition,
                    self.condition_flow_scope.clone().unwrap(),
                    Outcome::for_boolean(branch == Branch::ON_TRUE),
                )
            }
            _ => self.output.clone(),
        }
    }
}

// port: TypeInference#TOKENS_ALLOWING_NULL_TYPES
// TODO(b/154044898): delete these exceptions. Names are given null types to enable inference of
// undeclared names assigned in multiple local scopes. The compiler also infers call and new
// types when the invocation target is such a name.
static TOKENS_ALLOWING_NULL_TYPES: [Token; 3] = [Token::NAME, Token::CALL, Token::NEW];

// port: TypeInference.BooleanOutcomePair
/// When traversing short-circuiting binary operations, we need to keep track of two sets of
/// boolean literals: 1. `toBooleanOutcomes`: boolean literals as converted from any types, 2.
/// `booleanValues`: boolean literals from just boolean types.
struct BooleanOutcomePair {
    to_boolean_outcomes: BooleanLiteralSet,
    boolean_values: BooleanLiteralSet,

    // The scope if only half of the expression executed, when applicable.
    left_scope: FS,

    // The scope when the whole expression executed.
    right_scope: FS,

    // The scope when we don't know how much of the expression is executed.
    joined_scope: Option<FS>,
}

impl BooleanOutcomePair {
    // port: TypeInference.BooleanOutcomePair#BooleanOutcomePair
    fn new(
        to_boolean_outcomes: BooleanLiteralSet,
        boolean_values: BooleanLiteralSet,
        left_scope: FS,
        right_scope: FS,
    ) -> Self {
        Self {
            to_boolean_outcomes,
            boolean_values,
            left_scope,
            right_scope,
            joined_scope: None,
        }
    }

    // port: TypeInference.BooleanOutcomePair#getJoinedFlowScope
    /// Gets the safe estimated scope without knowing if all of the subexpressions will be
    /// evaluated.
    fn get_joined_flow_scope(
        &mut self,
        ti: &TypeInference<'_>,
        compiler: &mut AbstractCompiler,
    ) -> FS {
        if self.joined_scope.is_none() {
            if same_scope(&self.left_scope, &self.right_scope) {
                self.joined_scope = Some(self.right_scope.clone());
            } else {
                self.joined_scope =
                    Some(ti.join(compiler, self.left_scope.clone(), self.right_scope.clone()));
            }
        }
        self.joined_scope.clone().unwrap()
    }

    // port: TypeInference.BooleanOutcomePair#getOutcomeFlowScope
    /// Gets the outcome scope if we do know the outcome of the entire expression.
    fn get_outcome_flow_scope(
        &mut self,
        ti: &TypeInference<'_>,
        compiler: &mut AbstractCompiler,
        node_type: Token,
        outcome: bool,
    ) -> FS {
        if (node_type == Token::AND && outcome) || (node_type == Token::OR && !outcome) {
            // We know that the whole expression must have executed.
            self.right_scope.clone()
        } else {
            self.get_joined_flow_scope(ti, compiler)
        }
    }
}

// port: TypeInference.BigIntPresence
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BigIntPresence {
    NO_BIGINT,
    ALL_BIGINT,
    BIGINT_OR_NUMBER,
    BIGINT_OR_OTHER,
}

impl TypeInference<'_> {
    // port: TypeInference#updateScopeForAssignment(FlowScope,Node,JSType,Node,AssignmentType)
    /// Updates the scope according to the result of an assignment.
    // Java's `!(isLet && !hasChildren)` condition is kept as written.
    #[allow(clippy::nonminimal_bool)]
    fn update_scope_for_assignment_with_update_node(
        &mut self,
        compiler: &mut AbstractCompiler,
        mut scope: FS,
        target: NodeId,
        result_type: Option<TypeId>,
        update_node: Option<NodeId>,
        type_: AssignmentType,
    ) -> FS {
        let result_type = check_not_null!(result_type);
        check_state!(update_node.is_none() || update_node == Some(target));

        let target_type = target.get_jstype(compiler); // may be null

        let right = NodeUtil::get_r_value_of_l_value(compiler, target);
        if Self::is_possible_mixin_application(compiler, target, right) {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            Self::add_missing_interface_properties(reg, ast, target_type);
        }

        match target.get_token(compiler) {
            Token::NAME => {
                let var_name = target.get_string(compiler);
                let var = Self::get_declared_var(compiler, &scope, var_name);
                let var_type = var.and_then(|var| var.get_type(compiler));
                let is_var_declaration = type_ == AssignmentType::DECLARATION
                    && var.is_some_and(|var| {
                        !var.is_type_inferred(compiler)
                            // implicit vars (like arguments) have no nameNode
                            && var.get_name_node(compiler).is_some()
                    });

                // Whether this variable is declared not because it has JSDoc with a declaration,
                // but because it is const and the right-hand-side is easily inferrable.
                // e.g. these are 'typeless const declarations':
                //   const x = 0;
                //   /** @const */
                //   a.b.c = SomeOtherConstructor;
                // but these are not:
                //    let x = 0;
                //    /** @const @constructor */
                //    a.b.c = someMixin();
                // This is messy, since the definition of 'typeless const' is duplicated in
                // TypedScopeCreator and this code.
                let is_typeless_const_decl = is_var_declaration
                    && {
                        let var = var.unwrap();
                        let name_node = var.get_name_node(compiler).unwrap();
                        let info = var.get_jsdoc_info(compiler);
                        name_node.is_name(compiler) // ignore redeclarations of implicit globals
                        && NodeUtil::is_constant_declaration(compiler, info.as_deref(), name_node)
                        && !info
                            .as_ref()
                            .is_some_and(|info| info.contains_declaration_excluding_typeless_const())
                        && !var.get_type(compiler).is_some_and(|t| {
                            t.is_no_resolved_type(compiler.get_type_registry())
                        })
                    };

                // When looking at VAR initializers for declared VARs, we tend
                // to use the declared type over the type it's being
                // initialized to in the global scope.
                //
                // For example,
                // /** @param {number} */ var f = goog.abstractMethod;
                // it's obvious that the programmer wants you to use
                // the declared function signature, not the inferred signature.
                //
                // Or,
                // /** @type {Object.<string>} */ var x = {};
                // the one-time anonymous object on the right side
                // is as narrow as it can possibly be, but we need to make
                // sure we back-infer the <string> element constraint on
                // the left hand side, so we use the left hand side.

                let is_var_type_better = is_var_declaration
                    // Makes it easier to check for NPEs.
                    && !result_type.is_null_type(compiler.get_type_registry())
                    && !result_type.is_void_type(compiler.get_type_registry())
                    // Do not use the var type if the declaration looked like
                    // /** @const */ var x = 3;
                    // because this type was computed from the RHS
                    && !is_typeless_const_decl;

                // TODO(nicksantos): This might be a better check once we have
                // back-inference of object/array constraints.  It will probably
                // introduce more type warnings.  It uses the result type iff it's
                // strictly narrower than the declared var type.
                //
                // boolean isVarTypeBetter = isVarDeclaration &&
                //    (varType.restrictByNotNullOrUndefined().isSubtype(resultType)
                //     || !resultType.isSubtype(varType));

                if is_var_type_better {
                    scope = self.redeclare_simple_var(compiler, scope, target, var_type);
                } else {
                    scope = self.redeclare_simple_var(compiler, scope, target, Some(result_type));
                }

                if let Some(update_node) = update_node {
                    update_node.set_jstype(compiler, Some(result_type));
                }

                if let Some(var) = var
                    && var.is_type_inferred(compiler)
                    // Don't change the typed scope to include "undefined" upon seeing "let foo;",
                    // because this is incompatible with how we currently handle VARs and breaks
                    // existing code.
                    // TODO(sdh): remove this condition after cleaning up code depending on it.
                    && !(target.get_parent(compiler).unwrap().is_let(compiler)
                        && !target.has_children(compiler))
                {
                    let old_type = var.get_type(compiler);
                    let new_type = match old_type {
                        None => result_type,
                        Some(old_type) => {
                            let (reg, ast) = compiler.get_type_registry_and_ast();
                            old_type.get_least_supertype(reg, ast, result_type)
                        }
                    };
                    var.set_type(compiler, Some(new_type));
                } else if is_typeless_const_decl {
                    // /** @const */ var x = y;
                    // should be redeclared, so that the type of y
                    // gets propagated to inner scopes.
                    var.unwrap().set_type(compiler, Some(result_type));
                }
            }
            Token::GETPROP => {
                if target.is_qualified_name(compiler) {
                    let qualified_name = target.get_qualified_name(compiler).unwrap();
                    let mut declared_slot_type = false;
                    let raw_obj_type = target
                        .get_first_child(compiler)
                        .unwrap()
                        .get_jstype(compiler);
                    if let Some(raw_obj_type) = raw_obj_type {
                        let prop_name = target.get_string(compiler);
                        let (reg, ast) = compiler.get_type_registry_and_ast();
                        let restricted = raw_obj_type.restrict_by_not_null_or_undefined(reg, ast);
                        let obj_type = object_type::cast(reg, Some(restricted));
                        if let Some(obj_type) = obj_type {
                            declared_slot_type =
                                obj_type.is_property_type_declared(reg, ast, prop_name);
                        }
                    }
                    let safe_left_type = target_type.unwrap_or(self.unknown_type);
                    scope = scope.infer_qualified_slot(
                        compiler,
                        target,
                        &qualified_name,
                        Some(safe_left_type),
                        result_type,
                        declared_slot_type,
                    );
                }

                if let Some(update_node) = update_node {
                    update_node.set_jstype(compiler, Some(result_type));
                }
                self.ensure_property_defined(compiler, target, result_type, &scope);
            }
            _ => {}
        }
        scope
    }

    // port: TypeInference#ensurePropertyDefined
    /// Defines a property if the property has not been defined yet.
    fn ensure_property_defined(
        &mut self,
        compiler: &mut AbstractCompiler,
        getprop: NodeId,
        right_type: TypeId,
        scope: &FS,
    ) {
        let prop_name = getprop.get_string(compiler);
        let obj = getprop.get_first_child(compiler).unwrap();
        let node_type = self.get_js_type(compiler, obj);
        let object_type = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let restricted = node_type.restrict_by_not_null_or_undefined(reg, ast);
            object_type::cast(reg, Some(restricted))
        };
        let prop_creation_in_constructor = obj.is_this(compiler) && {
            let root = self.container_scope.get_root_node(compiler);
            let root_type = self.get_js_type(compiler, root);
            root_type.is_constructor(compiler.get_type_registry())
        };

        let Some(object_type) = object_type else {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            reg.register_property_on_type(ast, prop_name, node_type);
            return;
        };
        let is_struct_without_prop = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            node_type.is_struct(reg, ast) && !object_type.has_property(reg, ast, prop_name.clone())
        };
        if is_struct_without_prop {
            // In general, we don't want to define a property on a struct object,
            // b/c TypeCheck will later check for improper property creation on
            // structs. There are two exceptions.
            // 1) If it's a property created inside the constructor, on the newly
            //    created instance, allow it.
            // 2) If it's a prototype property, allow it. For example:
            //    Foo.prototype.bar = baz;
            //    where Foo.prototype is a struct and the assignment happens at the
            //    top level and the constructor Foo is defined in the same file.
            let mut static_prop_creation = false;
            let maybe_assign_stm = getprop.get_grandparent(compiler).unwrap();
            if self.container_scope.is_global(compiler)
                && NodeUtil::is_prototype_property_declaration(compiler, maybe_assign_stm)
            {
                let prop_creation_filename = maybe_assign_stm.get_source_file_name(compiler);
                let reg = compiler.get_type_registry();
                let ctor = object_type.get_owner_function(reg).unwrap().get_source(reg);
                if let Some(ctor) = ctor
                    && ctor.get_source_file_name(compiler) == prop_creation_filename
                {
                    static_prop_creation = true;
                }
            }
            if !prop_creation_in_constructor && !static_prop_creation {
                return; // Early return to avoid creating the property below.
            }
        }

        if self.ensure_property_declared_helper(compiler, getprop, object_type, scope) {
            return;
        }

        let (reg, ast) = compiler.get_type_registry_and_ast();
        if !object_type.is_property_type_declared(reg, ast, prop_name.clone()) {
            // We do not want a "stray" assign to define an inferred property
            // for every object of this type in the program. So we use a heuristic
            // approach to determine whether to infer the property.
            //
            // 1) If the property is already defined, join it with the previously
            //    inferred type.
            // 2) If this isn't an instance object, define it.
            // 3) If the property of an object is being assigned in the constructor,
            //    define it.
            // 4) If this is a stub, define it.
            // 5) Otherwise, do not define the type, but declare it in the registry
            //    so that we can use it for missing property checks.
            if object_type.has_property(reg, ast, prop_name.clone())
                || !object_type.is_instance_type(reg)
            {
                if prop_name == "prototype" {
                    self.define_declared_property(
                        compiler,
                        object_type,
                        &prop_name,
                        Some(right_type),
                        getprop,
                    );
                } else {
                    object_type.define_inferred_property(
                        reg,
                        ast,
                        prop_name,
                        right_type,
                        Some(getprop),
                    );
                }
            } else if prop_creation_in_constructor {
                object_type.define_inferred_property(
                    reg,
                    ast,
                    prop_name,
                    right_type,
                    Some(getprop),
                );
            } else {
                reg.register_property_on_type(ast, prop_name, object_type);
            }
        }
    }

    // port: TypeInference#defineDeclaredProperty
    fn define_declared_property(
        &mut self,
        compiler: &mut AbstractCompiler,
        object_type: TypeId,
        prop_name: &JsString,
        right_type: Option<TypeId>,
        getprop: NodeId,
    ) -> bool {
        let getprop_parent = getprop.get_parent(compiler).unwrap();
        if *prop_name == "prototype" && !getprop_parent.is_expr_result(compiler) {
            let reg = compiler.get_type_registry();
            let function_type = object_type.to_maybe_function_type(reg);
            if let Some(function_type) = function_type
                && let Some(source) = function_type.get_source(reg)
                && source.is_class(compiler)
            {
                compiler.report(JSError::make(
                    compiler,
                    getprop_parent,
                    &REASSIGN_CLASS_PROTOTYPE,
                    &[],
                ));
                return true;
            }
        }
        // Java passes the declared type through; a declared property always has one.
        let right_type = check_not_null!(right_type);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        object_type.define_declared_property(reg, ast, prop_name.clone(), right_type, Some(getprop))
    }

    // port: TypeInference#ensurePropertyDeclaredHelper
    /// Declares a property on its owner, if necessary.
    ///
    /// Returns true if a property was declared.
    fn ensure_property_declared_helper(
        &mut self,
        compiler: &mut AbstractCompiler,
        getprop: NodeId,
        object_type: TypeId,
        scope: &FS,
    ) -> bool {
        if getprop.is_qualified_name(compiler) {
            let prop_name = getprop.get_string(compiler);
            let q_name = getprop.get_qualified_name(compiler).unwrap();
            let var = Self::get_declared_var(compiler, scope, q_name);
            if let Some(var) = var
                && !var.is_type_inferred(compiler)
            {
                // Handle normal declarations that could not be addressed earlier.
                let should_define = prop_name == "prototype" || {
                    // Handle prototype declarations that could not be addressed earlier.
                    let is_extern = var.is_extern(compiler);
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    !object_type.has_own_property(reg, ast, prop_name.clone())
                        && (!object_type.is_instance_type(reg)
                            || (is_extern && !object_type.is_native_object_type(reg)))
                };
                if should_define {
                    let var_type = var.get_type(compiler);
                    return self.define_declared_property(
                        compiler,
                        object_type,
                        &prop_name,
                        var_type,
                        getprop,
                    );
                }
            }
        }
        false
    }

    // port: TypeInference#traverseDeclaration
    fn traverse_declaration(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> FS {
        let mut declaration_child = n.get_first_child(compiler);
        while let Some(child) = declaration_child {
            scope = self.traverse_declaration_child(compiler, child, scope);
            declaration_child = child.get_next(compiler);
        }

        scope
    }

    // port: TypeInference#traverseDeclarationChild
    fn traverse_declaration_child(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> FS {
        if n.is_name(compiler) {
            return self.traverse_name(compiler, n, scope);
        }

        check_state!(
            n.is_destructuring_lhs(compiler),
            "%s",
            n.to_string(compiler)
        );
        let second = n.get_second_child(compiler).unwrap();
        scope = self.traverse(compiler, second, scope);
        let first = n.get_first_child(compiler).unwrap();
        let second_type = self.get_js_type(compiler, second);
        self.traverse_destructuring_pattern(
            compiler,
            first,
            scope,
            Some(second_type),
            AssignmentType::DECLARATION,
        )
    }

    // port: TypeInference#traverseDestructuringPattern
    /// Traverses a destructuring pattern in an assignment or declaration
    fn traverse_destructuring_pattern(
        &mut self,
        compiler: &mut AbstractCompiler,
        pattern: NodeId,
        scope: FS,
        pattern_type: Option<TypeId>,
        assignment_type: AssignmentType,
    ) -> FS {
        self.traverse_destructuring_pattern_helper(
            compiler,
            pattern,
            scope,
            pattern_type,
            &mut |ti: &mut TypeInference<'_>,
                  compiler: &mut AbstractCompiler,
                  flow_scope: FS,
                  target_node: NodeId,
                  target_type: Option<TypeId>| {
                let target_type = match target_type {
                    Some(t) => t,
                    None => ti.get_native_type(compiler, JSTypeNative::UNKNOWN_TYPE),
                };
                ti.update_scope_for_assignment(
                    compiler,
                    flow_scope,
                    target_node,
                    Some(target_type),
                    assignment_type,
                )
            },
        )
    }

    // port: TypeInference#traverseDestructuringPatternHelper
    /// Traverses a destructuring pattern, and calls `declarer.declareTypeInScope` on each lvalue
    ///
    /// The purpose of the callback is to abstract different logic for declaring lvalues in
    /// function parameters vs. in regular assignments/declarations.
    ///
    /// `declarer` contains a callback called on every lvalue.
    fn traverse_destructuring_pattern_helper(
        &mut self,
        compiler: &mut AbstractCompiler,
        pattern: NodeId,
        mut scope: FS,
        pattern_type: Option<TypeId>,
        declarer: &mut dyn TypeDeclaringCallback,
    ) -> FS {
        check_argument!(
            pattern.is_destructuring_pattern(compiler),
            "%s",
            pattern.to_string(compiler)
        );
        let pattern_type = check_not_null!(pattern_type);
        for target in DestructuredTarget::create_all_non_empty_targets_in_pattern(
            compiler,
            Some(pattern_type),
            pattern,
        ) {
            // The computed property is always evaluated first.
            if target.has_computed_property(compiler) {
                let computed = target
                    .get_computed_property(compiler)
                    .unwrap()
                    .get_first_child(compiler)
                    .unwrap();
                scope = self.traverse(compiler, computed, scope);
            }
            let target_node = target.get_node();

            if target_node.is_destructuring_pattern(compiler) {
                if target.has_default_value() {
                    let default_value = target.get_default_value().unwrap();
                    let _ = self.traverse(compiler, default_value, scope.clone());
                }

                // traverse into nested patterns
                let target_type = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    target.infer_type(reg, ast)
                };
                scope = self.traverse_destructuring_pattern_helper(
                    compiler,
                    target_node,
                    scope,
                    Some(target_type),
                    declarer,
                );
            } else {
                scope = self.traverse(compiler, target_node, scope);

                if target.has_default_value() {
                    // TODO(lharker): what do we do with the inferred slots in the scope?
                    // throw them away or join them with the previous scope?
                    let default_value = target.get_default_value().unwrap();
                    let _ = self.traverse(compiler, default_value, scope.clone());
                }

                // declare in the scope
                let inferred = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    target.infer_type(reg, ast)
                };
                scope = declarer.declare_type_in_scope(
                    self,
                    compiler,
                    scope,
                    target_node,
                    Some(inferred),
                );
            }
        }
        // put the `inferred type` of a pattern on it, to make it easier to do typechecking
        pattern.set_jstype(compiler, Some(pattern_type));
        scope
    }

    // port: TypeInference#traverseName
    fn traverse_name(&mut self, compiler: &mut AbstractCompiler, n: NodeId, mut scope: FS) -> FS {
        let var_name = n.get_string(compiler);
        let value = n.get_first_child(compiler);
        let mut type_ = n.get_jstype(compiler);
        if let Some(value) = value {
            // The only case where `value` isn't null is when we are in a name
            // declaration/initialization
            //     var x = 3;
            scope = self.traverse(compiler, value, scope);
            let value_type = self.get_js_type(compiler, value);
            return self.update_scope_for_assignment(
                compiler,
                scope,
                n,
                Some(value_type),
                AssignmentType::DECLARATION,
            );
        }

        let parent = n.get_parent(compiler).unwrap();
        if NodeUtil::is_name_declaration(compiler, Some(parent))
            && Self::is_possible_mixin_application(compiler, n, /* rvalue= */ None)
        {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            Self::add_missing_interface_properties(reg, ast, type_);
        }

        if parent.is_let(compiler) {
            // Whenever we see a LET, we're guaranteed it's not yet in the scope, and we don't need
            // to worry about it being from an outer scope.  In this case, it has no child, so the
            // actual type should be undefined, but we make a special allowance for type-annotated
            // variables. In that case, we use the annotated type instead.
            // TODO(sdh): I would have thought that #updateScopeForTypeChange would handle using
            // the declared type correctly, but for some reason it doesn't so we handle it here.
            let result_type = match type_ {
                Some(t) => t,
                None => self.get_native_type(compiler, JSTypeNative::VOID_TYPE),
            };
            scope = self.update_scope_for_assignment(
                compiler,
                scope,
                n,
                Some(result_type),
                AssignmentType::DECLARATION,
            );
            type_ = Some(result_type);
        } else {
            let var = scope.get_slot(compiler, &var_name);
            if let Some(var) = var {
                // There are two situations where we don't want to use type information
                // from the scope, even if we have it.

                // 1) The var is escaped and assigned in an inner scope, e.g.,
                // function f() { var x = 3; function g() { x = null } (x); }
                let is_inferred = var.is_type_inferred(compiler);
                let unflowable = is_inferred && {
                    let declared = Self::get_declared_var(compiler, &scope, var_name.clone());
                    self.is_unflowable_opt(compiler, declared)
                };

                // 2) We're reading type information from another scope for an
                // inferred variable. That variable is assigned more than once,
                // and we can't know which type we're getting.
                //
                // var t = null; function f() { (t); } doStuff(); t = {};
                //
                // Notice that this heuristic isn't perfect. For example, you might
                // have:
                //
                // function f() { (t); } f(); var t = 3;
                //
                // In this case, we would infer the first reference to t as
                // type {number}, even though it's undefined.
                let maybe_outer_var = if is_inferred && self.container_scope.is_local(compiler) {
                    let parent_scope = self.container_scope.get_parent(compiler).unwrap();
                    parent_scope.get_var(compiler, var_name.clone())
                } else {
                    None
                };
                // Java's `var.equals(maybeOuterVar)`: a TypedVar compares as a ScopedName, an
                // overlay slot by identity.
                let non_local_inferred_slot = match (&var, maybe_outer_var) {
                    (FlowSlot::Var(v), Some(outer)) => {
                        v.equals(compiler, outer)
                            && !outer.is_marked_assigned_exactly_once(compiler)
                    }
                    _ => false,
                };

                if !unflowable && !non_local_inferred_slot {
                    type_ = var.get_type(compiler);
                    if type_.is_none() {
                        type_ = Some(self.unknown_type);
                    }
                }
            }
        }
        n.set_jstype(compiler, type_);
        scope
    }

    // port: TypeInference#traverseImportSpec
    fn traverse_import_spec(
        &mut self,
        compiler: &mut AbstractCompiler,
        mut scope: FS,
        spec: NodeId,
    ) -> FS {
        let exported_name = spec.get_first_child(compiler).unwrap();
        let local_name = spec.get_second_child(compiler).unwrap();

        scope = self.traverse(compiler, local_name, scope);

        let local_type = local_name.get_jstype(compiler);
        exported_name.set_jstype(compiler, local_type);
        scope
    }

    // port: TypeInference#getBigIntPresence
    /// Utility function for determining the BigIntPresence for any type.
    pub fn get_big_int_presence(compiler: &mut AbstractCompiler, type_: TypeId) -> BigIntPresence {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        Self::get_big_int_presence_in(reg, ast, type_)
    }

    /// Rust-only: `getBigIntPresence` on a split-borrowed registry and arena.
    fn get_big_int_presence_in(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
    ) -> BigIntPresence {
        // Base case
        if type_.is_only_big_int(reg) {
            return BigIntPresence::ALL_BIGINT;
        }

        // Checking enum case
        let type_as_enum_element = type_.to_maybe_enum_element_type(reg);
        if let Some(type_as_enum_element) = type_as_enum_element {
            // No matter what type the enum element is, this function can resolve it to a
            // BigIntPresence
            let primitive = type_as_enum_element.get_primitive_type(reg);
            return Self::get_big_int_presence_in(reg, ast, primitive);
        }

        // Union case
        let type_as_union = type_.to_maybe_union_type(reg);
        if let Some(type_as_union) = type_as_union {
            let mut contains_big_int = false;
            let mut contains_number = false;
            let mut contains_other = false;
            for &alternate in type_as_union.get_alternates(reg, ast).iter() {
                if Self::get_big_int_presence_in(reg, ast, alternate) != BigIntPresence::NO_BIGINT {
                    contains_big_int = true;
                } else if alternate.is_number(reg, ast) && !alternate.is_unknown_type(reg, ast) {
                    contains_number = true;
                } else {
                    contains_other = true;
                }
            }
            if contains_big_int {
                if contains_other {
                    return BigIntPresence::BIGINT_OR_OTHER;
                } else if contains_number {
                    return BigIntPresence::BIGINT_OR_NUMBER;
                } else {
                    return BigIntPresence::ALL_BIGINT;
                }
            }
        }

        // If it’s not a bigint object, bigint value, enum containing either, or a union, then we
        // can safely assume that it’s not a bigint in anyway
        BigIntPresence::NO_BIGINT
    }

    // port: TypeInference#traverseUnaryPlus
    /// Check for BigInt with a unary plus.
    fn traverse_unary_plus(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> FS {
        scope = self.traverse_children(compiler, n, scope); // Find types.
        let first = n.get_first_child(compiler).unwrap();
        let first_type = self.get_js_type(compiler, first);
        let bigint_presence_in_operand = Self::get_big_int_presence(compiler, first_type);
        if bigint_presence_in_operand != BigIntPresence::NO_BIGINT {
            // Unary plus throws an exception when applied to a bigint
            let t = self.get_native_type(compiler, JSTypeNative::NO_TYPE);
            n.set_jstype(compiler, Some(t));
        } else {
            let t = self.get_native_type(compiler, JSTypeNative::NUMBER_TYPE);
            n.set_jstype(compiler, Some(t));
        }
        scope
    }

    // port: TypeInference#traverseBigIntCompatibleUnaryOperator
    /// Traverse unary minus, bitwise NOT, increment, and decrement
    fn traverse_big_int_compatible_unary_operator(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> FS {
        scope = self.traverse_children(compiler, n, scope); // Find types.
        let first = n.get_first_child(compiler).unwrap();
        let first_type = self.get_js_type(compiler, first);
        let native = match Self::get_big_int_presence(compiler, first_type) {
            // BigIntPresence in operand
            BigIntPresence::ALL_BIGINT => JSTypeNative::BIGINT_TYPE,
            BigIntPresence::NO_BIGINT => JSTypeNative::NUMBER_TYPE,
            BigIntPresence::BIGINT_OR_NUMBER | BigIntPresence::BIGINT_OR_OTHER => {
                JSTypeNative::BIGINT_NUMBER
            }
        };
        let t = self.get_native_type(compiler, native);
        n.set_jstype(compiler, Some(t));
        scope
    }

    // port: TypeInference#traverseBigIntCompatibleBinaryOperator
    /// Traverse all binary numeric operations (except for URSH and string concatenation with ADD)
    fn traverse_big_int_compatible_binary_operator(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> FS {
        scope = self.traverse_children(compiler, n, scope); // Find types.
        let first = n.get_first_child(compiler).unwrap();
        let last = n.get_last_child(compiler).unwrap();
        let first_type = self.get_js_type(compiler, first);
        let last_type = self.get_js_type(compiler, last);
        let left_big_int_presence = Self::get_big_int_presence(compiler, first_type);
        let right_big_int_presence = Self::get_big_int_presence(compiler, last_type);
        let native = if left_big_int_presence != right_big_int_presence {
            // disallowed mixture of bigint and non-bigint operands
            JSTypeNative::NO_TYPE
        } else {
            match left_big_int_presence {
                BigIntPresence::NO_BIGINT => JSTypeNative::NUMBER_TYPE,
                BigIntPresence::ALL_BIGINT => JSTypeNative::BIGINT_TYPE,
                BigIntPresence::BIGINT_OR_NUMBER => JSTypeNative::BIGINT_NUMBER,
                // In the case of arithmetic operations, BigInts are only compatible with other
                // BigInts. So if bigint is in a union with anything but number (and even then they
                // both have to be
                // {bigint|number}), then an error is reported.
                BigIntPresence::BIGINT_OR_OTHER => JSTypeNative::NO_TYPE,
            }
        };
        let t = self.get_native_type(compiler, native);
        n.set_jstype(compiler, Some(t));
        scope
    }

    // port: TypeInference#traverseClass
    fn traverse_class(&mut self, compiler: &mut AbstractCompiler, n: NodeId, mut scope: FS) -> FS {
        // The name already has a type applied (from TypedScopeCreator) if it's non-empty, and the
        // members are traversed in the class scope (and in their own function scopes).  But the
        // extends clause and computed property keys are in the outer scope and must be traversed
        // here.
        let second = n.get_second_child(compiler).unwrap();
        scope = self.traverse(compiler, second, scope);
        let class_members = NodeUtil::get_class_members(compiler, n);

        let mut member = class_members.get_first_child(compiler);
        while let Some(m) = member {
            // Computed properties LHS need to happen before any RHS values
            if m.is_computed_prop(compiler) || m.is_computed_field_def(compiler) {
                let first = m.get_first_child(compiler).unwrap();
                scope = self.traverse(compiler, first, scope);
            }
            member = m.get_next(compiler);
        }
        let mut member = class_members.get_first_child(compiler);
        while let Some(m) = member {
            scope = self.traverse_class_member_rhs(compiler, m, scope);
            member = m.get_next(compiler);
        }
        scope
    }

    // port: TypeInference#traverseClassMemberRhs
    fn traverse_class_member_rhs(
        &mut self,
        compiler: &mut AbstractCompiler,
        member: NodeId,
        scope: FS,
    ) -> FS {
        match member.get_token(compiler) {
            Token::MEMBER_FIELD_DEF | Token::COMPUTED_FIELD_DEF => {
                let rhs = Self::get_rhs_of_field(compiler, member);
                if let Some(rhs) = rhs {
                    let computed_field_def_typed_scope =
                        self.scope_creator.create_scope_for_node(compiler, member);
                    let computed_field_def_flow_scope = scope
                        .clone()
                        .with_syntactic_scope(compiler, computed_field_def_typed_scope);
                    let declaration_scope = scope.get_declaration_scope(compiler);
                    let rhs_scope = self
                        .traverse(compiler, rhs, computed_field_def_flow_scope)
                        .with_syntactic_scope(compiler, declaration_scope);
                    if member.is_static_member(compiler) {
                        return rhs_scope;
                    }
                }
                scope
            }
            Token::MEMBER_FUNCTION_DEF
            | Token::COMPUTED_PROP
            | Token::BLOCK
            | Token::GETTER_DEF
            | Token::SETTER_DEF => scope,
            _ => panic!("AssertionError"),
        }
    }

    // port: TypeInference#getRhsOfField
    fn get_rhs_of_field(ast: &Ast, field_node: NodeId) -> Option<NodeId> {
        match field_node.get_token(ast) {
            Token::MEMBER_FIELD_DEF => {
                if field_node.has_one_child(ast) {
                    return field_node.get_first_child(ast);
                }
                None
            }
            Token::COMPUTED_FIELD_DEF => {
                if field_node.has_two_children(ast) {
                    return field_node.get_second_child(ast);
                }
                None
            }
            _ => panic!("AssertionError"),
        }
    }

    // port: TypeInference#traverseArrayLiteral
    /// Traverse each element of the array.
    fn traverse_array_literal(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> FS {
        scope = self.traverse_children(compiler, n, scope);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let array_type = reg.get_native_object_type(JSTypeNative::ARRAY_TYPE);
        let unknown = reg.get_native_type(JSTypeNative::UNKNOWN_TYPE);
        let t = reg.create_templatized_type(ast, array_type, &[unknown]);
        n.set_jstype(compiler, Some(t));
        scope
    }
}

impl TypeInference<'_> {
    // port: TypeInference#traverseObjectLiteral
    fn traverse_object_literal(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> FS {
        let type_ = n.get_jstype(compiler);
        let type_ = check_not_null!(type_);

        let mut spread_operator_seen = false;
        let mut name = n.get_first_child(compiler);
        while let Some(nm) = name {
            spread_operator_seen |= nm.is_spread(compiler);
            scope = self.traverse_children(compiler, nm, scope);
            name = nm.get_next(compiler);
        }

        // Object literals can be reflected on other types.
        // See CodingConvention#getObjectLiteralCast and goog.reflect.object
        // Ignore these types of literals.
        let object_type = object_type::cast(compiler.get_type_registry(), Some(type_));
        let Some(object_type) = object_type else {
            return scope;
        };
        if n.get_boolean_prop(compiler, Prop::REFLECTED_OBJECT)
            || object_type.is_enum_type(compiler.get_type_registry())
        {
            return scope;
        }

        let best_l_value = NodeUtil::get_best_l_value(compiler, n);
        let q_obj_name = NodeUtil::get_best_l_value_name(compiler, best_l_value);
        let mut key = n.get_first_child(compiler);
        while let Some(k) = key {
            key = k.get_next(compiler);
            if k.is_computed_prop(compiler) {
                // Don't define computed properties as inferred properties on the object
                continue;
            }

            if k.is_spread(compiler) {
                // TODO(b/128355893): Do smarter inferrence. There are a lot of potential issues
                // with inference on object-spread, so for now we just give up and say `Object`.
                let t = compiler
                    .get_type_registry()
                    .get_native_type(JSTypeNative::OBJECT_TYPE);
                n.set_jstype(compiler, Some(t));
                break;
            }

            // Java's NodeUtil#getObjectOrClassLitKeyName is never null (it throws instead).
            let member_name = Some(NodeUtil::get_object_or_class_lit_key_name(compiler, k));
            if let Some(member_name) = member_name {
                let raw_value_type = k.get_first_child(compiler).unwrap().get_jstype(compiler);
                let value_type = {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    TypeCheck::get_object_lit_key_type_from_value_type(reg, ast, k, raw_value_type)
                };
                let value_type = value_type.unwrap_or(self.unknown_type);

                // See b/260837012. traverseObjectLiteral() can be invoked multiple times for the
                // same literal. When a spread operator is encountered, the type of the literal is
                // changed to OBJECT_TYPE. A second invocation of this code may then erroneously
                // define new properties on the global OBJECT_TYPE. So, if there are spread
                // operators in the literal and the type has already been propagated to
                // OBJECT_TYPE, do not add newly inferred properties to the type.
                let n_type = n.get_jstype(compiler).unwrap();
                let (reg, ast) = compiler.get_type_registry_and_ast();
                if !spread_operator_seen || !n_type.is_native_object_type(reg) {
                    object_type.define_inferred_property(
                        reg,
                        ast,
                        member_name.clone(),
                        value_type,
                        Some(k),
                    );
                }

                // Do normal flow inference if this is a direct property assignment.
                if let Some(q_obj_name) = &q_obj_name
                    && k.is_string_key(compiler)
                {
                    let q_key_name = q_obj_name.concat(&JsString::from(".")).concat(&member_name);
                    let var = Self::get_declared_var(compiler, &scope, q_key_name.clone());
                    let old_type = var.and_then(|var| var.get_type(compiler));
                    if let Some(var) = var
                        && var.is_type_inferred(compiler)
                    {
                        let new_type = match old_type {
                            None => value_type,
                            Some(old_type) => {
                                let (reg, ast) = compiler.get_type_registry_and_ast();
                                old_type.get_least_supertype(reg, ast, old_type)
                            }
                        };
                        var.set_type(compiler, Some(new_type));
                    }

                    scope = scope.infer_qualified_slot(
                        compiler,
                        k,
                        &q_key_name,
                        Some(old_type.unwrap_or(self.unknown_type)),
                        value_type,
                        false,
                    );
                }
            } else {
                n.set_jstype(compiler, Some(self.unknown_type));
            }
        }
        scope
    }

    // port: TypeInference#traverseAdd
    fn traverse_add(&mut self, compiler: &mut AbstractCompiler, n: NodeId, mut scope: FS) -> FS {
        let left = n.get_first_child(compiler).unwrap();
        let right = left.get_next(compiler).unwrap();
        scope = self.traverse_children(compiler, n, scope);

        let left_type = left.get_jstype(compiler);
        let right_type = right.get_jstype(compiler);

        let mut type_ = self.unknown_type;
        if let (Some(left_type), Some(right_type)) = (left_type, right_type) {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let left_is_unknown = left_type.is_unknown_type(reg, ast);
            let right_is_unknown = right_type.is_unknown_type(reg, ast);
            if (!left_is_unknown && left_type.is_string(reg, ast))
                || (!right_is_unknown && right_type.is_string(reg, ast))
            {
                type_ = reg.get_native_type(JSTypeNative::STRING_TYPE);
            } else if Self::get_big_int_presence_in(reg, ast, left_type)
                != BigIntPresence::NO_BIGINT
                || Self::get_big_int_presence_in(reg, ast, right_type) != BigIntPresence::NO_BIGINT
            {
                return self.traverse_big_int_compatible_binary_operator(compiler, n, scope);
            } else if left_is_unknown || right_is_unknown {
                type_ = self.unknown_type;
            } else if self.is_added_as_number(reg, ast, left_type)
                && self.is_added_as_number(reg, ast, right_type)
            {
                type_ = reg.get_native_type(JSTypeNative::NUMBER_TYPE);
            } else {
                type_ = reg.get_native_type(JSTypeNative::NUMBER_STRING);
            }
        }
        n.set_jstype(compiler, Some(type_));

        if n.is_assign_add(compiler) {
            // TODO(johnlenz): this should not update the type of the lhs as that is use as a
            // input and need to be preserved for type checking.
            // Instead call this overload `updateScopeForAssignment(scope, left, leftType, type,
            // null);`
            scope = self.update_scope_for_assignment(
                compiler,
                scope,
                left,
                Some(type_),
                AssignmentType::ASSIGN,
            );
        }

        scope
    }

    // port: TypeInference#isAddedAsNumber
    fn is_added_as_number(&self, reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> bool {
        type_.is_subtype_of(reg, ast, self.number_addition_supertype)
    }

    // port: TypeInference#traverseHook
    fn traverse_hook(&mut self, compiler: &mut AbstractCompiler, n: NodeId, mut scope: FS) -> FS {
        let condition = n.get_first_child(compiler).unwrap();
        let true_node = condition.get_next(compiler).unwrap();
        let false_node = n.get_last_child(compiler).unwrap();

        // verify the condition
        scope = self.traverse(compiler, condition, scope);

        // reverse abstract interpret the condition to produce two new scopes
        let reverse_interpreter = Arc::clone(&self.reverse_interpreter);
        let true_scope = reverse_interpreter.get_preciser_scope_knowing_condition_outcome(
            compiler,
            condition,
            scope.clone(),
            Outcome::TRUE,
        );
        let false_scope = reverse_interpreter.get_preciser_scope_knowing_condition_outcome(
            compiler,
            condition,
            scope.clone(),
            Outcome::FALSE,
        );

        // traverse the true node with the trueScope
        let _ = self.traverse(compiler, true_node, true_scope);

        // traverse the false node with the falseScope
        let _ = self.traverse(compiler, false_node, false_scope);

        // meet true and false nodes' types and assign
        let true_type = true_node.get_jstype(compiler);
        let false_type = false_node.get_jstype(compiler);
        if let (Some(true_type), Some(false_type)) = (true_type, false_type) {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let t = true_type.get_least_supertype(reg, ast, false_type);
            n.set_jstype(compiler, Some(t));
        } else {
            n.set_jstype(compiler, Some(self.unknown_type));
        }

        scope
    }

    // port: TypeInference#traverseNullishCoalesce
    fn traverse_nullish_coalesce(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> FS {
        check_argument!(n.is_nullish_coalesce(compiler) || n.is_assign_nullish_coalesce(compiler));
        let left = n.get_first_child(compiler).unwrap();
        let right = n.get_last_child(compiler).unwrap();

        scope = self.traverse(compiler, left, scope);

        let reverse_interpreter = Arc::clone(&self.reverse_interpreter);
        let right_scope = reverse_interpreter.get_preciser_scope_knowing_condition_outcome(
            compiler,
            left,
            scope.clone(),
            Outcome::NULLISH,
        );

        let scope_after_traverse_right = self.traverse(compiler, right, right_scope);

        let left_type = left.get_jstype(compiler);
        let right_type = right.get_jstype(compiler);

        let mut type_ = self.unknown_type;
        if let Some(left_type) = left_type {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            if !left_type.is_nullable(reg, ast) && !left_type.is_voidable(reg, ast) {
                type_ = left_type;
            } else if let Some(right_type) = right_type {
                let restricted = left_type.restrict_by_not_null_or_undefined(reg, ast);
                type_ = reg.create_union_type(ast, &[restricted, right_type]);
                scope = self.join(compiler, scope, scope_after_traverse_right);
                // Assignment occurs if lhs is null
                if n.is_assign_nullish_coalesce(compiler) {
                    scope = self.update_scope_for_assignment(
                        compiler,
                        scope,
                        left,
                        Some(type_),
                        AssignmentType::ASSIGN,
                    );
                }
            }
        }
        n.set_jstype(compiler, Some(type_));
        scope
    }

    // port: TypeInference#setCallNodeTypeAfterChildrenTraversed
    /// `n`: A non-constructor function invocation, i.e. CALL or TAGGED_TEMPLATELIT
    fn set_call_node_type_after_children_traversed(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        scope_after_children: FS,
    ) -> FS {
        // Resolve goog.{require,requireType,requireDynamic,forwardDeclare,module.get} calls
        // separately, as they are not normal functions.
        if n.is_call(compiler)
            && NodeUtil::is_expression_result_used(compiler, n) // Don't bother typing calls if the result is unused.
            && ModuleImportResolver::is_goog_module_dependency_call(compiler, Some(n))
        {
            let callee_node = n.get_first_child(compiler).unwrap();
            if callee_node.matches_qualified_name(compiler, GOOG_REQUIREDYNAMIC_NAME) {
                // Result of `goog.requireDynamic('module.name')` is `IThenable<MODULE_TYPE>`
                // IThenable is a supertype of Promise.
                let result_type = self.get_goog_module_dependency_call_result_type(compiler, n);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let t = Promises::wrap_in_i_thenable(reg, ast, check_not_null!(result_type));
                n.set_jstype(compiler, Some(t));
            } else {
                let t = self.get_goog_module_dependency_call_result_type(compiler, n);
                n.set_jstype(compiler, t);
            }
            return scope_after_children;
        }

        let left = n.get_first_child(compiler).unwrap();
        let left_type = self.get_js_type(compiler, left);
        let function_type = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            left_type.restrict_by_not_null_or_undefined(reg, ast)
        };
        let checked_unknown = self.get_native_type(compiler, JSTypeNative::CHECKED_UNKNOWN_TYPE);
        if left.is_super(compiler) {
            // TODO(sdh): This will probably return the super type; might want to return 'this'
            // instead?
            return self.traverse_instantiation(compiler, n, function_type, scope_after_children);
        } else if function_type.is_function_type(compiler.get_type_registry()) {
            let reg = compiler.get_type_registry();
            let fn_type = function_type.to_maybe_function_type(reg).unwrap();
            let return_type = fn_type.get_return_type(reg);
            n.set_jstype(compiler, Some(return_type));
            self.backwards_inference_from_call_site(compiler, n, fn_type, &scope_after_children);
        } else if with_reg(compiler, |reg, ast| {
            function_type.equals(reg, ast, checked_unknown)
        }) {
            n.set_jstype(compiler, Some(checked_unknown));
        } else if left.get_jstype(compiler).is_some_and(|t| {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            t.is_unknown_type(reg, ast)
        }) {
            // TODO(lharker): do we also want to set this to unknown if the left's type is null? We
            // would lose some inference that TypeCheck does when given a null type.
            n.set_jstype(compiler, Some(self.unknown_type));
        }
        scope_after_children
    }

    // port: TypeInference#getGoogModuleDependencyCallResultType
    fn get_goog_module_dependency_call_result_type(
        &mut self,
        compiler: &mut AbstractCompiler,
        call_node: NodeId,
    ) -> Option<TypeId> {
        let module_id = call_node
            .get_second_child(compiler)
            .unwrap()
            .get_string(compiler);
        let module = compiler
            .get_module_map()
            .expect("NullPointerException")
            .get_closure_module(&module_id)
            .cloned();
        // Fall back to the `?` type if the module is unknown to the compiler
        let Some(module) = module else {
            return Some(self.unknown_type);
        };

        let name = self
            .module_import_resolver
            .get_closure_namespace_type_from_call(compiler, call_node);
        if let Some(name) = name {
            let scope_root = name.get_scope_root(compiler);
            let other_module_scope = scope_root.and_then(|root| {
                let mapper = self.scope_creator.get_node_to_scope_mapper();
                mapper(Some(root))
            });
            let other_var = match other_module_scope {
                Some(other_module_scope) => {
                    let name = name.get_name(compiler);
                    other_module_scope.get_slot(compiler, name)
                }
                None => None,
            };
            if let Some(other_var) = other_var {
                return Some(other_var.get_type(compiler).unwrap_or(self.unknown_type));
            }

            if let Some(other_module_scope) = other_module_scope
                && module.metadata().module_type() == ModuleType::GOOG_PROVIDE
            {
                // A "provideAlreadyProvided" is type through the properties of the containing
                // namespace since it is a provided name it is rooted in the global scope.

                // We validated that this is a valid "provided name" above so it is ok to look it
                // up by properties.
                return other_module_scope.get_type_through_namespace(compiler, module_id);
            }
        }

        Some(self.unknown_type)
    }

    // port: TypeInference#tightenTypesAfterAssertions
    fn tighten_types_after_assertions(
        &mut self,
        compiler: &mut AbstractCompiler,
        mut scope: FS,
        call_node: NodeId,
    ) -> FS {
        let left = call_node.get_first_child(compiler).unwrap();
        let first_param = left.get_next(compiler);
        if first_param.is_none() {
            // this may be an assertion call but there are no arguments to assert
            return scope;
        }
        let assertion_function_spec = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            self.assertion_function_lookup
                .lookup_by_callee(ast, reg, left)
        };
        let Some(assertion_function_spec) = assertion_function_spec else {
            // this is not a recognized assertion function
            return scope;
        };
        let asserted_node = assertion_function_spec.get_asserted_arg(compiler, first_param);
        let Some(asserted_node) = asserted_node else {
            return scope;
        };
        let asserted_node_name = asserted_node.get_qualified_name(compiler);

        // Handle assertions that enforce expressions evaluate to true.
        match assertion_function_spec.get_assertion_kind() {
            AssertionKind::TRUTHY => {
                // Handle arbitrary expressions within the assert.
                // e.g. given `assert(typeof x === 'string')`, the resulting scope will infer x to
                // be a string.
                let reverse_interpreter = Arc::clone(&self.reverse_interpreter);
                scope = reverse_interpreter.get_preciser_scope_knowing_condition_outcome(
                    compiler,
                    asserted_node,
                    scope,
                    Outcome::TRUE,
                );
                // Build the result of the assertExpression
                let asserted_type = self.get_js_type(compiler, asserted_node);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let truthy_type = asserted_type.restrict_by_not_null_or_undefined(reg, ast);
                call_node.set_jstype(compiler, Some(truthy_type));
            }
            AssertionKind::MATCHES_RETURN_TYPE => {
                // Handle assertions that enforce expressions match the return type of the function
                let left_type = left.get_jstype(compiler);
                let type_ = self.get_js_type(compiler, asserted_node);
                let (reg, ast) = compiler.get_type_registry_and_ast();
                let call_type = closure_jstype::js_type::to_maybe_function_type(reg, left_type);
                let asserted_type = match call_type {
                    Some(call_type) => call_type.get_return_type(reg),
                    None => self.unknown_type,
                };
                let narrowed =
                    if asserted_type.is_unknown_type(reg, ast) || type_.is_unknown_type(reg, ast) {
                        asserted_type
                    } else {
                        type_.get_greatest_subtype(reg, ast, asserted_type)
                    };
                let differs =
                    asserted_node_name.is_some() && type_.differs_from(reg, ast, narrowed);
                call_node.set_jstype(compiler, Some(narrowed));
                if differs {
                    scope = self.narrow_scope(compiler, scope, asserted_node, narrowed);
                }
            }
        }

        scope
    }

    // port: TypeInference#narrowScope
    fn narrow_scope(
        &mut self,
        compiler: &mut AbstractCompiler,
        scope: FS,
        node: NodeId,
        narrowed: TypeId,
    ) -> FS {
        if node.is_this(compiler) {
            // "this" references don't need to be modeled in the control flow graph.
            return scope;
        }

        if node.is_get_prop(compiler) {
            let qualified_name = node
                .get_qualified_name(compiler)
                .expect("NullPointerException");
            let node_type = self.get_js_type(compiler, node);
            return scope.infer_qualified_slot(
                compiler,
                node,
                &qualified_name,
                Some(node_type),
                narrowed,
                false,
            );
        }
        self.redeclare_simple_var(compiler, scope, node, Some(narrowed))
    }

    // port: TypeInference#backwardsInferenceFromCallSite
    /// We only do forward type inference. We do not do full backwards type inference.
    ///
    /// In other words, if we have, `var x = f(); g(x);` a forward type-inference engine would try
    /// to figure out the type of "x" from the return type of "f". A backwards type-inference engine
    /// would try to figure out the type of "x" from the parameter type of "g".
    ///
    /// However, there are a few special syntactic forms where we do some some half-assed
    /// backwards type-inference, because programmers expect it in this day and age. To take an
    /// example from Java, `List<String> x = Lists.newArrayList();` The Java compiler will be able
    /// to infer the generic type of the List returned by newArrayList().
    ///
    /// In much the same way, we do some special-case backwards inference for JS. Those cases are
    /// enumerated here.
    fn backwards_inference_from_call_site(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut fn_type: TypeId,
        scope: &FS,
    ) {
        let updated_fn_type = self.infer_templated_types_for_call(compiler, n, fn_type, scope);
        if updated_fn_type {
            let first_type = n
                .get_first_child(compiler)
                .unwrap()
                .get_jstype(compiler)
                .unwrap();
            fn_type = first_type
                .to_maybe_function_type(compiler.get_type_registry())
                .expect("NullPointerException");
        }
        self.update_type_of_arguments(compiler, n, fn_type);
        self.update_bind(compiler, n);
    }
}

impl TypeInference<'_> {
    // port: TypeInference#updateBind
    /// When "bind" is called on a function, we infer the type of the returned "bound" function by
    /// looking at the number of parameters in the call site. We also infer the "this" type of the
    /// target, if it's a function expression.
    fn update_bind(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        // Java: compiler.getCodingConvention(), the options' convention or else the compiler's
        // default ClosureCodingConvention (stateless, so a fresh instance is the same convention).
        let convention: Arc<dyn CodingConvention + Send + Sync> = compiler
            .get_options()
            .get_coding_convention()
            .clone()
            .unwrap_or_else(|| Arc::new(ClosureCodingConvention::new()));
        let bind = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            convention.describe_function_bind(ast, Some(reg), n, true)
        };
        let Some(bind) = bind else {
            return;
        };

        let target = bind.target;
        let target_type = self.get_js_type(compiler, target);
        let call_target_fn = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            target_type
                .restrict_by_not_null_or_undefined(reg, ast)
                .to_maybe_function_type(reg)
        };
        let Some(mut call_target_fn) = call_target_fn else {
            return;
        };

        if let Some(this_value) = bind.this_value
            && target.is_function(compiler)
        {
            let this_type = self.get_js_type(compiler, this_value);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            if this_type.to_object_type(reg).is_some()
                && !this_type.is_unknown_type(reg, ast)
                && closure_jstype::function_type::get_type_of_this(call_target_fn, reg)
                    .is_unknown_type(reg, ast)
            {
                let this_object_type = this_type.to_object_type(reg);
                call_target_fn = call_target_fn
                    .to_builder(reg)
                    .with_type_of_this(this_object_type)
                    .with_source_node(None)
                    .build_and_resolve(reg, ast);
                target.set_jstype(compiler, Some(call_target_fn));
            }
        }

        let bound_parameter_count = bind.get_bound_parameter_count(compiler);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let return_type = call_target_fn.get_bind_return_type(
            reg,
            ast,
            // getBindReturnType expects the 'this' argument to be included.
            bound_parameter_count + 1,
        );
        let first = n.get_first_child(compiler).unwrap();
        let first_type = self.get_js_type(compiler, first);
        let bind_type = first_type.to_maybe_function_type(compiler.get_type_registry());
        if let Some(bind_type) = bind_type
            && first != target
        {
            // Update the type of the
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let bind_type = bind_type
                .to_builder(reg)
                .with_return_type(return_type)
                .with_source_node(None)
                .build_and_resolve(reg, ast);
            first.set_jstype(compiler, Some(bind_type));
        }

        n.set_jstype(compiler, Some(return_type));
    }

    // port: TypeInference#updateTypeOfArguments
    /// Performs a limited back-inference on function arguments based on the expected parameter
    /// types.
    ///
    /// Currently this only does back-inference in two cases: it infers the type of function
    /// literal arguments and adds inferred properties to inferred object-typed arguments.
    ///
    /// For example: if someone calls `Promise<string>.prototype.then` with `(result) => ...` then
    /// we infer that the type of the arrow function is `function(string): ?`, and inside the arrow
    /// function body we know that `result` is a string.
    fn update_type_of_arguments(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        fn_type: TypeId,
    ) {
        check_state!(
            NodeUtil::is_invocation(compiler, n),
            "%s",
            n.to_string(compiler)
        );
        let parameters = fn_type.get_parameters(compiler.get_type_registry());
        let mut parameters = parameters.into_iter().peekable();
        if n.is_tagged_template_lit(compiler) {
            // Skip the first parameter because it corresponds to a constructed array of the
            // template lit subs, not an actual AST node, so there's nothing to update.
            if parameters.peek().is_none() {
                // TypeCheck will warn if there is no first parameter. Just bail out here.
                return;
            }
            parameters.next();
        }
        let arguments = NodeUtil::get_invocation_args_as_iterable(compiler, n);
        let mut arguments = arguments.into_iter().peekable();

        // Note: if there are too many or too few arguments, TypeCheck will warn.
        while parameters.peek().is_some() && arguments.peek().is_some() {
            let i_argument = arguments.next().unwrap();
            let i_argument_type = self.get_js_type(compiler, i_argument);

            let i_parameter = parameters.next().unwrap();
            let i_parameter_type = i_parameter.get_jstype();

            Self::infer_property_types_to_match_constraint(
                compiler,
                Some(i_argument_type),
                Some(i_parameter_type),
            );

            // If the parameter to the call is a function expression, propagate the
            // function signature from the call site to the function node.

            // Filter out non-function types (such as null and undefined) as
            // we only care about FUNCTION subtypes here.
            let mut restricted_parameter: Option<TypeId> = None;
            let (reg, ast) = compiler.get_type_registry_and_ast();
            if i_parameter_type.is_union_type(reg) {
                let union = i_parameter_type.to_maybe_union_type(reg).unwrap();
                for &alternative in union.get_alternates(reg, ast).iter() {
                    if alternative.is_function_type(reg) {
                        // There is only one function type per union.
                        restricted_parameter = alternative.to_maybe_function_type(reg);
                        break;
                    }
                }
            } else {
                restricted_parameter = i_parameter_type.to_maybe_function_type(reg);
            }

            if let Some(restricted_parameter) = restricted_parameter
                && i_argument.is_function(compiler)
                && i_argument_type.is_function_type(compiler.get_type_registry())
            {
                let arg_fn_type = i_argument_type
                    .to_maybe_function_type(compiler.get_type_registry())
                    .unwrap();
                let arg_jsdoc = i_argument.get_jsdoc_info(compiler);
                // Treat the parameter & return types of the function as 'declared' if the function
                // has JSDoc with type annotations, or a parameter has inline JSDoc.
                // Note that this does not distinguish between cases where all parameters have JSDoc
                // vs only one parameter has JSDoc.
                let declared = arg_jsdoc.is_some_and(|info| info.contains_declaration())
                    || NodeUtil::function_has_inline_jsdocs(compiler, i_argument);
                let matched =
                    self.match_function(compiler, restricted_parameter, arg_fn_type, declared);
                i_argument.set_jstype(compiler, Some(matched));
            }
        }
    }

    // port: TypeInference#matchFunction
    /// Take the current function type, and try to match the expected function type. This is a
    /// form of backwards-inference, like record-type constraint matching.
    ///
    /// `declared`: Whether the given function type is user-provided as opposed to inferred
    fn match_function(
        &self,
        compiler: &mut AbstractCompiler,
        expected_type: TypeId,
        current_type: TypeId,
        declared: bool,
    ) -> TypeId {
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if declared {
            // If the function was declared but it doesn't have a known "this"
            // but the expected type does, back fill it.
            let current_this = closure_jstype::function_type::get_type_of_this(current_type, reg);
            let expected_this = closure_jstype::function_type::get_type_of_this(expected_type, reg);
            if current_this.is_unknown_type(reg, ast) && !expected_this.is_unknown_type(reg, ast) {
                return current_type
                    .to_builder(reg)
                    .with_type_of_this(expected_this)
                    .build_and_resolve(reg, ast);
            }
        } else {
            // For now, we just make sure the current type has enough
            // arguments to match the expected type, and return the
            // expected type if it does.
            if current_type.get_max_arity(reg) <= expected_type.get_max_arity(reg) {
                return expected_type;
            }
        }
        current_type
    }

    // port: TypeInference#buildTypeVariables
    /// Build the type environment where type transformations will be evaluated. It only considers
    /// the template type variables that do not have a type transformation.
    fn build_type_variables(
        &self,
        reg: &JSTypeRegistry,
        inferred_types: &IndexMap<TypeId, TypeId>,
    ) -> IndexMap<JsString, TypeId> {
        let mut type_vars = IndexMap::<_, _>::default();
        for (&key, &value) in inferred_types {
            // Only add the template type that do not have a type transformation
            if !key.is_type_transformation(reg) {
                type_vars.insert(key.get_reference_name(reg).unwrap(), value);
            }
        }
        type_vars
    }

    // port: TypeInference#evaluateTypeTransformations
    /// This function will evaluate the type transformations associated to the template types
    fn evaluate_type_transformations(
        &self,
        compiler: &mut AbstractCompiler,
        template_types: &[TypeId],
        inferred_types: &IndexMap<TypeId, TypeId>,
        scope: &FS,
    ) -> Option<IndexMap<TypeId, TypeId>> {
        let mut type_vars: Option<IndexMap<JsString, TypeId>> = None;
        let mut result: Option<IndexMap<TypeId, TypeId>> = None;
        let mut ttl_obj: Option<TypeTransformation> = None;

        for &type_ in template_types {
            if type_.is_type_transformation(compiler.get_type_registry()) {
                // Lazy initialization when the first type transformation is found
                if ttl_obj.is_none() {
                    let declaration_scope = scope.get_declaration_scope(compiler);
                    ttl_obj = Some(TypeTransformation::new(
                        declaration_scope.as_static_typed_scope_arc(compiler),
                    ));
                    type_vars = Some(
                        self.build_type_variables(compiler.get_type_registry(), inferred_types),
                    );
                    result = Some(IndexMap::<_, _>::default());
                }
                // Evaluate the type transformation expression using the current
                // known types for the template type variables
                let ttl = type_
                    .get_type_transformation(compiler.get_type_registry())
                    .unwrap();
                let transformed_type =
                    ttl_obj
                        .as_ref()
                        .unwrap()
                        .eval(compiler, ttl, type_vars.clone().unwrap());
                result.as_mut().unwrap().insert(type_, transformed_type);
                // Add the transformed type to the type variables
                let reference_name = type_
                    .get_reference_name(compiler.get_type_registry())
                    .unwrap();
                type_vars
                    .as_mut()
                    .unwrap()
                    .insert(reference_name, transformed_type);
            }
        }
        result
    }

    // port: TypeInference#inferTemplatedTypesForCall
    /// For functions that use template types, specialize the function type for the call target
    /// based on the call-site specific arguments. Specifically, this enables inference to set the
    /// type of any function literal parameters based on these inferred types.
    fn infer_templated_types_for_call(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        fn_type: TypeId,
        scope: &FS,
    ) -> bool {
        let keys: Vec<TypeId> = fn_type
            .get_template_type_map(compiler.get_type_registry())
            .get_template_keys()
            .to_vec();
        if keys.is_empty() {
            return false;
        }

        // Try to infer the template types
        let type_of_this = scope.get_type_of_this(compiler);
        let bindings = {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            InvocationTemplateTypeMatcher::new(reg, ast, fn_type, type_of_this, n).r#match()
        };
        let mut inferred: IndexMap<TypeId, TypeId> = IndexMap::<_, _>::default();
        for &key in &keys {
            inferred.insert(
                key,
                bindings.get(&key).copied().unwrap_or(self.unknown_type),
            );
        }

        // If the inferred type doesn't satisfy the template bound, swap to using the bound. This
        // ensures errors will be reported in type-checking.
        {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            for (k, v) in inferred.iter_mut() {
                let bound = k.get_bound(reg);
                if !v.is_subtype_of(reg, ast, bound) {
                    *v = bound;
                }
            }
        }

        // Try to infer the template types using the type transformations
        let type_transformations =
            self.evaluate_type_transformations(compiler, &keys, &inferred, scope);
        if let Some(type_transformations) = type_transformations {
            inferred.extend(type_transformations);
        }

        // Replace all template types. If we couldn't find a replacement, we
        // replace it with UNKNOWN.
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let bindings: IndexMap<TypeId, Option<TypeId>> =
            inferred.iter().map(|(&k, &v)| (k, Some(v))).collect();
        let mut replacer = TemplateTypeReplacer::for_inference(reg, ast, &bindings);
        let call_target = n.get_first_child(compiler).unwrap();

        let (reg, ast) = compiler.get_type_registry_and_ast();
        let replacement_fn_type = fn_type
            .visit(reg, ast, &mut replacer)
            .to_maybe_function_type(reg);
        let replacement_fn_type = check_not_null!(replacement_fn_type);
        let return_type = replacement_fn_type.get_return_type(reg);
        call_target.set_jstype(compiler, Some(replacement_fn_type));
        n.set_jstype(compiler, Some(return_type));

        replacer.has_made_replacement()
    }

    // port: TypeInference#traverseNew
    fn traverse_new(&mut self, compiler: &mut AbstractCompiler, n: NodeId, mut scope: FS) -> FS {
        scope = self.traverse_children(compiler, n, scope);
        let constructor = n.get_first_child(compiler).unwrap();
        let constructor_type = constructor.get_jstype(compiler);
        self.traverse_instantiation_opt(compiler, n, constructor_type, scope)
    }

    // port: TypeInference#traverseInstantiation
    fn traverse_instantiation(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        ctor_type: TypeId,
        scope: FS,
    ) -> FS {
        self.traverse_instantiation_opt(compiler, n, Some(ctor_type), scope)
    }

    /// Rust-only: `traverseInstantiation` with Java's nullable `ctorType`.
    fn traverse_instantiation_opt(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        ctor_type: Option<TypeId>,
        scope: FS,
    ) -> FS {
        let ctor_type = match ctor_type {
            Some(t)
                if {
                    let (reg, ast) = compiler.get_type_registry_and_ast();
                    !t.is_unknown_type(reg, ast)
                } =>
            {
                t
            }
            _ => {
                n.set_jstype(compiler, Some(self.unknown_type));
                return scope;
            }
        };

        let (reg, ast) = compiler.get_type_registry_and_ast();
        let ctor_type = ctor_type.restrict_by_not_null_or_undefined(reg, ast);

        let mut ctor_fn_type = ctor_type.to_maybe_function_type(reg);
        if ctor_fn_type.is_none() && is_instance_of_function_type(reg, ctor_type) {
            // If ctorType is a NoObjectType, then toMaybeFunctionType will
            // return null. But NoObjectType implements the FunctionType
            // interface, precisely because it can validly construct objects.
            ctor_fn_type = Some(ctor_type);
        }

        let Some(ctor_fn_type) = ctor_fn_type.filter(|t| t.is_constructor(reg)) else {
            n.set_jstype(compiler, Some(self.unknown_type));
            return scope;
        };

        // TODO(nickreid): This probably isn't the right thing to do based on the differences
        // between the `this` parameter between CALL and NEW.
        self.backwards_inference_from_call_site(compiler, n, ctor_fn_type, &scope);

        let reg = compiler.get_type_registry();
        let mut instantiated_type = ctor_fn_type.get_instance_type(reg);
        let object_function_type = reg.get_native_type(JSTypeNative::OBJECT_FUNCTION_TYPE);
        if ctor_fn_type == object_function_type {
            // TODO(b/138617950): Delete this case when `Object` and `Object<?, ?> are sparate.
        } else if with_reg(compiler, |reg, ast| {
            ctor_fn_type.has_any_template_types(reg, ast)
        }) {
            let reg = compiler.get_type_registry();
            let it = instantiated_type.expect("NullPointerException");
            if it.is_templatized_type(reg) {
                instantiated_type =
                    Some(it.to_maybe_templatized_type(reg).unwrap().get_raw_type(reg));
            }
            // If necessary, templatized the instance type based on the the constructor parameters.
            let type_of_this = scope.get_type_of_this(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let inferred_types =
                InvocationTemplateTypeMatcher::new(reg, ast, ctor_fn_type, type_of_this, n)
                    .r#match();
            let inferred: Vec<(TypeId, TypeId)> = inferred_types.into_iter().collect();
            instantiated_type = reg
                .create_templatized_type_from_map(ast, instantiated_type.unwrap(), &inferred)
                .to_maybe_object_type(reg);
        }

        n.set_jstype(
            compiler,
            Some(instantiated_type.unwrap_or(self.unknown_type)),
        );
        scope
    }

    // port: TypeInference#traverseAnd
    fn traverse_and(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        scope: FS,
    ) -> BooleanOutcomePair {
        let left = n.get_first_child(compiler).unwrap();
        let left_outcome = self.traverse_within_short_circuiting_bin_op(compiler, left, scope);
        self.traverse_short_circuiting_bin_op(compiler, n, left, left_outcome)
    }

    // port: TypeInference#traverseChildren
    fn traverse_children(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> FS {
        let mut el = n.get_first_child(compiler);
        while let Some(e) = el {
            scope = self.traverse(compiler, e, scope);
            el = e.get_next(compiler);
        }
        scope
    }

    // port: TypeInference#traverseGetElem
    fn traverse_get_elem(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        original_scope: FS,
    ) -> FS {
        let executed_scope = self.traverse_children(compiler, n, original_scope);
        self.set_get_elem_node_type_after_children_traversed(compiler, n, executed_scope)
    }

    // port: TypeInference#setGetElemNodeTypeAfterChildrenTraversed
    /// Sets the appropriate type on the GETELEM node `n` after its children have been traversed.
    ///
    /// `n` the node that we want to finishTraversing, `scope_after_children` scope after children
    /// are traversed
    fn set_get_elem_node_type_after_children_traversed(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope_after_children: FS,
    ) -> FS {
        check_argument!(n.is_get_elem(compiler) || n.is_opt_chain_get_elem(compiler));
        self.infer_get_elem_type(compiler, n);
        let first = n.get_first_child(compiler).unwrap();
        scope_after_children =
            self.tighten_type_after_dereference(compiler, first, scope_after_children);
        scope_after_children
    }

    // port: TypeInference#inferGetElemType
    fn infer_get_elem_type(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        let first = n.get_first_child(compiler).unwrap();
        let first_type = self.get_js_type(compiler, first);
        let index_key = n.get_last_child(compiler).unwrap();
        let index_type = self.get_js_type(compiler, index_key);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let obj_type = first_type.restrict_by_not_null_or_undefined(reg, ast);

        let inferred_type = if index_type.is_known_symbol_value_type(reg) {
            let index_symbol = index_type.to_maybe_known_symbol_type(reg).unwrap();
            self.dereference_known_symbol_prop(reg, ast, obj_type, index_symbol)
        } else if index_type.is_symbol_value_type(reg) {
            // For now, allow symbols definitions/access on any type. In the future only allow them
            // on the subtypes for which they are defined.
            // TODO(b/77474174): be stricter about accesses for non-well-known symbols
            Some(self.unknown_type)
        } else {
            self.dereference_index_signature(reg, ast, obj_type)
        };
        n.set_jstype(compiler, Some(inferred_type.unwrap_or(self.unknown_type)));
    }

    // port: TypeInference#dereferenceKnownSymbolProp
    fn dereference_known_symbol_prop(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        obj: TypeId,
        index_symbol: TypeId,
    ) -> Option<TypeId> {
        // If this is a union type, then we must extract type arguments from each option.
        let mut argument_types = Some(closure_jstype::union_type::builder());
        let alternates: Vec<TypeId> = if obj.is_union_type(reg) {
            obj.to_maybe_union_type(reg)
                .unwrap()
                .get_alternates(reg, ast)
                .to_vec()
        } else {
            vec![obj]
        };
        let key = PropertyKey::Symbol(index_symbol);
        for option in alternates {
            let Some(option_object) = option.to_maybe_object_type(reg) else {
                // This isn't an array or object, drop out.
                argument_types = None;
                break;
            };

            // Extract the element type and add all options to our set of alternates.
            let property_type = option_object.find_property_type(reg, ast, key.clone());
            let Some(property_type) = property_type else {
                // this union member doesn't have the property. just make it unknown.
                argument_types = None;
                break;
            };
            argument_types
                .as_mut()
                .unwrap()
                .add_alternate(reg, ast, property_type);
        }

        // Unwrap the union if possible, and fail if we had no alternates.
        argument_types.map(|mut builder| builder.build(reg, ast))
    }

    // port: TypeInference#dereferenceIndexSignature
    fn dereference_index_signature(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        obj: TypeId,
    ) -> Option<TypeId> {
        // If this is a union type, then we must extract type arguments from each option.
        let mut argument_types = Some(closure_jstype::union_type::builder());
        let alternates: Vec<TypeId> = if obj.is_union_type(reg) {
            obj.to_maybe_union_type(reg)
                .unwrap()
                .get_alternates(reg, ast)
                .to_vec()
        } else {
            vec![obj]
        };
        for option in alternates {
            let type_map = option.get_template_type_map(reg);
            let object_element_key = reg.get_object_element_key();
            if !type_map.has_template_type(object_element_key) {
                // This isn't an array or object, drop out.
                argument_types = None;
                break;
            }

            // Extract the element type and add all options to our set of alternates.
            let element_type = type_map.get_resolved_template_type(reg, ast, object_element_key);
            argument_types
                .as_mut()
                .unwrap()
                .add_alternate(reg, ast, element_type);
        }

        // Unwrap the union if possible, and fail if we had no alternates.
        argument_types.map(|mut builder| builder.build(reg, ast))
    }

    // port: TypeInference#traverseGetProp
    fn traverse_get_prop(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> FS {
        scope = self.traverse_children(compiler, n, scope);
        self.set_get_prop_node_type_after_children_traversed(compiler, n, scope)
    }

    // port: TypeInference#setGetPropNodeTypeAfterChildrenTraversed
    // Sets the appropriate type on the GETPROP node `n` after its children have been traversed.
    fn set_get_prop_node_type_after_children_traversed(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        scope_after_children: FS,
    ) -> FS {
        check_argument!(n.is_get_prop(compiler) || n.is_opt_chain_get_prop(compiler));
        let obj_node = n.get_first_child(compiler).unwrap();
        let obj_type = obj_node.get_jstype(compiler);
        let prop_name = n.get_string(compiler);
        let t = self.get_property_type(compiler, obj_type, &prop_name, n, &scope_after_children);
        n.set_jstype(compiler, Some(t));
        let first = n.get_first_child(compiler).unwrap();
        self.tighten_type_after_dereference(compiler, first, scope_after_children)
    }

    // port: TypeInference#setOptChainNodeTypeAfterChildrenTraversed
    // Sets the appropriate type on the OptChain node `n` after its children have been traversed.
    fn set_opt_chain_node_type_after_children_traversed(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        scope_after_children: FS,
    ) -> FS {
        match n.get_token(compiler) {
            Token::OPTCHAIN_GETPROP => self.set_get_prop_node_type_after_children_traversed(
                compiler,
                n,
                scope_after_children,
            ),
            Token::OPTCHAIN_GETELEM => self.set_get_elem_node_type_after_children_traversed(
                compiler,
                n,
                scope_after_children,
            ),
            Token::OPTCHAIN_CALL => {
                self.set_call_node_type_after_children_traversed(compiler, n, scope_after_children)
            }
            _ => panic!("Illegal token inside finishTraversingOptChain"),
        }
    }

    // port: TypeInference#traverseOptChain
    /// Traversal requires holding scopes from the unconditional start (lhs of the `?.`) and the
    /// conditional part (rhs of the `?.`), and using the startType to determine the resulting
    /// scope.
    fn traverse_opt_chain(&mut self, compiler: &mut AbstractCompiler, n: NodeId, scope: FS) -> FS {
        check_argument!(NodeUtil::is_opt_chain_node(compiler, n));

        if NodeUtil::is_end_of_opt_chain_segment(compiler, n) {
            // Create new optional chain tracking object and push it onto the stack.
            let start_of_chain = NodeUtil::get_start_of_opt_chain_segment(compiler, n);
            let opt_chain_info = OptChainInfo::new(n, start_of_chain);
            self.opt_chain_array_deque.push_front(opt_chain_info);
        }

        let first = n.get_first_child(compiler).unwrap();
        let lhs_scope = self.traverse(compiler, first, scope);

        if n.is_optional_chain_start(compiler) {
            // Store lhsScope into top-of-stack as unexecuted (unconditional) scope.
            self.opt_chain_array_deque
                .front_mut()
                .expect("NullPointerException")
                .unconditional_scope = Some(lhs_scope.clone());
        }

        // Traverse the remaining children and capture their changes into a new FlowScope var
        // `aboutToExecuteScope`. This FlowScope must be constructed on top of the `lhsScope` and
        // not the original scope `scope`, otherwise changes to outer variable that are preserved in
        // the lhsScope would not be captured in the `aboutToExecuteScope`.
        let mut about_to_execute_scope = lhs_scope;
        let mut next_child = n.get_second_child(compiler);
        while let Some(child) = next_child {
            about_to_execute_scope = self.traverse(compiler, child, about_to_execute_scope);
            next_child = child.get_next(compiler);
        }

        // Assigns the type to `n` assuming the entire chain executes and returns the the executed
        // scope.
        let executed_scope = self.set_opt_chain_node_type_after_children_traversed(
            compiler,
            n,
            about_to_execute_scope,
        );

        // Unlike CALL, the OPTCHAIN_CALL nodes must not remain untyped when left child is untyped.
        if n.get_jstype(compiler).is_none() {
            n.set_jstype(compiler, Some(self.unknown_type));
        }

        if NodeUtil::is_end_of_opt_chain_segment(compiler, n) {
            // Use the startNode's type to selectively join the executed scope with the unexecuted
            // scope, and update the type assigned to `n` in `setXAfterChildrenTraversed()`
            let start_of_chain = NodeUtil::get_start_of_opt_chain_segment(compiler, n);

            // Pop the stack to obtain the current chain.
            let current_chain = self
                .opt_chain_array_deque
                .pop_front()
                .expect("NoSuchElementException");

            // Sanity check that the popped chain correctly corresponds to the current optional
            // chain
            check_state!(current_chain.end_of_chain == n);
            check_state!(current_chain.start_of_chain == start_of_chain);

            let start_node = check_not_null!(start_of_chain.get_first_child(compiler));
            self.update_type_when_end_of_opt_chain(
                compiler,
                n,
                start_node,
                current_chain,
                executed_scope,
            )
        } else {
            // `n` is just an inner node (i.e. not the end of the current chain). Simply return the
            // executedScope.
            executed_scope
        }
    }

    // port: TypeInference#updateTypeWhenEndOfOptChain
    // Uses the startNode's type to selectively join the executed scope with the unexecuted scope,
    // and updates the type assigned to `optChain` in `setXAfterChildrenTraversed()`
    fn update_type_when_end_of_opt_chain(
        &mut self,
        compiler: &mut AbstractCompiler,
        opt_chain: NodeId,
        start_node: NodeId,
        current_chain: OptChainInfo,
        executed_scope: FS,
    ) -> FS {
        let start_type = self.get_js_type(compiler, start_node);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if start_type.is_unknown_type(reg, ast) {
            // Unknown startType: Conditional part may execute. Result type must be unknown.
            opt_chain.set_jstype(compiler, Some(self.unknown_type));
            let unconditional = current_chain
                .unconditional_scope
                .expect("NullPointerException");
            self.join(compiler, executed_scope, unconditional)
        } else if start_type.is_null_type(reg) || start_type.is_void_type(reg) {
            // Conditional part will not execute.
            let void_type = reg.get_native_type(JSTypeNative::VOID_TYPE);
            opt_chain.set_jstype(compiler, Some(void_type));
            current_chain
                .unconditional_scope
                .expect("NullPointerException")
        } else if !start_type.is_nullable(reg, ast) {
            // Conditional part will execute; `optChain` was assigned the right type in
            // `setXAfterChildrenTraversed()`.
            executed_scope
        } else {
            // Nullable start: Conditional part may execute. Need to add a VOID_TYPE to the type
            // assigned to `optChain` within `setXAfterChildrenTraversed()`.
            let opt_chain_type = opt_chain
                .get_jstype(compiler)
                .expect("NullPointerException");
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let void_type = reg.get_native_type(JSTypeNative::VOID_TYPE);
            let union = reg.create_union_type(ast, &[void_type, opt_chain_type]);
            opt_chain.set_jstype(compiler, Some(union));
            let reverse_interpreter = Arc::clone(&self.reverse_interpreter);
            let unexecuted_scope = reverse_interpreter
                .get_preciser_scope_knowing_condition_outcome(
                    compiler,
                    start_node,
                    current_chain
                        .unconditional_scope
                        .expect("NullPointerException"),
                    Outcome::NULLISH,
                );
            self.join(compiler, executed_scope, unexecuted_scope)
        }
    }

    // port: TypeInference#inferPropertyTypesToMatchConstraint
    /// Suppose X is an object with inferred properties. Suppose also that X is used in a way where
    /// it would only type-check correctly if some of those properties are widened. Then we should
    /// be polite and automatically widen X's properties.
    ///
    /// For a concrete example, consider: param x {{prop: (number|undefined)}} function f(x) {}
    /// f({});
    ///
    /// If we give the anonymous object an inferred property of (number|undefined), then this code
    /// will type-check appropriately.
    fn infer_property_types_to_match_constraint(
        compiler: &mut AbstractCompiler,
        type_: Option<TypeId>,
        constraint: Option<TypeId>,
    ) {
        let (Some(type_), Some(constraint)) = (type_, constraint) else {
            return;
        };

        let (reg, ast) = compiler.get_type_registry_and_ast();
        type_.match_constraint(reg, ast, constraint);
    }

    // port: TypeInference#tightenTypeAfterDereference
    /// If we access a property of a symbol, then that symbol is not null or undefined.
    fn tighten_type_after_dereference(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> FS {
        if n.is_qualified_name(compiler) {
            let type_ = self.get_js_type(compiler, n);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let narrowed = type_.restrict_by_not_null_or_undefined(reg, ast);
            if !type_.equals(reg, ast, narrowed) {
                scope = self.narrow_scope(compiler, scope, n, narrowed);
            }
        }
        scope
    }

    // port: TypeInference#getPropertyType
    fn get_property_type(
        &mut self,
        compiler: &mut AbstractCompiler,
        obj_type: Option<TypeId>,
        prop_name: &JsString,
        n: NodeId,
        scope: &FS,
    ) -> TypeId {
        // We often have a couple of different types to choose from for the
        // property. Ordered by accuracy, we have
        // 1) A locally inferred qualified name (which is in the FlowScope)
        // 2) A globally declared qualified name (which is in the FlowScope)
        // 3) A property on the owner type (which is on objType)
        // 4) A name in the type registry (as a last resort)
        let mut property_type: Option<TypeId> = None;
        let mut is_locally_inferred = false;

        // Scopes sometimes contain inferred type info about qualified names.
        let qualified_name = n.get_qualified_name(compiler);
        let var = match &qualified_name {
            Some(qualified_name) => scope.get_slot(compiler, qualified_name),
            None => None,
        };
        if let Some(var) = var {
            let var_type = var.get_type(compiler);
            if let Some(var_type) = var_type {
                let is_declared = !var.is_type_inferred(compiler);
                let declared_var =
                    Self::get_declared_var(compiler, scope, qualified_name.clone().unwrap());
                // Java's `var != getDeclaredVar(scope, qualifiedName)` (object identity).
                is_locally_inferred = !matches!(
                    (&var, declared_var),
                    (FlowSlot::Var(v), Some(d)) if *v == d
                );
                if is_declared || is_locally_inferred {
                    property_type = Some(var_type);
                }
            }
        }

        if property_type.is_none()
            && let Some(obj_type) = obj_type
        {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let found_type = obj_type.find_property_type(reg, ast, prop_name.clone());
            if found_type.is_some() {
                property_type = found_type;
            }
        }

        if property_type.is_none_or(|t| {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            t.is_unknown_type(reg, ast)
        }) && let Some(qualified_name) = &qualified_name
        {
            // If we find this node in the registry, then we can infer its type.
            let declaration_scope = scope.get_declaration_scope(compiler);
            let view = declaration_scope.as_static_typed_scope(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let found = reg.get_type(ast, Some(view), qualified_name.clone());
            let reg_type = object_type::cast(reg, found);
            if let Some(reg_type) = reg_type {
                property_type = reg_type.get_constructor(reg);
            }
        }

        match property_type {
            None => self.unknown_type,
            Some(property_type) => {
                let (reg, ast) = compiler.get_type_registry_and_ast();
                if property_type.equals(reg, ast, self.unknown_type) && is_locally_inferred {
                    // If the type has been checked in this scope,
                    // then use CHECKED_UNKNOWN_TYPE instead to indicate that.
                    reg.get_native_type(JSTypeNative::CHECKED_UNKNOWN_TYPE)
                } else {
                    property_type
                }
            }
        }
    }

    // port: TypeInference#traverseOr
    fn traverse_or(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        scope: FS,
    ) -> BooleanOutcomePair {
        let left = n.get_first_child(compiler).unwrap();
        let left_outcome = self.traverse_within_short_circuiting_bin_op(compiler, left, scope);
        self.traverse_short_circuiting_bin_op(compiler, n, left, left_outcome)
    }

    // port: TypeInference#traverseShortCircuitingBinOpAssignment
    fn traverse_short_circuiting_bin_op_assignment(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        scope: FS,
    ) -> FS {
        let left = n.get_first_child(compiler).unwrap();
        let n_is_and = n.is_assign_and(compiler);
        let left_outcome = self.traverse_within_short_circuiting_bin_op(compiler, left, scope);
        let left_to_boolean_outcomes = left_outcome.to_boolean_outcomes;
        let mut outcome = self.traverse_short_circuiting_bin_op(compiler, n, left, left_outcome);

        let outcome_joined_flow_scope = outcome.get_joined_flow_scope(self, compiler);

        if left_to_boolean_outcomes == BooleanLiteralSet::get(!n_is_and) {
            // Either n is && and lhs has a toBooleanOutcome of false,
            // or n is || and lhs has a toBooleanOutcome of true, so assignment does not occur
            // The scope is otherwise updated according to the result of an assignment.
            return outcome_joined_flow_scope;
        }
        let n_type = n.get_jstype(compiler);
        self.update_scope_for_assignment(
            compiler,
            outcome_joined_flow_scope,
            left,
            n_type,
            AssignmentType::ASSIGN,
        )
    }

    // port: TypeInference#traverseShortCircuitingBinOp
    fn traverse_short_circuiting_bin_op(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        left: NodeId,
        mut left_outcome: BooleanOutcomePair,
    ) -> BooleanOutcomePair {
        check_argument!(
            n.is_and(compiler)
                || n.is_or(compiler)
                || n.is_assign_and(compiler)
                || n.is_assign_or(compiler)
        );
        let n_is_and = n.is_and(compiler) || n.is_assign_and(compiler);
        let right = n.get_last_child(compiler).unwrap();

        // type the left node
        let left_type = left.get_jstype(compiler);

        // reverse abstract interpret the left node to produce the correct
        // scope in which to verify the right node
        let left_token = left.get_token(compiler);
        let outcome_scope =
            left_outcome.get_outcome_flow_scope(self, compiler, left_token, n_is_and);
        let reverse_interpreter = Arc::clone(&self.reverse_interpreter);
        let right_scope = reverse_interpreter.get_preciser_scope_knowing_condition_outcome(
            compiler,
            left,
            outcome_scope,
            Outcome::for_boolean(n_is_and),
        );

        // type the right node
        let mut right_outcome =
            self.traverse_within_short_circuiting_bin_op(compiler, right, right_scope);
        let right_type = right.get_jstype(compiler);

        let mut type_;
        let outcome;
        if let (Some(left_type), Some(right_type)) = (left_type, right_type) {
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let left_type = left_type.get_restricted_type_given_outcome(
                reg,
                ast,
                Outcome::for_boolean(!n_is_and),
            );
            if left_outcome.to_boolean_outcomes == BooleanLiteralSet::get(!n_is_and) {
                // Either n is && and lhs is false, or n is || and lhs is true.
                // Use the restricted left type; the right side never gets evaluated.
                type_ = left_type;
                outcome = left_outcome;
            } else {
                // Use the join of the restricted left type knowing the outcome of the
                // ToBoolean predicate and of the right type.
                type_ = left_type.get_least_supertype(reg, ast, right_type);
                let to_boolean_outcomes = Self::join_boolean_outcomes(
                    n_is_and,
                    left_outcome.to_boolean_outcomes,
                    right_outcome.to_boolean_outcomes,
                );
                let boolean_values = Self::join_boolean_outcomes(
                    n_is_and,
                    left_outcome.boolean_values,
                    right_outcome.boolean_values,
                );
                let left_joined = left_outcome.get_joined_flow_scope(self, compiler);
                let right_joined = right_outcome.get_joined_flow_scope(self, compiler);
                outcome = BooleanOutcomePair::new(
                    to_boolean_outcomes,
                    boolean_values,
                    left_joined,
                    right_joined,
                );
            }
            // Exclude the boolean type if the literal set is empty because a boolean
            // can never actually be returned.
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let boolean_type = reg.get_native_type(JSTypeNative::BOOLEAN_TYPE);
            if outcome.boolean_values == BooleanLiteralSet::EMPTY
                && boolean_type.is_subtype_of(reg, ast, type_)
            {
                // Exclusion only makes sense for a union type.
                if type_.is_union_type(reg) {
                    type_ = type_
                        .to_maybe_union_type(reg)
                        .unwrap()
                        .get_restricted_union(reg, ast, boolean_type);
                }
            }
        } else {
            type_ = self.unknown_type;
            let left_joined = left_outcome.get_joined_flow_scope(self, compiler);
            let right_joined = right_outcome.get_joined_flow_scope(self, compiler);
            outcome = BooleanOutcomePair::new(
                BooleanLiteralSet::BOTH,
                BooleanLiteralSet::BOTH,
                left_joined,
                right_joined,
            );
        }
        n.set_jstype(compiler, Some(type_));
        outcome
    }

    // port: TypeInference#traverseWithinShortCircuitingBinOp
    fn traverse_within_short_circuiting_bin_op(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        mut scope: FS,
    ) -> BooleanOutcomePair {
        match n.get_token(compiler) {
            Token::AND => self.traverse_and(compiler, n, scope),
            Token::OR => self.traverse_or(compiler, n, scope),
            _ => {
                scope = self.traverse(compiler, n, scope);
                let n_type = n.get_jstype(compiler);
                self.new_boolean_outcome_pair(compiler, n_type, scope)
            }
        }
    }

    // port: TypeInference#traverseAwait
    fn traverse_await(
        &mut self,
        compiler: &mut AbstractCompiler,
        await_: NodeId,
        mut scope: FS,
    ) -> FS {
        scope = self.traverse_children(compiler, await_, scope);

        let expr = await_.get_first_child(compiler).unwrap();
        let expr_type = self.get_js_type(compiler, expr);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let t = Promises::get_resolved_type(reg, ast, expr_type);
        await_.set_jstype(compiler, Some(t));

        scope
    }

    // port: TypeInference#traverseDynamicImport
    fn traverse_dynamic_import(
        &mut self,
        compiler: &mut AbstractCompiler,
        dynamic_import: NodeId,
        scope: FS,
    ) -> FS {
        let mut template_type = compiler
            .get_type_registry()
            .get_native_type(JSTypeNative::UNKNOWN_TYPE);

        // If the module specifier is a string, attempt to resolve the module
        let module_map = compiler.get_module_map().cloned();
        let import_specifier = dynamic_import.get_first_child(compiler).unwrap();
        let input_id = NodeUtil::get_input_id(compiler, dynamic_import);
        let input = input_id.and_then(|id| compiler.get_input(&id).cloned());
        if import_specifier.is_string_lit(compiler)
            && let Some(module_map) = module_map
            && let Some(input) = input
        {
            let specifier = import_specifier.get_string(compiler).to_string_lossy();
            let source_file_name = import_specifier.get_source_file_name(compiler);
            let target_path = input.get_path(compiler).resolve_js_module(
                &specifier,
                source_file_name.as_deref(),
                import_specifier.get_lineno(compiler),
                import_specifier.get_charno(compiler),
            );
            // The module loader reported to the compiler (its ErrorHandler) during the resolution.
            compiler.flush_module_errors();
            let target_module = target_path
                .as_ref()
                .and_then(|target_path| module_map.get_module_by_path(target_path).cloned());
            if let Some(target_module) = target_module {
                // TypedScopeCreator ensures that the MODULE_BODY type is the export namespace type
                let script_node = target_module.metadata().root_node();
                let script = script_node.expect("NullPointerException");
                if script.has_one_child(compiler)
                    && script
                        .get_first_child(compiler)
                        .unwrap()
                        .is_module_body(compiler)
                {
                    let export_namespace_type =
                        script_node.and_then(|s| s.get_only_child(compiler).get_jstype(compiler));
                    if let Some(export_namespace_type) = export_namespace_type {
                        template_type = export_namespace_type;
                    }
                } else {
                    // Module transpilation has occurred before type inference so a MODULE_BODY
                    // node no longer exists. Traverse the script to locate the module namespace
                    // variable and retrieve the type from it.
                    let module_name_str = target_path.as_ref().unwrap().to_module_name();
                    let module_name = NodeUtil::find_preorder(
                        compiler,
                        script,
                        &|ast: &Ast, node: NodeId| {
                            node.matches_qualified_name(ast, module_name_str.as_str())
                        },
                        &|_: &Ast, _: NodeId| true,
                    );
                    if let Some(module_name) = module_name
                        && let Some(t) = module_name.get_jstype(compiler)
                    {
                        template_type = t;
                    }
                }
            }
        }

        let (reg, ast) = compiler.get_type_registry_and_ast();
        let promise_type = reg.get_native_object_type(JSTypeNative::PROMISE_TYPE);
        let t = reg.create_templatized_type(ast, promise_type, &[template_type]);
        dynamic_import.set_jstype(compiler, Some(t));

        self.traverse_children(compiler, dynamic_import, scope)
    }
}

/// Rust-only: runs `f` on the compiler's split-borrowed registry and arena.
fn with_reg<R>(
    compiler: &mut AbstractCompiler,
    f: impl FnOnce(&mut JSTypeRegistry, &Ast) -> R,
) -> R {
    let (reg, ast) = compiler.get_type_registry_and_ast();
    f(reg, ast)
}

/// Rust-only: Java's `ctorType instanceof FunctionType` (FunctionType and its subclasses
/// NoObjectType, NoType and NoResolvedType).
fn is_instance_of_function_type(reg: &JSTypeRegistry, t: TypeId) -> bool {
    matches!(
        t.get_type_class(reg),
        JSTypeClass::FUNCTION | JSTypeClass::NO_OBJECT | JSTypeClass::NO | JSTypeClass::NO_RESOLVED
    )
}

impl OptChainInfo {
    // port: TypeInference.OptChainInfo#OptChainInfo
    fn new(end_of_chain: NodeId, start_of_chain: NodeId) -> Self {
        Self {
            end_of_chain,
            start_of_chain,
            unconditional_scope: None,
        }
    }
}

// port: TypeInference.OptChainInfo
struct OptChainInfo {
    end_of_chain: NodeId,
    start_of_chain: NodeId,
    unconditional_scope: Option<FS>,
}
