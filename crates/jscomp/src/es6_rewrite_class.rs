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
//   src/com/google/javascript/jscomp/Es6RewriteClass.java.

//! Port of `Es6RewriteClass.java`: converts ES6 classes to valid ES5 or ES3 code.

use crate::{
    AbstractCompiler,
    ast_factory::{AstFactory, Type},
    compiler_options::Es6SubclassTranspilation,
    compiler_pass::CompilerPass,
    es6_convert_super_constructor_calls::Es6ConvertSuperConstructorCalls,
    global_namespace::GlobalNamespace,
    js::runtime_js_lib_manager::JsLibField,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    transpilation_namespace::TranspilationNamespace,
    transpilation_passes::TranspilationPasses,
    transpilation_util::TranspilationUtil,
};
use closure_parsing::parser::feature_set::{Feature, FeatureSet};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jscomp_colors::{Color, standard_colors},
    jsdoc_info::{self, JSDocInfo},
    node::{NodeId, Prop},
    token::Token,
};
use std::sync::{Arc, LazyLock};

// port: Es6RewriteClass#features
static FEATURES: LazyLock<FeatureSet> = LazyLock::new(|| {
    FeatureSet::BARE_MINIMUM.with_features(&[
        Feature::CLASSES,
        Feature::CLASS_GETTER_SETTER,
        Feature::NEW_TARGET,
        Feature::SUPER,
    ])
});

/// Converts ES6 classes to valid ES5 or ES3 code.
pub struct Es6RewriteClass {
    ast_factory: AstFactory,
    convert_super_constructor_calls: Es6ConvertSuperConstructorCalls,
    transpilation_namespace: TranspilationNamespace,
    jscomp_inherits: Arc<dyn JsLibField>,
}

impl Es6RewriteClass {
    // port: Es6RewriteClass#Es6RewriteClass
    pub fn new(
        compiler: &mut AbstractCompiler,
        es6_subclass_transpilation: Es6SubclassTranspilation,
    ) -> Self {
        let ast_factory = compiler.create_ast_factory();
        let transpilation_namespace = TranspilationNamespace::get(compiler);
        let convert_super_constructor_calls =
            Es6ConvertSuperConstructorCalls::new(compiler, es6_subclass_transpilation);
        let jscomp_inherits = compiler
            .get_runtime_js_lib_manager()
            .lock()
            .unwrap()
            .get_js_lib_field("$jscomp.inherits");
        Self {
            ast_factory,
            convert_super_constructor_calls,
            transpilation_namespace,
            jscomp_inherits,
        }
    }

