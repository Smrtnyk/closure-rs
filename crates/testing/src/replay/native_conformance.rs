/*
 * Copyright 2014 The Closure Compiler Authors.
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
// Copyright 2014 The Closure Compiler Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/CheckConformance.java,
//   src/com/google/javascript/jscomp/conformance/conformance.proto.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Replay adapters for the conformance checks (CheckConformanceTest#getProcessor: TextFormat#parse of
//! the test's ConformanceConfig text fields, ConformanceConfig#newBuilder().build() and the
//! CheckConformance constructor).
use crate::{
    jscomp_api::Compiler,
    replay::{
        options_values::OptionValue,
        registry::{BorrowedEntry, Entry},
        replay_dsl::{Ctx, DslValue},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    check_conformance::CheckConformance, compiler_options::ConformanceReportingMode,
    compiler_pass::CompilerPass, conformance_config::ConformanceConfig, protobuf::text_format,
};
use std::{cell::RefCell, rc::Rc};

const CONFORMANCE_CONFIG: &str = "com.google.javascript.jscomp.ConformanceConfig";
const CONFORMANCE_CONFIG_BUILDER: &str = "com.google.javascript.jscomp.ConformanceConfig$Builder";
const CHECK_CONFORMANCE_INIT: &str = "com.google.javascript.jscomp.CheckConformance#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.common.collect.ImmutableList,com.google.javascript.jscomp.CompilerOptions$ConformanceReportingMode)";

// port: ReplayDsl#invoke (resolved conformance signatures backed by native implementations)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.protobuf.TextFormat#parse(java.lang.CharSequence,java.lang.Class)" => {
            text_format_parse
        }
        "com.google.javascript.jscomp.ConformanceConfig#newBuilder()" => new_builder,
        "com.google.javascript.jscomp.ConformanceConfig$Builder#build()" => builder_build,
        CHECK_CONFORMANCE_INIT => check_conformance,
        _ => return None,
    })
}

// port: ReplayDsl#invoke (the constructor called inside a running processor, which holds the
// compiler)
pub fn borrowed_entry(signature: &str) -> Option<BorrowedEntry> {
    Some(match signature {
        CHECK_CONFORMANCE_INIT => check_conformance_borrowed,
        _ => return None,
    })
}

fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}

// port: TextFormat#parse(CharSequence,Class)
fn text_format_parse(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::String(input), DslValue::Class(class)] = args.as_slice() else {
        return Err(bad());
    };
    if class != CONFORMANCE_CONFIG {
        return Err(Throwable::Unported(format!(
            "com.google.protobuf.TextFormat#parse({class})"
        )));
    }
    // Internal.getDefaultInstance(type).newBuilderForType(); merge(input, builder); build().
    let mut builder = ConformanceConfig::new_builder();
    if let Err(e) = text_format::merge(&input.to_string_lossy(), &mut builder) {
        return Err(Throwable::Exception {
            class: "com.google.protobuf.TextFormat$ParseException".into(),
            message: Some(e.get_message().to_string()),
        });
    }
    builder.build().encode_value()
}

// port: ConformanceConfig#newBuilder
fn new_builder(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    if !args.is_empty() {
        return Err(bad());
    }
    Ok(DslValue::Typed {
        class: CONFORMANCE_CONFIG_BUILDER.into(),
        value: Box::new(ConformanceConfig::new_builder().encode_value()?),
    })
}

// port: ConformanceConfig.Builder#build
fn builder_build(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [builder] = args.as_slice() else {
        return Err(bad());
    };
    ConformanceConfig::decode_value(builder)?
        .build()
        .encode_value()
}

// port: CheckConformance#CheckConformance(AbstractCompiler,ImmutableList,ConformanceReportingMode)
fn check_conformance(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c), ..] = args.as_slice() else {
        return Err(bad());
    };
    let c = c.clone();
    new_check_conformance(&args, &mut c.borrow_mut())
}

// port: CheckConformance#CheckConformance(AbstractCompiler,ImmutableList,ConformanceReportingMode)
fn check_conformance_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    new_check_conformance(&args, compiler)
}

// port: CheckConformance#CheckConformance(AbstractCompiler,ImmutableList,ConformanceReportingMode)
fn new_check_conformance(
    args: &[DslValue],
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(_), configs, reporting_mode] = args else {
        return Err(bad());
    };
    let (DslValue::List(configs) | DslValue::Sequence(configs)) = configs.untyped() else {
        return Err(bad());
    };
    let configs = configs
        .iter()
        .map(ConformanceConfig::decode_value)
        .collect::<Result<Vec<_>, _>>()?;
    let reporting_mode = match reporting_mode {
        DslValue::Null => None,
        v => Some(ConformanceReportingMode::decode_value(v)?),
    };
    // A CUSTOM requirement whose java_class is not one of ConformanceRules' own classes (the
    // Java test's CheckConformanceTest$CustomRule...) cannot be loaded by the port
    // (ConformanceRules.CustomRuleProxy panics "unported: ..."): report the record Unported.
    let pass = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        CheckConformance::new(compiler, &configs, reporting_mode)
    }))
    .map_err(|panic| {
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied());
        match message.and_then(|m| m.strip_prefix("unported: ")) {
            Some(item) => Throwable::Unported(item.to_string()),
            None => std::panic::resume_unwind(panic),
        }
    })?;
    Ok(DslValue::Pass(Rc::new(RefCell::new(
        Box::new(pass) as Box<dyn CompilerPass>
    ))))
}
