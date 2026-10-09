/*
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/FunctionArgumentInjector.java.

//! Port of `com.google.javascript.jscomp.FunctionArgumentInjector`.
//!
//! A nifty set of functions to deal with the issues of replacing function parameters with a set
//! of call argument expressions.
use crate::{
    abstract_compiler::AbstractCompiler, ast_analyzer::AstAnalyzer,
    coding_convention::CodingConvention, node_util::NodeUtil,
};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use indexmap::{IndexMap, IndexSet};

// A string to use to represent "this".  Anything that is not a valid
// identifier can be used, so we use "this".
pub const THIS_MARKER: &str = "this";

pub const REST_MARKER: &str = "rest param";

pub struct FunctionArgumentInjector {
    ast_analyzer: AstAnalyzer,
}

/// Java: `static final Node THIS_MARKER_NODE = IR.name(THIS_MARKER);`
///
/// Java shares one detached NAME node across compilations; nodes live in a compilation's arena
/// here, so each use creates the same detached `NAME this` node in the arena it is paired with.
/// The node is only ever used as the (never attached, never cloned) paramNode of the
/// THIS_MARKER entry.
pub fn this_marker_node(ast: &mut Ast) -> NodeId {
    IR::name(ast, THIS_MARKER)
}

impl FunctionArgumentInjector {
    // port: FunctionArgumentInjector#FunctionArgumentInjector
    pub fn new(ast_analyzer: AstAnalyzer) -> Self {
        Self { ast_analyzer }
    }

    /// With the map provided, replace the names with expression trees.
    ///
    /// Returns the root node or its replacement.
    // port: FunctionArgumentInjector#inject(AbstractCompiler,Node,Node,Map)
    pub fn inject(
        &self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        parent: Option<NodeId>,
        replacements: &mut IndexMap<JsString, NodeId>,
    ) -> NodeId {
        self.inject_with_replace_this(compiler, node, parent, replacements, true)
    }

    // port: FunctionArgumentInjector#inject(AbstractCompiler,Node,Node,Map,boolean)
    fn inject_with_replace_this(
        &self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        parent: Option<NodeId>,
        replacements: &mut IndexMap<JsString, NodeId>,
        mut replace_this: bool,
    ) -> NodeId {
        if node.is_name(compiler) {
            let replacement_template = replacements.get(&node.get_string(compiler)).copied();
            if let Some(replacement_template) = replacement_template {
                // This should not be replacing declared names.
                let parent = parent.unwrap();
                check_state!(
                    !(parent.is_function(compiler)
                        || parent.is_var(compiler)
                        || parent.is_catch(compiler)),
                    "%s",
                    parent.to_string(compiler)
                );
                // The name may need to be replaced more than once,
                // so we need to clone the node.
                let replacement = replacement_template.clone_tree(compiler);
                node.replace_with(compiler, replacement);
                return replacement;
            }
        } else if replace_this && node.is_this(compiler) {
            let replacement_template = replacements.get(&JsString::from(THIS_MARKER)).copied();
            let replacement_template = check_not_null!(replacement_template);
            if !replacement_template.is_this(compiler) {
                // The name may need to be replaced more than once,
                // so we need to clone the node.
                let replacement = replacement_template.clone_tree(compiler);
                node.replace_with(compiler, replacement);

                // Remove the value.  This isn't required but it ensures that we won't
                // inject side-effects multiple times as it will trigger the null
                // check above if we do.
                if compiler
                    .get_ast_analyzer()
                    .may_have_side_effects(compiler, replacement_template)
                {
                    replacements.shift_remove(&JsString::from(THIS_MARKER));
                }

                return replacement;
            }
        } else if (node.is_function(compiler) && !node.is_arrow_function(compiler))
            || node.is_member_field_def(compiler)
            || NodeUtil::is_class_static_block(compiler, node)
        {
            // Once we enter another non-arrow function, class field initializer, or static block,
            // the "this" value changes. Don't try to replace it within an inner scope.
            replace_this = false;
        }

        if node.is_computed_field_def(compiler) {
            let key = node.get_first_child(compiler);
            if let Some(key) = key {
                let key = self.inject_with_replace_this(
                    compiler,
                    key,
                    Some(node),
                    replacements,
                    replace_this,
                );
                let value = key.get_next(compiler);
                if let Some(value) = value {
                    self.inject_with_replace_this(
                        compiler,
                        value,
                        Some(node),
                        replacements,
                        /* replaceThis= */ false,
                    );
                }
            }
            return node;
        }

        let mut c = node.get_first_child(compiler);
        while let Some(current) = c {
            // We have to reassign c in case it was replaced, because the removed c's
            // getNext() would no longer be correct.
            let current = self.inject_with_replace_this(
                compiler,
                current,
                Some(node),
                replacements,
                replace_this,
            );
            c = current.get_next(compiler);
        }

        node
    }

    /// Get a mapping for function parameter names to call arguments.
    // port: FunctionArgumentInjector#getFunctionCallParameterMap
    pub fn get_function_call_parameter_map(
        &self,
        ast: &mut Ast,
        fn_node: NodeId,
        call_node: NodeId,
        safe_name_id_supplier: &(dyn Fn() -> String + Send + Sync),
    ) -> IndexMap<JsString, ParamArgPair> {
        // Create an parameterName -> expression map
        let mut arg_map = ImmutableMapBuilder::new();

        // CALL NODE: [ NAME, ARG1, ARG2, ... ]
        let mut c_arg = call_node.get_second_child(ast);
        if c_arg.is_some() && NodeUtil::is_function_object_call(ast, call_node) {
            if !fn_node.is_arrow_function(ast) {
                let marker = this_marker_node(ast);
                arg_map.put(THIS_MARKER, ParamArgPair::new(marker, c_arg.unwrap()));
            } else {
                let marker = this_marker_node(ast);
                let this_node = IR::this_node(ast);
                arg_map.put(THIS_MARKER, ParamArgPair::new(marker, this_node));
            }
            c_arg = c_arg.unwrap().get_next(ast);
        } else {
            // 'apply' isn't supported yet.
            check_state!(
                !NodeUtil::is_function_object_apply(ast, call_node),
                "%s",
                call_node.to_string(ast)
            );
            let marker = this_marker_node(ast);
            let undefined = NodeUtil::new_undefined_node(ast, Some(call_node));
            arg_map.put(THIS_MARKER, ParamArgPair::new(marker, undefined));
        }

        let mut fn_param = NodeUtil::get_function_parameters(ast, fn_node).get_first_child(ast);
        while let Some(param) = fn_param {
            if let Some(arg) = c_arg {
                if param.is_rest(ast) {
                    check_state!(
                        param.get_only_child(ast).is_name(ast),
                        "%s",
                        param.get_only_child(ast).to_string(ast)
                    );
                    let array = IR::arraylit(ast, &[]);
                    array.srcref_tree_if_missing(ast, arg);
                    let mut rest_arg = Some(arg);
                    while let Some(current) = rest_arg {
                        let clone = current.clone_tree(ast);
                        array.add_child_to_back(ast, clone);
                        rest_arg = current.get_next(ast);
                    }
                    let rest_name = param.get_only_child(ast);
                    arg_map.put(
                        rest_name.get_string(ast),
                        ParamArgPair::new(rest_name, array),
                    );
                    return arg_map.build_or_throw(ast);
                } else {
                    check_state!(param.is_name(ast), "%s", param.to_string(ast));
                    arg_map.put(param.get_string(ast), ParamArgPair::new(param, arg));
                }
                c_arg = arg.get_next(ast);
            } else {
                // cArg == null
                if param.is_rest(ast) {
                    check_state!(
                        param.get_only_child(ast).is_name(ast),
                        "%s",
                        param.to_string(ast)
                    );
                    // No arguments for REST parameters
                    let array = IR::arraylit(ast, &[]).srcref(ast, param);
                    let rest_name = param.get_only_child(ast);
                    arg_map.put(
                        rest_name.get_string(ast),
                        ParamArgPair::new(rest_name, array),
                    );
                } else {
                    check_state!(param.is_name(ast), "%s", param.to_string(ast));
                    let src_location = call_node;
                    let undefined = NodeUtil::new_undefined_node(ast, Some(src_location));
                    arg_map.put(param.get_string(ast), ParamArgPair::new(param, undefined));
                }
            }
            fn_param = param.get_next(ast);
        }

        // Add temp names for arguments that don't have named parameters in the
        // called function.
        while let Some(arg) = c_arg {
            let unique_placeholder =
                Self::get_unique_anonymous_parameter_name(safe_name_id_supplier);
            let name = IR::name(ast, unique_placeholder.as_str());
            arg_map.put(unique_placeholder.as_str(), ParamArgPair::new(name, arg));
            c_arg = arg.get_next(ast);
        }

        arg_map.build_or_throw(ast)
    }

    /// Parameter names will be name unique when at a later time.
    // port: FunctionArgumentInjector#getUniqueAnonymousParameterName
    fn get_unique_anonymous_parameter_name(
        safe_name_id_supplier: &(dyn Fn() -> String + Send + Sync),
    ) -> String {
        format!("JSCompiler_inline_anon_param_{}", safe_name_id_supplier())
    }

    /// Retrieve a set of names that can not be safely substituted in place.
    ///
    /// Example: `function(a) { a = 0; }`. Inlining this without taking precautions would cause
    /// the call site value to be modified (bad).
    // port: FunctionArgumentInjector#findModifiedParameters(Node)
    pub fn find_modified_parameters(&self, ast: &Ast, fn_node: NodeId) -> IndexSet<JsString> {
        let names = Self::get_function_parameter_set(ast, fn_node);
        Self::find_modified_parameters_in(ast, fn_node.get_last_child(ast).unwrap(), &names, false)
    }

    /// Check for uses of the named value that imply a pass-by-value parameter is expected. This
    /// is used to prevent cases like: `function (x) { x=2; return x; }`
    ///
    /// We don't want "undefined" to be substituted for "x", and get undefined=2
    // port: FunctionArgumentInjector#findModifiedParameters(Node,ImmutableSet,boolean)
    fn find_modified_parameters_in(
        ast: &Ast,
        n: NodeId,
        names: &IndexSet<JsString>,
        mut in_inner_function: bool,
    ) -> IndexSet<JsString> {
        let mut unsafe_ = IndexSet::new();
        if n.is_name(ast) {
            if names.contains(&n.get_string(ast))
                && (in_inner_function || Self::can_name_value_change(ast, n))
            {
                unsafe_.insert(n.get_string(ast));
            }
        } else if n.is_function(ast) {
            // A function parameter can not be replaced with a direct inlined value
            // if it is referred to by an inner function. The inner function
            // can out live the call we are replacing, so inner function must
            // capture a unique name.  This approach does not work within loop
            // bodies so those are forbidden elsewhere.
            in_inner_function = true;
        }

        for c in n.children(ast) {
            unsafe_.extend(Self::find_modified_parameters_in(
                ast,
                c,
                names,
                in_inner_function,
            ));
        }

        unsafe_
    }

    /// This is similar to NodeUtil.isLValue except that object properties and array member
    /// modification aren't important ("o" in "o.a = 2" is still "o" after assignment, where in
    /// as "o = x", "o" is now "x").
    ///
    /// This also looks for the redefinition of a name. function (x) {var x;}
    // port: FunctionArgumentInjector#canNameValueChange
    fn can_name_value_change(ast: &Ast, n: NodeId) -> bool {
        if !NodeUtil::is_l_value(ast, n) {
            return false;
        }
        if NodeUtil::is_declaration_l_value(ast, n) {
            let declaring = NodeUtil::get_declaring_parent(ast, n);
            return !declaring.is_const(ast) && !declaring.is_let(ast);
        }
        true
    }

    /// Updates the set of parameter names that correspond to arguments from the call site that
    /// require aliases.
    ///
    /// If an argument requires a temp (e.g. it is side-effectful, affects mutable state or due
    /// to other reasons), we need to evaluate it into a temporary at the callsite before
    /// inlining the function block into that callsite. However, that argument's early evaluation
    /// could also change the values of previous arguments. Hence, in this function we decide to
    /// hoist all those previous args and evaluate them before this side-effectful argument, even
    /// if they are not actually affected by that side-effectful argument. This simplifies the
    /// implementation and doesn't hurt performance.
    // port: FunctionArgumentInjector#gatherCallArgumentsNeedingTemps
    ///
    /// `convention` is `None` where Java passes `compiler.getCodingConvention()` (every pass
    /// does): the analyzer needs the compiler mutably while the convention is in use.
    pub fn gather_call_arguments_needing_temps(
        &self,
        compiler: &mut AbstractCompiler,
        fn_node: NodeId,
        arg_map: &IndexMap<JsString, ParamArgPair>,
        modified_parameters: &IndexSet<JsString>,
        convention: Option<&dyn CodingConvention>,
    ) -> IndexSet<JsString> {
        check_argument!(
            fn_node.is_function(compiler),
            "%s",
            fn_node.to_string(compiler)
        );
        if arg_map.is_empty() {
            // No arguments to check, we are done.
            return modified_parameters.clone();
        }
        let mut names_needing_temps: IndexSet<JsString> = modified_parameters.clone();
        let block = fn_node.get_last_child(compiler).unwrap();

        // This field holds the parameter name corresponding to the last side-effectful
        // argument. Args corresponding to all parameters before and including this parameter
        // name would get hoisted using temps.
        let mut requires_temps_up_to_this_parameter_name = JsString::from("");

        let arg_count = arg_map.len() as i32;
        // We limit the "trivial" bodies to those where there is a single expression or
        // return, the expression is
        let is_trivial_body = !block.has_children(compiler)
            || (block.has_one_child(compiler)
                && !self.body_may_have_conditional_code(
                    compiler,
                    block.get_last_child(compiler).unwrap(),
                ));
        let this_arg = arg_map[&JsString::from(THIS_MARKER)].arg();
        let has_minimal_parameters = (NodeUtil::is_undefined(compiler, this_arg)
            || (fn_node.is_arrow_function(compiler) && this_arg.is_this(compiler)))
            && arg_count <= 2; // this + one parameter

        // Get the list of parameters that may need temporaries due to side-effects.
        let parameters_that_may_need_temps = self.find_parameters_referenced_after_side_effect(
            compiler,
            &arg_map.keys().cloned().collect(),
            block,
        );
        let names_after_side_effects =
            parameters_that_may_need_temps.parameters_referenced_after_side_effect;
        let parameters_with_names_referenced_before =
            parameters_that_may_need_temps.parameters_with_names_referenced_before_parameter;

        // Check for arguments that are evaluated more than once.
        for (parameter_name, pair) in arg_map {
            let c_arg = pair.arg();
            if names_needing_temps.contains(parameter_name) {
                requires_temps_up_to_this_parameter_name = parameter_name.clone();
                continue;
            }
            // Stores whether this arg need to get hoisted using a temporary
            let mut requires_temporary = false;
            let references = NodeUtil::get_name_reference_count(compiler, block, parameter_name);

            let arg_side_effects = compiler
                .get_ast_analyzer()
                .may_have_side_effects(compiler, c_arg);
            if !arg_side_effects && references == 0 {
                requires_temporary = false;
            } else if is_trivial_body
                && has_minimal_parameters
                && references == 1
                // The below line is checking: Can this be affected by side-effects in the
                // function body?
                && !(NodeUtil::can_be_side_effected(compiler, c_arg)
                    && names_after_side_effects.contains(parameter_name))
                // The below line is checking: Can the function body be affected by the side
                // effects of the argument? If the argument has side effects and the function
                // body contains names that are referenced before the argument, then we need to
                // inline using temporaries. If not, then we can enter this condition's else-if
                // block.
                && (!arg_side_effects
                    || !parameters_with_names_referenced_before.contains(parameter_name))
            {
                // For functions with a trivial body, and where the parameter evaluation order
                // can't change, and there aren't any side-effect before the parameter, we can
                // avoid creating a temporary.
                //
                // This is done to help inline common trivial functions
                // TODO: b/407603216 - return `true` when return expressions don't have
                // references that can be side-effected
                requires_temporary = false;
            } else if compiler
                .get_ast_analyzer()
                .may_effect_mutable_state(compiler, c_arg)
                && references > 0
            {
                // Note: Mutable arguments should be assigned to temps, as the
                // may be within in a loop:
                //   function x(a) {
                //     for(var i=0; i<0; i++) {
                //       foo(a);
                //     }
                //   x( [] );
                // The parameter in the call to foo should not become "[]".
                requires_temporary = true;
            } else if arg_side_effects {
                // even if there are no references, we still need to evaluate the
                // expression if it has side-effects.
                requires_temporary = true;
            } else if NodeUtil::can_be_side_effected(compiler, c_arg)
                && names_after_side_effects.contains(parameter_name)
            {
                requires_temporary = true;
            } else if references > 1 {
                // Safe is a misnomer, this is a check for "large".
                match c_arg.get_token(compiler) {
                    Token::NAME => {
                        let name = c_arg.get_string(compiler);
                        // Don't worry about whether this is global or local, just check if it is
                        // "exported" in either case.
                        let convention =
                            convention.unwrap_or_else(|| compiler.get_coding_convention());
                        requires_temporary = convention.is_exported(&name, true)
                            || convention.is_exported(&name, false);
                    }
                    Token::THIS => requires_temporary = false,
                    Token::STRINGLIT => {
                        requires_temporary = c_arg.get_string_ref(compiler).length() >= 2
                    }
                    _ => requires_temporary = !NodeUtil::is_immutable_value(compiler, c_arg),
                }
            }

            if requires_temporary {
                requires_temps_up_to_this_parameter_name = parameter_name.clone();
            }
        }

        if !requires_temps_up_to_this_parameter_name.is_empty() {
            // mark all names upto requiresTempsUptoParameterName as namesNeedingTemps
            for parameter_name in arg_map.keys() {
                if *parameter_name == *THIS_MARKER
                    && (NodeUtil::is_undefined(compiler, this_arg)
                        || (fn_node.is_arrow_function(compiler) && this_arg.is_this(compiler)))
                {
                    // When there is no explicit this arg passed into the call, the argMap
                    // contains an entry <"this", undefined Node>. See the
                    // `getFunctionCallParameterMap` method.
                    //
                    // We do not want to add an unnecessary temp for "this", when there wasn't an
                    // explicit "this" passed in at the callsite. That is, we want to transform a
                    // call-site like: `foo(a,b)` to `let a$inline$0 = a; foo(a$inline$0 ,b)` and
                    // not `let this$inline$0 = this; let a$inline$0=a; foo(a$inline$0, b)`.
                    // Hence we skip generating a temporary for an implicit "this" arg
                    continue;
                }
                // TODO: b/298828688 Use `NodeUtil.isImmutableValue` to detect some simple
                // immutable cases and skip generating temps for those as well.

                if *parameter_name == requires_temps_up_to_this_parameter_name {
                    names_needing_temps.insert(parameter_name.clone());
                    break;
                } else {
                    names_needing_temps.insert(parameter_name.clone());
                }
            }
        }
        names_needing_temps
    }

    /// We consider a return or expression trivial if it doesn't contain a conditional expression
    /// or a function.
    // port: FunctionArgumentInjector#bodyMayHaveConditionalCode
    pub fn body_may_have_conditional_code(&self, ast: &Ast, n: NodeId) -> bool {
        if !n.is_return(ast) && !n.is_expr_result(ast) {
            return true;
        }
        self.may_have_conditional_code(ast, n)
    }

    /// We consider an expression trivial if it doesn't contain a conditional expression or a
    /// function.
    // port: FunctionArgumentInjector#mayHaveConditionalCode
    pub fn may_have_conditional_code(&self, ast: &Ast, n: NodeId) -> bool {
        for c in n.children(ast) {
            match c.get_token(ast) {
                Token::FUNCTION
                | Token::AND
                | Token::OR
                | Token::HOOK
                | Token::COALESCE
                | Token::OPTCHAIN_CALL
                | Token::OPTCHAIN_GETELEM
                | Token::OPTCHAIN_GETPROP => {
                    return true;
                }
                _ => {}
            }
            if self.may_have_conditional_code(ast, c) {
                return true;
            }
        }
        false
    }

    /// Bootstrap a traversal to look for parameters referenced after a non-local side-effect,
    /// and parameters with names referenced before the parameter is referenced.
    ///
    /// NOTE: This assumes no-inner functions.
    // port: FunctionArgumentInjector#findParametersReferencedAfterSideEffect
    fn find_parameters_referenced_after_side_effect(
        &self,
        compiler: &mut AbstractCompiler,
        parameters: &IndexSet<JsString>,
        root: NodeId,
    ) -> SetContainer {
        // TODO(johnlenz): Consider using scope for this.
        let mut locals: IndexSet<JsString> = parameters.clone();
        Self::gather_local_names(compiler, root, &mut locals);

        let mut collector =
            ReferencedAfterSideEffect::new(&self.ast_analyzer, parameters.clone(), locals);
        visit_post_order(compiler, root, &mut collector);
        // port: FunctionArgumentInjector.SetContainer#SetContainer
        SetContainer {
            parameters_referenced_after_side_effect: collector
                .get_parameters_referenced_after_side_effect(),
            parameters_with_names_referenced_before_parameter: collector
                .get_parameters_with_names_referenced_before_parameter(),
        }
    }

    /// Gather any names declared in the local scope.
    // port: FunctionArgumentInjector#gatherLocalNames
    fn gather_local_names(ast: &Ast, n: NodeId, names: &mut IndexSet<JsString>) {
        if n.is_function(ast) {
            if NodeUtil::is_function_declaration(ast, n) {
                names.insert(n.get_first_child(ast).unwrap().get_string(ast));
            }
            // Don't traverse into inner function scopes;
            return;
        } else if n.is_name(ast) {
            match n.get_parent(ast).unwrap().get_token(ast) {
                Token::VAR | Token::LET | Token::CONST | Token::CATCH => {
                    names.insert(n.get_string(ast));
                }
                _ => {}
            }
        }

        for c in n.children(ast) {
            Self::gather_local_names(ast, c, names);
        }
    }

    /// Get a set of function parameter names.
    // port: FunctionArgumentInjector#getFunctionParameterSet
    fn get_function_parameter_set(ast: &Ast, fn_node: NodeId) -> IndexSet<JsString> {
        let mut builder = IndexSet::new();
        for n in NodeUtil::get_function_parameters(ast, fn_node).children(ast) {
            if n.is_rest(ast) {
                builder.insert(JsString::from(REST_MARKER));
            } else if n.is_default_value(ast) || n.is_object_pattern(ast) || n.is_array_pattern(ast)
            {
                panic!("Not supported: {}", n.to_string(ast));
            } else {
                builder.insert(n.get_string(ast));
            }
        }
        builder
    }
}