    /// Classes are processed in 3 phases:
    ///
    /// 1. The class name is extracted.
    /// 2. Class members are processed and rewritten.
    /// 3. The constructor is built.
    // port: Es6RewriteClass#visitClass
    fn visit_class(&mut self, t: &mut NodeTraversal<'_>, class_node: NodeId, parent: NodeId) {
        // Collect Metadata
        let metadata = ClassDeclarationMetadata::create(
            t.get_compiler(),
            class_node,
            parent,
            &self.ast_factory,
        );

        let Some(mut metadata) = metadata else {
            panic!(
                "Can only convert classes that are declarations or the right hand side of a simple \
                 assignment: {}",
                class_node.to_string(t)
            );
        };
        if metadata.has_super_class(t) {
            check_state!(
                metadata.super_class_name_node().is_qualified_name(t),
                "Expected Es6NormalizeClasses to make all extends clauses into qualified  names, \
                 found %s",
                metadata.super_class_name_node().to_string(t)
            );
        }

        check_state!(
            NodeUtil::is_statement(t, metadata.insertion_point().get_node()),
            "insertion point must be a statement: %s",
            metadata.insertion_point().get_node().to_string(t)
        );

        let mut constructor: Option<NodeId> = None;
        // Process all members of the class
        let class_members = class_node.get_last_child(t).unwrap();
        let mut member_opt = class_members.get_first_child(t);
        while let Some(member) = member_opt {
            let next = member.get_next(t);
            if (member.is_computed_prop(t)
                && (member.get_boolean_prop(t, Prop::COMPUTED_PROP_GETTER)
                    || member.get_boolean_prop(t, Prop::COMPUTED_PROP_SETTER)))
                || (member.is_getter_def(t) || member.is_setter_def(t))
            {
                self.visit_non_method_member(t.get_compiler(), member, &mut metadata);
            } else if NodeUtil::is_es6_constructor_member_function_def(t, member) {
                let class_color = class_node.get_color(t);
                let ctor = member
                    .remove_first_child(t)
                    .unwrap()
                    .set_color(t, class_color);
                constructor = Some(ctor);
                if !metadata.anonymous() {
                    // Turns class Foo { constructor: function() {} } into function Foo() {},
                    // i.e. attaches the name to the ctor function.
                    let name = metadata.class_name_node().clone_node(t);
                    ctor.get_first_child(t).unwrap().replace_with(t, name);
                }
            } else if member.is_empty(t) {
                // Do nothing.
            } else {
                check_state!(
                    member.is_member_function_def(t) || member.is_computed_prop(t),
                    "Unexpected class member: (%s)",
                    member.to_string(t)
                );
                check_state!(
                    !member.get_boolean_prop(t, Prop::COMPUTED_PROP_VARIABLE),
                    "Member variables should have been transpiled earlier: (%s)",
                    member.to_string(t)
                );
                self.visit_method(t.get_compiler(), member, &mut metadata);
            }
            member_opt = next;
        }
        let constructor = check_not_null!(
            constructor,
            "Es6RewriteClasses expects all classes to have (possibly synthetic) constructors"
        );

        self.flush_define_properties_for_prototype(t.get_compiler(), &mut metadata, class_node);
        self.flush_define_properties_for_class(t.get_compiler(), &mut metadata, class_node);

        let class_js_doc = NodeUtil::get_best_jsdoc_info(t, class_node);
        let mut new_info = jsdoc_info::Builder::maybe_copy_from(class_js_doc.as_deref());
        new_info.record_constructor();

        let enclosing_statement = NodeUtil::get_enclosing_statement(t, class_node).unwrap();
        if metadata.has_super_class(t) && !class_node.is_from_externs(t) {
            let compiler = t.get_compiler();
            let callee = self.ast_factory.create_qname_for_field(
                compiler,
                &self.transpilation_namespace,
                self.jscomp_inherits.as_ref(),
            );
            let full_class_name = metadata.full_class_name_node().clone_tree(compiler);
            let super_class_name = metadata.super_class_name_node().clone_tree(compiler);
            let call = self.ast_factory.create_call(
                compiler,
                callee,
                AstFactory::type_(standard_colors::NULL_OR_VOID.clone()),
                &[full_class_name, super_class_name],
            );
            let inherits_call = IR::expr_result(compiler, call)
                .srcref_tree_if_missing(compiler, metadata.super_class_name_node());
            inherits_call.insert_after(compiler, enclosing_statement);
        }

        self.add_type_declarations(t.get_compiler(), &metadata, enclosing_statement);

        if NodeUtil::is_statement(t, class_node) {
            constructor.get_first_child(t).unwrap().set_string(t, "");
            let name = metadata.class_name_node().clone_node(t);
            let ctor_var = IR::let_with_value(t, name, constructor);
            ctor_var.srcref_tree_if_missing(t, class_node);
            class_node.replace_with(t, ctor_var);
            let script = t.get_current_script().unwrap();
            NodeUtil::add_feature_to_script(t.get_compiler(), script, Feature::LET_DECLARATIONS);
        } else {
            class_node.replace_with(t, constructor);
        }
        NodeUtil::mark_functions_deleted(t.get_compiler(), class_node);

        if NodeUtil::is_statement(t, constructor) {
            constructor.set_jsdoc_info(t, new_info.build());
        } else if parent.is_name(t) {
            // The constructor function is the RHS of a var statement.
            // Add the JSDoc to the VAR node.
            let var = parent.get_parent(t).unwrap();
            var.set_jsdoc_info(t, new_info.build());
        } else if constructor.get_parent(t).unwrap().is_name(t) {
            // Is a newly created VAR node.
            let var = constructor.get_grandparent(t).unwrap();
            var.set_jsdoc_info(t, new_info.build());
        } else if parent.is_assign(t) {
            // The constructor function is the RHS of an assignment.
            // Add the JSDoc to the ASSIGN node.
            parent.set_jsdoc_info(t, new_info.build());
        } else {
            panic!("Unexpected parent node {}", parent.to_string(t));
        }

        t.report_code_change();
    }

    // port: Es6RewriteClass#createObjectDotDefineProperties
    fn create_object_dot_define_properties(&self, compiler: &mut AbstractCompiler) -> NodeId {
        self.ast_factory.create_jscomp_dot_global_access(
            compiler,
            &self.transpilation_namespace,
            "Object.defineProperties",
        )
    }

    // port: Es6RewriteClass#createObjectDotDefineProperty
    fn create_object_dot_define_property(&self, compiler: &mut AbstractCompiler) -> NodeId {
        self.ast_factory.create_jscomp_dot_global_access(
            compiler,
            &self.transpilation_namespace,
            "Object.defineProperty",
        )
    }

    // port: Es6RewriteClass#flushDefinePropertiesForPrototype
    fn flush_define_properties_for_prototype(
        &self,
        compiler: &mut AbstractCompiler,
        metadata: &mut ClassDeclarationMetadata,
        srcref_node: NodeId,
    ) {
        if metadata
            .define_properties_obj_for_prototype()
            .has_children(compiler)
        {
            let callee = self.create_object_dot_define_properties(compiler);
            let prototype = metadata.class_prototype_node().clone_tree(compiler);
            let call = self.ast_factory.create_call(
                compiler,
                callee,
                AstFactory::type_node(metadata.class_prototype_node()),
                &[prototype, metadata.define_properties_obj_for_prototype()],
            );
            let define_props_call = IR::expr_result(compiler, call);
            define_props_call.srcref_tree_if_missing(compiler, srcref_node);
            metadata.insert_node_and_advance(compiler, define_props_call);
            metadata.reset_define_properties_obj_for_prototype(compiler, &self.ast_factory);
        }
    }

