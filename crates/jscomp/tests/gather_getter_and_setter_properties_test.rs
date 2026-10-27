/*
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit b2f69b3:
//   test/com/google/javascript/jscomp/CompilerTestCase.java,
//   test/com/google/javascript/jscomp/GatherGetterAndSetterPropertiesTest.java.

//! Port of the `GatherGetterAndSetterPropertiesTest.java` tests that have no corpus record. The
//! other tests replay from corpus/unit/records (GatherGetterAndSetterPropertiesTest, 33 records).
use closure_jscomp::abstract_compiler::AbstractCompiler;
use closure_jscomp::accessor_summary::PropertyAccessKind;
use closure_jscomp::compiler::Compiler;
use closure_jscomp::compiler_options::CompilerOptions;
use closure_jscomp::compiler_pass::CompilerPass;
use closure_jscomp::gather_getter_and_setter_properties::GatherGetterAndSetterProperties;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use closure_testing::{
    compiler_test_case::{CompilerTestCase, CompilerTestCaseHooks},
    replay::{
        registry::Registry,
        replay_dsl::{CompilerHandle, Ctx, DslValue},
    },
    throwable::Throwable,
};
use std::{cell::RefCell, rc::Rc};

struct GatherGetterAndSetterPropertiesTest {
    harness: CompilerTestCase,
    hooks: Hooks,
}

struct Hooks {
    ctx: Ctx,
}

impl CompilerTestCaseHooks for Hooks {
    // port: GatherGetterAndSetterPropertiesTest#getProcessor
    fn get_processor(&mut self, _compiler: CompilerHandle) -> Result<DslValue, Throwable> {
        let pass: Box<dyn CompilerPass> = Box::new(
            |compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId| {
                GatherGetterAndSetterProperties::new().process(compiler, externs, root);
            },
        );
        Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
    }

    fn ctx(&mut self) -> &mut Ctx {
        &mut self.ctx
    }
}

impl GatherGetterAndSetterPropertiesTest {
    // port: CompilerTestCase#CompilerTestCase()
    // port: CompilerTestCase#setUp
    fn new() -> Self {
        let mut harness = CompilerTestCase::new("");
        harness.set_up();
        Self {
            harness,
            hooks: Hooks {
                ctx: Ctx::new(
                    "GatherGetterAndSetterPropertiesTest".into(),
                    closure_testing::replay::replay_values::object([]),
                    IndexMap::<_, _>::default(),
                    Registry::from_tsv("descriptor\tlookup\tdeclaringClass\tsignature\twidened\n")
                        .unwrap(),
                ),
            },
        }
    }

    fn test_same(&mut self, js: &str) {
        self.harness
            .test_same_string(&mut self.hooks, js)
            .unwrap_or_else(|e| panic!("{e:?}"));
    }

    // port: GatherGetterAndSetterPropertiesTest#getLastAccessorSummary (getKind)
    fn get_last_accessor_kind(&self, name: &str) -> PropertyAccessKind {
        let compiler = self.harness.get_last_compiler().expect("lastCompiler");
        let compiler = compiler.borrow();
        compiler
            .get_accessor_summary()
            .expect("accessorSummary")
            .get_kind(&JsString::from(name))
    }
}

// port: GatherGetterAndSetterPropertiesTest#detectObjectCreateWithProperties
#[test]
fn detect_object_create_with_properties() {
    let mut t = GatherGetterAndSetterPropertiesTest::new();
    // Getters and setters in the descriptors of `Object.create(proto, descriptors)` are collected
    // like those of `Object.defineProperties`.
    t.test_same(
        r#"Object.create(null, {
  prop: {
    get() {},
  }
});
"#,
    );

    assert_eq!(
        t.get_last_accessor_kind("prop"),
        PropertyAccessKind::GETTER_ONLY
    );

    t.test_same(
        r#"Object.create(proto, {
  prop: {
    get: function() {},
    set: function(v) {},
  }
});
"#,
    );

    assert_eq!(
        t.get_last_accessor_kind("prop"),
        PropertyAccessKind::GETTER_AND_SETTER
    );

    t.test_same(
        r#"other.Object.create(null, {
  prop: {
    get() {},
  }
});
"#,
    );

    assert_eq!(t.get_last_accessor_kind("prop"), PropertyAccessKind::NORMAL);
}

// port: GatherGetterAndSetterPropertiesTest#noAssumePropertiesAreStaticallyAnalyzable
#[test]
fn no_assume_properties_are_statically_analyzable() {
    let mut options = CompilerOptions::new();
    options.set_assume_properties_are_statically_analyzable(false);
    let mut compiler = Compiler::new();
    compiler.init(&[], &[], options);

    let externs = compiler.get_externs_root().unwrap();
    let root = compiler.get_js_root().unwrap();
    GatherGetterAndSetterProperties::new().process(&mut compiler, externs, root);

    assert_eq!(
        compiler
            .get_accessor_summary()
            .unwrap()
            .get_kind(&JsString::from("dne")),
        PropertyAccessKind::GETTER_AND_SETTER
    );
}
