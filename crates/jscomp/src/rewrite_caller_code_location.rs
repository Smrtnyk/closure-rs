/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2024 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/RewriteCallerCodeLocation.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    ast_factory::AstFactory,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    var::VarId,
};
use closure_jstype::prelude::JSType;
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{
    check_state, ir::IR, js_string::JsString, node::NodeId, qualified_name::QualifiedName,
};
use std::sync::LazyLock;

// port: RewriteCallerCodeLocation#JSC_CALLER_LOCATION_POSITION_ERROR
pub static JSC_CALLER_LOCATION_POSITION_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_CALLER_LOCATION_POSITION_ERROR",
    "Please make sure there is only one goog.callerLocation argument in your function's parameter list, and it is the first optional argument in the list",
);

// port: RewriteCallerCodeLocation#JSC_CALLER_LOCATION_MISUSE_ERROR
pub static JSC_CALLER_LOCATION_MISUSE_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_CALLER_LOCATION_MISUSE_ERROR",
    "goog.callerLocation should only be used as a default parameter initializer",
);

// port: RewriteCallerCodeLocation#JSC_UNDEFINED_CODE_LOCATION_ERROR
pub static JSC_UNDEFINED_CODE_LOCATION_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_UNDEFINED_CODE_LOCATION_ERROR",
    "Do not pass in undefined as an argument to goog.CodeLocation parameter",
);

// port: RewriteCallerCodeLocation#JSC_ANONYMOUS_FUNCTION_CODE_LOCATION_ERROR
pub static JSC_ANONYMOUS_FUNCTION_CODE_LOCATION_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_ANONYMOUS_FUNCTION_CODE_LOCATION_ERROR",
    "Do not use goog.callerLocation in an anonymous functions. Functions that use goog.callerLocation should be named.",
);

// port: RewriteCallerCodeLocation#GOOG_CALLER_LOCATION_QUALIFIED_NAME
pub static GOOG_CALLER_LOCATION_QUALIFIED_NAME: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.callerLocation"));

/// Rewrites call-sites of functions that have goog.callerLocation as a default parameter.
///
/// E.g: function signal(val, here: goog.CodeLocation = goog.callerLocation()) {}
///
/// When a function is called without providing the optional `here` argument, we will rewrite the
/// function call to include the code location.
///
/// i.e. `signal(0, goog.callerLocationIdInternalDoNotCallOrElse(path/to/file.ts:lineno:charno))`
pub struct RewriteCallerCodeLocation {
    ast_factory: AstFactory,
    // Map of function name to a FunctionVarAndParamPosition object which contains:
    // 1. the function variable
    // 2. the position of the param which has goog.callerLocation as default value
    // E.g:
    // `function signal(val, here: goog.CodeLocation = goog.callerLocation()) {}`
    // callerLocationFunctionNames.put("signal", {functionVar: `Var signal @ NAME signal 1:9 ...`,
    // paramPosition: 2}); // 2 because "here" is the second param
    caller_location_function_names: IndexMap<String, FunctionVarAndParamPosition>,
}

impl RewriteCallerCodeLocation {
    // port: RewriteCallerCodeLocation#RewriteCallerCodeLocation
    /// Creates an instance.
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        Self {
            ast_factory: compiler.create_ast_factory(),
            caller_location_function_names: IndexMap::<_, _>::default(),
        }
    }
}

impl CompilerPass for RewriteCallerCodeLocation {
    // port: RewriteCallerCodeLocation#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, _root: NodeId) {
        // Find all functions that have goog.callerLocation as a default parameter. We want to
        // traverse the root node as well as the externs because the function that contains
        // `goog.callerLocation` parameter could be an
        // .i.js file in the externs node subtree
        let compiler_root = compiler.get_root().unwrap();
        NodeTraversal::traverse(
            compiler,
            compiler_root,
            &mut FindCallerLocationFunctions {
                caller_location_function_names: &mut self.caller_location_function_names,
            },
        );
        if !self.caller_location_function_names.is_empty() {
            // Rewrite call-sites of functions that have goog.callerLocation as a default
            // parameter. We want to traverse the root node as well as the externs to ensure the
            // scope of the call-site is the same scope as the function with the
            // `goog.callerLocation` parameter.
            let compiler_root = compiler.get_root().unwrap();
            NodeTraversal::traverse(
                compiler,
                compiler_root,
                &mut RewriteCallerLocationFunctionCalls {
                    ast_factory: &self.ast_factory,
                    caller_location_function_names: &self.caller_location_function_names,
                },
            );
        }
    }
}

