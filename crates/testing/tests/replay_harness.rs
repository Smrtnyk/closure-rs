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
/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2022 The Closure Compiler Authors.
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
//   oracle/replay/src/com/google/javascript/jscomp/ReplayBridge.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayCompilerTest.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/CompilerOptions.java,
//   src/com/google/javascript/jscomp/JSError.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/UnitTestUtils.java.

use closure_rhino::js_string::JsString;
use closure_testing::{
    compiler_test_case::{
        CompilerTestCase, Diagnostic, Expected, FlatSources, contains_exactly,
        normalize_string_comparison,
    },
    corpus,
    descriptor::{Case, Descriptor},
    dsl::Expr,
    jscomp_api::{
        CheckLevel, DiagnosticType, JSError, SourceFile, StaticSourceFile, WarningsGuard,
    },
    json::{JsonValue, parse_json},
    replay::{
        options_fields,
        registry::Registry,
        replay_compiler_test::{canonical_class_names, expected_test_fields_after},
        replay_dsl::{Ctx, DslValue, Object, eval, invoke_lambda},
        replay_main::{
            ReplayResult, compare, neutral_equal, post_state, postcondition_counts, replay_one,
        },
        replay_values::{decode_json, errors, object},
    },
    throwable::Throwable,
    unit_recorder::ref_value,
    unit_test_utils,
};
use indexmap::IndexMap;
use std::{cell::RefCell, rc::Rc, sync::Arc};

static ERROR: DiagnosticType = DiagnosticType::error("JSC_TEST", "{0}");
static SAME_KEY: DiagnosticType = DiagnosticType::warning("JSC_TEST", "different format");

