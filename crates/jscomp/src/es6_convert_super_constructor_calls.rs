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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/Es6ConvertSuperConstructorCalls.java.

//! Port of `Es6ConvertSuperConstructorCalls.java`: converts `super()` calls.

use crate::{
    AbstractCompiler,
    ast_factory::{AstFactory, Type},
    compiler_input::CompilerInput,
    compiler_options::Es6SubclassTranspilation,
    global_namespace::{GlobalNamespace, Ref},
    js::runtime_js_lib_manager::JsLibField,
    node_traversal::{AbstractShallowCallback, Callback, NodeTraversal},
    node_util::NodeUtil,
    transpilation_namespace::TranspilationNamespace,
    transpilation_util::CANNOT_CONVERT_YET,
};
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    jscomp_colors::{Color, standard_colors},
    node::{NodeId, Prop},
};
use std::sync::Arc;

// port: Es6ConvertSuperConstructorCalls#TMP_ERROR
const TMP_ERROR: &str = "$jscomp$tmp$error";
// port: Es6ConvertSuperConstructorCalls#SUPER_THIS
const SUPER_THIS: &str = "$jscomp$super$this";

/// Stores superCalls for a constructor.
// port: Es6ConvertSuperConstructorCalls.ConstructorData
struct ConstructorData {
    constructor: NodeId,
    super_calls: Vec<NodeId>,
}

impl ConstructorData {
    // port: Es6ConvertSuperConstructorCalls.ConstructorData#ConstructorData
    fn new(constructor: NodeId) -> Self {
        Self {
            constructor,
            super_calls: Vec::new(),
        }
    }
}

/// Converts `super()` calls.
pub struct Es6ConvertSuperConstructorCalls {
    jscomp_inherits: Arc<dyn JsLibField>,
    jscomp_construct: Arc<dyn JsLibField>,
    /// Java's `Deque` used as a stack: `push`/`peek`/`pop` act on the last element here.
    constructor_data_stack: Vec<ConstructorData>,
    ast_factory: AstFactory,
    // Note: this GlobalNamespace needs to be the namespace we get after the transpilation passes
    // run. meaning compiler.getTranspilationNamespace() does not work.
    global_namespace: Option<GlobalNamespace>,
    transpilation_namespace: TranspilationNamespace,
    es6_subclass_transpilation: Es6SubclassTranspilation,
}

impl Es6ConvertSuperConstructorCalls {
    // port: Es6ConvertSuperConstructorCalls#Es6ConvertSuperConstructorCalls
    pub fn new(
        compiler: &mut AbstractCompiler,
        es6_subclass_transpilation: Es6SubclassTranspilation,
    ) -> Self {
        let ast_factory = compiler.create_ast_factory();
        let transpilation_namespace = TranspilationNamespace::get(compiler);
        // Java keeps `compiler.getUniqueIdSupplier()` in a field; the Rust pass reads it from the
        // compiler at each use (DESIGN §6).
        let runtime_js_lib_manager = compiler.get_runtime_js_lib_manager();
        let mut runtime_js_lib_manager = runtime_js_lib_manager.lock().unwrap();
        let jscomp_inherits = runtime_js_lib_manager.get_js_lib_field("$jscomp.inherits");
        let jscomp_construct = runtime_js_lib_manager.get_js_lib_field("$jscomp.construct");
        drop(runtime_js_lib_manager);
        Self {
            jscomp_inherits,
            jscomp_construct,
            constructor_data_stack: Vec::new(),
            ast_factory,
            global_namespace: None,
            transpilation_namespace,
            es6_subclass_transpilation,
        }
    }

    // port: Es6ConvertSuperConstructorCalls#visitSuper
    fn visit_super(&mut self, t: &mut NodeTraversal<'_>, constructor_data: ConstructorData) {
        // NOTE: When this pass runs:
        // -   ES6 classes have already been rewritten as ES5 functions.
        // -   All subclasses have $jscomp.inherits() calls connecting them to their parent class.
        // -   All instances of `super` that are not super constructor calls have been rewritten.
        let constructor = constructor_data.constructor;
        let super_calls = constructor_data.super_calls;
        if super_calls.is_empty() {
            return; // nothing to do
        }

        if constructor.is_from_externs(t) {
            // This class is defined in an externs file, so it's only a stub, not the actual
            // implementation that should be instantiated.
            // A call to super() shouldn't actually exist for a stub and is problematic to
            // transpile, so just drop it.
            for super_call in super_calls {
                let enclosing_statement = NodeUtil::get_enclosing_statement(t, super_call).unwrap();
                let enclosing_scope = enclosing_statement.get_parent(t).unwrap();
                enclosing_statement.detach(t);
                t.get_compiler()
                    .report_change_to_enclosing_scope(enclosing_scope);
            }
            return;
        }
        // Give a unique name to the $jscomp$super$this variables created when rewriting this
        // super to preserve normalization.
        let unique_super_this_name = {
            let compiler = t.get_compiler();
            let input_id = NodeUtil::get_input_id(compiler, constructor);
            let input = compiler
                .get_input(input_id.as_deref().expect("NullPointerException: inputId"))
                .cloned()
                .expect("NullPointerException: input");
            format!(
                "{}${}",
                SUPER_THIS,
                compiler.get_unique_id_supplier().get_unique_id(&input)
            )
        };
        // Find the `foo.SuperClass` part of `$jscomp.inherits(foo.SubClass, foo.SuperClass)`
        let super_class_name_node = self.get_super_class_qname_node(t.get_compiler(), constructor);
        let super_class_qname = super_class_name_node
            .get_qualified_name(t)
            .expect("NullPointerException: qualified name");
        let this_type =
            Self::get_type_of_this_for_constructor(t.get_compiler(), constructor_data.constructor);

        if Self::is_native_object_class(t, &super_class_qname) {
            // There's no need to call Object as a super constructor, so just replace the call
            // with `this`, which is its correct return value.
            self.replace_super_calls_with_this(t.get_compiler(), &super_calls, this_type);
        } else if Self::is_known_native_class(t, &super_class_qname) {
            // Although we're transpiling down to ES5, it's quite possible that the code will end
            // up running in an environment where native classes are ES6 classes.
            // To correctly extend them with the ES5 classes we're generating here, we must use
            // `$jscomp.construct`, which is our wrapper around `Reflect.construct`.
            self.rewrite_super_calls_to_js_comp_construct_calls(
                t.get_compiler(),
                constructor,
                &super_calls,
                super_class_name_node,
                this_type,
                &unique_super_this_name,
            );
        } else if Self::is_native_error_class(t, &super_class_qname) {
            // TODO(bradfordcsmith): It might be better to use $jscomp.construct() for these
            // instead of our custom-made, Error-specific workaround.
            self.rewrite_native_error_super_calls(
                t,
                &super_calls,
                super_class_name_node,
                this_type,
            );
        } else if self.is_known_to_return_only_undefined(t.get_compiler(), &super_class_qname) {
            self.rewrite_super_calls(
                t.get_compiler(),
                &super_calls,
                super_class_name_node,
                this_type,
            );
        } else {
            self.rewrite_super_and_preserve_returned_this(
                t.get_compiler(),
                constructor,
                &super_calls,
                super_class_name_node,
                this_type,
                &unique_super_this_name,
            );
        }
    }