    // port: Es6RewriteClass#flushDefinePropertiesForClass
    fn flush_define_properties_for_class(
        &self,
        compiler: &mut AbstractCompiler,
        metadata: &mut ClassDeclarationMetadata,
        srcref_node: NodeId,
    ) {
        if metadata
            .define_properties_obj_for_class()
            .has_children(compiler)
        {
            let callee = self.create_object_dot_define_properties(compiler);
            let full_class_name = metadata.full_class_name_node().clone_tree(compiler);
            let call = self.ast_factory.create_call(
                compiler,
                callee,
                AstFactory::type_node(metadata.full_class_name_node()),
                &[full_class_name, metadata.define_properties_obj_for_class()],
            );
            let define_props_call = IR::expr_result(compiler, call);
            define_props_call.srcref_tree_if_missing(compiler, srcref_node);
            metadata.insert_node_and_advance(compiler, define_props_call);
            metadata.reset_define_properties_obj_for_class(compiler, &self.ast_factory);
        }
    }

    // port: Es6RewriteClass#flushPendingDefineProperties
    fn flush_pending_define_properties(
        &self,
        compiler: &mut AbstractCompiler,
        metadata: &mut ClassDeclarationMetadata,
        is_static: bool,
        srcref_node: NodeId,
    ) {
        if is_static {
            self.flush_define_properties_for_class(compiler, metadata, srcref_node);
        } else {
            self.flush_define_properties_for_prototype(compiler, metadata, srcref_node);
        }
    }

    /// `member`: A getter or setter
    // port: Es6RewriteClass#addToDefinePropertiesObject
    fn add_to_define_properties_object(
        &self,
        compiler: &mut AbstractCompiler,
        metadata: &mut ClassDeclarationMetadata,
        member: NodeId,
    ) {
        check_argument!(!member.is_computed_prop(compiler));
        let mut obj = if member.is_static_member(compiler) {
            metadata.define_properties_obj_for_class()
        } else {
            metadata.define_properties_obj_for_prototype()
        };
        let member_string = member.get_string(compiler);
        let mut prop = NodeUtil::get_first_prop_matching_key(compiler, obj, &member_string);
        let accessor_kind = if member.is_getter_def(compiler) {
            "get"
        } else {
            "set"
        };
        if let Some(p) = prop
            && NodeUtil::get_first_prop_matching_key(compiler, p, &JsString::from(accessor_kind))
                .is_some()
        {
            let is_static = member.is_static_member(compiler);
            self.flush_pending_define_properties(compiler, metadata, is_static, member);
            obj = if member.is_static_member(compiler) {
                metadata.define_properties_obj_for_class()
            } else {
                metadata.define_properties_obj_for_prototype()
            };
            prop = None;
        }
        let prop = match prop {
            Some(prop) => prop,
            None => {
                let prop = self.create_property_descriptor(compiler);
                if member_string == "__proto__" {
                    let key = self.ast_factory.create_string(compiler, "__proto__");
                    let string_key = self
                        .ast_factory
                        .create_computed_property(compiler, key, prop);
                    obj.add_child_to_back(compiler, string_key);
                    let script_node = NodeUtil::get_enclosing_script(compiler, member);
                    if let Some(script_node) = script_node {
                        NodeUtil::add_feature_to_script(
                            compiler,
                            script_node,
                            Feature::COMPUTED_PROPERTIES,
                        );
                    }
                } else {
                    let string_key =
                        self.ast_factory
                            .create_string_key(compiler, member_string.clone(), prop);
                    if member.is_quoted_string_key(compiler) {
                        string_key.put_boolean_prop(compiler, Prop::QUOTED, true);
                    }
                    obj.add_child_to_back(compiler, string_key);
                }
                prop
            }
        };

        let function = member.get_last_child(compiler).unwrap();
        let info = NodeUtil::get_best_jsdoc_info(compiler, function);

        let detached = function.detach(compiler);
        let string_key = self
            .ast_factory
            .create_string_key(compiler, accessor_kind, detached);
        string_key.set_jsdoc_info(compiler, info);
        prop.add_child_to_back(compiler, string_key);
        prop.srcref_tree_if_missing(compiler, member);
    }

    /// Appends an Object.defineProperty call defining the given computed getter or setter
    // port: Es6RewriteClass#extractComputedProperty
    fn extract_computed_property(
        &self,
        compiler: &mut AbstractCompiler,
        computed_member: NodeId,
        metadata: &mut ClassDeclarationMetadata,
    ) {
        let is_static = computed_member.is_static_member(compiler);
        self.flush_pending_define_properties(compiler, metadata, is_static, computed_member);

        let owner = if computed_member.is_static_member(compiler) {
            metadata.full_class_name_node()
        } else {
            metadata.class_prototype_node()
        };
        let property = computed_member.remove_first_child(compiler).unwrap();
        let property_value = computed_member.remove_first_child(compiler).unwrap();

        let property_descriptor = self.create_property_descriptor(compiler);
        let string_key = self.ast_factory.create_string_key(
            compiler,
            if computed_member.get_boolean_prop(compiler, Prop::COMPUTED_PROP_GETTER) {
                "get"
            } else {
                "set"
            },
            property_value,
        );
        property_descriptor.add_child_to_back(compiler, string_key);

        let callee = self.create_object_dot_define_property(compiler);
        let owner_clone = owner.clone_tree(compiler);
        let object_define_property_call = self.ast_factory.create_call(
            compiler,
            callee,
            AstFactory::type_node(owner),
            &[owner_clone, property, property_descriptor],
        );

        let statement = IR::expr_result(compiler, object_define_property_call)
            .srcref_tree_if_missing(compiler, computed_member);
        metadata.insert_node_and_advance(compiler, statement);
    }

