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
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2019 The Closure Compiler Authors.
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
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/lint/CheckExtraRequires.java,
//   test/com/google/javascript/jscomp/lint/CheckProvidesSortedTest.java,
//   test/com/google/javascript/jscomp/lint/CheckRequiresSortedTest.java.

//! Replay adapters for the lint checks: the pass constructors the
//! lint test descriptors name, and the getProcessor helpers of CheckProvidesSortedTest
//! and CheckRequiresSortedTest (oracle/replay/helpers).
use crate::{
    jscomp_api::{AbstractCompiler, Compiler},
    replay::{
        registry::{BorrowedEntry, Entry},
        replay_dsl::{Ctx, DslValue, Object},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass,
    lint::{
        check_const_private_properties::CheckConstPrivateProperties,
        check_constant_case_names::CheckConstantCaseNames,
        check_empty_statements::CheckEmptyStatements, check_enums::CheckEnums,
        check_es6_module_file_structure::CheckEs6ModuleFileStructure,
        check_es6_modules::CheckEs6Modules, check_extra_requires::CheckExtraRequires,
        check_goog_module_type_script_name::CheckGoogModuleTypeScriptName,
        check_interfaces::CheckInterfaces, check_jsdoc_style::CheckJSDocStyle,
        check_missing_semicolon::CheckMissingSemicolon,
        check_no_mutated_es6_exports::CheckNoMutatedEs6Exports,
        check_nullability_modifiers::CheckNullabilityModifiers,
        check_primitive_as_object::CheckPrimitiveAsObject,
        check_prototype_properties::CheckPrototypeProperties, check_provides_sorted,
        check_requires_sorted, check_unused_labels::CheckUnusedLabels,
        check_unused_private_properties::CheckUnusedPrivateProperties,
        check_useless_blocks::CheckUselessBlocks, check_var::CheckVar,
    },
    node_traversal::NodeTraversal,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc};

const PROVIDES_SORTED_HOST: &str =
    "com.google.javascript.jscomp.CheckProvidesSortedTest_Helpers$GetProcessorHost";
const REQUIRES_SORTED_HOST: &str = "com.google.javascript.jscomp.CheckRequiresSortedTest_Helpers";

// Every lint pass constructor `<init>(AbstractCompiler)`: the unborrowed and the
// borrowed (compiler loaned to a running factory) entry.
macro_rules! lint_constructors {
    ($($class:literal => $ty:ty),* $(,)?) => {
        // port: ReplayDsl#invoke (lint pass constructors)
        pub fn entry(signature: &str) -> Option<Entry> {
            $(
                if signature
                    == concat!(
                        "com.google.javascript.jscomp.lint.",
                        $class,
                        "#<init>(com.google.javascript.jscomp.AbstractCompiler)"
                    )
                {
                    return Some(|_ctx: &mut Ctx, args: Vec<DslValue>| {
                        let [DslValue::Compiler(c)] = args.as_slice() else {
                            return Err(bad());
                        };
                        let pass = <$ty>::new(&c.borrow());
                        Ok(pass_value(pass))
                    });
                }
            )*
            other_entry(signature)
        }
        // port: ReplayDsl#invoke (lint pass constructors under the compiler loan)
        pub fn borrowed_entry(signature: &str) -> Option<BorrowedEntry> {
            $(
                if signature
                    == concat!(
                        "com.google.javascript.jscomp.lint.",
                        $class,
                        "#<init>(com.google.javascript.jscomp.AbstractCompiler)"
                    )
                {
                    return Some(
                        |_ctx: &mut Ctx, args: Vec<DslValue>, compiler: &mut Compiler| {
                            let [DslValue::Compiler(_)] = args.as_slice() else {
                                return Err(bad());
                            };
                            Ok(pass_value(<$ty>::new(compiler)))
                        },
                    );
                }
            )*
            other_borrowed_entry(signature)
        }
    };
}

lint_constructors! {
    "CheckConstPrivateProperties" => CheckConstPrivateProperties,
    "CheckConstantCaseNames" => CheckConstantCaseNames,
    "CheckEmptyStatements" => CheckEmptyStatements,
    "CheckEnums" => CheckEnums,
    "CheckEs6ModuleFileStructure" => CheckEs6ModuleFileStructure,
    "CheckEs6Modules" => CheckEs6Modules,
    "CheckGoogModuleTypeScriptName" => CheckGoogModuleTypeScriptName,
    "CheckInterfaces" => CheckInterfaces,
    "CheckJSDocStyle" => CheckJSDocStyle,
    "CheckMissingSemicolon" => CheckMissingSemicolon,
    "CheckNoMutatedEs6Exports" => CheckNoMutatedEs6Exports,
    "CheckNullabilityModifiers" => CheckNullabilityModifiers,
    "CheckPrimitiveAsObject" => CheckPrimitiveAsObject,
    "CheckPrototypeProperties" => CheckPrototypeProperties,
    "CheckUnusedLabels" => CheckUnusedLabels,
    "CheckUnusedPrivateProperties" => CheckUnusedPrivateProperties,
    "CheckUselessBlocks" => CheckUselessBlocks,
    "CheckVar" => CheckVar,
}

const EXTRA_REQUIRES_INIT: &str = "com.google.javascript.jscomp.lint.CheckExtraRequires#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.common.collect.ImmutableSet)";

// port: ReplayDsl#invoke (CheckExtraRequires and the sorted-check helpers)
fn other_entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        EXTRA_REQUIRES_INIT => extra_requires,
        s if s == format!("{PROVIDES_SORTED_HOST}#<init>()") => provides_sorted_host,
        s if s
            == format!(
                "{PROVIDES_SORTED_HOST}#getProcessor(com.google.javascript.jscomp.Compiler)"
            ) =>
        {
            provides_sorted_get_processor
        }
        s if s == format!("{REQUIRES_SORTED_HOST}#<init>()") => requires_sorted_host,
        s if s
            == format!(
                "{REQUIRES_SORTED_HOST}#getProcessor(com.google.javascript.jscomp.Compiler)"
            ) =>
        {
            requires_sorted_get_processor
        }
        _ => return None,
    })
}