    // port: Es6ConvertSuperConstructorCalls#replaceSuperCallsWithThis
    fn replace_super_calls_with_this(
        &self,
        compiler: &mut AbstractCompiler,
        super_calls: &[NodeId],
        this_type: Type,
    ) {
        for &super_call in super_calls {
            let mut side_effect_args: Vec<NodeId> = Vec::new();
            let callee = super_call.remove_first_child(compiler).unwrap();
            check_state!(callee.is_super(compiler), "%s", callee.to_string(compiler));
            while super_call.has_children(compiler) {
                let mut arg = super_call.remove_first_child(compiler).unwrap();
                if arg.is_spread(compiler) {
                    arg = arg.remove_first_child(compiler).unwrap();
                }
                let ast_analyzer = compiler.get_ast_analyzer();
                if ast_analyzer.may_have_side_effects(compiler, arg) {
                    side_effect_args.push(arg);
                }
            }

            let mut replacement = self
                .ast_factory
                .create_this(compiler, this_type.clone())
                .srcref(compiler, super_call);
            if !side_effect_args.is_empty() {
                let mut exprs = side_effect_args[0];
                for &side_effect_arg in &side_effect_args[1..] {
                    exprs = self
                        .ast_factory
                        .create_comma(compiler, exprs, side_effect_arg)
                        .srcref_tree_if_missing(compiler, super_call);
                }
                replacement = self
                    .ast_factory
                    .create_comma(compiler, exprs, replacement)
                    .srcref_tree_if_missing(compiler, super_call);
            }
            super_call.replace_with(compiler, replacement);
            compiler.report_change_to_enclosing_scope(replacement);
        }
    }

    /// Change calls to `super` to use `$jscomp.construct` instead.
    ///
    /// ```text
    ///   // note that conversion of the ES6 class to ES5 happens before this pass.
    ///   // we're just cleaning up the super() calls now
    ///   var Foo = function(arg1, arg2) {
    ///     super(arg1);
    ///     this.prop = arg2;
    ///   }
    ///   // becomes
    ///   var Foo = function(arg1, arg2) {
    ///     // tmp var and return are necessary, because $jscomp.construct() always creates a new
    ///     // object to be used as `this`
    ///     var $jscomp$super$this;
    ///     $jscomp$super$this = $jscomp.construct(SuperClassName, [arg1], this.constructor);
    ///     $jscomp$super$this.prop = arg2
    ///     return $jscomp$super$this;
    ///   }
    /// ```
    // port: Es6ConvertSuperConstructorCalls#rewriteSuperCallsToJsCompConstructCalls
    fn rewrite_super_calls_to_js_comp_construct_calls(
        &self,
        compiler: &mut AbstractCompiler,
        constructor: NodeId,
        super_calls: &[NodeId],
        super_class_name_node: NodeId,
        this_type: Type,
        unique_super_this_name: &str,
    ) {
        let constructor_body = check_not_null!(constructor.get_child_at_index(compiler, 2));
        let first_statement = constructor_body.get_first_child(compiler);
        // A constructor body with no call to `super()` is a syntax error for a class that has an
        // extends clause. An error should have been reported and we should never reach this point
        // for an empty constructor body.
        let first_statement = check_not_null!(first_statement, "Empty constructor body");
        let first_super_call = super_calls[0];

        if constructor_body.has_one_child(compiler)
            && first_statement.is_expr_result(compiler)
            && first_statement.has_one_child(compiler)
            && first_statement.get_first_child(compiler) == Some(first_super_call)
        {
            check_state!(
                super_calls.len() == 1,
                "%s",
                constructor.to_string(compiler)
            );
            // Super call is the entire constructor, so just replace it with.
            // `return $jscomp.construct(SuperClassName, [args], this.constructor);`
            let construct_call = self.create_js_comp_constructor_call(
                compiler,
                super_class_name_node,
                first_super_call,
                this_type,
            );
            let return_node = self.ast_factory.create_return(compiler, construct_call);
            first_statement.replace_with(compiler, return_node);
        } else {
            let type_of_this = Self::get_type_of_this_for_constructor(compiler, constructor);
            // `this` -> `$jscomp$super$this` throughout the constructor body,
            // except for super() calls.
            self.update_this_to_super_this(
                compiler,
                type_of_this.clone(),
                constructor_body,
                super_calls,
                unique_super_this_name,
            );
            // Start constructor with `var $jscomp$super$this;`, but place it after any hoisted
            // function declarations
            let declaration = self
                .ast_factory
                .create_single_var_name_declaration(compiler, unique_super_this_name)
                .srcref_tree(compiler, constructor_body);
            let insert_before_point =
                NodeUtil::get_insertion_point_after_all_inner_function_declarations(
                    compiler,
                    constructor_body,
                );
            if let Some(insert_before_point) = insert_before_point {
                declaration.insert_before(compiler, insert_before_point);
            } else {
                // functionBody only contains hoisted function declarations
                constructor_body.add_child_to_back(compiler, declaration);
            }

            // End constructor with `return $jscomp$super$this;`
            let name = self.ast_factory.create_name(
                compiler,
                unique_super_this_name,
                type_of_this.clone(),
            );
            let return_node = self
                .ast_factory
                .create_return(compiler, name)
                .srcref_tree(compiler, constructor_body);
            constructor_body.add_child_to_back(compiler, return_node);
            // Replace each super() call with `($jscomp$super$this = $jscomp.construct(...))`
            for &super_call in super_calls {
                let name = self
                    .ast_factory
                    .create_name(compiler, unique_super_this_name, type_of_this.clone())
                    .srcref(compiler, super_call);
                let construct_call = self.create_js_comp_constructor_call(
                    compiler,
                    super_class_name_node,
                    super_call,
                    this_type.clone(),
                );
                let assign = self
                    .ast_factory
                    .create_assign(compiler, name, construct_call)
                    .srcref(compiler, super_call);
                super_call.replace_with(compiler, assign);
            }
        }
        compiler.report_change_to_enclosing_scope(constructor_body);
    }

