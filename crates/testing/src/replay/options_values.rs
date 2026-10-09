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
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Conversion of captured values into the real option field types.
use crate::jscomp_api::{CompilerPassRef, InputStreamWrapper, OutputStreamWrapper};
use crate::{
    json::JsonValue,
    replay::replay_dsl::{DslValue, Object},
    throwable::Throwable,
};
use closure_jscomp::{
    check_level::CheckLevel,
    compiler_options::*,
    dependency_options::{DependencyMode, DependencyOptions},
    module_identifier::ModuleIdentifier,
};
use closure_parsing::{
    config::JsDocParsing,
    parser::feature_set::{Feature, FeatureSet},
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    java_lang::{charset::Charset, pattern::Pattern},
    js_string::JsString,
    jscomp_base::Tri,
};
use std::{cell::RefCell, path::PathBuf, rc::Rc, sync::Arc};

pub trait OptionValue: Sized {
    // port: ReplayValues#decode (declared native option field type)
    fn decode_value(value: &DslValue) -> Result<Self, Throwable>;
    // port: UnitRecorder#value (native option field type)
    fn encode_value(&self) -> Result<DslValue, Throwable>;
}
// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError("decoded option value has the wrong declared type".into())
}
// port: ReplayValues#enumValue
fn enum_name(value: &DslValue) -> Result<&str, Throwable> {
    crate::replay::options_fields::enum_name(value)
}
// port: UnitRecorder#fields (native Java object)
fn java_object(class: &str, fields: IndexMap<String, DslValue>) -> DslValue {
    DslValue::Object(Rc::new(RefCell::new(Object {
        class: class.into(),
        fields,
        field_types: IndexMap::<_, _>::default(),
    })))
}
// port: ReplayValues#fields (decoded Java object)
fn fields(value: &DslValue, class: &str) -> Result<IndexMap<String, DslValue>, Throwable> {
    match value.untyped() {
        DslValue::Object(o) if o.borrow().class == class => Ok(o.borrow().fields.clone()),
        _ => Err(bad()),
    }
}
// port: ReplayValues#findField
fn field<'a>(fields: &'a IndexMap<String, DslValue>, key: &str) -> Result<&'a DslValue, Throwable> {
    fields.get(key).ok_or_else(bad)
}
impl OptionValue for bool {
    // port: ReplayValues#decode (boolean)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        if let DslValue::Bool(b) = v.untyped() {
            Ok(*b)
        } else {
            Err(bad())
        }
    }
    // port: UnitRecorder#value (boolean)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(DslValue::Bool(*self))
    }
}
impl OptionValue for i32 {
    // port: ReplayValues#decode (int)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        if let DslValue::Int(i) = v.untyped() {
            Ok(*i)
        } else {
            Err(bad())
        }
    }
    // port: UnitRecorder#value (int)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(DslValue::Int(*self))
    }
}
impl OptionValue for u16 {
    // port: ReplayValues#decode (char)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        if let DslValue::Char(c) = v.untyped() {
            Ok(*c)
        } else {
            Err(bad())
        }
    }
    // port: UnitRecorder#value (char)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(DslValue::Char(*self))
    }
}
impl OptionValue for String {
    // port: ReplayValues#decode (String)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        if let DslValue::String(s) = v.untyped() {
            Ok(s.to_string_lossy())
        } else {
            Err(bad())
        }
    }
    // port: UnitRecorder#value (String)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(DslValue::String(self.as_str().into()))
    }
}
impl OptionValue for JsString {
    // port: ReplayValues#decode (WTF-16 String)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        if let DslValue::String(s) = v.untyped() {
            Ok(s.clone())
        } else {
            Err(bad())
        }
    }
    // port: UnitRecorder#value (WTF-16 String)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(DslValue::String(self.clone()))
    }
}
impl<T: OptionValue> OptionValue for Option<T> {
    // port: ReplayValues#decode (nullable field / Optional)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        match v.untyped() {
            DslValue::Null | DslValue::Optional(None) => Ok(None),
            DslValue::Optional(Some(v)) => T::decode_value(v).map(Some),
            v => T::decode_value(v).map(Some),
        }
    }
    // port: UnitRecorder#value (nullable field; Optional tagging is supplied by the declared schema)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        self.as_ref()
            .map_or(Ok(DslValue::Null), OptionValue::encode_value)
    }
}
impl<T: OptionValue> OptionValue for Vec<T> {
    // port: ReplayValues#decode (List)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        match v.untyped() {
            DslValue::List(v) | DslValue::Set(v) | DslValue::Array { items: v, .. } => {
                v.iter().map(T::decode_value).collect()
            }
            _ => Err(bad()),
        }
    }
    // port: UnitRecorder#value (List)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(DslValue::List(
            self.iter().map(T::encode_value).collect::<Result<_, _>>()?,
        ))
    }
}
impl<T: OptionValue + Eq + std::hash::Hash> OptionValue for IndexSet<T> {
    // port: ReplayValues#decode (Set)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        Ok(Vec::<T>::decode_value(v)?.into_iter().collect())
    }
    // port: UnitRecorder#value (Set)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(DslValue::Set(
            self.iter().map(T::encode_value).collect::<Result<_, _>>()?,
        ))
    }
}
impl<K: OptionValue + Eq + std::hash::Hash, V: OptionValue> OptionValue for IndexMap<K, V> {
    // port: ReplayValues#decode (Map)
    fn decode_value(value: &DslValue) -> Result<Self, Throwable> {
        if let DslValue::Map(v) = value.untyped() {
            v.iter()
                .map(|(k, v)| Ok((K::decode_value(k)?, V::decode_value(v)?)))
                .collect()
        } else {
            Err(bad())
        }
    }
    // port: UnitRecorder#value (Map)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(DslValue::Map(
            self.iter()
                .map(|(k, v)| Ok((k.encode_value()?, v.encode_value()?)))
                .collect::<Result<_, Throwable>>()?,
        ))
    }
}
impl<T: OptionValue> OptionValue for Arc<T> {
    // port: ReplayValues#decode (immutable object handle)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        T::decode_value(v).map(Arc::new)
    }
    // port: UnitRecorder#value (immutable object handle)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        self.as_ref().encode_value()
    }
}
impl OptionValue for PathBuf {
    // port: ReplayValues#decode (Path)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        Ok(String::decode_value(v)?.into())
    }
    // port: UnitRecorder#value (Path)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(DslValue::String(self.to_string_lossy().as_ref().into()))
    }
}
impl OptionValue for DefineValue {
    // port: ReplayValues#decode (define replacement literal)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        Ok(match v.untyped() {
            DslValue::Bool(b) => Self::Boolean(*b),
            DslValue::Int(i) => Self::Integer(*i),
            DslValue::Double(d) => Self::Double(*d),
            DslValue::String(s) => Self::String(s.clone()),
            _ => return Err(bad()),
        })
    }
    // port: UnitRecorder#value (define replacement literal)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(match self {
            Self::Boolean(b) => DslValue::Bool(*b),
            Self::Integer(i) => DslValue::Int(*i),
            Self::Double(d) => DslValue::Double(*d),
            Self::String(s) => DslValue::String(s.clone()),
        })
    }
}
impl OptionValue for Pattern {
    // port: ReplayValues#decode (Pattern.compile)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        if let DslValue::Regex { pattern, flags } = v.untyped() {
            Ok(Self::compile_with_flags(&pattern.to_string_lossy(), *flags))
        } else {
            Err(bad())
        }
    }
    // port: UnitRecorder#value (Pattern)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(DslValue::Regex {
            pattern: self.pattern().into(),
            flags: self.flags(),
        })
    }
}
impl OptionValue for Charset {
    // port: ReplayValues#decode (Charset.forName)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        Ok(Self::for_name(&String::decode_value(v)?))
    }
    // port: UnitRecorder#value (Charset)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(DslValue::String(self.name().into()))
    }
}
impl OptionValue for FeatureSet {
    // port: ReplayValues#decode (FeatureSet fields)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        let f = fields(v, "com.google.javascript.jscomp.parsing.parser.FeatureSet")?;
        let features = Vec::<Feature>::decode_value(field(&f, "features")?)?;
        Ok(Self::from_features(&features))
    }
    // port: UnitRecorder#value (FeatureSet fields)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        let features = self.get_features();
        let class = if features.len() > 1 {
            "com.google.common.collect.ImmutableEnumSet"
        } else if features.len() == 1 {
            "com.google.common.collect.SingletonImmutableSet"
        } else {
            "com.google.common.collect.RegularImmutableSet"
        };
        Ok(java_object(
            "com.google.javascript.jscomp.parsing.parser.FeatureSet",
            IndexMap::<_, _>::from_iter([(
                "features".into(),
                DslValue::Typed {
                    class: class.into(),
                    value: Box::new(
                        features
                            .into_iter()
                            .collect::<IndexSet<_>>()
                            .encode_value()?,
                    ),
                },
            )]),
        ))
    }
}
impl OptionValue for DependencyOptions {
    // port: ReplayValues#decode (DependencyOptions fields)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        let f = fields(v, "com.google.javascript.jscomp.DependencyOptions")?;
        Ok(Self::new(
            DependencyMode::decode_value(field(&f, "mode")?)?,
            Vec::decode_value(field(&f, "entryPoints")?)?,
        ))
    }
    // port: UnitRecorder#fields (DependencyOptions)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(java_object(
            "com.google.javascript.jscomp.DependencyOptions",
            IndexMap::<_, _>::from_iter([
                ("mode".into(), self.mode().encode_value()?),
                (
                    "entryPoints".into(),
                    DslValue::Typed {
                        class: "com.google.common.collect.RegularImmutableList".into(),
                        value: Box::new(self.entry_points().to_vec().encode_value()?),
                    },
                ),
            ]),
        ))
    }
}
impl OptionValue for ModuleIdentifier {
    // port: ReplayValues#decode (ModuleIdentifier fields)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        let f = fields(v, "com.google.javascript.jscomp.ModuleIdentifier")?;
        Ok(Self::new(
            String::decode_value(field(&f, "name")?)?,
            String::decode_value(field(&f, "closureNamespace")?)?,
            String::decode_value(field(&f, "moduleName")?)?,
        ))
    }
    // port: UnitRecorder#fields (ModuleIdentifier)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(java_object(
            "com.google.javascript.jscomp.ModuleIdentifier",
            IndexMap::<_, _>::from_iter([
                ("name".into(), self.name().to_string().encode_value()?),
                (
                    "closureNamespace".into(),
                    self.closure_namespace().to_string().encode_value()?,
                ),
                (
                    "moduleName".into(),
                    self.module_name().to_string().encode_value()?,
                ),
            ]),
        ))
    }
}
impl OptionValue for closure_jscomp::variable_map::VariableMap {
    // port: ReplayValues#decode (VariableMap fields)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        let f = fields(v, "com.google.javascript.jscomp.VariableMap")?;
        Ok(Self::new(&IndexMap::<JsString, JsString>::decode_value(
            field(&f, "map")?,
        )?))
    }
    // port: UnitRecorder#fields (VariableMap)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(java_object(
            "com.google.javascript.jscomp.VariableMap",
            IndexMap::<_, _>::from_iter([("map".into(), self.to_map().encode_value()?)]),
        ))
    }
}
impl OptionValue for closure_jscomp::source_map_input::SourceMapInput {
    // port: ReplayValues#decode (SourceMapInput fields)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        use closure_jscomp::source_map_input::SourceMapInputReplayFields;
        let f = fields(v, "com.google.javascript.jscomp.SourceMapInput")?;
        let DslValue::SourceFile(source_file) = field(&f, "sourceFile")?.untyped() else {
            return Err(bad());
        };
        let parsed_source_map = match field(&f, "parsedSourceMap")?.untyped() {
            DslValue::Null => None,
            _ => {
                return Err(Throwable::Unported(
                    "com.google.debugging.sourcemap.SourceMapConsumerV3#fields".into(),
                ));
            }
        };
        Ok(Self::from_replay_fields(SourceMapInputReplayFields {
            source_file: source_file.clone(),
            parsed_source_map,
            cached: bool::decode_value(field(&f, "cached")?)?,
        }))
    }
    // port: UnitRecorder#fields (SourceMapInput)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        let state = self.replay_fields();
        if state.parsed_source_map.is_some() {
            return Err(Throwable::Unported(
                "com.google.debugging.sourcemap.SourceMapConsumerV3#fields".into(),
            ));
        }
        Ok(java_object(
            "com.google.javascript.jscomp.SourceMapInput",
            IndexMap::<_, _>::from_iter([
                ("sourceFile".into(), DslValue::SourceFile(state.source_file)),
                ("parsedSourceMap".into(), DslValue::Null),
                ("cached".into(), state.cached.encode_value()?),
            ]),
        ))
    }
}
impl OptionValue for crate::jscomp_api::ComposeWarningsGuard {
    // port: ReplayValues#decode (reverse effective guard order)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        if let DslValue::WarningsGuard { guard, .. } = v.untyped() {
            let out = Self::new(vec![]);
            out.add_guard(guard.clone());
            Ok(out)
        } else {
            Err(bad())
        }
    }
    // port: UnitRecorder#value (ComposeWarningsGuard)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        let guards = self.get_guards();
        let encoding = crate::replay::replay_values::object([(
            "composeWarningsGuard",
            crate::replay::replay_values::object([(
                "guards",
                JsonValue::Array(guards.iter().map(encode_guard).collect::<Result<_, _>>()?),
            )]),
        )]);
        Ok(DslValue::WarningsGuard {
            guard: Arc::new(Self::new(guards.into_iter().rev().collect())),
            encoding,
        })
    }
}
impl OptionValue for Arc<dyn closure_jscomp::coding_convention::CodingConvention + Send + Sync> {
    // port: ReplayValues#decode (CodingConvention)
    fn decode_value(v: &DslValue) -> Result<Self, Throwable> {
        crate::harness_passes::decode_coding_convention(v)
    }
    // port: UnitRecorder#fields (CodingConvention)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        coding_convention_value(self.as_ref())
    }
}
// port: UnitRecorder#fields (CodingConvention native value)
pub fn encode_coding_convention(
    value: &dyn closure_jscomp::coding_convention::CodingConvention,
) -> Result<JsonValue, Throwable> {
    crate::replay::replay_values::encode(&coding_convention_value(value)?)
}