/// Java: `static record SetContainer(ImmutableSet<String>, ImmutableSet<String>)`.
pub struct SetContainer {
    pub parameters_referenced_after_side_effect: IndexSet<JsString>,
    pub parameters_with_names_referenced_before_parameter: IndexSet<JsString>,
}

/// Collect parameter names referenced after a non-local side-effect.
///
/// Assumptions:
/// - We assume parameters are not modified in the function body (that is checked separately).
/// - There are no inner functions (also checked separately).
///
/// As we are trying to replace parameters with there passed in values we are interested in
/// anything that may affect those value. So, ignoring changes to local variables, we look for
/// things that may affect anything outside the local-state. Once such a side-effect is seen any
/// following reference to the function parameters are collected. These will need to be assigned
/// to temporaries to prevent changes to their value as would have happened during the function
/// call.
///
/// To properly handle loop structures all references to the function parameters are recorded
/// and the decision to keep or throw away those references is deferred until exiting the loop
/// structure.
struct ReferencedAfterSideEffect<'a> {
    ast_analyzer: &'a AstAnalyzer,
    parameters: IndexSet<JsString>,
    locals: IndexSet<JsString>,
    side_effect_seen: bool,
    parameters_referenced: IndexSet<JsString>,

    name_node_has_been_seen: bool,
    parameters_with_names_referenced_before_parameter: IndexSet<JsString>,

    loops_entered: i32,
}