    // port: Es6ConvertSuperConstructorCalls#rewriteNativeErrorSuperCalls
    fn rewrite_native_error_super_calls(
        &self,
        t: &mut NodeTraversal<'_>,
        super_calls: &[NodeId],
        super_class_name_node: NodeId,
        this_type: Type,
    ) {
        for &super_call in super_calls {
            let new_super_call = self.create_new_super_call(
                t.get_compiler(),
                super_class_name_node,
                super_call,
                this_type.clone(),
                AstFactory::type_node(super_call),
            );
            let input = t.get_input().cloned();
            self.replace_native_error_super_call(
                t.get_compiler(),
                super_call,
                new_super_call,
                input.as_ref(),
            );
        }
    }

    // port: Es6ConvertSuperConstructorCalls#rewriteSuperCalls
    fn rewrite_super_calls(
        &self,
        compiler: &mut AbstractCompiler,
        super_calls: &[NodeId],
        super_class_name_node: NodeId,
        this_type: Type,
    ) {
        // super() will not change the value of `this`.
        for &super_call in super_calls {
            let new_super_call = self.create_new_super_call(
                compiler,
                super_class_name_node,
                super_call,
                this_type.clone(),
                AstFactory::type_(standard_colors::NULL_OR_VOID.clone()),
            );
            let super_call_parent = super_call.get_parent(compiler).unwrap();
            if super_call_parent.has_one_child(compiler)
                && NodeUtil::is_statement(compiler, super_call_parent)
            {
                // super() is a statement unto itself
                super_call.replace_with(compiler, new_super_call);
            } else {
                // super() is part of an expression, so it must return `this`.
                let this_node = self.ast_factory.create_this(compiler, this_type.clone());
                let comma = self
                    .ast_factory
                    .create_comma(compiler, new_super_call, this_node)
                    .srcref_tree_if_missing(compiler, super_call);
                super_call.replace_with(compiler, comma);
            }
            compiler.report_change_to_enclosing_scope(super_call_parent);
        }
    }

