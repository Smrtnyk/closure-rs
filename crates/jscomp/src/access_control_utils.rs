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
//   src/com/google/javascript/jscomp/AccessControlUtils.java.

//! Port of AccessControlUtils.java: helper functions for computing the visibility of names and
//! properties in JavaScript source code.
use crate::abstract_compiler::AbstractCompiler;
use crate::collect_file_overview_visibility::FileVisibilityMap;
use crate::var::Var;
use closure_jstype::prelude::*;
use closure_rhino::js_string::JsString;
use closure_rhino::jsdoc_info::Visibility;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::static_source_file::StaticSourceFile;
use std::sync::Arc;

// port: AccessControlUtils
pub struct AccessControlUtils;

impl AccessControlUtils {
    /// Returns the effective visibility of the given name. This can differ from the name's
    /// declared visibility if the file's `@fileoverview` JsDoc specifies a default visibility.
    // port: AccessControlUtils#getEffectiveNameVisibility
    pub fn get_effective_name_visibility(
        compiler: &mut AbstractCompiler,
        name: NodeId,
        var: Var,
        file_visibility_map: &FileVisibilityMap,
    ) -> Visibility {
        let js_doc_info = var.get_jsdoc_info(compiler);
        // Java also tests getVisibility() == null, which never holds.
        let raw = match &js_doc_info {
            None => Visibility::INHERITED,
            Some(info) => info.get_visibility(),
        };
        if raw != Visibility::INHERITED {
            return raw;
        }
        let default_visibility_for_file =
            file_visibility_map.get(var.get_source_file(compiler).as_ref());
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let type_ = name.get_jstype(ast);
        let created_from_goog_provide = type_.is_some_and(|t| t.is_literal_object(reg));
        // Ignore @fileoverview visibility when computing the effective visibility
        // for names created by goog.provide.
        //
        // ProcessClosurePrimitives rewrites goog.provide()s as object literal
        // declarations, but the exact form depends on the ordering of the
        // input files. If goog.provide('a.b') occurs in the inputs before
        // goog.provide('a'), it is rewritten like
        //
        // var a={};a.b={};
        //
        // If the file containing goog.provide('a.b') also declares a @fileoverview
        // visibility, it must not apply to a, as this would make every a.* namespace
        // effectively package-private.
        match default_visibility_for_file {
            Some(default_visibility_for_file) if !created_from_goog_provide => {
                default_visibility_for_file
            }
            _ => raw,
        }
    }

    /// Returns the effective visibility of the given property. This can differ from the
    /// property's declared visibility if the property is inherited from a superclass, or if the
    /// file's `@fileoverview` JsDoc specifies a default visibility.
    // port: AccessControlUtils#getEffectivePropertyVisibility
    pub fn get_effective_property_visibility(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        property: NodeId,
        reference_type: Option<TypeId>,
        file_visibility_map: &FileVisibilityMap,
    ) -> Visibility {
        let property_name = property.get_string(ast);
        let defining_source =
            Self::get_defining_source(reg, ast, property, reference_type, &property_name);
        let file_overview_visibility = file_visibility_map.get(defining_source.as_ref());
        let parent = property.get_parent(ast).unwrap();
        let is_override = parent.get_jsdoc_info_ref(ast).is_some()
            && parent.is_assign(ast)
            && parent.get_first_child(ast) == Some(property);
        let object_type =
            Self::get_object_type(reg, ast, reference_type, is_override, &property_name);
        if is_override {
            let overridden =
                Self::get_overridden_property_visibility(reg, ast, object_type, &property_name);
            Self::get_effective_visibility_for_overridden_property(
                overridden,
                file_overview_visibility,
                &property_name,
            )
        } else {
            Self::get_effective_visibility_for_non_overridden_property(
                reg,
                ast,
                property,
                object_type,
                file_overview_visibility,
            )
        }
    }

