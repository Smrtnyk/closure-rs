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
//   src/com/google/javascript/jscomp/CrossChunkMethodMotion.java.

//! Port of CrossChunkMethodMotion.java: move prototype methods into later chunks.

use crate::abstract_compiler::AbstractCompiler;
use crate::analyze_prototype_properties::{AnalyzePrototypeProperties, NameInfoId, Symbol};
use crate::ast_factory::AstFactory;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::id_generator::IdGenerator;
use crate::js_chunk::JSChunk;
use crate::js_error::JSError;
use crate::node_util::NodeUtil;
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use closure_rhino::{check_argument, check_not_null, check_state};

// Internal errors
// port: CrossChunkMethodMotion#NULL_COMMON_CHUNK_ERROR
pub static NULL_COMMON_CHUNK_ERROR: DiagnosticType = DiagnosticType::error(
    "JSC_INTERNAL_ERROR_CHUNK_DEPEND",
    "null deepest common chunk",
);

/// The `IdGenerator` Java passes to the constructor. DefaultPassConfig passes the compiler-owned
/// `compiler.getCrossChunkIdGenerator()`, which a pass cannot hold while it mutates the compiler,
/// so it is named here and fetched at each use; tests pass `new IdGenerator()`.
pub enum CrossChunkIdGenerator {
    /// `compiler.getCrossChunkIdGenerator()`
    Compiler,
    /// An id generator owned by this pass.
    Owned(IdGenerator),
}

/// Move prototype methods into later chunks.
pub struct CrossChunkMethodMotion {
    id_generator: CrossChunkIdGenerator,
    analyzer: AnalyzePrototypeProperties,
    no_stub_functions: bool,
    ast_factory: AstFactory,
}

impl CrossChunkMethodMotion {
    // port: CrossChunkMethodMotion#STUB_METHOD_NAME
    pub const STUB_METHOD_NAME: &'static str = "JSCompiler_stubMethod";
    // port: CrossChunkMethodMotion#UNSTUB_METHOD_NAME
    pub const UNSTUB_METHOD_NAME: &'static str = "JSCompiler_unstubMethod";