macro_rules! option_enum {
    ($ty:path, $class:expr, [$($variant:ident),* $(,)?]) => {
        impl OptionValue for $ty {
            // port: ReplayValues#enumValue (Java enum constants)
            fn decode_value(value: &DslValue) -> Result<Self, Throwable> {
                match enum_name(value)? { $(stringify!($variant) => Ok(Self::$variant),)* _=>Err(bad()) }
            }
            // port: UnitRecorder#value (Java enum constants)
            fn encode_value(&self) -> Result<DslValue, Throwable> {
                Ok(DslValue::Enum { class: $class.into(), name:format!("{self:?}") })
            }
        }
    }
}
option_enum!(
    CheckLevel,
    "com.google.javascript.jscomp.CheckLevel",
    [OFF, WARNING, ERROR]
);
option_enum!(
    Reach,
    "com.google.javascript.jscomp.CompilerOptions$Reach",
    [ALL, LOCAL_ONLY, NONE]
);
option_enum!(
    PropertyCollapseLevel,
    "com.google.javascript.jscomp.CompilerOptions$PropertyCollapseLevel",
    [ALL, NONE, MODULE_EXPORT]
);
option_enum!(
    BrowserFeaturesetYear,
    "com.google.javascript.jscomp.CompilerOptions$BrowserFeaturesetYear",
    [
        YEAR_2012, YEAR_2018, YEAR_2019, YEAR_2020, YEAR_2021, YEAR_2022, YEAR_2023, YEAR_2024,
        YEAR_2025, YEAR_2026
    ]
);
option_enum!(
    IncrementalCheckMode,
    "com.google.javascript.jscomp.CompilerOptions$IncrementalCheckMode",
    [OFF, GENERATE_IJS]
);
option_enum!(
    ExtractPrototypeMemberDeclarationsMode,
    "com.google.javascript.jscomp.CompilerOptions$ExtractPrototypeMemberDeclarationsMode",
    [OFF, USE_GLOBAL_TEMP, USE_CHUNK_TEMP, USE_IIFE]
);
option_enum!(
    OutputJs,
    "com.google.javascript.jscomp.CompilerOptions$OutputJs",
    [NONE, SENTINEL, NORMAL]
);
option_enum!(
    ConformanceReportingMode,
    "com.google.javascript.jscomp.CompilerOptions$ConformanceReportingMode",
    [
        IGNORE_LIBRARY_LEVEL_BEHAVIOR_SPECIFIED_IN_CONFIG,
        RESPECT_LIBRARY_LEVEL_BEHAVIOR_SPECIFIED_IN_CONFIG
    ]
);
option_enum!(
    ExperimentalOutputFeatureSet,
    "com.google.javascript.jscomp.CompilerOptions$ExperimentalOutputFeatureSet",
    [
        ES5_WITH_SOME_PERFORMANT_ES2015_AND_ASYNC_FUNCTIONS,
        BROWSER_2019_WITHOUT_CLASSES_AND_SPREAD
    ]
);
option_enum!(
    Es6ModuleTranspilation,
    "com.google.javascript.jscomp.CompilerOptions$Es6ModuleTranspilation",
    [
        NONE,
        RELATIVIZE_IMPORT_PATHS,
        TO_COMMON_JS_LIKE_MODULES,
        COMPILE
    ]
);
option_enum!(
    Es6SubclassTranspilation,
    "com.google.javascript.jscomp.CompilerOptions$Es6SubclassTranspilation",
    [CONCISE_UNSAFE, SAFE_REFLECT_CONSTRUCT]
);
option_enum!(
    InstrumentOption,
    "com.google.javascript.jscomp.CompilerOptions$InstrumentOption",
    [NONE, LINE_ONLY, BRANCH_ONLY, PRODUCTION]
);
option_enum!(
    ChunkOutputType,
    "com.google.javascript.jscomp.CompilerOptions$ChunkOutputType",
    [GLOBAL_NAMESPACE, ES_MODULES]
);
option_enum!(
    OptimizeLocalAccess,
    "com.google.javascript.jscomp.CompilerOptions$OptimizeLocalAccess",
    [
        DISABLED,
        DEFINING_CHUNK_ONLY,
        ALL_CHUNKS,
        ALL_CHUNKS_WITH_WRAPPED_REASSIGNABLE_SYMBOLS
    ]
);
option_enum!(
    LanguageMode,
    "com.google.javascript.jscomp.CompilerOptions$LanguageMode",
    [
        ECMASCRIPT3,
        ECMASCRIPT5,
        ECMASCRIPT5_STRICT,
        ECMASCRIPT_2015,
        ECMASCRIPT_2016,
        ECMASCRIPT_2017,
        ECMASCRIPT_2018,
        ECMASCRIPT_2019,
        ECMASCRIPT_2020,
        ECMASCRIPT_2021,
        ECMASCRIPT_2022,
        ECMASCRIPT_NEXT,
        STABLE,
        NO_TRANSPILE,
        UNSTABLE,
        UNSUPPORTED
    ]
);
option_enum!(
    DevMode,
    "com.google.javascript.jscomp.CompilerOptions$DevMode",
    [OFF, START, START_AND_END, EVERY_PASS]
);
option_enum!(
    TracerMode,
    "com.google.javascript.jscomp.CompilerOptions$TracerMode",
    [
        ALL,
        RAW_SIZE,
        AST_SIZE_AND_PRUNING,
        AST_SIZE,
        TIMING_ONLY,
        OFF
    ]
);
option_enum!(
    TweakProcessing,
    "com.google.javascript.jscomp.CompilerOptions$TweakProcessing",
    [OFF, CHECK, STRIP]
);
option_enum!(
    IsolationMode,
    "com.google.javascript.jscomp.CompilerOptions$IsolationMode",
    [NONE, IIFE]
);
option_enum!(
    SegmentOfCompilationToRun,
    "com.google.javascript.jscomp.CompilerOptions$SegmentOfCompilationToRun",
    [
        ENTIRE_COMPILATION,
        CHECKS,
        OPTIMIZATIONS_FIRST_HALF,
        OPTIMIZATIONS_SECOND_HALF,
        OPTIMIZATIONS,
        OPTIMIZATIONS_AND_FINALIZATIONS,
        FINALIZATIONS
    ]
);
option_enum!(
    AliasStringsMode,
    "com.google.javascript.jscomp.CompilerOptions$AliasStringsMode",
    [NONE, LARGE, ALL, ALL_AGGRESSIVE]
);
option_enum!(
    Environment,
    "com.google.javascript.jscomp.CompilerOptions$Environment",
    [BROWSER, CUSTOM]
);
option_enum!(
    JsonStreamMode,
    "com.google.javascript.jscomp.CompilerOptions$JsonStreamMode",
    [NONE, IN, OUT, BOTH]
);
option_enum!(
    J2clPassMode,
    "com.google.javascript.jscomp.CompilerOptions$J2clPassMode",
    [OFF, AUTO]
);
option_enum!(
    JsDocParsing,
    "com.google.javascript.jscomp.parsing.Config$JsDocParsing",
    [
        TYPES_ONLY,
        INCLUDE_DESCRIPTIONS_NO_WHITESPACE,
        INCLUDE_DESCRIPTIONS_WITH_WHITESPACE,
        INCLUDE_ALL_COMMENTS,
        LICENSE_COMMENTS_ONLY
    ]
);
option_enum!(
    Feature,
    "com.google.javascript.jscomp.parsing.parser.FeatureSet$Feature",
    [
        REGEXP_SYNTAX,
        ES3_KEYWORDS_AS_IDENTIFIERS,
        GETTER,
        KEYWORDS_AS_PROPERTIES,
        SETTER,
        STRING_CONTINUATION,
        TRAILING_COMMA,
        ARRAY_DESTRUCTURING,
        ARRAY_PATTERN_REST,
        ARROW_FUNCTIONS,
        BINARY_LITERALS,
        BLOCK_SCOPED_FUNCTION_DECLARATION,
        CLASSES,
        CLASS_GETTER_SETTER,
        COMPUTED_PROPERTIES,
        CONST_DECLARATIONS,
        DEFAULT_PARAMETERS,
        FOR_OF,
        GENERATORS,
        LET_DECLARATIONS,
        MEMBER_DECLARATIONS,
        NEW_TARGET,
        OBJECT_DESTRUCTURING,
        OCTAL_LITERALS,
        REGEXP_FLAG_U,
        REGEXP_FLAG_Y,
        REST_PARAMETERS,
        SHORTHAND_OBJECT_PROPERTIES,
        SPREAD_EXPRESSIONS,
        SUPER,
        TEMPLATE_LITERALS,
        MODULES,
        EXPONENT_OP,
        ASYNC_FUNCTIONS,
        OBJECT_LITERALS_WITH_SPREAD,
        OBJECT_PATTERN_REST,
        ASYNC_GENERATORS,
        FOR_AWAIT_OF,
        REGEXP_FLAG_S,
        REGEXP_NAMED_GROUPS,
        REGEXP_UNICODE_PROPERTY_ESCAPE,
        REGEXP_LOOKBEHIND,
        UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP,
        OPTIONAL_CATCH_BINDING,
        DYNAMIC_IMPORT,
        BIGINT,
        IMPORT_META,
        NULL_COALESCE_OP,
        OPTIONAL_CHAINING,
        NUMERIC_SEPARATOR,
        LOGICAL_ASSIGNMENT,
        PUBLIC_CLASS_FIELDS,
        CLASS_STATIC_BLOCK,
        REGEXP_FLAG_D,
        TOP_LEVEL_AWAIT,
        ES_NEXT_RUNTIME,
        ES_UNSTABLE_RUNTIME,
        PRIVATE_ELEMENTS,
        TYPE_ANNOTATION
    ]
);
option_enum!(
    DependencyMode,
    "com.google.javascript.jscomp.DependencyOptions$DependencyMode",
    [
        NONE,
        SORT_ONLY,
        PRUNE_LEGACY,
        PRUNE,
        PRUNE_ALLOW_NO_ENTRY_POINTS
    ]
);
option_enum!(
    closure_jscomp::deps::module_loader::ResolutionMode,
    "com.google.javascript.jscomp.deps.ModuleLoader$ResolutionMode",
    [BROWSER, BROWSER_WITH_TRANSFORMED_PREFIXES, NODE, WEBPACK]
);
option_enum!(
    closure_jscomp::deps::module_loader::PathEscaper,
    "com.google.javascript.jscomp.deps.ModuleLoader$PathEscaper",
    [ESCAPE, CANONICALIZE_ONLY]
);
option_enum!(
    closure_jscomp::variable_renaming_policy::VariableRenamingPolicy,
    "com.google.javascript.jscomp.VariableRenamingPolicy",
    [OFF, LOCAL, ALL]
);
option_enum!(
    closure_jscomp::property_renaming_policy::PropertyRenamingPolicy,
    "com.google.javascript.jscomp.PropertyRenamingPolicy",
    [OFF, ALL_UNQUOTED]
);
option_enum!(
    closure_jscomp::js::runtime_js_lib_manager::RuntimeLibraryMode,
    "com.google.javascript.jscomp.js.RuntimeJsLibManager$RuntimeLibraryMode",
    [
        INJECT,
        NO_OP,
        RECORD_ONLY,
        RECORD_AND_VALIDATE_FIELDS,
        EXTERN_FIELD_NAMES
    ]
);
option_enum!(
    closure_jscomp::source_map::DetailLevel,
    "com.google.javascript.jscomp.SourceMap$DetailLevel",
    [ALL, SYMBOLS]
);
option_enum!(
    closure_jscomp::source_map::Format,
    "com.google.javascript.jscomp.SourceMap$Format",
    [DEFAULT, V3]
);
option_enum!(
    closure_jscomp::error_format::ErrorFormat,
    "com.google.javascript.jscomp.ErrorFormat",
    [SINGLELINE, FULL, MULTILINE, SOURCELESS]
);
option_enum!(
    Tri,
    "com.google.javascript.jscomp.base.Tri",
    [FALSE, UNKNOWN, TRUE]
);
option_enum!(
    closure_jscomp::custom_pass_execution_time::CustomPassExecutionTime,
    "com.google.javascript.jscomp.CustomPassExecutionTime",
    [
        BEFORE_CHECKS,
        BEFORE_OPTIMIZATIONS,
        BEFORE_OPTIMIZATION_LOOP,
        AFTER_OPTIMIZATION_LOOP
    ]
);