    // port: Es6ConvertSuperConstructorCalls#rewriteSuperAndPreserveReturnedThis
    fn rewrite_super_and_preserve_returned_this(
        &self,
        compiler: &mut AbstractCompiler,
        constructor: NodeId,
        super_calls: &[NodeId],
        super_class_name_node: NodeId,
        this_type: Type,
        unique_super_this_name: &str,
    ) {
        // Either the superclass constructor returns a value, or we cannot find its definition in
        // the sources, so we don't know if it does.
        //
        // 1. We must use the value it returns, if defined, as the 'this' value in the constructor
        //    we're currently transpiling, and we must also return it from this constructor.
        // 2. It may be an ES6 class defined outside of the sources we can see.
        if self.es6_subclass_transpilation == Es6SubclassTranspilation::SAFE_REFLECT_CONSTRUCT {
            // To safely extend the ES5 classes we're generating here, we must use
            // `$jscomp.construct`, which is our wrapper around `Reflect.construct`.
            self.rewrite_super_calls_to_js_comp_construct_calls(
                compiler,
                constructor,
                super_calls,
                super_class_name_node,
                this_type,
                unique_super_this_name,
            );
            return;
        }
        // The code below works as long as the class we're extending is an ES5 class, but will
        // break if we're extending an ES6 class (#2), because it calls the superclass constructor
        // without using `new` or `Reflect.construct()`
        // TODO(b/36789413): we should always use `$jscomp.construct`.
        let constructor_body = check_not_null!(constructor.get_child_at_index(compiler, 2));
        let first_statement = constructor_body.get_first_child(compiler);
        let first_super_call = super_calls[0];

        if constructor_body.has_one_child(compiler)
            && first_statement
                .expect("NullPointerException: firstStatement")
                .is_expr_result(compiler)
            && first_statement.unwrap().has_one_child(compiler)
            && first_statement.unwrap().get_first_child(compiler) == Some(first_super_call)
        {
            let first_statement = first_statement.unwrap();
            check_state!(
                super_calls.len() == 1,
                "%s",
                constructor.to_string(compiler)
            );
            // Super call is the entire constructor, so just replace it with.
            // `return <newSuperCall> || this;`
            let new_super_call = self.create_new_super_call(
                compiler,
                super_class_name_node,
                super_calls[0],
                AstFactory::type_node(super_calls[0]),
                AstFactory::type_(standard_colors::UNKNOWN.clone()),
            );
            let this_node = self.ast_factory.create_this(compiler, this_type);
            let new_return = self
                .ast_factory
                .create_or(compiler, new_super_call, this_node);
            let return_node = IR::return_node_with_expression(compiler, new_return)
                .srcref_tree_if_missing(compiler, first_statement);
            first_statement.replace_with(compiler, return_node);
        } else {
            let type_of_this = Self::get_type_of_this_for_constructor(compiler, constructor);
            // Start constructor with `var $jscomp$super$this;`, but place it after any hoisted
            // function declarations
            let name = self.ast_factory.create_name(
                compiler,
                unique_super_this_name,
                type_of_this.clone(),
            );
            let declaration = IR::var(compiler, name).srcref_tree(compiler, constructor_body);
            let insert_before_point =
                NodeUtil::get_insertion_point_after_all_inner_function_declarations(
                    compiler,
                    constructor_body,
                );
            if let Some(insert_before_point) = insert_before_point {
                declaration.insert_before(compiler, insert_before_point);
            } else {
                // functionBody only contains hoisted function declarations
                constructor_body.add_child_to_back(compiler, declaration);
            }
            // End constructor with `return $jscomp$super$this;`
            let name = self.ast_factory.create_name(
                compiler,
                unique_super_this_name,
                type_of_this.clone(),
            );
            let return_node = IR::return_node_with_expression(compiler, name)
                .srcref_tree(compiler, constructor_body);
            constructor_body.add_child_to_back(compiler, return_node);

            // Replace each `this` -> `$jscomp$super$this` throughout the constructor body,
            // except for super() calls.
            self.update_this_to_super_this(
                compiler,
                type_of_this.clone(),
                constructor_body,
                super_calls,
                unique_super_this_name,
            );

            // Replace each super() call with `($jscomp$super$this = <newSuperCall> || this)`
            for &super_call in super_calls {
                let new_super_call = self.create_new_super_call(
                    compiler,
                    super_class_name_node,
                    super_call,
                    type_of_this.clone(),
                    AstFactory::type_(standard_colors::UNKNOWN.clone()),
                );
                let name = self.ast_factory.create_name(
                    compiler,
                    unique_super_this_name,
                    type_of_this.clone(),
                );
                let this_node = self.ast_factory.create_this(compiler, type_of_this.clone());
                let or = self
                    .ast_factory
                    .create_or(compiler, new_super_call, this_node);
                let assign = self
                    .ast_factory
                    .create_assign(compiler, name, or)
                    .srcref_tree_if_missing(compiler, super_call);
                super_call.replace_with(compiler, assign);
            }
        }
        compiler.report_change_to_enclosing_scope(constructor_body);
    }

    // port: Es6ConvertSuperConstructorCalls#isKnownToReturnOnlyUndefined
    fn is_known_to_return_only_undefined(
        &mut self,
        compiler: &mut AbstractCompiler,
        function_qname: &JsString,
    ) -> bool {
        let Some(global_namespace) = self.global_namespace.as_mut() else {
            return false;
        };
        let Some(global_name) = global_namespace.get_slot(compiler, function_qname) else {
            return false;
        };

        let mut declaration_ref: Option<Ref> = global_name.get_declaration(global_namespace);
        if declaration_ref.is_none() {
            for r#ref in global_name.get_refs(global_namespace) {
                if r#ref.is_set(global_namespace) {
                    declaration_ref = Some(r#ref);
                }
            }
        }
        let Some(declaration_ref) = declaration_ref else {
            return false;
        };

        let declared_var_or_prop = declaration_ref
            .get_node(global_namespace)
            .expect("NullPointerException: declarationRef.getNode()");
        if declared_var_or_prop.is_from_externs(compiler) {
            return false;
        }

