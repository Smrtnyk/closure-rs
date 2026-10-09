/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/CssRenamingMap.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/ProcessClosurePrimitives.java.

#![allow(clippy::collapsible_match)] // Keep Java's if statements inside the switch cases.
use crate::{
    abstract_compiler::AbstractCompiler,
    closure_primitive_errors::{
        INVALID_CLOSURE_CALL_SCOPE_ERROR, NULL_ARGUMENT_ERROR, TOO_MANY_ARGUMENTS_ERROR,
    },
    compiler_pass::CompilerPass,
    css_renaming_map::{CssRenamingMap, Style},
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    modules::module_metadata_map::ModuleMetadata,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    renaming_map::RenamingMap,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_not_null, ir::IR, js_string::JsString, jsdoc_info::JSDocInfo, node::NodeId,
    qualified_name::QualifiedName, token::Token,
};
use std::sync::{Arc, LazyLock};

// port: ProcessClosurePrimitives#EXPECTED_OBJECTLIT_ERROR
pub static EXPECTED_OBJECTLIT_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_EXPECTED_OBJECTLIT_ERROR",
    "method \"{0}\" expected an object literal argument",
);

// port: ProcessClosurePrimitives#EXPECTED_STRING_ERROR
pub static EXPECTED_STRING_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_EXPECTED_STRING_ERROR",
    "method \"{0}\" expected a string argument",
);

// port: ProcessClosurePrimitives#INVALID_STYLE_ERROR
pub static INVALID_STYLE_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_CSS_NAME_MAP_STYLE_ERROR",
    "Invalid CSS name map style {0}",
);

// port: ProcessClosurePrimitives#WEAK_NAMESPACE_TYPE
pub static WEAK_NAMESPACE_TYPE: DiagnosticType = DiagnosticType::warning(
    "JSC_WEAK_NAMESPACE_TYPE",
    "Provided symbol declared with type Object. This is rarely useful. For more information see https://github.com/google/closure-compiler/wiki/A-word-about-the-type-Object",
);

// port: ProcessClosurePrimitives#CLASS_NAMESPACE_ERROR
pub static CLASS_NAMESPACE_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_CLASS_NAMESPACE_ERROR",
    "\"{0}\" cannot be both provided and declared as a class. Try var {0} = class '{'...'}' (metadata {1})",
);

// port: ProcessClosurePrimitives#FUNCTION_NAMESPACE_ERROR
pub static FUNCTION_NAMESPACE_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_FUNCTION_NAMESPACE_ERROR",
    "\"{0}\" cannot be both provided and declared as a function. (metadata {1})",
);

// port: ProcessClosurePrimitives#INVALID_PROVIDE_ERROR
pub static INVALID_PROVIDE_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_PROVIDE_ERROR",
    "\"{0}\" is not a valid {1} qualified name",
);

// port: ProcessClosurePrimitives#NON_STRING_PASSED_TO_SET_CSS_NAME_MAPPING_ERROR
pub static NON_STRING_PASSED_TO_SET_CSS_NAME_MAPPING_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_NON_STRING_PASSED_TO_SET_CSS_NAME_MAPPING_ERROR",
    "goog.setCssNameMapping only takes an object literal with string values",
);

// port: ProcessClosurePrimitives#INVALID_CSS_RENAMING_MAP
pub static INVALID_CSS_RENAMING_MAP: DiagnosticType = DiagnosticType::warning(
    "INVALID_CSS_RENAMING_MAP",
    "Invalid entries in css renaming map: {0}",
);

// port: ProcessClosurePrimitives#BASE_CLASS_ERROR
pub static BASE_CLASS_ERROR: DiagnosticType =
    DiagnosticType::error("JSC_BASE_CLASS_ERROR", "incorrect use of {0}.base: {1}");

// port: ProcessClosurePrimitives#POSSIBLE_BASE_CLASS_ERROR
pub static POSSIBLE_BASE_CLASS_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_POSSIBLE_BASE_CLASS_ERROR",
    "potentially incorrect use of {0}.base: {1}\nNote: if this .base method is not on a Closure subclass, this error is a false positive. Suppress with /** @suppress '{closureClassChecks}' */",
);

// port: ProcessClosurePrimitives#INVALID_FORWARD_DECLARE
pub static INVALID_FORWARD_DECLARE: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_FORWARD_DECLARE",
    "Malformed goog.forwardDeclare",
);

// port: ProcessClosurePrimitives#CLOSURE_CALL_CANNOT_BE_ALIASED_ERROR
pub static CLOSURE_CALL_CANNOT_BE_ALIASED_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_CLOSURE_CALL_CANNOT_BE_ALIASED_ERROR",
    "Closure primitive method {0} may not be aliased",
);