// port: ReplayValues#decode (test fixture)
fn json(s: &str) -> JsonValue {
    parse_json(s).unwrap()
}
// port: ReplayDsl#eval (test fixture)
fn expression(s: &str) -> Expr {
    Expr::from_json(&json(s), "$").unwrap()
}
// port: ReplayDsl.Ctx#Ctx (test fixture)
fn context(tsv: &str) -> Ctx {
    Ctx::new(
        "Fixture".into(),
        object([]),
        IndexMap::new(),
        Registry::from_tsv(tsv).unwrap(),
    )
}
// port: JSError#make(String,int,int,DiagnosticType,String...) (test fixture)
fn error(message: &str) -> JSError {
    let mut e = JSError::make_with_source_location("test.js", 2, 3, &ERROR, &[message]);
    e.length = 4;
    e
}
// port: CompilerTestCase.Diagnostic#withMessage / withMessageContaining / withLocation
#[test]
fn diagnostic_message_location_and_key_equality() {
    let d = Diagnostic::new(CheckLevel::WARNING, &SAME_KEY)
        .with_message(" \tfoo\n ")
        .unwrap()
        .with_location(2, 3, 4);
    assert!(d.format_diff(&error("\rfoo\t")).is_none());
    assert!(d.format_diff(&error("foo bar")).is_some());
    assert!(
        d.clone()
            .with_location(2, 3, 5)
            .format_diff(&error("foo"))
            .unwrap()
            .contains("length")
    );
    assert!(
        Diagnostic::new(CheckLevel::ERROR, &ERROR)
            .with_message("foo")
            .unwrap()
            .format_diff(&error("\u{a0}foo\u{a0}"))
            .is_some()
    );
    let contains = Diagnostic::new(CheckLevel::ERROR, &ERROR)
        .with_message_containing("foo")
        .unwrap();
    assert!(contains.format_diff(&error("prefix foo suffix")).is_none());
    assert!(
        matches!(contains.with_message("foo"), Err(Throwable::Exception { class, .. }) if class=="java.lang.IllegalStateException")
    );
    // Java creates a new Diagnostic with default location when adding a message predicate.
    assert!(
        Diagnostic::new(CheckLevel::ERROR, &ERROR)
            .with_location(99, 99, 99)
            .with_message("foo")
            .unwrap()
            .format_diff(&error("foo"))
            .is_none()
    );
}
// port: CompilerTestCase#DIAGNOSTIC_CORRESPONDENCE
#[test]
fn diagnostic_exact_pairing_preserves_multiplicity_and_order() {
    let broad = Diagnostic::new(CheckLevel::ERROR, &ERROR)
        .with_message_containing("foo")
        .unwrap();
    let narrow = Diagnostic::new(CheckLevel::ERROR, &ERROR)
        .with_message("foo")
        .unwrap();
    let actual = [error("foo"), error("foo bar")];
    contains_exactly(&actual, &[broad.clone(), narrow.clone()], false, "errors").unwrap();
    assert!(contains_exactly(&actual, &[broad, narrow.clone()], true, "errors").is_err());
    assert!(contains_exactly(&actual, &[narrow.clone(), narrow.clone()], false, "errors").is_err());
    contains_exactly(
        &[error("foo"), error("foo")],
        &[narrow.clone(), narrow],
        false,
        "errors",
    )
    .unwrap();
}
// port: CompilerTestCase#setUp / enableNormalize / enableTypeCheck
#[test]
fn harness_setters_preserve_java_prerequisites_and_side_effects() {
    let mut h = CompilerTestCase::new("");
    assert!(matches!(
        h.enable_normalize(),
        Err(Throwable::Exception { .. })
    ));
    h.set_up();
    assert!(h.compare_as_tree && h.compare_js_doc && h.check_ast_change_marking);
    assert!(h.enable_normalize_expected_output().is_err());
    h.enable_normalize().unwrap();
    assert!(h.multistage_compilation);
    h.enable_normalize_expected_output().unwrap();
    h.enable_type_check().unwrap();
    assert!(h.create_module_map);
    h.disable_type_check().unwrap();
    assert!(h.create_module_map);
    assert!(h.create_compiler().is_ok());
    h.tear_down();
    assert!(h.enable_type_check().is_err());
}
// port: CompilerTestCase#compareExpectedToActualAsStrings
#[test]
fn string_comparison_strips_only_spaces_before_linefeeds() {
    let input = JsString::from_units(vec![
        b'a' as u16,
        32,
        32,
        10,
        9,
        32,
        10,
        32,
        13,
        10,
        0xd800,
        32,
    ]);
    assert_eq!(
        normalize_string_comparison(&input).as_units(),
        &[b'a' as u16, 10, 9, 10, 32, 13, 10, 0xd800, 32]
    );
}
// port: UnitTestUtils#updateGenericVarNamesInExpectedFiles
#[test]
fn generic_names_use_wrapping_hash_and_ordered_plain_replacements() {
    let filename = "polygenelubricants";
    assert_eq!(JsString::from(filename).hash_code(), i32::MIN);
    let inputs = FlatSources {
        sources: vec![Arc::new(SourceFile::from_code(filename, ""))],
    };
    let outputs = Expected {
        expected: Some(vec![Arc::new(SourceFile::from_code("out.js", "x xx"))]),
        same: false,
    };
    let prefixes = IndexMap::from([("x".into(), "g".into()), ("g".into(), "z".into())]);
    let mut reverse = IndexMap::new();
    let got = unit_test_utils::update_generic_var_names_in_expected_files(
        &inputs,
        &outputs,
        &prefixes,
        &mut reverse,
    )
    .unwrap();
    assert_eq!(got[0].get_name(), "out.js");
    assert_eq!(
        got[0].get_code().unwrap().to_string_lossy(),
        "zm-2147483648m-2147483648 zm-2147483648m-2147483648zm-2147483648m-2147483648"
    );
    assert_eq!(
        reverse.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["gm-2147483648", "zm-2147483648"]
    );
    assert_eq!(
        unit_test_utils::replace(
            &JsString::from("aaaaa"),
            &JsString::from("aa"),
            &JsString::from("b")
        ),
        JsString::from("bba")
    );
    assert_eq!(
        unit_test_utils::replace(
            &JsString::from("ab"),
            &JsString::from(""),
            &JsString::from("x")
        ),
        JsString::from("xaxbx")
    );
    let negative = FlatSources {
        sources: vec![Arc::new(SourceFile::from_code("zzzzzz", ""))],
    };
    let hash = JsString::from("zzzzzz").hash_code();
    assert!(hash < 0 && hash != i32::MIN);
    let got = unit_test_utils::update_generic_var_names(
        &negative,
        &outputs,
        &IndexMap::from([("x".into(), "v".into())]),
    )
    .unwrap();
    assert!(
        got[0]
            .get_code()
            .unwrap()
            .to_string_lossy()
            .starts_with(&format!("vm{}", hash.wrapping_neg()))
    );
}
// port: CompilerOptions#CompilerOptions / ReplayValues#setField / ReplayBridge#options / setField
#[test]
fn all_205_option_defaults_decode_and_round_trip() {
    let mut ctx = context("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n");
    let options =
        closure_testing::replay::replay_bridge::options(&IndexMap::new(), &mut ctx).unwrap();
    let handle = Rc::new(RefCell::new(options));
    let instance = DslValue::Options(handle.clone());
    assert_eq!(options_fields::defaults().fields.len(), 205);
    let differences = options_fields::defaults()
        .fields
        .iter()
        .filter_map(
            |(name, want)| match options_fields::get_field(&handle.borrow(), name) {
                Ok(got) if got.gson_equals(&want.to_json()) => None,
                Ok(got) => Some(format!(
                    "{name}: {} != {}",
                    got.to_json_string(),
                    want.to_json().to_json_string()
                )),
                Err(error) => Some(format!("{name}: {error}")),
            },
        )
        .collect::<Vec<_>>();
    assert!(
        differences.is_empty(),
        "native CompilerOptions defaults differ:\n{}",
        differences.join("\n")
    );
    for (name, want) in &options_fields::defaults().fields {
        closure_testing::replay::replay_bridge::set_field(
            &instance,
            "com.google.javascript.jscomp.CompilerOptions",
            name,
            want,
            &ctx,
        )
        .unwrap();
        let got = options_fields::get_field(&handle.borrow(), name).unwrap();
        assert!(
            got.gson_equals(&want.to_json()),
            "{name}: {} != {}",
            got.to_json_string(),
            want.to_json().to_json_string()
        );
    }
    let mut options = handle.borrow().clone();
    assert!(matches!(
        options_fields::set_field(
            &mut options,
            "unknown",
            &closure_testing::value::Value::Bool(true),
            &IndexMap::new()
        ),
        Err(Throwable::HarnessError(_))
    ));
    assert!(matches!(
        options_fields::set_field(
            &mut options,
            "checkTypes",
            &closure_testing::value::Value::Null,
            &IndexMap::new()
        ),
        Err(Throwable::HarnessError(_))
    ));
}
// port: ReplayValues#decode / adapt
#[test]
fn value_slots_keep_utf16_numeric_widths_and_live_diagnostic_types() {
    let m = IndexMap::from([("Old".into(), "New".into())]);
    let decode = |s: &str, t: &str| decode_json(&json(s), t, &m).unwrap();
    assert!(matches!(decode("257", "byte"), DslValue::Int(1)));
    assert!(matches!(decode("65535", "short"), DslValue::Int(-1)));
    assert!(matches!(decode("4294967297", "int"), DslValue::Int(1)));
    assert!(matches!(decode("1e20", "int"), DslValue::Int(1661992960)));
    assert!(matches!(decode("-123.99", "int"), DslValue::Int(-123)));
    assert!(matches!(decode("1234e-2", "int"), DslValue::Int(12)));
    assert!(matches!(decode("1e100", "int"), DslValue::Int(0)));
    assert!(matches!(decode("3", "long"), DslValue::Long(3)));
    assert!(matches!(
        decode(r#"{"long":"9223372036854775807"}"#, "long"),
        DslValue::Long(i64::MAX)
    ));
    assert!(matches!(decode(r#"{"double":"NaN"}"#,"double"),DslValue::Double(d) if d.is_nan()));
    assert!(
        matches!(decode(r#"{"double":"1.1"}"#,"float"),DslValue::Double(d) if d==f64::from(1.1f32))
    );
    assert!(matches!(
        decode(r#"{"char":"\ud800"}"#, "char"),
        DslValue::Char(0xd800)
    ));
    assert!(
        matches!(decode(r#"{"enum":"Old","name":"X"}"#,"Old"),DslValue::Enum {class,..} if class=="New")
    );
    assert!(
        matches!(decode(r#"{"classRef":"Old"}"#,"java.lang.Class"),DslValue::Class(c) if c=="New")
    );
    assert!(
        matches!(decode(r#"{"arrayOf":"int","items":[1,2]}"#,"int[]"),DslValue::Array {items,..} if items.len()==2)
    );
    assert!(
        matches!(decode(r#"{"object":"com.google.common.base.Present","fields":{"reference":3}}"#,"java.lang.Object"),DslValue::Optional(Some(v)) if matches!(*v,DslValue::Int(3)))
    );
    let value = decode(
        r#"{"diagnosticType":{"key":"JSC_TEST","level":"WARNING"}}"#,
        "java.lang.Object",
    );
    assert!(matches!(value,DslValue::DiagnosticType(t) if t==&ERROR));
    assert!(matches!(
        decode_json(&json(r#"{"ref":"Old"}"#), "java.lang.Object", &m),
        Err(Throwable::HarnessError(_))
    ));
}
// port: ReplayValues#decode (composeWarningsGuard order)
#[test]
fn recorded_guard_iteration_order_is_rebuilt_in_reverse() {
    let group = r#"{"diagnosticGroup":{"group":null,"types":["JSC_TEST"]}}"#;
    let guard = |level: &str| {
        format!(
            r#"{{"object":"com.google.javascript.jscomp.DiagnosticGroupWarningsGuard","fields":{{"group":{group},"level":{{"enum":"com.google.javascript.jscomp.CheckLevel","name":"{level}"}}}}}}"#
        )
    };
    let raw = json(&format!(
        r#"{{"composeWarningsGuard":{{"guards":[{},{}]}}}}"#,
        guard("OFF"),
        guard("WARNING")
    ));
    let v = decode_json(
        &raw,
        "com.google.javascript.jscomp.ComposeWarningsGuard",
        &IndexMap::new(),
    )
    .unwrap();
    let DslValue::WarningsGuard { guard, .. } = v else {
        panic!("guard")
    };
    assert_eq!(guard.level(&error("x")), Some(CheckLevel::OFF));
}
// port: ReplayDsl#eval / lambda
// port: ReplayValues#decode (native path and strict warning guards)
#[test]
fn native_path_and_strict_guards_preserve_fields_and_apply_levels() {
    let raw = json(
        r#"{"composeWarningsGuard":{"guards":[{"object":"com.google.javascript.jscomp.ShowByPathWarningsGuard","fields":{"warningsGuard":{"object":"com.google.javascript.jscomp.ByPathWarningsGuard","fields":{"paths":{"list":["selected"],"impl":"com.google.common.collect.SingletonImmutableList"},"include":false,"priority":1,"level":{"enum":"com.google.javascript.jscomp.CheckLevel","name":"OFF"}}}}},{"object":"com.google.javascript.jscomp.StrictWarningsGuard","fields":{}}]}}"#,
    );
    let mut options = closure_testing::jscomp_api::CompilerOptions::new();
    options_fields::set_field(
        &mut options,
        "warningsGuard",
        &closure_testing::value::Value::from_json(&raw, "$").unwrap(),
        &IndexMap::new(),
    )
    .unwrap();
    assert!(neutral_equal(
        &options_fields::get_field(&options, "warningsGuard").unwrap(),
        &raw
    ));
    let selected = JSError::make_with_source_location("selected.js", 1, 0, &SAME_KEY, &[]);
    let other = JSError::make_with_source_location("other.js", 1, 0, &SAME_KEY, &[]);
    assert_eq!(
        options.get_warnings_guard().level(&selected),
        Some(CheckLevel::ERROR)
    );
    assert_eq!(
        options.get_warnings_guard().level(&other),
        Some(CheckLevel::OFF)
    );
}
// port: ReplayValues#decode (VariableMap and SourceMapInput fields)
#[test]
fn variable_map_and_source_map_input_options_round_trip_their_captured_fields() {
    let variable_map = json(
        r#"{"object":"com.google.javascript.jscomp.VariableMap","fields":{"map":{"map":[["b","a"],["a","b"]],"impl":"com.google.common.collect.RegularImmutableBiMap"}}}"#,
    );
    let source_maps = json(
        r#"{"map":[["in.js",{"object":"com.google.javascript.jscomp.SourceMapInput","fields":{"sourceFile":{"sourceFile":{"name":"in.js.map","code":"{}","kind":"STRONG"}},"parsedSourceMap":null,"cached":false}}]],"impl":"java.util.HashMap"}"#,
    );
    let mut options = closure_testing::jscomp_api::CompilerOptions::new();
    for (name, raw) in [
        ("inputVariableMap", &variable_map),
        ("inputSourceMaps", &source_maps),
    ] {
        options_fields::set_field(
            &mut options,
            name,
            &closure_testing::value::Value::from_json(raw, "$").unwrap(),
            &IndexMap::new(),
        )
        .unwrap();
    }
    // The VariableMap dump's field is itself named "map", so compare the object and its field
    // container separately (the neutral projection reads a "map" key as a container tag).
    let got = options_fields::get_field(&options, "inputVariableMap").unwrap();
    assert_eq!(got.get("object"), variable_map.get("object"));
    let field_map = |v: &JsonValue| v.get("fields").unwrap().get("map").unwrap().clone();
    assert!(neutral_equal(&field_map(&got), &field_map(&variable_map)));
    assert!(neutral_equal(
        &options_fields::get_field(&options, "inputSourceMaps").unwrap(),
        &source_maps
    ));
    let map = options.get_input_variable_map().as_ref().unwrap();
    assert_eq!(
        map.lookup_new_name(&JsString::from("b")),
        Some(JsString::from("a"))
    );
    assert_eq!(
        map.lookup_source_name(&JsString::from("b")),
        Some(JsString::from("a"))
    );
    assert_eq!(
        map.to_map().keys().cloned().collect::<Vec<_>>(),
        vec![JsString::from("b"), JsString::from("a")]
    );
    let input = &options.get_input_source_maps()["in.js"];
    assert_eq!(input.get_original_path(), "in.js.map");
    assert!(!input.replay_fields().cached);
}

// port: ReplayDsl#eval / lambda
#[test]
fn dsl_let_once_do_lambdas_keep_scope_and_lazy_evaluation() {
    let mut ctx = context("descriptor\tlookup\tclass\tsignature\twidened\n");
    let expr = expression(
        r#"{"let":[["x",{"int":7}],["y",{"var":"x"}]],"in":{"lambda":["p"],"iface":"java.util.function.Function","body":{"list":[{"var":"y"},{"var":"p"}]}}}"#,
    );
    let DslValue::Lambda(l) = eval(&expr, &mut ctx).unwrap() else {
        panic!("lambda")
    };
    assert!(ctx.vars.is_empty());
    let v = invoke_lambda(&l, vec![DslValue::Int(9)], &mut ctx).unwrap();
    assert!(
        matches!(v,DslValue::List(v) if v[0].equals(&DslValue::Int(7)) && v[1].equals(&DslValue::Int(9)))
    );
    let first = expression(
        r#"{"do":[{"once":"x","value":{"int":1}}],"value":{"once":"x","value":{"var":"unbound"}}}"#,
    );
    assert!(eval(&first, &mut ctx).unwrap().equals(&DslValue::Int(1)));
    let error = expression(r#"{"let":[["x",{"int":3}]],"in":{"var":"missing"}}"#);
    assert!(eval(&error, &mut ctx).is_err());
    assert!(ctx.vars.is_empty());
    assert!(
        eval(
            &expression(r#"{"mutationPoint":"Value","value":{"int":4}}"#),
            &mut ctx
        )
        .unwrap()
        .equals(&DslValue::Int(4))
    );
}
// port: ReplayDsl#eval (test fixture registry side effect)
fn mark(ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let v = args.into_iter().next().unwrap_or(DslValue::Null);
    let DslValue::List(log) = ctx
        .field_overrides
        .entry("log".into())
        .or_insert_with(|| DslValue::List(vec![]))
    else {
        panic!("log")
    };
    log.push(v.clone());
    Ok(v)
}
// port: ReplayDsl#eval (evaluation order and withFields)
#[test]
fn dsl_evaluation_order_class_map_and_field_assignment() {
    let mut ctx = context(
        "descriptor\tlookup\tclass\tsignature\twidened\nFixture\tNew.mark\tNew\tmark(java.lang.Object)\t\n",
    );
    ctx.class_map.insert("Old".into(), "New".into());
    ctx.registry.register("New#mark(java.lang.Object)", mark);
    let holder = DslValue::Object(Rc::new(RefCell::new(Object {
        class: "Holder".into(),
        fields: IndexMap::new(),
        field_types: IndexMap::from([("a".into(), "int".into()), ("b".into(), "int".into())]),
    })));
    ctx.field_overrides.insert("holder".into(), holder.clone());
    let e = expression(
        r#"{"withFields":{"field":"holder"},"fields":{"b":{"static":"Old","method":"mark","args":[{"int":2}]},"a":{"static":"Old","method":"mark","args":[{"int":1}]}}}"#,
    );
    eval(&e, &mut ctx).unwrap();
    assert!(
        matches!(&ctx.field_overrides["log"],DslValue::List(v) if v[0].equals(&DslValue::Int(2)) && v[1].equals(&DslValue::Int(1)))
    );
    let e = expression(
        r#"{"map":[[{"static":"Old","method":"mark","args":[{"int":3}]},{"static":"Old","method":"mark","args":[{"int":4}]}]]}"#,
    );
    eval(&e, &mut ctx).unwrap();
    assert!(
        matches!(&ctx.field_overrides["log"],DslValue::List(v) if v[2].equals(&DslValue::Int(3)) && v[3].equals(&DslValue::Int(4)))
    );
    assert!(
        matches!(eval(&expression(r#"{"class":"Old"}"#),&mut ctx).unwrap(),DslValue::Class(c) if c=="New")
    );
}
// port: ReplayDsl#pick / invoke
#[test]
fn real_tsv_distinguishes_unported_signatures_from_resolution_errors() {
    let registry = Registry::load(&corpus::corpus_unit_dir()).unwrap();
    // ChromePass is not ported (out of scope, docs/PORTING.md §2); AliasStrings,
    // the earlier example, is ported.
    let resolution = registry
        .resolve(
            "ChromePassTest",
            "com.google.javascript.jscomp.ChromePass",
            1,
        )
        .unwrap();
    assert!(
        resolution
            .signature
            .starts_with("com.google.javascript.jscomp.ChromePass#<init>(")
    );
    let mut ctx = Ctx::new(
        "ChromePassTest".into(),
        object([]),
        IndexMap::new(),
        registry,
    );
    let e =
        expression(r#"{"new":"com.google.javascript.jscomp.ChromePass","args":[{"null":true}]}"#);
    assert!(
        matches!(eval(&e,&mut ctx),Err(Throwable::Unported(s)) if s.contains("ChromePass#<init>"))
    );
    assert!(matches!(
        eval(&expression(r#"{"new":"missing"}"#), &mut ctx),
        Err(Throwable::HarnessError(_))
    ));
}
// port: ImmutableList#of(java.lang.Object) / ReplayDsl#invoke
#[test]
fn native_singleton_list_keeps_its_java_runtime_class() {
    let registry = Registry::load(&corpus::corpus_unit_dir()).unwrap();
    let mut ctx = Ctx::new(
        "Es6NormalizeShorthandPropertiesTest".into(),
        object([]),
        IndexMap::new(),
        registry,
    );
    let value = eval(
        &expression(
            r#"{"static":"com.google.common.collect.ImmutableList","method":"of","args":[{"string":"x"}]}"#,
        ),
        &mut ctx,
    )
    .unwrap();
    assert_eq!(
        value.class_name(),
        "com.google.common.collect.SingletonImmutableList"
    );
    assert!(ref_value(&value, 2).unwrap().gson_equals(&json(
        r#"{"list":["x"],"impl":"com.google.common.collect.SingletonImmutableList"}"#
    )));
    assert!(
        matches!(eval(&expression(r#"{"static":"com.google.common.collect.ImmutableList","method":"of","args":[{"null":true}]}"#), &mut ctx), Err(Throwable::Exception {class,message:None}) if class=="java.lang.NullPointerException")
    );
}
// port: UnitRecorder#errors
#[test]
fn errors_dump_real_diagnostics_in_the_recorded_shape() {
    assert!(errors(&[error("bad")]).gson_equals(&json(r#"[{"key":"JSC_TEST","defaultLevel":"ERROR","description":"bad","source":"test.js","line":2,"charno":3,"length":4}]"#)));
    assert!(
        closure_testing::replay::replay_bridge::errors(&[error("bad")])
            .gson_equals(&errors(&[error("bad")]))
    );
}
// port: UnitRecorder#refValue
#[test]
fn referenceable_dump_cuts_cycles_and_object_depth() {
    let o = Rc::new(RefCell::new(Object {
        class: "Holder".into(),
        fields: IndexMap::new(),
        field_types: IndexMap::new(),
    }));
    let v = DslValue::Object(o.clone());
    o.borrow_mut().fields.insert("next".into(), v.clone());
    assert!(ref_value(&v, 2).unwrap().gson_equals(&json(
        r#"{"object":"Holder","fields":{"next":{"ref":"Holder"}}}"#
    )));
    assert!(
        ref_value(&v, -1)
            .unwrap()
            .gson_equals(&json(r#"{"ref":"Holder"}"#))
    );
    // UnitRecorder#dump: an object at depthLeft <= 0 is written as a reference.
    assert!(
        ref_value(&v, 0)
            .unwrap()
            .gson_equals(&json(r#"{"ref":"Holder"}"#))
    );
    o.borrow_mut().fields.clear(); // Release the fixture's reference cycle.
}
// port: ReplayMain#compare (FORMAT.md neutral equality)
#[test]
fn post_call_neutral_equality_keeps_order_only_for_ordered_containers() {
    let a = json(r#"{"map":[["a",1],["b",2]],"impl":"java.util.LinkedHashMap"}"#);
    let b =
        json(r#"{"map":[["a",1],["b",2]],"impl":"com.google.common.collect.RegularImmutableMap"}"#);
    let c = json(r#"{"map":[["b",2],["a",1]],"impl":"java.util.HashMap"}"#);
    assert!(neutral_equal(&a, &b));
    assert!(neutral_equal(&a, &c));
    assert!(!neutral_equal(
        &json(r#"{"map":[["b",2],["a",1]],"impl":"java.util.LinkedHashMap"}"#),
        &c
    ));
    assert!(!neutral_equal(
        &json(r#"{"list":[1,2],"impl":"java.util.ArrayList"}"#),
        &json(r#"{"list":[2,1],"impl":"java.util.ArrayList"}"#)
    ));
    assert!(neutral_equal(
        &json(r#"{"set":[2,1],"impl":"java.util.HashSet"}"#),
        &json(r#"{"set":[1,2],"impl":"java.util.HashSet"}"#)
    ));
    assert!(!neutral_equal(
        &json(r#"{"arrayOf":"int","items":[1]}"#),
        &json(r#"{"arrayOf":"long","items":[1]}"#)
    ));
    assert!(!neutral_equal(
        &json(r#"{"compiler":{},"pass":{}}"#),
        &json(r#"{"compiler":{}}"#)
    ));
}
// port: ReplayMain#compare / postState / postconditionCounts
#[test]
fn outcome_and_post_state_contracts_on_real_records() {
    let root = corpus::corpus_unit_dir();
    let records = corpus::load_records(&root.join("records/AliasStringsTest.jsonl.gz")).unwrap();
    let descriptor =
        corpus::load_descriptor(&root.join("descriptors/AliasStringsTest.json")).unwrap();
    let record = &records[0];
    let result = ReplayResult {
        outcome: record.raw.get("outcome").unwrap().clone(),
        observed: record.raw.get("observed").unwrap().clone(),
        post_call: record.raw.get("postCall").cloned(),
        test_fields_after_want: None,
        exception_mapped: false,
        omitted_unported: None,
    };
    assert_eq!(compare(&record.raw, &result), None);
    assert_eq!(
        postcondition_counts(record, Some(&descriptor.descriptor)),
        Some((0, 0))
    );
    assert_eq!(
        post_state(record, Some(&descriptor.descriptor))
            .unwrap()
            .get("postconditionsRecorded"),
        Some(&JsonValue::int(0))
    );
    let mut bad = result.clone();
    bad.observed = object([]);
    assert!(compare(&record.raw, &bad).is_some());
    let exception = json(
        r#"{"outcome":{"status":"exception","exceptionClass":"java.lang.IllegalStateException","message":"x","assertion":false}}"#,
    );
    let mapped = ReplayResult::from_run(Err(Throwable::Exception {
        class: "java.lang.IllegalStateException".into(),
        message: Some("x".into()),
    }))
    .unwrap();
    assert_eq!(compare(&exception, &mapped), None);
    let mut bad = mapped.clone();
    bad.outcome = json(
        r#"{"status":"exception","exceptionClass":"java.lang.IllegalStateException","message":"y","assertion":false}"#,
    );
    assert!(compare(&exception, &bad).is_some());
    let mut unmapped = mapped.clone();
    unmapped.exception_mapped = false;
    unmapped.outcome =
        json(r#"{"status":"exception","message":"unmapped Rust exception","assertion":false}"#);
    assert_eq!(compare(&exception, &unmapped), None);
    assert!(compare(&exception, &ReplayResult::panic("ported compiler panic")).is_some());
    let assertion = json(r#"{"outcome":{"status":"exception","assertion":true}}"#);
    assert!(compare(&assertion, &ReplayResult::panic("panic")).is_some());
    let asserted = ReplayResult::from_run(Err(Throwable::Assertion {
        message: "Truth text may differ".into(),
    }))
    .unwrap();
    assert_eq!(compare(&assertion, &asserted), None);
}
// port: ReplayMain#replayOne (FORMAT.md outcome contract, D-010: depth-sensitive records)
#[test]
fn stack_overflow_error_records_are_reported_unported_without_running() {
    let root = corpus::corpus_unit_dir();
    let records = corpus::load_records(&root.join("records/TypeCheckTest.jsonl.gz")).unwrap();
    let descriptor = corpus::load_descriptor_for("TypeCheckTest").unwrap();
    let registry = Registry::load(&root).unwrap();
    let overflowing: Vec<_> = records
        .iter()
        .filter(|r| {
            r.raw
                .get("outcome")
                .and_then(|o| o.get("exceptionClass"))
                .is_some_and(|c| matches!(c, JsonValue::String(c) if c.eq_str("java.lang.StackOverflowError")))
        })
        .collect();
    // TypeCheckTest#testCyclicUnionTypedefs: Java itself overflows the stack.
    assert_eq!(overflowing.len(), 1);
    let record = overflowing[0];
    match replay_one(
        "TypeCheckTest",
        record,
        descriptor.as_ref().map(|d| &d.descriptor),
        registry,
        None,
    ) {
        Err(Throwable::Unported(message)) => assert_eq!(
            message,
            "java.lang.StackOverflowError (depth-sensitive, D-010)"
        ),
        other => panic!("expected an unported depth-sensitive record, got {other:?}"),
    }
}
// port: ReplayCompilerTest#expectedTestFieldsAfter / canonicalClassNames
#[test]
fn field_projection_and_class_name_canonicalization() {
    let record =
        json(r#"{"testFields":{"before":1,"skip":2},"testFieldsAfter":{"changed":3,"skip":4}}"#);
    let d=Descriptor::from_json(&json(r#"{"source":"Fixture.java","cases":[{"testFieldsAfter":{"null":true},"testFieldsAfterAlso":["before"],"testFieldsAfterSkip":{"skip":"checked by a postcondition"}}]}"#),"$").unwrap();
    let case: &Case = &d.cases[0];
    assert!(
        expected_test_fields_after(&record, case)
            .unwrap()
            .gson_equals(&json(r#"{"changed":3,"before":1}"#))
    );
    let classes = IndexMap::from([("Old".into(), "New".into())]);
    assert!(
        canonical_class_names(
            &json(r#"{"object":"New","fields":{"x":["New",{"enum":"New","name":"Old"}]}}"#),
            &classes
        )
        .gson_equals(&json(
            r#"{"object":"Old","fields":{"x":["Old",{"enum":"Old","name":"Old"}]}}"#
        ))
    );
}
// port: ReplayMain#postState / postconditionCounts (integration and type-check records)
#[test]
fn post_state_distinguishes_returned_results_and_unreplayed_postconditions() {
    use closure_testing::record::{Api, ComparisonMode};
    let root = corpus::corpus_unit_dir();
    let records = corpus::load_records(&root.join("records/IntegrationTest.jsonl.gz")).unwrap();
    let record = records
        .iter()
        .find(|r| r.record.api == Api::Compile)
        .unwrap();
    assert_eq!(postcondition_counts(record, None), None);
    assert_eq!(
        post_state(record, None).unwrap().get("postState"),
        Some(&JsonValue::str("notCaptured"))
    );
    let records = corpus::load_records(&root.join("records/TypeCheckTest.jsonl.gz")).unwrap();
    let record = records
        .iter()
        .find(|r| r.record.comparison.mode == ComparisonMode::ObservedOnly)
        .unwrap();
    assert_eq!(
        post_state(record, None).unwrap().get("postState"),
        Some(&JsonValue::str("notCaptured"))
    );
    let records = corpus::load_records(&root.join("records/AliasStringsTest.jsonl.gz")).unwrap();
    let descriptor =
        corpus::load_descriptor(&root.join("descriptors/AliasStringsTest.json")).unwrap();
    let mut descriptor = descriptor.descriptor;
    for case in &mut descriptor.cases {
        case.postconditions = Some(vec![Expr::Null]);
    }
    assert_eq!(
        postcondition_counts(&records[0], Some(&descriptor)),
        Some((0, 1))
    );
    assert_eq!(
        post_state(&records[0], Some(&descriptor))
            .unwrap()
            .get("postState"),
        Some(&JsonValue::str("notCaptured"))
    );
}
struct ProducerFixture;
impl closure_testing::replay::replay_dsl::NativeObject for ProducerFixture {
    // port: UnitRecorder#isInstance (live producer fixture)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.CheckRegExp"
    }
    // port: UnitRecorder#collect (live producer fixture)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::new())
    }
    // port: UnitRecorder#findMethod (live producer fixture)
    fn call(&mut self, method: &str, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        assert_eq!(method, "isGlobalRegExpPropertiesUsed");
        Ok(DslValue::Bool(true))
    }
    // port: ReplayDsl#invoke (live receiver cast fixture)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: UnitRecorder#postCallSnapshot (native getter dispatch)
#[test]
fn post_call_snapshot_reads_the_first_reachable_native_result_producer() {
    let p = DslValue::Native(Rc::new(RefCell::new(ProducerFixture)));
    let got =
        closure_testing::unit_recorder::post_call_snapshot(None, Some(&p), &[], None).unwrap();
    assert!(got.gson_equals(&json(
        r#"{"compiler":{},"pass":{"pass.globalRegExpPropertiesUsed":true}}"#
    )));
    assert!(ref_value(&p, 2).unwrap().gson_equals(&json(
        r#"{"object":"com.google.javascript.jscomp.CheckRegExp","fields":{}}"#
    )));
    let sequence = DslValue::Sequence(vec![p.clone()]);
    assert!(
        closure_testing::unit_recorder::post_call_snapshot(None, Some(&sequence), &[], None)
            .unwrap()
            .gson_equals(&got)
    );
    // UnitRecorder#dump writes an object reached at depthLeft <= 0 as a reference: the
    // SequencePass expands at depth 1, its `passes` list holds the pass at depth 0.
    assert!(ref_value(&sequence, 1).unwrap().gson_equals(&json(
        r#"{"object":"com.google.javascript.jscomp.ReplayDsl$SequencePass","fields":{"passes":{"list":[{"ref":"com.google.javascript.jscomp.CheckRegExp"}],"impl":"java.util.ArrayList"}}}"#
    )));
    // An extra root is visited at depth 1; the SequencePass's list consumes that depth.
    assert!(
        closure_testing::unit_recorder::post_call_snapshot(None, None, &[sequence], None)
            .unwrap()
            .gson_equals(&json(r#"{"compiler":{},"pass":{}}"#))
    );
}
// port: CompilerTestCase#srcs / externs / findQualifiedNameNodes
#[test]
fn convenience_helpers_preserve_file_names_extern_copying_and_postorder() {
    use closure_rhino::{ir::IR, node::Ast, token::Token};
    let golden = json(include_str!("data/test_externs_builder.json"));
    assert_eq!(
        CompilerTestCase::default_externs().unwrap().as_units(),
        golden.get("default").unwrap().as_js_string().unwrap().0
    );
    assert_eq!(
        CompilerTestCase::minimal_externs().unwrap().as_units(),
        golden.get("minimal").unwrap().as_js_string().unwrap().0
    );
    // VAR_CHECK_EXTERNS: one `var <symbol>;` line per VarCheck.REQUIRED_SYMBOLS entry (32, in
    // keep-sorted order, var-checks).
    let var_check_externs = CompilerTestCase::var_check_externs()
        .unwrap()
        .to_string_lossy();
    assert!(var_check_externs.starts_with("var AggregateError;\nvar Array;\nvar Error;\n"));
    assert_eq!(var_check_externs.lines().count(), 32);
    assert!(
        var_check_externs
            .lines()
            .all(|l| l.starts_with("var ") && l.ends_with(';'))
    );
    assert!(
        closure_testing::integration::integration_test_case::IntegrationTestCase::default_externs()
            .unwrap()[0]
            .get_code()
            .unwrap()
            .to_string_lossy()
            .contains("Math.pow")
    );
    assert!(closure_testing::harness_passes::google_coding_convention().is_ok());
    let parts = CompilerTestCase::srcs_strings(&["a".into(), "b".into()]);
    let closure_testing::compiler_test_case::Sources::Flat(sources) = parts else {
        panic!("flat")
    };
    assert_eq!(
        sources
            .sources
            .iter()
            .map(|s| s.get_name())
            .collect::<Vec<_>>(),
        vec!["testcode0", "testcode1"]
    );
    let input = Arc::new(SourceFile::from_code("input.js", "x"));
    let externs = CompilerTestCase::externs_file_array(std::slice::from_ref(&input)).unwrap();
    assert_eq!(
        input.get_kind(),
        closure_testing::jscomp_api::SourceKind::STRONG
    );
    assert_eq!(
        externs.externs[0].get_kind(),
        closure_testing::jscomp_api::SourceKind::EXTERN
    );
    assert!(!Arc::ptr_eq(&input, &externs.externs[0]));
    let mut ast = Ast::new();
    let a = IR::name(&mut ast, "x");
    let b = IR::name(&mut ast, "x");
    let root = ast.new_node_with_children2(Token::ROOT, a, b);
    assert_eq!(
        CompilerTestCase::find_qualified_name_nodes(&ast, &"x".into(), root),
        vec![a, b]
    );
    assert!(CompilerTestCase::find_qualified_name_node(&ast, &"missing".into(), root).is_err());
}

// port: ReplayMain#compare (NodeTraversal#throwInternalError RuntimeException)
#[test]
fn internal_compiler_error_runtime_exception_requires_the_exact_message() {
    let internal = "INTERNAL COMPILER ERROR.\nPlease report this problem.\n\nUnexpected renaming in \
                    MakeDeclaredNamesUnique. new name: a$jscomp$1 for NAME a 1:28";
    let record = object([(
        "outcome",
        object([
            ("status", JsonValue::str("exception")),
            (
                "exceptionClass",
                JsonValue::str("java.lang.RuntimeException"),
            ),
            ("message", JsonValue::str(internal)),
            ("assertion", JsonValue::Bool(false)),
        ]),
    )]);
    let message = internal.to_string();
    let caught =
        ReplayResult::catch_precondition_run(&record, move || panic!("{message}")).unwrap();
    assert_eq!(compare(&record, &caught), None);
    // No prefix or substring matching: a longer, shorter or different panic is not reproduced.
    for different in [
        format!("{internal} "),
        internal[..internal.len() - 1].to_string(),
        "INTERNAL COMPILER ERROR.\nPlease report this problem.\n\n".to_string(),
        format!("x{internal}"),
    ] {
        assert!(compare(&record, &ReplayResult::panic(&different)).is_some());
        assert!(
            std::panic::catch_unwind(|| {
                ReplayResult::catch_precondition_run(&record, || panic!("{different}"))
            })
            .is_err()
        );
    }
    let asserted = object([(
        "outcome",
        object([
            ("status", JsonValue::str("exception")),
            (
                "exceptionClass",
                JsonValue::str("java.lang.RuntimeException"),
            ),
            ("message", JsonValue::str(internal)),
            ("assertion", JsonValue::Bool(true)),
        ]),
    )]);
    assert!(compare(&asserted, &caught).is_some());
}

// port: ReplayMain#compare (DESIGN.md precondition panics)
#[test]
fn precondition_panics_require_exact_recorded_class_category_and_message() {
    let outcome = |class: &str, message: JsonValue, assertion: bool| {
        object([(
            "outcome",
            object([
                ("status", JsonValue::str("exception")),
                ("exceptionClass", JsonValue::str(class)),
                ("message", message),
                ("assertion", JsonValue::Bool(assertion)),
            ]),
        )])
    };
    for class in [
        "java.lang.IllegalStateException",
        "java.lang.IllegalArgumentException",
        "java.lang.NullPointerException",
        "java.lang.ClassCastException",
    ] {
        let record = outcome(class, JsonValue::str("Java message"), false);
        let caught =
            ReplayResult::catch_precondition_run(&record, || panic!("Java message")).unwrap();
        assert_eq!(compare(&record, &caught), None);
        assert!(compare(&record, &ReplayResult::panic("different message")).is_some());
        let asserted = outcome(class, JsonValue::str("Java message"), true);
        assert!(compare(&asserted, &caught).is_some());
        let typed_wrong_class = ReplayResult::from_run(Err(Throwable::Exception {
            class: "java.lang.RuntimeException".into(),
            message: Some("Java message".into()),
        }))
        .unwrap();
        assert!(compare(&record, &typed_wrong_class).is_some());
        let with_observed = object([
            ("outcome", record.get("outcome").unwrap().clone()),
            ("observed", object([("errors", JsonValue::Array(vec![]))])),
        ]);
        assert!(compare(&with_observed, &caught).is_some());
    }
    let other = outcome(
        "java.lang.UnsupportedOperationException",
        JsonValue::str("Java message"),
        false,
    );
    assert!(compare(&other, &ReplayResult::panic("Java message")).is_some());
    assert!(
        std::panic::catch_unwind(|| {
            ReplayResult::catch_precondition_run(&other, || panic!("Java message"))
        })
        .is_err()
    );
    let normal = json(r#"{"outcome":{"status":"normal"}}"#);
    assert!(compare(&normal, &ReplayResult::panic("Java message")).is_some());
    let null = outcome("java.lang.NullPointerException", JsonValue::Null, false);
    assert!(compare(&null, &ReplayResult::panic("")).is_some());

    let opaque = outcome(
        "java.lang.IllegalStateException",
        JsonValue::str("non-string panic"),
        false,
    );
    assert!(compare(&opaque, &ReplayResult::from_panic(&7_i32)).is_some());
    assert!(
        std::panic::catch_unwind(|| {
            ReplayResult::catch_precondition_run(&opaque, || std::panic::panic_any(7_i32))
        })
        .is_err()
    );

    // Java substring may split a surrogate pair. Count and truncate code units, not scalars.
    let message = format!("{}😀tail", "a".repeat(3999));
    let mut units = vec![b'a' as u16; 3999];
    units.push(0xd83d);
    units.extend("...<truncated 5 chars>".encode_utf16());
    let record = outcome(
        "java.lang.IllegalArgumentException",
        JsonValue::String(closure_testing::json::JsString(units)),
        false,
    );
    assert_eq!(compare(&record, &ReplayResult::panic(&message)), None);
}

// port: CompilerTestCase.NamedPredicate#of / apply / toString
// port: CompilerTestCase.ErrorDiagnostic#ErrorDiagnostic / CompilerTestCase.WarningDiagnostic#WarningDiagnostic
// port: CompilerTestCase.Diagnostic#toString
#[test]
fn named_predicates_and_diagnostic_subclasses_keep_java_names() {
    use closure_testing::compiler_test_case::{CompilerTestCase, NamedPredicate};
    let predicate = NamedPredicate::of(|value: &i32| *value > 2, "greater than two".into());
    assert!(predicate.apply(&3));
    assert!(!predicate.apply(&2));
    assert_eq!(predicate.to_string(), "greater than two");
    let diagnostic = &ERROR;
    let error = CompilerTestCase::error(diagnostic);
    let warning = CompilerTestCase::warning(diagnostic);
    assert_eq!(error.level, CheckLevel::ERROR);
    assert_eq!(warning.level, CheckLevel::WARNING);
    assert_eq!(error.to_string(), "JSC_TEST");
    assert_eq!(
        warning
            .with_message_containing("needle")
            .unwrap()
            .to_string(),
        "JSC_TEST with message containing \"needle\""
    );
}