    /// Visits getters and setters, including both static and instance properties, and computed
    /// and non-computed properties.
    ///
    /// Non-computed getters and setters are aggregated into two Object.defineProperties calls.
    /// One defines static members and the other instance members. This is just an optimization;
    /// it's just as sound to append a single Object.defineProperty definition for each here.
    ///
    /// Computed getters and setters are defined in individual Object.defineProperty calls
    // port: Es6RewriteClass#visitNonMethodMember
    fn visit_non_method_member(
        &self,
        compiler: &mut AbstractCompiler,
        member: NodeId,
        metadata: &mut ClassDeclarationMetadata,
    ) {
        if member.is_computed_prop(compiler) {
            let detached = member.detach(compiler);
            self.extract_computed_property(compiler, detached, metadata);
            return;
        }

        self.add_to_define_properties_object(compiler, metadata, member);

        if !member.is_static_member(compiler) {
            return;
        }
        check_state!(
            !member.is_computed_prop(compiler),
            "%s",
            member.to_string(compiler)
        );
        // Add stub declarations of static properties so that they are not broken by property
        // collapsing

        let mut builder = ClassProperty::builder();
        let member_name = member.get_string(compiler);

        if member.is_quoted_string_key(compiler) {
            builder.kind(PropertyKind::QUOTED_PROPERTY);
        } else {
            builder.kind(PropertyKind::NORMAL_PROPERTY);
        }

        builder
            .property_key(member_name.clone())
            .property_type(member.get_color(compiler));

        let mut js_doc = JSDocInfo::builder();
        js_doc.record_no_collapse();
        builder.js_doc_info(js_doc.build());
        let members_to_declare = metadata.class_members_to_declare();
        members_to_declare.insert(member_name, builder.build());
    }

    /// Handles transpilation of a standard class member function. Getters, setters, and the
    /// constructor are not handled here.
    // port: Es6RewriteClass#visitMethod
    fn visit_method(
        &self,
        compiler: &mut AbstractCompiler,
        member: NodeId,
        metadata: &mut ClassDeclarationMetadata,
    ) {
        let is_static = member.is_static_member(compiler);
        self.flush_pending_define_properties(compiler, metadata, is_static, member);

        let qualified_member_access = self.get_qualified_member_access(compiler, member, metadata);
        let method = member.get_last_child(compiler).unwrap().detach(compiler);

        // Use the source info from the method (a FUNCTION) not the MEMBER_FUNCTION_DEF
        // because the MEMBER_FUNCTION_DEf source info only corresponds to the identifier
        let assign = self
            .ast_factory
            .create_assign(compiler, qualified_member_access, method)
            .srcref_if_missing(compiler, method);

        let mut info = member.get_jsdoc_info(compiler);
        if member.is_static_member(compiler)
            && NodeUtil::references_own_receiver(compiler, assign.get_last_child(compiler).unwrap())
        {
            let mut member_doc = jsdoc_info::Builder::maybe_copy_from(info.as_deref());
            // adding an @this type prevents a JSC_UNSAFE_THIS error later on.
            let qmark = compiler.new_node(Token::QMARK);
            let bang = compiler
                .new_node_with_child(Token::BANG, qmark)
                .srcref_tree(compiler, member);
            let source_file_name = member.get_source_file_name(compiler);
            member_doc.record_this_type(Some(Arc::new(JSTypeExpression::new(
                bang,
                source_file_name.unwrap_or_default(),
            ))));
            info = member_doc.build();
        }
        if info.is_some() {
            assign.set_jsdoc_info(compiler, info);
        }

        let new_node = NodeUtil::new_expr(compiler, assign);
        metadata.insert_node_and_advance(compiler, new_node);
    }

    /// Adds declarations for static properties defined with a getter or setter, so that
    /// optimizations like property collapsing recognize the properties' existence.
    // port: Es6RewriteClass#addTypeDeclarations
    fn add_type_declarations(
        &self,
        compiler: &mut AbstractCompiler,
        metadata: &ClassDeclarationMetadata,
        mut insertion_point: NodeId,
    ) {
        for property in metadata.class_members_to_declare.values() {
            let to_declare_on = metadata.full_class_name_node().clone_tree(compiler);
            let declaration = property.get_declaration(compiler, &self.ast_factory, to_declare_on);
            declaration.srcref_tree_if_missing(compiler, metadata.class_name_node());
            declaration.insert_after(compiler, insertion_point);
            insertion_point = declaration;
        }
    }