struct FindCallerLocationFunctions<'a> {
    caller_location_function_names: &'a mut IndexMap<String, FunctionVarAndParamPosition>,
}

impl FindCallerLocationFunctions<'_> {
    // port: RewriteCallerCodeLocation.FindCallerLocationFunctions#isGoogCallerLocationMisused
    /// Checks if `goog.callerLocation` is misused. `goog.callerLocation` should only be used as a
    /// default parameter initializer.
    ///
    /// Returns true if goog.callerLocation is misused, false otherwise.
    fn is_goog_caller_location_misused(
        t: &NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if !GOOG_CALLER_LOCATION_QUALIFIED_NAME.matches(t, n) {
            // not a `goog.callerLocation` node
            return false;
        }

        let parent = parent.unwrap();
        // Check for misuse of goog.callerLocation.
        if parent.is_call(t) && parent.get_parent(t).unwrap().is_default_value(t) {
            if parent.get_grandparent(t).unwrap().is_string_key(t) {
                // Throw an error when `goog.callerLocation` is used in an object literal.
                // E.g:
                // function foo({val1, val2, here = goog.callerLocation()}) {}
                // The AST for `here = goog.callerLocation()`looks like:
                // STRING_KEY here (This node tells us we are in an object literal)
                //   DEFAULT_VALUE
                //     NAME here 1:26
                //     CALL 1:33
                //       GETPROP callerLocation (This is the node `n` we are currently at)
                //         NAME goog 1:33
                return true;
            }
            // `goog.callerLocation` is used correctly as a default value in a function's
            // parameter list.
            return false;
        }

        if parent.is_assign(t) && n.get_next(t).is_some() && n.get_next(t).unwrap().is_function(t) {
            // This is the definition of goog.callerLocation, so this is not a misuse.
            // i.e: `goog.callerLocation = function(...) { ... }`
            return false;
        }

        // `goog.callerLocation` is NOT used as a default value in a function's parameter list.
        true
    }

    // port: RewriteCallerCodeLocation.FindCallerLocationFunctions#visitParamListAndAddCallerLocationFunctionNames
    /// Visits the param list of a function and checks if it contains a default value of
    /// goog.callerLocation. If it does, `visitParamList` stores the function name and the index
    /// of the param in `callerLocationFunctionNames` map.
    ///
    /// Example: function signal(val, here: goog.CodeLocation = goog.callerLocation()) {}
    ///
    /// `callerLocationFunctionNames` will have an entry for "signal" -> 2 (because "here" is the
    /// second param)
    fn visit_param_list_and_add_caller_location_function_names(
        &mut self,
        n: NodeId,
        t: &mut NodeTraversal<'_>,
    ) {
        check_state!(n.is_param_list(t), "%s", n.to_string(t));
        // function foo(<params>) ...
        // Each item in <params> is scanned for having a default value.
        // If there exists a default value, check if it is `goog.callerLocation`.
        // Check if there is another optional argument that comes before the goog.callerLocation
        // arg.
        let mut default_values_count = 0;
        let mut param_position = 0; // keep track of the position of the param in the param list.
        let mut c = n.get_first_child(t);
        while let Some(child) = c {
            c = child.get_next(t);
            param_position += 1;
            if !child.is_default_value(t) {
                continue;
            }
            default_values_count += 1;
            let call = child.get_second_child(t).unwrap(); // CALL node (E.g: goog.callerLocation())
            if !call.is_call(t) {
                continue;
            }

            let get_prop = call.get_first_child(t).unwrap();
            if GOOG_CALLER_LOCATION_QUALIFIED_NAME.matches(t, get_prop) {
                if default_values_count > 1 {
                    let error = JSError::make(t, child, &JSC_CALLER_LOCATION_POSITION_ERROR, &[]);
                    t.get_compiler().report(error);
                }

                // E.g: function signal(val, here: goog.CodeLocation = goog.callerLocation()) {}
                // Add ("signal" -> 2) to `callerLocationFunctionNames`
                let function_name_node = n.get_parent(t).unwrap().get_first_child(t).unwrap();
                let function_name = function_name_node.get_qualified_name(t);
                let Some(function_name) = function_name else {
                    // Anonymous functions are not allowed to use goog.callerLocation.
                    let error = JSError::make(
                        t,
                        function_name_node,
                        &JSC_ANONYMOUS_FUNCTION_CODE_LOCATION_ERROR,
                        &[],
                    );
                    t.get_compiler().report(error);
                    return;
                };
                let scope = t.get_scope();
                let function_var = scope.get_var(t.get_compiler(), &function_name);
                let function_var_and_position =
                    FunctionVarAndParamPosition::new(function_var, param_position);
                self.caller_location_function_names
                    .insert(function_name.to_string(), function_var_and_position);
            }
        }
    }
}

