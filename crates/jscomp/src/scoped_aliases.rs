/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/ScopedAliases.java.

//! Port of `ScopedAliases.java`: process aliases in goog.scope blocks.
#![allow(clippy::collapsible_if)] // Keep Java's control flow.
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    make_declared_names_unique::{ContextualRenamer, MakeDeclaredNamesUnique, TargettedRenamer},
    modules::module_metadata_map::ModuleMetadataMap,
    node_traversal::{Callback, NodeTraversal, ScopedCallback},
    node_util::NodeUtil,
    preprocessor_symbol_table::PreprocessorSymbolTable,
    scope::ScopeId,
    var::VarId,
};
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_not_null, check_state,
    input_id::InputId,
    ir::IR,
    js_string::JsString,
    jsdoc_info::Builder as JSDocInfoBuilder,
    node::{Ast, NodeId},
    qualified_name::QualifiedName,
    token::Token,
};
use std::{
    rc::Rc,
    sync::{Arc, LazyLock, Mutex},
};

/// Name used to denote an scoped function block used for aliasing.
// port: ScopedAliases#SCOPING_METHOD_NAME
pub const SCOPING_METHOD_NAME: &str = "goog.scope";

// port: ScopedAliases#SCOPED_ALIASES_PREFIX
const SCOPED_ALIASES_PREFIX: &str = "$jscomp$scope$";
// LINT.IfChange
// port: ScopedAliases#MISSING_ALIASES_PREFIX
const MISSING_ALIASES_PREFIX: &str = "$$jscomp$missingAlias$";
// LINT.ThenChange(//depot/google3/third_party/java_src/clutz/src/main/java/com/google/javascript/clutz/imports/ImportRenameMapBuilder.java)

// Errors
// port: ScopedAliases#GOOG_SCOPE_MUST_BE_ALONE
pub static GOOG_SCOPE_MUST_BE_ALONE: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_SCOPE_MUST_BE_ALONE",
    "The call to goog.scope must be alone in a single statement.",
);

// port: ScopedAliases#GOOG_SCOPE_MUST_BE_IN_GLOBAL_SCOPE
pub static GOOG_SCOPE_MUST_BE_IN_GLOBAL_SCOPE: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_SCOPE_MUST_BE_IN_GLOBAL_SCOPE",
    "The call to goog.scope must be in the global scope.",
);

// port: ScopedAliases#GOOG_SCOPE_HAS_BAD_PARAMETERS
pub static GOOG_SCOPE_HAS_BAD_PARAMETERS: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_SCOPE_HAS_BAD_PARAMETERS",
    "The call to goog.scope must take only a single parameter.  It must be an anonymous function that itself takes no parameters.",
);

// port: ScopedAliases#GOOG_SCOPE_REFERENCES_THIS
pub static GOOG_SCOPE_REFERENCES_THIS: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_SCOPE_REFERENCES_THIS",
    "The body of a goog.scope function cannot reference 'this'.",
);

// port: ScopedAliases#GOOG_SCOPE_USES_RETURN
pub static GOOG_SCOPE_USES_RETURN: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_SCOPE_USES_RETURN",
    "The body of a goog.scope function cannot use 'return'.",
);

// port: ScopedAliases#GOOG_SCOPE_USES_THROW
pub static GOOG_SCOPE_USES_THROW: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_SCOPE_USES_THROW",
    "The body of a goog.scope function cannot use 'throw'.",
);

// port: ScopedAliases#GOOG_SCOPE_ALIAS_REDEFINED
pub static GOOG_SCOPE_ALIAS_REDEFINED: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_SCOPE_ALIAS_REDEFINED",
    "The alias {0} is assigned a value more than once.",
);

// port: ScopedAliases#GOOG_SCOPE_ALIAS_CYCLE
pub static GOOG_SCOPE_ALIAS_CYCLE: DiagnosticType =
    DiagnosticType::error("JSC_GOOG_SCOPE_ALIAS_CYCLE", "The aliases {0} has a cycle.");

// port: ScopedAliases#GOOG_SCOPE_NON_ALIAS_LOCAL
pub static GOOG_SCOPE_NON_ALIAS_LOCAL: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_SCOPE_NON_ALIAS_LOCAL",
    "The local variable {0} is in a goog.scope and is not an alias.",
);

// port: ScopedAliases#GOOG_SCOPE_INVALID_VARIABLE
pub static GOOG_SCOPE_INVALID_VARIABLE: DiagnosticType = DiagnosticType::error(
    "JSC_GOOG_SCOPE_INVALID_VARIABLE",
    "The variable {0} cannot be declared in this scope",
);

// port: ScopedAliases#GOOG_MODULE_GET
static GOOG_MODULE_GET: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.module.get"));

/// What to do with goog.module.get calls importing an inexistent Closure namespace
// port: ScopedAliases.InvalidModuleGetHandling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum InvalidModuleGetHandling {
    PRESERVE,
    GIVE_UNIQUE_NAME,
}

impl InvalidModuleGetHandling {
    // port: ScopedAliases.InvalidModuleGetHandling#shouldGiveUniqueName
    fn should_give_unique_name(self) -> bool {
        self == InvalidModuleGetHandling::GIVE_UNIQUE_NAME
    }
}