        let declaration = declared_var_or_prop.get_parent(compiler).unwrap();
        let declared_value = if declaration.is_function(compiler) {
            declaration
        } else if NodeUtil::is_name_declaration(compiler, Some(declaration))
            && declared_var_or_prop.is_name(compiler)
        {
            if declared_var_or_prop.has_children(compiler) {
                check_not_null!(declared_var_or_prop.get_first_child(compiler))
            } else {
                return false; // Declaration without an assigned value.
            }
        } else if declaration.is_assign(compiler)
            && declaration.get_first_child(compiler) == Some(declared_var_or_prop)
        {
            check_not_null!(declaration.get_second_child(compiler))
        } else if declaration.is_object_lit(compiler)
            && declared_var_or_prop.has_one_child(compiler)
        {
            check_not_null!(declared_var_or_prop.get_first_child(compiler))
        } else {
            panic!(
                "Unexpected declaration format:\n{}",
                declaration.to_string_tree(compiler)
            );
        };
        self.is_node_known_to_only_return_undefined(compiler, declared_value)
    }

    // port: Es6ConvertSuperConstructorCalls#isNodeKnownToOnlyReturnUndefined
    fn is_node_known_to_only_return_undefined(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
    ) -> bool {
        if node.is_function(compiler) {
            let function_body = check_not_null!(node.get_child_at_index(compiler, 2));
            !UndefinedReturnValueCheck::new().may_return_defined_value(compiler, function_body)
        } else if node.is_qualified_name(compiler) {
            let qname = node.get_qualified_name(compiler).unwrap();
            self.is_known_to_return_only_undefined(compiler, &qname)
        } else if node.is_hook(compiler) {
            // cond ? left : right;
            let left = node.get_second_child(compiler).unwrap();
            let right = node.get_last_child(compiler).unwrap();
            self.is_node_known_to_only_return_undefined(compiler, left)
                && self.is_node_known_to_only_return_undefined(compiler, right)
        } else {
            // TODO(bradfordcsmith): What cases are these? Can we do better?
            false
        }
    }

    /// Returns a transpiled version of the super constructor call.
    ///
    /// The children of the passed in `superCall` are all removed from it by this method, but the
    /// existing call itself is not replaced in the AST yet. The returned node is not yet attached
    /// to the AST.
    // port: Es6ConvertSuperConstructorCalls#createNewSuperCall
    fn create_new_super_call(
        &self,
        compiler: &mut AbstractCompiler,
        super_class_qname_node: NodeId,
        super_call: NodeId,
        this_type: Type,
        result_type: Type,
    ) -> NodeId {
        check_argument!(
            super_class_qname_node.is_qualified_name(compiler),
            "%s",
            super_class_qname_node.to_string(compiler)
        );
        check_argument!(
            super_call.is_call(compiler),
            "%s",
            super_call.to_string(compiler)
        );

        let callee = super_call.remove_first_child(compiler).unwrap();
        check_state!(callee.is_super(compiler), "%s", callee.to_string(compiler));

        let mut args: Vec<NodeId> = Vec::new();
        let mut has_spread_arg = false;
        while super_call.has_children(compiler) {
            let arg = super_call.remove_first_child(compiler).unwrap();
            has_spread_arg = has_spread_arg || arg.is_spread(compiler);
            args.push(arg);
        }

        // Node to which args should be appended
        if has_spread_arg {
            // We want to convert
            //
            // super(x, ...params, y)
            // to
            // Foo.apply(this, [x, ...params, y])
            //
            // because, after transpilation of spread this becomes
            //
            // Foo.apply(this, [x, $jscomp.arrayFromIterable(params), y])
            //
            // If we used `call`, we'd get this nonsense instead
            //
            // Foo.call.apply(Foo, [this, x, $jscomp.arrayFromIterable(params), y])
            let receiver = super_class_qname_node.clone_tree(compiler);
            let super_class_dot_apply = self
                .ast_factory
                .create_get_prop_with_unknown_type(compiler, receiver, "apply")
                .srcref_tree(compiler, callee);
            // Create `SuperClass.call(this)`
            let new_super_call = self
                .ast_factory
                .create_call(compiler, super_class_dot_apply, result_type, &[])
                .srcref(compiler, super_call);
            let this_node = self
                .ast_factory
                .create_this(compiler, this_type)
                .srcref(compiler, callee);
            new_super_call.add_child_to_back(compiler, this_node);
            new_super_call.put_boolean_prop(compiler, Prop::FREE_CALL, false); // callee is now a getprop
            // It's very common to just have `super(...arguments)`, because we generate
            // constructors containing that for extending classes that don't have an explicit
            // constructor. For that case it's more efficient to just convert
            // `super(...arguments)` to `SuperClass.apply(this, arguments)` here rather than
            // relying on later optimizations to convert `[...arguments]` to `arguments`.
            if Self::is_single_spread_of_arguments(compiler, &args) {
                let only = get_only_element(&args)
                    .get_only_child(compiler)
                    .detach(compiler);
                new_super_call.add_child_to_back(compiler, only);
            } else {
                let array = self
                    .ast_factory
                    .create_arraylit(compiler, &args)
                    .srcref(compiler, super_call);
                new_super_call.add_child_to_back(compiler, array);
            }
            new_super_call
        } else {
            // We want to convert
            //
            // super(arg1, arg2)
            // to
            // Foo.call(this, arg1, arg2)
            //
            // Using `call` is shorter than using `apply`.
            let receiver = super_class_qname_node.clone_tree(compiler);
            let super_class_dot_call = self
                .ast_factory
                .create_get_prop(
                    compiler,
                    receiver,
                    "call",
                    AstFactory::type_(standard_colors::TOP_OBJECT.clone()),
                )
                .srcref_tree(compiler, callee);
            let new_super_call = self
                .ast_factory
                .create_call(compiler, super_class_dot_call, result_type, &[])
                .srcref(compiler, super_call);
            let this_node = self
                .ast_factory
                .create_this(compiler, this_type)
                .srcref(compiler, callee);
            new_super_call.add_child_to_back(compiler, this_node);
            new_super_call.put_boolean_prop(compiler, Prop::FREE_CALL, false); // callee is now a getprop
            for arg in args {
                new_super_call.add_child_to_back(compiler, arg);
            }
            new_super_call
        }
    }

    /// Returns a transpiled version of the super constructor call using `$jscomp.constructor`.
    ///
    /// The children of the passed in `superCall` are all removed from it by this method, but the
    /// existing call itself is not replaced in the AST yet. The returned node is not yet attached
    /// to the AST.
    // port: Es6ConvertSuperConstructorCalls#createJSCompConstructorCall
    fn create_js_comp_constructor_call(
        &self,
        compiler: &mut AbstractCompiler,
        super_class_qname_node: NodeId,
        super_call: NodeId,
        this_type: Type,
    ) -> NodeId {
        check_argument!(
            super_class_qname_node.is_qualified_name(compiler),
            "%s",
            super_class_qname_node.to_string(compiler)
        );
        check_argument!(
            super_call.is_call(compiler),
            "%s",
            super_call.to_string(compiler)
        );

        let callee = check_not_null!(
            super_call.remove_first_child(compiler),
            "%s",
            super_call.to_string(compiler)
        );
        check_state!(callee.is_super(compiler), "%s", callee.to_string(compiler));

        // `$jscomp.construct`
        let jscomp_dot_construct = self
            .ast_factory
            .create_qname_for_field(
                compiler,
                &self.transpilation_namespace,
                self.jscomp_construct.as_ref(),
            )
            .srcref_tree(compiler, callee);

        let super_class_qname = super_class_qname_node.clone_tree(compiler);

        // extract the arguments from the super() call and create the arguments list to pass to
        // $jscomp.construct()
        let mut super_call_arg_list: Vec<NodeId> = Vec::new();
        while super_call.has_children(compiler) {
            super_call_arg_list.push(super_call.remove_first_child(compiler).unwrap());
        }
        // It's very common to just have `super(...arguments)`, because we generate constructors
        // containing that for extending classes that don't have an explicit constructor.
        // For that case it's more efficient to just convert `super(...arguments)` to
        // `$jscomp.construct(SuperClass, arguments, this.constructor)` here rather than relying on
        // later optimizations to
        // convert `[...arguments]` to `arguments`.
        let super_args = if Self::is_single_spread_of_arguments(compiler, &super_call_arg_list) {
            // pull out `arguments` from `...arguments`
            get_only_element(&super_call_arg_list)
                .get_only_child(compiler)
                .detach(compiler)
        } else {
            self.ast_factory
                .create_arraylit(compiler, &super_call_arg_list)
                .srcref(compiler, super_call)
        };

        // `this.constructor`
        let this_node = self.ast_factory.create_this(compiler, this_type);
        let this_dot_constructor = self
            .ast_factory
            .create_get_prop(
                compiler,
                this_node,
                "constructor",
                AstFactory::type_node(super_class_qname_node),
            )
            .srcref_tree(compiler, super_call);

        // `super(arg1, arg2)`
        // becomes
        // `$jscomp.construct(SuperClassName, [arg1, arg2], this.constructor)`
        self.ast_factory
            .create_call(
                compiler,
                jscomp_dot_construct,
                AstFactory::type_node(super_call),
                &[super_class_qname, super_args, this_dot_constructor],
            )
            .srcref(compiler, super_call)
    }

    // port: Es6ConvertSuperConstructorCalls#isSingleSpreadOfArguments
    fn is_single_spread_of_arguments(compiler: &AbstractCompiler, node_list: &[NodeId]) -> bool {
        node_list.len() == 1 && Self::is_spread_of_arguments(compiler, get_only_element(node_list))
    }

    // port: Es6ConvertSuperConstructorCalls#isSpreadOfArguments
    fn is_spread_of_arguments(compiler: &AbstractCompiler, node: NodeId) -> bool {
        node.is_spread(compiler)
            && node
                .get_only_child(compiler)
                .matches_name(compiler, "arguments")
    }

    // port: Es6ConvertSuperConstructorCalls#replaceNativeErrorSuperCall
    fn replace_native_error_super_call(
        &self,
        compiler: &mut AbstractCompiler,
        super_call: NodeId,
        new_super_call: NodeId,
        compiler_input: Option<&CompilerInput>,
    ) {
        // The native error class constructors always return a new object instead of initializing
        // `this`, so a workaround is needed.
        let super_statement = NodeUtil::get_enclosing_statement(compiler, super_call).unwrap();
        let body = super_statement.get_parent(compiler).unwrap();
        check_state!(body.is_block(compiler), "%s", body.to_string(compiler));

        let this_type = AstFactory::type_node(new_super_call);

        // Give a unique name to the $jscomp$tmp$error; variables created when rewriting this
        // super to preserve normalization.
        let tmp_error_name = format!(
            "{}${}",
            TMP_ERROR,
            compiler
                .get_unique_id_supplier()
                .get_unique_id(compiler_input.expect("NullPointerException: compilerInput"))
        );

        // var $jscomp$tmp$error;
        let name =
            self.ast_factory
                .create_name(compiler, tmp_error_name.as_str(), this_type.clone());
        let get_error = IR::var(compiler, name).srcref_tree_if_missing(compiler, super_call);
        get_error.insert_before(compiler, super_statement);

        // Create an expression to initialize `this` from temporary Error object at the point
        // where super.apply() was called.
        // $jscomp$tmp$error = Error.call(this, ...),
        let name =
            self.ast_factory
                .create_name(compiler, tmp_error_name.as_str(), this_type.clone());
        let get_tmp_error = self
            .ast_factory
            .create_assign(compiler, name, new_super_call);
        // this.message = $jscomp$tmp$error.message,
        let this_node = self.ast_factory.create_this(compiler, this_type.clone());
        let this_message = self.ast_factory.create_get_prop(
            compiler,
            this_node,
            "message",
            AstFactory::type_(standard_colors::STRING.clone()),
        );
        let name =
            self.ast_factory
                .create_name(compiler, tmp_error_name.as_str(), this_type.clone());
        let error_message = self.ast_factory.create_get_prop(
            compiler,
            name,
            "message",
            AstFactory::type_(standard_colors::STRING.clone()),
        );
        let copy_message = self
            .ast_factory
            .create_assign(compiler, this_message, error_message);

        // Old versions of IE Don't set stack until the object is thrown, and won't set it then
        // if it already exists on the object.
        // ('stack' in $jscomp$tmp$error) && (this.stack = $jscomp$tmp$error.stack)
        let stack = self.ast_factory.create_string(compiler, "stack");
        let name =
            self.ast_factory
                .create_name(compiler, tmp_error_name.as_str(), this_type.clone());
        let in_node = self.ast_factory.create_in(compiler, stack, name);
        let this_node = self.ast_factory.create_this(compiler, this_type.clone());
        let this_stack = self.ast_factory.create_get_prop(
            compiler,
            this_node,
            "stack",
            AstFactory::type_(standard_colors::STRING.clone()),
        );
        let name =
            self.ast_factory
                .create_name(compiler, tmp_error_name.as_str(), this_type.clone());
        let error_stack = self.ast_factory.create_get_prop(
            compiler,
            name,
            "stack",
            AstFactory::type_(standard_colors::STRING.clone()),
        );
        let assign_stack = self
            .ast_factory
            .create_assign(compiler, this_stack, error_stack);
        let set_stack = self.ast_factory.create_and(compiler, in_node, assign_stack);
        let this_node = self.ast_factory.create_this(compiler, this_type);
        let super_error_expr = self
            .ast_factory
            .create_commas(
                compiler,
                get_tmp_error,
                copy_message,
                &[set_stack, this_node],
            )
            .srcref_tree_if_missing(compiler, super_call);
        super_call.replace_with(compiler, super_error_expr);
        compiler.report_change_to_enclosing_scope(super_error_expr);
    }

    // port: Es6ConvertSuperConstructorCalls#isNativeObjectClass
    fn is_native_object_class(t: &mut NodeTraversal<'_>, class_name: &JsString) -> bool {
        *class_name == "Object" && !Self::is_defined_in_sources(t, class_name)
    }

    // port: Es6ConvertSuperConstructorCalls#isNativeErrorClass
    fn is_native_error_class(t: &mut NodeTraversal<'_>, super_class_name: &JsString) -> bool {
        match super_class_name.to_string_lossy().as_str() {
            "AggregateError" | "Error" | "EvalError" | "RangeError" | "ReferenceError"
            | "SyntaxError" | "TypeError" | "URIError" => {
                // All Error classes listed in the ECMAScript spec as of 2016
                !Self::is_defined_in_sources(t, super_class_name)
            }
            _ => false,
        }
    }

    /// Is `className` the name of a known native JS class for which we haven't seen a definition
    /// in the source code we're compiling. (Note that our own polyfill definitions don't count as
    /// a definition being present in the source code.)
    // port: Es6ConvertSuperConstructorCalls#isKnownNativeClass
    fn is_known_native_class(t: &mut NodeTraversal<'_>, class_name: &JsString) -> bool {
        // This list originally taken from the list of built-in objects at
        // https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference
        // as of 2016-10-22.
        // - Intl.* classes were left out, because it doesn't seem worth the extra effort
        //   of handling the qualified name.
        // - Deprecated and experimental classes were left out.
        match class_name.to_string_lossy().as_str() {
            "Array" | "ArrayBuffer" | "Boolean" | "DataView" | "Date" | "Float32Array"
            | "Function" | "Generator" | "GeneratorFunction" | "Int16Array" | "Int32Array"
            | "Int8Array" | "InternalError" | "Iterator" | "Map" | "Number" | "Object"
            | "Promise" | "Proxy" | "RegExp" | "Set" | "String" | "Symbol" | "TypedArray"
            | "Uint16Array" | "Uint32Array" | "Uint8Array" | "Uint8ClampedArray" | "WeakMap"
            | "WeakSet" => !Self::is_defined_in_sources(t, class_name),
            _ => false,
        }
    }

    /// Is a variable with the given name defined in the source code being compiled?
    ///
    /// Please note that the call to `t.getScope()` is expensive, so we should avoid calling this
    /// method when possible.
    // port: Es6ConvertSuperConstructorCalls#isDefinedInSources
    fn is_defined_in_sources(t: &mut NodeTraversal<'_>, var_name: &JsString) -> bool {
        let scope = t.get_scope();
        let object_var = scope.get_var(t.get_compiler(), var_name);
        object_var.is_some_and(|object_var| !object_var.is_extern(t.get_compiler()))
    }

    // port: Es6ConvertSuperConstructorCalls#updateThisToSuperThis
    fn update_this_to_super_this(
        &self,
        compiler: &mut AbstractCompiler,
        type_of_this: Type,
        constructor_body: NodeId,
        super_calls: &[NodeId],
        unique_super_this_name: &str,
    ) {
        let mut replace_this_with_super_this = ReplaceThisWithSuperThis {
            ast_factory: &self.ast_factory,
            type_of_this,
            super_calls,
            unique_super_this_name,
        };
        NodeTraversal::traverse(
            compiler,
            constructor_body,
            &mut replace_this_with_super_this,
        );
    }

    // port: Es6ConvertSuperConstructorCalls#getTypeOfThisForConstructor
    fn get_type_of_this_for_constructor(compiler: &AbstractCompiler, constructor: NodeId) -> Type {
        check_argument!(
            constructor.is_function(compiler),
            "%s",
            constructor.to_string(compiler)
        );
        let constructor_type = constructor.get_color(compiler);
        match constructor_type {
            Some(constructor_type) if !constructor_type.get_instance_colors().is_empty() => {
                AstFactory::type_(Color::create_union(constructor_type.get_instance_colors()))
            }
            _ => AstFactory::type_(standard_colors::UNKNOWN.clone()),
        }
    }

    // port: Es6ConvertSuperConstructorCalls#getSuperClassQNameNode
    fn get_super_class_qname_node(
        &self,
        compiler: &AbstractCompiler,
        constructor: NodeId,
    ) -> NodeId {
        let class_name = NodeUtil::get_name_node(compiler, constructor)
            .expect("NullPointerException: getNameNode")
            .get_qualified_name(compiler)
            .expect("NullPointerException: getQualifiedName");
        let constructor_statement =
            check_not_null!(NodeUtil::get_enclosing_statement(compiler, constructor));

        let mut super_class_name_node: Option<NodeId> = None;
        let mut statement = constructor_statement.get_next(compiler);
        while let Some(s) = statement {
            super_class_name_node =
                self.get_super_class_name_node_if_is_inherits_statement(compiler, s, &class_name);
            if super_class_name_node.is_some() {
                break;
            }
            statement = s.get_next(compiler);
        }

        check_not_null!(super_class_name_node, "$jscomp.inherits() call not found.")
    }

    // port: Es6ConvertSuperConstructorCalls#getSuperClassNameNodeIfIsInheritsStatement
    fn get_super_class_name_node_if_is_inherits_statement(
        &self,
        compiler: &AbstractCompiler,
        statement: NodeId,
        class_name: &JsString,
    ) -> Option<NodeId> {
        // $jscomp.inherits(ChildClass, SuperClass);
        if !statement.is_expr_result(compiler) {
            return None;
        }
        let call_node = statement.get_first_child(compiler).unwrap();
        if !call_node.is_call(compiler) {
            return None;
        }
        let jscomp_dot_inherits = call_node.get_first_child(compiler).unwrap();
        if !self.jscomp_inherits.matches(compiler, jscomp_dot_inherits) {
            return None;
        }
        let class_name_node = check_not_null!(jscomp_dot_inherits.get_next(compiler));
        if class_name_node.matches_qualified_name(compiler, class_name.clone()) {
            Some(check_not_null!(class_name_node.get_next(compiler)))
        } else {
            None
        }
    }

    // port: Es6ConvertSuperConstructorCalls#setGlobalNamespace
    pub(crate) fn set_global_namespace(&mut self, global_namespace: GlobalNamespace) {
        self.global_namespace = Some(global_namespace);
    }
}

