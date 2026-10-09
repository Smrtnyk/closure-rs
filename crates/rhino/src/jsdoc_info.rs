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
 *   Bob Jervis
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
 *   Nick Santos
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/rhino/JSDocInfo.java,
//   src/com/google/javascript/rhino/SourcePosition.java,
//   src/com/google/javascript/rhino/jstype/Property.java.

#[derive(Clone, Copy, Debug)]
pub enum Bit {
    INLINE_TYPE,
    INCLUDE_DOCUMENTATION,
    CONST,
    CONSTRUCTOR,
    DEFINE,
    TYPE_SUMMARY,
    FINAL,
    OVERRIDE,
    DEPRECATED,
    INTERFACE,
    EXPORT,
    ENHANCED_NAMESPACE,
    NOINLINE,
    FILEOVERVIEW,
    IMPLICITCAST,
    NOSIDEEFFECTS,
    EXTERNS,
    NOCOMPILE,
    NODTS,
    UNRESTRICTED,
    USED_VIA_DOT_CONSTRUCTOR,
    STRUCT,
    DICT,
    NOCOLLAPSE,
    RECORD,
    ABSTRACT,
    PURE_OR_BREAK_MY_CODE,
    COLLAPSIBLE_OR_BREAK_MY_CODE,
    NOCOVERAGE,
    REQUIRE_INLINING,
    ENCOURAGE_INLINING,
    NG_INJECT,
    WIZ_ACTION,
    POLYMER_BEHAVIOR,
    POLYMER,
    CUSTOM_ELEMENT,
    MIXIN_CLASS,
    MIXIN_FUNCTION,
    SASS_GENERATED_CSS_TS,
    CLOSURE_UNAWARE_CODE,
    PROVIDE_GOOG,
    PROVIDE_ALREADY_PROVIDED,
    WIZ_CALLBACK,
    LOG_TYPE_IN_COMPILER,
    CLOSURE_UNAWARE_CONFIG,
}
impl Bit {
    // port: Bit#Bit
    pub const fn mask(self) -> u64 {
        1 << (self as u32)
    }
    // port: Bit#Bit
    pub fn name(self) -> &'static str {
        match self {
            Self::INLINE_TYPE => "inlineType",
            Self::INCLUDE_DOCUMENTATION => "includeDocumentation",
            Self::CONST => "const",
            Self::CONSTRUCTOR => "constructor",
            Self::DEFINE => "define",
            Self::TYPE_SUMMARY => "typeSummary",
            Self::FINAL => "final",
            Self::OVERRIDE => "override",
            Self::DEPRECATED => "deprecated",
            Self::INTERFACE => "interface",
            Self::EXPORT => "export",
            Self::ENHANCED_NAMESPACE => "enhancedNamespace",
            Self::NOINLINE => "noinline",
            Self::FILEOVERVIEW => "fileoverview",
            Self::IMPLICITCAST => "implicitcast",
            Self::NOSIDEEFFECTS => "nosideeffects",
            Self::EXTERNS => "externs",
            Self::NOCOMPILE => "nocompile",
            Self::NODTS => "nodts",
            Self::UNRESTRICTED => "unrestricted",
            Self::USED_VIA_DOT_CONSTRUCTOR => "usedViaDotConstructor",
            Self::STRUCT => "struct",
            Self::DICT => "dict",
            Self::NOCOLLAPSE => "nocollapse",
            Self::RECORD => "record",
            Self::ABSTRACT => "abstract",
            Self::PURE_OR_BREAK_MY_CODE => "pureOrBreakMyCode",
            Self::COLLAPSIBLE_OR_BREAK_MY_CODE => "collapsibleOrBreakMyCode",
            Self::NOCOVERAGE => "nocoverage",
            Self::REQUIRE_INLINING => "requireInlining",
            Self::ENCOURAGE_INLINING => "encourageInlining",
            Self::NG_INJECT => "ngInject",
            Self::WIZ_ACTION => "wizAction",
            Self::POLYMER_BEHAVIOR => "polymerBehavior",
            Self::POLYMER => "polymer",
            Self::CUSTOM_ELEMENT => "customElement",
            Self::MIXIN_CLASS => "mixinClass",
            Self::MIXIN_FUNCTION => "mixinFunction",
            Self::SASS_GENERATED_CSS_TS => "sassGeneratedCssTs",
            Self::CLOSURE_UNAWARE_CODE => "closureUnawareCode",
            Self::PROVIDE_GOOG => "provideGoog",
            Self::PROVIDE_ALREADY_PROVIDED => "provideAlreadyProvided",
            Self::WIZ_CALLBACK => "wizCallback",
            Self::LOG_TYPE_IN_COMPILER => "logTypeInCompiler",
            Self::CLOSURE_UNAWARE_CONFIG => "closureUnawareConfig",
        }
    }
}
const BITS: &[Bit] = &[
    Bit::INLINE_TYPE,
    Bit::INCLUDE_DOCUMENTATION,
    Bit::CONST,
    Bit::CONSTRUCTOR,
    Bit::DEFINE,
    Bit::TYPE_SUMMARY,
    Bit::FINAL,
    Bit::OVERRIDE,
    Bit::DEPRECATED,
    Bit::INTERFACE,
    Bit::EXPORT,
    Bit::ENHANCED_NAMESPACE,
    Bit::NOINLINE,
    Bit::FILEOVERVIEW,
    Bit::IMPLICITCAST,
    Bit::NOSIDEEFFECTS,
    Bit::EXTERNS,
    Bit::NOCOMPILE,
    Bit::NODTS,
    Bit::UNRESTRICTED,
    Bit::USED_VIA_DOT_CONSTRUCTOR,
    Bit::STRUCT,
    Bit::DICT,
    Bit::NOCOLLAPSE,
    Bit::RECORD,
    Bit::ABSTRACT,
    Bit::PURE_OR_BREAK_MY_CODE,
    Bit::COLLAPSIBLE_OR_BREAK_MY_CODE,
    Bit::NOCOVERAGE,
    Bit::REQUIRE_INLINING,
    Bit::ENCOURAGE_INLINING,
    Bit::NG_INJECT,
    Bit::WIZ_ACTION,
    Bit::POLYMER_BEHAVIOR,
    Bit::POLYMER,
    Bit::CUSTOM_ELEMENT,
    Bit::MIXIN_CLASS,
    Bit::MIXIN_FUNCTION,
    Bit::SASS_GENERATED_CSS_TS,
    Bit::CLOSURE_UNAWARE_CODE,
    Bit::PROVIDE_GOOG,
    Bit::PROVIDE_ALREADY_PROVIDED,
    Bit::WIZ_CALLBACK,
    Bit::LOG_TYPE_IN_COMPILER,
    Bit::CLOSURE_UNAWARE_CONFIG,
];
pub const VISIBILITY: Property = Property::new("visibility", 0, PropertyKind::Plain);
pub const TYPE: Property = Property::new("type", 1, PropertyKind::Type);
pub const RETURN_TYPE: Property = Property::new("returnType", 2, PropertyKind::Type);
pub const ENUM_PARAMETER_TYPE: Property = Property::new("enumParameterType", 3, PropertyKind::Type);
pub const TYPEDEF_TYPE: Property = Property::new("typedefType", 4, PropertyKind::Type);
pub const THIS_TYPE: Property = Property::new("thisType", 5, PropertyKind::Type);
pub const ORIGINAL_COMMENT_POSITION: Property =
    Property::new("originalCommentPosition", 6, PropertyKind::Plain);
pub const ID_GENERATOR: Property = Property::new("idGenerator", 7, PropertyKind::Plain);
pub const BASE_TYPE: Property = Property::new("baseType", 8, PropertyKind::Type);
pub const EXTENDED_INTERFACES: Property =
    Property::new("extendedInterfaces", 9, PropertyKind::TypeList);
pub const IMPLEMENTED_INTERFACES: Property =
    Property::new("extendedInterfaces", 10, PropertyKind::TypeList);
pub const PARAMETERS: Property = Property::new("parameters", 11, PropertyKind::TypeMap);
pub const THROWS_ANNOTATIONS: Property = Property::new("throws", 12, PropertyKind::Plain);
pub const TEMPLATE_TYPE_NAMES: Property =
    Property::new("templateTypeNames", 13, PropertyKind::TypeMap);
pub const TYPE_TRANSFORMATIONS: Property =
    Property::new("typeTransformations", 14, PropertyKind::NodeMap);
pub const DESCRIPTION: Property = Property::new("description", 15, PropertyKind::Plain);
pub const MEANING: Property = Property::new("meaning", 16, PropertyKind::Plain);
pub const ALTERNATE_MESSAGE_ID: Property =
    Property::new("alternateMessageId", 17, PropertyKind::Plain);
pub const DEPRECATION_REASON: Property =
    Property::new("deprecationReason", 18, PropertyKind::Plain);
pub const LICENSE: Property = Property::new("license", 19, PropertyKind::Plain);
pub const SUPPRESSIONS: Property = Property::new("suppressions", 20, PropertyKind::Plain);
pub const MODIFIES: Property = Property::new("modifies", 21, PropertyKind::Plain);
pub const LENDS_NAME: Property = Property::new("lendsName", 22, PropertyKind::Type);
pub const CLOSURE_PRIMITIVE_ID: Property =
    Property::new("closurePrimitiveId", 23, PropertyKind::Plain);
pub const SOURCE_COMMENT: Property = Property::new("sourceComment", 24, PropertyKind::Plain);
pub const MARKERS: Property = Property::new("markers", 25, PropertyKind::MarkerList);
pub const PARAMETER_DESCRIPTIONS: Property =
    Property::new("parameterDescriptions", 26, PropertyKind::Plain);
pub const BLOCK_DESCRIPTION: Property = Property::new("blockDescription", 27, PropertyKind::Plain);
pub const FILEOVERVIEW_DESCRIPTION: Property =
    Property::new("fileoverviewDescription", 28, PropertyKind::Plain);
pub const RETURN_DESCRIPTION: Property =
    Property::new("returnDescription", 29, PropertyKind::Plain);
pub const ENHANCED_NAMESPACE: Property = Property::new("enhance", 30, PropertyKind::Plain);
pub const MODS: Property = Property::new("mods", 31, PropertyKind::Plain);
pub const TS_TYPES: Property = Property::new("tsType", 32, PropertyKind::Plain);
pub const CLOSURE_UNAWARE_CONFIG_VALUE: Property =
    Property::new("perFileClosureUnawareMode", 33, PropertyKind::Plain);
pub const AUTHORS: Property = Property::new("authors", 34, PropertyKind::Plain);
pub const SEES: Property = Property::new("sees", 35, PropertyKind::Plain);
pub const PROPERTIES: &[Property] = &[
    VISIBILITY,
    TYPE,
    RETURN_TYPE,
    ENUM_PARAMETER_TYPE,
    TYPEDEF_TYPE,
    THIS_TYPE,
    ORIGINAL_COMMENT_POSITION,
    ID_GENERATOR,
    BASE_TYPE,
    EXTENDED_INTERFACES,
    IMPLEMENTED_INTERFACES,
    PARAMETERS,
    THROWS_ANNOTATIONS,
    TEMPLATE_TYPE_NAMES,
    TYPE_TRANSFORMATIONS,
    DESCRIPTION,
    MEANING,
    ALTERNATE_MESSAGE_ID,
    DEPRECATION_REASON,
    LICENSE,
    SUPPRESSIONS,
    MODIFIES,
    LENDS_NAME,
    CLOSURE_PRIMITIVE_ID,
    SOURCE_COMMENT,
    MARKERS,
    PARAMETER_DESCRIPTIONS,
    BLOCK_DESCRIPTION,
    FILEOVERVIEW_DESCRIPTION,
    RETURN_DESCRIPTION,
    ENHANCED_NAMESPACE,
    MODS,
    TS_TYPES,
    CLOSURE_UNAWARE_CONFIG_VALUE,
    AUTHORS,
    SEES,
];
impl PropertyValue {
    fn as_str(&self) -> Option<&JsString> {
        if let Self::Str(v) = self {
            Some(v)
        } else {
            None
        }
    }
    fn as_type_expr(&self) -> Option<&Option<Arc<JSTypeExpression>>> {
        if let Self::TypeExpr(v) = self {
            Some(v)
        } else {
            None
        }
    }
    fn as_type_map(&self) -> Option<&TypeMap> {
        if let Self::TypeMap(v) = self {
            Some(v)
        } else {
            None
        }
    }
    fn as_type_list(&self) -> Option<&Vec<Arc<JSTypeExpression>>> {
        if let Self::TypeList(v) = self {
            Some(v)
        } else {
            None
        }
    }
    fn as_node_map(&self) -> Option<&IndexMap<JsString, NodeId>> {
        if let Self::NodeMap(v) = self {
            Some(v)
        } else {
            None
        }
    }
    fn as_int(&self) -> Option<&i32> {
        if let Self::Int(v) = self {
            Some(v)
        } else {
            None
        }
    }
    fn as_id_generator(&self) -> Option<&IdGenerator> {
        if let Self::IdGenerator(v) = self {
            Some(v)
        } else {
            None
        }
    }
    fn as_visibility(&self) -> Option<&Visibility> {
        if let Self::Visibility(v) = self {
            Some(v)
        } else {
            None
        }
    }
    fn as_closure_unaware_mode(&self) -> Option<&PerFileClosureUnawareMode> {
        if let Self::ClosureUnawareMode(v) = self {
            Some(v)
        } else {
            None
        }
    }
    fn as_str_list(&self) -> Option<&Vec<JsString>> {
        if let Self::StrList(v) = self {
            Some(v)
        } else {
            None
        }
    }
    fn as_str_set(&self) -> Option<&IndexSet<JsString>> {
        if let Self::StrSet(v) = self {
            Some(v)
        } else {
            None
        }
    }
    fn as_str_map(&self) -> Option<&IndexMap<JsString, JsString>> {
        if let Self::StrMap(v) = self {
            Some(v)
        } else {
            None
        }
    }
    fn as_markers(&self) -> Option<&Vec<Marker>> {
        if let Self::Markers(v) = self {
            Some(v)
        } else {
            None
        }
    }
    fn as_suppressions(&self) -> Option<&Suppressions> {
        if let Self::Suppressions(v) = self {
            Some(v)
        } else {
            None
        }
    }
}
use crate::{
    check_argument,
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    node::{Ast, NodeId},
    rhino_string_pool::RhinoStringPool,
    source_position::SourcePosition,
    token::Token,
};
use indexmap::{IndexMap, IndexSet};
use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
    sync::Arc,
};