/// Process aliases in goog.scope blocks.
///
/// ```text
/// goog.scope(function() {
///   var dom = goog.dom;
///   var DIV = dom.TagName.DIV;
///
///   dom.createElement(DIV);
/// });
/// ```
///
/// should become
///
/// ```text
/// goog.dom.createElement(goog.dom.TagName.DIV);
/// ```
///
/// The advantage of using goog.scope is that the compiler will *guarantee* the anonymous function
/// will be inlined, even if it can't prove that it's semantically correct to do so.
pub struct ScopedAliases {
    preprocessor_symbol_table: Option<Arc<Mutex<PreprocessorSymbolTable>>>,
    module_metadata_map: Option<Arc<ModuleMetadataMap>>,
    // Java stores the CompilerInput; the input is looked up by its id where it is read.
    unique_id_input: Option<Arc<InputId>>,

    // Java's HashMultiset (written only).
    scoped_alias_names: IndexMap<JsString, usize>,
    closure_namespaces: IndexSet<JsString>,
    invalid_module_get_handling: InvalidModuleGetHandling,
}

impl ScopedAliases {
    // port: ScopedAliases#ScopedAliases
    fn new(
        compiler: &AbstractCompiler,
        preprocessor_symbol_table: Option<Arc<Mutex<PreprocessorSymbolTable>>>,
        closure_namespaces: IndexSet<JsString>,
        invalid_module_get_handling: InvalidModuleGetHandling,
    ) -> Self {
        Self {
            preprocessor_symbol_table,
            module_metadata_map: compiler.get_module_metadata_map().cloned(),
            unique_id_input: None,
            scoped_alias_names: IndexMap::<_, _>::default(),
            closure_namespaces,
            invalid_module_get_handling,
        }
    }

    // port: ScopedAliases#builder
    pub fn builder() -> Builder {
        Builder::new()
    }

    // port: ScopedAliases#isScopedAliases
    pub fn is_scoped_aliases(name: &str) -> bool {
        name.starts_with(SCOPED_ALIASES_PREFIX)
    }
}

/// Rust-only: Java's builder holds the compiler; here `build` receives it.
pub struct Builder {
    preprocessor_symbol_table: Option<Arc<Mutex<PreprocessorSymbolTable>>>,
    module_metadata_map: Option<Arc<ModuleMetadataMap>>,
    invalid_module_get_handling: InvalidModuleGetHandling,
}

impl Builder {
    // port: ScopedAliases.Builder#Builder
    fn new() -> Self {
        Self {
            preprocessor_symbol_table: None,
            module_metadata_map: None,
            invalid_module_get_handling: InvalidModuleGetHandling::PRESERVE,
        }
    }

    // port: ScopedAliases.Builder#setPreprocessorSymbolTable
    pub fn set_preprocessor_symbol_table(
        mut self,
        preprocessor_symbol_table: Option<Arc<Mutex<PreprocessorSymbolTable>>>,
    ) -> Self {
        self.preprocessor_symbol_table = preprocessor_symbol_table;
        self
    }

    // port: ScopedAliases.Builder#setModuleMetadataMap
    pub fn set_module_metadata_map(
        mut self,
        module_metadata_map: Option<Arc<ModuleMetadataMap>>,
    ) -> Self {
        self.module_metadata_map = module_metadata_map;
        self
    }

    /// Configures whether to delete or preserve invalid goog.module get calls that are top-level
    /// aliases in a goog.scope.
    // port: ScopedAliases.Builder#setInvalidModuleGetHandling
    pub fn set_invalid_module_get_handling(
        mut self,
        invalid_module_get_handling: InvalidModuleGetHandling,
    ) -> Self {
        self.invalid_module_get_handling = invalid_module_get_handling;
        self
    }

    // port: ScopedAliases.Builder#build
    pub fn build(self, compiler: &AbstractCompiler) -> ScopedAliases {
        ScopedAliases::new(
            compiler,
            self.preprocessor_symbol_table,
            match &self.module_metadata_map {
                None => IndexSet::<_>::default(),
                Some(module_metadata_map) => module_metadata_map
                    .get_modules_by_goog_namespace()
                    .keys()
                    .cloned()
                    .collect(),
            },
            self.invalid_module_get_handling,
        )
    }
}

impl CompilerPass for ScopedAliases {
    // port: ScopedAliases#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let mut traversal = Traversal::new(self);
        NodeTraversal::traverse(compiler, root, &mut traversal);