impl<'a> ReferencedAfterSideEffect<'a> {
    // port: FunctionArgumentInjector.ReferencedAfterSideEffect#ReferencedAfterSideEffect
    fn new(
        ast_analyzer: &'a AstAnalyzer,
        parameters: IndexSet<JsString>,
        locals: IndexSet<JsString>,
    ) -> Self {
        Self {
            ast_analyzer,
            parameters,
            locals,
            side_effect_seen: false,
            parameters_referenced: IndexSet::new(),
            name_node_has_been_seen: false,
            parameters_with_names_referenced_before_parameter: IndexSet::new(),
            loops_entered: 0,
        }
    }

    // port: FunctionArgumentInjector.ReferencedAfterSideEffect#getParametersReferencedAfterSideEffect
    fn get_parameters_referenced_after_side_effect(&self) -> IndexSet<JsString> {
        self.parameters_referenced.clone()
    }

    // port: FunctionArgumentInjector.ReferencedAfterSideEffect#getParametersWithNamesReferencedBeforeParameter
    fn get_parameters_with_names_referenced_before_parameter(&self) -> IndexSet<JsString> {
        self.parameters_with_names_referenced_before_parameter
            .clone()
    }

    // port: FunctionArgumentInjector.ReferencedAfterSideEffect#apply
    fn apply(&mut self, ast: &Ast, node: NodeId) -> bool {
        // Keep track of any loop structures entered.
        if NodeUtil::is_loop_structure(ast, node) {
            self.loops_entered += 1;
        }

        // If we have found all the parameters, don't bother looking
        // at the children.
        !(self.side_effect_seen && self.parameters.len() == self.parameters_referenced.len())
    }