impl Callback for FindCallerLocationFunctions<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: RewriteCallerCodeLocation.FindCallerLocationFunctions#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if Self::is_goog_caller_location_misused(t, n, parent) {
            let error = JSError::make(
                t,
                parent.unwrap().get_parent(t).unwrap(),
                &JSC_CALLER_LOCATION_MISUSE_ERROR,
                &[],
            );
            t.get_compiler().report(error);
            return;
        }

        if n.is_param_list(t) {
            self.visit_param_list_and_add_caller_location_function_names(n, t);
        }
    }
}

struct RewriteCallerLocationFunctionCalls<'a> {
    ast_factory: &'a AstFactory,
    caller_location_function_names: &'a IndexMap<String, FunctionVarAndParamPosition>,
}

impl RewriteCallerLocationFunctionCalls<'_> {
    // port: RewriteCallerCodeLocation.RewriteCallerLocationFunctionCalls#visitCallNodeAndRewrite
    /// Visits call expression nodes and checks if they require transformations. If they do, it
    /// rewrites the call expression node to include the code location.
    ///
    /// E.g: `signal(0)` will be rewritten to `signal(0,
    /// goog.callerLocationIdInternalDoNotCallOrElse(path/to/file.ts:lineno:charno))`
    fn visit_call_node_and_rewrite(&self, n: NodeId, t: &mut NodeTraversal<'_>) {
        let first_child = n.get_first_child(t);
        let Some(first_child) = first_child else {
            return;
        };
        if (!first_child.is_name(t) && !first_child.is_get_prop(t))
            || first_child.get_qualified_name(t).is_none()
        {
            return;
        }

        // Check if the call-site is calling a callerLocation function.
        // E.g: signal(0); name = "signal"
        let name = first_child.get_qualified_name(t).unwrap().to_string();

        // ClosureRewriteModule pass will run before this pass and will rename exported function
        // names and aliases in goog.modules.
        // E.g:
        // function name = module$contents$google3$javascript$apps$wiz$signals$signal_signal
        // call node name = module$exports$google3$javascript$apps$wiz$signals$signal.signal
        let module_contents_name = name
            .replace('.', "_")
            .replace("module$exports$", "module$contents$");
        if !self
            .caller_location_function_names
            .contains_key(&module_contents_name)
        {
            return;
        }

        let function_var_and_position = self
            .caller_location_function_names
            .get(&module_contents_name)
            .unwrap();
        let caller_location_function = function_var_and_position.get_function_var();
        let scope = t.get_scope();
        let callee_function = scope.get_var(
            t.get_compiler(),
            JsString::from(module_contents_name.as_str()),
        );
        let (Some(caller_location_function), Some(callee_function)) =
            (caller_location_function, callee_function)
        else {
            return;
        };
        if caller_location_function.get_name_node(t.get_compiler())
            != callee_function.get_name_node(t.get_compiler())
        {
            return;
        }

        // Check if the argument is provided.
        // E.g:
        // function signal(val, here: goog.CodeLocation = goog.callerLocation()) {}
        // signal(0, xid('path/to/file.ts:25')); // goog.CodeLocation is provided
        let position_of_caller_location_arg = function_var_and_position.get_param_position();
        let number_of_args = n.get_child_count(t) - 1; // -1 for the firstChild which is the function name.
        if number_of_args >= position_of_caller_location_arg {
            // goog.CodeLocation is provided as an argument, we will not rewrite this call-site.
            // If `undefined` is passed in as an argument to goog.CodeLocation, we will throw an
            // error. Otherwise continue without rewriting.
            let js_type = n
                .get_child_at_index(t, position_of_caller_location_arg)
                .unwrap()
                .get_jstype(t);
            if let Some(js_type) = js_type
                && {
                    let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
                    js_type.is_explicitly_voidable(registry, ast)
                }
            {
                // user is passing in undefined as an argument to goog.CodeLocation
                let error = JSError::make(t, first_child, &JSC_UNDEFINED_CODE_LOCATION_ERROR, &[]);
                t.get_compiler().report(error);
            }
            return;
        }

        // create the goog.callerLocationIdInternalDoNotCallOrElse(path/to/file.ts:lineno:charno)
        // and add it as the last parameter.
        let xid_call = self.create_goog_xid_file_path_node(t.get_compiler(), n);
        let compiler = t.get_compiler();
        compiler.report_change_to_enclosing_scope(n);
        n.add_child_to_back(compiler, xid_call);
    }

    // port: RewriteCallerCodeLocation.RewriteCallerLocationFunctionCalls#createGoogXidFilePathNode
    /// Creates a call node for
    /// "goog.callerLocationIdInternalDoNotCallOrElse(path/to/file.ts:lineno:charno)"
    ///
    /// `n`: call node of a function that needs to be rewritten to include the code location.
    /// Returns call node including code location i.e.
    /// "goog.callerLocationIdInternalDoNotCallOrElse(path/to/file.ts:lineno:charno)"
    fn create_goog_xid_file_path_node(&self, compiler: &mut AbstractCompiler, n: NodeId) -> NodeId {
        // googNode is "goog"
        let goog_node = IR::name(compiler, "goog");
        goog_node.srcref_if_missing(compiler, n);

        // googXid is "goog.callerLocationIdInternalDoNotCallOrElse" node
        let goog_xid = self.ast_factory.create_get_prop_with_unknown_type(
            compiler,
            goog_node,
            "callerLocationIdInternalDoNotCallOrElse",
        );
        goog_xid.srcref_if_missing(compiler, n);

        // callNode is "goog.callerLocationIdInternalDoNotCallOrElse()" node
        let call_node = self
            .ast_factory
            .create_call_with_unknown_type(compiler, goog_xid, &[]);
        call_node.srcref_if_missing(compiler, n);

        // stringNode is "path/to/file.ts:lineno:charno" node
        // (Java string concatenation prints a null source file name as "null".)
        let value = format!(
            "{}:{}:{}",
            n.get_source_file_name(compiler)
                .unwrap_or_else(|| "null".to_owned()),
            n.get_lineno(compiler),
            n.get_charno(compiler)
        );
        let string_node = self.ast_factory.create_string(compiler, value.as_str());
        string_node.srcref_if_missing(compiler, n);

        // turns callNode from "goog.callerLocationIdInternalDoNotCallOrElse()" to
        // "goog.callerLocationIdInternalDoNotCallOrElse(path/to/file.ts:lineno:charno)"
        call_node.add_child_to_back(compiler, string_node);
        call_node
    }
}