// port: ProcessClosurePrimitives#CLOSURE_CALL_CANNOT_BE_ALIASED_OUTSIDE_MODULE_ERROR
pub static CLOSURE_CALL_CANNOT_BE_ALIASED_OUTSIDE_MODULE_ERROR: DiagnosticType =
    DiagnosticType::error(
        "JSC_CLOSURE_CALL_CANNOT_BE_ALIASED_ERROR",
        "Closure primitive method {0} may not be aliased  outside a module (ES module, CommonJS module, or goog.module)",
    );

// port: ProcessClosurePrimitives#INVALID_RENAME_FUNCTION
pub static INVALID_RENAME_FUNCTION: DiagnosticType =
    DiagnosticType::error("JSC_INVALID_RENAME_FUNCTION", "{0} call is invalid: {1}");

// port: ProcessClosurePrimitives#INVALID_GOOG_WEAK_USAGE_CALL
pub static INVALID_GOOG_WEAK_USAGE_CALL: DiagnosticType =
    DiagnosticType::error("JSC_INVALID_GOOG_WEAK_USAGE", "{0} call is invalid: {1}");

/// The root Closure namespace
// port: ProcessClosurePrimitives#GOOG
pub const GOOG: &str = "goog";

// port: ProcessClosurePrimitives#GOOG_INHERITS
static GOOG_INHERITS: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.inherits"));

/// Java's string conversion of a nullable string argument (MessageFormat and string
/// concatenation print "null").
fn string_value_of(s: Option<JsString>) -> String {
    s.map_or_else(|| "null".to_string(), |s| s.to_string())
}

/// Java's `AbstractCollection#toString` for a list of strings: `[a, b]`.
fn java_list_to_string(items: &[String]) -> String {
    format!("[{}]", items.join(", "))
}

/// Performs some Closure-specific simplifications including rewriting goog.base,
/// goog.addDependency.
///
/// Records forwardDeclared names in the internal compiler state.
pub struct ProcessClosurePrimitives {
    known_closure_subclasses: IndexSet<JsString>,

    closure_modules: IndexMap<JsString, Arc<ModuleMetadata>>,
}

impl ProcessClosurePrimitives {
    // port: ProcessClosurePrimitives#ProcessClosurePrimitives
    pub fn new(compiler: &AbstractCompiler) -> Self {
        let closure_modules = check_not_null!(
            compiler.get_module_metadata_map(),
            "Need to run GatherModuleMetadata"
        )
        .get_modules_by_goog_namespace()
        .clone();
        Self {
            known_closure_subclasses: IndexSet::<_>::default(),
            closure_modules,
        }
    }

    // port: ProcessClosurePrimitives#checkPossibleGoogProvideInit
    fn check_possible_goog_provide_init(
        &mut self,
        compiler: &mut AbstractCompiler,
        namespace: &JsString,
        info: Option<Arc<JSDocInfo>>,
        definition: NodeId,
    ) {
        let Some(info) = info else {
            return;
        };
        if definition.is_from_externs(compiler) {
            return;
        }

        let Some(metadata) = self.closure_modules.get(namespace) else {
            return;
        };
        if !metadata.is_goog_provide() {
            return;
        }

        // Validate that the namespace is not declared as a generic object type.
        let Some(expr) = info.get_type() else {
            return;
        };
        let mut n = expr.get_root();
        if n.get_token(compiler) == Token::BANG {
            n = n.get_first_child(compiler).unwrap();
        }
        if n.is_string_lit(compiler)
            && !n.has_children(compiler) // templated object types are ok.
            && n.get_string_ref(compiler) == "Object"
        {
            compiler.report(JSError::make(
                compiler,
                definition,
                &WEAK_NAMESPACE_TYPE,
                &[],
            ));
        }
    }

    // port: ProcessClosurePrimitives#checkGoogFunctions
    fn check_goog_functions(&mut self, t: &mut NodeTraversal<'_>, call: NodeId) {
        let callee = call.get_first_child(t).unwrap();
        if !callee.is_get_prop(t) {
            return;
        }

        let receiver = callee.get_first_child(t).unwrap();
        if !receiver.is_name(t) || receiver.get_string(t) != GOOG {
            return;
        }

        // For the sake of simplicity, we report code changes
        // when we see a provides/requires, and don't worry about
        // reporting the change when we actually do the replacement.
        let method_name = callee.get_string(t).to_string();
        match method_name.as_str() {
            // Note: inherits is allowed in local scope
            "inherits" => self.process_inherits_call(t.get_compiler(), call),
            "addDependency" => {
                if self.validate_unaliasable_primitive_call(t, call, &method_name) {
                    self.process_add_dependency(t.get_compiler(), call);
                }
            }
            "setCssNameMapping" => {
                let parent = call.get_parent(t).unwrap();
                let _unused = Self::process_set_css_name_mapping(t.get_compiler(), call, parent);
            }
            "forwardDeclare" => {
                if self.validate_primitive_call_with_message(
                    t,
                    call,
                    &method_name,
                    &CLOSURE_CALL_CANNOT_BE_ALIASED_OUTSIDE_MODULE_ERROR,
                ) {
                    self.process_forward_declare(t.get_compiler(), call);
                }
            }
            "weakUsage" => self.validate_weak_usage_call(t.get_compiler(), call),
            _ => {}
        }
    }

