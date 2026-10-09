/*
 * Copyright 2004 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/lint/CheckEnums.java.

//! Checks the following:
//!
//! 1. Whether there are duplicate values in enums.
//! 2. Whether enum type (and its initializer values) are string/number or not.
//! 3. Whether string enum values are statically initialized or not.

#![allow(clippy::nonminimal_bool)] // Preserve Java boolean expressions as written.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::{
    check_argument, java_lang::double_to_string, js_string::JsString, jsdoc_info::JSDocInfo,
    node::NodeId,
};

// port: CheckEnums#DUPLICATE_ENUM_VALUE
pub static DUPLICATE_ENUM_VALUE: DiagnosticType = DiagnosticType::disabled(
    "JSC_DUPLICATE_ENUM_VALUE",
    "The value {0} is duplicated in this enum.",
);

// port: CheckEnums#COMPUTED_PROP_NAME_IN_ENUM
pub static COMPUTED_PROP_NAME_IN_ENUM: DiagnosticType = DiagnosticType::disabled(
    "JSC_COMPUTED_PROP_NAME_IN_ENUM",
    "Computed property name used in enum.",
);

// port: CheckEnums#SHORTHAND_ASSIGNMENT_IN_ENUM
pub static SHORTHAND_ASSIGNMENT_IN_ENUM: DiagnosticType = DiagnosticType::disabled(
    "JSC_SHORTHAND_ASSIGNMENT_IN_ENUM",
    "Shorthand assignment used in enum.",
);

// port: CheckEnums#ENUM_PROP_NOT_CONSTANT
pub static ENUM_PROP_NOT_CONSTANT: DiagnosticType = DiagnosticType::disabled(
    "JSC_ENUM_PROP_NOT_CONSTANT",
    "enum key {0} must be in ALL_CAPS.",
);

// port: CheckEnums#ENUM_TYPE_NOT_STRING_OR_NUMBER
pub static ENUM_TYPE_NOT_STRING_OR_NUMBER: DiagnosticType = DiagnosticType::disabled(
    "JSC_ENUM_VALUE_NOT_STRING_OR_NUMBER",
    "enum type must be either string or number.",
);

// port: CheckEnums#NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM
pub static NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM: DiagnosticType = DiagnosticType::disabled(
    "JSC_NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM",
    "Enum string values must be statically initialized as per the style guide.",
);

pub struct CheckEnums;

impl CheckEnums {
    // port: CheckEnums#CheckEnums
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }

    // port: CheckEnums#checkEnumTypeAndInitializerValues
    fn check_enum_type_and_initializer_values(
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        js_doc_info: &JSDocInfo,
    ) {
        check_argument!(n.is_object_lit(t), "%s", n.to_string(t));
        let enum_type_expr = js_doc_info.get_enum_parameter_type().unwrap();

        let enum_type = enum_type_expr.get_root();
        let is_string_enum = enum_type.is_string_lit(t) && enum_type.get_string_ref(t) == "string";
        let is_number_enum = enum_type.is_string_lit(t) && enum_type.get_string_ref(t) == "number";
        if !is_string_enum && !is_number_enum {
            // warn on `@enum {?}`, `@enum {boolean}`, `@enum {Some|Another}`, `@enum {SomeName}`
            // etc`
            t.report(n, &ENUM_TYPE_NOT_STRING_OR_NUMBER, &[]);
        }
        if is_string_enum {
            Self::check_string_enum_initializer_values(t, n);
        }
    }

    /// Reports a warning if the string enum value is not statically initialized
    // port: CheckEnums#checkStringEnumInitializerValues
    fn check_string_enum_initializer_values(t: &mut NodeTraversal<'_>, enum_node: NodeId) {
        check_argument!(enum_node.is_object_lit(t), "%s", enum_node.to_string(t));
        let mut prop = enum_node.get_first_child(t);
        while let Some(p) = prop {
            // valueNode is guaranteed to exist by this time, as shorthand `{A}`s are converted to
            // `{A:A}`
            let value_node = p.get_last_child(t).unwrap();
            if !value_node.is_string_lit(t)
                && !(value_node.is_template_lit(t) && value_node.has_one_child(t))
            {
                // neither string nor substitution-free template literal; report finding.
                t.report(
                    value_node,
                    &NON_STATIC_INITIALIZER_STRING_VALUE_IN_ENUM,
                    &[],
                );
            }
            prop = p.get_next(t);
        }
    }

    // port: CheckEnums#checkNamingAndAssignmentUsage
    fn check_naming_and_assignment_usage(t: &mut NodeTraversal<'_>, obj_lit: NodeId) {
        let mut child = obj_lit.get_first_child(t);
        while let Some(c) = child {
            Self::check_name(t, c);
            child = c.get_next(t);
        }
    }

    // port: CheckEnums#checkName
    fn check_name(t: &mut NodeTraversal<'_>, prop: NodeId) {
        if prop.is_computed_prop(t) {
            t.report(prop, &COMPUTED_PROP_NAME_IN_ENUM, &[]);
            return;
        }

        if prop.is_string_key(t) && prop.is_shorthand_property(t) {
            t.report(prop, &SHORTHAND_ASSIGNMENT_IN_ENUM, &[]);
        }

        let key = prop.get_string(t);
        if !t
            .get_compiler()
            .get_coding_convention()
            .is_valid_enum_key(Some(&key))
        {
            t.report(prop, &ENUM_PROP_NOT_CONSTANT, &[]);
        }
    }

    // port: CheckEnums#checkDuplicateEnumValues
    fn check_duplicate_enum_values(t: &mut NodeTraversal<'_>, enum_node: NodeId) {
        let mut values: IndexSet<JsString> = IndexSet::<_>::default();
        let mut prop = enum_node.get_first_child(t);
        while let Some(p) = prop {
            let value_node = p.get_last_child(t);
            let value: JsString;
            let Some(value_node) = value_node else {
                return;
            };
            if value_node.is_string_lit(t) {
                value = value_node.get_string(t);
            } else if value_node.is_number(t) {
                value = JsString::from(double_to_string(value_node.get_double(t)));
            } else {
                return;
            }

            if !values.insert(value.clone()) {
                t.report(value_node, &DUPLICATE_ENUM_VALUE, &[&value.to_string()]);
            }
            prop = p.get_next(t);
        }
    }
}

impl CompilerPass for CheckEnums {
    // port: CheckEnums#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckEnums {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: CheckEnums#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_object_lit(t) {
            let jsdoc = NodeUtil::get_best_jsdoc_info(t, n);
            if let Some(jsdoc) = jsdoc
                && jsdoc.has_enum_parameter_type()
            {
                Self::check_naming_and_assignment_usage(t, n);
                Self::check_duplicate_enum_values(t, n);
                Self::check_enum_type_and_initializer_values(t, n, &jsdoc);
            }
        }
    }
}