    // port: FunctionArgumentInjector.ReferencedAfterSideEffect#inLoop
    fn in_loop(&self) -> bool {
        self.loops_entered != 0
    }

    // port: FunctionArgumentInjector.ReferencedAfterSideEffect#visit
    fn visit(&mut self, compiler: &mut AbstractCompiler, n: NodeId) {
        if n.is_name(compiler) {
            let name = n.get_string(compiler);
            if self.parameters.contains(&name) && self.name_node_has_been_seen {
                // We have seen a name node before this parameter. If this parameter has side
                // effects, it is not safe to inline.
                self.parameters_with_names_referenced_before_parameter
                    .insert(name);
            }
            self.name_node_has_been_seen = true;
        }

        // If we are exiting a loop.
        if NodeUtil::is_loop_structure(compiler, n) {
            self.loops_entered -= 1;
            if !self.in_loop() && !self.side_effect_seen {
                // Now that the loops has been fully traversed and
                // no side-effects have been seen, throw away
                // the references seen in them.
                self.parameters_referenced.clear();
            }
        }

        if !self.side_effect_seen {
            // Look for side-effects.
            if self.has_non_local_side_effect(compiler, n) {
                self.side_effect_seen = true;
            }
        }

        // If traversing the nodes of a loop save any references
        // that are seen.
        if self.in_loop() || self.side_effect_seen {
            // Record references to parameters.
            if n.is_name(compiler) {
                let name = n.get_string(compiler);
                if self.parameters.contains(&name) {
                    self.parameters_referenced.insert(name);
                }
            } else if n.is_this(compiler) {
                self.parameters_referenced
                    .insert(JsString::from(THIS_MARKER));
            }
        }
    }

