/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Ben Lickly
 *   Dimitris Vardoulakis
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/rhino/NominalTypeBuilder.java.

use crate::{
    TypeId, function_type::FunctionType, js_type_registry::JSTypeRegistry, object_type::ObjectType,
};
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
};

pub struct NominalTypeBuilder {
    constructor: TypeId,
    instance: TypeId,
    prototype: TypeId,
}
impl NominalTypeBuilder {
    // port: NominalTypeBuilder#NominalTypeBuilder
    pub fn new(reg: &mut JSTypeRegistry, ast: &Ast, constructor: TypeId, instance: TypeId) -> Self {
        Self {
            constructor,
            instance,
            prototype: constructor.get_prototype_property(reg, ast),
        }
    }
    // port: NominalTypeBuilder#declarePrototypeProperty
    pub fn declare_prototype_property(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<JsString>,
        type_: TypeId,
        def_site: Option<NodeId>,
    ) {
        self.prototype
            .define_declared_property(reg, ast, name.into(), type_, def_site);
    }
    // port: NominalTypeBuilder#declareInstanceProperty
    pub fn declare_instance_property(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<JsString>,
        type_: TypeId,
        def_site: Option<NodeId>,
    ) {
        self.instance
            .define_declared_property(reg, ast, name.into(), type_, def_site);
    }
    // port: NominalTypeBuilder#declareConstructorProperty
    pub fn declare_constructor_property(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        name: impl Into<JsString>,
        type_: TypeId,
        def_site: Option<NodeId>,
    ) {
        self.constructor
            .define_declared_property(reg, ast, name.into(), type_, def_site);
    }
    // port: NominalTypeBuilder#superClass
    pub fn super_class(&self, reg: &mut JSTypeRegistry, ast: &Ast) -> Option<Self> {
        let ctor = self.instance.get_super_class_constructor(reg, ast)?;
        Some(Self::new(
            reg,
            ast,
            ctor,
            ctor.get_instance_type(reg).unwrap(),
        ))
    }
    // port: NominalTypeBuilder#constructor
    pub fn constructor(&self) -> TypeId {
        self.constructor
    }
    // port: NominalTypeBuilder#instance
    pub fn instance(&self) -> TypeId {
        self.instance
    }
    // port: NominalTypeBuilder#prototypeOrInstance
    pub fn prototype_or_instance(&self) -> TypeId {
        self.prototype
    }
}
