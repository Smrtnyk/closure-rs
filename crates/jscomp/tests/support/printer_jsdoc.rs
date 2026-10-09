/*
 * Copyright 2026 The closure-rs Authors.
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

// Rebuild the complete dumped JSDocInfo through Rhino's public Builder API.
// Every public bean getter is checked after reconstruction, including descriptions,
// markers, ordered maps, type ASTs and the parameterised getters in $parameters.
use super::{js_string, load_node};
use closure_jscomp::source_file::SourceFile;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jsdoc_info::{
        JSDocInfo, Marker, NamePosition, PerFileClosureUnawareMode, StringPosition, TypePosition,
        Visibility,
    },
    node::{Ast, NodeId, Prop},
    source_position::SourcePosition,
};
use serde_json::Value;
use std::sync::Arc;

fn expression(
    ast: &mut Ast,
    value: &Value,
    source: &Arc<SourceFile>,
    numbers: &mut Vec<(NodeId, Option<JsString>)>,
) -> Option<Arc<JSTypeExpression>> {
    if value.is_null() {
        return None;
    }
    let root = load_node(ast, &value["root"], source, numbers);
    Some(Arc::new(JSTypeExpression::new(
        root,
        value["source_name"].as_str().unwrap(),
    )))
}
pub fn load(
    ast: &mut Ast,
    value: &Value,
    source: &Arc<SourceFile>,
    numbers: &mut Vec<(NodeId, Option<JsString>)>,
) -> Arc<JSDocInfo> {
    let mut builder = JSDocInfo::builder();
    if value["$documentation_included"]
        .as_bool()
        .unwrap_or_else(|| {
            !value["getOriginalCommentString"].is_null()
                || !value["getMarkers"].as_array().unwrap().is_empty()
        })
    {
        builder.parse_documentation();
    }
    if value["isFinal"].as_bool().unwrap() {
        builder.record_finality();
    }
    if value["isConstructor"].as_bool().unwrap() {
        builder.record_constructor();
    }
    if value["isInterface"].as_bool().unwrap() && !value["usesImplicitMatch"].as_bool().unwrap() {
        builder.record_interface();
    }
    if value["usesImplicitMatch"].as_bool().unwrap() {
        builder.record_implicit_match();
    }
    if value["isAbstract"].as_bool().unwrap() {
        builder.record_abstract();
    }
    if value["makesDicts"].as_bool().unwrap() {
        builder.record_dict();
    }
    if value["makesStructs"].as_bool().unwrap() {
        builder.record_struct();
    }
    if value["makesUnrestricted"].as_bool().unwrap() {
        builder.record_unrestricted();
    }
    if value["isOverride"].as_bool().unwrap() {
        builder.record_override();
    }
    if value["isDeprecated"].as_bool().unwrap() {
        builder.record_deprecated();
    }
    if value["isExport"].as_bool().unwrap() {
        builder.record_export();
    }
    if value["isImplicitCast"].as_bool().unwrap() {
        builder.record_implicit_cast();
    }
    if value["isNoSideEffects"].as_bool().unwrap() {
        builder.record_no_side_effects();
    }
    if value["isExterns"].as_bool().unwrap() {
        builder.record_externs();
    }
    if value["isTypeSummary"].as_bool().unwrap() {
        builder.record_type_summary();
    }
    if value["isNoCompile"].as_bool().unwrap() {
        builder.record_no_compile();
    }
    if value["isNoDts"].as_bool().unwrap() {
        builder.record_no_dts();
    }
    if value["isNoCollapse"].as_bool().unwrap() {
        builder.record_no_collapse();
    }
    if value["isNoInline"].as_bool().unwrap() {
        builder.record_no_inline();
    }
    if value["isRequireInlining"].as_bool().unwrap() {
        builder.record_require_inlining();
    }
    if value["isEncourageInlining"].as_bool().unwrap() {
        builder.record_encourage_inlining();
    }
    if value["isPureOrBreakMyCode"].as_bool().unwrap() {
        builder.record_pure_or_break_my_code();
    }
    if value["isCollapsibleOrBreakMyCode"].as_bool().unwrap() {
        builder.record_collapsible_or_break_my_code();
    }
    if value["isProvideGoog"].as_bool().unwrap() {
        builder.record_provide_goog();
    }
    if value["isProvideAlreadyProvided"].as_bool().unwrap() {
        builder.record_provide_already_provided();
    }
    if value["isNoCoverage"].as_bool().unwrap() {
        builder.record_no_coverage();
    }
    if value["isNgInject"].as_bool().unwrap() {
        builder.record_ng_inject(true);
    }
    if value["isWizaction"].as_bool().unwrap() {
        builder.record_wizaction();
    }
    if value["isWizcallback"].as_bool().unwrap() {
        builder.record_wizcallback();
    }
    if value["isPolymer"].as_bool().unwrap() {
        builder.record_polymer();
    }
    if value["isPolymerBehavior"].as_bool().unwrap() {
        builder.record_polymer_behavior();
    }
    if value["isCustomElement"].as_bool().unwrap() {
        builder.record_custom_element();
    }
    if value["isMixinClass"].as_bool().unwrap() {
        builder.record_mixin_class();
    }
    if value["isMixinFunction"].as_bool().unwrap() {
        builder.record_mixin_function();
    }
    if value["isSassGeneratedCssTs"].as_bool().unwrap() {
        builder.record_sass_generated_css_ts();
    }
    if value["isClosureUnawareCode"].as_bool().unwrap() {
        builder.record_closure_unaware_code();
    }
    if value["isUsedViaDotConstructor"].as_bool().unwrap() {
        builder.record_used_via_dot_constructor();
    }
    if value["getLogTypeInCompiler"].as_bool().unwrap() {
        builder.record_log_type_in_compiler();
    }
    if value["isInlineType"].as_bool().unwrap() {
        builder.record_inline_type();
    }
    if value["isIdGenerator"].as_bool().unwrap() {
        builder.record_id_generator();
    }
    if value["isConsistentIdGenerator"].as_bool().unwrap() {
        builder.record_consistent_id_generator();
    }
    if value["isStableIdGenerator"].as_bool().unwrap() {
        builder.record_stable_id_generator();
    }
    if value["isXidGenerator"].as_bool().unwrap() {
        builder.record_xid_generator();
    }
    if value["isMappedIdGenerator"].as_bool().unwrap() {
        builder.record_mapped_id_generator();
    }
    let visibility = match value["getVisibility"].as_str().unwrap() {
        "PRIVATE" => Visibility::PRIVATE,
        "PACKAGE" => Visibility::PACKAGE,
        "PROTECTED" => Visibility::PROTECTED,
        "PUBLIC" => Visibility::PUBLIC,
        "INHERITED" => Visibility::INHERITED,
        x => panic!("Unknown visibility {x}"),
    };
    builder.record_visibility(visibility);
    if let Some(mode) = value["getPerFileClosureUnawareMode"].as_str() {
        builder.record_closure_unaware_code_with_mode(match mode {
            "UNSPECIFIED" => PerFileClosureUnawareMode::UNSPECIFIED,
            "SIMPLE" => PerFileClosureUnawareMode::SIMPLE,
            "WHITESPACE" => PerFileClosureUnawareMode::WHITESPACE,
            x => panic!("Unknown closure unaware mode {x}"),
        });
    }
    if !value["getType"].is_null() {
        let expr = expression(ast, &value["getType"], source, numbers);
        if value["isDefine"].as_bool().unwrap() {
            builder.record_define_type(expr);
        } else {
            builder.record_type(expr);
        }
    }
    if !value["getTypedefType"].is_null() {
        let expr = expression(ast, &value["getTypedefType"], source, numbers);
        builder.record_typedef(expr);
    }
    if !value["getEnumParameterType"].is_null() {
        let expr = expression(ast, &value["getEnumParameterType"], source, numbers);
        builder.record_enum_parameter_type(expr);
    }
    if !value["getLendsName"].is_null() {
        let expr = expression(ast, &value["getLendsName"], source, numbers);
        builder.record_lends(expr);
    }
    if !value["getBaseType"].is_null() {
        let expr = expression(ast, &value["getBaseType"], source, numbers);
        builder.record_base_type(expr);
    }
    if !value["getThisType"].is_null() {
        let expr = expression(ast, &value["getThisType"], source, numbers);
        builder.record_this_type(expr);
    }
    if !value["getReturnType"].is_null() {
        let expr = expression(ast, &value["getReturnType"], source, numbers);
        builder.record_return_type(expr);
    }
    if value["hasConstAnnotation"].as_bool().unwrap() {
        builder.record_constancy();
    }
    for ty in value["getImplementedInterfaces"].as_array().unwrap() {
        let expr = expression(ast, ty, source, numbers);
        builder.record_implemented_interface(ast, expr);
    }
    for ty in value["getExtendedInterfaces"].as_array().unwrap() {
        let expr = expression(ast, ty, source, numbers);
        builder.record_extended_interface(ast, expr);
    }
    for param in value["$parameters"].as_array().unwrap() {
        let name = js_string(&param["name"]);
        let expr = expression(ast, &param["type"], source, numbers);
        builder.record_parameter(name.clone(), expr);
        if !param["description"].is_null() {
            builder.record_parameter_description(name, js_string(&param["description"]));
        }
    }
    for pair in value["getTemplateTypes"]["$map"].as_array().unwrap() {
        let expr = expression(ast, &pair[1], source, numbers);
        builder.record_template_type_name_with_bound(ast, js_string(&pair[0]), expr);
    }
    for pair in value["getTypeTransformations"]["$map"].as_array().unwrap() {
        let root = load_node(ast, &pair[1], source, numbers);
        builder.record_type_transformation(js_string(&pair[0]), root);
    }
    for pair in value["getSuppressionsAndTheirDescription"]["$map"]
        .as_array()
        .unwrap()
    {
        let warnings = pair[0].as_array().unwrap().iter().map(js_string).collect();
        builder.record_suppressions_with_description(&warnings, js_string(&pair[1]));
    }
    if !value["getModifies"].as_array().unwrap().is_empty() {
        builder.record_modifies(
            &value["getModifies"]
                .as_array()
                .unwrap()
                .iter()
                .map(js_string)
                .collect(),
        );
    }
    if !value["getBlockDescription"].is_null() {
        builder.record_block_description(js_string(&value["getBlockDescription"]));
    }
    if !value["getReturnDescription"].is_null() {
        builder.record_return_description(js_string(&value["getReturnDescription"]));
    }
    if !value["getDescription"].is_null() {
        builder.record_description(js_string(&value["getDescription"]));
    }
    if !value["getMeaning"].is_null() {
        builder.record_meaning(js_string(&value["getMeaning"]));
    }
    if !value["getAlternateMessageId"].is_null() {
        builder.record_alternate_message_id(js_string(&value["getAlternateMessageId"]));
    }
    if !value["getClosurePrimitiveId"].is_null() {
        builder.record_closure_primitive_id(js_string(&value["getClosurePrimitiveId"]));
    }
    if !value["getLicense"].is_null() {
        builder.record_license(js_string(&value["getLicense"]));
    }
    if value["hasFileOverview"].as_bool().unwrap() && value["getFileOverview"].is_null() {
        builder.record_file_overview("");
    }
    if !value["getFileOverview"].is_null() {
        builder.record_file_overview(js_string(&value["getFileOverview"]));
    }
    if !value["getEnhance"].is_null() {
        builder.record_enhance(js_string(&value["getEnhance"]));
    }
    if !value["getMods"].is_null() {
        builder.record_mods(js_string(&value["getMods"]));
    }
    if !value["getDeprecationReason"].is_null() {
        builder.record_deprecation_reason(js_string(&value["getDeprecationReason"]));
    }
    if let Some(values) = value["getAuthors"].as_array() {
        for v in values {
            builder.record_author(js_string(v));
        }
    }
    if let Some(values) = value["getReferences"].as_array() {
        for v in values {
            builder.record_reference(js_string(v));
        }
    }
    if let Some(values) = value["getThrowsAnnotations"].as_array() {
        for v in values {
            builder.record_throws_annotation(js_string(v));
        }
    }
    if let Some(values) = value["getTsTypes"].as_array() {
        for v in values {
            builder.record_ts_type(js_string(v));
        }
    }
    if !value["getOriginalCommentString"].is_null() {
        builder.record_original_comment_string(js_string(&value["getOriginalCommentString"]));
        builder.record_original_comment_position(
            value["getOriginalCommentPosition"].as_i64().unwrap() as i32,
        );
    }
    let template = ast.new_node(closure_rhino::token::Token::SCRIPT);
    template.set_static_source_file(ast, Some(source.clone()));
    for marker in value["getMarkers"].as_array().unwrap() {
        let annotation = &marker["getAnnotation"];
        assert!(
            !annotation.is_null(),
            "Marker without annotation cannot be produced by IRFactory"
        );
        builder.mark_annotation(
            js_string(&annotation["getItem"]),
            annotation["getStartLine"].as_i64().unwrap() as i32,
            annotation["getPositionOnStartLine"].as_i64().unwrap() as i32,
        );
        let description = &marker["getDescription"];
        if !description.is_null() {
            builder.mark_text(
                js_string(&description["getItem"]),
                description["getStartLine"].as_i64().unwrap() as i32,
                description["getPositionOnStartLine"].as_i64().unwrap() as i32,
                description["getEndLine"].as_i64().unwrap() as i32,
                description["getPositionOnEndLine"].as_i64().unwrap() as i32,
            );
        }
        let type_ = &marker["getType"];
        if !type_.is_null() {
            let root = load_node(ast, &type_["getItem"], source, numbers);
            builder.mark_type_node(
                Some(root),
                type_["getStartLine"].as_i64().unwrap() as i32,
                type_["getPositionOnStartLine"].as_i64().unwrap() as i32,
                type_["getEndLine"].as_i64().unwrap() as i32,
                type_["getPositionOnEndLine"].as_i64().unwrap() as i32,
                type_["hasBrackets"].as_bool().unwrap(),
            );
        }
        let name = &marker["getNameNode"];
        if !name.is_null() {
            builder.mark_name(
                ast,
                js_string(&name["getItem"]["string"]),
                Some(template),
                name["getStartLine"].as_i64().unwrap() as i32,
                name["getPositionOnStartLine"].as_i64().unwrap() as i32,
            );
        }
    }
    let info = builder.build_with_always(true).unwrap();
    verify(ast, &info, value);
    info
}

trait Check {
    fn check(&self, ast: &Ast, expected: &Value);
}
impl<T: Check + ?Sized> Check for &T {
    fn check(&self, ast: &Ast, expected: &Value) {
        (**self).check(ast, expected);
    }
}
impl<T: Check> Check for Arc<T> {
    fn check(&self, ast: &Ast, expected: &Value) {
        (**self).check(ast, expected);
    }
}
impl<T: Check> Check for Option<T> {
    fn check(&self, ast: &Ast, expected: &Value) {
        match self {
            Some(v) => v.check(ast, expected),
            None => assert!(expected.is_null(), "expected null, got {expected}"),
        }
    }
}
impl Check for bool {
    fn check(&self, _: &Ast, expected: &Value) {
        assert_eq!(Some(*self), expected.as_bool());
    }
}
impl Check for i32 {
    fn check(&self, _: &Ast, expected: &Value) {
        assert_eq!(Some(i64::from(*self)), expected.as_i64());
    }
}
impl Check for JsString {
    fn check(&self, _: &Ast, expected: &Value) {
        assert_eq!(*self, js_string(expected));
    }
}
impl Check for str {
    fn check(&self, _: &Ast, expected: &Value) {
        assert_eq!(Some(self), expected.as_str());
    }
}
impl<T: Check> Check for Vec<T> {
    fn check(&self, ast: &Ast, expected: &Value) {
        let e = expected.as_array().unwrap();
        assert_eq!(self.len(), e.len());
        for (a, b) in self.iter().zip(e) {
            a.check(ast, b);
        }
    }
}
impl Check for IndexSet<JsString> {
    fn check(&self, ast: &Ast, expected: &Value) {
        self.iter().collect::<Vec<_>>().check(ast, expected);
    }
}
impl<T: Check> Check for IndexMap<JsString, T> {
    fn check(&self, ast: &Ast, expected: &Value) {
        let e = expected["$map"].as_array().unwrap();
        assert_eq!(self.len(), e.len());
        for ((k, v), p) in self.iter().zip(e) {
            k.check(ast, &p[0]);
            v.check(ast, &p[1]);
        }
    }
}
impl Check for Vec<(IndexSet<JsString>, JsString)> {
    fn check(&self, ast: &Ast, expected: &Value) {
        let e = expected["$map"].as_array().unwrap();
        assert_eq!(self.len(), e.len());
        for ((k, v), p) in self.iter().zip(e) {
            k.check(ast, &p[0]);
            v.check(ast, &p[1]);
        }
    }
}
impl Check for Visibility {
    fn check(&self, _: &Ast, expected: &Value) {
        assert_eq!(format!("{self:?}"), expected.as_str().unwrap());
    }
}
impl Check for PerFileClosureUnawareMode {
    fn check(&self, _: &Ast, expected: &Value) {
        assert_eq!(format!("{self:?}"), expected.as_str().unwrap());
    }
}
impl Check for JSTypeExpression {
    fn check(&self, ast: &Ast, expected: &Value) {
        self.get_source_name().check(ast, &expected["source_name"]);
        self.get_root().check(ast, &expected["root"]);
    }
}
impl Check for NodeId {
    fn check(&self, ast: &Ast, expected: &Value) {
        assert_eq!(
            format!("{:?}", self.get_token(ast)),
            expected["token"].as_str().unwrap()
        );
        self.get_lineno(ast).check(ast, &expected["lineno"]);
        self.get_charno(ast).check(ast, &expected["charno"]);
        self.get_length(ast).check(ast, &expected["length"]);
        if expected.get("string").is_some() {
            self.get_string(ast).check(ast, &expected["string"]);
        }
        if let Some(name) = expected["source_name"].as_str() {
            assert_eq!(self.get_source_file_name(ast).as_deref(), Some(name));
        }
        self.children(ast)
            .collect::<Vec<_>>()
            .check(ast, &expected["children"]);
        for (name, value) in expected["props"].as_object().unwrap() {
            let prop = *Prop::VALUES
                .iter()
                .find(|&&prop| format!("{prop:?}") == *name)
                .unwrap();
            if let Some(i) = value.as_i64() {
                assert_eq!(i64::from(self.get_int_prop(ast, prop)), i);
            } else if prop == Prop::SOURCE_FILE {
                assert_eq!(
                    self.get_source_file_name(ast).as_deref(),
                    value["name"].as_str()
                );
            } else {
                panic!("Unverified JSDoc type AST property {name}");
            }
        }
    }
}
impl<T: Check> Check for SourcePosition<T> {
    fn check(&self, ast: &Ast, e: &Value) {
        self.get_start_line().check(ast, &e["getStartLine"]);
        self.get_end_line().check(ast, &e["getEndLine"]);
        self.get_position_on_start_line()
            .check(ast, &e["getPositionOnStartLine"]);
        self.get_position_on_end_line()
            .check(ast, &e["getPositionOnEndLine"]);
        self.get_item().check(ast, &e["getItem"]);
    }
}
impl Check for StringPosition {
    fn check(&self, ast: &Ast, e: &Value) {
        std::ops::Deref::deref(self).check(ast, e);
    }
}
impl Check for NamePosition {
    fn check(&self, ast: &Ast, e: &Value) {
        std::ops::Deref::deref(self).check(ast, e);
    }
}
impl Check for TypePosition {
    fn check(&self, ast: &Ast, e: &Value) {
        std::ops::Deref::deref(self).check(ast, e);
        self.has_brackets().check(ast, &e["hasBrackets"]);
    }
}
impl Check for Marker {
    fn check(&self, ast: &Ast, e: &Value) {
        self.get_annotation().check(ast, &e["getAnnotation"]);
        self.get_description().check(ast, &e["getDescription"]);
        self.get_type().check(ast, &e["getType"]);
        self.get_name_node().check(ast, &e["getNameNode"]);
    }
}
fn verify(ast: &Ast, info: &JSDocInfo, value: &Value) {
    if let Some(documentation) = value["$documentation_included"].as_bool() {
        assert_eq!(info.is_documentation_included(), documentation);
    }
    info.is_constant().check(ast, &value["isConstant"]);
    info.is_any_id_generator()
        .check(ast, &value["isAnyIdGenerator"]);
    info.is_consistent_id_generator()
        .check(ast, &value["isConsistentIdGenerator"]);
    info.is_stable_id_generator()
        .check(ast, &value["isStableIdGenerator"]);
    info.is_xid_generator().check(ast, &value["isXidGenerator"]);
    info.is_mapped_id_generator()
        .check(ast, &value["isMappedIdGenerator"]);
    info.is_id_generator().check(ast, &value["isIdGenerator"]);
    info.has_const_annotation()
        .check(ast, &value["hasConstAnnotation"]);
    info.is_final().check(ast, &value["isFinal"]);
    info.is_constructor().check(ast, &value["isConstructor"]);
    info.is_abstract().check(ast, &value["isAbstract"]);
    info.uses_implicit_match()
        .check(ast, &value["usesImplicitMatch"]);
    info.makes_unrestricted()
        .check(ast, &value["makesUnrestricted"]);
    info.makes_structs().check(ast, &value["makesStructs"]);
    info.makes_dicts().check(ast, &value["makesDicts"]);
    info.is_define().check(ast, &value["isDefine"]);
    info.is_override().check(ast, &value["isOverride"]);
    info.is_deprecated().check(ast, &value["isDeprecated"]);
    info.is_interface().check(ast, &value["isInterface"]);
    info.is_constructor_or_interface()
        .check(ast, &value["isConstructorOrInterface"]);
    info.is_export().check(ast, &value["isExport"]);
    info.is_implicit_cast().check(ast, &value["isImplicitCast"]);
    info.is_no_side_effects()
        .check(ast, &value["isNoSideEffects"]);
    info.is_externs().check(ast, &value["isExterns"]);
    info.is_no_coverage().check(ast, &value["isNoCoverage"]);
    info.is_type_summary().check(ast, &value["isTypeSummary"]);
    info.is_no_compile().check(ast, &value["isNoCompile"]);
    info.is_no_dts().check(ast, &value["isNoDts"]);
    info.is_no_collapse().check(ast, &value["isNoCollapse"]);
    info.is_no_inline().check(ast, &value["isNoInline"]);
    info.is_require_inlining()
        .check(ast, &value["isRequireInlining"]);
    info.is_encourage_inlining()
        .check(ast, &value["isEncourageInlining"]);
    info.is_collapsible_or_break_my_code()
        .check(ast, &value["isCollapsibleOrBreakMyCode"]);
    info.is_pure_or_break_my_code()
        .check(ast, &value["isPureOrBreakMyCode"]);
    info.is_provide_goog().check(ast, &value["isProvideGoog"]);
    info.is_provide_already_provided()
        .check(ast, &value["isProvideAlreadyProvided"]);
    info.contains_declaration_excluding_typeless_const()
        .check(ast, &value["containsDeclarationExcludingTypelessConst"]);
    info.contains_type_declaration()
        .check(ast, &value["containsTypeDeclaration"]);
    info.contains_declaration()
        .check(ast, &value["containsDeclaration"]);
    info.contains_type_definition()
        .check(ast, &value["containsTypeDefinition"]);
    info.has_enum_parameter_type()
        .check(ast, &value["hasEnumParameterType"]);
    info.has_typedef_type().check(ast, &value["hasTypedefType"]);
    info.has_return_type().check(ast, &value["hasReturnType"]);
    info.has_type().check(ast, &value["hasType"]);
    info.has_type_information()
        .check(ast, &value["hasTypeInformation"]);
    info.is_inline_type().check(ast, &value["isInlineType"]);
    info.get_return_type().check(ast, &value["getReturnType"]);
    info.get_enum_parameter_type()
        .check(ast, &value["getEnumParameterType"]);
    info.get_typedef_type().check(ast, &value["getTypedefType"]);
    info.get_type().check(ast, &value["getType"]);
    info.get_this_type().check(ast, &value["getThisType"]);
    info.has_this_type().check(ast, &value["hasThisType"]);
    info.get_base_type().check(ast, &value["getBaseType"]);
    info.get_description().check(ast, &value["getDescription"]);
    info.get_meaning().check(ast, &value["getMeaning"]);
    info.get_alternate_message_id()
        .check(ast, &value["getAlternateMessageId"]);
    info.get_lends_name().check(ast, &value["getLendsName"]);
    info.has_lends_name().check(ast, &value["hasLendsName"]);
    info.get_closure_primitive_id()
        .check(ast, &value["getClosurePrimitiveId"]);
    info.has_closure_primitive_id()
        .check(ast, &value["hasClosurePrimitiveId"]);
    info.is_ng_inject().check(ast, &value["isNgInject"]);
    info.is_wizaction().check(ast, &value["isWizaction"]);
    info.is_wizcallback().check(ast, &value["isWizcallback"]);
    info.is_polymer_behavior()
        .check(ast, &value["isPolymerBehavior"]);
    info.is_polymer().check(ast, &value["isPolymer"]);
    info.is_custom_element()
        .check(ast, &value["isCustomElement"]);
    info.is_mixin_class().check(ast, &value["isMixinClass"]);
    info.is_mixin_function()
        .check(ast, &value["isMixinFunction"]);
    info.is_sass_generated_css_ts()
        .check(ast, &value["isSassGeneratedCssTs"]);
    info.is_closure_unaware_code()
        .check(ast, &value["isClosureUnawareCode"]);
    info.get_per_file_closure_unaware_mode()
        .check(ast, &value["getPerFileClosureUnawareMode"]);
    info.is_used_via_dot_constructor()
        .check(ast, &value["isUsedViaDotConstructor"]);
    info.get_license().check(ast, &value["getLicense"]);
    info.has_base_type().check(ast, &value["hasBaseType"]);
    info.get_deprecation_reason()
        .check(ast, &value["getDeprecationReason"]);
    info.get_authors().check(ast, &value["getAuthors"]);
    info.get_references().check(ast, &value["getReferences"]);
    info.get_return_description()
        .check(ast, &value["getReturnDescription"]);
    info.get_block_description()
        .check(ast, &value["getBlockDescription"]);
    info.has_file_overview()
        .check(ast, &value["hasFileOverview"]);
    info.get_file_overview()
        .check(ast, &value["getFileOverview"]);
    info.has_enhance().check(ast, &value["hasEnhance"]);
    info.get_enhance().check(ast, &value["getEnhance"]);
    info.has_mods().check(ast, &value["hasMods"]);
    info.get_mods().check(ast, &value["getMods"]);
    info.has_modifies().check(ast, &value["hasModifies"]);
    info.get_original_comment_string()
        .check(ast, &value["getOriginalCommentString"]);
    info.modifies_this().check(ast, &value["modifiesThis"]);
    info.get_log_type_in_compiler()
        .check(ast, &value["getLogTypeInCompiler"]);
    info.get_visibility().check(ast, &value["getVisibility"]);
    info.contains_function_declaration(ast)
        .check(ast, &value["containsFunctionDeclaration"]);
    info.is_at_sign_code_present()
        .check(ast, &value["isAtSignCodePresent"]);
    info.get_original_comment_position()
        .check(ast, &value["getOriginalCommentPosition"]);
    info.get_parameter_names()
        .check(ast, &value["getParameterNames"]);
    info.get_template_type_names()
        .check(ast, &value["getTemplateTypeNames"]);
    info.get_suppressions()
        .check(ast, &value["getSuppressions"]);
    info.get_type_expressions()
        .check(ast, &value["getTypeExpressions"]);
    info.get_type_nodes().check(ast, &value["getTypeNodes"]);
    info.has_side_effects_arguments_annotation()
        .check(ast, &value["hasSideEffectsArgumentsAnnotation"]);
    info.get_ts_types().check(ast, &value["getTsTypes"]);
    info.get_throws_annotations()
        .check(ast, &value["getThrowsAnnotations"]);
    info.get_modifies().check(ast, &value["getModifies"]);
    info.get_markers().check(ast, &value["getMarkers"]);
    info.get_template_types()
        .check(ast, &value["getTemplateTypes"]);
    info.get_type_transformations()
        .check(ast, &value["getTypeTransformations"]);
    info.get_suppressions_and_their_description()
        .check(ast, &value["getSuppressionsAndTheirDescription"]);
    info.get_implemented_interfaces()
        .check(ast, &value["getImplementedInterfaces"]);
    info.get_extended_interfaces()
        .check(ast, &value["getExtendedInterfaces"]);
    info.get_parameter_count()
        .check(ast, &value["getParameterCount"]);
    info.get_implemented_interface_count()
        .check(ast, &value["getImplementedInterfaceCount"]);
    info.get_extended_interfaces_count()
        .check(ast, &value["getExtendedInterfacesCount"]);
    for param in value["$parameters"].as_array().unwrap() {
        let name = js_string(&param["name"]);
        info.get_parameter_type(name.clone())
            .check(ast, &param["type"]);
        info.get_description_for_parameter(name.clone())
            .check(ast, &param["description"]);
        info.has_parameter_type(name)
            .check(ast, &param["has_parameter_type"]);
    }
}