impl Callback for Es6ConvertSuperConstructorCalls {
    // port: Es6ConvertSuperConstructorCalls#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        if n.is_script(t)
            && !NodeUtil::get_feature_set_of_script(t, n)
                .expect("NullPointerException: getFeatureSetOfScript")
                .contains(Feature::SUPER)
        {
            // If a script contains Feature.SUPER, only then process super constructor calls for it
            return false;
        }
        if n.is_function(t) {
            // TODO(bradfordcsmith): Avoid creating data for non-constructor functions.
            self.constructor_data_stack.push(ConstructorData::new(n));
        } else if n.is_super(t) {
            let parent = parent.unwrap();
            // super(args) or super.prop
            check_state!(
                n.is_first_child_of(t, Some(parent)),
                "%s",
                parent.to_string(t)
            );
            if parent.is_get_prop(t) {
                // TODO(bradfordcsmith): `super.prop` should have been removed before this code
                //     executes, but instead is being left untranspiled when there's no `extends`
                //     clause, so we have to report that problem here.
                t.report(
                    n,
                    &CANNOT_CONVERT_YET,
                    &["super access with no extends clause"],
                );
                return false;
            }
            // must be super(args)
            check_state!(parent.is_call(t), "%s", parent.to_string(t));
            let constructor_data = check_not_null!(self.constructor_data_stack.last_mut());
            constructor_data.super_calls.push(parent);
        }
        true
    }

    // port: Es6ConvertSuperConstructorCalls#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let is_constructor = self
            .constructor_data_stack
            .last()
            .is_some_and(|constructor_data| n == constructor_data.constructor);
        if is_constructor {
            let constructor_data = self.constructor_data_stack.pop().unwrap();
            self.visit_super(t, constructor_data);
        }
    }
}

