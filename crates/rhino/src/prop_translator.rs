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
 *   Google Inc.
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
//   src/com/google/javascript/rhino/PropTranslator.java.

use crate::{check_state, jscomp_serialization::node_property::NodeProperty, node::Prop};
use std::sync::OnceLock;
pub struct PropTranslator;
struct PropTables {
    proto_to_rhino_prop: [Option<Prop>; NodeProperty::VALUES.len()],
    rhino_to_proto_prop: [Option<NodeProperty>; Prop::VALUES.len()],
}
static TABLES: OnceLock<PropTables> = OnceLock::new();
impl PropTranslator {
    fn tables() -> &'static PropTables {
        TABLES.get_or_init(|| {
            let mut tables = PropTables {
                proto_to_rhino_prop: [None; NodeProperty::VALUES.len()],
                rhino_to_proto_prop: [None; Prop::VALUES.len()],
            };
            Self::set_props(&mut tables);
            Self::check_unexpected_null_proto_props(&tables);
            tables
        })
    }
    // port: PropTranslator#serialize
    pub fn serialize(x: Prop) -> Option<NodeProperty> {
        Self::tables().rhino_to_proto_prop[x as usize]
    }
    // port: PropTranslator#deserialize
    pub fn deserialize(x: NodeProperty) -> Option<Prop> {
        Self::tables().proto_to_rhino_prop[x.ordinal()]
    }
    // The private Java constructor is intentionally uncallable.
    // port: PropTranslator#PropTranslator
    #[allow(dead_code)]
    fn new() -> Self {
        panic!("AssertionError")
    }
    // port: PropTranslator#setProps
    fn set_props(tables: &mut PropTables) {
        for rhino_prop in Prop::VALUES {
            if let Some(proto_prop) = Self::serialize_prop(rhino_prop) {
                check_state!(
                    proto_prop.get_number() < 63,
                    "enum %s value %s",
                    proto_prop,
                    proto_prop.get_number()
                );
                tables.proto_to_rhino_prop[proto_prop.ordinal()] = Some(rhino_prop);
                tables.rhino_to_proto_prop[rhino_prop as usize] = Some(proto_prop);
            }
        }
    }
    // port: PropTranslator#serializeProp
    fn serialize_prop(prop: Prop) -> Option<NodeProperty> {
        match prop {
            Prop::ARROW_FN => Some(NodeProperty::ARROW_FN),
            Prop::ASYNC_FN => Some(NodeProperty::ASYNC_FN),
            Prop::GENERATOR_FN => Some(NodeProperty::GENERATOR_FN),
            Prop::YIELD_ALL => Some(NodeProperty::YIELD_ALL),
            Prop::IS_PARENTHESIZED => Some(NodeProperty::IS_PARENTHESIZED),
            Prop::SYNTHETIC => Some(NodeProperty::SYNTHETIC),
            Prop::ADDED_BLOCK => Some(NodeProperty::ADDED_BLOCK),
            Prop::STATIC_MEMBER => Some(NodeProperty::STATIC_MEMBER),
            Prop::IS_GENERATOR_MARKER => Some(NodeProperty::IS_GENERATOR_MARKER),
            Prop::IS_GENERATOR_SAFE => Some(NodeProperty::IS_GENERATOR_SAFE),
            Prop::COLOR_FROM_CAST => Some(NodeProperty::COLOR_FROM_CAST),
            Prop::NON_INDEXABLE => Some(NodeProperty::NON_INDEXABLE),
            Prop::DELETED => Some(NodeProperty::DELETED),
            Prop::IS_UNUSED_PARAMETER => Some(NodeProperty::IS_UNUSED_PARAMETER),
            Prop::IS_SHORTHAND_PROPERTY => Some(NodeProperty::IS_SHORTHAND_PROPERTY),
            Prop::START_OF_OPT_CHAIN => Some(NodeProperty::START_OF_OPT_CHAIN),
            Prop::TRAILING_COMMA => Some(NodeProperty::TRAILING_COMMA),
            Prop::IS_CONSTANT_NAME => Some(NodeProperty::IS_CONSTANT_NAME),
            Prop::IS_NAMESPACE => Some(NodeProperty::IS_NAMESPACE),
            Prop::DIRECT_EVAL => Some(NodeProperty::DIRECT_EVAL),
            Prop::FREE_CALL => Some(NodeProperty::FREE_CALL),
            Prop::REFLECTED_OBJECT => Some(NodeProperty::REFLECTED_OBJECT),
            Prop::EXPORT_DEFAULT => Some(NodeProperty::EXPORT_DEFAULT),
            Prop::EXPORT_ALL_FROM => Some(NodeProperty::EXPORT_ALL_FROM),
            Prop::COMPUTED_PROP_METHOD => Some(NodeProperty::COMPUTED_PROP_METHOD),
            Prop::COMPUTED_PROP_GETTER => Some(NodeProperty::COMPUTED_PROP_GETTER),
            Prop::COMPUTED_PROP_SETTER => Some(NodeProperty::COMPUTED_PROP_SETTER),
            Prop::COMPUTED_PROP_VARIABLE => Some(NodeProperty::COMPUTED_PROP_VARIABLE),
            Prop::GOOG_MODULE => Some(NodeProperty::GOOG_MODULE),
            Prop::MODULE_ALIAS => Some(NodeProperty::MODULE_ALIAS),
            Prop::MODULE_EXPORT => Some(NodeProperty::MODULE_EXPORT),
            Prop::ES6_MODULE => Some(NodeProperty::ES6_MODULE),
            Prop::CONSTANT_VAR_FLAGS => Some(NodeProperty::CONSTANT_VAR_FLAGS),
            Prop::PRIVATE_IDENTIFIER => Some(NodeProperty::PRIVATE_IDENTIFIER),
            Prop::SYNTHESIZED_UNFULFILLED_NAME_DECLARATION => {
                Some(NodeProperty::SYNTHESIZED_UNFULFILLED_NAME_DECLARATION)
            }
            _ => None,
        }
    }
    // port: PropTranslator#checkUnexpectedNullProtoProps
    fn check_unexpected_null_proto_props(tables: &PropTables) {
        for proto_prop in NodeProperty::VALUES {
            match proto_prop {
                NodeProperty::NODE_PROPERTY_UNSPECIFIED => {}
                NodeProperty::IS_DECLARED_CONSTANT | NodeProperty::IS_INFERRED_CONSTANT => {}
                NodeProperty::UNRECOGNIZED | NodeProperty::UNUSED_11 => {}
                NodeProperty::MUTATES_GLOBAL_STATE
                | NodeProperty::MUTATES_THIS
                | NodeProperty::MUTATES_ARGUMENTS
                | NodeProperty::THROWS => {}
                NodeProperty::CLOSURE_UNAWARE_SHADOW => {}
                _ => check_state!(
                    tables.proto_to_rhino_prop[proto_prop.ordinal()].is_some(),
                    "Hit unhandled node property: %s",
                    proto_prop
                ),
            }
        }
    }
}