    // port: ProcessClosurePrimitives#validateWeakUsageCall
    fn validate_weak_usage_call(&mut self, compiler: &mut AbstractCompiler, call: NodeId) {
        // goog.weakUsage() should have exactly one argument, and it should be a name (possibly
        // qualified).
        let child_count = call.get_child_count(compiler);
        let arg = call.get_second_child(compiler);
        let callee_name = string_value_of(
            call.get_first_child(compiler)
                .unwrap()
                .get_qualified_name(compiler),
        );
        if child_count != 2 {
            let message = format!(
                "should have exactly one argument, not {}",
                child_count.wrapping_sub(1)
            );
            compiler.report(JSError::make(
                compiler,
                call,
                &INVALID_GOOG_WEAK_USAGE_CALL,
                &[&callee_name, &message],
            ));
        }
        if child_count >= 2
            && !call
                .get_second_child(compiler)
                .unwrap()
                .is_qualified_name(compiler)
        {
            let message = format!(
                "argument should be a name or qualified name, not {}",
                arg.unwrap().to_string(compiler)
            );
            compiler.report(JSError::make(
                compiler,
                call,
                &INVALID_GOOG_WEAK_USAGE_CALL,
                &[&callee_name, &message],
            ));
        }
    }

    /// Verifies that a) the call is in the top level of a file and b) the return value is unused
    ///
    /// This method is for primitives that never return a value.
    // port: ProcessClosurePrimitives#validateUnaliasablePrimitiveCall
    fn validate_unaliasable_primitive_call(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        method_name: &str,
    ) -> bool {
        self.validate_primitive_call_with_message(
            t,
            n,
            method_name,
            &CLOSURE_CALL_CANNOT_BE_ALIASED_ERROR,
        )
    }

    /// @param methodName list of primitive types classed together with this one
    /// @param invalidAliasingError which DiagnosticType to emit if this call is aliased. this
    ///     depends on whether the primitive is sometimes aliasiable in a module or never
    ///     aliasable.
    // port: ProcessClosurePrimitives#validatePrimitiveCallWithMessage
    fn validate_primitive_call_with_message(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        method_name: &str,
        invalid_aliasing_error: &'static DiagnosticType,
    ) -> bool {
        // Ignore invalid primitives if we didn't strip module sugar.
        if t.get_compiler().get_options().should_preserve_goog_module() {
            return true;
        }

        if !t.in_global_hoist_scope() && !t.in_module_scope() {
            let compiler = t.get_compiler();
            compiler.report(JSError::make(
                compiler,
                n,
                &INVALID_CLOSURE_CALL_SCOPE_ERROR,
                &[],
            ));
            return false;
        } else if !n.get_parent(t).unwrap().is_expr_result(t) && !t.in_module_scope() {
            // If the call is in the global hoist scope, but the result is used
            let name = format!("{GOOG}.{method_name}");
            let compiler = t.get_compiler();
            compiler.report(JSError::make(compiler, n, invalid_aliasing_error, &[&name]));
            return false;
        }
        true
    }