    // port: CrossChunkMethodMotion#STUB_DECLARATIONS
    pub const STUB_DECLARATIONS: &'static str = concat!(
        "var JSCompiler_stubMap = [];",
        "function JSCompiler_stubMethod(JSCompiler_stubMethod_id) {",
        "  return function() {",
        "    return JSCompiler_stubMap[JSCompiler_stubMethod_id].apply(",
        "        this, arguments);",
        "  };",
        "}",
        "function JSCompiler_unstubMethod(",
        "    JSCompiler_unstubMethod_id, JSCompiler_unstubMethod_body) {",
        "  return JSCompiler_stubMap[JSCompiler_unstubMethod_id] = ",
        "      JSCompiler_unstubMethod_body;",
        "}"
    );

    /// Creates a new pass for moving prototype properties.
    ///
    /// @param id_generator An id generator for method stubs.
    /// @param can_modify_externs If true, then we can move prototype properties that are declared
    ///     in the externs file.
    /// @param no_stub_functions if true, we can move methods without stub functions in the parent
    ///     chunk.
    // port: CrossChunkMethodMotion#CrossChunkMethodMotion
    pub fn new(
        compiler: &mut AbstractCompiler,
        id_generator: CrossChunkIdGenerator,
        can_modify_externs: bool,
        no_stub_functions: bool,
    ) -> Self {
        let analyzer = AnalyzePrototypeProperties::new(
            compiler.get_chunk_graph(),
            can_modify_externs,
            /* anchor_unused_vars= */ false,
            no_stub_functions,
        );
        Self {
            id_generator,
            analyzer,
            no_stub_functions,
            ast_factory: compiler.create_ast_factory(),
        }
    }

    fn id_generator<'s>(&'s mut self, compiler: &'s mut AbstractCompiler) -> &'s mut IdGenerator {
        match &mut self.id_generator {
            CrossChunkIdGenerator::Compiler => compiler.get_cross_chunk_id_generator(),
            CrossChunkIdGenerator::Owned(id_generator) => id_generator,
        }
    }

    /// Move methods deeper in the chunk graph when possible.
    // port: CrossChunkMethodMotion#moveMethods
    fn move_methods(&mut self, compiler: &mut AbstractCompiler, all_name_info: Vec<NameInfoId>) {
        let has_stub_declaration = self.id_generator(compiler).has_generated_any_ids();
        for name_info in all_name_info {
            let info = self.analyzer.get_name_info(name_info);
            if !info.is_referenced() {
                // The code below can't do anything with unreferenced name
                // infos.  They should be skipped to avoid NPE since their
                // deepestCommonChunkRef is null.
                continue;
            }

            if info.reads_closure_variables() {
                continue;
            }

            let Some(deepest_common_chunk_ref) = info.get_deepest_common_chunk_ref() else {
                compiler.report(JSError::make_without_location(
                    &NULL_COMMON_CHUNK_ERROR,
                    &[],
                ));
                continue;
            };

            let declaration_count = info.get_declarations().len();
            for i in (0..declaration_count).rev() {
                let symbol = &self.analyzer.get_name_info(name_info).get_declarations()[i];
                if symbol.is_prototype_property() {
                    self.try_to_move_prototype_method(
                        compiler,
                        name_info,
                        &deepest_common_chunk_ref,
                        i,
                    );
                } else if matches!(symbol, Symbol::ClassMemberFunction(_)) {
                    self.try_to_move_member_function(
                        compiler,
                        name_info,
                        &deepest_common_chunk_ref,
                        i,
                    );
                } // else it's a variable definition, and we don't move those.
            }
        }

        if !self.no_stub_functions
            && !has_stub_declaration
            && self.id_generator(compiler).has_generated_any_ids()
        {
            // Declare stub functions in the top-most chunk.
            let declarations =
                compiler.parse_synthetic_code("chunk_method_stubbing", Self::STUB_DECLARATIONS);
            NodeUtil::mark_new_scopes_changed(compiler, declarations);
            let first_script = compiler.get_node_for_code_insertion(None);
            let children = declarations.remove_children(compiler);
            first_script.add_children_to_front(compiler, children);
            compiler.report_change_to_enclosing_scope(first_script);
        }
    }

    // port: CrossChunkMethodMotion#tryToMovePrototypeMethod
    fn try_to_move_prototype_method(
        &mut self,
        compiler: &mut AbstractCompiler,
        name_info: NameInfoId,
        deepest_common_chunk_ref: &JSChunk,
        prop: usize,
    ) {
        // We should only move a property across chunks if:
        // 1) We can move it deeper in the chunk graph, and
        // 2) it's a function, and
        // 3) it is not a GETTER_DEF or a SETTER_DEF, and
        // 4) it does not refer to `super`
        // 5) the class is available in the global scope.
        //
        // #1 should be obvious. #2 is more subtle. It's possible
        // to copy off of a prototype, as in the code:
        // for (var k in Foo.prototype) {
        //   doSomethingWith(Foo.prototype[k]);
        // }
        // This is a common way to implement pseudo-multiple inheritance in JS.
        //
        // So if we move a prototype method into a deeper chunk, we must
        // replace it with a stub function so that it preserves its original
        // behavior.
        let info = self.analyzer.get_name_info(name_info);
        let symbol = &info.get_declarations()[prop];
        if symbol
            .get_root_var()
            .is_none_or(|root_var| !root_var.is_global(compiler))
        {
            return;
        }

        if info.references_super() {
            // It is illegal to move `super` outside of a member function def.
            return;
        }

        let value = symbol.get_value(compiler);
        let value_parent = value.get_parent(compiler).unwrap();
        // Only attempt to move normal functions.
        if !value.is_function(compiler)
            // A GET or SET can't be deferred like a normal
            // FUNCTION property definition as a mix-in would get the result
            // of a GET instead of the function itself.
            || value_parent.is_getter_def(compiler)
            || value_parent.is_setter_def(compiler)
        {
            return;
        }

        let prop_chunk = symbol.get_chunk().unwrap();
        if compiler
            .get_chunk_graph()
            .unwrap()
            .depends_on(deepest_common_chunk_ref, prop_chunk)
        {
            if Self::has_unmovable_redeclaration(&self.analyzer, name_info, prop) {
                // If it has been redeclared on the same object, skip it.
                return;
            }
            let name = info.name.clone();
            let dest_parent = compiler.get_node_for_code_insertion(Some(deepest_common_chunk_ref));
            if value_parent.is_member_function_def(compiler) {
                self.move_prototype_object_literal_method_shorthand(
                    compiler,
                    &name,
                    dest_parent,
                    value,
                );
            } else if value_parent.is_string_key(compiler) {
                self.move_prototype_object_literal_property(compiler, &name, dest_parent, value);
            } else {
                // Note that computed properties should have been filtered out by
                // AnalyzePrototypeProperties, because they don't have a recognizable property
                // name. Getters and setters are filtered out by the code above
                check_state!(
                    value_parent.is_assign(compiler),
                    "%s",
                    value_parent.to_string(compiler)
                );
                self.move_prototype_dot_method_assignment(compiler, dest_parent, value);
            }
        }
    }

    /// Move a property defined by object literal assigned to `.prototype`.
    ///
    /// ```text
    ///     Foo.prototype = { propName: function() {}};
    /// ```
    // port: CrossChunkMethodMotion#movePrototypeObjectLiteralProperty
    fn move_prototype_object_literal_property(
        &mut self,
        compiler: &mut AbstractCompiler,
        prop_name: &JsString,
        dest_parent: NodeId,
        function_node: NodeId,
    ) {
        check_state!(
            function_node.is_function(compiler),
            "%s",
            function_node.to_string(compiler)
        );
        let string_key = function_node.get_parent(compiler).unwrap();
        check_state!(
            string_key.is_string_key(compiler),
            "%s",
            string_key.to_string(compiler)
        );
        let prototype_object_literal = string_key.get_parent(compiler).unwrap();
        check_state!(
            prototype_object_literal.is_object_lit(compiler),
            "%s",
            prototype_object_literal.to_string(compiler)
        );
        let assign_node = prototype_object_literal.get_parent(compiler).unwrap();
        check_state!(
            assign_node.is_assign(compiler)
                && prototype_object_literal.is_second_child_of(compiler, Some(assign_node)),
            "%s",
            assign_node.to_string(compiler)
        );
        let owner_dot_prototype_node = assign_node.get_first_child(compiler).unwrap();
        check_state!(
            owner_dot_prototype_node.is_qualified_name(compiler)
                && owner_dot_prototype_node.get_string_ref(compiler) == "prototype",
            "%s",
            owner_dot_prototype_node.to_string(compiler)
        );

        if self.no_stub_functions {
            // Remove the definition from the object literal
            string_key.detach(compiler);
            compiler.report_change_to_enclosing_scope(prototype_object_literal);

            // Prepend definition to new chunk
            // Foo.prototype.propName = function() {};
            let receiver = owner_dot_prototype_node.clone_tree(compiler);
            let type_ = AstFactory::type_node(function_node);
            let owner_dot_prototype_dot_prop_name = self
                .ast_factory
                .create_get_prop(compiler, receiver, prop_name, type_);
            function_node.detach(compiler);
            let definition_statement = self
                .ast_factory
                .create_assign_statement(compiler, owner_dot_prototype_dot_prop_name, function_node)
                .srcref_tree_if_missing(compiler, string_key);
            dest_parent.add_child_to_front(compiler, definition_statement);
            compiler.report_change_to_enclosing_scope(dest_parent);
        } else {
            let stub_id = self.id_generator(compiler).new_id();
            // { propName: function() {} } => { propName: JSCompiler_stubMethod(0) }
            let stub_call = self.create_stub_call(compiler, function_node, stub_id);
            function_node.replace_with(compiler, stub_call);
            compiler.report_change_to_enclosing_scope(prototype_object_literal);

            // Prepend definition to new chunk
            // Foo.prototype.propName = function() {};
            let receiver = owner_dot_prototype_node.clone_tree(compiler);
            let type_ = AstFactory::type_node(function_node);
            let owner_dot_prototype_dot_prop_name = self
                .ast_factory
                .create_get_prop(compiler, receiver, prop_name, type_);
            let unstub_call = self.create_unstub_call(compiler, function_node, stub_id);
            let definition_statement = self
                .ast_factory
                .create_assign_statement(compiler, owner_dot_prototype_dot_prop_name, unstub_call)
                .srcref_tree_if_missing(compiler, string_key);
            dest_parent.add_child_to_front(compiler, definition_statement);
            compiler.report_change_to_enclosing_scope(dest_parent);
        }
    }

    /// Move a property defined by assignment to `.prototype` or `.prototype.propName`.
    ///
    /// ```text
    ///     Foo.prototype.propName = function() {};
    /// ```
    // port: CrossChunkMethodMotion#movePrototypeDotMethodAssignment
    fn move_prototype_dot_method_assignment(
        &mut self,
        compiler: &mut AbstractCompiler,
        dest_parent: NodeId,
        function_node: NodeId,
    ) {
        check_state!(
            function_node.is_function(compiler),
            "%s",
            function_node.to_string(compiler)
        );
        let assign_node = function_node.get_parent(compiler).unwrap();
        check_state!(
            assign_node.is_assign(compiler)
                && function_node.is_second_child_of(compiler, Some(assign_node)),
            "%s",
            assign_node.to_string(compiler)
        );
        let definition_statement = assign_node.get_parent(compiler).unwrap();
        check_state!(
            definition_statement.is_expr_result(compiler),
            "%s",
            assign_node.to_string(compiler)
        );

        if self.no_stub_functions {
            // Remove the definition statement from its current location
            let assign_statement_parent = definition_statement.get_parent(compiler).unwrap();
            definition_statement.detach(compiler);
            compiler.report_change_to_enclosing_scope(assign_statement_parent);

            // Prepend definition to new chunk
            // Foo.prototype.propName = function() {};
            dest_parent.add_child_to_front(compiler, definition_statement);
            compiler.report_change_to_enclosing_scope(dest_parent);
        } else {
            let stub_id = self.id_generator(compiler).new_id();

            // replace function definition with temporary placeholder so we can clone the whole
            // assignment statement without cloning the function definition itself.
            let original_definition_placeholder = self.ast_factory.create_empty(compiler);
            function_node.replace_with(compiler, original_definition_placeholder);
            let new_definition_statement = definition_statement.clone_tree(compiler);
            let new_definition_placeholder = new_definition_statement // EXPR_RESULT
                .get_only_child(compiler) // ASSIGN
                .get_last_child(compiler) // EMPTY RHS node
                .unwrap();

            // convert original assignment statement to
            // owner.prototype.propName = JSCompiler_stubMethod(0);
            let stub_call = self.create_stub_call(compiler, function_node, stub_id);
            original_definition_placeholder.replace_with(compiler, stub_call);
            compiler.report_change_to_enclosing_scope(definition_statement);

            // Prepend new definition to new chunk
            // Foo.prototype.propName = JSCompiler_unstubMethod(0, function() {});
            let unstub_call = self.create_unstub_call(compiler, function_node, stub_id);
            new_definition_placeholder.replace_with(compiler, unstub_call);
            dest_parent.add_child_to_front(compiler, new_definition_statement);
            compiler.report_change_to_enclosing_scope(dest_parent);
        }
    }

    /// Move a property defined by assignment to `.prototype` or `.prototype.propName`.
    ///
    /// ```text
    ///     Foo.prototype = { propName() {}};
    /// ```
    // port: CrossChunkMethodMotion#movePrototypeObjectLiteralMethodShorthand
    fn move_prototype_object_literal_method_shorthand(
        &mut self,
        compiler: &mut AbstractCompiler,
        prop_name: &JsString,
        dest_parent: NodeId,
        function_node: NodeId,
    ) {
        check_state!(
            function_node.is_function(compiler),
            "%s",
            function_node.to_string(compiler)
        );
        let member_function_def = function_node.get_parent(compiler).unwrap();
        check_state!(
            member_function_def.is_member_function_def(compiler),
            "%s",
            member_function_def.to_string(compiler)
        );
        let prototype_object_literal = member_function_def.get_parent(compiler).unwrap();
        check_state!(
            prototype_object_literal.is_object_lit(compiler),
            "%s",
            prototype_object_literal.to_string(compiler)
        );
        let assign_node = prototype_object_literal.get_parent(compiler).unwrap();
        check_state!(
            assign_node.is_assign(compiler)
                && prototype_object_literal.is_second_child_of(compiler, Some(assign_node)),
            "%s",
            assign_node.to_string(compiler)
        );
        let owner_dot_prototype_node = assign_node.get_first_child(compiler).unwrap();
        check_state!(
            owner_dot_prototype_node.is_qualified_name(compiler)
                && owner_dot_prototype_node.get_string_ref(compiler) == "prototype",
            "%s",
            owner_dot_prototype_node.to_string(compiler)
        );
        let original_script =
            NodeUtil::get_enclosing_script(compiler, member_function_def).unwrap();

        if self.no_stub_functions {
            // Remove the definition from the object literal
            member_function_def.detach(compiler);
            compiler.report_change_to_enclosing_scope(prototype_object_literal);

            // Prepend definition to new chunk
            // Foo.prototype.propName = function() {};
            let receiver = owner_dot_prototype_node.clone_tree(compiler);
            let type_ = AstFactory::type_node(function_node);
            let owner_dot_prototype_dot_prop_name = self
                .ast_factory
                .create_get_prop(compiler, receiver, prop_name, type_);
            let function_node = function_node.detach(compiler);
            let definition_statement = self
                .ast_factory
                .create_assign_statement(compiler, owner_dot_prototype_dot_prop_name, function_node)
                .srcref_tree_if_missing(compiler, member_function_def);
            dest_parent.add_child_to_front(compiler, definition_statement);
            let features = NodeUtil::get_feature_set_of_script(compiler, original_script).unwrap();
            NodeUtil::add_features_to_script(compiler, dest_parent, features);
            compiler.report_change_to_enclosing_scope(dest_parent);
        } else {
            let stub_id = self.id_generator(compiler).new_id();
            // { propName() {} } => { propName: JSCompiler_stubMethod(0) }
            let stub_call = self.create_stub_call(compiler, function_node, stub_id);
            let string_key = self
                .ast_factory
                .create_string_key(compiler, prop_name, stub_call);
            member_function_def.replace_with(compiler, string_key);
            compiler.report_change_to_enclosing_scope(prototype_object_literal);

            // Prepend definition to new chunk
            // Foo.prototype.propName = function() {};
            let receiver = owner_dot_prototype_node.clone_tree(compiler);
            let type_ = AstFactory::type_node(function_node);
            let owner_dot_prototype_dot_prop_name = self
                .ast_factory
                .create_get_prop(compiler, receiver, prop_name, type_);
            let detached = function_node.detach(compiler);
            let unstub_call = self.create_unstub_call(compiler, detached, stub_id);
            let definition_statement = self
                .ast_factory
                .create_assign_statement(compiler, owner_dot_prototype_dot_prop_name, unstub_call)
                .srcref_tree_if_missing(compiler, member_function_def);
            dest_parent.add_child_to_front(compiler, definition_statement);
            let features = NodeUtil::get_feature_set_of_script(compiler, original_script).unwrap();
            NodeUtil::add_features_to_script(compiler, dest_parent, features);

            compiler.report_change_to_enclosing_scope(dest_parent);
        }
    }

    /// Returns a new Node to be used as the stub definition for a method.
    ///
    /// @param original_definition function Node whose definition is being stubbed
    /// @param stub_id ID to use for stubbing and unstubbing
    /// @return a Node that looks like `JSCompiler_stubMethod(0)`
    // port: CrossChunkMethodMotion#createStubCall
    fn create_stub_call(
        &self,
        compiler: &mut AbstractCompiler,
        original_definition: NodeId,
        stub_id: i32,
    ) -> NodeId {
        // We can't look up the type of the stub creating method, because we add its
        // definition after type checking.
        let callee = self
            .ast_factory
            .create_name_with_unknown_type(compiler, Self::STUB_METHOD_NAME);
        let type_ = AstFactory::type_node(original_definition);
        let id = self.ast_factory.create_number(compiler, f64::from(stub_id));
        self.ast_factory
            .create_call(compiler, callee, type_, &[id])
            .srcref_tree_if_missing(compiler, original_definition)
    }

    /// Returns a new Node to be used as the stub definition for a method.
    ///
    /// @param function_node actual function definition to be attached. Must be detached now.
    /// @param stub_id ID to use for stubbing and unstubbing
    /// @return a Node that looks like `JSCompiler_unstubMethod(0, function() {})`
    // port: CrossChunkMethodMotion#createUnstubCall
    fn create_unstub_call(
        &self,
        compiler: &mut AbstractCompiler,
        function_node: NodeId,
        stub_id: i32,
    ) -> NodeId {
        // We can't look up the type of the stub creating method, because we add its
        // definition after type checking.
        let callee = self
            .ast_factory
            .create_name_with_unknown_type(compiler, Self::UNSTUB_METHOD_NAME);
        let type_ = AstFactory::type_node(function_node);
        let id = self.ast_factory.create_number(compiler, f64::from(stub_id));
        self.ast_factory
            .create_call(compiler, callee, type_, &[id, function_node])
            .srcref_tree_if_missing(compiler, function_node)
    }

    /// If possible, move a class instance member function definition to the deepest chunk common
    /// to all uses of the method.
    ///
    /// @param name_info information about all definitions of the given property name
    /// @param deepest_common_chunk_ref all uses of the method are either in this chunk or in
    ///     chunks that depend on it
    /// @param class_member_function definition of the method within its class body
    // port: CrossChunkMethodMotion#tryToMoveMemberFunction
    fn try_to_move_member_function(
        &mut self,
        compiler: &mut AbstractCompiler,
        name_info: NameInfoId,
        deepest_common_chunk_ref: &JSChunk,
        class_member_function: usize,
    ) {
        // We should only move a property across chunks if:
        // 1) We can move it deeper in the chunk graph,
        // 2) and it's a normal member function, and not a GETTER_DEF or a SETTER_DEF, or
        //    or the class constructor.
        // 3) and it does not refer to `super`, which would be invalid outside of the class.
        // 4) and the class is available in the global scope.
        let info = self.analyzer.get_name_info(name_info);
        let Symbol::ClassMemberFunction(member) = &info.get_declarations()[class_member_function]
        else {
            unreachable!()
        };
        let root_var = member.get_root_var();
        let Some(root_var) = root_var.filter(|root_var| root_var.is_global(compiler)) else {
            return;
        };

        let definition_node = member.get_definition_node();

        // Only attempt to move normal member functions.
        // A getter or setter cannot be as easily defined outside of the class to which it
        // belongs.
        if !definition_node.is_member_function_def(compiler) {
            return;
        }

        if NodeUtil::is_es6_constructor_member_function_def(compiler, definition_node) {
            // Constructor functions cannot be moved.
            return;
        }

        if info.references_super() {
            // Do not move methods containing `super`, because it doesn't work outside of a
            // class method or object literal method.
            return;
        }

        if compiler
            .get_chunk_graph()
            .unwrap()
            .depends_on(deepest_common_chunk_ref, member.get_chunk().unwrap())
        {
            if Self::has_unmovable_redeclaration(&self.analyzer, name_info, class_member_function) {
                // If it has been redeclared on the same object, skip it.
                return;
            }

            let destination_parent =
                compiler.get_node_for_code_insertion(Some(deepest_common_chunk_ref));
            let class_name_node = root_var.get_name_node(compiler).unwrap();
            if self.no_stub_functions {
                self.move_class_instance_method_without_stub(
                    compiler,
                    class_name_node,
                    definition_node,
                    destination_parent,
                );
            } else {
                self.move_class_instance_method_with_stub(
                    compiler,
                    class_name_node,
                    definition_node,
                    destination_parent,
                );
            }
        }
    }

    // port: CrossChunkMethodMotion#moveClassInstanceMethodWithoutStub
    fn move_class_instance_method_without_stub(
        &mut self,
        compiler: &mut AbstractCompiler,
        class_name_node: NodeId,
        method_definition: NodeId,
        destination_parent: NodeId,
    ) {
        check_argument!(
            method_definition.is_member_function_def(compiler),
            "%s",
            method_definition.to_string(compiler)
        );
        let class_members = check_not_null!(method_definition.get_parent(compiler));
        check_state!(
            class_members.is_class_members(compiler),
            "%s",
            class_members.to_string(compiler)
        );
        let class_node = class_members.get_parent(compiler).unwrap();
        check_state!(
            class_node.is_class(compiler),
            "%s",
            class_node.to_string(compiler)
        );
        let original_script = NodeUtil::get_enclosing_script(compiler, method_definition).unwrap();

        method_definition.detach(compiler);
        compiler.report_change_to_enclosing_scope(class_members);

        // ClassName.prototype.propertyName = function() {};
        let function_node = method_definition.get_only_child(compiler);
        let cloned_name_node = class_name_node
            .clone_node(compiler)
            .copy_type_from(compiler, class_node);
        let prototype_access = self
            .ast_factory
            .create_prototype_access(compiler, cloned_name_node);
        let property_name = method_definition.get_string(compiler);
        let type_ = AstFactory::type_node(function_node);
        let class_name_dot_prototype_dot_prop_name =
            self.ast_factory
                .create_get_prop(compiler, prototype_access, &property_name, type_);
        function_node.detach(compiler);
        let definition_statement_node = self
            .ast_factory
            .create_assign_statement(
                compiler,
                class_name_dot_prototype_dot_prop_name,
                function_node,
            )
            .srcref_tree_if_missing(compiler, method_definition);
        destination_parent.add_child_to_front(compiler, definition_statement_node);
        let features = NodeUtil::get_feature_set_of_script(compiler, original_script).unwrap();
        NodeUtil::add_features_to_script(compiler, destination_parent, features);
        compiler.report_change_to_enclosing_scope(destination_parent);
    }

    // port: CrossChunkMethodMotion#moveClassInstanceMethodWithStub
    fn move_class_instance_method_with_stub(
        &mut self,
        compiler: &mut AbstractCompiler,
        class_name_node: NodeId,
        method_definition: NodeId,
        destination_parent: NodeId,
    ) {
        check_argument!(
            method_definition.is_member_function_def(compiler),
            "%s",
            method_definition.to_string(compiler)
        );
        let class_members = check_not_null!(method_definition.get_parent(compiler));
        check_state!(
            class_members.is_class_members(compiler),
            "%s",
            class_members.to_string(compiler)
        );
        let class_node = class_members.get_parent(compiler).unwrap();
        check_state!(
            class_node.is_class(compiler),
            "%s",
            class_node.to_string(compiler)
        );

        let stub_id = self.id_generator(compiler).new_id();

        // Put a stub definition after the class
        // ClassName.prototype.propertyName = JSCompiler_stubMethod(id);
        let cloned_name_node = class_name_node
            .clone_node(compiler)
            .copy_type_from(compiler, class_node);
        let prototype_access = self
            .ast_factory
            .create_prototype_access(compiler, cloned_name_node);
        let property_name = method_definition.get_string(compiler);
        let type_ = AstFactory::type_node(method_definition);
        let class_name_dot_prototype_dot_prop_name =
            self.ast_factory
                .create_get_prop(compiler, prototype_access, &property_name, type_);
        let stub_call = self.create_stub_call(compiler, method_definition, stub_id);
        let stub_definition_statement = self
            .ast_factory
            .create_assign_statement(compiler, class_name_dot_prototype_dot_prop_name, stub_call)
            .srcref_tree_if_missing(compiler, method_definition);
        let class_defining_statement =
            NodeUtil::get_enclosing_statement(compiler, class_members).unwrap();
        stub_definition_statement.insert_after(compiler, class_defining_statement);

        let original_script = NodeUtil::get_enclosing_script(compiler, method_definition).unwrap();

        // remove the definition from the class
        method_definition.detach(compiler);

        compiler.report_change_to_enclosing_scope(class_members);

        // Prepend unstub definition to the new location.
        // ClassName.prototype.propertyName = JSCompiler_unstubMethod(id, function(...) {...});
        let class_name_dot_prototype_dot_prop_name2 =
            class_name_dot_prototype_dot_prop_name.clone_tree(compiler);
        let function_node = method_definition.get_only_child(compiler);
        function_node.detach(compiler);
        let unstub_call = self.create_unstub_call(compiler, function_node, stub_id);
        let statement_node = self
            .ast_factory
            .create_assign_statement(
                compiler,
                class_name_dot_prototype_dot_prop_name2,
                unstub_call,
            )
            .srcref_tree_if_missing(compiler, method_definition);
        destination_parent.add_child_to_front(compiler, statement_node);
        let features = NodeUtil::get_feature_set_of_script(compiler, original_script).unwrap();
        NodeUtil::add_features_to_script(compiler, destination_parent, features);
        compiler.report_change_to_enclosing_scope(destination_parent);
    }

    /// Java's `prop` is a declaration of `nameInfo` (identity compare = index compare).
    // port: CrossChunkMethodMotion#hasUnmovableRedeclaration
    pub fn has_unmovable_redeclaration(
        analyzer: &AnalyzePrototypeProperties,
        name_info: NameInfoId,
        prop: usize,
    ) -> bool {
        let declarations = analyzer.get_name_info(name_info).get_declarations();
        let prop_symbol = &declarations[prop];
        for (other_index, symbol) in declarations.iter().enumerate() {
            if symbol.is_property() {
                let other_prop = symbol;
                // It is possible to do better here if the dependencies are well defined
                // but redefinitions are usually in optional chunks so it isn't likely
                // worth the effort to check.
                if prop != other_index
                    && prop_symbol.get_root_var() == other_prop.get_root_var()
                    && prop_symbol.get_chunk() != other_prop.get_chunk()
                {
                    return true;
                }
            }
        }
        false
    }
}

impl CompilerPass for CrossChunkMethodMotion {
    // port: CrossChunkMethodMotion#process
    fn process(&mut self, compiler: &mut AbstractCompiler, extern_root: NodeId, root: NodeId) {
        // If there are < 2 chunks, then we will never move anything,
        // so we're done.
        if compiler.get_chunk_graph().unwrap().get_chunk_count() > 1 {
            self.analyzer.process(compiler, extern_root, root);
            let all_name_info = self.analyzer.get_all_name_info();
            self.move_methods(compiler, all_name_info);
        }
    }
}
