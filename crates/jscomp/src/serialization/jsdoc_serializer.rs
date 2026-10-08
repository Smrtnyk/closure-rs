/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/serialization/JSDocSerializer.java.

//! Port of serialization/JSDocSerializer.java: utilities for serializing and deserializing JSDoc
//! necessary for optimizations.
use super::malformed_typed_ast_exception::MalformedTypedAstException;
use super::optimization_jsdoc_proto::{JsdocTag, OptimizationJsdoc};
use super::string_pool::{StringPool, StringPoolBuilder};
use crate::source_file::SourceFile;
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::js_type_expression::JSTypeExpression;
use closure_rhino::jsdoc_info::{JSDocInfo, PerFileClosureUnawareMode};
use closure_rhino::node::Ast;
use closure_rhino::static_source_file::StaticSourceFile;
use closure_rhino::token::Token;
use indexmap::IndexSet;
use std::collections::BTreeSet;
use std::sync::{Arc, LazyLock};

/// port: JSDocSerializer
pub struct JSDocSerializer;

// Optimizations shouldn't care about the contents of JSTypeExpressions but some JSDoc APIs
// expect their presence, so create a placeholder type.
static SYNTHETIC_SOURCE: LazyLock<Arc<SourceFile>> = LazyLock::new(|| {
    Arc::new(SourceFile::from_code(
        "JSDocSerializer_placeholder_source",
        "",
    ))
});

const PLACEHOLDER_TYPE_NAME: &str = "JSDocSerializer_placeholder_type";

impl JSDocSerializer {
    /// Returns a variant of input JSDocInfo where fields not needed for optimizations are removed
    ///
    /// This uses the serialization / deserialization logic for JSDoc and ensures optimizations
    /// can't accidentally depend on fields that we don't serialize.
    ///
    /// Returns a new JSDocInfo object or None if no serializable fields are found
    // port: JSDocSerializer#convertJSDocInfoForOptimizations
    pub fn convert_jsdoc_info_for_optimizations(
        ast: &mut Ast,
        jsdoc: Option<&JSDocInfo>,
    ) -> Option<Arc<JSDocInfo>> {
        let mut string_pool = StringPool::builder();
        let serialized = Self::serialize_jsdoc(jsdoc, &mut string_pool);
        Self::deserialize_jsdoc(ast, serialized.as_ref(), &string_pool.build())
    }