        if !traversal.has_errors() {
            // Apply the aliases.
            let mut alias_work_queue: Vec<AliasUsage> = traversal.get_alias_usages().clone();
            while !alias_work_queue.is_empty() {
                let mut new_queue: Vec<AliasUsage> = Vec::new();
                for alias_usage in &alias_work_queue {
                    if alias_usage.references_other_alias(compiler, &traversal.deleted_alias_vars) {
                        new_queue.push(alias_usage.clone());
                    } else {
                        alias_usage.apply_alias(compiler);
                    }
                }

                // Prevent an infinite loop.
                if new_queue.len() == alias_work_queue.len() {
                    let cycle_var = new_queue[0].alias_var;
                    let cycle_var_node = check_not_null!(cycle_var.get_node(compiler));
                    let cycle_var_name = cycle_var.get_name(compiler);
                    let error = JSError::make(
                        compiler,
                        cycle_var_node,
                        &GOOG_SCOPE_ALIAS_CYCLE,
                        &[&cycle_var_name.to_string()],
                    );
                    compiler.report(error);
                    break;
                } else {
                    alias_work_queue = new_queue;
                }
            }

            // Remove the alias definitions.
            for &alias_definition in traversal.get_alias_definitions_to_delete() {
                compiler.report_change_to_enclosing_scope(alias_definition);
                let parent = alias_definition.get_parent(compiler);
                if NodeUtil::is_name_declaration(compiler, parent)
                    && parent.unwrap().has_one_child(compiler)
                {
                    parent.unwrap().detach(compiler);
                } else {
                    alias_definition.detach(compiler);
                }
            }

            // Collapse the scopes.
            for &scope_call in traversal.get_scope_calls() {
                let expression_with_scope_call = scope_call.get_parent(compiler).unwrap();
                let scope_closure_block = scope_call
                    .get_last_child(compiler)
                    .unwrap()
                    .get_last_child(compiler)
                    .unwrap();
                scope_closure_block.detach(compiler);
                expression_with_scope_call.replace_with(compiler, scope_closure_block);
                NodeUtil::mark_functions_deleted(compiler, expression_with_scope_call);
                compiler.report_change_to_enclosing_scope(scope_closure_block);
                NodeUtil::try_merge_block(compiler, scope_closure_block, false);
            }
        }
    }
}

// port: ScopedAliases.AliasUsage (abstract class; its subclasses are the kinds)
#[derive(Debug, Clone)]
struct AliasUsage {
    alias_var: VarId,
    alias_reference: NodeId,
    kind: AliasUsageKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AliasUsageKind {
    // port: ScopedAliases.AliasedNode
    AliasedNode,
    // port: ScopedAliases.AliasedTypeNode
    AliasedTypeNode,
}

impl AliasUsage {
    // port: ScopedAliases.AliasedNode#AliasedNode
    fn aliased_node(alias_var: VarId, alias_reference: NodeId) -> Self {
        Self {
            alias_var,
            alias_reference,
            kind: AliasUsageKind::AliasedNode,
        }
    }

    // port: ScopedAliases.AliasedTypeNode#AliasedTypeNode
    fn aliased_type_node(alias_var: VarId, alias_reference: NodeId) -> Self {
        Self {
            alias_var,
            alias_reference,
            kind: AliasUsageKind::AliasedTypeNode,
        }
    }

    /// Checks to see if this references another alias.
    // port: ScopedAliases.AliasUsage#referencesOtherAlias
    fn references_other_alias(
        &self,
        compiler: &mut AbstractCompiler,
        deleted_alias_vars: &IndexSet<VarId>,
    ) -> bool {
        let alias_definition = self.alias_var.get_initial_value(compiler);
        let qname = get_aliased_namespace(compiler, check_not_null!(alias_definition));
        let dot_index = qname.index_of_char(b'.' as u16);
        let root_name = if dot_index == -1 {
            qname.clone()
        } else {
            qname.substring(0, dot_index as usize)
        };
        let other_alias_var = self
            .alias_var
            .get_scope(compiler)
            .get_own_slot(compiler, root_name);
        other_alias_var.is_some_and(|other_alias_var| {
            !set_contains_var(compiler, deleted_alias_vars, other_alias_var)
        })
    }

    // port: ScopedAliases.AliasUsage#applyAlias
    fn apply_alias(&self, compiler: &mut AbstractCompiler) {
        match self.kind {
            AliasUsageKind::AliasedNode => self.apply_alias_aliased_node(compiler),
            AliasUsageKind::AliasedTypeNode => self.apply_alias_aliased_type_node(compiler),
        }
    }

    // port: ScopedAliases.AliasedNode#applyAlias
    fn apply_alias_aliased_node(&self, compiler: &mut AbstractCompiler) {
        let alias_definition = check_not_null!(self.alias_var.get_initial_value(compiler));
        let replacement = alias_definition.clone_tree(compiler);
        replacement.srcref_tree(compiler, self.alias_reference);
        // Given alias "var Bar = foo.Bar;" here we replace a usage of Bar with foo.Bar.
        // foo is generated and never visible to user. Because of that we should mark all new nodes
        // as non-indexable leaving only Bar indexable.
        // Given that replacement is GETPROP node, prefix is first child. It's also possible that
        // replacement is single-part namespace. Like goog.provide('Foo') in that case replacement
        // won't have children.
        if replacement.has_children(compiler) {
            replacement
                .get_first_child(compiler)
                .unwrap()
                .make_non_indexable_recursive(compiler);
        }
        self.alias_reference.replace_with(compiler, replacement);
        compiler.report_change_to_enclosing_scope(replacement);
    }