    // port: ProcessClosurePrimitives#maybeProcessClassBaseCall
    fn maybe_process_class_base_call(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        // Two things must hold for every base call:
        // 1) We must be calling it on "this".
        // 2) We must be calling it on a prototype method of the same name as
        //    the one we're in, OR we must be calling it from a constructor.
        // If both of those things are true, then we can rewrite:
        // <pre>
        // function Foo() {
        //   Foo.base(this);
        // }
        // goog.inherits(Foo, BaseFoo);
        // Foo.prototype.bar = function() {
        //   Foo.base(this, 'bar', 1);
        // };
        // </pre>
        // as the easy-to-optimize:
        // <pre>
        // function Foo() {
        //   BaseFoo.call(this);
        // }
        // goog.inherits(Foo, BaseFoo);
        // Foo.prototype.bar = function() {
        //   Foo.superClass_.bar.call(this, 1);
        // };
        //
        // Most of the logic here is just to make sure the AST's
        // structure is what we expect it to be.

        let call_target = n.get_first_child(compiler).unwrap();
        if !call_target.is_get_prop(compiler) || call_target.get_string_ref(compiler) != "base" {
            return;
        }

        let base_container_node = call_target.get_first_child(compiler).unwrap();
        if !base_container_node.is_unscoped_qualified_name(compiler) {
            // Some unknown "base" method.
            return;
        }
        let base_container = call_target
            .get_first_child(compiler)
            .unwrap()
            .get_qualified_name(compiler)
            .unwrap();

        let enclosing_fn_name_node = Self::get_enclosing_decl_name_node(compiler, n);
        let Some(enclosing_fn_name_node) =
            enclosing_fn_name_node.filter(|e| e.is_unscoped_qualified_name(compiler))
        else {
            // some unknown container method or a MEMBER_FUNCTION_DEF.
            if self.known_closure_subclasses.contains(&base_container) {
                self.report_bad_base_method_use(
                    compiler,
                    n,
                    &base_container.to_string(),
                    "Could not find enclosing method.",
                );
            } else if Self::base_used_in_class(compiler, n) {
                let clazz = NodeUtil::get_enclosing_class(compiler, n).unwrap();
                // TODO(lharker): this check ignores class expressions like "foo.Bar = class
                // extends X {}"
                let first = clazz.get_first_child(compiler).unwrap();
                let second = clazz.get_second_child(compiler).unwrap();
                if (first.is_name(compiler) && first.get_string(compiler) == base_container)
                    || (second.is_name(compiler) && second.get_string(compiler) == base_container)
                {
                    let class_name = first.get_string(compiler).to_string();
                    self.report_bad_base_method_use(
                        compiler,
                        n,
                        &class_name,
                        "base method is not allowed in ES6 class. Use super instead.",
                    );
                }
            }
            return;
        };

        if Self::base_used_in_class(compiler, n) {
            let clazz = NodeUtil::get_enclosing_class(compiler, n);
            let name = string_value_of(NodeUtil::get_best_l_value_name(compiler, clazz));
            self.report_bad_base_method_use(
                compiler,
                n,
                &name,
                "base method is not allowed in ES6 class. Use super instead.",
            );
            return;
        }

        let enclosing_qname = enclosing_fn_name_node.get_qualified_name(compiler).unwrap();
        if enclosing_qname.index_of(".prototype.") < 0 {
            self.rewrite_base_call_in_constructor(
                compiler,
                &enclosing_qname,
                &base_container,
                n,
                enclosing_fn_name_node,
            );
        } else {
            self.rewrite_base_call_in_method(
                compiler,
                Some(&enclosing_qname),
                &base_container,
                n,
                enclosing_fn_name_node,
            );
        }
    }

    // port: ProcessClosurePrimitives#rewriteBaseCallInConstructor
    fn rewrite_base_call_in_constructor(
        &mut self,
        compiler: &mut AbstractCompiler,
        enclosing_qname: &JsString,
        base_container: &JsString,
        n: NodeId,
        enclosing_fn_name_node: NodeId,
    ) {
        let base_container_str = base_container.to_string();
        // Check if this is some other "base" method.
        if enclosing_qname != base_container {
            // Report misuse of "base" methods from other known classes.
            if self.known_closure_subclasses.contains(base_container) {
                let message = format!("Must be used within {base_container_str} methods");
                self.report_bad_base_method_use(compiler, n, &base_container_str, &message);
            }
            return;
        }

        // Determine if this is a class with a "base" method created by
        // goog.inherits.
        let enclosing_parent = enclosing_fn_name_node.get_parent(compiler).unwrap();
        let base_class_node = Self::find_goog_inherits_call(
            compiler,
            if enclosing_parent.is_assign(compiler) {
                enclosing_parent.get_parent(compiler).unwrap()
            } else {
                enclosing_parent
            },
        );
        let Some(base_class_node) = base_class_node else {
            // If there is no "goog.inherits", this might be some other "base" method.
            return;
        };

        // This is the expected method, validate its parameters.
        let callee = n.get_first_child(compiler).unwrap();
        let this_arg = callee.get_next(compiler);
        let Some(this_arg) = this_arg.filter(|a| a.is_this(compiler)) else {
            self.report_bad_base_method_use(
                compiler,
                n,
                &base_container_str,
                "First argument must be 'this'.",
            );
            return;
        };

        // Handle methods.
        let method_name_node = this_arg.get_next(compiler);
        let Some(method_name_node) = method_name_node
            .filter(|m| m.is_string_lit(compiler) && m.get_string_ref(compiler) == "constructor")
        else {
            self.report_bad_base_method_use(
                compiler,
                n,
                &base_container_str,
                "Second argument must be 'constructor'.",
            );
            return;
        };

        // We're good to go.
        let base_class_qname = base_class_node.get_qualified_name(compiler).unwrap();
        let new_callee = NodeUtil::new_qname_with_basis(
            compiler,
            base_class_qname.concat(&JsString::from(".call")),
            callee,
            enclosing_qname.concat(&JsString::from(".base")),
        );
        callee.replace_with(compiler, new_callee);
        method_name_node.detach(compiler);
        compiler.report_change_to_enclosing_scope(n);
    }