    // port: JSDocSerializer#serializeJsdoc
    pub fn serialize_jsdoc(
        jsdoc: Option<&JSDocInfo>,
        string_pool: &mut StringPoolBuilder,
    ) -> Option<OptimizationJsdoc> {
        let jsdoc = jsdoc?;

        let mut builder = OptimizationJsdoc::new_builder();

        if jsdoc.has_file_overview() {
            builder = builder.add_kind(JsdocTag::JSDOC_FILEOVERVIEW);
        }

        if let Some(license) = jsdoc.get_license() {
            builder = builder.set_license_text_pointer(string_pool.put(license));
        }

        if jsdoc.is_sass_generated_css_ts() {
            builder = builder.add_kind(JsdocTag::JSDOC_SASS_GENERATED_CSS_TS);
        }

        // Used by CoverageInstrumentationCallback
        if jsdoc.is_no_coverage() {
            builder = builder.add_kind(JsdocTag::JSDOC_NO_COVERAGE);
        }

        if jsdoc.is_no_inline() {
            builder = builder.add_kind(JsdocTag::JSDOC_NO_INLINE);
        }

        if jsdoc.is_encourage_inlining() {
            builder = builder.add_kind(JsdocTag::JSDOC_ENCOURAGE_INLINING);
        }

        if jsdoc.is_require_inlining() {
            builder = builder.add_kind(JsdocTag::JSDOC_REQUIRE_INLINING);
        }

        if jsdoc.is_no_collapse() {
            builder = builder.add_kind(JsdocTag::JSDOC_NO_COLLAPSE);
        }

        if jsdoc.is_provide_goog() {
            builder = builder.add_kind(JsdocTag::JSDOC_PROVIDE_GOOG);
        }

        if jsdoc.is_provide_already_provided() {
            builder = builder.add_kind(JsdocTag::JSDOC_PROVIDE_ALREADY_PROVIDED);
        }

        if jsdoc.is_type_summary() {
            builder = builder.add_kind(JsdocTag::JSDOC_TYPE_SUMMARY_FILE);
        }

        if jsdoc.is_pure_or_break_my_code() {
            builder = builder.add_kind(JsdocTag::JSDOC_PURE_OR_BREAK_MY_CODE);
        }
        if jsdoc.is_collapsible_or_break_my_code() {
            builder = builder.add_kind(JsdocTag::JSDOC_COLLAPSIBLE_OR_BREAK_MY_CODE);
        }
        if jsdoc.has_this_type() {
            builder = builder.add_kind(JsdocTag::JSDOC_THIS);
        }
        if jsdoc.has_enum_parameter_type() {
            builder = builder.add_kind(JsdocTag::JSDOC_ENUM);
        }
        if jsdoc.is_define() {
            builder = builder.add_kind(JsdocTag::JSDOC_DEFINE);
        }
        if jsdoc.has_const_annotation() {
            builder = builder.add_kind(JsdocTag::JSDOC_CONST);
        }
        if jsdoc.is_any_id_generator() {
            builder = builder.add_kind(Self::serialize_id_generator(jsdoc));
        }

        // Used by PureFunctionIdentifier
        if jsdoc.is_no_side_effects() {
            builder = builder.add_kind(JsdocTag::JSDOC_NO_SIDE_EFFECTS);
        }
        if jsdoc.has_modifies() {
            for modifies in jsdoc.get_modifies() {
                if modifies == "this" {
                    builder = builder.add_kind(JsdocTag::JSDOC_MODIFIES_THIS);
                } else if modifies == "arguments" {
                    builder = builder.add_kind(JsdocTag::JSDOC_MODIFIES_ARGUMENTS);
                } else {
                    // Currently, anything other than "this" is considered a modification to
                    // arguments
                    builder = builder.add_kind(JsdocTag::JSDOC_MODIFIES_ARGUMENTS);
                }
            }
        }
        if !jsdoc.get_throws_annotations().is_empty() {
            builder = builder.add_kind(JsdocTag::JSDOC_THROWS);
        }

        // Used by DevirtualizeMethods and CollapseProperties
        if jsdoc.is_constructor() {
            builder = builder.add_kind(JsdocTag::JSDOC_CONSTRUCTOR);
        }
        if jsdoc.is_interface() {
            builder = builder.add_kind(JsdocTag::JSDOC_INTERFACE);
        }
        if jsdoc
            .get_suppressions()
            .contains(&JsString::from("partialAlias"))
        {
            builder = builder.add_kind(JsdocTag::JSDOC_SUPPRESS_PARTIAL_ALIAS);
        }

        // Used by ClosureCodeRemoval
        if jsdoc.is_abstract() {
            builder = builder.add_kind(JsdocTag::JSDOC_ABSTRACT);
        }

        // Used by ReplaceMessages
        if let Some(description) = jsdoc.get_description() {
            builder = builder.set_description_pointer(string_pool.put(description));
        }
        if let Some(alternate_message_id) = jsdoc.get_alternate_message_id() {
            builder =
                builder.set_alternate_message_id_pointer(string_pool.put(alternate_message_id));
        }
        if let Some(meaning) = jsdoc.get_meaning() {
            builder = builder.set_meaning_pointer(string_pool.put(meaning));
        }
        if jsdoc
            .get_suppressions()
            .contains(&JsString::from("messageConventions"))
        {
            builder = builder.add_kind(JsdocTag::JSDOC_SUPPRESS_MESSAGE_CONVENTION);
        }
        if jsdoc
            .get_suppressions()
            .contains(&JsString::from("untranspilableFeatures"))
        {
            builder = builder.add_kind(JsdocTag::JSDOC_SUPPRESS_UNTRANSPILABLE_FEATURES);
        }
        if jsdoc.is_used_via_dot_constructor() {
            builder = builder.add_kind(JsdocTag::JSDOC_USED_VIA_DOT_CONSTRUCTOR);
        }
        if let Some(mode) = jsdoc.get_per_file_closure_unaware_mode() {
            builder = builder.set_closure_unaware_mode_pointer(
                string_pool.put(Self::closure_unaware_mode_name(mode)),
            );
        }

        let result = builder.build();
        if OptimizationJsdoc::get_default_instance() == result {
            return None;
        }
        Some(result)
    }