    // port: ScopedAliases.AliasedTypeNode#applyAlias
    fn apply_alias_aliased_type_node(&self, compiler: &mut AbstractCompiler) {
        let alias_definition = self.alias_var.get_initial_value(compiler);
        let alias_name = self.alias_var.get_name(compiler);
        let type_name = self.alias_reference.get_string(compiler);
        if type_name.starts_with("$jscomp$scope$") {
            // Already visited.
            return;
        }
        let alias_expanded = get_aliased_namespace(compiler, check_not_null!(alias_definition));
        check_state!(
            type_name.starts_with(&alias_name),
            "%s must start with %s",
            type_name,
            alias_name
        );
        let replacement = alias_expanded.concat(&type_name.substring_from(alias_name.length()));
        self.alias_reference.set_string(compiler, replacement);
    }
}

/// Java's `Set<Var>#contains`: `Var` equality is `ScopedName#equals`.
fn set_contains_var(compiler: &AbstractCompiler, set: &IndexSet<VarId>, var: VarId) -> bool {
    set.contains(&var) || set.iter().any(|v| var.equals(compiler, *v))
}

// port: ScopedAliases#isValidAliasRhs
fn is_valid_alias_rhs(ast: &Ast, rhs: NodeId) -> bool {
    match rhs.get_token(ast) {
        Token::GETPROP => is_valid_alias_rhs(ast, rhs.get_first_child(ast).unwrap()),
        Token::NAME => true,
        Token::CALL => NodeUtil::is_call_to_qualified_name(ast, rhs, &GOOG_MODULE_GET),
        _ => false,
    }
}

// port: ScopedAliases#isAliasDefinition
fn is_alias_definition(ast: &Ast, name_node: NodeId) -> bool {
    if !name_node.has_children(ast) {
        return false;
    }
    let rhs = name_node.get_last_child(ast).unwrap();
    is_valid_alias_rhs(ast, rhs)
}

// port: ScopedAliases#getAliasedNamespace
fn get_aliased_namespace(ast: &Ast, rhs: NodeId) -> JsString {
    match rhs.get_token(ast) {
        Token::GETPROP => get_aliased_namespace(ast, rhs.get_first_child(ast).unwrap())
            .concat(&JsString::from("."))
            .concat(&rhs.get_string(ast)),
        Token::NAME => rhs.get_string(ast),
        Token::CALL => {
            check_state!(
                NodeUtil::is_call_to_qualified_name(ast, rhs, &GOOG_MODULE_GET),
                &rhs.to_string(ast)
            );
            check_state!(rhs.has_two_children(ast), &rhs.to_string(ast));
            rhs.get_last_child(ast).unwrap().get_string(ast)
        }
        _ => panic!("RuntimeException: Invalid alias RHS:{}", rhs.to_string(ast)),
    }
}

// port: ScopedAliases.Traversal
struct Traversal<'p> {
    outer: &'p mut ScopedAliases,

    // The job of this class is to collect these three data sets.
    alias_definitions_to_delete: Vec<NodeId>,

    scope_calls: Vec<NodeId>,

    alias_usages: Vec<AliasUsage>,

    // This map is temporary and cleared for each scope.
    aliases: IndexMap<JsString, VarId>,

    // Also temporary and cleared for each scope.
    injected_decls: IndexSet<NodeId>,

    // Persists across scopes.
    deleted_alias_vars: IndexSet<VarId>,

    // Suppose you create an alias.
    // var x = goog.x;
    // As a side-effect, this means you can shadow the namespace 'goog'
    // in inner scopes. When we inline the namespaces, we have to rename
    // these shadows.
    //
    // Fortunately, we already have a name uniquifier that runs during tree
    // normalization (before optimizations). We run it here on a limited
    // set of variables, but only as a last resort (because this will screw
    // up warning messages downstream).
    forbidden_locals: IndexSet<JsString>,

    has_namespace_shadows: bool,

    has_errors: bool,

    // The body of the function that is passed to goog.scope.
    // Set when the traversal enters the body, and set back to null when it exits.
    scope_function_body: Option<NodeId>,
}

impl<'p> Traversal<'p> {
    fn new(outer: &'p mut ScopedAliases) -> Self {
        Self {
            outer,
            alias_definitions_to_delete: Vec::new(),
            scope_calls: Vec::new(),
            alias_usages: Vec::new(),
            aliases: IndexMap::<_, _>::default(),
            injected_decls: IndexSet::<_>::default(),
            deleted_alias_vars: IndexSet::<_>::default(),
            forbidden_locals: IndexSet::<_>::from_iter([JsString::from("$jscomp")]),
            has_namespace_shadows: false,
            has_errors: false,
            scope_function_body: None,
        }
    }

    // port: ScopedAliases.Traversal#getAliasDefinitionsToDelete
    fn get_alias_definitions_to_delete(&self) -> &Vec<NodeId> {
        &self.alias_definitions_to_delete
    }

    // port: ScopedAliases.Traversal#getAliasUsages
    fn get_alias_usages(&self) -> &Vec<AliasUsage> {
        &self.alias_usages
    }

    // port: ScopedAliases.Traversal#getScopeCalls
    fn get_scope_calls(&self) -> &Vec<NodeId> {
        &self.scope_calls
    }

    // port: ScopedAliases.Traversal#hasErrors
    fn has_errors(&self) -> bool {
        self.has_errors
    }

    /// Returns true if this NodeTraversal is currently within a goog.scope function body
    // port: ScopedAliases.Traversal#inGoogScopeBody
    fn in_goog_scope_body(&self) -> bool {
        self.scope_function_body.is_some()
    }

    /// Returns true if n is the goog.scope function body
    // port: ScopedAliases.Traversal#isGoogScopeFunctionBody
    fn is_goog_scope_function_body(&self, n: Option<NodeId>) -> bool {
        self.in_goog_scope_body() && n == self.scope_function_body
    }

    // port: ScopedAliases.Traversal#isCallToScopeMethod
    fn is_call_to_scope_method(ast: &Ast, n: NodeId) -> bool {
        n.is_call(ast)
            && n.get_first_child(ast)
                .unwrap()
                .matches_qualified_name(ast, SCOPING_METHOD_NAME)
    }

