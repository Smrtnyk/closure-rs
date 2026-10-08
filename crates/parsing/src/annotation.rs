/*
 * Copyright 2010 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/Annotation.java.

//! Port of `com.google.javascript.jscomp.parsing.Annotation`.

use std::sync::LazyLock;

use closure_rhino::js_string::JsString;
use indexmap::IndexMap;

/// All natively recognized JSDoc annotations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Annotation {
    ABSTRACT,
    ALTERNATE_MESSAGE_ID,
    AUTHOR,
    CLOSURE_PRIMITIVE,
    COLLAPSIBLE_OR_BREAK_MY_CODE,
    CONSTANT,
    CONSTRUCTOR,
    CUSTOM_ELEMENT,
    RECORD,
    DEFINE,
    DEPRECATED,
    DESC,
    DICT,
    ENCOURAGE_INLINING,
    ENHANCE,
    ENUM,
    EXTENDS,
    EXTERNS,
    EXPORT,
    FILE_OVERVIEW,
    FINAL,
    IDGENERATOR,
    IMPLEMENTS,
    IMPLICIT_CAST,
    INHERIT_DOC,
    INTERFACE,
    JSX,
    JSX_FRAGMENT,
    LENDS,
    LICENSE, // same as preserve
    LOG_TYPE_IN_COMPILER,
    MAY_HAVE_EXTRA_EDGE,
    MEANING,
    MIXIN_CLASS,
    MIXIN_FUNCTION,
    MODIFIES,
    MODS,
    NG_INJECT,
    NO_COLLAPSE,
    NO_COMPILE,
    NO_COVERAGE,
    /// A tag to suppress clutz's d.ts generation for classes and method. This is specifically for the
    /// use of J2CL.
    ///
    /// <p>Annotating classes, class methods and fields with @nodts has a side effect of not triggering
    /// a hard error on the code which the extended subclass unintentionally reuses the same property
    /// from base class. The author of the code using this tag should be specifically aware of this and
    /// should be able to communicate this to the consumers of their code.
    NO_DTS,
    NO_INLINE,
    NO_SIDE_EFFECTS,
    NOT_IMPLEMENTED,
    OVERRIDE,
    PACKAGE,
    PARAM,
    POLYMER,
    POLYMER_BEHAVIOR,
    PRESERVE, // same as license
    PRIVATE,
    PROTECTED,
    PROVIDE_GOOG, // @provideGoog - appears only in base.js
    PROVIDE_ALREADY_PROVIDED,
    PUBLIC,
    PURE_OR_BREAK_MY_CODE,
    REQUIRE_INLINING,
    RETURN,
    SASS_GENERATED_CSS_TS,
    SEE,
    SOY_MODULE,
    SOY_TEMPLATE,
    STRUCT,
    SUPPRESS,
    TEMPLATE,
    CLOSURE_UNAWARE_CODE,
    THIS,
    THROWS,
    TYPE,
    TYPEDEF,
    TYPE_SUMMARY,
    UNRESTRICTED,
    USED_VIA_DOT_CONSTRUCTOR,
    WIZACTION,
    TS_TYPE,
    WIZ_ANALYZER,
    WIZCALLBACK,
}

impl Annotation {
    /// Java `static final ImmutableMap<String, Annotation> recognizedAnnotations` (insertion order
    /// kept).
    // port: Annotation#recognizedAnnotations
    pub fn recognized_annotations() -> &'static IndexMap<JsString, Annotation> {
        &RECOGNIZED_ANNOTATIONS
    }
}

static RECOGNIZED_ANNOTATIONS: LazyLock<IndexMap<JsString, Annotation>> = LazyLock::new(|| {
    let mut m = IndexMap::new();
    put(&mut m, "ngInject", Annotation::NG_INJECT);
    put(&mut m, "abstract", Annotation::ABSTRACT);
    put(
        &mut m,
        "alternateMessageId",
        Annotation::ALTERNATE_MESSAGE_ID,
    );
    put(&mut m, "argument", Annotation::PARAM);
    put(&mut m, "author", Annotation::AUTHOR);
    put(&mut m, "closurePrimitive", Annotation::CLOSURE_PRIMITIVE);
    put(&mut m, "closureUnaware", Annotation::CLOSURE_UNAWARE_CODE);
    put(&mut m, "const", Annotation::CONSTANT);
    put(
        &mut m,
        "collapsibleOrBreakMyCode",
        Annotation::COLLAPSIBLE_OR_BREAK_MY_CODE,
    );
    put(&mut m, "constant", Annotation::CONSTANT);
    put(&mut m, "constructor", Annotation::CONSTRUCTOR);
    put(&mut m, "customElement", Annotation::CUSTOM_ELEMENT);
    put(&mut m, "copyright", Annotation::LICENSE);
    put(&mut m, "define", Annotation::DEFINE);
    put(&mut m, "deprecated", Annotation::DEPRECATED);
    put(&mut m, "desc", Annotation::DESC);
    put(&mut m, "dict", Annotation::DICT);
    put(&mut m, "requireInlining", Annotation::REQUIRE_INLINING);
    put(&mut m, "encourageInlining", Annotation::ENCOURAGE_INLINING);
    put(&mut m, "enum", Annotation::ENUM);
    put(&mut m, "enhance", Annotation::ENHANCE);
    put(&mut m, "export", Annotation::EXPORT);
    put(&mut m, "extends", Annotation::EXTENDS);
    put(&mut m, "externs", Annotation::EXTERNS);
    put(&mut m, "fileoverview", Annotation::FILE_OVERVIEW);
    put(&mut m, "final", Annotation::FINAL);
    put(&mut m, "idGenerator", Annotation::IDGENERATOR);
    put(&mut m, "implements", Annotation::IMPLEMENTS);
    put(&mut m, "implicitCast", Annotation::IMPLICIT_CAST);
    put(&mut m, "inheritDoc", Annotation::INHERIT_DOC);
    put(&mut m, "interface", Annotation::INTERFACE);
    put(&mut m, "record", Annotation::RECORD);
    put(&mut m, "lends", Annotation::LENDS);
    put(&mut m, "license", Annotation::LICENSE);
    put(
        &mut m,
        "logTypeInCompiler",
        Annotation::LOG_TYPE_IN_COMPILER,
    );
    put(&mut m, "mayhaveextraedge", Annotation::MAY_HAVE_EXTRA_EDGE);
    put(&mut m, "meaning", Annotation::MEANING);
    put(&mut m, "mixinClass", Annotation::MIXIN_CLASS);
    put(&mut m, "mixinFunction", Annotation::MIXIN_FUNCTION);
    put(&mut m, "modifies", Annotation::MODIFIES);
    put(&mut m, "mods", Annotation::MODS);
    put(&mut m, "nocollapse", Annotation::NO_COLLAPSE);
    put(&mut m, "nocompile", Annotation::NO_COMPILE);
    put(&mut m, "nocoverage", Annotation::NO_COVERAGE);
    put(&mut m, "nodts", Annotation::NO_DTS);
    put(&mut m, "noinline", Annotation::NO_INLINE);
    put(&mut m, "nosideeffects", Annotation::NO_SIDE_EFFECTS);
    put(&mut m, "override", Annotation::OVERRIDE);
    put(&mut m, "owner", Annotation::AUTHOR);
    put(&mut m, "package", Annotation::PACKAGE);
    put(&mut m, "param", Annotation::PARAM);
    put(&mut m, "polymer", Annotation::POLYMER);
    put(&mut m, "polymerBehavior", Annotation::POLYMER_BEHAVIOR);
    put(&mut m, "preserve", Annotation::PRESERVE);
    put(&mut m, "private", Annotation::PRIVATE);
    put(&mut m, "protected", Annotation::PROTECTED);
    put(&mut m, "provideGoog", Annotation::PROVIDE_GOOG);
    put(
        &mut m,
        "provideAlreadyProvided",
        Annotation::PROVIDE_ALREADY_PROVIDED,
    );
    put(&mut m, "public", Annotation::PUBLIC);
    put(
        &mut m,
        "pureOrBreakMyCode",
        Annotation::PURE_OR_BREAK_MY_CODE,
    );
    put(&mut m, "return", Annotation::RETURN);
    put(&mut m, "returns", Annotation::RETURN);
    put(
        &mut m,
        "sassGeneratedCssTs",
        Annotation::SASS_GENERATED_CSS_TS,
    );
    put(&mut m, "see", Annotation::SEE);
    put(&mut m, "soyModule", Annotation::SOY_MODULE);
    put(&mut m, "soyTemplate", Annotation::SOY_TEMPLATE);
    put(&mut m, "struct", Annotation::STRUCT);
    put(&mut m, "suppress", Annotation::SUPPRESS);
    put(&mut m, "template", Annotation::TEMPLATE);
    put(&mut m, "this", Annotation::THIS);
    put(&mut m, "throws", Annotation::THROWS);
    put(&mut m, "type", Annotation::TYPE);
    put(&mut m, "typedef", Annotation::TYPEDEF);
    put(&mut m, "typeSummary", Annotation::TYPE_SUMMARY);
    put(&mut m, "unrestricted", Annotation::UNRESTRICTED);
    put(
        &mut m,
        "usedViaDotConstructor",
        Annotation::USED_VIA_DOT_CONSTRUCTOR,
    );
    put(&mut m, "wizaction", Annotation::WIZACTION);
    put(&mut m, "tsType", Annotation::TS_TYPE);
    put(&mut m, "wizAnalyzer", Annotation::WIZ_ANALYZER);
    put(&mut m, "wizcallback", Annotation::WIZCALLBACK);
    m
});

/// `ImmutableMap.Builder#put` followed by `buildOrThrow`: a duplicate key is an error.
fn put(m: &mut IndexMap<JsString, Annotation>, key: &str, value: Annotation) {
    let previous = m.insert(JsString::from(key), value);
    assert!(previous.is_none(), "Multiple entries with same key: {key}");
}