impl OptionValue for Arc<dyn closure_jscomp::message_bundle::MessageBundle + Send + Sync> {
    // port: ReplayValues#decode (MessageBundle generic object dump)
    fn decode_value(value: &DslValue) -> Result<Self, Throwable> {
        const EMPTY: &str = "com.google.javascript.jscomp.EmptyMessageBundle";
        match value.untyped() {
            DslValue::Object(o) if o.borrow().class == EMPTY => {
                fields(value, EMPTY)?;
                Ok(Arc::new(
                    closure_jscomp::empty_message_bundle::EmptyMessageBundle,
                ))
            }
            DslValue::Object(o) => Err(Throwable::Unported(o.borrow().class.clone())),
            _ => Err(bad()),
        }
    }
    // port: UnitRecorder#fields (MessageBundle)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        let bundle: &dyn std::any::Any = self.as_ref();
        if bundle.is::<closure_jscomp::empty_message_bundle::EmptyMessageBundle>() {
            Ok(DslValue::Object(Rc::new(RefCell::new(
                crate::replay::replay_dsl::Object {
                    class: "com.google.javascript.jscomp.EmptyMessageBundle".into(),
                    fields: IndexMap::<_, _>::default(),
                    field_types: IndexMap::<_, _>::default(),
                },
            ))))
        } else {
            Err(Throwable::Unported(
                "com.google.javascript.jscomp.MessageBundle".into(),
            ))
        }
    }
}
impl OptionValue for Arc<dyn closure_jscomp::name_generator::NameGenerator + Send + Sync> {
    // port: ReplayValues#decode (DefaultNameGenerator raw fields)
    fn decode_value(value: &DslValue) -> Result<Self, Throwable> {
        if let DslValue::NameGenerator(generator) = value.untyped() {
            return Ok(generator.clone());
        }
        let fields = fields(value, "com.google.javascript.jscomp.DefaultNameGenerator")?;
        let mut generator = closure_jscomp::default_name_generator::DefaultNameGenerator::new();
        *generator.replay_fields_mut().priority_lookup_map =
            IndexMap::decode_value(field(&fields, "priorityLookupMap")?)?;
        *generator.replay_fields_mut().reserved_names = Arc::new(std::sync::RwLock::new(
            IndexSet::decode_value(field(&fields, "reservedNames")?)?,
        ));
        *generator.replay_fields_mut().prefix = JsString::decode_value(field(&fields, "prefix")?)?;
        *generator.replay_fields_mut().name_count =
            i32::decode_value(field(&fields, "nameCount")?)?;
        *generator.replay_fields_mut().first_chars =
            decode_priorities(&generator, field(&fields, "firstChars")?)?;
        *generator.replay_fields_mut().non_first_chars =
            decode_priorities(&generator, field(&fields, "nonFirstChars")?)?;
        Ok(Arc::new(generator))
    }
    // port: UnitRecorder#fields (DefaultNameGenerator live state)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        encode_name_generator(self.as_ref())
    }
}
impl OptionValue for closure_jscomp::default_name_generator::CharPriority {
    // port: ReplayValues#decode (CharPriority raw fields)
    fn decode_value(value: &DslValue) -> Result<Self, Throwable> {
        let fields = fields(
            value,
            "com.google.javascript.jscomp.DefaultNameGenerator$CharPriority",
        )?;
        let mut priority = Self::new(
            u16::decode_value(field(&fields, "name")?)?,
            i32::decode_value(field(&fields, "order")?)?,
        );
        priority.occurrence = i32::decode_value(field(&fields, "occurrence")?)?;
        Ok(priority)
    }
    // port: UnitRecorder#fields (CharPriority)
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        Ok(java_object(
            "com.google.javascript.jscomp.DefaultNameGenerator$CharPriority",
            IndexMap::<_, _>::from_iter([
                ("name".into(), self.name.encode_value()?),
                ("occurrence".into(), self.occurrence.encode_value()?),
                ("order".into(), self.order.encode_value()?),
            ]),
        ))
    }
}
// port: ReplayValues#decode (native CharPriority array representation)
fn decode_priorities(
    generator: &closure_jscomp::default_name_generator::DefaultNameGenerator,
    value: &DslValue,
) -> Result<Vec<usize>, Throwable> {
    let priorities =
        Vec::<closure_jscomp::default_name_generator::CharPriority>::decode_value(value)?;
    priorities.into_iter().map(|priority| {
        let index=generator.replay_fields().priority_lookup_map.get_index_of(&priority.name).ok_or_else(bad)?;
        let original=&generator.replay_fields().priority_lookup_map[&priority.name];
        if original.occurrence != priority.occurrence || original.order != priority.order {
            return Err(Throwable::Unported("com.google.javascript.jscomp.DefaultNameGenerator$CharPriority (independent array entries)".into()));
        }
        Ok(index)
    }).collect()
}
// port: UnitRecorder#fields (DefaultNameGenerator arrays)
fn encode_priorities(
    generator: &closure_jscomp::default_name_generator::DefaultNameGenerator,
    indices: &[usize],
) -> Result<DslValue, Throwable> {
    Ok(DslValue::Array {
        component: "com.google.javascript.jscomp.DefaultNameGenerator$CharPriority".into(),
        items: indices
            .iter()
            .map(|&index| {
                generator
                    .replay_fields()
                    .priority_lookup_map
                    .get_index(index)
                    .expect("CharPriority index")
                    .1
                    .encode_value()
            })
            .collect::<Result<_, _>>()?,
    })
}
// port: UnitRecorder#fields (DefaultNameGenerator live fields)
pub fn encode_name_generator(
    value: &dyn closure_jscomp::name_generator::NameGenerator,
) -> Result<DslValue, Throwable> {
    let any: &dyn std::any::Any = value;
    let generator = any
        .downcast_ref::<closure_jscomp::default_name_generator::DefaultNameGenerator>()
        .ok_or_else(|| {
            Throwable::Unported("com.google.javascript.jscomp.NameGenerator#fields".into())
        })?;
    Ok(java_object(
        "com.google.javascript.jscomp.DefaultNameGenerator",
        IndexMap::<_, _>::from_iter([
            (
                "priorityLookupMap".into(),
                generator
                    .replay_fields()
                    .priority_lookup_map
                    .encode_value()?,
            ),
            (
                "reservedNames".into(),
                generator
                    .replay_fields()
                    .reserved_names
                    .read()
                    .unwrap()
                    .encode_value()?,
            ),
            (
                "prefix".into(),
                generator.replay_fields().prefix.encode_value()?,
            ),
            (
                "nameCount".into(),
                generator.replay_fields().name_count.encode_value()?,
            ),
            (
                "firstChars".into(),
                encode_priorities(generator, generator.replay_fields().first_chars)?,
            ),
            (
                "nonFirstChars".into(),
                encode_priorities(generator, generator.replay_fields().non_first_chars)?,
            ),
        ]),
    ))
}
// port: UnitRecorder#fields (CodingConvention declaring class)
pub fn coding_convention_class(
    value: &dyn closure_jscomp::coding_convention::CodingConvention,
) -> &'static str {
    let any: &dyn std::any::Any = value;
    if any.is::<closure_jscomp::google_coding_convention::GoogleCodingConvention>() {
        "com.google.javascript.jscomp.GoogleCodingConvention"
    } else if any.is::<closure_jscomp::closure_coding_convention::ClosureCodingConvention>() {
        "com.google.javascript.jscomp.ClosureCodingConvention"
    } else if any.is::<closure_jscomp::coding_conventions::DefaultCodingConvention>() {
        "com.google.javascript.jscomp.CodingConventions$DefaultCodingConvention"
    } else {
        "com.google.javascript.jscomp.CodingConvention"
    }
}
// port: UnitRecorder#fields (CodingConvention proxy fields)
fn coding_convention_value(
    value: &dyn closure_jscomp::coding_convention::CodingConvention,
) -> Result<DslValue, Throwable> {
    let any: &dyn std::any::Any = value;
    let next = if let Some(google) =
        any.downcast_ref::<closure_jscomp::google_coding_convention::GoogleCodingConvention>()
    {
        Some(google.replay_fields().proxy.replay_fields().next_convention)
    } else if let Some(closure) =
        any.downcast_ref::<closure_jscomp::closure_coding_convention::ClosureCodingConvention>()
    {
        Some(
            closure
                .replay_fields()
                .proxy
                .replay_fields()
                .next_convention,
        )
    } else if any.is::<closure_jscomp::coding_conventions::DefaultCodingConvention>() {
        None
    } else {
        return Err(Throwable::Unported(
            "com.google.javascript.jscomp.CodingConvention#fields".into(),
        ));
    };
    let mut fields = IndexMap::<_, _>::default();
    if let Some(next) = next {
        fields.insert(
            "Proxy.nextConvention".into(),
            coding_convention_value(next.as_ref())?,
        );
    }
    Ok(java_object(coding_convention_class(value), fields))
}
// port: UnitRecorder#value (WarningsGuard native fields)
fn encode_guard(guard: &Arc<dyn crate::jscomp_api::WarningsGuard>) -> Result<JsonValue, Throwable> {
    if let Some(group) = guard
        .as_any()
        .downcast_ref::<crate::jscomp_api::DiagnosticGroupWarningsGuard>()
    {
        return crate::replay::replay_values::encode(&java_object(
            "com.google.javascript.jscomp.DiagnosticGroupWarningsGuard",
            IndexMap::<_, _>::from_iter([
                (
                    "group".into(),
                    DslValue::DiagnosticGroup(group.replay_fields().group.clone()),
                ),
                (
                    "level".into(),
                    DslValue::Enum {
                        class: "com.google.javascript.jscomp.CheckLevel".into(),
                        name: format!("{:?}", group.replay_fields().level),
                    },
                ),
            ]),
        ));
    }
    if guard
        .as_any()
        .is::<closure_jscomp::strict_warnings_guard::StrictWarningsGuard>()
    {
        return crate::replay::replay_values::encode(&java_object(
            "com.google.javascript.jscomp.StrictWarningsGuard",
            IndexMap::<_, _>::default(),
        ));
    }
    if guard
        .as_any()
        .is::<crate::replay::disambiguate_test_helpers::SilenceNoiseGuard>()
    {
        return crate::replay::replay_values::encode(&java_object(
            crate::replay::disambiguate_test_helpers::SilenceNoiseGuard::CLASS,
            IndexMap::<_, _>::default(),
        ));
    }
    if guard
        .as_any()
        .is::<crate::replay::disambiguate_test_helpers::SilenceChecksWarningsGuard>()
    {
        return crate::replay::replay_values::encode(&java_object(
            crate::replay::disambiguate_test_helpers::SilenceChecksWarningsGuard::CLASS,
            IndexMap::<_, _>::default(),
        ));
    }
    if let Some(path) = guard
        .as_any()
        .downcast_ref::<closure_jscomp::by_path_warnings_guard::ByPathWarningsGuard>()
    {
        return crate::replay::replay_values::encode(&encode_path_guard(path)?);
    }
    if let Some(show) = guard
        .as_any()
        .downcast_ref::<closure_jscomp::show_by_path_warnings_guard::ShowByPathWarningsGuard>(
    ) {
        return crate::replay::replay_values::encode(&java_object(
            "com.google.javascript.jscomp.ShowByPathWarningsGuard",
            IndexMap::<_, _>::from_iter([(
                "warningsGuard".into(),
                encode_path_guard(show.replay_fields().warnings_guard)?,
            )]),
        ));
    }
    Err(Throwable::Unported(
        "com.google.javascript.jscomp.WarningsGuard#fields".into(),
    ))
}