pub type TypeTransform<'a> =
    dyn FnMut(&mut Ast, &Arc<JSTypeExpression>) -> Arc<JSTypeExpression> + 'a;
pub type TypeMap = IndexMap<JsString, Option<Arc<JSTypeExpression>>>;
pub type Suppressions = Vec<(IndexSet<JsString>, JsString)>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropertyKind {
    Plain,
    MarkerList,
    Type,
    TypeList,
    TypeMap,
    NodeMap,
}
#[derive(Clone, Copy, Debug)]
pub struct Property {
    pub name: &'static str,
    pub bit: u32,
    pub mask: u64,
    pub kind: PropertyKind,
}
impl Property {
    // port: Property#Property
    const fn new(name: &'static str, bit: u32, kind: PropertyKind) -> Self {
        Self {
            name,
            bit,
            mask: 1 << bit,
            kind,
        }
    }
    // port: Property#get
    pub fn get(self, info: &JSDocInfo) -> Option<&PropertyValue> {
        if info.property_keys_bitset & self.mask == 0 {
            None
        } else {
            Some(info.get_property_value_by_index(
                (info.property_keys_bitset & (self.mask - 1)).count_ones() as usize,
            ))
        }
    }
    // port: Property#compareTo
    pub fn compare_to(self, that: Self) -> i32 {
        self.bit as i32 - that.bit as i32
    }
    // port: Property#isDefault
    pub fn is_default(self, value: &PropertyValue) -> bool {
        value.is_default()
    }
    // port: Property#clone
    pub fn clone_value(
        self,
        ast: &mut Ast,
        arg: &PropertyValue,
        transform: &mut Option<&mut TypeTransform<'_>>,
    ) -> PropertyValue {
        match (self.kind, arg) {
            (PropertyKind::Type, PropertyValue::TypeExpr(v)) => PropertyValue::TypeExpr(
                v.as_ref()
                    .map(|t| transform.as_mut().map_or_else(|| t.clone(), |f| f(ast, t))),
            ),
            (PropertyKind::TypeList, PropertyValue::TypeList(v)) => PropertyValue::TypeList(
                v.iter()
                    .map(|t| transform.as_mut().map_or_else(|| t.clone(), |f| f(ast, t)))
                    .collect(),
            ),
            (PropertyKind::TypeMap, PropertyValue::TypeMap(v)) => PropertyValue::TypeMap(
                v.iter()
                    .map(|(k, t)| {
                        (
                            k.clone(),
                            t.as_ref().map(|t| {
                                transform.as_mut().map_or_else(|| t.clone(), |f| f(ast, t))
                            }),
                        )
                    })
                    .collect(),
            ),
            _ => arg.clone(),
        }
    }
    // port: Property#equalValues
    pub fn equal_values(self, ast: &Ast, left: &PropertyValue, right: &PropertyValue) -> bool {
        self.equal_values_across(ast, ast, left, right)
    }
    // port: Property#equalValues (separate owning arenas)
    pub fn equal_values_across(
        self,
        ast: &Ast,
        ast_b: &Ast,
        left: &PropertyValue,
        right: &PropertyValue,
    ) -> bool {
        match (left, right) {
            (PropertyValue::TypeExpr(a), PropertyValue::TypeExpr(b)) => {
                type_equivalent(ast, ast_b, a.as_ref(), b.as_ref())
            }
            (PropertyValue::TypeList(a), PropertyValue::TypeList(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b)
                        .all(|(a, b)| a.is_equivalent_to_across(ast, ast_b, Some(b)))
            }
            (PropertyValue::TypeMap(a), PropertyValue::TypeMap(b)) => {
                a.len() == b.len()
                    && a.iter().all(|(k, a)| {
                        b.get(k)
                            .is_some_and(|b| type_equivalent(ast, ast_b, a.as_ref(), b.as_ref()))
                    })
            }
            (PropertyValue::NodeMap(a), PropertyValue::NodeMap(b)) => {
                a.len() == b.len()
                    && a.iter().all(|(k, a)| {
                        b.get(k)
                            .is_some_and(|b| a.is_equivalent_to_across(ast, ast_b, *b))
                    })
            }
            (PropertyValue::Markers(a), PropertyValue::Markers(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b)
                        .all(|(a, b)| a.is_equivalent_to_across(ast, ast_b, b))
            }
            (PropertyValue::Visibility(a), PropertyValue::Visibility(b)) => a == b,
            (PropertyValue::Int(a), PropertyValue::Int(b)) => a == b,
            (PropertyValue::IdGenerator(a), PropertyValue::IdGenerator(b)) => a == b,
            (PropertyValue::Str(a), PropertyValue::Str(b)) => a == b,
            (PropertyValue::StrList(a), PropertyValue::StrList(b)) => a == b,
            (PropertyValue::StrMap(a), PropertyValue::StrMap(b)) => {
                a.len() == b.len() && a.iter().all(|(k, v)| b.get(k) == Some(v))
            }
            (PropertyValue::StrSet(a), PropertyValue::StrSet(b)) => a == b,
            (PropertyValue::Suppressions(a), PropertyValue::Suppressions(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.iter().any(|(l, w)| k == l && v == w))
            }
            (PropertyValue::ClosureUnawareMode(a), PropertyValue::ClosureUnawareMode(b)) => a == b,
            (PropertyValue::Null, PropertyValue::Null) => true,
            _ => false,
        }
    }
    // port: Property#getTypeExpressions
    pub fn get_type_expressions(self, value: &PropertyValue) -> Vec<Arc<JSTypeExpression>> {
        match value {
            PropertyValue::TypeExpr(v) => v.iter().cloned().collect(),
            PropertyValue::TypeList(v) => v.clone(),
            PropertyValue::TypeMap(v) => v.values().flatten().cloned().collect(),
            _ => vec![],
        }
    }
}
// port: TypeProperty#equalValues
fn type_equivalent(
    ast: &Ast,
    ast_b: &Ast,
    a: Option<&Arc<JSTypeExpression>>,
    b: Option<&Arc<JSTypeExpression>>,
) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            (std::ptr::eq(ast, ast_b) && Arc::ptr_eq(a, b))
                || a.is_equivalent_to_across(ast, ast_b, Some(b))
        }
        _ => false,
    }
}
#[derive(Clone, Debug)]
pub enum PropertyValue {
    Null,
    Visibility(Visibility),
    TypeExpr(Option<Arc<JSTypeExpression>>),
    TypeList(Vec<Arc<JSTypeExpression>>),
    TypeMap(TypeMap),
    NodeMap(IndexMap<JsString, NodeId>),
    Int(i32),
    IdGenerator(IdGenerator),
    Str(JsString),
    StrList(Vec<JsString>),
    StrMap(IndexMap<JsString, JsString>),
    Suppressions(Suppressions),
    StrSet(IndexSet<JsString>),
    Markers(Vec<Marker>),
    ClosureUnawareMode(PerFileClosureUnawareMode),
}
impl PropertyValue {
    // port: Property#isDefault
    fn is_default(&self) -> bool {
        match self {
            Self::Null | Self::TypeExpr(None) | Self::Visibility(Visibility::INHERITED) => true,
            Self::TypeList(v) => v.is_empty(),
            Self::TypeMap(v) => v.is_empty(),
            Self::NodeMap(v) => v.is_empty(),
            Self::StrList(v) => v.is_empty(),
            Self::StrMap(v) => v.is_empty(),
            Self::Suppressions(v) => v.is_empty(),
            Self::StrSet(v) => v.is_empty(),
            Self::Markers(v) => v.is_empty(),
            _ => false,
        }
    }
    fn is_null(&self) -> bool {
        matches!(self, Self::Null | Self::TypeExpr(None))
    }
    // port: Property#toString
    fn java_string(
        &self,
        ast: &Ast,
        type_printer: &mut crate::node::JSTypePrinter<'_>,
    ) -> JsString {
        // port: AbstractCollection#toString
        fn list(v: impl Iterator<Item = JsString>) -> JsString {
            let mut result = JsString::from("[");
            for (i, value) in v.enumerate() {
                if i > 0 {
                    result = result.concat(&", ".into());
                }
                result = result.concat(&value);
            }
            result.concat(&"]".into())
        }
        // port: AbstractMap#toString
        fn map(v: impl Iterator<Item = (JsString, JsString)>) -> JsString {
            let mut result = JsString::from("{");
            for (i, (key, value)) in v.enumerate() {
                if i > 0 {
                    result = result.concat(&", ".into());
                }
                result = result.concat(&key).concat(&"=".into()).concat(&value);
            }
            result.concat(&"}".into())
        }
        match self {
            Self::Null | Self::TypeExpr(None) => "null".into(),
            Self::TypeExpr(Some(v)) => v.to_string_with_types_utf16(ast, type_printer),
            Self::TypeList(v) => list(
                v.iter()
                    .map(|v| v.to_string_with_types_utf16(ast, type_printer)),
            ),
            Self::TypeMap(v) => map(v.iter().map(|(k, v)| {
                (
                    k.clone(),
                    v.as_ref().map_or("null".into(), |v| {
                        v.to_string_with_types_utf16(ast, type_printer)
                    }),
                )
            })),
            Self::NodeMap(v) => map(v.iter().map(|(k, v)| {
                (
                    k.clone(),
                    v.to_string_with_options_and_types_utf16(ast, true, true, true, type_printer),
                )
            })),
            Self::Int(v) => v.to_string().into(),
            Self::Visibility(v) => format!("{v:?}").into(),
            Self::IdGenerator(v) => format!("{v:?}").into(),
            Self::ClosureUnawareMode(v) => format!("{v:?}").into(),
            Self::Str(v) => v.clone(),
            Self::StrList(v) => list(v.iter().cloned()),
            Self::StrSet(v) => list(v.iter().cloned()),
            Self::StrMap(v) => map(v.iter().map(|(k, v)| (k.clone(), v.clone()))),
            Self::Suppressions(v) => {
                map(v.iter().map(|(k, v)| (list(k.iter().cloned()), v.clone())))
            }
            Self::Markers(v) => list(v.iter().enumerate().map(|(i, _)| {
                format!("com.google.javascript.rhino.JSDocInfo$Marker@{i:x}").into()
            })), // Java uses an identity hash for Marker.
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Visibility {
    PRIVATE,
    PACKAGE,
    PROTECTED,
    PUBLIC,
    INHERITED,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdGenerator {
    XID,
    CONSISTENT,
    UNIQUE,
    STABLE,
    MAPPED,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerFileClosureUnawareMode {
    UNSPECIFIED,
    SIMPLE,
    WHITESPACE,
}

#[derive(Clone, Debug, Default)]
pub struct StringPosition(SourcePosition<JsString>);
impl Deref for StringPosition {
    type Target = SourcePosition<JsString>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for StringPosition {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl StringPosition {
    // port: StringPosition#isEquivalentTo
    pub fn is_equivalent_to(&self, that: Option<&Self>) -> bool {
        that.is_some_and(|that| {
            self.is_same_position_as(that) && self.get_item() == that.get_item()
        })
    }
    // port: SourcePosition#setItem
    pub fn set_item(&mut self, item: impl Into<JsString>) {
        self.0.set_item(item.into());
    }
}
#[derive(Clone, Debug, Default)]
pub struct TrimmedStringPosition(StringPosition);
impl Deref for TrimmedStringPosition {
    type Target = StringPosition;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for TrimmedStringPosition {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl TrimmedStringPosition {
    // port: TrimmedStringPosition#setItem
    pub fn set_item(&mut self, item: impl Into<JsString>) {
        let item = item.into();
        check_argument!(
            item.char_at(0) != 32 && item.char_at(item.length() - 1) != 32,
            "String has leading or trailing whitespace"
        );
        self.0.set_item(item);
    }
}
#[derive(Clone, Debug, Default)]
pub struct NamePosition(SourcePosition<NodeId>);
impl Deref for NamePosition {
    type Target = SourcePosition<NodeId>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for NamePosition {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl NamePosition {
    // port: NamePosition#isEquivalentTo
    pub fn is_equivalent_to(&self, ast: &Ast, that: Option<&Self>) -> bool {
        self.is_equivalent_to_across(ast, ast, that)
    }
    // port: NamePosition#isEquivalentTo (separate owning arenas)
    pub fn is_equivalent_to_across(&self, ast: &Ast, ast_b: &Ast, that: Option<&Self>) -> bool {
        that.is_some_and(|that| {
            self.is_same_position_as(that)
                && match (self.get_item(), that.get_item()) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a.is_equivalent_to_across(ast, ast_b, *b),
                    _ => false,
                }
        })
    }
}
#[derive(Clone, Debug, Default)]
pub struct TypePosition {
    position: NamePosition,
    brackets: bool,
}
impl Deref for TypePosition {
    type Target = NamePosition;
    fn deref(&self) -> &Self::Target {
        &self.position
    }
}
impl DerefMut for TypePosition {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.position
    }
}
impl TypePosition {
    // port: TypePosition#hasBrackets
    pub fn has_brackets(&self) -> bool {
        self.brackets
    }
    // port: TypePosition#setHasBrackets
    pub fn set_has_brackets(&mut self, new_val: bool) {
        self.brackets = new_val;
    }
    // port: TypePosition#isEquivalentTo
    pub fn is_equivalent_to(&self, ast: &Ast, that: Option<&Self>) -> bool {
        self.is_equivalent_to_across(ast, ast, that)
    }
    // port: TypePosition#isEquivalentTo (separate owning arenas)
    pub fn is_equivalent_to_across(&self, ast: &Ast, ast_b: &Ast, that: Option<&Self>) -> bool {
        that.is_some_and(|that| {
            self.brackets == that.brackets
                && self
                    .position
                    .is_equivalent_to_across(ast, ast_b, Some(&that.position))
        })
    }
}
#[derive(Clone, Debug, Default)]
pub struct Marker {
    annotation: Option<TrimmedStringPosition>,
    name_node: Option<NamePosition>,
    description: Option<StringPosition>,
    r#type: Option<TypePosition>,
}
impl Marker {
    // port: Marker#getAnnotation
    pub fn get_annotation(&self) -> Option<&StringPosition> {
        self.annotation.as_deref()
    }
    // port: Marker#setAnnotation
    pub fn set_annotation(&mut self, p: TrimmedStringPosition) {
        self.annotation = Some(p);
    }
    // port: Marker#getNameNode
    pub fn get_name_node(&self) -> Option<&NamePosition> {
        self.name_node.as_ref()
    }
    // port: Marker#setNameNode
    pub fn set_name_node(&mut self, p: NamePosition) {
        self.name_node = Some(p);
    }
    // port: Marker#getDescription
    pub fn get_description(&self) -> Option<&StringPosition> {
        self.description.as_ref()
    }
    // port: Marker#setDescription
    pub fn set_description(&mut self, p: StringPosition) {
        self.description = Some(p);
    }
    // port: Marker#getType
    pub fn get_type(&self) -> Option<&TypePosition> {
        self.r#type.as_ref()
    }
    // port: Marker#setType
    pub fn set_type(&mut self, p: TypePosition) {
        self.r#type = Some(p);
    }
    // port: Marker#isEquivalentTo
    pub fn is_equivalent_to(&self, ast: &Ast, that: &Self) -> bool {
        self.is_equivalent_to_across(ast, ast, that)
    }
    // port: Marker#isEquivalentTo (separate owning arenas)
    pub fn is_equivalent_to_across(&self, ast: &Ast, ast_b: &Ast, that: &Self) -> bool {
        Self::are_equivalent(self.get_annotation(), that.get_annotation(), |a, b| {
            a.is_equivalent_to(Some(b))
        }) && Self::are_equivalent(self.get_name_node(), that.get_name_node(), |a, b| {
            a.is_equivalent_to_across(ast, ast_b, Some(b))
        }) && Self::are_equivalent(self.get_description(), that.get_description(), |a, b| {
            a.is_equivalent_to(Some(b))
        }) && Self::are_equivalent(self.get_type(), that.get_type(), |a, b| {
            a.is_equivalent_to_across(ast, ast_b, Some(b))
        })
    }
    // port: Marker#areEquivalent
    fn are_equivalent<T>(a: Option<&T>, b: Option<&T>, f: impl FnOnce(&T, &T) -> bool) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => f(a, b),
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Default)]
enum PackedPropertyValues {
    #[default]
    Empty,
    Single(PropertyValue),
    Multiple(Vec<PropertyValue>),
}
#[derive(Clone, Debug, Default)]
pub struct JSDocInfo {
    property_bits: u64,
    property_keys_bitset: u64,
    property_values: PackedPropertyValues,
}
impl JSDocInfo {
    // port: JSDocInfo#JSDocInfo
    fn new(bits: u64, props: &BTreeMap<u32, PropertyValue>) -> Self {
        let mut keys = 0;
        let mut values = vec![];
        for (&bit, value) in props {
            if !PROPERTIES[bit as usize].is_default(value) {
                keys |= 1 << bit;
                values.push(value.clone());
            }
        }
        Self {
            property_bits: bits,
            property_keys_bitset: keys,
            property_values: Self::pack_property_values(values),
        }
    }
    // port: JSDocInfo#packPropertyValues
    fn pack_property_values(mut values: Vec<PropertyValue>) -> PackedPropertyValues {
        match values.len() {
            0 => PackedPropertyValues::Empty,
            1 => PackedPropertyValues::Single(values.pop().unwrap()),
            _ => PackedPropertyValues::Multiple(values),
        }
    }
    // port: JSDocInfo#getPropertyValueByIndex
    pub fn get_property_value_by_index(&self, index: usize) -> &PropertyValue {
        match &self.property_values {
            PackedPropertyValues::Empty => panic!("no property value"),
            PackedPropertyValues::Single(v) => {
                check_argument!(index == 0);
                v
            }
            PackedPropertyValues::Multiple(v) => &v[index],
        }
    }
    // port: JSDocInfo#checkBit
    fn check_bit(&self, bit: Bit) -> bool {
        self.property_bits & bit.mask() != 0
    }
    // port: JSDocInfo#asMap
    fn as_map(&self) -> BTreeMap<u32, PropertyValue> {
        PROPERTIES
            .iter()
            .filter_map(|p| p.get(self).map(|v| (p.bit, v.clone())))
            .collect()
    }
    // port: JSDocInfo#builder
    pub fn builder() -> Builder {
        Builder::default()
    }
    // port: JSDocInfo#toBuilder
    pub fn to_builder(&self) -> Builder {
        // Copy collections: Closure does not mutate shared collections after build/toBuilder.
        Builder {
            bits: self.property_bits & !Bit::INLINE_TYPE.mask(),
            props: self.as_map(),
            populated: true,
            ..Builder::default()
        }
    }
    // port: JSDocInfo#toBuilder(TypeTransform)
    fn to_builder_with_transform(
        &self,
        ast: &mut Ast,
        mut transform: Option<&mut TypeTransform<'_>>,
    ) -> Builder {
        let mut builder = self.to_builder();
        for (&bit, value) in builder.props.iter_mut() {
            *value = PROPERTIES[bit as usize].clone_value(ast, value, &mut transform);
        }
        builder
    }
    // port: JSDocInfo#clone
    pub fn clone_info(&self) -> Self {
        self.to_builder().build().unwrap().as_ref().clone()
    }
    // port: JSDocInfo#clone(boolean)
    pub fn clone_with_type_nodes(&self, ast: &mut Ast, clone_type_nodes: bool) -> Self {
        if clone_type_nodes {
            self.to_builder_with_transform(ast, Some(&mut |ast, t| Arc::new(t.copy(ast))))
                .build()
                .unwrap()
                .as_ref()
                .clone()
        } else {
            self.clone_info()
        }
    }
    // port: JSDocInfo#cloneAndReplaceTypeNames
    pub fn clone_and_replace_type_names(&self, ast: &mut Ast, names: &IndexSet<JsString>) -> Self {
        self.to_builder_with_transform(
            ast,
            Some(&mut |ast, t| Arc::new(t.replace_names_with_unknown_type(ast, names))),
        )
        .build()
        .unwrap()
        .as_ref()
        .clone()
    }
    // port: JSDocInfo#areEquivalent
    pub fn are_equivalent(ast: &Ast, a: Option<&Self>, b: Option<&Self>) -> bool {
        Self::are_equivalent_across(ast, ast, a, b)
    }
    // port: JSDocInfo#areEquivalent (separate owning arenas)
    pub fn are_equivalent_across(
        ast: &Ast,
        ast_b: &Ast,
        a: Option<&Self>,
        b: Option<&Self>,
    ) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                if (a.property_bits ^ b.property_bits)
                    & !(Bit::INCLUDE_DOCUMENTATION.mask() | Bit::INLINE_TYPE.mask())
                    != 0
                    || a.property_keys_bitset != b.property_keys_bitset
                {
                    return false;
                }
                PROPERTIES.iter().all(|p| match (p.get(a), p.get(b)) {
                    (None, None) => true,
                    (Some(a), Some(b)) => p.equal_values_across(ast, ast_b, a, b),
                    _ => false,
                })
            }
            _ => false,
        }
    }
    // port: JSDocInfo#isConstant
    pub fn is_constant(&self) -> bool {
        self.property_bits & (Bit::CONST.mask() | Bit::DEFINE.mask() | Bit::FINAL.mask()) != 0
            || self.property_keys_bitset & DESCRIPTION.mask != 0
    }
    // port: JSDocInfo#isDocumentationIncluded
    pub fn is_documentation_included(&self) -> bool {
        self.check_bit(Bit::INCLUDE_DOCUMENTATION)
    }
    // port: JSDocInfo#toString
    pub fn to_string(&self, ast: &Ast) -> String {
        self.to_string_verbose(ast)
    }
    // port: JSDocInfo#toStringVerbose
    pub fn to_string_verbose(&self, ast: &Ast) -> String {
        crate::java_lang::charset::utf8_encoded_text(self.to_string_utf16(ast).as_units())
    }
    // port: JSDocInfo#toStringVerbose
    pub fn to_string_utf16(&self, ast: &Ast) -> JsString {
        self.to_string_with_types_utf16(ast, &mut crate::node::no_registry_jstype_printer)
    }
    // port: JSDocInfo#toStringVerbose
    /// `toStringVerbose` with a registry-aware JSType printer (see [`crate::node::JSTypePrinter`]).
    pub fn to_string_with_types_utf16(
        &self,
        ast: &Ast,
        type_printer: &mut crate::node::JSTypePrinter<'_>,
    ) -> JsString {
        let mut fields: Vec<JsString> = vec![];
        if self.property_bits != 0 {
            fields.push(format!("bitset={:x}", self.property_bits).into());
        }
        for b in BITS {
            if self.check_bit(*b) {
                fields.push(format!("bit:{}=true", b.name()).into());
            }
        }
        for p in PROPERTIES {
            if let Some(v) = p.get(self) {
                fields.push(
                    JsString::from(format!("{}=", p.name))
                        .concat(&v.java_string(ast, type_printer)),
                );
            }
        }
        let mut result = JsString::from("JSDocInfo{");
        for (i, field) in fields.into_iter().enumerate() {
            if i > 0 {
                result = result.concat(&", ".into());
            }
            result = result.concat(&field);
        }
        result.concat(&"}".into())
    }
    // port: JSDocInfo#isAnyIdGenerator
    pub fn is_any_id_generator(&self) -> bool {
        (self.property_keys_bitset & ID_GENERATOR.mask) != 0
    }
    // port: JSDocInfo#isConsistentIdGenerator
    pub fn is_consistent_id_generator(&self) -> bool {
        ID_GENERATOR
            .get(self)
            .and_then(PropertyValue::as_id_generator)
            == Some(&IdGenerator::CONSISTENT)
    }
    // port: JSDocInfo#isStableIdGenerator
    pub fn is_stable_id_generator(&self) -> bool {
        ID_GENERATOR
            .get(self)
            .and_then(PropertyValue::as_id_generator)
            == Some(&IdGenerator::STABLE)
    }
    // port: JSDocInfo#isXidGenerator
    pub fn is_xid_generator(&self) -> bool {
        ID_GENERATOR
            .get(self)
            .and_then(PropertyValue::as_id_generator)
            == Some(&IdGenerator::XID)
    }
    // port: JSDocInfo#isMappedIdGenerator
    pub fn is_mapped_id_generator(&self) -> bool {
        ID_GENERATOR
            .get(self)
            .and_then(PropertyValue::as_id_generator)
            == Some(&IdGenerator::MAPPED)
    }
    // port: JSDocInfo#isIdGenerator
    pub fn is_id_generator(&self) -> bool {
        ID_GENERATOR
            .get(self)
            .and_then(PropertyValue::as_id_generator)
            == Some(&IdGenerator::UNIQUE)
    }
    // port: JSDocInfo#hasConstAnnotation
    pub fn has_const_annotation(&self) -> bool {
        self.check_bit(Bit::CONST)
    }
    // port: JSDocInfo#isFinal
    pub fn is_final(&self) -> bool {
        self.check_bit(Bit::FINAL)
    }
    // port: JSDocInfo#isConstructor
    pub fn is_constructor(&self) -> bool {
        self.check_bit(Bit::CONSTRUCTOR)
    }
    // port: JSDocInfo#isAbstract
    pub fn is_abstract(&self) -> bool {
        self.check_bit(Bit::ABSTRACT)
    }
    // port: JSDocInfo#usesImplicitMatch
    pub fn uses_implicit_match(&self) -> bool {
        self.check_bit(Bit::RECORD)
    }
    // port: JSDocInfo#makesUnrestricted
    pub fn makes_unrestricted(&self) -> bool {
        self.check_bit(Bit::UNRESTRICTED)
    }
    // port: JSDocInfo#makesStructs
    pub fn makes_structs(&self) -> bool {
        self.check_bit(Bit::STRUCT)
    }
    // port: JSDocInfo#makesDicts
    pub fn makes_dicts(&self) -> bool {
        self.check_bit(Bit::DICT)
    }
    // port: JSDocInfo#isDefine
    pub fn is_define(&self) -> bool {
        self.check_bit(Bit::DEFINE)
    }
    // port: JSDocInfo#isOverride
    pub fn is_override(&self) -> bool {
        self.check_bit(Bit::OVERRIDE)
    }
    // port: JSDocInfo#isDeprecated
    pub fn is_deprecated(&self) -> bool {
        self.check_bit(Bit::DEPRECATED)
    }
    // port: JSDocInfo#isInterface
    pub fn is_interface(&self) -> bool {
        (self.property_bits & (Bit::INTERFACE.mask() | Bit::RECORD.mask())) != 0
    }
    // port: JSDocInfo#isConstructorOrInterface
    pub fn is_constructor_or_interface(&self) -> bool {
        (self.property_bits
            & (Bit::CONSTRUCTOR.mask() | Bit::INTERFACE.mask() | Bit::RECORD.mask()))
            != 0
    }
    // port: JSDocInfo#isExport
    pub fn is_export(&self) -> bool {
        self.check_bit(Bit::EXPORT)
    }
    // port: JSDocInfo#isImplicitCast
    pub fn is_implicit_cast(&self) -> bool {
        self.check_bit(Bit::IMPLICITCAST)
    }
    // port: JSDocInfo#isNoSideEffects
    pub fn is_no_side_effects(&self) -> bool {
        self.check_bit(Bit::NOSIDEEFFECTS)
    }
    // port: JSDocInfo#isExterns
    pub fn is_externs(&self) -> bool {
        self.check_bit(Bit::EXTERNS)
    }
    // port: JSDocInfo#isNoCoverage
    pub fn is_no_coverage(&self) -> bool {
        self.check_bit(Bit::NOCOVERAGE)
    }
    // port: JSDocInfo#isTypeSummary
    pub fn is_type_summary(&self) -> bool {
        self.check_bit(Bit::TYPE_SUMMARY)
    }
    // port: JSDocInfo#isNoCompile
    pub fn is_no_compile(&self) -> bool {
        self.check_bit(Bit::NOCOMPILE)
    }
    // port: JSDocInfo#isNoDts
    pub fn is_no_dts(&self) -> bool {
        self.check_bit(Bit::NODTS)
    }
    // port: JSDocInfo#isNoCollapse
    pub fn is_no_collapse(&self) -> bool {
        self.check_bit(Bit::NOCOLLAPSE)
    }
    // port: JSDocInfo#isNoInline
    pub fn is_no_inline(&self) -> bool {
        self.check_bit(Bit::NOINLINE)
    }
    // port: JSDocInfo#isRequireInlining
    pub fn is_require_inlining(&self) -> bool {
        self.check_bit(Bit::REQUIRE_INLINING)
    }
    // port: JSDocInfo#isEncourageInlining
    pub fn is_encourage_inlining(&self) -> bool {
        self.check_bit(Bit::ENCOURAGE_INLINING)
    }
    // port: JSDocInfo#isCollapsibleOrBreakMyCode
    pub fn is_collapsible_or_break_my_code(&self) -> bool {
        self.check_bit(Bit::COLLAPSIBLE_OR_BREAK_MY_CODE)
    }
    // port: JSDocInfo#isPureOrBreakMyCode
    pub fn is_pure_or_break_my_code(&self) -> bool {
        self.check_bit(Bit::PURE_OR_BREAK_MY_CODE)
    }
    // port: JSDocInfo#isProvideGoog
    pub fn is_provide_goog(&self) -> bool {
        self.check_bit(Bit::PROVIDE_GOOG)
    }
    // port: JSDocInfo#isProvideAlreadyProvided
    pub fn is_provide_already_provided(&self) -> bool {
        self.check_bit(Bit::PROVIDE_ALREADY_PROVIDED)
    }
    // port: JSDocInfo#containsDeclarationExcludingTypelessConst
    pub fn contains_declaration_excluding_typeless_const(&self) -> bool {
        (self.property_bits
            & (Bit::CONSTRUCTOR.mask()
                | Bit::DEFINE.mask()
                | Bit::OVERRIDE.mask()
                | Bit::EXPORT.mask()
                | Bit::DEPRECATED.mask()
                | Bit::INTERFACE.mask()
                | Bit::IMPLICITCAST.mask()
                | Bit::NOSIDEEFFECTS.mask()
                | Bit::RECORD.mask()))
            != 0
            || (self.property_keys_bitset
                & (TYPE.mask
                    | RETURN_TYPE.mask
                    | ENUM_PARAMETER_TYPE.mask
                    | TYPEDEF_TYPE.mask
                    | THIS_TYPE.mask
                    | PARAMETERS.mask
                    | IMPLEMENTED_INTERFACES.mask
                    | BASE_TYPE.mask
                    | VISIBILITY.mask))
                != 0
    }
    // port: JSDocInfo#containsTypeDeclaration
    pub fn contains_type_declaration(&self) -> bool {
        (self.property_keys_bitset
            & (TYPE.mask
                | RETURN_TYPE.mask
                | ENUM_PARAMETER_TYPE.mask
                | TYPEDEF_TYPE.mask
                | THIS_TYPE.mask
                | PARAMETERS.mask
                | BASE_TYPE.mask))
            != 0
    }
    // port: JSDocInfo#containsDeclaration
    pub fn contains_declaration(&self) -> bool {
        self.contains_declaration_excluding_typeless_const() || self.check_bit(Bit::CONST)
    }
    // port: JSDocInfo#containsTypeDefinition
    pub fn contains_type_definition(&self) -> bool {
        (self.property_bits & (Bit::CONSTRUCTOR.mask() | Bit::INTERFACE.mask())) != 0
            || (self.property_keys_bitset & (ENUM_PARAMETER_TYPE.mask | TYPEDEF_TYPE.mask)) != 0
    }
    // port: JSDocInfo#hasEnumParameterType
    pub fn has_enum_parameter_type(&self) -> bool {
        ENUM_PARAMETER_TYPE.get(self).is_some()
    }
    // port: JSDocInfo#hasTypedefType
    pub fn has_typedef_type(&self) -> bool {
        TYPEDEF_TYPE.get(self).is_some()
    }
    // port: JSDocInfo#hasReturnType
    pub fn has_return_type(&self) -> bool {
        RETURN_TYPE.get(self).is_some()
    }
    // port: JSDocInfo#hasType
    pub fn has_type(&self) -> bool {
        TYPE.get(self).is_some()
    }
    // port: JSDocInfo#hasTypeInformation
    pub fn has_type_information(&self) -> bool {
        (self.property_keys_bitset
            & (TYPE.mask | TYPEDEF_TYPE.mask | ENUM_PARAMETER_TYPE.mask | RETURN_TYPE.mask))
            != 0
    }
    // port: JSDocInfo#isInlineType
    pub fn is_inline_type(&self) -> bool {
        self.check_bit(Bit::INLINE_TYPE)
    }
    // port: JSDocInfo#getReturnType
    pub fn get_return_type(&self) -> Option<Arc<JSTypeExpression>> {
        RETURN_TYPE
            .get(self)
            .and_then(PropertyValue::as_type_expr)
            .cloned()
            .flatten()
    }
    // port: JSDocInfo#getEnumParameterType
    pub fn get_enum_parameter_type(&self) -> Option<Arc<JSTypeExpression>> {
        ENUM_PARAMETER_TYPE
            .get(self)
            .and_then(PropertyValue::as_type_expr)
            .cloned()
            .flatten()
    }
    // port: JSDocInfo#getTypedefType
    pub fn get_typedef_type(&self) -> Option<Arc<JSTypeExpression>> {
        TYPEDEF_TYPE
            .get(self)
            .and_then(PropertyValue::as_type_expr)
            .cloned()
            .flatten()
    }
    // port: JSDocInfo#getType
    pub fn get_type(&self) -> Option<Arc<JSTypeExpression>> {
        TYPE.get(self)
            .and_then(PropertyValue::as_type_expr)
            .cloned()
            .flatten()
    }
    // port: JSDocInfo#getThisType
    pub fn get_this_type(&self) -> Option<Arc<JSTypeExpression>> {
        THIS_TYPE
            .get(self)
            .and_then(PropertyValue::as_type_expr)
            .cloned()
            .flatten()
    }
    // port: JSDocInfo#hasThisType
    pub fn has_this_type(&self) -> bool {
        THIS_TYPE.get(self).is_some()
    }
    // port: JSDocInfo#getBaseType
    pub fn get_base_type(&self) -> Option<Arc<JSTypeExpression>> {
        BASE_TYPE
            .get(self)
            .and_then(PropertyValue::as_type_expr)
            .cloned()
            .flatten()
    }
    // port: JSDocInfo#getDescription
    pub fn get_description(&self) -> Option<JsString> {
        DESCRIPTION
            .get(self)
            .and_then(PropertyValue::as_str)
            .cloned()
    }
    // port: JSDocInfo#getMeaning
    pub fn get_meaning(&self) -> Option<JsString> {
        MEANING.get(self).and_then(PropertyValue::as_str).cloned()
    }
    // port: JSDocInfo#getAlternateMessageId
    pub fn get_alternate_message_id(&self) -> Option<JsString> {
        ALTERNATE_MESSAGE_ID
            .get(self)
            .and_then(PropertyValue::as_str)
            .cloned()
    }
    // port: JSDocInfo#getLendsName
    pub fn get_lends_name(&self) -> Option<Arc<JSTypeExpression>> {
        LENDS_NAME
            .get(self)
            .and_then(PropertyValue::as_type_expr)
            .cloned()
            .flatten()
    }
    // port: JSDocInfo#hasLendsName
    pub fn has_lends_name(&self) -> bool {
        self.get_lends_name().is_some()
    }
    // port: JSDocInfo#getClosurePrimitiveId
    pub fn get_closure_primitive_id(&self) -> Option<JsString> {
        CLOSURE_PRIMITIVE_ID
            .get(self)
            .and_then(PropertyValue::as_str)
            .cloned()
    }
    // port: JSDocInfo#hasClosurePrimitiveId
    pub fn has_closure_primitive_id(&self) -> bool {
        CLOSURE_PRIMITIVE_ID.get(self).is_some()
    }
    // port: JSDocInfo#isNgInject
    pub fn is_ng_inject(&self) -> bool {
        self.check_bit(Bit::NG_INJECT)
    }
    // port: JSDocInfo#isWizaction
    pub fn is_wizaction(&self) -> bool {
        self.check_bit(Bit::WIZ_ACTION)
    }
    // port: JSDocInfo#isWizcallback
    pub fn is_wizcallback(&self) -> bool {
        self.check_bit(Bit::WIZ_CALLBACK)
    }
    // port: JSDocInfo#isPolymerBehavior
    pub fn is_polymer_behavior(&self) -> bool {
        self.check_bit(Bit::POLYMER_BEHAVIOR)
    }
    // port: JSDocInfo#isPolymer
    pub fn is_polymer(&self) -> bool {
        self.check_bit(Bit::POLYMER)
    }
    // port: JSDocInfo#isCustomElement
    pub fn is_custom_element(&self) -> bool {
        self.check_bit(Bit::CUSTOM_ELEMENT)
    }
    // port: JSDocInfo#isMixinClass
    pub fn is_mixin_class(&self) -> bool {
        self.check_bit(Bit::MIXIN_CLASS)
    }
    // port: JSDocInfo#isMixinFunction
    pub fn is_mixin_function(&self) -> bool {
        self.check_bit(Bit::MIXIN_FUNCTION)
    }
    // port: JSDocInfo#isSassGeneratedCssTs
    pub fn is_sass_generated_css_ts(&self) -> bool {
        self.check_bit(Bit::SASS_GENERATED_CSS_TS)
    }
    // port: JSDocInfo#isClosureUnawareCode
    pub fn is_closure_unaware_code(&self) -> bool {
        CLOSURE_UNAWARE_CONFIG_VALUE.get(self).is_some()
    }
    // port: JSDocInfo#getPerFileClosureUnawareMode
    pub fn get_per_file_closure_unaware_mode(&self) -> Option<PerFileClosureUnawareMode> {
        CLOSURE_UNAWARE_CONFIG_VALUE
            .get(self)
            .and_then(PropertyValue::as_closure_unaware_mode)
            .cloned()
    }
    // port: JSDocInfo#isUsedViaDotConstructor
    pub fn is_used_via_dot_constructor(&self) -> bool {
        self.check_bit(Bit::USED_VIA_DOT_CONSTRUCTOR)
    }
    // port: JSDocInfo#getLicense
    pub fn get_license(&self) -> Option<JsString> {
        LICENSE.get(self).and_then(PropertyValue::as_str).cloned()
    }
    // port: JSDocInfo#hasBaseType
    pub fn has_base_type(&self) -> bool {
        self.get_base_type().is_some()
    }
    // port: JSDocInfo#getDeprecationReason
    pub fn get_deprecation_reason(&self) -> Option<JsString> {
        DEPRECATION_REASON
            .get(self)
            .and_then(PropertyValue::as_str)
            .cloned()
    }
    // port: JSDocInfo#getAuthors
    pub fn get_authors(&self) -> Option<Vec<JsString>> {
        AUTHORS
            .get(self)
            .and_then(PropertyValue::as_str_list)
            .cloned()
    }
    // port: JSDocInfo#getReferences
    pub fn get_references(&self) -> Option<Vec<JsString>> {
        SEES.get(self).and_then(PropertyValue::as_str_list).cloned()
    }
    // port: JSDocInfo#getReturnDescription
    pub fn get_return_description(&self) -> Option<JsString> {
        RETURN_DESCRIPTION
            .get(self)
            .and_then(PropertyValue::as_str)
            .cloned()
    }
    // port: JSDocInfo#getBlockDescription
    pub fn get_block_description(&self) -> Option<JsString> {
        BLOCK_DESCRIPTION
            .get(self)
            .and_then(PropertyValue::as_str)
            .cloned()
    }
    // port: JSDocInfo#hasFileOverview
    pub fn has_file_overview(&self) -> bool {
        self.check_bit(Bit::FILEOVERVIEW)
    }
    // port: JSDocInfo#getFileOverview
    pub fn get_file_overview(&self) -> Option<JsString> {
        FILEOVERVIEW_DESCRIPTION
            .get(self)
            .and_then(PropertyValue::as_str)
            .cloned()
    }
    // port: JSDocInfo#hasEnhance
    pub fn has_enhance(&self) -> bool {
        self.check_bit(Bit::ENHANCED_NAMESPACE)
    }
    // port: JSDocInfo#getEnhance
    pub fn get_enhance(&self) -> Option<JsString> {
        ENHANCED_NAMESPACE
            .get(self)
            .and_then(PropertyValue::as_str)
            .cloned()
    }
    // port: JSDocInfo#hasMods
    pub fn has_mods(&self) -> bool {
        MODS.get(self).is_some()
    }
    // port: JSDocInfo#getMods
    pub fn get_mods(&self) -> Option<JsString> {
        MODS.get(self).and_then(PropertyValue::as_str).cloned()
    }
    // port: JSDocInfo#hasModifies
    pub fn has_modifies(&self) -> bool {
        MODIFIES.get(self).is_some()
    }
    // port: JSDocInfo#getOriginalCommentString
    pub fn get_original_comment_string(&self) -> Option<JsString> {
        SOURCE_COMMENT
            .get(self)
            .and_then(PropertyValue::as_str)
            .cloned()
    }
    // port: JSDocInfo#modifiesThis
    pub fn modifies_this(&self) -> bool {
        self.get_modifies().contains(&JsString::from("this"))
    }
    // port: JSDocInfo#getLogTypeInCompiler
    pub fn get_log_type_in_compiler(&self) -> bool {
        self.check_bit(Bit::LOG_TYPE_IN_COMPILER)
    }
    // port: JSDocInfo#getVisibility
    pub fn get_visibility(&self) -> Visibility {
        VISIBILITY
            .get(self)
            .and_then(PropertyValue::as_visibility)
            .copied()
            .unwrap_or(Visibility::INHERITED)
    }
    // port: JSDocInfo#containsFunctionDeclaration
    pub fn contains_function_declaration(&self, ast: &Ast) -> bool {
        if self.check_bit(Bit::CONSTRUCTOR)
            || (self.property_keys_bitset & (RETURN_TYPE.mask | THIS_TYPE.mask | PARAMETERS.mask))
                != 0
        {
            return true;
        }
        if let Some(t) = self.get_type() {
            return t.get_root().is_function(ast);
        }
        self.check_bit(Bit::NOSIDEEFFECTS)
    }
    // port: JSDocInfo#isAtSignCodePresent
    pub fn is_at_sign_code_present(&self) -> bool {
        self.get_original_comment_string()
            .is_some_and(|s| s.index_of("@code") != -1)
    }
    // port: JSDocInfo#getOriginalCommentPosition
    pub fn get_original_comment_position(&self) -> i32 {
        ORIGINAL_COMMENT_POSITION
            .get(self)
            .and_then(PropertyValue::as_int)
            .copied()
            .unwrap_or(0)
    }
    // port: JSDocInfo#getParameterNames
    pub fn get_parameter_names(&self) -> IndexSet<JsString> {
        PARAMETERS
            .get(self)
            .and_then(PropertyValue::as_type_map)
            .map(|p| p.keys().cloned().collect())
            .unwrap_or_default()
    }
    // port: JSDocInfo#getTemplateTypeNames
    pub fn get_template_type_names(&self) -> Vec<JsString> {
        TEMPLATE_TYPE_NAMES
            .get(self)
            .and_then(PropertyValue::as_type_map)
            .map(|p| p.keys().cloned().collect())
            .unwrap_or_default()
    }
    // port: JSDocInfo#getSuppressions
    pub fn get_suppressions(&self) -> IndexSet<JsString> {
        self.get_suppressions_and_their_description()
            .iter()
            .flat_map(|(s, _)| s.iter().cloned())
            .collect()
    }
    // port: JSDocInfo#getTypeExpressions
    pub fn get_type_expressions(&self) -> Vec<Arc<JSTypeExpression>> {
        PROPERTIES
            .iter()
            .flat_map(|p| {
                p.get(self)
                    .map(|v| p.get_type_expressions(v))
                    .unwrap_or_default()
            })
            .collect()
    }
    // port: JSDocInfo#getTypeNodes
    pub fn get_type_nodes(&self) -> Vec<NodeId> {
        self.get_type_expressions()
            .iter()
            .map(|t| t.get_root())
            .collect()
    }
    // port: JSDocInfo#hasSideEffectsArgumentsAnnotation
    pub fn has_side_effects_arguments_annotation(&self) -> bool {
        let modifies = self.get_modifies();
        modifies.len() > 1 || (modifies.len() == 1 && !modifies.contains(&JsString::from("this")))
    }
    // port: JSDocInfo#getTsTypes
    pub fn get_ts_types(&self) -> Vec<JsString> {
        TS_TYPES
            .get(self)
            .and_then(PropertyValue::as_str_list)
            .cloned()
            .unwrap_or_default()
    }
    // port: JSDocInfo#getThrowsAnnotations
    pub fn get_throws_annotations(&self) -> Vec<JsString> {
        THROWS_ANNOTATIONS
            .get(self)
            .and_then(PropertyValue::as_str_list)
            .cloned()
            .unwrap_or_default()
    }
    // port: JSDocInfo#getModifies
    pub fn get_modifies(&self) -> IndexSet<JsString> {
        MODIFIES
            .get(self)
            .and_then(PropertyValue::as_str_set)
            .cloned()
            .unwrap_or_default()
    }
    // port: JSDocInfo#getMarkers
    pub fn get_markers(&self) -> Vec<Marker> {
        MARKERS
            .get(self)
            .and_then(PropertyValue::as_markers)
            .cloned()
            .unwrap_or_default()
    }
    // port: JSDocInfo#getTemplateTypes
    pub fn get_template_types(&self) -> TypeMap {
        TEMPLATE_TYPE_NAMES
            .get(self)
            .and_then(PropertyValue::as_type_map)
            .cloned()
            .unwrap_or_default()
    }
    // port: JSDocInfo#getTypeTransformations
    pub fn get_type_transformations(&self) -> IndexMap<JsString, NodeId> {
        TYPE_TRANSFORMATIONS
            .get(self)
            .and_then(PropertyValue::as_node_map)
            .cloned()
            .unwrap_or_default()
    }
    // port: JSDocInfo#getSuppressionsAndTheirDescription
    pub fn get_suppressions_and_their_description(&self) -> Suppressions {
        SUPPRESSIONS
            .get(self)
            .and_then(PropertyValue::as_suppressions)
            .cloned()
            .unwrap_or_default()
    }
    // port: JSDocInfo#getImplementedInterfaces
    pub fn get_implemented_interfaces(&self) -> Vec<Arc<JSTypeExpression>> {
        IMPLEMENTED_INTERFACES
            .get(self)
            .and_then(PropertyValue::as_type_list)
            .cloned()
            .unwrap_or_default()
    }
    // port: JSDocInfo#getExtendedInterfaces
    pub fn get_extended_interfaces(&self) -> Vec<Arc<JSTypeExpression>> {
        EXTENDED_INTERFACES
            .get(self)
            .and_then(PropertyValue::as_type_list)
            .cloned()
            .unwrap_or_default()
    }
    // port: JSDocInfo#getParameterCount
    pub fn get_parameter_count(&self) -> i32 {
        PARAMETERS
            .get(self)
            .and_then(PropertyValue::as_type_map)
            .map_or(0, |p| p.len() as i32)
    }
    // port: JSDocInfo#getImplementedInterfaceCount
    pub fn get_implemented_interface_count(&self) -> i32 {
        IMPLEMENTED_INTERFACES
            .get(self)
            .and_then(PropertyValue::as_type_list)
            .map_or(0, |p| p.len() as i32)
    }
    // port: JSDocInfo#getExtendedInterfacesCount
    pub fn get_extended_interfaces_count(&self) -> i32 {
        EXTENDED_INTERFACES
            .get(self)
            .and_then(PropertyValue::as_type_list)
            .map_or(0, |p| p.len() as i32)
    }
    // port: JSDocInfo#getParameterType
    pub fn get_parameter_type(
        &self,
        parameter: impl Into<JsString>,
    ) -> Option<Arc<JSTypeExpression>> {
        PARAMETERS
            .get(self)
            .and_then(PropertyValue::as_type_map)
            .and_then(|p| p.get(&parameter.into()))
            .cloned()
            .flatten()
    }
    // port: JSDocInfo#hasParameter
    pub fn has_parameter(&self, parameter: impl Into<JsString>) -> bool {
        PARAMETERS
            .get(self)
            .and_then(PropertyValue::as_type_map)
            .is_some_and(|p| p.contains_key(&parameter.into()))
    }
    // port: JSDocInfo#hasParameterType
    pub fn has_parameter_type(&self, parameter: impl Into<JsString>) -> bool {
        self.get_parameter_type(parameter).is_some()
    }
    // port: JSDocInfo#getParameterNameAt
    pub fn get_parameter_name_at(&self, index: i32) -> Option<JsString> {
        let p = PARAMETERS.get(self).and_then(PropertyValue::as_type_map)?;
        if index >= p.len() as i32 {
            return None;
        }
        check_argument!(index >= 0);
        p.get_index(index as usize).map(|(k, _)| k.clone())
    }
    // port: JSDocInfo#hasDescriptionForParameter
    pub fn has_description_for_parameter(&self, name: impl Into<JsString>) -> bool {
        PARAMETER_DESCRIPTIONS
            .get(self)
            .and_then(PropertyValue::as_str_map)
            .is_some_and(|p| p.contains_key(&name.into()))
    }
    // port: JSDocInfo#getDescriptionForParameter
    pub fn get_description_for_parameter(&self, name: impl Into<JsString>) -> Option<JsString> {
        PARAMETER_DESCRIPTIONS
            .get(self)
            .and_then(PropertyValue::as_str_map)
            .and_then(|p| p.get(&name.into()))
            .cloned()
    }
}
enum EntryValue {
    TypeExpr(Option<Arc<JSTypeExpression>>),
    Str(JsString),
    Node(NodeId),
}
#[derive(Debug, Default)]
pub struct Builder {
    props: BTreeMap<u32, PropertyValue>,
    bits: u64,
    populated: bool,
    current_marker: Option<usize>,
    license_texts: Option<IndexSet<JsString>>,
}
impl Builder {
    // port: JSDocInfo.Builder#copyFrom
    pub fn copy_from(info: &JSDocInfo) -> Self {
        info.to_builder()
    }
    // port: JSDocInfo.Builder#maybeCopyFrom
    pub fn maybe_copy_from(info: Option<&JSDocInfo>) -> Self {
        info.map_or_else(
            || {
                let mut b = JSDocInfo::builder();
                b.parse_documentation();
                b
            },
            JSDocInfo::to_builder,
        )
    }
    // port: JSDocInfo.Builder#maybeCopyFromWithNewType
    pub fn maybe_copy_from_with_new_type(
        info: Option<&JSDocInfo>,
        type_expression: Option<Arc<JSTypeExpression>>,
    ) -> Self {
        let mut b = Self::maybe_copy_from(info);
        b.set_type(type_expression);
        b
    }
    // port: JSDocInfo.Builder#copyFromWithNewType
    pub fn copy_from_with_new_type(
        info: &JSDocInfo,
        type_expression: Option<Arc<JSTypeExpression>>,
    ) -> Self {
        let mut b = info.to_builder();
        b.set_type(type_expression);
        b
    }
    // port: JSDocInfo.Builder#maybeCopyFromAndReplaceNames
    pub fn maybe_copy_from_and_replace_names(
        ast: &mut Ast,
        info: Option<&JSDocInfo>,
        names: &IndexSet<JsString>,
    ) -> Self {
        info.map_or_else(
            || Self::maybe_copy_from(None),
            |info| Self::copy_from_and_replace_names(ast, info, names),
        )
    }
    // port: JSDocInfo.Builder#copyFromAndReplaceNames
    pub fn copy_from_and_replace_names(
        ast: &mut Ast,
        info: &JSDocInfo,
        names: &IndexSet<JsString>,
    ) -> Self {
        info.clone_and_replace_type_names(ast, names).to_builder()
    }
    // port: JSDocInfo.Builder#parseDocumentation
    pub fn parse_documentation(&mut self) -> &mut Self {
        self.set_bit(Bit::INCLUDE_DOCUMENTATION, true);
        self
    }
    // port: JSDocInfo.Builder#recordOriginalCommentString
    pub fn record_original_comment_string(&mut self, source_comment: impl Into<JsString>) {
        if self.should_parse_documentation() {
            self.populated = true;
            self.set_prop(SOURCE_COMMENT, PropertyValue::Str(source_comment.into()));
        }
    }
    // port: JSDocInfo.Builder#recordOriginalCommentPosition
    pub fn record_original_comment_position(&mut self, position: i32) {
        if self.should_parse_documentation() {
            self.populated = true;
            self.set_prop(ORIGINAL_COMMENT_POSITION, PropertyValue::Int(position));
        }
    }
    // port: JSDocInfo.Builder#build
    pub fn build(&mut self) -> Option<Arc<JSDocInfo>> {
        self.build_with_always(false)
    }
    // port: JSDocInfo.Builder#build(boolean)
    pub fn build_with_always(&mut self, always: bool) -> Option<Arc<JSDocInfo>> {
        // Copy collections: Closure does not mutate shared collections after build/toBuilder.
        if self.populated || always {
            let info = JSDocInfo::new(self.bits, &self.props);
            self.populated = false;
            Some(Arc::new(info))
        } else {
            None
        }
    }
    // port: JSDocInfo.Builder#buildAndReset
    pub fn build_and_reset(&mut self) -> Option<Arc<JSDocInfo>> {
        let info = self.build();
        self.bits &= Bit::INCLUDE_DOCUMENTATION.mask();
        self.props.clear();
        self.populated = false;
        info
    }
    // port: JSDocInfo.Builder#markAnnotation
    pub fn mark_annotation(&mut self, annotation: impl Into<JsString>, lineno: i32, charno: i32) {
        let annotation = annotation.into();
        let marker = self.add_marker();
        if let Some(i) = marker {
            let mut position = TrimmedStringPosition::default();
            position.set_item(annotation.clone());
            position.set_position_information(
                lineno,
                charno,
                lineno,
                charno + annotation.length() as i32,
            );
            self.marker_mut(i).set_annotation(position);
            self.populated = true;
        }
        self.current_marker = marker;
    }
    // port: JSDocInfo.Builder#addMarker
    fn add_marker(&mut self) -> Option<usize> {
        if self.should_parse_documentation() {
            let v = self.get_prop_with_default(MARKERS, || PropertyValue::Markers(vec![]));
            if let PropertyValue::Markers(v) = v {
                v.push(Marker::default());
                return Some(v.len() - 1);
            }
        }
        None
    }
    fn marker_mut(&mut self, i: usize) -> &mut Marker {
        if let PropertyValue::Markers(v) = self.props.get_mut(&MARKERS.bit).unwrap() {
            &mut v[i]
        } else {
            unreachable!()
        }
    }
    // port: JSDocInfo.Builder#markText
    pub fn mark_text(
        &mut self,
        text: impl Into<JsString>,
        start_lineno: i32,
        start_charno: i32,
        end_lineno: i32,
        end_charno: i32,
    ) {
        if let Some(i) = self.current_marker {
            let mut p = StringPosition::default();
            p.set_item(text);
            p.set_position_information(start_lineno, start_charno, end_lineno, end_charno);
            self.marker_mut(i).set_description(p);
        }
    }
    // port: JSDocInfo.Builder#markTypeNode
    pub fn mark_type_node(
        &mut self,
        type_node: Option<NodeId>,
        lineno: i32,
        start_charno: i32,
        end_lineno: i32,
        end_charno: i32,
        has_lc: bool,
    ) {
        if let Some(i) = self.current_marker {
            let mut p = TypePosition::default();
            p.set_item(type_node);
            p.set_has_brackets(has_lc);
            p.set_position_information(lineno, start_charno, end_lineno, end_charno);
            self.marker_mut(i).set_type(p);
        }
    }
    // port: JSDocInfo.Builder#markName
    pub fn mark_name(
        &mut self,
        ast: &mut Ast,
        name: impl Into<JsString>,
        template_node: Option<NodeId>,
        lineno: i32,
        charno: i32,
    ) {
        if let Some(i) = self.current_marker {
            let name = name.into();
            let len = name.length() as i32;
            let mut p = TrimmedStringPosition::default();
            p.set_item(name.clone());
            p.set_position_information(lineno, charno, lineno, charno + len);
            let mut node_pos = NamePosition::default();
            let node = ast.new_string_with_token(Token::NAME, name);
            node.set_lineno_charno(ast, lineno, charno);
            node.set_length(ast, len);
            if let Some(n) = template_node {
                node.set_static_source_file_from(ast, n);
            }
            node_pos.set_item(node);
            node_pos.set_position_information(lineno, charno, lineno, charno + len);
            self.marker_mut(i).set_name_node(node_pos);
        }
    }
    // port: JSDocInfo.Builder#recordVisibility
    pub fn record_visibility(&mut self, visibility: Visibility) -> bool {
        if self.get_prop(VISIBILITY).is_none() {
            self.populated = true;
            self.set_prop(VISIBILITY, PropertyValue::Visibility(visibility));
            true
        } else {
            false
        }
    }
    // port: JSDocInfo.Builder#overwriteVisibility
    pub fn overwrite_visibility(&mut self, visibility: Visibility) {
        self.populated = true;
        self.set_prop(VISIBILITY, PropertyValue::Visibility(visibility));
    }
    // port: JSDocInfo.Builder#recordParameter
    pub fn record_parameter(
        &mut self,
        parameter_name: impl Into<JsString>,
        r#type: Option<Arc<JSTypeExpression>>,
    ) -> bool {
        !self.has_any_singleton_type_tags()
            && self.populate_prop_entry(
                PARAMETERS,
                RhinoStringPool::add_or_get(parameter_name),
                EntryValue::TypeExpr(r#type),
            )
    }
    // port: JSDocInfo.Builder#recordParameterDescription
    pub fn record_parameter_description(
        &mut self,
        parameter_name: impl Into<JsString>,
        description: impl Into<JsString>,
    ) -> bool {
        if !self.should_parse_documentation() {
            return true;
        }
        self.populate_prop_entry(
            PARAMETER_DESCRIPTIONS,
            RhinoStringPool::add_or_get(parameter_name),
            EntryValue::Str(description.into()),
        )
    }
    // port: JSDocInfo.Builder#recordTemplateTypeName
    pub fn record_template_type_name(&mut self, ast: &mut Ast, name: impl Into<JsString>) -> bool {
        self.record_template_type_name_with_bound(ast, name, None)
    }
    // port: JSDocInfo.Builder#recordTemplateTypeName(String, JSTypeExpression)
    pub fn record_template_type_name_with_bound(
        &mut self,
        ast: &mut Ast,
        name: impl Into<JsString>,
        bound: Option<Arc<JSTypeExpression>>,
    ) -> bool {
        let bound = bound.unwrap_or_else(|| JSTypeExpression::implicit_template_bound(ast));
        let name = name.into();
        if self
            .get_prop(TYPE_TRANSFORMATIONS)
            .and_then(PropertyValue::as_node_map)
            .is_some_and(|t| t.contains_key(&name))
            || self.props.contains_key(&TYPEDEF_TYPE.bit)
        {
            return false;
        }
        self.populate_prop_entry(
            TEMPLATE_TYPE_NAMES,
            RhinoStringPool::add_or_get(name),
            EntryValue::TypeExpr(Some(bound)),
        )
    }
    // port: JSDocInfo.Builder#recordTypeTransformation
    pub fn record_type_transformation(&mut self, name: impl Into<JsString>, expr: NodeId) -> bool {
        let name = name.into();
        if self
            .get_prop(TEMPLATE_TYPE_NAMES)
            .and_then(PropertyValue::as_type_map)
            .is_some_and(|t| t.contains_key(&name))
        {
            return false;
        }
        self.populate_prop_entry(
            TYPE_TRANSFORMATIONS,
            RhinoStringPool::add_or_get(name),
            EntryValue::Node(expr),
        )
    }
    // port: JSDocInfo.Builder#recordThrowsAnnotation
    pub fn record_throws_annotation(&mut self, annotation: impl Into<JsString>) -> bool {
        self.populated = true;
        if self.check_bit(Bit::NOSIDEEFFECTS) {
            return false;
        }
        if !self.has_any_singleton_type_tags() {
            let doc = self.should_parse_documentation();
            let PropertyValue::StrList(v) =
                self.get_prop_with_default(THROWS_ANNOTATIONS, || PropertyValue::StrList(vec![]))
            else {
                unreachable!()
            };
            if doc {
                v.push(annotation.into());
            } else if v.is_empty() {
                v.push("".into());
            }
        }
        true
    }
    // port: JSDocInfo.Builder#recordSuppressions(ImmutableSet, String)
    pub fn record_suppressions_with_description(
        &mut self,
        suppressions: &IndexSet<JsString>,
        description: impl Into<JsString>,
    ) {
        self.populated = true;
        let PropertyValue::Suppressions(v) =
            self.get_prop_with_default(SUPPRESSIONS, || PropertyValue::Suppressions(vec![]))
        else {
            unreachable!()
        };
        if v.iter().any(|(s, _)| s == suppressions) {
            return;
        }
        v.push((intern_set(suppressions), description.into()));
    }
    // port: JSDocInfo.Builder#recordSuppressions(Set)
    pub fn record_suppressions(&mut self, suppressions: &IndexSet<JsString>) {
        self.record_suppressions_with_description(suppressions, "");
    }
    // port: JSDocInfo.Builder#recordSuppression
    pub fn record_suppression(&mut self, suppression: impl Into<JsString>) {
        self.record_suppressions(&IndexSet::from([suppression.into()]));
    }
    // port: JSDocInfo.Builder#recordModifies
    pub fn record_modifies(&mut self, modifies: &IndexSet<JsString>) -> bool {
        !self.has_any_singleton_side_effect_tags()
            && self.populate_prop(MODIFIES, PropertyValue::StrSet(intern_set(modifies)))
    }
    // port: JSDocInfo.Builder#recordDefineType
    pub fn record_define_type(&mut self, r#type: Option<Arc<JSTypeExpression>>) -> bool {
        if r#type.is_some()
            && !self.check_bit(Bit::CONST)
            && !self.check_bit(Bit::DEFINE)
            && self.record_type(r#type)
        {
            return self.populate_bit(Bit::DEFINE, true);
        }
        false
    }
    // port: JSDocInfo.Builder#recordEnumParameterType
    pub fn record_enum_parameter_type(&mut self, r#type: Option<Arc<JSTypeExpression>>) -> bool {
        if r#type.is_some() && !self.has_any_type_related_tags() {
            self.set_prop(ENUM_PARAMETER_TYPE, PropertyValue::TypeExpr(r#type));
            self.populated = true;
            true
        } else {
            false
        }
    }
    // port: JSDocInfo.Builder#changeBaseType
    pub fn change_base_type(&mut self, r#type: Option<Arc<JSTypeExpression>>) -> bool {
        if r#type.is_some() && !self.has_any_singleton_type_tags() {
            self.set_prop(BASE_TYPE, PropertyValue::TypeExpr(r#type));
            self.populated = true;
            true
        } else {
            false
        }
    }
    // port: JSDocInfo.Builder#addLicense
    pub fn add_license(&mut self, license: impl Into<JsString>) -> bool {
        let license = license.into();
        if !self
            .license_texts
            .get_or_insert_with(IndexSet::new)
            .insert(license.clone())
        {
            return false;
        }
        let txt = self
            .get_prop(LICENSE)
            .and_then(PropertyValue::as_str)
            .cloned()
            .unwrap_or_default();
        self.record_license(RhinoStringPool::add_or_get(txt.concat(&license)))
    }
    // port: JSDocInfo.Builder#hasParameter
    pub fn has_parameter(&self, name: impl Into<JsString>) -> bool {
        self.get_prop(PARAMETERS)
            .and_then(PropertyValue::as_type_map)
            .is_some_and(|p| p.contains_key(&name.into()))
    }
    // port: JSDocInfo.Builder#recordImplementedInterface
    pub fn record_implemented_interface(
        &mut self,
        ast: &Ast,
        interface_type: Option<Arc<JSTypeExpression>>,
    ) -> bool {
        interface_type.is_some_and(|t| self.add_unique(ast, IMPLEMENTED_INTERFACES, t))
    }
    // port: JSDocInfo.Builder#recordExtendedInterface
    pub fn record_extended_interface(
        &mut self,
        ast: &Ast,
        interface_type: Option<Arc<JSTypeExpression>>,
    ) -> bool {
        interface_type.is_some_and(|t| self.add_unique(ast, EXTENDED_INTERFACES, t))
    }
    // port: JSDocInfo.Builder#addUnique
    fn add_unique(&mut self, ast: &Ast, prop: Property, elem: Arc<JSTypeExpression>) -> bool {
        let PropertyValue::TypeList(list) =
            self.get_prop_with_default(prop, || PropertyValue::TypeList(vec![]))
        else {
            unreachable!()
        };
        if list.iter().any(|t| elem.is_equivalent_to(ast, Some(t))) {
            false
        } else {
            list.push(elem);
            self.populated = true;
            true
        }
    }
    // port: JSDocInfo.Builder#recordClosureUnawareCode(PerFileClosureUnawareMode)
    pub fn record_closure_unaware_code_with_mode(
        &mut self,
        mode: PerFileClosureUnawareMode,
    ) -> bool {
        self.populate_prop(
            CLOSURE_UNAWARE_CONFIG_VALUE,
            PropertyValue::ClosureUnawareMode(mode),
        )
    }
    // port: JSDocInfo.Builder#removeClosureUnawareCode
    pub fn remove_closure_unaware_code(&mut self) -> bool {
        self.props
            .remove(&CLOSURE_UNAWARE_CONFIG_VALUE.bit)
            .is_some()
    }
    // port: JSDocInfo.Builder#setType
    pub fn set_type(&mut self, r#type: Option<Arc<JSTypeExpression>>) -> &mut Self {
        self.props.remove(&RETURN_TYPE.bit);
        self.props.remove(&ENUM_PARAMETER_TYPE.bit);
        self.props.remove(&TYPEDEF_TYPE.bit);
        self.set_prop(TYPE, PropertyValue::TypeExpr(r#type));
        self
    }
    // port: JSDocInfo.Builder#hasAnyParameters
    fn has_any_parameters(&self) -> bool {
        self.get_prop(PARAMETERS)
            .and_then(PropertyValue::as_type_map)
            .is_some_and(|p| !p.is_empty())
    }
    // port: JSDocInfo.Builder#isPropEmpty
    fn is_prop_empty(&self, prop: Property) -> bool {
        self.get_prop(prop).is_none_or(PropertyValue::is_default)
    }
    // port: JSDocInfo.Builder#setProp
    fn set_prop(&mut self, prop: Property, value: PropertyValue) {
        self.props.insert(prop.bit, value);
    }
    // port: JSDocInfo.Builder#getProp
    fn get_prop(&self, prop: Property) -> Option<&PropertyValue> {
        self.props.get(&prop.bit).filter(|v| !v.is_null())
    }
    // port: JSDocInfo.Builder#getPropWithDefault
    fn get_prop_with_default(
        &mut self,
        prop: Property,
        supplier: impl FnOnce() -> PropertyValue,
    ) -> &mut PropertyValue {
        let v = self
            .props
            .entry(prop.bit)
            .or_insert_with(|| PropertyValue::Null);
        if v.is_null() {
            *v = supplier();
        }
        v
    }
    // port: JSDocInfo.Builder#putPropEntry
    fn put_prop_entry(&mut self, prop: Property, key: JsString, value: EntryValue) -> bool {
        let default = || match prop.kind {
            PropertyKind::TypeMap => PropertyValue::TypeMap(IndexMap::new()),
            PropertyKind::NodeMap => PropertyValue::NodeMap(IndexMap::new()),
            _ => PropertyValue::StrMap(IndexMap::new()),
        };
        match (self.get_prop_with_default(prop, default), value) {
            (PropertyValue::TypeMap(m), EntryValue::TypeExpr(v)) => {
                if m.get(&key).is_some_and(Option::is_some) {
                    false
                } else {
                    m.insert(key, v);
                    true
                }
            }
            (PropertyValue::StrMap(m), EntryValue::Str(v)) => {
                if m.contains_key(&key) {
                    false
                } else {
                    m.insert(key, v);
                    true
                }
            }
            (PropertyValue::NodeMap(m), EntryValue::Node(v)) => {
                if m.contains_key(&key) {
                    false
                } else {
                    m.insert(key, v);
                    true
                }
            }
            _ => unreachable!(),
        }
    }
    // port: JSDocInfo.Builder#populatePropEntry
    fn populate_prop_entry(&mut self, prop: Property, key: JsString, value: EntryValue) -> bool {
        if self.put_prop_entry(prop, key, value) {
            self.populated = true;
            true
        } else {
            false
        }
    }
    // port: JSDocInfo.Builder#populateProp
    fn populate_prop(&mut self, prop: Property, value: PropertyValue) -> bool {
        self.populated = true;
        if self.get_prop(prop).is_some() {
            false
        } else {
            self.set_prop(prop, value);
            true
        }
    }
    // port: JSDocInfo.Builder#checkBit
    fn check_bit(&self, bit: Bit) -> bool {
        self.bits & bit.mask() != 0
    }
    // port: JSDocInfo.Builder#setBit
    fn set_bit(&mut self, bit: Bit, value: bool) {
        if value {
            self.bits |= bit.mask();
        } else {
            self.bits &= !bit.mask();
        }
    }
    // port: JSDocInfo.Builder#populateBit
    fn populate_bit(&mut self, bit: Bit, value: bool) -> bool {
        if self.check_bit(bit) != value {
            self.set_bit(bit, value);
            self.populated = true;
            true
        } else {
            false
        }
    }
    // port: JSDocInfo.Builder#recordType
    pub fn record_type(&mut self, r#type: Option<Arc<JSTypeExpression>>) -> bool {
        r#type.is_some()
            && !self.has_any_type_related_tags()
            && self.populate_prop(TYPE, PropertyValue::TypeExpr(r#type))
    }
    // port: JSDocInfo.Builder#recordTypedef
    pub fn record_typedef(&mut self, r#type: Option<Arc<JSTypeExpression>>) -> bool {
        r#type.is_some()
            && !self.has_any_type_related_tags()
            && self.get_prop(TEMPLATE_TYPE_NAMES).is_none()
            && self.populate_prop(TYPEDEF_TYPE, PropertyValue::TypeExpr(r#type))
    }
    // port: JSDocInfo.Builder#recordReturnType
    pub fn record_return_type(&mut self, r#type: Option<Arc<JSTypeExpression>>) -> bool {
        r#type.is_some()
            && !self.has_any_singleton_type_tags()
            && self.populate_prop(RETURN_TYPE, PropertyValue::TypeExpr(r#type))
    }
    // port: JSDocInfo.Builder#recordThisType
    pub fn record_this_type(&mut self, r#type: Option<Arc<JSTypeExpression>>) -> bool {
        r#type.is_some()
            && !self.has_any_singleton_type_tags()
            && self.populate_prop(THIS_TYPE, PropertyValue::TypeExpr(r#type))
    }
    // port: JSDocInfo.Builder#recordBaseType
    pub fn record_base_type(&mut self, r#type: Option<Arc<JSTypeExpression>>) -> bool {
        r#type.is_some()
            && !self.has_any_singleton_type_tags()
            && self.populate_prop(BASE_TYPE, PropertyValue::TypeExpr(r#type))
    }
    // port: JSDocInfo.Builder#recordLends
    pub fn record_lends(&mut self, r#type: Option<Arc<JSTypeExpression>>) -> bool {
        !self.has_any_type_related_tags()
            && self.populate_prop(LENDS_NAME, PropertyValue::TypeExpr(r#type))
    }
    // port: JSDocInfo.Builder#recordBlockDescription
    pub fn record_block_description(&mut self, value: impl Into<JsString>) -> bool {
        self.populated = true;
        if !self.should_parse_documentation() {
            return true;
        }
        self.populate_prop(BLOCK_DESCRIPTION, PropertyValue::Str(value.into()))
    }
    // port: JSDocInfo.Builder#recordAuthor
    pub fn record_author(&mut self, value: impl Into<JsString>) -> bool {
        self.populated = true;
        if self.should_parse_documentation() {
            let PropertyValue::StrList(v) =
                self.get_prop_with_default(AUTHORS, || PropertyValue::StrList(vec![]))
            else {
                unreachable!()
            };
            v.push(value.into());
        }
        true
    }
    // port: JSDocInfo.Builder#recordReference
    pub fn record_reference(&mut self, value: impl Into<JsString>) -> bool {
        self.populated = true;
        if self.should_parse_documentation() {
            let PropertyValue::StrList(v) =
                self.get_prop_with_default(SEES, || PropertyValue::StrList(vec![]))
            else {
                unreachable!()
            };
            v.push(value.into());
        }
        true
    }
    // port: JSDocInfo.Builder#recordDeprecationReason
    pub fn record_deprecation_reason(&mut self, value: impl Into<JsString>) -> bool {
        self.populate_prop(DEPRECATION_REASON, PropertyValue::Str(value.into()))
    }
    // port: JSDocInfo.Builder#recordReturnDescription
    pub fn record_return_description(&mut self, value: impl Into<JsString>) -> bool {
        if !self.should_parse_documentation() {
            return true;
        }
        self.populate_prop(RETURN_DESCRIPTION, PropertyValue::Str(value.into()))
    }
    // port: JSDocInfo.Builder#recordDescription
    pub fn record_description(&mut self, value: impl Into<JsString>) -> bool {
        self.populate_prop(DESCRIPTION, PropertyValue::Str(value.into()))
    }
    // port: JSDocInfo.Builder#recordTsType
    pub fn record_ts_type(&mut self, value: impl Into<JsString>) {
        self.populated = true;
        let PropertyValue::StrList(v) =
            self.get_prop_with_default(TS_TYPES, || PropertyValue::StrList(vec![]))
        else {
            unreachable!()
        };
        v.push(value.into());
    }
    // port: JSDocInfo.Builder#recordMeaning
    pub fn record_meaning(&mut self, value: impl Into<JsString>) -> bool {
        self.populate_prop(MEANING, PropertyValue::Str(value.into()))
    }
    // port: JSDocInfo.Builder#recordAlternateMessageId
    pub fn record_alternate_message_id(&mut self, value: impl Into<JsString>) -> bool {
        self.populate_prop(ALTERNATE_MESSAGE_ID, PropertyValue::Str(value.into()))
    }
    // port: JSDocInfo.Builder#recordClosurePrimitiveId
    pub fn record_closure_primitive_id(&mut self, value: impl Into<JsString>) -> bool {
        self.populate_prop(
            CLOSURE_PRIMITIVE_ID,
            PropertyValue::Str(RhinoStringPool::add_or_get(value)),
        )
    }
    // port: JSDocInfo.Builder#recordFileOverview
    pub fn record_file_overview(&mut self, value: impl Into<JsString>) -> bool {
        self.populated = true;
        self.set_bit(Bit::FILEOVERVIEW, true);
        if !self.should_parse_documentation() {
            return true;
        }
        self.populate_prop(FILEOVERVIEW_DESCRIPTION, PropertyValue::Str(value.into()))
    }
    // port: JSDocInfo.Builder#recordEnhance
    pub fn record_enhance(&mut self, value: impl Into<JsString>) -> bool {
        self.populated = true;
        self.set_bit(Bit::ENHANCED_NAMESPACE, true);
        self.populate_prop(ENHANCED_NAMESPACE, PropertyValue::Str(value.into()))
    }
    // port: JSDocInfo.Builder#recordMods
    pub fn record_mods(&mut self, value: impl Into<JsString>) -> bool {
        self.populate_prop(MODS, PropertyValue::Str(RhinoStringPool::add_or_get(value)))
    }
    // port: JSDocInfo.Builder#recordLicense
    pub fn record_license(&mut self, value: impl Into<JsString>) -> bool {
        self.populated = true;
        self.set_prop(
            LICENSE,
            PropertyValue::Str(RhinoStringPool::add_or_get(value)),
        );
        true
    }
    // port: JSDocInfo.Builder#shouldParseDocumentation
    pub fn should_parse_documentation(&self) -> bool {
        self.check_bit(Bit::INCLUDE_DOCUMENTATION)
    }
    // port: JSDocInfo.Builder#isPopulatedWithFileOverview
    pub fn is_populated_with_file_overview(&self) -> bool {
        self.populated
            && ((self.bits
                & (Bit::FILEOVERVIEW.mask()
                    | Bit::EXTERNS.mask()
                    | Bit::NOCOMPILE.mask()
                    | Bit::NOCOVERAGE.mask()
                    | Bit::TYPE_SUMMARY.mask()
                    | Bit::ENHANCED_NAMESPACE.mask()))
                != 0
                || self.is_mods_recorded())
    }
    // port: JSDocInfo.Builder#isDescriptionRecorded
    pub fn is_description_recorded(&self) -> bool {
        self.get_prop(DESCRIPTION).is_some()
    }
    // port: JSDocInfo.Builder#isNoInline
    pub fn is_no_inline(&self) -> bool {
        self.check_bit(Bit::NOINLINE)
    }
    // port: JSDocInfo.Builder#isRequireInlining
    pub fn is_require_inlining(&self) -> bool {
        self.check_bit(Bit::REQUIRE_INLINING)
    }
    // port: JSDocInfo.Builder#isEncourageInlining
    pub fn is_encourage_inlining(&self) -> bool {
        self.check_bit(Bit::ENCOURAGE_INLINING)
    }
    // port: JSDocInfo.Builder#recordConsistentIdGenerator
    pub fn record_consistent_id_generator(&mut self) -> bool {
        self.populate_prop(
            ID_GENERATOR,
            PropertyValue::IdGenerator(IdGenerator::CONSISTENT),
        )
    }
    // port: JSDocInfo.Builder#recordStableIdGenerator
    pub fn record_stable_id_generator(&mut self) -> bool {
        self.populate_prop(
            ID_GENERATOR,
            PropertyValue::IdGenerator(IdGenerator::STABLE),
        )
    }
    // port: JSDocInfo.Builder#recordXidGenerator
    pub fn record_xid_generator(&mut self) -> bool {
        self.populate_prop(ID_GENERATOR, PropertyValue::IdGenerator(IdGenerator::XID))
    }
    // port: JSDocInfo.Builder#recordMappedIdGenerator
    pub fn record_mapped_id_generator(&mut self) -> bool {
        self.populate_prop(
            ID_GENERATOR,
            PropertyValue::IdGenerator(IdGenerator::MAPPED),
        )
    }
    // port: JSDocInfo.Builder#recordIdGenerator
    pub fn record_id_generator(&mut self) -> bool {
        self.populate_prop(
            ID_GENERATOR,
            PropertyValue::IdGenerator(IdGenerator::UNIQUE),
        )
    }
    // port: JSDocInfo.Builder#isDeprecationReasonRecorded
    pub fn is_deprecation_reason_recorded(&self) -> bool {
        self.get_prop(DEPRECATION_REASON).is_some()
    }
    // port: JSDocInfo.Builder#recordInlineType
    pub fn record_inline_type(&mut self) {
        self.populate_bit(Bit::INLINE_TYPE, true);
    }
    // port: JSDocInfo.Builder#recordConstancy
    pub fn record_constancy(&mut self) -> bool {
        self.populate_bit(Bit::CONST, true)
    }
    // port: JSDocInfo.Builder#recordMutable
    pub fn record_mutable(&mut self) -> bool {
        self.populate_bit(Bit::CONST, false)
    }
    // port: JSDocInfo.Builder#recordFinality
    pub fn record_finality(&mut self) -> bool {
        self.populate_bit(Bit::FINAL, true)
    }
    // port: JSDocInfo.Builder#isModsRecorded
    pub fn is_mods_recorded(&self) -> bool {
        self.get_prop(MODS).is_some()
    }
    // port: JSDocInfo.Builder#recordNoCompile
    pub fn record_no_compile(&mut self) -> bool {
        self.populate_bit(Bit::NOCOMPILE, true)
    }
    // port: JSDocInfo.Builder#recordNoDts
    pub fn record_no_dts(&mut self) -> bool {
        self.populate_bit(Bit::NODTS, true)
    }
    // port: JSDocInfo.Builder#recordNoCollapse
    pub fn record_no_collapse(&mut self) -> bool {
        self.populate_bit(Bit::NOCOLLAPSE, true)
    }
    // port: JSDocInfo.Builder#recordNoInline
    pub fn record_no_inline(&mut self) -> bool {
        self.populate_bit(Bit::NOINLINE, true)
    }
    // port: JSDocInfo.Builder#recordEncourageInlining
    pub fn record_encourage_inlining(&mut self) -> bool {
        self.populate_bit(Bit::ENCOURAGE_INLINING, true)
    }
    // port: JSDocInfo.Builder#recordRequireInlining
    pub fn record_require_inlining(&mut self) -> bool {
        self.populate_bit(Bit::REQUIRE_INLINING, true)
    }
    // port: JSDocInfo.Builder#recordPureOrBreakMyCode
    pub fn record_pure_or_break_my_code(&mut self) -> bool {
        self.populate_bit(Bit::PURE_OR_BREAK_MY_CODE, true)
    }
    // port: JSDocInfo.Builder#recordCollapsibleOrBreakMyCode
    pub fn record_collapsible_or_break_my_code(&mut self) -> bool {
        self.populate_bit(Bit::COLLAPSIBLE_OR_BREAK_MY_CODE, true)
    }
    // port: JSDocInfo.Builder#recordConstructor
    pub fn record_constructor(&mut self) -> bool {
        !self.has_any_singleton_type_tags()
            && !self.is_constructor_or_interface()
            && self.populate_bit(Bit::CONSTRUCTOR, true)
    }
    // port: JSDocInfo.Builder#recordImplicitMatch
    pub fn record_implicit_match(&mut self) -> bool {
        !self.has_any_singleton_type_tags()
            && !self.is_constructor_or_interface()
            && self.populate_bit(Bit::RECORD, true)
            && self.populate_bit(Bit::INTERFACE, true)
    }
    // port: JSDocInfo.Builder#recordProvideGoog
    pub fn record_provide_goog(&mut self) -> bool {
        self.populate_bit(Bit::PROVIDE_GOOG, true)
    }
    // port: JSDocInfo.Builder#recordProvideAlreadyProvided
    pub fn record_provide_already_provided(&mut self) -> bool {
        self.populate_bit(Bit::PROVIDE_ALREADY_PROVIDED, true)
    }
    // port: JSDocInfo.Builder#isConstructorRecorded
    pub fn is_constructor_recorded(&self) -> bool {
        self.check_bit(Bit::CONSTRUCTOR)
    }
    // port: JSDocInfo.Builder#recordUnrestricted
    pub fn record_unrestricted(&mut self) -> bool {
        !self.has_any_singleton_type_tags()
            && ((self.bits & (Bit::INTERFACE.mask() | Bit::DICT.mask() | Bit::STRUCT.mask())) == 0)
            && self.populate_bit(Bit::UNRESTRICTED, true)
    }
    // port: JSDocInfo.Builder#isUnrestrictedRecorded
    pub fn is_unrestricted_recorded(&self) -> bool {
        self.check_bit(Bit::UNRESTRICTED)
    }
    // port: JSDocInfo.Builder#recordAbstract
    pub fn record_abstract(&mut self) -> bool {
        !self.has_any_singleton_type_tags()
            && ((self.bits & (Bit::INTERFACE.mask() | Bit::FINAL.mask())) == 0)
            && self
                .get_prop(VISIBILITY)
                .and_then(PropertyValue::as_visibility)
                != Some(&Visibility::PRIVATE)
            && self.populate_bit(Bit::ABSTRACT, true)
    }
    // port: JSDocInfo.Builder#recordStruct
    pub fn record_struct(&mut self) -> bool {
        !self.has_any_singleton_type_tags()
            && ((self.bits & (Bit::DICT.mask() | Bit::UNRESTRICTED.mask())) == 0)
            && self.populate_bit(Bit::STRUCT, true)
    }
    // port: JSDocInfo.Builder#isStructRecorded
    pub fn is_struct_recorded(&self) -> bool {
        self.check_bit(Bit::STRUCT)
    }
    // port: JSDocInfo.Builder#recordDict
    pub fn record_dict(&mut self) -> bool {
        !self.has_any_singleton_type_tags()
            && ((self.bits & (Bit::STRUCT.mask() | Bit::UNRESTRICTED.mask())) == 0)
            && self.populate_bit(Bit::DICT, true)
    }
    // port: JSDocInfo.Builder#isDictRecorded
    pub fn is_dict_recorded(&self) -> bool {
        self.check_bit(Bit::DICT)
    }
    // port: JSDocInfo.Builder#recordOverride
    pub fn record_override(&mut self) -> bool {
        self.populate_bit(Bit::OVERRIDE, true)
    }
    // port: JSDocInfo.Builder#recordDeprecated
    pub fn record_deprecated(&mut self) -> bool {
        self.populate_bit(Bit::DEPRECATED, true)
    }
    // port: JSDocInfo.Builder#recordInterface
    pub fn record_interface(&mut self) -> bool {
        !self.has_any_singleton_type_tags()
            && ((self.bits & (Bit::CONSTRUCTOR.mask() | Bit::ABSTRACT.mask())) == 0)
            && self.populate_bit(Bit::INTERFACE, true)
    }
    // port: JSDocInfo.Builder#recordExport
    pub fn record_export(&mut self) -> bool {
        self.populate_bit(Bit::EXPORT, true)
    }
    // port: JSDocInfo.Builder#removeExport
    pub fn remove_export(&mut self) -> bool {
        self.populate_bit(Bit::EXPORT, false)
    }
    // port: JSDocInfo.Builder#recordImplicitCast
    pub fn record_implicit_cast(&mut self) -> bool {
        self.populate_bit(Bit::IMPLICITCAST, true)
    }
    // port: JSDocInfo.Builder#recordNoSideEffects
    pub fn record_no_side_effects(&mut self) -> bool {
        !self.has_any_singleton_side_effect_tags()
            && self.is_prop_empty(THROWS_ANNOTATIONS)
            && self.populate_bit(Bit::NOSIDEEFFECTS, true)
    }
    // port: JSDocInfo.Builder#isNoSideEffectsRecorded
    pub fn is_no_side_effects_recorded(&self) -> bool {
        self.check_bit(Bit::NOSIDEEFFECTS)
    }
    // port: JSDocInfo.Builder#isModifiesRecorded
    pub fn is_modifies_recorded(&self) -> bool {
        !self.is_prop_empty(MODIFIES)
    }
    // port: JSDocInfo.Builder#isThrowsRecorded
    pub fn is_throws_recorded(&self) -> bool {
        !self.is_prop_empty(THROWS_ANNOTATIONS)
    }
    // port: JSDocInfo.Builder#recordExterns
    pub fn record_externs(&mut self) -> bool {
        self.populate_bit(Bit::EXTERNS, true)
    }
    // port: JSDocInfo.Builder#recordNoCoverage
    pub fn record_no_coverage(&mut self) -> bool {
        self.populate_bit(Bit::NOCOVERAGE, true)
    }
    // port: JSDocInfo.Builder#recordTypeSummary
    pub fn record_type_summary(&mut self) -> bool {
        self.populate_bit(Bit::TYPE_SUMMARY, true)
    }
    // port: JSDocInfo.Builder#isInterfaceRecorded
    pub fn is_interface_recorded(&self) -> bool {
        self.check_bit(Bit::INTERFACE)
    }
    // port: JSDocInfo.Builder#isNgInjectRecorded
    pub fn is_ng_inject_recorded(&self) -> bool {
        self.check_bit(Bit::NG_INJECT)
    }
    // port: JSDocInfo.Builder#recordNgInject
    pub fn record_ng_inject(&mut self, _ng_inject: bool) -> bool {
        self.populate_bit(Bit::NG_INJECT, true)
    }
    // port: JSDocInfo.Builder#isWizactionRecorded
    pub fn is_wizaction_recorded(&self) -> bool {
        self.check_bit(Bit::WIZ_ACTION)
    }
    // port: JSDocInfo.Builder#recordWizaction
    pub fn record_wizaction(&mut self) -> bool {
        self.populate_bit(Bit::WIZ_ACTION, true)
    }
    // port: JSDocInfo.Builder#isWizcallbackRecorded
    pub fn is_wizcallback_recorded(&self) -> bool {
        self.check_bit(Bit::WIZ_CALLBACK)
    }
    // port: JSDocInfo.Builder#recordWizcallback
    pub fn record_wizcallback(&mut self) -> bool {
        self.populate_bit(Bit::WIZ_CALLBACK, true)
    }
    // port: JSDocInfo.Builder#isPolymerBehaviorRecorded
    pub fn is_polymer_behavior_recorded(&self) -> bool {
        self.check_bit(Bit::POLYMER_BEHAVIOR)
    }
    // port: JSDocInfo.Builder#recordPolymerBehavior
    pub fn record_polymer_behavior(&mut self) -> bool {
        self.populate_bit(Bit::POLYMER_BEHAVIOR, true)
    }
    // port: JSDocInfo.Builder#isPolymerRecorded
    pub fn is_polymer_recorded(&self) -> bool {
        self.check_bit(Bit::POLYMER)
    }
    // port: JSDocInfo.Builder#recordPolymer
    pub fn record_polymer(&mut self) -> bool {
        self.populate_bit(Bit::POLYMER, true)
    }
    // port: JSDocInfo.Builder#isCustomElementRecorded
    pub fn is_custom_element_recorded(&self) -> bool {
        self.check_bit(Bit::CUSTOM_ELEMENT)
    }
    // port: JSDocInfo.Builder#recordCustomElement
    pub fn record_custom_element(&mut self) -> bool {
        self.populate_bit(Bit::CUSTOM_ELEMENT, true)
    }
    // port: JSDocInfo.Builder#isMixinClassRecorded
    pub fn is_mixin_class_recorded(&self) -> bool {
        self.check_bit(Bit::MIXIN_CLASS)
    }
    // port: JSDocInfo.Builder#recordMixinClass
    pub fn record_mixin_class(&mut self) -> bool {
        self.populate_bit(Bit::MIXIN_CLASS, true)
    }
    // port: JSDocInfo.Builder#isMixinFunctionRecorded
    pub fn is_mixin_function_recorded(&self) -> bool {
        self.check_bit(Bit::MIXIN_FUNCTION)
    }
    // port: JSDocInfo.Builder#recordMixinFunction
    pub fn record_mixin_function(&mut self) -> bool {
        self.populate_bit(Bit::MIXIN_FUNCTION, true)
    }
    // port: JSDocInfo.Builder#isSassGeneratedCssTsRecorded
    pub fn is_sass_generated_css_ts_recorded(&self) -> bool {
        self.check_bit(Bit::SASS_GENERATED_CSS_TS)
    }
    // port: JSDocInfo.Builder#recordSassGeneratedCssTs
    pub fn record_sass_generated_css_ts(&mut self) {
        self.populate_bit(Bit::SASS_GENERATED_CSS_TS, true);
    }
    // port: JSDocInfo.Builder#logTypeInCompiler
    pub fn log_type_in_compiler(&self) -> bool {
        self.check_bit(Bit::LOG_TYPE_IN_COMPILER)
    }
    // port: JSDocInfo.Builder#recordLogTypeInCompiler
    pub fn record_log_type_in_compiler(&mut self) -> bool {
        self.populate_bit(Bit::LOG_TYPE_IN_COMPILER, true)
    }
    // port: JSDocInfo.Builder#isClosureUnawareCode
    pub fn is_closure_unaware_code(&self) -> bool {
        self.get_prop(CLOSURE_UNAWARE_CONFIG_VALUE).is_some()
    }
    // port: JSDocInfo.Builder#recordClosureUnawareCode
    pub fn record_closure_unaware_code(&mut self) -> bool {
        self.populate_prop(
            CLOSURE_UNAWARE_CONFIG_VALUE,
            PropertyValue::ClosureUnawareMode(PerFileClosureUnawareMode::UNSPECIFIED),
        )
    }
    // port: JSDocInfo.Builder#recordUsedViaDotConstructor
    pub fn record_used_via_dot_constructor(&mut self) -> bool {
        self.populate_bit(Bit::USED_VIA_DOT_CONSTRUCTOR, true)
    }
    // port: JSDocInfo.Builder#hasAnyTypeRelatedTags
    pub fn has_any_type_related_tags(&self) -> bool {
        (self.bits & (Bit::CONSTRUCTOR.mask() | Bit::INTERFACE.mask() | Bit::ABSTRACT.mask())) != 0
            || self.has_any_parameters()
            || self.get_prop(RETURN_TYPE).is_some()
            || self.get_prop(BASE_TYPE).is_some()
            || !self.is_prop_empty(EXTENDED_INTERFACES)
            || self.get_prop(LENDS_NAME).is_some()
            || self.get_prop(THIS_TYPE).is_some()
            || self.has_any_singleton_type_tags()
    }
    // port: JSDocInfo.Builder#hasAnySingletonTypeTags
    pub fn has_any_singleton_type_tags(&self) -> bool {
        self.get_prop(TYPE).is_some()
            || self.get_prop(TYPEDEF_TYPE).is_some()
            || self.get_prop(ENUM_PARAMETER_TYPE).is_some()
    }
    // port: JSDocInfo.Builder#hasAnySingletonSideEffectTags
    pub fn has_any_singleton_side_effect_tags(&self) -> bool {
        self.check_bit(Bit::NOSIDEEFFECTS) || !self.is_prop_empty(MODIFIES)
    }
    // port: JSDocInfo.Builder#isConstructorOrInterface
    pub fn is_constructor_or_interface(&self) -> bool {
        (self.bits & (Bit::CONSTRUCTOR.mask() | Bit::INTERFACE.mask())) != 0
    }
}
// port: JSDocInfo#internSet
pub fn intern_set(strings: &IndexSet<JsString>) -> IndexSet<JsString> {
    strings
        .iter()
        .map(|s| RhinoStringPool::add_or_get(s.clone()))
        .collect()
}