    /// Returns whether the node may have non-local side-effects.
    // port: FunctionArgumentInjector.ReferencedAfterSideEffect#hasNonLocalSideEffect
    fn has_non_local_side_effect(&self, compiler: &mut AbstractCompiler, n: NodeId) -> bool {
        let mut side_effect = false;
        let type_ = n.get_token(compiler);
        // Note: Only care about changes to non-local names, specifically
        // ignore VAR declaration assignments.
        if NodeUtil::is_assignment_op(compiler, n) || type_ == Token::INC || type_ == Token::DEC {
            let lhs = n.get_first_child(compiler).unwrap();
            // Ignore changes to local names.
            if !self.is_local_name(compiler, lhs) {
                side_effect = true;
            }
        } else if type_ == Token::CALL {
            side_effect = self
                .ast_analyzer
                .function_call_has_side_effects(compiler, n);
        } else if type_ == Token::NEW {
            side_effect = self
                .ast_analyzer
                .constructor_call_has_side_effects(compiler, n);
        } else if type_ == Token::DELPROP {
            side_effect = true;
        }

        side_effect
    }

    /// Returns whether node is a reference to locally declared name.
    // port: FunctionArgumentInjector.ReferencedAfterSideEffect#isLocalName
    fn is_local_name(&self, ast: &Ast, node: NodeId) -> bool {
        if node.is_name(ast) {
            let name = node.get_string(ast);
            return self.locals.contains(&name);
        }
        false
    }
}