/// Guava's `Iterables.getOnlyElement`.
fn get_only_element(list: &[NodeId]) -> NodeId {
    match list {
        [only] => *only,
        [] => panic!("NoSuchElementException"),
        _ => panic!("IllegalArgumentException: expected one element"),
    }
}

// port: Es6ConvertSuperConstructorCalls.UndefinedReturnValueCheck
struct UndefinedReturnValueCheck {
    found_non_empty_return: bool,
}

impl UndefinedReturnValueCheck {
    fn new() -> Self {
        Self {
            found_non_empty_return: false,
        }
    }

    // port: Es6ConvertSuperConstructorCalls.UndefinedReturnValueCheck#mayReturnDefinedValue
    fn may_return_defined_value(
        &mut self,
        compiler: &mut AbstractCompiler,
        function_body: NodeId,
    ) -> bool {
        self.found_non_empty_return = false;
        let found_non_empty_return = &mut self.found_non_empty_return;
        let mut check_for_defined_return_value = AbstractShallowCallback::new(
            |t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>| {
                if !*found_non_empty_return
                    && n.is_return(t)
                    && n.has_children(t)
                    && !n.get_first_child(t).unwrap().matches_name(t, "undefined")
                {
                    *found_non_empty_return = true;
                }
            },
        );
        NodeTraversal::traverse(compiler, function_body, &mut check_for_defined_return_value);
        self.found_non_empty_return
    }
}