    // port: ProcessClosurePrimitives#rewriteBaseCallInMethod
    fn rewrite_base_call_in_method(
        &mut self,
        compiler: &mut AbstractCompiler,
        enclosing_qname: Option<&JsString>,
        base_container: &JsString,
        n: NodeId,
        enclosing_fn_name_node: NodeId,
    ) {
        let base_container_str = base_container.to_string();
        if !self.known_closure_subclasses.contains(base_container) {
            // Can't determine if this is a known "class" that has a known "base" method. Don't
            // rewrite the "base" method call because if it's not a Closure subclass, the
            // rewriting would be wrong. Instead emit an error the user can either suppress or
            // fix.
            let message = format!(
                "base used outside a known Closure subclass, in: {base_container_str}\nIf {base_container_str} is actually a Closure subclass, the compiler is unable to statically determine this. This can happen when calling .base off of some import or other alias of the original Closure subclass, such as from a goog.require in another file. Instead of using .base, rewrite this call as ParentClass.prototype.method.call(this)"
            );
            self.report_possible_bad_base_method_use(compiler, n, &base_container_str, &message);
            return;
        }

        let misuse_of_base = !enclosing_fn_name_node
            .get_first_first_child(compiler)
            .unwrap()
            .matches_qualified_name(compiler, base_container.clone());
        if misuse_of_base {
            // Report misuse of "base" methods from other known classes.
            let message = format!("Must be used within {base_container_str} methods");
            self.report_bad_base_method_use(compiler, n, &base_container_str, &message);
            return;
        }

        // The super class is known.
        let callee = n.get_first_child(compiler).unwrap();
        let this_arg = callee.get_next(compiler);
        let Some(this_arg) = this_arg.filter(|a| a.is_this(compiler)) else {
            self.report_bad_base_method_use(
                compiler,
                n,
                &base_container_str,
                "First argument must be 'this'.",
            );
            return;
        };

        // Handle methods.
        let method_name_node = this_arg.get_next(compiler);
        let Some(method_name_node) = method_name_node.filter(|m| m.is_string_lit(compiler)) else {
            self.report_bad_base_method_use(
                compiler,
                n,
                &base_container_str,
                "Second argument must name a method.",
            );
            return;
        };

        let method_name = method_name_node.get_string(compiler);
        let ending = JsString::from(".prototype.").concat(&method_name);
        let Some(enclosing_qname) = enclosing_qname.filter(|q| q.ends_with(&ending)) else {
            let message = format!("Enclosing method does not match {method_name}");
            self.report_bad_base_method_use(compiler, n, &base_container_str, &message);
            return;
        };

        // We're good to go.
        let class_name = enclosing_fn_name_node
            .get_first_first_child(compiler)
            .unwrap();
        let class_qname = class_name.get_qualified_name(compiler).unwrap();
        let new_callee = NodeUtil::new_qname_with_basis(
            compiler,
            class_qname
                .concat(&JsString::from(".superClass_."))
                .concat(&method_name)
                .concat(&JsString::from(".call")),
            callee,
            enclosing_qname.concat(&JsString::from(".base")),
        );
        callee.replace_with(compiler, new_callee);
        method_name_node.detach(compiler);
        compiler.report_change_to_enclosing_scope(n);
    }

    // port: ProcessClosurePrimitives#findGoogInheritsCall
    fn find_goog_inherits_call(
        compiler: &AbstractCompiler,
        ctor_declaration_node: NodeId,
    ) -> Option<NodeId> {
        let mut maybe_inherits_expr = ctor_declaration_node.get_next(compiler);

        while let Some(expr) = maybe_inherits_expr
            && (expr.is_empty(compiler) || NodeUtil::is_expr_assign(compiler, expr))
        {
            // Skip empty nodes (e.g. those created by an unnecessary semicolon) and potential
            // aliases, e.g. "exports = Ctor;" in a goog.module.
            maybe_inherits_expr = expr.get_next(compiler);
        }
        let mut base_class_node = None;
        if let Some(maybe_inherits_expr) = maybe_inherits_expr
            && NodeUtil::is_expr_call(compiler, maybe_inherits_expr)
        {
            let call_node = maybe_inherits_expr.get_first_child(compiler).unwrap();
            if GOOG_INHERITS.matches(compiler, call_node.get_first_child(compiler).unwrap())
                && call_node
                    .get_last_child(compiler)
                    .unwrap()
                    .is_qualified_name(compiler)
            {
                base_class_node = call_node.get_last_child(compiler);
            }
        }
        base_class_node
    }

