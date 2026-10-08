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
// Ported from closure-rs' own Java oracle tooling:
//   oracle/src/com/google/javascript/jscomp/ParseDump.java.

// Getter set derived with javap -public from the pinned reference jar; sorted as ParseDump.beanJson.
impl DumpValue for JSDocInfo {
    // port: ParseDump#jsdoc
    // port: ParseDump#beanJson
    fn dump(&self, d: &Dump<'_>) -> Value {
        let mut o = Map::new();
        o.insert("$class".into(), json!("com.google.javascript.rhino.JSDocInfo"));
        o.insert("containsDeclaration".into(), self.contains_declaration().dump(d));
        o.insert("containsDeclarationExcludingTypelessConst".into(), self.contains_declaration_excluding_typeless_const().dump(d));
        o.insert("containsFunctionDeclaration".into(), self.contains_function_declaration(d.ast).dump(d));
        o.insert("containsTypeDeclaration".into(), self.contains_type_declaration().dump(d));
        o.insert("containsTypeDefinition".into(), self.contains_type_definition().dump(d));
        o.insert("getAlternateMessageId".into(), self.get_alternate_message_id().dump(d));
        o.insert("getAuthors".into(), self.get_authors().dump(d));
        o.insert("getBaseType".into(), self.get_base_type().dump(d));
        o.insert("getBlockDescription".into(), self.get_block_description().dump(d));
        o.insert("getClosurePrimitiveId".into(), self.get_closure_primitive_id().dump(d));
        o.insert("getDeprecationReason".into(), self.get_deprecation_reason().dump(d));
        o.insert("getDescription".into(), self.get_description().dump(d));
        o.insert("getEnhance".into(), self.get_enhance().dump(d));
        o.insert("getEnumParameterType".into(), self.get_enum_parameter_type().dump(d));
        o.insert("getExtendedInterfaces".into(), self.get_extended_interfaces().dump(d));
        o.insert("getExtendedInterfacesCount".into(), self.get_extended_interfaces_count().dump(d));
        o.insert("getFileOverview".into(), self.get_file_overview().dump(d));
        o.insert("getImplementedInterfaceCount".into(), self.get_implemented_interface_count().dump(d));
        o.insert("getImplementedInterfaces".into(), self.get_implemented_interfaces().dump(d));
        o.insert("getLendsName".into(), self.get_lends_name().dump(d));
        o.insert("getLicense".into(), self.get_license().dump(d));
        o.insert("getLogTypeInCompiler".into(), self.get_log_type_in_compiler().dump(d));
        o.insert("getMarkers".into(), self.get_markers().dump(d));
        o.insert("getMeaning".into(), self.get_meaning().dump(d));
        o.insert("getModifies".into(), self.get_modifies().dump(d));
        o.insert("getMods".into(), self.get_mods().dump(d));
        o.insert("getOriginalCommentPosition".into(), self.get_original_comment_position().dump(d));
        o.insert("getOriginalCommentString".into(), self.get_original_comment_string().dump(d));
        o.insert("getParameterCount".into(), self.get_parameter_count().dump(d));
        o.insert("getParameterNames".into(), self.get_parameter_names().dump(d));
        o.insert("getPerFileClosureUnawareMode".into(), self.get_per_file_closure_unaware_mode().dump(d));
        o.insert("getReferences".into(), self.get_references().dump(d));
        o.insert("getReturnDescription".into(), self.get_return_description().dump(d));
        o.insert("getReturnType".into(), self.get_return_type().dump(d));
        o.insert("getSuppressions".into(), self.get_suppressions().dump(d));
        o.insert("getTemplateTypeNames".into(), self.get_template_type_names().dump(d));
        o.insert("getThisType".into(), self.get_this_type().dump(d));
        o.insert("getThrowsAnnotations".into(), self.get_throws_annotations().dump(d));
        o.insert("getTsTypes".into(), self.get_ts_types().dump(d));
        o.insert("getType".into(), self.get_type().dump(d));
        o.insert("getTypeExpressions".into(), self.get_type_expressions().dump(d));
        o.insert("getTypeNodes".into(), self.get_type_nodes().dump(d));
        o.insert("getTypedefType".into(), self.get_typedef_type().dump(d));
        o.insert("getVisibility".into(), self.get_visibility().dump(d));
        o.insert("hasBaseType".into(), self.has_base_type().dump(d));
        o.insert("hasClosurePrimitiveId".into(), self.has_closure_primitive_id().dump(d));
        o.insert("hasConstAnnotation".into(), self.has_const_annotation().dump(d));
        o.insert("hasEnhance".into(), self.has_enhance().dump(d));
        o.insert("hasEnumParameterType".into(), self.has_enum_parameter_type().dump(d));
        o.insert("hasFileOverview".into(), self.has_file_overview().dump(d));
        o.insert("hasLendsName".into(), self.has_lends_name().dump(d));
        o.insert("hasModifies".into(), self.has_modifies().dump(d));
        o.insert("hasMods".into(), self.has_mods().dump(d));
        o.insert("hasReturnType".into(), self.has_return_type().dump(d));
        o.insert("hasSideEffectsArgumentsAnnotation".into(), self.has_side_effects_arguments_annotation().dump(d));
        o.insert("hasThisType".into(), self.has_this_type().dump(d));
        o.insert("hasType".into(), self.has_type().dump(d));
        o.insert("hasTypeInformation".into(), self.has_type_information().dump(d));
        o.insert("hasTypedefType".into(), self.has_typedef_type().dump(d));
        o.insert("isAbstract".into(), self.is_abstract().dump(d));
        o.insert("isAnyIdGenerator".into(), self.is_any_id_generator().dump(d));
        o.insert("isAtSignCodePresent".into(), self.is_at_sign_code_present().dump(d));
        o.insert("isClosureUnawareCode".into(), self.is_closure_unaware_code().dump(d));
        o.insert("isCollapsibleOrBreakMyCode".into(), self.is_collapsible_or_break_my_code().dump(d));
        o.insert("isConsistentIdGenerator".into(), self.is_consistent_id_generator().dump(d));
        o.insert("isConstant".into(), self.is_constant().dump(d));
        o.insert("isConstructor".into(), self.is_constructor().dump(d));
        o.insert("isConstructorOrInterface".into(), self.is_constructor_or_interface().dump(d));
        o.insert("isCustomElement".into(), self.is_custom_element().dump(d));
        o.insert("isDefine".into(), self.is_define().dump(d));
        o.insert("isDeprecated".into(), self.is_deprecated().dump(d));
        o.insert("isEncourageInlining".into(), self.is_encourage_inlining().dump(d));
        o.insert("isExport".into(), self.is_export().dump(d));
        o.insert("isExterns".into(), self.is_externs().dump(d));
        o.insert("isFinal".into(), self.is_final().dump(d));
        o.insert("isIdGenerator".into(), self.is_id_generator().dump(d));
        o.insert("isImplicitCast".into(), self.is_implicit_cast().dump(d));
        o.insert("isInlineType".into(), self.is_inline_type().dump(d));
        o.insert("isInterface".into(), self.is_interface().dump(d));
        o.insert("isMappedIdGenerator".into(), self.is_mapped_id_generator().dump(d));
        o.insert("isMixinClass".into(), self.is_mixin_class().dump(d));
        o.insert("isMixinFunction".into(), self.is_mixin_function().dump(d));
        o.insert("isNgInject".into(), self.is_ng_inject().dump(d));
        o.insert("isNoCollapse".into(), self.is_no_collapse().dump(d));
        o.insert("isNoCompile".into(), self.is_no_compile().dump(d));
        o.insert("isNoCoverage".into(), self.is_no_coverage().dump(d));
        o.insert("isNoDts".into(), self.is_no_dts().dump(d));
        o.insert("isNoInline".into(), self.is_no_inline().dump(d));
        o.insert("isNoSideEffects".into(), self.is_no_side_effects().dump(d));
        o.insert("isOverride".into(), self.is_override().dump(d));
        o.insert("isPolymer".into(), self.is_polymer().dump(d));
        o.insert("isPolymerBehavior".into(), self.is_polymer_behavior().dump(d));
        o.insert("isProvideAlreadyProvided".into(), self.is_provide_already_provided().dump(d));
        o.insert("isProvideGoog".into(), self.is_provide_goog().dump(d));
        o.insert("isPureOrBreakMyCode".into(), self.is_pure_or_break_my_code().dump(d));
        o.insert("isRequireInlining".into(), self.is_require_inlining().dump(d));
        o.insert("isSassGeneratedCssTs".into(), self.is_sass_generated_css_ts().dump(d));
        o.insert("isStableIdGenerator".into(), self.is_stable_id_generator().dump(d));
        o.insert("isTypeSummary".into(), self.is_type_summary().dump(d));
        o.insert("isUsedViaDotConstructor".into(), self.is_used_via_dot_constructor().dump(d));
        o.insert("isWizaction".into(), self.is_wizaction().dump(d));
        o.insert("isWizcallback".into(), self.is_wizcallback().dump(d));
        o.insert("isXidGenerator".into(), self.is_xid_generator().dump(d));
        o.insert("makesDicts".into(), self.makes_dicts().dump(d));
        o.insert("makesStructs".into(), self.makes_structs().dump(d));
        o.insert("makesUnrestricted".into(), self.makes_unrestricted().dump(d));
        o.insert("modifiesThis".into(), self.modifies_this().dump(d));
        o.insert("usesImplicitMatch".into(), self.uses_implicit_match().dump(d));
        o.insert("getSuppressionsAndTheirDescription".into(), json!({"$map": self.get_suppressions_and_their_description().iter().map(|(k,v)| vec![k.dump(d), v.dump(d)]).collect::<Vec<_>>()}));
        o.insert("getTemplateTypes".into(), self.get_template_types().dump(d));
        o.insert("getTypeTransformations".into(), self.get_type_transformations().dump(d));
        // ParseDump.beanJson sorts all reflected methods by name, including generic getters.
        o.sort_keys();
        let params = (0..self.get_parameter_count()).map(|i| {
            let pn = self.get_parameter_name_at(i).unwrap();
            json!({"name":super::diagnostic_utf8(&pn),"type":self.get_parameter_type(pn.clone()).dump(d),
                "description":self.get_description_for_parameter(pn.clone()).dump(d),
                "has_parameter_type":self.has_parameter_type(pn)})
        }).collect();
        o.insert("$parameters".into(), Value::Array(params));
        Value::Object(o)
    }
}