// port: Es6ConvertSuperConstructorCalls#updateThisToSuperThis (anonymous NodeTraversal.Callback)
struct ReplaceThisWithSuperThis<'a> {
    ast_factory: &'a AstFactory,
    type_of_this: Type,
    super_calls: &'a [NodeId],
    unique_super_this_name: &'a str,
}

impl Callback for ReplaceThisWithSuperThis<'_> {
    // port: Es6ConvertSuperConstructorCalls#updateThisToSuperThis (shouldTraverse)
    fn should_traverse(
        &mut self,
        node_traversal: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        if self.super_calls.contains(&n) {
            false // Leave `this` intact on super calls.
        } else if n.is_function(node_traversal) && !n.is_arrow_function(node_traversal) {
            // Don't replace `this` in non-arrow function definitions.
            false
        } else {
            true
        }
    }

    // port: Es6ConvertSuperConstructorCalls#updateThisToSuperThis (visit)
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_this(t) {
            let super_this = self
                .ast_factory
                .create_name(
                    t.get_compiler(),
                    self.unique_super_this_name,
                    AstFactory::type_node(n),
                )
                .srcref(t, n);
            n.replace_with(t, super_this);
        } else if n.is_return(t) && !n.has_children(t) {
            // An empty return needs to be changed to return $jscomp$super$this
            let name = self
                .ast_factory
                .create_name(
                    t.get_compiler(),
                    self.unique_super_this_name,
                    self.type_of_this.clone(),
                )
                .srcref(t, n);
            n.add_child_to_front(t, name);
        }
    }
}