// port: ReplayDsl#invoke (CheckExtraRequires under the compiler loan)
fn other_borrowed_entry(signature: &str) -> Option<BorrowedEntry> {
    Some(match signature {
        EXTRA_REQUIRES_INIT => extra_requires_borrowed,
        _ => return None,
    })
}

// port: ReplayDsl#invoke (a constructed callback pass as the processor)
fn pass_value(pass: impl CompilerPass + 'static) -> DslValue {
    let pass: Box<dyn CompilerPass> = Box::new(pass);
    DslValue::Pass(Rc::new(RefCell::new(pass)))
}

// port: ReplayDsl#invoke (ImmutableSet<String> or null argument)
fn requires_to_remove(value: &DslValue) -> Result<Option<IndexSet<String>>, Throwable> {
    match value.untyped() {
        DslValue::Null => Ok(None),
        DslValue::Set(values) => values
            .iter()
            .map(|v| match v {
                DslValue::String(s) => Ok(s.to_string_lossy()),
                _ => Err(bad()),
            })
            .collect::<Result<IndexSet<_>, _>>()
            .map(Some),
        _ => Err(bad()),
    }
}

// port: CheckExtraRequires#CheckExtraRequires
fn extra_requires(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(c), set] = args.as_slice() else {
        return Err(bad());
    };
    let set = requires_to_remove(set)?;
    let pass = CheckExtraRequires::new(&c.borrow(), set);
    Ok(pass_value(pass))
}

// port: CheckExtraRequires#CheckExtraRequires (compiler loaned to a factory)
fn extra_requires_borrowed(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut Compiler,
) -> Result<DslValue, Throwable> {
    let [DslValue::Compiler(_), set] = args.as_slice() else {
        return Err(bad());
    };
    let set = requires_to_remove(set)?;
    Ok(pass_value(CheckExtraRequires::new(compiler, set)))
}

// port: ReplayValues#instantiate (helper holder's declared no-argument constructor)
fn helper_object(class: &str) -> DslValue {
    DslValue::Object(Rc::new(RefCell::new(Object {
        class: class.into(),
        fields: IndexMap::<_, _>::default(),
        field_types: IndexMap::<_, _>::default(),
    })))
}

// port: CheckProvidesSortedTest_Helpers.GetProcessorHost#GetProcessorHost
fn provides_sorted_host(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    if !args.is_empty() {
        return Err(bad());
    }
    Ok(helper_object(PROVIDES_SORTED_HOST))
}

// port: CheckProvidesSortedTest_Helpers.GetProcessorHost#getProcessor
fn provides_sorted_get_processor(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [_host, DslValue::Compiler(_)] = args.as_slice() else {
        return Err(bad());
    };
    let mut callback = check_provides_sorted::CheckProvidesSorted::new(
        check_provides_sorted::Mode::COLLECT_AND_REPORT,
    );
    Ok(pass_value(
        move |compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId| {
            NodeTraversal::traverse(compiler, root, &mut callback);
        },
    ))
}

// port: CheckRequiresSortedTest_Helpers#CheckRequiresSortedTest_Helpers
fn requires_sorted_host(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    if !args.is_empty() {
        return Err(bad());
    }
    Ok(helper_object(REQUIRES_SORTED_HOST))
}

// port: CheckRequiresSortedTest_Helpers#getProcessor
fn requires_sorted_get_processor(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [_host, DslValue::Compiler(_)] = args.as_slice() else {
        return Err(bad());
    };
    let mut callback = check_requires_sorted::CheckRequiresSorted::new(
        check_requires_sorted::Mode::COLLECT_AND_REPORT,
    );
    Ok(pass_value(
        move |compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId| {
            NodeTraversal::traverse(compiler, root, &mut callback);
        },
    ))
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