    /// Returns the goog.scope() CALL node containing the scopeRoot, or null if scopeRoot is not in
    /// a goog.scope() call.
    // port: ScopedAliases.Traversal#findScopeMethodCall
    fn find_scope_method_call(ast: &Ast, scope_root: NodeId) -> Option<NodeId> {
        let n = check_not_null!(scope_root.get_grandparent(ast));
        if Self::is_call_to_scope_method(ast, n) {
            return Some(n);
        }
        None
    }

    // port: ScopedAliases.Traversal#reportInvalidVariables
    fn report_invalid_variables(&mut self, t: &mut NodeTraversal<'_>) {
        let scope_root = check_not_null!(t.get_scope_root());
        let enclosing_function_body = check_not_null!(t.get_enclosing_function()).get_last_child(t);
        if self.is_goog_scope_function_body(enclosing_function_body)
            && scope_root.is_block(t)
            && !scope_root.get_parent(t).unwrap().is_function(t)
        {
            let scope = t.get_scope();
            for v in scope.get_var_iterable(t.get_compiler()) {
                let compiler = t.get_compiler();
                let parent = check_not_null!(v.get_name_node(compiler))
                    .get_parent(compiler)
                    .unwrap();
                if NodeUtil::is_function_declaration(compiler, parent) {
                    // Disallow block-scoped function declarations that leak into the goog.scope
                    // function body. Technically they shouldn't leak in ES6 but the browsers don't
                    // agree on that yet.
                    let node = check_not_null!(v.get_node(compiler));
                    let name = v.get_name(compiler);
                    self.report(
                        compiler,
                        node,
                        &GOOG_SCOPE_INVALID_VARIABLE,
                        &[&name.to_string()],
                    );
                }
            }
        }
    }

    // port: ScopedAliases.Traversal#report
    fn report(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        error: &'static DiagnosticType,
        arguments: &[&str],
    ) {
        let error = JSError::make(compiler, n, error, arguments);
        compiler.report(error);
        self.has_errors = true;
    }

    // port: ScopedAliases.Traversal#findAliases
    fn find_aliases(&mut self, compiler: &mut AbstractCompiler, scope: ScopeId) {
        for v in scope.get_var_iterable(compiler) {
            let n = check_not_null!(v.get_node(compiler));
            let parent = n.get_parent(compiler).unwrap();
            // We use isBlock to avoid variables declared in loop headers.
            let is_var = NodeUtil::is_name_declaration(compiler, Some(parent))
                && parent.get_parent(compiler).unwrap().is_block(compiler);
            let is_function_decl = NodeUtil::is_function_declaration(compiler, parent);
            if is_var && is_alias_definition(compiler, n) {
                self.record_alias(compiler, v);
            } else if v.is_bleeding_function(compiler) {
                // Bleeding functions already get a BAD_PARAMETERS error, so just
                // do nothing.
            } else if parent.is_param_list(compiler) {
                // Parameters of the scope function also get a BAD_PARAMETERS
                // error.
            } else if is_var || is_function_decl || NodeUtil::is_class_declaration(compiler, parent)
            {
                let is_hoisted = NodeUtil::is_hoisted_function_declaration(compiler, parent);
                let grandparent = parent.get_parent(compiler).unwrap();
                let value = v.get_initial_value(compiler);
                let var_node;

                // Pull out inline type declaration if present.
                if let Some(n_info) = n.get_jsdoc_info(compiler) {
                    let parent_info = parent.get_jsdoc_info(compiler);
                    let mut builder = JSDocInfoBuilder::maybe_copy_from(parent_info.as_deref());
                    if is_function_decl {
                        // Fix inline return type.
                        builder.record_return_type(n_info.get_type());
                    } else {
                        builder.record_type(n_info.get_type());
                    }
                    parent.set_jsdoc_info(compiler, builder.build());
                    n.set_jsdoc_info(compiler, None);
                }
                // Grab the docinfo before we do any AST manipulation.
                let var_doc_info = v.get_jsdoc_info(compiler);

                let name = n.get_string(compiler);
                *self
                    .outer
                    .scoped_alias_names
                    .entry(name.clone())
                    .or_insert(0) += 1;
                let unique_id_input = check_not_null!(
                    self.outer
                        .unique_id_input
                        .as_ref()
                        .and_then(|input_id| compiler.get_input(input_id))
                        .cloned()
                );
                let unique_id = compiler
                    .get_unique_id_supplier()
                    .get_unique_id(&unique_id_input);

                let global_name = format!("$jscomp$scope${unique_id}${name}");

                // First, we need to free up the function expression (EXPR)
                // to be used in another expression.
                if is_function_decl || NodeUtil::is_class_declaration(compiler, parent) {
                    // Replace "function NAME() { ... }" with "var NAME;".
                    // Replace "class NAME { ... }" with "var NAME;".

                    // We can't keep the local name on the function expression,
                    // because IE is buggy and will leak the name into the global
                    // scope. This is covered in more detail here:
                    // http://wiki.ecmascript.org/lib/exe/fetch.php?id=resources:resources&cache=cache&media=resources:jscriptdeviationsfromes3.pdf
                    //
                    // This will only cause problems if this is a hoisted, recursive
                    // function, and the programmer is using the hoisting.
                    let new_name = if is_function_decl {
                        IR::name(compiler, "")
                    } else {
                        IR::empty(compiler)
                    };
                    new_name.srcref(compiler, n);
                    n.replace_with(compiler, new_name);
                    compiler.report_change_to_enclosing_scope(new_name);

                    var_node = IR::var(compiler, n).srcref(compiler, n);
                    parent.replace_with(compiler, var_node);
                } else {
                    if let Some(value) = value {
                        // If this is a VAR, we can just detach the expression and
                        // the tree will still be valid.
                        value.detach(compiler);
                    }
                    var_node = parent;
                }

                // Add var declarations for name
                // var $jscomp$scope$uniqueId$name = EXPR; or var $jscomp$scope$uniqueId$name;
                let new_decl = NodeUtil::new_var_node(compiler, global_name.as_str(), value)
                    .set_jsdoc_info(compiler, var_doc_info)
                    .srcref_tree_if_missing(compiler, n);

                new_decl.srcref(compiler, n);
                new_decl.set_original_name(compiler, Some(name.clone()));
                if is_hoisted {
                    grandparent.add_child_to_front(compiler, new_decl);
                } else {
                    new_decl.insert_before(compiler, var_node);
                }
                if value.is_some() {
                    compiler.report_change_to_enclosing_scope(new_decl);
                }
                self.injected_decls.insert(new_decl);

                // Rewrite "var name = EXPR;" to "var name = $jscomp$scope$uniqueId$name;"
                let name_node = check_not_null!(v.get_name_node(compiler));
                let new_name_node =
                    NodeUtil::new_name_with_basis(compiler, global_name.as_str(), n, name);
                name_node.add_child_to_front(compiler, new_name_node);
                self.record_alias(compiler, v);
            } else {
                // Do not other kinds of local symbols, like catch params.
                let name = n.get_string(compiler);
                self.report(
                    compiler,
                    n,
                    &GOOG_SCOPE_NON_ALIAS_LOCAL,
                    &[&name.to_string()],
                );
            }
        }
    }