    /// Constructs a Node that represents an access to the given class member, qualified by either
    /// the static or the instance access context, depending on whether the member is static.
    ///
    /// **WARNING:** `member` may be modified/destroyed by this method, do not use it afterwards.
    // port: Es6RewriteClass#getQualifiedMemberAccess
    fn get_qualified_member_access(
        &self,
        compiler: &mut AbstractCompiler,
        member: NodeId,
        metadata: &ClassDeclarationMetadata,
    ) -> NodeId {
        let context = if member.is_static_member(compiler) {
            metadata.full_class_name_node().clone_tree(compiler)
        } else {
            metadata.class_prototype_node().clone_tree(compiler)
        };

        context.make_non_indexable_recursive(compiler);
        if member.is_computed_prop(compiler) {
            let key = member.remove_first_child(compiler).unwrap();
            self.ast_factory
                .create_get_elem(compiler, context, key)
                .srcref_tree_if_missing(compiler, member)
        } else {
            let method_name = member.get_first_first_child(compiler).unwrap();
            let member_string = member.get_string(compiler);
            self.ast_factory
                .create_get_prop(
                    compiler,
                    context,
                    member_string,
                    AstFactory::type_node(member),
                )
                .srcref_tree(compiler, method_name)
        }
    }

    // port: Es6RewriteClass#createPropertyDescriptor
    fn create_property_descriptor(&self, compiler: &mut AbstractCompiler) -> NodeId {
        let configurable_value = self.ast_factory.create_boolean(compiler, true);
        let configurable =
            self.ast_factory
                .create_string_key(compiler, "configurable", configurable_value);
        let enumerable_value = self.ast_factory.create_boolean(compiler, true);
        let enumerable =
            self.ast_factory
                .create_string_key(compiler, "enumerable", enumerable_value);
        self.ast_factory
            .create_object_lit(compiler, &[configurable, enumerable])
    }
}

impl CompilerPass for Es6RewriteClass {
    // port: Es6RewriteClass#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        // TODO(b/171853310): This transpilation should be turned off in externs
        TranspilationPasses::process_transpile(compiler, externs, *FEATURES, &mut [self]);
        TranspilationPasses::process_transpile(compiler, root, *FEATURES, &mut [self]);
        // Super constructor calls are done all at once as a separate step largely for historical
        // reasons. It used to be an entirely separate pass, but that has been fixed so we no
        // longer have an invalid AST state between passes.
        // TODO(bradfordcsmith): It would probably be more readable and efficient to merge the
        //     super constructor rewriting logic into this class.
        // The code here only creates the GlobalNamespace object which is very cheap. The expensive
        // building of global namespace happens inside es6ConvertSuperConstructorCalls pass.
        let global_namespace = GlobalNamespace::new(compiler, externs, root);
        self.convert_super_constructor_calls
            .set_global_namespace(global_namespace);
        NodeTraversal::traverse(compiler, root, &mut self.convert_super_constructor_calls);
        TranspilationPasses::maybe_mark_features_as_transpiled_away(compiler, root, *FEATURES);
    }
}

impl Callback for Es6RewriteClass {
    // port: Es6RewriteClass#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::GETTER_DEF | Token::SETTER_DEF
                if FeatureSet::ES3
                    .contains(t.get_compiler().get_options().get_output_feature_set()) =>
            {
                TranspilationUtil::cannot_convert(
                    t.get_compiler(),
                    n,
                    "ES5 getters/setters (consider using --language_out=ES5)",
                );
                return false;
            }
            _ => {}
        }
        true
    }

    // port: Es6RewriteClass#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if n.is_class(t) {
            self.visit_class(t, n, parent.unwrap());
        }
    }
}

// port: Es6RewriteClass.ClassProperty.PropertyKind
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyKind {
    /// Any kind of quoted property, which can include numeric properties that we treated as
    /// quoted.
    ///
    /// ```text
    /// class Example {
    ///   'quoted'() {}
    ///   42() { return 'the answer'; }
    /// }
    /// ```
    QUOTED_PROPERTY,

    /// A computed property, e.g. using bracket [] access.
    ///
    /// Computed properties *must* currently be qualified names, and not literals, function
    /// calls, etc.
    ///
    /// ```text
    /// class Example {
    ///   [variable]() {}
    ///   [another.example]() {}
    /// }
    /// ```
    COMPUTED_PROPERTY,

    /// A normal property definition.
    ///
    /// ```text
    /// class Example {
    ///   normal() {}
    /// }
    /// ```
    NORMAL_PROPERTY,
}

/// `property_key`: The name of this ClassProperty for NORMAL_PROPERTY, the string value of this
/// property if QUOTED_PROPERTY, or the qualified name of the computed property.
// port: Es6RewriteClass.ClassProperty
#[derive(Clone, Debug)]
pub(crate) struct ClassProperty {
    property_key: JsString,
    kind: PropertyKind,
    js_doc_info: Arc<JSDocInfo>,
    property_type: Option<Color>,
}

impl ClassProperty {
    // port: Es6RewriteClass.ClassProperty#ClassProperty
    fn new(
        property_key: Option<JsString>,
        kind: Option<PropertyKind>,
        js_doc_info: Option<Arc<JSDocInfo>>,
        property_type: Option<Color>,
    ) -> Self {
        Self {
            property_key: property_key.expect("propertyKey"),
            kind: kind.expect("kind"),
            js_doc_info: js_doc_info.expect("jsDocInfo"),
            property_type,
        }
    }