    /// Processes the goog.inherits call.
    // port: ProcessClosurePrimitives#processInheritsCall
    fn process_inherits_call(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        if n.has_x_children(compiler, 3) {
            let sub_class = n.get_second_child(compiler).unwrap();
            let super_class = sub_class.get_next(compiler).unwrap();
            if sub_class.is_unscoped_qualified_name(compiler)
                && super_class.is_unscoped_qualified_name(compiler)
            {
                self.known_closure_subclasses
                    .insert(sub_class.get_qualified_name(compiler).unwrap());
            }
        }
    }

    /// Returns the qualified name node of the function whose scope we're in, or null if it
    /// cannot be found.
    // port: ProcessClosurePrimitives#getEnclosingDeclNameNode
    fn get_enclosing_decl_name_node(compiler: &AbstractCompiler, n: NodeId) -> Option<NodeId> {
        let fn_ = NodeUtil::get_enclosing_function(compiler, n);
        match fn_ {
            None => None,
            Some(fn_) => NodeUtil::get_name_node(compiler, fn_),
        }
    }

    /// Verify if goog.base call is used in a class
    // port: ProcessClosurePrimitives#baseUsedInClass
    fn base_used_in_class(compiler: &AbstractCompiler, n: NodeId) -> bool {
        let mut curr = Some(n);
        while let Some(c) = curr {
            if c.is_class_members(compiler) {
                return true;
            }
            curr = c.get_parent(compiler);
        }
        false
    }

    /// Reports an incorrect use of super-method calling.
    // port: ProcessClosurePrimitives#reportBadBaseMethodUse
    fn report_bad_base_method_use(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        class_name: &str,
        extra_message: &str,
    ) {
        compiler.report(JSError::make(
            compiler,
            n,
            &BASE_CLASS_ERROR,
            &[class_name, extra_message],
        ));
    }

    /// Reports a potential incorrect use of super-method calling.
    ///
    /// Use this instead of `reportBadBaseMethodUse` when reporting a potential false positive
    /// that should be suppressible in JS code, instead of a coding pattern that is definitely
    /// bad and is not suppressible.
    // port: ProcessClosurePrimitives#reportPossibleBadBaseMethodUse
    fn report_possible_bad_base_method_use(
        &self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        class_name: &str,
        extra_message: &str,
    ) {
        compiler.report(JSError::make(
            compiler,
            n,
            &POSSIBLE_BASE_CLASS_ERROR,
            &[class_name, extra_message],
        ));
    }