    // port: JSDocSerializer#serializeIdGenerator
    fn serialize_id_generator(doc: &JSDocInfo) -> JsdocTag {
        if doc.is_consistent_id_generator() {
            return JsdocTag::JSDOC_ID_GENERATOR_CONSISTENT;
        } else if doc.is_stable_id_generator() {
            return JsdocTag::JSDOC_ID_GENERATOR_STABLE;
        } else if doc.is_xid_generator() {
            return JsdocTag::JSDOC_ID_GENERATOR_XID;
        } else if doc.is_mapped_id_generator() {
            return JsdocTag::JSDOC_ID_GENERATOR_MAPPED;
        } else if doc.is_id_generator() {
            return JsdocTag::JSDOC_ID_GENERATOR_INCONSISTENT;
        }
        panic!("Failed to identify idGenerator inside JSDoc: {doc:?}");
    }

    /// Java `PerFileClosureUnawareMode.name()`.
    fn closure_unaware_mode_name(mode: PerFileClosureUnawareMode) -> &'static str {
        match mode {
            PerFileClosureUnawareMode::UNSPECIFIED => "UNSPECIFIED",
            PerFileClosureUnawareMode::SIMPLE => "SIMPLE",
            PerFileClosureUnawareMode::WHITESPACE => "WHITESPACE",
        }
    }

    /// Java `PerFileClosureUnawareMode.valueOf(name)`.
    fn closure_unaware_mode_value_of(name: &JsString) -> PerFileClosureUnawareMode {
        match name.to_string_lossy().as_str() {
            "UNSPECIFIED" => PerFileClosureUnawareMode::UNSPECIFIED,
            "SIMPLE" => PerFileClosureUnawareMode::SIMPLE,
            "WHITESPACE" => PerFileClosureUnawareMode::WHITESPACE,
            other => panic!(
                "No enum constant com.google.javascript.rhino.JSDocInfo.PerFileClosureUnawareMode.{other}"
            ),
        }
    }

    // port: JSDocSerializer#createPlaceholderType
    fn create_placeholder_type(ast: &mut Ast) -> Arc<JSTypeExpression> {
        // the BANG (!) token makes unit testing easier, as the JSDoc parser implicitly adds "!"
        // to some JSTypeExpressions
        let name = IR::string(ast, PLACEHOLDER_TYPE_NAME);
        let bang = ast.new_node_with_child(Token::BANG, name);
        let source: Arc<dyn StaticSourceFile> = SYNTHETIC_SOURCE.clone();
        name.set_static_source_file(ast, Some(source.clone()));
        bang.set_static_source_file(ast, Some(source));
        Arc::new(JSTypeExpression::new(
            bang,
            SYNTHETIC_SOURCE.get_name().to_string(),
        ))
    }

    // port: JSDocSerializer#placeholderType
    fn placeholder_type(ast: &mut Ast) -> Arc<JSTypeExpression> {
        if let Some(placeholder) = &ast.jsdoc_serializer_placeholder_type {
            return placeholder.clone();
        }
        let placeholder = Self::create_placeholder_type(ast);
        ast.jsdoc_serializer_placeholder_type = Some(placeholder.clone());
        placeholder
    }

    // port: JSDocSerializer#deserializeJsdoc
    pub fn deserialize_jsdoc(
        ast: &mut Ast,
        serialized_jsdoc: Option<&OptimizationJsdoc>,
        string_pool: &StringPool,
    ) -> Option<Arc<JSDocInfo>> {
        let serialized_jsdoc = serialized_jsdoc?;
        let mut builder = JSDocInfo::builder();
        if serialized_jsdoc.get_closure_unaware_mode_pointer() != 0 {
            builder.record_closure_unaware_code_with_mode(Self::closure_unaware_mode_value_of(
                &string_pool.get(serialized_jsdoc.get_closure_unaware_mode_pointer()),
            ));
        }
        if serialized_jsdoc.get_license_text_pointer() != 0 {
            builder.add_license(string_pool.get(serialized_jsdoc.get_license_text_pointer()));
        }
        if serialized_jsdoc.get_meaning_pointer() != 0 {
            builder.record_meaning(string_pool.get(serialized_jsdoc.get_meaning_pointer()));
        }
        if serialized_jsdoc.get_description_pointer() != 0 {
            builder.record_description(string_pool.get(serialized_jsdoc.get_description_pointer()));
        }
        if serialized_jsdoc.get_alternate_message_id_pointer() != 0 {
            builder.record_alternate_message_id(
                string_pool.get(serialized_jsdoc.get_alternate_message_id_pointer()),
            );
        }

        // lazily create these sets to save a few hundred ms for some large projects
        let mut modifies: Option<BTreeSet<JsString>> = None;
        let mut suppressions: Option<BTreeSet<JsString>> = None;

        for i in 0..serialized_jsdoc.get_kind_count() {
            let tag = serialized_jsdoc.get_kind_list()[i as usize];
            match tag {
                JsdocTag::JSDOC_CONST => {
                    builder.record_constancy();
                    continue;
                }
                JsdocTag::JSDOC_ENUM => {
                    builder.record_enum_parameter_type(Some(Self::placeholder_type(ast)));
                    continue;
                }
                JsdocTag::JSDOC_THIS => {
                    builder.record_this_type(Some(Self::placeholder_type(ast)));
                    continue;
                }
                JsdocTag::JSDOC_NO_COLLAPSE => {
                    builder.record_no_collapse();
                    continue;
                }
                JsdocTag::JSDOC_NO_INLINE => {
                    builder.record_no_inline();
                    continue;
                }
                JsdocTag::JSDOC_REQUIRE_INLINING => {
                    builder.record_require_inlining();
                    continue;
                }
                JsdocTag::JSDOC_ENCOURAGE_INLINING => {
                    builder.record_encourage_inlining();
                    continue;
                }
                JsdocTag::JSDOC_PROVIDE_GOOG => {
                    builder.record_provide_goog();
                    continue;
                }
                JsdocTag::JSDOC_PROVIDE_ALREADY_PROVIDED => {
                    builder.record_provide_already_provided();
                    continue;
                }
                JsdocTag::JSDOC_TYPE_SUMMARY_FILE => {
                    builder.record_type_summary();
                    continue;
                }
                JsdocTag::JSDOC_PURE_OR_BREAK_MY_CODE => {
                    builder.record_pure_or_break_my_code();
                    continue;
                }
                JsdocTag::JSDOC_COLLAPSIBLE_OR_BREAK_MY_CODE => {
                    builder.record_collapsible_or_break_my_code();
                    continue;
                }
                JsdocTag::JSDOC_DEFINE => {
                    builder.record_define_type(Some(Self::placeholder_type(ast)));
                    continue;
                }
                JsdocTag::JSDOC_NO_SIDE_EFFECTS => {
                    builder.record_no_side_effects();
                    continue;
                }
                JsdocTag::JSDOC_MODIFIES_THIS => {
                    modifies
                        .get_or_insert_with(BTreeSet::new)
                        .insert(JsString::from("this"));
                    continue;
                }
                JsdocTag::JSDOC_MODIFIES_ARGUMENTS => {
                    modifies
                        .get_or_insert_with(BTreeSet::new)
                        .insert(JsString::from("arguments"));
                    continue;
                }
                JsdocTag::JSDOC_THROWS => {
                    builder.record_throws_annotation(format!("{{!{PLACEHOLDER_TYPE_NAME}}}"));
                    continue;
                }
                JsdocTag::JSDOC_CONSTRUCTOR => {
                    builder.record_constructor();
                    continue;
                }
                JsdocTag::JSDOC_INTERFACE => {
                    builder.record_interface();
                    continue;
                }
                JsdocTag::JSDOC_SUPPRESS_PARTIAL_ALIAS => {
                    suppressions
                        .get_or_insert_with(BTreeSet::new)
                        .insert(JsString::from("partialAlias"));
                    continue;
                }
                JsdocTag::JSDOC_ID_GENERATOR_CONSISTENT => {
                    builder.record_consistent_id_generator();
                    continue;
                }
                JsdocTag::JSDOC_ID_GENERATOR_STABLE => {
                    builder.record_stable_id_generator();
                    continue;
                }
                JsdocTag::JSDOC_ID_GENERATOR_MAPPED => {
                    builder.record_mapped_id_generator();
                    continue;
                }
                JsdocTag::JSDOC_ID_GENERATOR_XID => {
                    builder.record_xid_generator();
                    continue;
                }
                JsdocTag::JSDOC_ID_GENERATOR_INCONSISTENT => {
                    builder.record_id_generator();
                    continue;
                }
                JsdocTag::JSDOC_ABSTRACT => {
                    builder.record_abstract();
                    continue;
                    // TODO(lharker): stage 2 passes ideally shouldn't report diagnostics, so this
                    // could be moved to stage 1.
                }
                JsdocTag::JSDOC_SUPPRESS_MESSAGE_CONVENTION => {
                    suppressions
                        .get_or_insert_with(BTreeSet::new)
                        .insert(JsString::from("messageConventions"));
                    continue;
                    // ReportUntranspilableFeatures pass will run in stage2 since it uses
                    // languageOut information. It reports diagnostic
                    // UNTRANSPILABLE_FEATURE_PRESENT that can be supppressed using
                    // `untranspilableFeatures` suppression tag. Hence we must propagate it.
                }
                JsdocTag::JSDOC_SUPPRESS_UNTRANSPILABLE_FEATURES => {
                    suppressions
                        .get_or_insert_with(BTreeSet::new)
                        .insert(JsString::from("untranspilableFeatures"));
                    continue;
                }
                JsdocTag::JSDOC_FILEOVERVIEW => {
                    builder.record_file_overview("");
                    continue;
                }
                JsdocTag::JSDOC_NO_COVERAGE => {
                    builder.record_no_coverage();
                    continue;
                }
                JsdocTag::JSDOC_SASS_GENERATED_CSS_TS => {
                    builder.record_sass_generated_css_ts();
                    continue;
                }
                JsdocTag::JSDOC_USED_VIA_DOT_CONSTRUCTOR => {
                    let _unused = builder.record_used_via_dot_constructor();
                    continue;
                }
                JsdocTag::JSDOC_UNSPECIFIED | JsdocTag::UNRECOGNIZED => {
                    MalformedTypedAstException::new(format!(
                        "Unsupported JSDoc tag can't be deserialized: {tag}"
                    ))
                    .throw();
                }
            }
        }
        if let Some(modifies) = modifies {
            builder.record_modifies(&modifies.into_iter().collect::<IndexSet<JsString>>());
        }
        if let Some(suppressions) = suppressions {
            builder.record_suppressions(&suppressions.into_iter().collect::<IndexSet<JsString>>());
        }

        builder.build()
    }
}