    /// Returns an EXPR_RESULT node that declares this property on the given node.
    ///
    /// Examples:
    ///
    /// ```text
    ///   /** @type {string} */
    ///   Class.prototype.property;
    ///
    ///   /** @type {string} */
    ///   Class.staticProperty;
    /// ```
    ///
    /// `to_declare_on`: the node to declare the property on. This should either be a reference
    /// to the class (if a static property) or a class' prototype (if non-static). This should
    /// always be a new node, as this method will insert it into the returned EXPR_RESULT.
    // port: Es6RewriteClass.ClassProperty#getDeclaration
    fn get_declaration(
        &self,
        compiler: &mut AbstractCompiler,
        ast_factory: &AstFactory,
        to_declare_on: NodeId,
    ) -> NodeId {
        let decl = match self.kind {
            PropertyKind::QUOTED_PROPERTY => {
                let key = ast_factory.create_string(compiler, self.property_key.clone());
                ast_factory.create_get_elem(compiler, to_declare_on, key)
            }
            PropertyKind::COMPUTED_PROPERTY => {
                // No need to declare computed properties as they're unaffected by property
                // collapsing
                panic!("UnsupportedOperationException: {self:?}")
            }
            PropertyKind::NORMAL_PROPERTY => ast_factory.create_get_prop(
                compiler,
                to_declare_on,
                self.property_key.clone(),
                AstFactory::type_jstype_and_color(None, self.property_type.clone()),
            ),
        };

        decl.set_jsdoc_info(compiler, Some(self.js_doc_info.clone()));
        ast_factory.expr_result(compiler, decl)
    }

    // port: Es6RewriteClass.ClassProperty#builder
    fn builder() -> ClassPropertyBuilder {
        ClassPropertyBuilder::default()
    }
}

// port: Es6RewriteClass.ClassProperty.Builder
#[derive(Default)]
struct ClassPropertyBuilder {
    property_key: Option<JsString>,
    kind: Option<PropertyKind>,
    js_doc_info: Option<Arc<JSDocInfo>>,
    property_type: Option<Color>,
}

impl ClassPropertyBuilder {
    // port: Es6RewriteClass.ClassProperty.Builder#propertyKey
    fn property_key(&mut self, value: JsString) -> &mut Self {
        self.property_key = Some(value);
        self
    }

    // port: Es6RewriteClass.ClassProperty.Builder#kind
    fn kind(&mut self, value: PropertyKind) -> &mut Self {
        self.kind = Some(value);
        self
    }

    // port: Es6RewriteClass.ClassProperty.Builder#jsDocInfo
    fn js_doc_info(&mut self, value: Option<Arc<JSDocInfo>>) -> &mut Self {
        // AutoBuilder rejects a null for a non-@Nullable property.
        self.js_doc_info = Some(value.expect("Null jsDocInfo"));
        self
    }

    // port: Es6RewriteClass.ClassProperty.Builder#propertyType
    fn property_type(&mut self, type_: Option<Color>) -> &mut Self {
        self.property_type = type_;
        self
    }

    // port: Es6RewriteClass.ClassProperty.Builder#build
    fn build(&mut self) -> ClassProperty {
        ClassProperty::new(
            self.property_key.clone(),
            self.kind,
            self.js_doc_info.clone(),
            self.property_type.clone(),
        )
    }
}

/// Represents static metadata on a class declaration expression - i.e. the qualified name that a
/// class declares (directly or by assignment), whether it's anonymous, and where transpiled code
/// should be inserted (i.e. which object will hold the prototype after transpilation).
///
/// - `insertion_point`: A statement node. Transpiled methods etc of the class are inserted after
///   this node.
/// - `define_properties_obj_for_prototype`: An object literal node that will be used in a call to
///   Object.defineProperties, to add getters and setters to the prototype.
/// - `define_properties_obj_for_class`: An object literal node that will be used in a call to
///   Object.defineProperties, to add getters and setters to the class.
/// - `class_members_to_declare`: Property declarations to be added to the class
/// - `full_class_name_node`: The fully qualified name of the class, as a cloneable node. May come
///   from the class itself or the LHS of an assignment.
/// - `class_prototype_node`: The fully qualified name of this class, plus ".prototype", as a
///   cloneable node with type information as needed.
/// - `anonymous`: Whether the constructor function in the output should be anonymous.
// port: Es6RewriteClass.ClassDeclarationMetadata
pub(crate) struct ClassDeclarationMetadata {
    insertion_point: InsertionPoint,
    define_properties_obj_for_prototype: NodeId,
    define_properties_obj_for_class: NodeId,
    class_members_to_declare: IndexMap<JsString, ClassProperty>,
    full_class_name_node: NodeId,
    class_prototype_node: NodeId,
    anonymous: bool,
    class_name_node: NodeId,
    super_class_name_node: NodeId,
}