// port: ReplayValues#decode (native path and strict warning guards)
pub fn decode_guard(
    value: &DslValue,
) -> Result<Arc<dyn crate::jscomp_api::WarningsGuard>, Throwable> {
    use closure_jscomp::{
        by_path_warnings_guard::ByPathWarningsGuard,
        show_by_path_warnings_guard::ShowByPathWarningsGuard,
        strict_warnings_guard::StrictWarningsGuard,
    };
    let DslValue::Object(object) = value else {
        return Err(bad());
    };
    let object = object.borrow();
    Ok(match object.class.as_str() {
        "com.google.javascript.jscomp.StrictWarningsGuard" => Arc::new(StrictWarningsGuard),
        crate::replay::disambiguate_test_helpers::SilenceNoiseGuard::CLASS => {
            Arc::new(crate::replay::disambiguate_test_helpers::SilenceNoiseGuard)
        }
        crate::replay::disambiguate_test_helpers::SilenceChecksWarningsGuard::CLASS => {
            Arc::new(crate::replay::disambiguate_test_helpers::SilenceChecksWarningsGuard)
        }
        "com.google.javascript.jscomp.ByPathWarningsGuard" => {
            let mut path = ByPathWarningsGuard::for_path(Vec::new(), CheckLevel::OFF);
            let state = path.replay_fields_mut();
            *state.paths = Vec::decode_value(field(&object.fields, "paths")?)?;
            *state.include = bool::decode_value(field(&object.fields, "include")?)?;
            *state.priority = i32::decode_value(field(&object.fields, "priority")?)?;
            *state.level = CheckLevel::decode_value(field(&object.fields, "level")?)?;
            Arc::new(path)
        }
        "com.google.javascript.jscomp.ShowByPathWarningsGuard" => {
            let DslValue::WarningsGuard { guard, .. } = field(&object.fields, "warningsGuard")?
            else {
                return Err(bad());
            };
            let path = guard
                .as_any()
                .downcast_ref::<ByPathWarningsGuard>()
                .ok_or_else(bad)?;
            let original = path.replay_fields();
            let mut copy = ByPathWarningsGuard::for_path(Vec::new(), CheckLevel::OFF);
            let state = copy.replay_fields_mut();
            *state.paths = original.paths.clone();
            *state.include = *original.include;
            *state.priority = *original.priority;
            *state.level = *original.level;
            let mut show = ShowByPathWarningsGuard::new("");
            *show.replay_fields_mut().warnings_guard = copy;
            Arc::new(show)
        }
        class => return Err(Throwable::Unported(class.into())),
    })
}