impl Callback for RewriteCallerLocationFunctionCalls<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: RewriteCallerCodeLocation.RewriteCallerLocationFunctionCalls#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_call(t) {
            self.visit_call_node_and_rewrite(n, t);
        }
    }
}

/// Class to store the function variable and the position of the param which has
/// goog.callerLocation as default value.
///
/// Since it is not guaranteed that names are unique at this point in compilation, we could have
/// something like (1) a function using goog.callerLocation that's actually nested within another
/// function, and not available globally or (2) a function using goog.callerLocation that's then
/// shadowed by another function locally. We do not want to rewrite the call-site in these cases,
/// so we'll check the scope of the call-site against the function with the `goog.callerLocation`
/// default parameter.
///
/// E.g: `function signal(val, here: goog.CodeLocation = goog.callerLocation()) {}`
///
/// This class will have:
///
/// 1. functionVar: `Var signal @ NAME signal 1:9 ...`
///
/// 2. paramPosition: 2 (because "here" is the second param)
///
/// Storing the functionVar is useful for checking if the call-site is calling the same function
/// as the one with the `goog.callerLocation` default parameter.
struct FunctionVarAndParamPosition {
    function_var: Option<VarId>,
    param_position: i32,
}

impl FunctionVarAndParamPosition {
    // port: RewriteCallerCodeLocation.FunctionVarAndParamPosition#FunctionVarAndParamPosition
    fn new(function_var: Option<VarId>, param_position: i32) -> Self {
        Self {
            function_var,
            param_position,
        }
    }

    // port: RewriteCallerCodeLocation.FunctionVarAndParamPosition#getFunctionVar
    fn get_function_var(&self) -> Option<VarId> {
        self.function_var
    }

    // port: RewriteCallerCodeLocation.FunctionVarAndParamPosition#getParamPosition
    fn get_param_position(&self) -> i32 {
        self.param_position
    }
}