impl ClassDeclarationMetadata {
    // port: Es6RewriteClass.ClassDeclarationMetadata#insertionPoint
    fn insertion_point(&self) -> &InsertionPoint {
        &self.insertion_point
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata#definePropertiesObjForPrototype
    fn define_properties_obj_for_prototype(&self) -> NodeId {
        self.define_properties_obj_for_prototype
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata#resetDefinePropertiesObjForPrototype
    fn reset_define_properties_obj_for_prototype(
        &mut self,
        compiler: &mut AbstractCompiler,
        ast_factory: &AstFactory,
    ) {
        self.define_properties_obj_for_prototype = ast_factory.create_object_lit_with_type(
            compiler,
            AstFactory::type_node(self.full_class_name_node),
            &[],
        );
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata#definePropertiesObjForClass
    fn define_properties_obj_for_class(&self) -> NodeId {
        self.define_properties_obj_for_class
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata#resetDefinePropertiesObjForClass
    fn reset_define_properties_obj_for_class(
        &mut self,
        compiler: &mut AbstractCompiler,
        ast_factory: &AstFactory,
    ) {
        self.define_properties_obj_for_class = ast_factory.create_object_lit_with_type(
            compiler,
            AstFactory::type_node(self.full_class_name_node),
            &[],
        );
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata#classMembersToDeclare
    fn class_members_to_declare(&mut self) -> &mut IndexMap<JsString, ClassProperty> {
        &mut self.class_members_to_declare
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata#fullClassNameNode
    fn full_class_name_node(&self) -> NodeId {
        self.full_class_name_node
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata#classPrototypeNode
    fn class_prototype_node(&self) -> NodeId {
        self.class_prototype_node
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata#anonymous
    fn anonymous(&self) -> bool {
        self.anonymous
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata#classNameNode
    fn class_name_node(&self) -> NodeId {
        self.class_name_node
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata#superClassNameNode
    fn super_class_name_node(&self) -> NodeId {
        self.super_class_name_node
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata#builder
    fn builder() -> ClassDeclarationMetadataBuilder {
        let mut builder = ClassDeclarationMetadataBuilder::default();
        builder.set_class_members_to_declare(IndexMap::<_, _>::default());
        builder
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata#create
    fn create(
        compiler: &mut AbstractCompiler,
        class_node: NodeId,
        parent: NodeId,
        ast_factory: &AstFactory,
    ) -> Option<ClassDeclarationMetadata> {
        let class_name_node = class_node.get_first_child(compiler).unwrap();
        let super_class_name_node = class_name_node.get_next(compiler).unwrap();
        let mut builder = ClassDeclarationMetadata::builder();
        builder
            .set_super_class_name_node(super_class_name_node)
            .set_class_name_node(class_name_node);

        // If this is a class statement, or a class expression in a simple
        // assignment or var statement, convert it. In any other case, the
        // code is too dynamic, so return null.
        if NodeUtil::is_class_declaration(compiler, class_node) {
            builder
                .set_insertion_point(InsertionPoint::from(class_node))
                .set_full_class_name_node(class_name_node)
                .set_anonymous(false);
        } else if parent.is_assign(compiler)
            && parent
                .get_parent(compiler)
                .unwrap()
                .is_expr_result(compiler)
        {
            // Add members after the EXPR_RESULT node:
            // example.C = class {}; example.C.prototype.foo = function() {};
            let full_class_name_node = parent.get_first_child(compiler).unwrap();
            if !full_class_name_node.is_qualified_name(compiler) {
                return None;
            }
            builder
                .set_insertion_point(InsertionPoint::from(parent.get_parent(compiler).unwrap()))
                .set_full_class_name_node(full_class_name_node)
                .set_anonymous(true);
        } else if parent.is_export(compiler) {
            builder
                .set_insertion_point(InsertionPoint::from(class_node))
                .set_full_class_name_node(class_name_node)
                .set_anonymous(false);
        } else if parent.is_name(compiler) {
            // Add members after the 'var' statement.
            // var C = class {}; C.prototype.foo = function() {};
            let parent_clone = parent.clone_node(compiler); // specifically don't want children
            builder
                .set_insertion_point(InsertionPoint::from(parent.get_parent(compiler).unwrap()))
                .set_full_class_name_node(parent_clone)
                .set_anonymous(true);
        } else {
            // Cannot handle this class declaration.
            return None;
        }

        // TODO(sdh): are these types safe?
        let class_type: Type = AstFactory::type_node(builder.full_class_name_node());
        let full_class_name_clone = builder.full_class_name_node().clone_tree(compiler);
        let class_prototype_node =
            ast_factory.create_prototype_access(compiler, full_class_name_clone);
        builder.set_class_prototype_node(class_prototype_node);
        let obj_for_class =
            ast_factory.create_object_lit_with_type(compiler, class_type.clone(), &[]);
        builder.set_define_properties_obj_for_class(obj_for_class);
        let obj_for_prototype = ast_factory.create_object_lit_with_type(compiler, class_type, &[]);
        builder.set_define_properties_obj_for_prototype(obj_for_prototype);
        Some(builder.build())
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata#insertNodeAndAdvance
    fn insert_node_and_advance(&mut self, compiler: &mut AbstractCompiler, new_node: NodeId) {
        self.insertion_point
            .insert_node_and_advance(compiler, new_node);
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata#hasSuperClass
    fn has_super_class(&self, ast: &closure_rhino::node::Ast) -> bool {
        !self.super_class_name_node().is_empty(ast)
    }
}

// port: Es6RewriteClass.ClassDeclarationMetadata.Builder
#[derive(Default)]
struct ClassDeclarationMetadataBuilder {
    insertion_point: Option<InsertionPoint>,
    full_class_name_node: Option<NodeId>,
    class_members_to_declare: Option<IndexMap<JsString, ClassProperty>>,
    anonymous: Option<bool>,
    class_name_node: Option<NodeId>,
    super_class_name_node: Option<NodeId>,
    class_prototype_node: Option<NodeId>,
    define_properties_obj_for_class: Option<NodeId>,
    define_properties_obj_for_prototype: Option<NodeId>,
}

impl ClassDeclarationMetadataBuilder {
    // port: Es6RewriteClass.ClassDeclarationMetadata.Builder#setInsertionPoint
    fn set_insertion_point(&mut self, insertion_point: InsertionPoint) -> &mut Self {
        self.insertion_point = Some(insertion_point);
        self
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata.Builder#setFullClassNameNode
    fn set_full_class_name_node(&mut self, full_class_name_node: NodeId) -> &mut Self {
        self.full_class_name_node = Some(full_class_name_node);
        self
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata.Builder#fullClassNameNode
    fn full_class_name_node(&self) -> NodeId {
        // AutoBuilder's property getter throws when the property has not been set.
        self.full_class_name_node
            .expect("Property \"fullClassNameNode\" has not been set")
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata.Builder#setClassMembersToDeclare
    fn set_class_members_to_declare(
        &mut self,
        class_members_to_declare: IndexMap<JsString, ClassProperty>,
    ) -> &mut Self {
        self.class_members_to_declare = Some(class_members_to_declare);
        self
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata.Builder#setAnonymous
    fn set_anonymous(&mut self, anonymous: bool) -> &mut Self {
        self.anonymous = Some(anonymous);
        self
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata.Builder#setClassNameNode
    fn set_class_name_node(&mut self, class_name_node: NodeId) -> &mut Self {
        self.class_name_node = Some(class_name_node);
        self
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata.Builder#setSuperClassNameNode
    fn set_super_class_name_node(&mut self, super_class_name_node: NodeId) -> &mut Self {
        self.super_class_name_node = Some(super_class_name_node);
        self
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata.Builder#setClassPrototypeNode
    fn set_class_prototype_node(&mut self, node: NodeId) -> &mut Self {
        self.class_prototype_node = Some(node);
        self
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata.Builder#setDefinePropertiesObjForClass
    fn set_define_properties_obj_for_class(&mut self, node: NodeId) -> &mut Self {
        self.define_properties_obj_for_class = Some(node);
        self
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata.Builder#setDefinePropertiesObjForPrototype
    fn set_define_properties_obj_for_prototype(&mut self, node: NodeId) -> &mut Self {
        self.define_properties_obj_for_prototype = Some(node);
        self
    }

    // port: Es6RewriteClass.ClassDeclarationMetadata.Builder#build
    // port: Es6RewriteClass.ClassDeclarationMetadata#ClassDeclarationMetadata
    fn build(&mut self) -> ClassDeclarationMetadata {
        ClassDeclarationMetadata {
            insertion_point: self.insertion_point.take().expect("insertionPoint"),
            define_properties_obj_for_prototype: self
                .define_properties_obj_for_prototype
                .expect("definePropertiesObjForPrototype"),
            define_properties_obj_for_class: self
                .define_properties_obj_for_class
                .expect("definePropertiesObjForClass"),
            class_members_to_declare: self
                .class_members_to_declare
                .take()
                .expect("classMembersToDeclare"),
            full_class_name_node: self.full_class_name_node.expect("fullClassNameNode"),
            class_prototype_node: self.class_prototype_node.expect("classPrototypeNode"),
            class_name_node: self.class_name_node.expect("classNameNode"),
            super_class_name_node: self.super_class_name_node.expect("superClassNameNode"),
            anonymous: self.anonymous.expect("anonymous"),
        }
    }
}

/// Used by ClassDeclarationMetadata to represent the point where we insert the next transpiled
/// method of a class
// port: Es6RewriteClass.InsertionPoint
pub(crate) struct InsertionPoint {
    insertion_point: NodeId,
}

impl InsertionPoint {
    // port: Es6RewriteClass.InsertionPoint#InsertionPoint
    fn new(insertion_point: NodeId) -> Self {
        Self { insertion_point }
    }

    // port: Es6RewriteClass.InsertionPoint#insertNodeAndAdvance
    fn insert_node_and_advance(&mut self, compiler: &mut AbstractCompiler, new_node: NodeId) {
        new_node.insert_after(compiler, self.insertion_point);
        self.insertion_point = new_node;
    }

    // port: Es6RewriteClass.InsertionPoint#from
    fn from(start: NodeId) -> InsertionPoint {
        InsertionPoint::new(start)
    }

    // port: Es6RewriteClass.InsertionPoint#getNode
    fn get_node(&self) -> NodeId {
        self.insertion_point
    }
}