// port: UnitRecorder#fields (native ByPathWarningsGuard)
fn encode_path_guard(
    path: &closure_jscomp::by_path_warnings_guard::ByPathWarningsGuard,
) -> Result<DslValue, Throwable> {
    let class = if path.replay_fields().paths.len() == 1 {
        "com.google.common.collect.SingletonImmutableList"
    } else {
        "com.google.common.collect.RegularImmutableList"
    };
    Ok(java_object(
        "com.google.javascript.jscomp.ByPathWarningsGuard",
        IndexMap::<_, _>::from_iter([
            (
                "paths".into(),
                DslValue::Typed {
                    class: class.into(),
                    value: Box::new(path.replay_fields().paths.encode_value()?),
                },
            ),
            (
                "include".into(),
                path.replay_fields().include.encode_value()?,
            ),
            (
                "priority".into(),
                path.replay_fields().priority.encode_value()?,
            ),
            ("level".into(), path.replay_fields().level.encode_value()?),
        ]),
    ))
}

option_enum!(
    closure_rhino::token::Token,
    "com.google.javascript.rhino.Token",
    [
        RETURN,
        BITOR,
        BITXOR,
        BITAND,
        EQ,
        NE,
        LT,
        LE,
        GT,
        GE,
        LSH,
        RSH,
        URSH,
        ADD,
        SUB,
        MUL,
        DIV,
        MOD,
        EXPONENT,
        NOT,
        BITNOT,
        POS,
        NEG,
        NEW,
        DELPROP,
        TYPEOF,
        GETPROP,
        GETELEM,
        CALL,
        OPTCHAIN_GETPROP,
        OPTCHAIN_GETELEM,
        OPTCHAIN_CALL,
        NAME,
        NUMBER,
        BIGINT,
        STRINGLIT,
        NULL,
        THIS,
        FALSE,
        TRUE,
        SHEQ,
        SHNE,
        REGEXP,
        THROW,
        IN,
        INSTANCEOF,
        ARRAYLIT,
        OBJECTLIT,
        TRY,
        PARAM_LIST,
        COMMA,
        ASSIGN,
        ASSIGN_BITOR,
        ASSIGN_BITXOR,
        ASSIGN_BITAND,
        ASSIGN_LSH,
        ASSIGN_RSH,
        ASSIGN_URSH,
        ASSIGN_ADD,
        ASSIGN_SUB,
        ASSIGN_MUL,
        ASSIGN_DIV,
        ASSIGN_MOD,
        ASSIGN_EXPONENT,
        ASSIGN_OR,
        ASSIGN_AND,
        ASSIGN_COALESCE,
        HOOK,
        OR,
        AND,
        COALESCE,
        INC,
        DEC,
        FUNCTION,
        IF,
        SWITCH,
        CASE,
        DEFAULT_CASE,
        WHILE,
        DO,
        FOR,
        FOR_IN,
        BREAK,
        CONTINUE,
        VAR,
        WITH,
        CATCH,
        VOID,
        EMPTY,
        ROOT,
        BLOCK,
        SWITCH_BODY,
        LABEL,
        EXPR_RESULT,
        SCRIPT,
        GETTER_DEF,
        SETTER_DEF,
        CONST,
        DEBUGGER,
        LABEL_NAME,
        STRING_KEY,
        CAST,
        ARRAY_PATTERN,
        OBJECT_PATTERN,
        DESTRUCTURING_LHS,
        CLASS,
        CLASS_MEMBERS,
        MEMBER_FUNCTION_DEF,
        MEMBER_FIELD_DEF,
        COMPUTED_FIELD_DEF,
        SUPER,
        LET,
        FOR_OF,
        FOR_AWAIT_OF,
        YIELD,
        AWAIT,
        IMPORT,
        IMPORT_SPECS,
        IMPORT_SPEC,
        IMPORT_STAR,
        EXPORT,
        EXPORT_SPECS,
        EXPORT_SPEC,
        MODULE_BODY,
        DYNAMIC_IMPORT,
        ITER_REST,
        OBJECT_REST,
        ITER_SPREAD,
        OBJECT_SPREAD,
        COMPUTED_PROP,
        TAGGED_TEMPLATELIT,
        TEMPLATELIT,
        TEMPLATELIT_SUB,
        TEMPLATELIT_STRING,
        DEFAULT_VALUE,
        NEW_TARGET,
        IMPORT_META,
        STRING_TYPE,
        BOOLEAN_TYPE,
        NUMBER_TYPE,
        FUNCTION_TYPE,
        PARAMETERIZED_TYPE,
        UNION_TYPE,
        ANY_TYPE,
        NULLABLE_TYPE,
        VOID_TYPE,
        REST_PARAMETER_TYPE,
        NAMED_TYPE,
        OPTIONAL_PARAMETER,
        RECORD_TYPE,
        UNDEFINED_TYPE,
        ARRAY_TYPE,
        GENERIC_TYPE,
        GENERIC_TYPE_LIST,
        ANNOTATION,
        PIPE,
        STAR,
        EOC,
        QMARK,
        BANG,
        EQUALS,
        LB,
        LC,
        COLON,
        INTERFACE,
        INTERFACE_EXTENDS,
        INTERFACE_MEMBERS,
        ENUM,
        ENUM_MEMBERS,
        IMPLEMENTS,
        TYPE_ALIAS,
        DECLARE,
        MEMBER_VARIABLE_DEF,
        INDEX_SIGNATURE,
        CALL_SIGNATURE,
        NAMESPACE,
        NAMESPACE_ELEMENTS,
        PLACEHOLDER1,
        PLACEHOLDER2,
        PLACEHOLDER3
    ]
);

