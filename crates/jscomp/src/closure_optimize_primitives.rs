/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ClosureOptimizePrimitives.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `ClosureOptimizePrimitives.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use closure_parsing::parser::feature_set::Feature;
use closure_rhino::dtoa::d_to_a;
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::node::{NodeId, Prop};
use closure_rhino::qualified_name::QualifiedName;
use closure_rhino::token::Token;
use indexmap::IndexSet;
use std::sync::LazyLock;

// port: ClosureOptimizePrimitives#DUPLICATE_SET_MEMBER
pub static DUPLICATE_SET_MEMBER: DiagnosticType = DiagnosticType::warning(
    "JSC_DUPLICATE_SET_MEMBER",
    "Found duplicate value ''{0}'' in set",
);

static GOOG_OBJECT_CREATE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.object.create"));
static GOOG_OBJECT_CREATESET: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.object.createSet"));

// port: DToA#numberToString (static import)
fn number_to_string(value: f64) -> JsString {
    JsString::from_units(d_to_a::number_to_string(value).unwrap_or_else(|cause| panic!("{cause}")))
}

/// Compiler pass that converts primitive calls:
///
/// Converts goog.object.create(key1, val1, key2, val2, ...) where all of the keys are literals
/// into object literals.
///
/// Converts goog.object.createSet(key1, key2, ...) into an object literal with the given keys,
/// where all the values are `true`.
///
/// Converts goog.reflect.objectProperty(propName, object) to JSCompiler_renameProperty
pub struct ClosureOptimizePrimitives {
    /// Whether we can use Es6 syntax
    can_use_es6_syntax: bool,
}

/// Identifies all calls to closure primitive functions
struct FindPrimitives<'a> {
    outer: &'a ClosureOptimizePrimitives,
}

impl Callback for FindPrimitives<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ClosureOptimizePrimitives.FindPrimitives#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.is_call(t) {
            let fn_ = n.get_first_child(t).unwrap();
            let compiler = t.get_compiler();
            // TODO(user): Once goog.object becomes a goog.module, remove "goog$object$create" and
            // related checks.
            if compiler
                .get_coding_convention()
                .is_property_rename_function(compiler, fn_)
            {
                self.outer.process_rename_property_call(compiler, n);
            } else if fn_.matches_name(compiler, "goog$object$create")
                || fn_.matches_name(compiler, "module$contents$goog$object_create")
                || GOOG_OBJECT_CREATE.matches(compiler, fn_)
            {
                self.outer.process_object_create_call(compiler, n);
            } else if fn_.matches_name(compiler, "goog$object$createSet")
                || fn_.matches_name(compiler, "module$contents$goog$object_createSet")
                || GOOG_OBJECT_CREATESET.matches(compiler, fn_)
            {
                self.outer.process_object_create_set_call(compiler, n);
            }
        }
    }
}

impl ClosureOptimizePrimitives {
    /// `compiler` The AbstractCompiler
    // port: ClosureOptimizePrimitives#ClosureOptimizePrimitives
    pub fn new(_compiler: &AbstractCompiler, can_use_es6_syntax: bool) -> Self {
        Self { can_use_es6_syntax }
    }

    /// Converts all of the given call nodes to object literals that are safe to do so.
    // port: ClosureOptimizePrimitives#processObjectCreateCall
    fn process_object_create_call(&self, compiler: &mut AbstractCompiler, call_node: NodeId) {
        let mut cur_param = call_node.get_second_child(compiler);
        if self.can_optimize_object_create(compiler, cur_param) {
            let obj_node = IR::objectlit(compiler, &[]).srcref(compiler, call_node);
            while let Some(key_node) = cur_param {
                let value_node = key_node.get_next(compiler).unwrap();
                cur_param = value_node.get_next(compiler);

                key_node.detach(compiler);
                value_node.detach(compiler);

                let script = NodeUtil::get_enclosing_script(compiler, call_node).unwrap();
                self.add_key_value_to_obj_lit(compiler, obj_node, key_node, value_node, script);
            }
            call_node.replace_with(compiler, obj_node);
            compiler.report_change_to_enclosing_scope(obj_node);
        }
    }

    /// Converts all of the given call nodes to object literals that are safe to do so.
    // port: ClosureOptimizePrimitives#processRenamePropertyCall
    fn process_rename_property_call(&self, compiler: &mut AbstractCompiler, call_node: NodeId) {
        let name_node = call_node.get_first_child(compiler).unwrap();
        if name_node.matches_qualified_name(compiler, NodeUtil::JSC_PROPERTY_NAME_FN) {
            return;
        }

        let new_target =
            IR::name(compiler, NodeUtil::JSC_PROPERTY_NAME_FN).srcref(compiler, name_node);
        let original_name = name_node.get_original_qualified_name(compiler);
        new_target.set_original_name(compiler, original_name);

        name_node.replace_with(compiler, new_target);
        call_node.put_boolean_prop(compiler, Prop::FREE_CALL, true);
        compiler.report_change_to_enclosing_scope(call_node);
    }

    /// Returns whether the given call to goog.object.create can be converted to an object literal.
    // port: ClosureOptimizePrimitives#canOptimizeObjectCreate
    fn can_optimize_object_create(
        &self,
        compiler: &AbstractCompiler,
        first_param: Option<NodeId>,
    ) -> bool {
        let mut cur_param = first_param;
        while let Some(param) = cur_param {
            if !self.is_optimizable_key(compiler, param) {
                return false;
            }
            cur_param = param.get_next(compiler);

            // Check for an odd number of parameters.
            let Some(param) = cur_param else {
                return false;
            };
            cur_param = param.get_next(compiler);
        }
        true
    }

    /// Converts all of the given call nodes to object literals that are safe to do so.
    // port: ClosureOptimizePrimitives#processObjectCreateSetCall
    fn process_object_create_set_call(&self, compiler: &mut AbstractCompiler, call_node: NodeId) {
        let mut cur_param = call_node.get_second_child(compiler);
        if self.can_optimize_object_create_set(compiler, cur_param) {
            let obj_node = IR::objectlit(compiler, &[]).srcref(compiler, call_node);
            while let Some(key_node) = cur_param {
                let value_node = IR::true_node(compiler).srcref(compiler, key_node);

                cur_param = key_node.get_next(compiler);
                key_node.detach(compiler);

                let script = NodeUtil::get_enclosing_script(compiler, call_node).unwrap();
                self.add_key_value_to_obj_lit(compiler, obj_node, key_node, value_node, script);
            }
            call_node.replace_with(compiler, obj_node);
            compiler.report_change_to_enclosing_scope(obj_node);
        }
    }

    /// Returns whether the given call to goog.object.createSet can be converted to an object
    /// literal.
    // port: ClosureOptimizePrimitives#canOptimizeObjectCreateSet
    fn can_optimize_object_create_set(
        &self,
        compiler: &mut AbstractCompiler,
        first_param: Option<NodeId>,
    ) -> bool {
        if let Some(first) = first_param
            && first.get_next(compiler).is_none()
            && !(first.is_number(compiler) || first.is_string_lit(compiler))
        {
            // if there is only one argument, and it's an array, then the method uses the array
            // elements as keys. Don't optimize it to {[arr]: true}. We only special-case number and
            // string arguments in order to not regress ES5-out behavior
            return false;
        }

        let mut cur_param = first_param;
        let mut keys: IndexSet<JsString> = IndexSet::new();
        while let Some(param) = cur_param {
            // All keys must be strings or numbers, otherwise we can't optimize the call.
            if !self.is_optimizable_key(compiler, param) {
                return false;
            }
            if param.is_string_lit(compiler) || param.is_number(compiler) {
                let key = if param.is_string_lit(compiler) {
                    param.get_string(compiler)
                } else {
                    number_to_string(param.get_double(compiler))
                };
                if !keys.insert(key.clone()) {
                    let previous = first_param.unwrap().get_previous(compiler).unwrap();
                    let key = key.to_string_lossy();
                    compiler.report(JSError::make(
                        compiler,
                        previous,
                        &DUPLICATE_SET_MEMBER,
                        &[&key],
                    ));
                    return false;
                }
            }
            cur_param = param.get_next(compiler);
        }
        true
    }

    // port: ClosureOptimizePrimitives#addKeyValueToObjLit
    fn add_key_value_to_obj_lit(
        &self,
        compiler: &mut AbstractCompiler,
        obj_node: NodeId,
        mut key_node: NodeId,
        value_node: NodeId,
        script_node: NodeId,
    ) {
        if key_node.is_number(compiler) || key_node.is_string_lit(compiler) {
            if key_node.is_number(compiler) {
                let key = number_to_string(key_node.get_double(compiler));
                key_node = IR::string(compiler, key).srcref(compiler, key_node);
            }
            // It isn't valid for a `STRING_KEY` to be marked as parenthesized.
            key_node.set_is_parenthesized(compiler, false);
            key_node.set_token(compiler, Token::STRING_KEY);
            key_node.set_quoted_string_key(compiler);
            let propdef = IR::propdef(compiler, key_node, value_node);
            obj_node.add_child_to_back(compiler, propdef);
        } else {
            let computed_prop =
                IR::computed_prop(compiler, key_node, value_node).srcref(compiler, key_node);
            obj_node.add_child_to_back(compiler, computed_prop);
            NodeUtil::add_feature_to_script(compiler, script_node, Feature::COMPUTED_PROPERTIES);
        }
    }

    // port: ClosureOptimizePrimitives#isOptimizableKey
    fn is_optimizable_key(&self, compiler: &AbstractCompiler, cur_param: NodeId) -> bool {
        if self.can_use_es6_syntax {
            !NodeUtil::is_statement(compiler, cur_param)
        } else {
            // Not ES6, all keys must be strings or numbers.
            cur_param.is_string_lit(compiler) || cur_param.is_number(compiler)
        }
    }
}

impl CompilerPass for ClosureOptimizePrimitives {
    // port: ClosureOptimizePrimitives#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let mut pass = FindPrimitives { outer: self };
        NodeTraversal::traverse(compiler, root, &mut pass);
    }
}
