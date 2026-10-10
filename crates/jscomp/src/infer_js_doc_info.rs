/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/InferJSDocInfo.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Sets the JSDocInfo on all JSTypes, including their properties, using the JSDoc on the node
//! defining that type or property.
//!
//! This pass propagates JSDocs across the type graph, but not across the symbol graph (see the
//! class comment of Java's `com.google.javascript.jscomp.InferJSDocInfo` for the three cases:
//! nominal types, object type properties and anonymous structural types).
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_jstype::{
    JSTypeRegistry, TypeId,
    enum_type::EnumType,
    js_type,
    prelude::{FunctionType, JSType, ObjectType},
    property::PropertyKey,
};
use closure_rhino::{
    js_string::JsString,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
    token::Token,
};
use std::sync::Arc;

pub struct InferJSDocInfo;

impl InferJSDocInfo {
    // port: InferJSDocInfo#InferJSDocInfo
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }

    // port: InferJSDocInfo#inferJSDocForName
    fn infer_jsdoc_for_name(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: Option<NodeId>,
    ) {
        let Some(parent) = parent else {
            return;
        };

        // Only allow JSDoc on variable declarations, named functions, named classes, and assigns.
        let type_doc: Option<Arc<JSDocInfo>>;
        let aliased_type: Option<TypeId>; // if the right-hand side is a qualified name, its type
        let inferred_type: Option<TypeId>;
        if NodeUtil::is_name_declaration(compiler, Some(parent)) {
            // Case: `/** ... */ (var|let|const) x = function() { ... }`.
            // Case: `(var|let|const) /** ... */ x = function() { ... }`.
            let name_info = n.get_jsdoc_info(compiler);
            type_doc = if name_info.is_some() {
                name_info
            } else {
                parent.get_jsdoc_info(compiler)
            };

            inferred_type = n.get_jstype(compiler);
            // Example: for `const y = x.y;`, the type of `x.y`.
            let value = n.get_first_child(compiler);
            aliased_type = match value {
                Some(value) if value.is_qualified_name(compiler) => value.get_jstype(compiler),
                _ => None,
            };
        } else if NodeUtil::is_function_declaration(compiler, parent)
            || NodeUtil::is_class_declaration(compiler, parent)
        {
            // Case: `/** ... */ function f() { ... }`.
            // Case: `/** ... */ class Foo() { ... }`.
            type_doc = parent.get_jsdoc_info(compiler);
            inferred_type = parent.get_jstype(compiler);
            aliased_type = None; // the value is definitely a class or function, not a qualified name.
        } else if parent.is_assign(compiler) && n.is_first_child_of(compiler, Some(parent)) {
            type_doc = parent.get_jsdoc_info(compiler);
            inferred_type = n.get_jstype(compiler);
            // Example: y = x.y;
            let value = n.get_next(compiler).unwrap();
            aliased_type = if value.is_qualified_name(compiler) {
                value.get_jstype(compiler)
            } else {
                None
            };
        } else {
            return;
        }

        let Some(type_doc) = type_doc else {
            return;
        };

        let name = n.get_string(compiler);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let obj_type = Self::dereferenced(reg, ast, inferred_type);
        if Self::should_attach_jsdoc_to_nominal_type_or_shape(reg, ast, obj_type, aliased_type) {
            Self::attach_jsdoc_info_to_nominal_type_or_shape(
                reg,
                obj_type.unwrap(),
                type_doc,
                Some(&name),
            );
        }
    }

    // port: InferJSDocInfo#inferJSDocForObjectKeyOrClassField
    fn infer_jsdoc_for_object_key_or_class_field(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: NodeId,
    ) {
        let Some(type_doc) = n.get_jsdoc_info(compiler) else {
            return;
        };

        let owning_type: Option<TypeId> = if parent.is_class_members(compiler) {
            let ctor_jstype = parent.get_parent(compiler).unwrap().get_jstype(compiler);
            let is_static_member = n.is_static_member(compiler);
            let is_member_field_def = n.is_member_field_def(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            let Some(ctor_type) = js_type::to_maybe_function_type(reg, ctor_jstype) else {
                return;
            };

            if is_static_member {
                Some(ctor_type)
            } else if is_member_field_def {
                ctor_type.get_instance_type(reg)
            } else {
                Some(ctor_type.get_prototype(reg, ast))
            }
        } else {
            let parent_type = parent.get_jstype(compiler);
            let (reg, ast) = compiler.get_type_registry_and_ast();
            Self::dereferenced(reg, ast, parent_type)
        };
        let Some(owning_type) = owning_type else {
            return;
        };

        let prop_name: PropertyKey =
            if n.is_computed_field_def(compiler) || n.is_computed_prop(compiler) {
                let key_type = n.get_first_child(compiler).unwrap().get_jstype(compiler);
                let reg = compiler.get_type_registry();
                let Some(key_type) = key_type else {
                    return;
                };
                if !key_type.is_known_symbol_value_type(reg) {
                    return;
                }
                PropertyKey::Symbol(key_type.to_maybe_known_symbol_type(reg).unwrap())
            } else {
                PropertyKey::String(n.get_string(compiler))
            };
        let (reg, ast) = compiler.get_type_registry_and_ast();
        if owning_type.has_own_property(reg, ast, prop_name.clone())
            && owning_type
                .get_property_jsdoc_info(reg, ast, prop_name.clone())
                .is_none()
        {
            owning_type.set_property_jsdoc_info(reg, ast, prop_name, Some(type_doc));
        }

        // NOTE(lharker): it seems surprising that this doesn't also call
        // attachJSDocInfoToNominalTypeOrShape. I don't know for sure this is a bug, but leaving a
        // comment to document that this is probably not intentional.
    }

    // port: InferJSDocInfo#inferJSDocForProperty
    fn infer_jsdoc_for_property(
        &mut self,
        compiler: &mut AbstractCompiler,
        n: NodeId,
        parent: NodeId,
    ) {
        // Infer JSDocInfo on properties.
        // There are two ways to write doc comments on a property.
        let type_doc: Option<Arc<JSDocInfo>>;
        let aliased_type: Option<TypeId>; // if the rhs is a qualified name, its type
        if parent.is_assign(compiler) && n.is_first_child_of(compiler, Some(parent)) {
            // Case: `/** @deprecated */ obj.prop = ...;`
            type_doc = parent.get_jsdoc_info(compiler);
            let rhs = n.get_next(compiler).unwrap();
            // Example: for `/** @deprecated */ obj.prop = obj.newProp;`, the type of `obj.newProp`.
            aliased_type = if rhs.is_qualified_name(compiler) {
                rhs.get_jstype(compiler)
            } else {
                None
            };
        } else if parent.is_expr_result(compiler) {
            // Case: `/** @deprecated */ obj.prop;`
            type_doc = n.get_jsdoc_info(compiler);
            aliased_type = None; // there's no value being assigned here
        } else {
            return;
        }

        let Some(type_doc) = type_doc else {
            return;
        };
        if !type_doc.contains_declaration() {
            return;
        }

        let lhs_jstype = n.get_first_child(compiler).unwrap().get_jstype(compiler);
        let prop_name = n.get_string(compiler);
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let Some(lhs_type) = Self::dereferenced(reg, ast, lhs_jstype) else {
            return;
        };

        // Put the JSDoc in the property slot, if there is one.
        if lhs_type.has_own_property(reg, ast, prop_name.clone())
            && lhs_type
                .get_property_jsdoc_info(reg, ast, prop_name.clone())
                .is_none()
        {
            lhs_type.set_property_jsdoc_info(
                reg,
                ast,
                prop_name.clone(),
                Some(Arc::clone(&type_doc)),
            );
        }

        let prop_jstype = lhs_type.get_property_type(reg, ast, prop_name);
        let prop_type = Self::dereferenced(reg, ast, Some(prop_jstype));
        if Self::should_attach_jsdoc_to_nominal_type_or_shape(reg, ast, prop_type, aliased_type) {
            // Rust-only: Java computes the qualified name up front; it is only used here.
            let q_name = n.get_qualified_name(ast);
            Self::attach_jsdoc_info_to_nominal_type_or_shape(
                reg,
                prop_type.unwrap(),
                type_doc,
                q_name.as_ref(),
            );
        }
    }

    // port: InferJSDocInfo#dereferenced
    /// Nullsafe wrapper for `JSType#dereference()`.
    fn dereferenced(reg: &mut JSTypeRegistry, ast: &Ast, type_: Option<TypeId>) -> Option<TypeId> {
        type_.and_then(|type_| type_.dereference(reg, ast))
    }

    // port: InferJSDocInfo#shouldAttachJSDocToNominalTypeOrShape
    /// `type_` is the type to which we're considering attaching JSDoc, or null; `aliased_type`, if
    /// not null, is the JSType of the qualified name on the right-hand side of this assignment.
    fn should_attach_jsdoc_to_nominal_type_or_shape(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: Option<TypeId>,
        aliased_type: Option<TypeId>,
    ) -> bool {
        // If we have no type, or the type already has a JSDocInfo, then no need to attach
        // `typeDoc`. Also check if we're just aliasing something else's type rather than defining
        // a new type. For example, consider this declaration:
        //   /** @const */
        //   var cbAlias = cb;
        // This is a declaration of the name `cbAlias` but doesn't declare a new type. So `type`
        // should equal `aliasedType`, and we shouldn't attach the `/** @const */`
        // JSDoc to the type of `cb`.
        match type_ {
            Some(type_) => {
                JSType::get_jsdoc_info(type_, reg).is_none()
                    && !type_.equals(reg, ast, aliased_type)
            }
            None => false,
        }
    }

    // port: InferJSDocInfo#attachJSDocInfoToNominalTypeOrShape
    /// Handle cases #1 and #3 in the class doc.
    fn attach_jsdoc_info_to_nominal_type_or_shape(
        reg: &mut JSTypeRegistry,
        obj_type: TypeId,
        doc_info: Arc<JSDocInfo>,
        q_name: Option<&JsString>,
    ) {
        if obj_type.is_constructor(reg) || obj_type.is_interface(reg) {
            if !Self::is_reference_name_of(reg, obj_type, q_name) {
                return;
            }

            obj_type.set_jsdoc_info(reg, Some(Arc::clone(&doc_info)));
            js_type::to_maybe_function_type(reg, Some(obj_type))
                .unwrap()
                .get_instance_type(reg)
                .unwrap()
                .set_jsdoc_info(reg, Some(doc_info));
        } else if obj_type.is_enum_type(reg) {
            // Given: `/** @enum {number} */ MyEnum = { FOO: 0 };`
            // Then: typeOf(MyEnum).referenceName() == "enum{MyEnum}"
            // Then: typeOf(MyEnum.FOO).referenceName() == "MyEnum"
            let element_type = obj_type
                .to_maybe_enum_type(reg)
                .unwrap()
                .get_elements_type(reg);
            if !Self::is_reference_name_of(reg, element_type, q_name) {
                return;
            }

            obj_type.set_jsdoc_info(reg, Some(Arc::clone(&doc_info)));
            element_type.set_jsdoc_info(reg, Some(doc_info));
        } else if !obj_type.is_native_object_type(reg) && obj_type.is_function_type(reg) {
            // Anonymous function types identified by their parameter and return types. Remember
            // there can be many unique but equal instances.
            obj_type.set_jsdoc_info(reg, Some(doc_info));
        }
    }

    // port: InferJSDocInfo#isReferenceNameOf
    fn is_reference_name_of(reg: &JSTypeRegistry, type_: TypeId, name: Option<&JsString>) -> bool {
        type_.has_reference_name(reg) && type_.get_reference_name(reg).as_ref() == name
    }
}