    // port: ScopedAliases.Traversal#recordAlias
    fn record_alias(&mut self, compiler: &mut AbstractCompiler, alias_var: VarId) {
        let initial_value = check_not_null!(alias_var.get_initial_value(compiler));
        self.alias_definitions_to_delete
            .push(check_not_null!(alias_var.get_name_node(compiler)));

        if self
            .outer
            .invalid_module_get_handling
            .should_give_unique_name()
            && self.contains_invalid_goog_module_get(compiler, initial_value)
        {
            // Create a new alias with a unique name, to replace the existing one. Needed for
            // integration with Clutz only: by giving the alias a unique name, Clutz may
            // then map back from the unique name to the original `const alias = goog.module.get(`
            // call.
            let name = alias_var.get_name(compiler);
            let file_name = initial_value.get_source_file_name(compiler);
            let modules_by_path =
                check_not_null!(self.outer.module_metadata_map.as_ref()).get_modules_by_path();
            let metadata = file_name.and_then(|file_name| modules_by_path.get(&file_name).cloned());
            // LINT.IfChange
            let suffix = match metadata {
                Some(metadata) => check_not_null!(metadata.path()).to_module_name(),
                None => "unknownScript".to_string(),
            };
            let new_name = format!("{name}{MISSING_ALIASES_PREFIX}{suffix}");
            // LINT.ThenChange(//depot/google3/third_party/java_src/clutz/src/main/java/com/google/javascript/clutz/imports/ImportRenameMapBuilder.java)
            let new_name_node = IR::name(compiler, new_name.as_str());
            initial_value.replace_with(compiler, new_name_node);
            self.deleted_alias_vars.insert(alias_var);
            self.aliases.insert(name, alias_var);
            return;
        }

        let name = alias_var.get_name(compiler);
        self.aliases.insert(name, alias_var);

        let qualified_name = get_aliased_namespace(compiler, initial_value);
        let root_index = qualified_name.index_of_char(b'.' as u16);
        if root_index != -1 {
            let q_name_root = qualified_name.substring(0, root_index as usize);
            if !self.aliases.contains_key(&q_name_root) {
                self.forbidden_locals.insert(q_name_root);
            }
        }
    }

    /// Returns whether the rhs contains any goog.module.get calls to inexistent namespaces
    // port: ScopedAliases.Traversal#containsInvalidGoogModuleGet
    fn contains_invalid_goog_module_get(&self, ast: &Ast, expression: NodeId) -> bool {
        match expression.get_token(ast) {
            Token::NAME => false,
            Token::GETPROP => {
                self.contains_invalid_goog_module_get(ast, expression.get_first_child(ast).unwrap())
            }
            Token::CALL => {
                let namespace = expression.get_second_child(ast).unwrap().get_string(ast);
                !self.outer.closure_namespaces.contains(&namespace)
            }
            _ => panic!(
                "IllegalStateException: Unrecognized alias rhs {}",
                expression.to_string(ast)
            ),
        }
    }

    /// Find out if there are any local shadows of namespaces.
    // port: ScopedAliases.Traversal#findNamespaceShadows
    fn find_namespace_shadows(&mut self, t: &mut NodeTraversal<'_>) {
        if self.has_namespace_shadows {
            return;
        }

        let scope = t.get_scope();
        for v in scope.get_var_iterable(t.get_compiler()) {
            if self
                .forbidden_locals
                .contains(&v.get_name(t.get_compiler()))
            {
                self.has_namespace_shadows = true;
                return;
            }
        }
    }

