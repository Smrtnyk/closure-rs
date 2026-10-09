/*
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/DestructuredTarget.java.

//! Represents a single target inside a destructuring pattern, whether another pattern or a
//! lhs expression.
//!
//! Call `inferType` to do type inference on the target lazily. This class is designed so that a
//! caller can get information about a target, use that information to do additional type
//! inference, and finally call `inferType` if desired.
//!
//! The registry is passed to the methods that use it (DESIGN §8), so Java's `registry` field and
//! the `Supplier<JSType>` (called with the registry) are not stored with it.
use closure_jstype::{JSTypeRegistry, TypeId, js_type_native::JSTypeNative, prelude::JSType};
use closure_rhino::{
    check_argument, check_not_null,
    node::{Ast, NodeId},
    token::Token,
};
use std::rc::Rc;

/// Rust-only: Java's `Supplier<JSType>` for the pattern type; it gets the registry because the
/// suppliers callers pass infer the types of enclosing targets.
pub type PatternTypeSupplier = Rc<dyn Fn(&mut JSTypeRegistry, &Ast) -> Option<TypeId>>;

#[derive(Clone)]
pub struct DestructuredTarget {
    /// Holds the STRING_KEY or COMPUTED_PROPERTY for a target in an object pattern. Null for
    /// targets in array patterns.
    object_pattern_key: Option<NodeId>,
    /// The target being assigned to. Can be null for empty array pattern elements.
    node: Option<NodeId>,
    /// A supplier to get the type of the pattern containing this target. e.g. for `a` in `const
    /// {a} = {a: 3}`, the supplier provides the record type `{a: number}`
    ///
    /// This is only called by `inferType`.
    pattern_type_supplier: PatternTypeSupplier,
    /// The default value of this target, or null if none
    default_value: Option<NodeId>,
    /// Whether this is a rest key
    is_rest: bool,
    /// The destructuring pattern containing this target
    pattern: NodeId,
}

impl DestructuredTarget {
    // port: DestructuredTarget#DestructuredTarget
    fn new(
        pattern: NodeId,
        node: Option<NodeId>,
        default_value: Option<NodeId>,
        object_pattern_key: Option<NodeId>,
        pattern_type_supplier: PatternTypeSupplier,
        is_rest: bool,
    ) -> Self {
        Self {
            pattern,
            node,
            object_pattern_key,
            pattern_type_supplier,
            is_rest,
            default_value,
        }
    }

    // port: DestructuredTarget#getComputedProperty
    pub fn get_computed_property(&self, ast: &Ast) -> Option<NodeId> {
        if self.has_computed_property(ast) {
            self.object_pattern_key
        } else {
            None
        }
    }

    // port: DestructuredTarget#hasComputedProperty
    pub fn has_computed_property(&self, ast: &Ast) -> bool {
        self.object_pattern_key
            .is_some_and(|key| key.is_computed_prop(ast))
    }

    // port: DestructuredTarget#hasStringKey
    pub fn has_string_key(&self, ast: &Ast) -> bool {
        self.object_pattern_key
            .is_some_and(|key| key.is_string_key(ast))
    }

    // port: DestructuredTarget#getStringKey
    pub fn get_string_key(&self, ast: &Ast) -> Option<NodeId> {
        if self.has_string_key(ast) {
            self.object_pattern_key
        } else {
            None
        }
    }

    // port: DestructuredTarget#getDefaultValue
    pub fn get_default_value(&self) -> Option<NodeId> {
        self.default_value
    }

    // port: DestructuredTarget#hasDefaultValue
    pub fn has_default_value(&self) -> bool {
        self.default_value.is_some()
    }

    // port: DestructuredTarget#getNode
    /// `Builder#build` checks that the node is set, so it is never null here.
    pub fn get_node(&self) -> NodeId {
        self.node.unwrap()
    }

    // port: DestructuredTarget#createTarget(JSTypeRegistry,JSType,Node)
    /// Converts a given child of a destructuring pattern (in the AST) to an instance of this
    /// class.
    ///
    /// NOTE: does not accept EMPTY nodes
    pub fn create_target(
        ast: &Ast,
        destructuring_pattern_type: Option<TypeId>,
        destructuring_child: NodeId,
    ) -> DestructuredTarget {
        Self::create_target_with_supplier(
            ast,
            Rc::new(move |_, _| destructuring_pattern_type),
            destructuring_child,
        )
    }

    // port: DestructuredTarget#createTarget(JSTypeRegistry,Supplier,Node)
    /// Converts a given child of a destructuring pattern (in the AST) to an instance of this
    /// class.
    ///
    /// NOTE: does not accept EMPTY nodes
    pub fn create_target_with_supplier(
        ast: &Ast,
        destructuring_pattern_type: PatternTypeSupplier,
        destructuring_child: NodeId,
    ) -> DestructuredTarget {
        check_argument!(
            destructuring_child
                .get_parent(ast)
                .unwrap()
                .is_destructuring_pattern(ast),
            "%s",
            destructuring_child.to_string(ast)
        );

        let mut builder = Builder::new(
            destructuring_child.get_parent(ast).unwrap(),
            destructuring_pattern_type,
        );
        let value;
        match destructuring_child.get_token(ast) {
            Token::STRING_KEY => {
                // const {objectLiteralKey: x} = ...
                builder.set_object_pattern_key(destructuring_child);
                value = destructuring_child.get_first_child(ast).unwrap();
                if value.is_default_value(ast) {
                    builder.set_node(value.get_first_child(ast).unwrap());
                    builder.set_default_value(value.get_second_child(ast).unwrap());
                } else {
                    builder.set_node(value);
                }
            }
            Token::COMPUTED_PROP => {
                // const {['objectLiteralKey']: x} = ...
                builder.set_object_pattern_key(destructuring_child);
                value = destructuring_child.get_second_child(ast).unwrap();
                if value.is_default_value(ast) {
                    builder.set_node(value.get_first_child(ast).unwrap());
                    builder.set_default_value(value.get_second_child(ast).unwrap());
                } else {
                    builder.set_node(value);
                }
            }
            // const [{x}] = ...
            // const [[x]] = ...
            // const [x] = ...
            // [obj[3]] = ...
            // [this.x] = ...
            Token::OBJECT_PATTERN
            | Token::ARRAY_PATTERN
            | Token::NAME
            | Token::GETELEM
            | Token::GETPROP => {
                builder.set_node(destructuring_child);
            }
            Token::DEFAULT_VALUE => {
                // const [x = 3] = ...
                builder.set_node(destructuring_child.get_first_child(ast).unwrap());
                builder.set_default_value(destructuring_child.get_second_child(ast).unwrap());
            }
            Token::ITER_REST | Token::OBJECT_REST => {
                // const [...x] = ...
                // const {...x} = ...
                builder.set_node(destructuring_child.get_first_child(ast).unwrap());
                builder.set_is_rest(true);
            }
            _ => panic!(
                "IllegalArgumentException: Unexpected child of destructuring pattern {}",
                destructuring_child.to_string(ast)
            ),
        }
        builder.build()
    }

    // port: DestructuredTarget#createAllNonEmptyTargetsInPattern(JSTypeRegistry,JSType,Node)
    /// Returns all the targets directly in the given pattern, except for EMPTY nodes
    ///
    /// EMPTY nodes occur in array patterns with elisions, e.g. `[, , a] = []`
    pub fn create_all_non_empty_targets_in_pattern(
        ast: &Ast,
        pattern_type: Option<TypeId>,
        pattern: NodeId,
    ) -> Vec<DestructuredTarget> {
        Self::create_all_non_empty_targets_in_pattern_with_supplier(
            ast,
            Rc::new(move |_, _| pattern_type),
            pattern,
        )
    }

    // port: DestructuredTarget#createAllNonEmptyTargetsInPattern(JSTypeRegistry,Supplier,Node)
    /// Returns all the targets directly in the given pattern, except for EMPTY nodes
    ///
    /// EMPTY nodes occur in array patterns with elisions, e.g. `[, , a] = []`
    pub fn create_all_non_empty_targets_in_pattern_with_supplier(
        ast: &Ast,
        pattern_type: PatternTypeSupplier,
        pattern: NodeId,
    ) -> Vec<DestructuredTarget> {
        check_argument!(
            pattern.is_destructuring_pattern(ast),
            "%s",
            pattern.to_string(ast)
        );
        let mut builder = Vec::new();
        let mut child = pattern.get_first_child(ast);
        while let Some(c) = child {
            child = c.get_next(ast);
            if c.is_empty(ast) {
                continue;
            }

            builder.push(Self::create_target_with_supplier(
                ast,
                Rc::clone(&pattern_type),
                c,
            ));
        }
        builder
    }

    // port: DestructuredTarget#inferTypeWithoutUsingDefaultValue
    /// Returns the type that this target is inferred to have
    ///
    /// The type includes `undefined` when the target has a default value, e.g. `{a = 3} = {}`.
    pub fn infer_type_without_using_default_value(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
    ) -> TypeId {
        if self.pattern.is_object_pattern(ast) {
            self.infer_object_pattern_key_type(reg, ast)
        } else {
            self.infer_array_pattern_target_type(reg, ast)
        }
    }

    // port: DestructuredTarget#inferType
    /// Returns the type of this target, including the default value's type if present.
    pub fn infer_type(&self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        let inferred_type = self.infer_type_without_using_default_value(reg, ast);
        if !inferred_type.is_unknown_type(reg, ast) && self.has_default_value() {
            let default_value_type = self
                .get_default_value()
                .unwrap()
                .get_jstype(ast)
                .unwrap_or_else(|| reg.get_native_type(JSTypeNative::UNKNOWN_TYPE));
            // We effectively replace '|undefined" with '|typeOfDefaultValue'
            let not_undefined = inferred_type.restrict_by_not_undefined(reg, ast);
            reg.create_union_type(ast, &[not_undefined, default_value_type])
        } else {
            inferred_type
        }
    }

    // port: DestructuredTarget#inferObjectPatternKeyType
    fn infer_object_pattern_key_type(&self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        let pattern_type = (self.pattern_type_supplier)(reg, ast);
        if self.is_rest {
            // TODO(b/128355893): Do smarter inferrence. There are a lot of potential issues with
            // inference on object-rest, so for now we just give up and say `Object`.
            return reg.get_native_type(JSTypeNative::OBJECT_TYPE);
        }

        let Some(pattern_type) = pattern_type.filter(|t| !t.is_unknown_type(reg, ast)) else {
            return reg.get_native_type(JSTypeNative::UNKNOWN_TYPE);
        };
        let object_pattern_key = self.object_pattern_key.expect("NullPointerException");
        match object_pattern_key.get_token(ast) {
            Token::STRING_KEY => {
                let property_type =
                    pattern_type.find_property_type(reg, ast, object_pattern_key.get_string(ast));
                property_type.unwrap_or_else(|| reg.get_native_type(JSTypeNative::UNKNOWN_TYPE))
            }
            // Java: `patternType != null ? .. : UNKNOWN_TYPE`; patternType is non-null here.
            Token::COMPUTED_PROP => {
                let key = reg.get_object_element_key();
                pattern_type
                    .get_template_type_map(reg)
                    .get_resolved_template_type(reg, ast, key)
            }
            _ => panic!(
                "IllegalStateException: Unexpected key {}",
                object_pattern_key.to_string(ast)
            ),
        }
    }

    // port: DestructuredTarget#inferArrayPatternTargetType
    fn infer_array_pattern_target_type(&self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        let pattern_type = (self.pattern_type_supplier)(reg, ast).expect("NullPointerException");

        // e.g. get `number` from `!Iterable<number>`
        let key = reg.get_iterable_value_template();
        let template_type_of_iterable = pattern_type
            .get_template_type_map(reg)
            .get_resolved_template_type(reg, ast, key);

        if self.is_rest {
            // return `!Array<number>`
            let array_type = reg.get_native_object_type(JSTypeNative::ARRAY_TYPE);
            reg.create_templatized_type(ast, array_type, &[template_type_of_iterable])
        } else {
            template_type_of_iterable
        }
    }
}

struct Builder {
    pattern_type_supplier: PatternTypeSupplier,
    pattern: NodeId,
    node: Option<NodeId>,
    default_value: Option<NodeId>,
    object_pattern_key: Option<NodeId>,
    is_rest: bool,
}

impl Builder {
    // port: DestructuredTarget.Builder#Builder
    fn new(pattern: NodeId, pattern_type_supplier: PatternTypeSupplier) -> Self {
        Self {
            pattern_type_supplier,
            pattern,
            node: None,
            default_value: None,
            object_pattern_key: None,
            is_rest: false,
        }
    }

    // port: DestructuredTarget.Builder#setNode
    fn set_node(&mut self, node: NodeId) -> &mut Self {
        self.node = Some(node);
        self
    }

    // port: DestructuredTarget.Builder#setDefaultValue
    fn set_default_value(&mut self, default_value: NodeId) -> &mut Self {
        self.default_value = Some(default_value);
        self
    }

    // port: DestructuredTarget.Builder#setObjectPatternKey
    fn set_object_pattern_key(&mut self, object_pattern_key: NodeId) -> &mut Self {
        self.object_pattern_key = Some(object_pattern_key);
        self
    }

    // port: DestructuredTarget.Builder#setIsRest
    fn set_is_rest(&mut self, is_rest: bool) -> &mut Self {
        self.is_rest = is_rest;
        self
    }

    // port: DestructuredTarget.Builder#build
    fn build(self) -> DestructuredTarget {
        check_not_null!(self.node, "Must set a node");

        DestructuredTarget::new(
            self.pattern,
            self.node,
            self.default_value,
            self.object_pattern_key,
            self.pattern_type_supplier,
            self.is_rest,
        )
    }
}