    /// Processes a call to goog.setCssNameMapping(). Either the argument to
    /// goog.setCssNameMapping() is valid, in which case it will be used to create a
    /// CssRenamingMap for the compiler of this CompilerPass, or it is invalid and a JSCompiler
    /// error will be reported.
    // port: ProcessClosurePrimitives#processSetCssNameMapping
    pub fn process_set_css_name_mapping(
        compiler: &mut AbstractCompiler,
        n: NodeId,
        _parent: NodeId,
    ) -> Option<Arc<dyn CssRenamingMap>> {
        let left = n.get_first_child(compiler).unwrap();
        let arg = left.get_next(compiler);
        if !Self::verify_set_css_name_mapping(compiler, left, arg) {
            return None;
        }
        let arg = arg.unwrap();
        // Translate OBJECTLIT into SubstitutionMap. All keys and
        // values must be strings, or an error will be thrown.
        let mut css_names: IndexMap<JsString, JsString> = IndexMap::<_, _>::default();

        let mut key = arg.get_first_child(compiler);
        while let Some(k) = key {
            let value = k.get_first_child(compiler);
            let Some(value) =
                value.filter(|v| k.is_string_key(compiler) && v.is_string_lit(compiler))
            else {
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &NON_STRING_PASSED_TO_SET_CSS_NAME_MAPPING_ERROR,
                    &[],
                ));
                return None;
            };
            css_names.insert(k.get_string(compiler), value.get_string(compiler));
            key = k.get_next(compiler);
        }

        let mut style_str = JsString::from("BY_PART");
        if let Some(next) = arg.get_next(compiler) {
            style_str = next.get_string(compiler);
        }

        // Java: CssRenamingMap.Style.valueOf(styleStr), whose IllegalArgumentException is caught.
        let Some(style) = Style::value_of(&style_str.to_string()) else {
            compiler.report(JSError::make(
                compiler,
                n,
                &INVALID_STYLE_ERROR,
                &[&style_str.to_string()],
            ));
            return None;
        };

        if style == Style::BY_PART {
            // Make sure that no class keys contain -'s
            // Variable keys can contain -'s since they're always named BY_WHOLE.
            let mut errors: Vec<String> = Vec::new();
            let dash = JsString::from("-");
            let double_dash = JsString::from("--");
            for key in css_names.keys() {
                if key.index_of(&dash) >= 0 && !key.starts_with(&double_dash) {
                    errors.push(key.to_string());
                }
            }
            if !errors.is_empty() {
                let errors = java_list_to_string(&errors);
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &INVALID_CSS_RENAMING_MAP,
                    &[&errors],
                ));
            }
        } else if style == Style::BY_WHOLE {
            // Verifying things is a lot trickier here. We just do a quick
            // n^2 check over the map which makes sure that if "a-b" in
            // the map, then map(a-b) = map(a)-map(b).
            // To speed things up, only consider cases where len(b) <= 10
            let mut errors: Vec<String> = Vec::new();
            let dash = JsString::from("-");
            for (b_key, b_value) in &css_names {
                if b_key.length() > 10 {
                    continue;
                }
                for (a_key, a_value) in &css_names {
                    let combined = css_names.get(&a_key.concat(&dash).concat(b_key));
                    if let Some(combined) = combined
                        && *combined != a_value.concat(&dash).concat(b_value)
                    {
                        errors.push(format!("map({a_key}-{b_key}) != map({a_key})-map({b_key})"));
                    }
                }
            }
            if !errors.is_empty() {
                let errors = java_list_to_string(&errors);
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &INVALID_CSS_RENAMING_MAP,
                    &[&errors],
                ));
            }
        }

        Some(Arc::new(SetCssNameMappingRenamingMap { css_names, style }))
    }

    /// Process a goog.addDependency() call and record any forward declarations.
    // port: ProcessClosurePrimitives#processAddDependency
    fn process_add_dependency(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        // We can't modify parent, so just create a node that will
        // get compiled out.
        let empty_node = IR::number(compiler, 0.0);
        n.replace_with(compiler, empty_node);
        compiler.report_change_to_enclosing_scope(empty_node);
    }

    /// Process a goog.forwardDeclare() call and record the specified forward declaration.
    // port: ProcessClosurePrimitives#processForwardDeclare
    fn process_forward_declare(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        if !n.get_parent(compiler).unwrap().is_expr_result(compiler) {
            //  Ignore "const Foo = goog.forwardDeclare('my.Foo');". It's legal but does not
            // actually forward declare the type 'my.Foo'.
            return;
        }
        let convention = compiler.get_coding_convention();

        // Java: Iterables.getOnlyElement(convention.identifyTypeDeclarationCall(n)); the
        // NullPointerException (null list, which the Rust convention returns as an empty list),
        // NoSuchElementException (empty list) and IllegalArgumentException (several elements)
        // are caught and reported.
        let declarations = convention.identify_type_declaration_call(compiler, n);
        let mut type_declaration = None;
        if declarations.len() == 1 {
            type_declaration = declarations.into_iter().next();
        } else {
            compiler.report(JSError::make(
                compiler,
                n,
                &INVALID_FORWARD_DECLARE,
                &["A single type could not identified for the goog.forwardDeclare statement"],
            ));
        }

        if let Some(type_declaration) = type_declaration {
            compiler.forward_declare_type(type_declaration.to_string());
        }
    }

    /// Verifies that setCssNameMapping is called with the correct methods.
    ///
    /// @return Whether the arguments checked out okay
    // port: ProcessClosurePrimitives#verifySetCssNameMapping
    fn verify_set_css_name_mapping(
        compiler: &mut AbstractCompiler,
        method_name: NodeId,
        first_arg: Option<NodeId>,
    ) -> bool {
        let mut diagnostic: Option<&'static DiagnosticType> = None;
        match first_arg {
            None => diagnostic = Some(&NULL_ARGUMENT_ERROR),
            Some(first_arg) if !first_arg.is_object_lit(compiler) => {
                diagnostic = Some(&EXPECTED_OBJECTLIT_ERROR);
            }
            Some(first_arg) => {
                if let Some(second_arg) = first_arg.get_next(compiler) {
                    if !second_arg.is_string_lit(compiler) {
                        diagnostic = Some(&EXPECTED_STRING_ERROR);
                    } else if second_arg.get_next(compiler).is_some() {
                        diagnostic = Some(&TOO_MANY_ARGUMENTS_ERROR);
                    }
                }
            }
        }
        if let Some(diagnostic) = diagnostic {
            let name = string_value_of(method_name.get_qualified_name(compiler));
            compiler.report(JSError::make(compiler, method_name, diagnostic, &[&name]));
            return false;
        }
        true
    }

    // port: ProcessClosurePrimitives#checkPropertyRenameCall
    fn check_property_rename_call(&mut self, compiler: &mut AbstractCompiler, call: NodeId) {
        let callee = call.get_first_child(compiler).unwrap();
        // TODO(b/193038601): make this work when the callee is a module import
        if !compiler
            .get_coding_convention()
            .is_property_rename_function(compiler, callee)
        {
            return;
        }
        let callee_name = string_value_of(callee.get_qualified_name(compiler));

        match call.get_child_count(compiler) - 1 {
            1 | 2 => {}
            _ => compiler.report(JSError::make(
                compiler,
                call,
                &INVALID_RENAME_FUNCTION,
                &[&callee_name, "Must be called with 1 or 2 arguments."],
            )),
        }

        let prop_name = callee.get_next(compiler);
        match prop_name.filter(|p| p.is_string_lit(compiler)) {
            None => compiler.report(JSError::make(
                compiler,
                call,
                &INVALID_RENAME_FUNCTION,
                &[&callee_name, "The first argument must be a string literal."],
            )),
            Some(prop_name) => {
                if prop_name.get_string(compiler).index_of(".") >= 0 {
                    compiler.report(JSError::make(
                        compiler,
                        call,
                        &INVALID_RENAME_FUNCTION,
                        &[
                            &callee_name,
                            "The first argument must not be a property path.",
                        ],
                    ));
                }
            }
        }
    }
}