    /// Rename any local shadows of namespaces. This should be a very rare occurrence, so only do
    /// this traversal if we know that we need it.
    // port: ScopedAliases.Traversal#renameNamespaceShadows
    fn rename_namespace_shadows(&mut self, t: &mut NodeTraversal<'_>) {
        let scope_root = check_not_null!(t.get_scope_root());
        check_state!(
            NodeUtil::is_function_block(t, scope_root),
            &scope_root.to_string(t)
        );

        if self.has_namespace_shadows {
            let renamer = TargettedRenamer::new(
                ContextualRenamer::new(),
                Rc::new(self.forbidden_locals.clone()),
            );
            for s in &self.forbidden_locals {
                renamer.borrow_mut().add_declared_name(s, false);
            }
            let mut uniquifier = MakeDeclaredNamesUnique::builder()
                .with_renamer(renamer)
                .build();
            NodeTraversal::traverse_scope_roots(
                t.get_compiler(),
                &[scope_root],
                &mut uniquifier,
                true,
            );
        }
    }

    // port: ScopedAliases.Traversal#renameBleedingFunctionName
    fn rename_bleeding_function_name(&mut self, t: &mut NodeTraversal<'_>, fn_name: NodeId) {
        let name = fn_name.get_string(t);
        let input = check_not_null!(t.get_input().cloned());
        let suffix = t
            .get_compiler()
            .get_unique_id_supplier()
            .get_unique_id(&input);

        // port: ScopedAliases.Traversal#renameBleedingFunctionName (anonymous AbstractPostOrderCallback)
        struct RenameCallback {
            name: JsString,
            suffix: String,
            fn_name: NodeId,
        }
        impl Callback for RenameCallback {
            // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
            fn should_traverse(
                &mut self,
                _t: &mut NodeTraversal<'_>,
                _n: NodeId,
                _parent: Option<NodeId>,
            ) -> bool {
                true
            }

            fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
                if n.is_name(t) && n.get_string(t) == self.name && {
                    let scope = t.get_scope();
                    let var = check_not_null!(scope.get_var(t.get_compiler(), self.name.clone()));
                    var.get_node(t.get_compiler()) == Some(self.fn_name)
                } {
                    n.set_string(
                        t,
                        format!("{}$jscomp$scopedAliases${}", self.name, self.suffix),
                    );
                    t.get_compiler().report_change_to_enclosing_scope(n);
                }
            }
        }
        let mut cb = RenameCallback {
            name: name.clone(),
            suffix: suffix.clone(),
            fn_name,
        };
        let scope = t.get_scope();
        let (compiler, scope_creator) = t.get_compiler_and_scope_creator();
        NodeTraversal::builder()
            .set_compiler(compiler)
            .set_callback(&mut cb)
            .set_scope_creator(scope_creator)
            .traverse_at_scope(scope);
        fn_name.set_string(t, format!("{name}$jscomp$scopedAliases${suffix}"));
    }

    // port: ScopedAliases.Traversal#validateScopeCall
    fn validate_scope_call(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: NodeId) {
        if let Some(preprocessor_symbol_table) = &self.outer.preprocessor_symbol_table {
            preprocessor_symbol_table
                .lock()
                .unwrap()
                .add_reference(t, n.get_first_child(t).unwrap());
        }
        if !parent.is_expr_result(t) {
            self.report(t.get_compiler(), n, &GOOG_SCOPE_MUST_BE_ALONE, &[]);
        }
        if t.get_enclosing_function().is_some() {
            self.report(
                t.get_compiler(),
                n,
                &GOOG_SCOPE_MUST_BE_IN_GLOBAL_SCOPE,
                &[],
            );
        }
        if !n.has_two_children(t) {
            // The goog.scope call should have exactly 1 parameter.  The first
            // child is the "goog.scope" and the second should be the parameter.
            self.report(t.get_compiler(), n, &GOOG_SCOPE_HAS_BAD_PARAMETERS, &[]);
        } else {
            let anonymous_fn_node = n.get_second_child(t).unwrap();
            if !anonymous_fn_node.is_function(t)
                || NodeUtil::get_name(t, anonymous_fn_node).is_some()
                || NodeUtil::get_function_parameters(t, anonymous_fn_node).has_children(t)
            {
                self.report(
                    t.get_compiler(),
                    anonymous_fn_node,
                    &GOOG_SCOPE_HAS_BAD_PARAMETERS,
                    &[],
                );
            } else {
                self.scope_calls.push(n);
            }
        }
    }

    // port: ScopedAliases.Traversal#fixTypeNode
    fn fix_type_node(&mut self, compiler: &mut AbstractCompiler, type_node: NodeId) {
        if type_node.is_string_lit(compiler) {
            let name = type_node.get_string(compiler);
            let mut end_index = name.index_of_char(b'.' as u16);
            if end_index == -1 {
                end_index = name.length() as i32;
            }
            let base_name = name.substring(0, end_index as usize);
            let alias_var = self.aliases.get(&base_name).copied();
            if let Some(alias_var) = alias_var {
                self.alias_usages
                    .push(AliasUsage::aliased_type_node(alias_var, type_node));
            }
            // For nodes that are referencing the aliased type, set the original name so it
            // can be accessed later in tools such as the CodePrinter or refactoring tools.
            if compiler.get_options().preserves_detailed_source_info() {
                type_node.set_original_name(compiler, Some(name));
            }
        }
        let mut child = type_node.get_first_child(compiler);
        while let Some(c) = child {
            self.fix_type_node(compiler, c);
            child = c.get_next(compiler);
        }
    }
}