    /// Returns the source file in which the given property is defined, or null if it is not
    /// known.
    // port: AccessControlUtils#getDefiningSource
    pub fn get_defining_source(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        node: NodeId,
        reference_type: Option<TypeId>,
        property_name: &JsString,
    ) -> Option<Arc<dyn StaticSourceFile>> {
        if let Some(reference_type) = reference_type {
            let prop_def_node = reference_type.get_property_def_site(reg, ast, property_name);
            if let Some(prop_def_node) = prop_def_node {
                return prop_def_node.get_static_source_file(ast);
            }
        }
        node.get_static_source_file(ast)
    }

    /// Returns the lowest property defined on a class with visibility information.
    // port: AccessControlUtils#getObjectType
    pub fn get_object_type(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        reference_type: Option<TypeId>,
        is_override: bool,
        property_name: &JsString,
    ) -> Option<TypeId> {
        let reference_type = reference_type?;

        // Find the lowest property defined on a class with visibility information.
        let mut current = if is_override {
            reference_type.get_implicit_prototype(reg, ast)
        } else {
            Some(reference_type)
        };
        while let Some(cur) = current {
            let doc_info = cur.get_own_property_jsdoc_info(reg, ast, property_name);
            if doc_info.is_some_and(|d| d.get_visibility() != Visibility::INHERITED) {
                return Some(cur);
            }
            current = cur.get_implicit_prototype(reg, ast);
        }
        None
    }

    /// Returns the original visibility of an overridden property.
    // port: AccessControlUtils#getOverriddenPropertyVisibility
    pub fn get_overridden_property_visibility(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        object_type: Option<TypeId>,
        property_name: &JsString,
    ) -> Visibility {
        match object_type {
            Some(object_type) => object_type
                .get_own_property_jsdoc_info(reg, ast, property_name)
                .expect("NullPointerException")
                .get_visibility(),
            None => Visibility::INHERITED,
        }
    }

    /// Returns the effective visibility of the given overridden property. An overridden property
    /// inherits the visibility of the property it overrides.
    // port: AccessControlUtils#getEffectiveVisibilityForOverriddenProperty
    pub fn get_effective_visibility_for_overridden_property(
        visibility: Visibility,
        file_overview_visibility: Option<Visibility>,
        _property_name: &JsString,
    ) -> Visibility {
        match file_overview_visibility {
            Some(file_overview_visibility) if visibility == Visibility::INHERITED => {
                file_overview_visibility
            }
            _ => visibility,
        }
    }

    /// Returns the effective visibility of the given non-overridden property. Non-overridden
    /// properties without an explicit visibility annotation receive the default visibility
    /// declared in the file's `@fileoverview` block, if one exists.
    // port: AccessControlUtils#getEffectiveVisibilityForNonOverriddenProperty
    fn get_effective_visibility_for_non_overridden_property(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        getprop: NodeId,
        object_type: Option<TypeId>,
        file_overview_visibility: Option<Visibility>,
    ) -> Visibility {
        let property_name = getprop.get_string(ast);
        let mut raw = Visibility::INHERITED;
        if let Some(object_type) = object_type {
            raw = object_type
                .get_own_property_jsdoc_info(reg, ast, &property_name)
                .expect("NullPointerException")
                .get_visibility();
        }
        let type_ = getprop.get_jstype(ast);
        let created_from_goog_provide = type_.is_some_and(|t| t.is_literal_object(reg));
        // Ignore @fileoverview visibility when computing the effective visibility
        // for properties created by goog.provide.
        //
        // ProcessClosurePrimitives rewrites goog.provide()s as object literal
        // declarations, but the exact form depends on the ordering of the
        // input files. If goog.provide('a.b.c') occurs in the inputs before
        // goog.provide('a'), it is rewritten like
        //
        // var a={};a.b={}a.b.c={};
        //
        // If the file containing goog.provide('a.b.c') also declares
        // a @fileoverview visibility, it must not apply to b, as this would make
        // every a.b.* namespace effectively package-private.
        match file_overview_visibility {
            Some(file_overview_visibility)
                if raw == Visibility::INHERITED && !created_from_goog_provide =>
            {
                file_overview_visibility
            }
            _ => raw,
        }
    }
}