/// `NodeUtil.visitPostOrder(root, collector, collector)`: Java's post-order walk (the
/// traverse-children predicate before the children, `next` fetched before each child is visited,
/// the visitor after the children) with the ReferencedAfterSideEffect collector as both the
/// Visitor and the Predicate. The collector's side-effect checks need the compiler mutably.
fn visit_post_order(
    compiler: &mut AbstractCompiler,
    node: NodeId,
    collector: &mut ReferencedAfterSideEffect<'_>,
) {
    if collector.apply(compiler, node) {
        let mut c = node.get_first_child(compiler);
        while let Some(current) = c {
            let next = current.get_next(compiler);
            visit_post_order(compiler, current, collector);
            c = next;
        }
    }
    collector.visit(compiler, node);
}

/// Stores a pair of function formal parameter + argument value at a specific call site.
///
/// `param_node` is the original formal parameter name, such as `x` in `function foo(x) { ... }`
/// or, when injecting a value for `this`, THIS_MARKER_NODE. `arg` is the argument passed as
/// `param_node` at some call site, such as `500` in `f(500);`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParamArgPair {
    param_node: NodeId,
    arg: NodeId,
}

impl ParamArgPair {
    // port: FunctionArgumentInjector.ParamArgPair#ParamArgPair
    pub fn new(param_node: NodeId, arg: NodeId) -> Self {
        Self { param_node, arg }
    }
    // port: FunctionArgumentInjector.ParamArgPair#paramNode
    pub fn param_node(&self) -> NodeId {
        self.param_node
    }
    // port: FunctionArgumentInjector.ParamArgPair#arg
    pub fn arg(&self) -> NodeId {
        self.arg
    }
    // port: FunctionArgumentInjector.ParamArgPair#toString
    fn to_string(self, ast: &Ast) -> String {
        format!(
            "ParamArgPair[paramNode={}, arg={}]",
            self.param_node.to_string(ast),
            self.arg.to_string(ast)
        )
    }
}

/// Guava `ImmutableMap.Builder<String, ParamArgPair>`: insertion order, and buildOrThrow
/// rejects duplicate keys with Guava's IllegalArgumentException text.
struct ImmutableMapBuilder {
    entries: Vec<(JsString, ParamArgPair)>,
}

impl ImmutableMapBuilder {
    // port: ImmutableMap#builder
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
    // port: ImmutableMap.Builder#put
    fn put(&mut self, key: impl Into<JsString>, value: ParamArgPair) {
        self.entries.push((key.into(), value));
    }
    // port: ImmutableMap.Builder#buildOrThrow
    fn build_or_throw(self, ast: &Ast) -> IndexMap<JsString, ParamArgPair> {
        let mut map: IndexMap<JsString, ParamArgPair> = IndexMap::new();
        for (key, value) in self.entries {
            if let Some(existing) = map.get(&key) {
                panic!(
                    "Multiple entries with same key: {}={} and {}={}",
                    key,
                    value.to_string(ast),
                    key,
                    existing.to_string(ast)
                );
            }
            map.insert(key, value);
        }
        map
    }
}