impl Callback for Traversal<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ScopedAliases.Traversal#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        // grab the source file path to generate the unique id
        self.outer.unique_id_input = t.get_input_id();
        if Self::is_call_to_scope_method(t, n) {
            let parent = n.get_parent(t).unwrap();
            self.validate_scope_call(t, n, parent);
        }

        if !self.in_goog_scope_body() {
            return;
        }

        let r#type = n.get_token(t);
        let mut alias_var: Option<VarId> = None;
        if r#type == Token::NAME {
            let name = n.get_string(t);
            let scope = t.get_scope();
            let lexical_var = scope.get_var(t.get_compiler(), name.clone());
            if let Some(lexical_var) = lexical_var {
                if self
                    .aliases
                    .get(&name)
                    .is_some_and(|alias| lexical_var.equals(t.get_compiler(), *alias))
                {
                    alias_var = Some(lexical_var);
                    // For nodes that are referencing the aliased type, set the original name so it
                    // can be accessed later in tools such as the CodePrinter or refactoring tools.
                    if t.get_compiler()
                        .get_options()
                        .preserves_detailed_source_info()
                        && n.is_name(t)
                    {
                        n.set_original_name(t, Some(name));
                    }
                }
            }
        }

        let enclosing_function_body = check_not_null!(t.get_enclosing_function()).get_last_child(t);
        if self.is_goog_scope_function_body(enclosing_function_body) {
            if let Some(alias_var) = alias_var {
                if NodeUtil::is_l_value(t, n) {
                    if alias_var.get_node(t.get_compiler()) == Some(n) {
                        // Return early, to ensure that we don't record this as an alias usage.
                        return;
                    } else {
                        let name = n.get_string(t);
                        self.report(
                            t.get_compiler(),
                            n,
                            &GOOG_SCOPE_ALIAS_REDEFINED,
                            &[&name.to_string()],
                        );
                    }
                }
            }

            match r#type {
                Token::RETURN => self.report(t.get_compiler(), n, &GOOG_SCOPE_USES_RETURN, &[]),
                Token::THIS => self.report(t.get_compiler(), n, &GOOG_SCOPE_REFERENCES_THIS, &[]),
                Token::THROW => self.report(t.get_compiler(), n, &GOOG_SCOPE_USES_THROW, &[]),
                _ => {}
            }
        }

        // If this is a bleeding function expression, like
        // var x = function y() { ... }
        // then old versions of IE declare "y" in the current scope. We don't
        // want the scope unboxing to add "y" to the global scope, so we
        // need to rename it.
        //
        // TODO(moz): Remove this once we stop supporting IE8.
        if NodeUtil::is_bleeding_function_name(t, n) {
            self.rename_bleeding_function_name(t, n);
        }

        // Check if this name points to an alias.
        if let Some(alias_var) = alias_var {
            // Note, to support the transitive case, it's important we don't
            // clone aliasedNode here.  For example,
            // var g = goog; var d = g.dom; d.createElement('DIV');
            // The node in aliasedNode (which is "g") will be replaced in the
            // changes pass above with "goog".  If we cloned here, we'd end up
            // with <code>g.dom.createElement('DIV')</code>.
            self.alias_usages
                .push(AliasUsage::aliased_node(alias_var, n));
        }

        // When we inject declarations, we duplicate jsdoc. Make sure
        // we only process that jsdoc once.
        let info = n.get_jsdoc_info(t);
        if let Some(info) = info {
            if !self.injected_decls.contains(&n) {
                for node in info.get_type_nodes() {
                    self.fix_type_node(t.get_compiler(), node);
                }
            }
        }
    }

    fn as_scoped_callback(&mut self) -> Option<&mut dyn ScopedCallback> {
        Some(self)
    }
}

impl ScopedCallback for Traversal<'_> {
    // port: ScopedAliases.Traversal#enterScope
    fn enter_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if t.in_global_hoist_scope() {
            return;
        }
        let scope_root = check_not_null!(t.get_scope_root());
        let scope_method_call = Self::find_scope_method_call(t, scope_root);
        if let Some(scope_method_call) = scope_method_call {
            let scope = t.get_scope();
            self.find_aliases(t.get_compiler(), scope);
            // Null when the last argument has no children (goog.scope(function() {}, 10)).
            self.scope_function_body = scope_method_call
                .get_last_child(t)
                .unwrap()
                .get_last_child(t);
        }
    }

    // port: ScopedAliases.Traversal#exitScope
    fn exit_scope(&mut self, t: &mut NodeTraversal<'_>) {
        if self.is_goog_scope_function_body(t.get_scope_root()) {
            self.scope_function_body = None;
            self.rename_namespace_shadows(t);
            self.injected_decls.clear();
            self.aliases.clear();
            self.forbidden_locals.clear();
            self.has_namespace_shadows = false;
        } else if self.in_goog_scope_body() {
            // Called on inner scopes within a goog.scope, including both block scopes and
            // function scopes.
            self.find_namespace_shadows(t);
            self.report_invalid_variables(t);
        }
    }
}