/// The anonymous `CssRenamingMap` that `processSetCssNameMapping` returns.
struct SetCssNameMappingRenamingMap {
    css_names: IndexMap<JsString, JsString>,
    style: Style,
}

impl RenamingMap for SetCssNameMappingRenamingMap {
    // port: ProcessClosurePrimitives#processSetCssNameMapping (anonymous CssRenamingMap#get)
    fn get(&self, value: &JsString) -> Option<JsString> {
        if self.css_names.contains_key(value) {
            self.css_names.get(value).cloned()
        } else {
            Some(value.clone())
        }
    }
}

impl CssRenamingMap for SetCssNameMappingRenamingMap {
    // port: ProcessClosurePrimitives#processSetCssNameMapping (anonymous CssRenamingMap#getStyle)
    fn get_style(&self) -> Style {
        self.style
    }
}

impl CompilerPass for ProcessClosurePrimitives {
    // port: ProcessClosurePrimitives#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        // Replace and validate other Closure primitives
        NodeTraversal::traverse_roots(compiler, self, externs, root);
    }
}

impl Callback for ProcessClosurePrimitives {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ProcessClosurePrimitives#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::CALL => {
                self.check_goog_functions(t, n);
                self.maybe_process_class_base_call(t.get_compiler(), n);
                self.check_property_rename_call(t.get_compiler(), n);
            }
            Token::FUNCTION | Token::CLASS => {
                if !t.in_global_hoist_scope() || n.is_from_externs(t) {
                    return;
                }
                if !NodeUtil::is_function_declaration(t, n) && !NodeUtil::is_class_declaration(t, n)
                {
                    return;
                }
                let name = n.get_first_child(t).unwrap().get_string(t);
                let pn = self.closure_modules.get(&name);
                if let Some(pn) = pn
                    && pn.is_goog_provide()
                {
                    let metadata = pn.to_string(t);
                    let compiler = t.get_compiler();
                    compiler.report(JSError::make(
                        compiler,
                        n,
                        if n.is_class(compiler) {
                            &CLASS_NAMESPACE_ERROR
                        } else {
                            &FUNCTION_NAMESPACE_ERROR
                        },
                        &[&name.to_string(), &metadata],
                    ));
                }
            }
            Token::EXPR_RESULT => {
                let first = n.get_first_child(t).unwrap();
                if !first.is_assign(t) || !n.get_first_first_child(t).unwrap().is_qualified_name(t)
                {
                    return;
                }
                let lhs = n
                    .get_first_first_child(t)
                    .unwrap()
                    .get_qualified_name(t)
                    .unwrap();
                let info = first.get_jsdoc_info(t);
                self.check_possible_goog_provide_init(t.get_compiler(), &lhs, info, n);
            }
            Token::VAR | Token::CONST | Token::LET => {
                let first = n.get_first_child(t).unwrap();
                if !first.is_name(t) {
                    return;
                }
                let name = first.get_string(t);
                let info = n.get_jsdoc_info(t);
                self.check_possible_goog_provide_init(t.get_compiler(), &name, info, n);
            }
            _ => {}
        }
    }
}