// Captured option objects whose concrete classes are helper implementations of an interface
// (CssRenamingMap, RenamingMap, Xid.HashFunction, ErrorReportGenerator, LocationMapping,
// ErrorHandler, CompilerPass, stream wrappers). A record holds them only as an opaque object, so
// a non-null value reports its interface as unported; null values and empty lists decode
// normally (every corpus record has only those).
macro_rules! unavailable_value {
    ($ty:ty, $class:expr) => {
        impl OptionValue for $ty {
            fn decode_value(_value: &DslValue) -> Result<Self, Throwable> {
                Err(Throwable::Unported($class.into()))
            }
            fn encode_value(&self) -> Result<DslValue, Throwable> {
                Err(Throwable::Unported($class.into()))
            }
        }
    };
}
unavailable_value!(
    Arc<dyn closure_jscomp::css_renaming_map::CssRenamingMap + Send + Sync>,
    "com.google.javascript.jscomp.CssRenamingMap"
);
// RenamingMap values: the RenamingToken enum constants (CompilerOptions#setIdGenerators(Set)
// stores RenamingToken.INCONSISTENT) and a RenamingMap the replay constructs itself (a helper
// class, e.g. IntegrationTest_Helpers.XidRenamingMap); a captured opaque one stays unported.
impl OptionValue for Arc<dyn closure_jscomp::renaming_map::RenamingMap + Send + Sync> {
    fn decode_value(value: &DslValue) -> Result<Self, Throwable> {
        use closure_jscomp::renaming_token::RenamingToken;
        match value {
            DslValue::Enum { class, name }
                if class == "com.google.javascript.jscomp.RenamingToken" =>
            {
                if let Some(token) = RenamingToken::value_of(name) {
                    return Ok(Arc::new(token));
                }
            }
            DslValue::Native(object) => {
                if let Some(map) = object.borrow_mut().as_renaming_map() {
                    return Ok(map);
                }
            }
            _ => {}
        }
        Err(Throwable::Unported(
            "com.google.javascript.jscomp.RenamingMap".into(),
        ))
    }
    fn encode_value(&self) -> Result<DslValue, Throwable> {
        use closure_jscomp::renaming_token::RenamingToken;
        if let Some(token) = (self.as_ref() as &dyn std::any::Any).downcast_ref::<RenamingToken>() {
            return Ok(DslValue::Enum {
                class: "com.google.javascript.jscomp.RenamingToken".into(),
                name: token.to_string(),
            });
        }
        Err(Throwable::Unported(
            "com.google.javascript.jscomp.RenamingMap".into(),
        ))
    }
}
unavailable_value!(
    Arc<dyn closure_jscomp::xid::HashFunction + Send + Sync>,
    "com.google.javascript.jscomp.Xid$HashFunction"
);
unavailable_value!(
    Arc<std::sync::Mutex<dyn closure_jscomp::sorting_error_manager::ErrorReportGenerator + Send>>,
    "com.google.javascript.jscomp.SortingErrorManager$ErrorReportGenerator"
);
unavailable_value!(
    Arc<dyn closure_jscomp::source_map::LocationMapping + Send + Sync>,
    "com.google.javascript.jscomp.SourceMap$LocationMapping"
);
unavailable_value!(
    Arc<std::sync::Mutex<dyn closure_jscomp::error_handler::ErrorHandler + Send>>,
    "com.google.javascript.jscomp.ErrorHandler"
);
unavailable_value!(CompilerPassRef, "com.google.javascript.jscomp.CompilerPass");
unavailable_value!(OutputStreamWrapper, "java.util.function.Function");
unavailable_value!(InputStreamWrapper, "java.util.function.Function");