impl InferJSDocInfo {
    /// Java's `process(Node externs, Node root)` with its null checks (TypeCheck#check passes
    /// null for one of the two roots).
    // port: InferJSDocInfo#process
    pub fn process_nullable(
        &mut self,
        compiler: &mut AbstractCompiler,
        externs: Option<NodeId>,
        root: Option<NodeId>,
    ) {
        if let Some(externs) = externs {
            NodeTraversal::traverse(compiler, externs, self);
        }
        if let Some(root) = root {
            NodeTraversal::traverse(compiler, root, self);
        }
    }
}

impl CompilerPass for InferJSDocInfo {
    // port: InferJSDocInfo#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        self.process_nullable(compiler, Some(externs), Some(root));
    }
}

impl Callback for InferJSDocInfo {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: InferJSDocInfo#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::NAME => {
                // Infer JSDocInfo on types of all type declarations on variables.
                self.infer_jsdoc_for_name(t.get_compiler(), n, parent);
            }
            Token::STRING_KEY
            | Token::GETTER_DEF
            | Token::SETTER_DEF
            | Token::MEMBER_FUNCTION_DEF
            | Token::MEMBER_FIELD_DEF
            | Token::COMPUTED_PROP
            | Token::COMPUTED_FIELD_DEF => {
                self.infer_jsdoc_for_object_key_or_class_field(
                    t.get_compiler(),
                    n,
                    parent.unwrap(),
                );
            }
            Token::GETPROP => {
                self.infer_jsdoc_for_property(t.get_compiler(), n, parent.unwrap());
            }
            _ => {}
        }
    }
}
