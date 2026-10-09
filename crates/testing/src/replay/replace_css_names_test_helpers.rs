/*
 * Copyright 2009 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/CssRenamingMap.java,
//   src/com/google/javascript/jscomp/ReplaceCssNames.java,
//   test/com/google/javascript/jscomp/ReplaceCssNamesTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Port of the replay helper `oracle/replay/helpers/.../ReplaceCssNamesTest_Helpers.java` (DSL
//! name `ReplaceCssNamesTest_Helpers`), itself copied from ReplaceCssNamesTest.java: the holder
//! fields useReplacementMap, replacementMap, replacementMapFull, renamingMap, skiplist and
//! cssNames, `getProcessor` (with its `cssNames::add` CssNameCollector), and the anonymous
//! renaming maps of `getPartialMap` (ReplaceCssNamesTest$1) and `getFullMap`
//! (ReplaceCssNamesTest$2).
use crate::{
    replay::replay_dsl::{Ctx, DslValue, NativeObject},
    throwable::Throwable,
};
use closure_jscomp::{
    compiler_pass::CompilerPass,
    css_renaming_map::{CssRenamingMap, Style},
    renaming_map::RenamingMap,
    replace_css_names::ReplaceCssNames,
};
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{js_string::JsString, node::NodeId};
use std::{cell::RefCell, rc::Rc, sync::Arc};

const HOLDER: &str = "com.google.javascript.jscomp.ReplaceCssNamesTest_Helpers";
const PARTIAL_MAP: &str = "com.google.javascript.jscomp.ReplaceCssNamesTest_Helpers$1";
const FULL_MAP: &str = "com.google.javascript.jscomp.ReplaceCssNamesTest_Helpers$2";
const PASS: &str = "com.google.javascript.jscomp.ReplaceCssNames";

// port: ReplaceCssNamesTest_Helpers#replacementMap
const REPLACEMENT_MAP: [(&str, &str); 12] = [
    ("active", "a"),
    ("buttonbar", "b"),
    ("colorswatch", "c"),
    ("disabled", "d"),
    ("elephant", "e"),
    ("footer", "f"),
    ("goog", "g"),
    ("fooStylesBar", "fsr"),
    ("fooStylesBaz", "fsz"),
    ("--foo-bar", "--fb"),
    ("---foo-baz", "--fbz"),
    ("--foo-bar-baz--qux", "--fbzq"),
];

// port: ReplaceCssNamesTest_Helpers#replacementMapFull
const REPLACEMENT_MAP_FULL: [(&str, &str); 9] = [
    ("long-prefix", "h"),
    ("suffix1", "i"),
    ("unrelated-word", "k"),
    ("unrelated", "l"),
    ("long-suffix", "m"),
    ("long-prefix-suffix1", "h-i"),
    ("--foo-bar", "--fb"),
    ("---foo-baz", "--fbz"),
    ("--foo-bar-baz--qux", "--fbzq"),
];

fn string_map(entries: &[(&str, &str)]) -> DslValue {
    DslValue::Typed {
        class: "com.google.common.collect.RegularImmutableMap".into(),
        value: Box::new(DslValue::Map(
            entries
                .iter()
                .map(|(k, v)| {
                    (
                        DslValue::String(JsString::from(*k)),
                        DslValue::String(JsString::from(*v)),
                    )
                })
                .collect(),
        )),
    }
}

/// The anonymous `CssRenamingMap.ByPart` (getPartialMap) or `CssRenamingMap.ByWhole`
/// (getFullMap) reading the holder's never-reassigned replacement maps.
#[derive(Clone, Copy)]
pub struct TestCssRenamingMap {
    style: Style,
    map: &'static [(&'static str, &'static str)],
}

impl RenamingMap for TestCssRenamingMap {
    // port: ReplaceCssNamesTest_Helpers$1#get / ReplaceCssNamesTest_Helpers$2#get
    fn get(&self, value: &JsString) -> Option<JsString> {
        self.map
            .iter()
            .find(|(k, _)| *value == **k)
            .map(|(_, v)| JsString::from(*v))
    }
}

impl CssRenamingMap for TestCssRenamingMap {
    // port: CssRenamingMap.ByPart#getStyle / CssRenamingMap.ByWhole#getStyle
    fn get_style(&self) -> Style {
        self.style
    }
}

impl NativeObject for TestCssRenamingMap {
    fn class_name(&self) -> &str {
        match self.style {
            Style::BY_PART => PARTIAL_MAP,
            Style::BY_WHOLE => FULL_MAP,
        }
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class_name()
            || class == "com.google.javascript.jscomp.CssRenamingMap"
            || class == "com.google.javascript.jscomp.RenamingMap"
    }
    // port: UnitRecorder#collect (the anonymous maps have no fields of their own)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// `final class ReplaceCssNamesTest_Helpers extends CompilerTestCase`.
pub struct ReplaceCssNamesTestHelpers {
    use_replacement_map: bool,
    renaming_map: DslValue,
    skiplist: DslValue,
    /// Shared with the CssNameCollector of the passes `getProcessor` created.
    css_names: Rc<RefCell<DslValue>>,
}

impl NativeObject for ReplaceCssNamesTestHelpers {
    fn class_name(&self) -> &str {
        HOLDER
    }
    // port: ReplayValues#findField (native object adapter)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let mut fields = IndexMap::<_, _>::default();
        fields.insert(
            "useReplacementMap".into(),
            DslValue::Bool(self.use_replacement_map),
        );
        fields.insert("replacementMap".into(), string_map(&REPLACEMENT_MAP));
        fields.insert(
            "replacementMapFull".into(),
            string_map(&REPLACEMENT_MAP_FULL),
        );
        fields.insert("renamingMap".into(), self.renaming_map.clone());
        fields.insert("skiplist".into(), self.skiplist.clone());
        fields.insert("cssNames".into(), self.css_names.borrow().clone());
        Ok(fields)
    }
    // port: ReplayValues#setField (native object adapter)
    fn set_field(&mut self, name: &str, value: DslValue) -> Result<(), Throwable> {
        match name {
            "useReplacementMap" => {
                let DslValue::Bool(b) = value else {
                    return Err(bad());
                };
                self.use_replacement_map = b;
            }
            "renamingMap" => self.renaming_map = value,
            "skiplist" => self.skiplist = value,
            "cssNames" => *self.css_names.borrow_mut() = value,
            _ => return Err(Throwable::Unported(format!("{HOLDER}#{name}"))),
        }
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ReplaceCssNamesTest_Helpers#ReplaceCssNamesTest_Helpers
pub fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(DslValue::Native(Rc::new(RefCell::new(
        ReplaceCssNamesTestHelpers {
            use_replacement_map: false,
            renaming_map: DslValue::Null,
            skiplist: DslValue::Null,
            css_names: Rc::new(RefCell::new(DslValue::Null)),
        },
    ))))
}

fn with_holder<R>(
    args: &[DslValue],
    f: impl FnOnce(&mut ReplaceCssNamesTestHelpers) -> Result<R, Throwable>,
) -> Result<R, Throwable> {
    let Some(DslValue::Native(this)) = args.first() else {
        return Err(bad());
    };
    let mut this = this.borrow_mut();
    let this = this
        .as_any_mut()
        .downcast_mut::<ReplaceCssNamesTestHelpers>()
        .ok_or_else(bad)?;
    f(this)
}

// port: ReplaceCssNamesTest_Helpers#getPartialMap
pub fn get_partial_map(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    with_holder(&args, |_| {
        Ok(DslValue::Native(Rc::new(RefCell::new(
            TestCssRenamingMap {
                style: Style::BY_PART,
                map: &REPLACEMENT_MAP,
            },
        ))))
    })
}

// port: ReplaceCssNamesTest_Helpers#getFullMap
pub fn get_full_map(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    with_holder(&args, |_| {
        Ok(DslValue::Native(Rc::new(RefCell::new(
            TestCssRenamingMap {
                style: Style::BY_WHOLE,
                map: &REPLACEMENT_MAP_FULL,
            },
        ))))
    })
}

/// `Set<String>#add` on the holder's cssNames (a `HashSet`, possibly behind its class tag).
// port: ReplaceCssNamesTest_Helpers#getProcessor (cssNames::add)
fn add_css_name(css_names: &mut DslValue, css_name: &JsString) {
    let set = match css_names {
        DslValue::Typed { value, .. } => value.as_mut(),
        other => other,
    };
    match set {
        DslValue::Set(items) => {
            let v = DslValue::String(css_name.clone());
            if !items.iter().any(|x| x.equals(&v)) {
                items.push(v);
            }
        }
        _ => panic!("NullPointerException: cssNames"),
    }
}

/// The ReplaceCssNames pass that getProcessor returned.
struct NativeReplaceCssNames(ReplaceCssNames<'static>);

impl NativeObject for NativeReplaceCssNames {
    fn class_name(&self) -> &str {
        PASS
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == PASS || class == "com.google.javascript.jscomp.CompilerPass"
    }
    // port: UnitRecorder#collect (no recorded result producer is reachable from this pass)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::<_, _>::default())
    }
    // port: ReplaceCssNames#process
    fn process(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        self.0.process(compiler, externs, root);
        Ok(())
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

// port: ReplaceCssNamesTest_Helpers#getProcessor
pub fn get_processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [_, DslValue::Compiler(compiler)] = args.as_slice() else {
        return Err(bad());
    };
    let (renaming_map, css_names, skiplist) = with_holder(&args, |this| {
        let renaming_map: Option<Arc<dyn CssRenamingMap>> = if this.use_replacement_map {
            match &this.renaming_map {
                DslValue::Null => None,
                DslValue::Native(o) => {
                    let mut o = o.borrow_mut();
                    let map = o
                        .as_any_mut()
                        .downcast_mut::<TestCssRenamingMap>()
                        .ok_or_else(bad)?;
                    Some(Arc::new(*map))
                }
                _ => return Err(bad()),
            }
        } else {
            None
        };
        let skiplist = match &this.skiplist {
            DslValue::Null => None,
            skiplist => {
                let items = match skiplist {
                    DslValue::Typed { value, .. } => value.as_ref(),
                    other => other,
                };
                let DslValue::Set(items) = items else {
                    return Err(bad());
                };
                let mut set = IndexSet::<_>::default();
                for item in items {
                    let DslValue::String(s) = item else {
                        return Err(bad());
                    };
                    set.insert(s.to_string());
                }
                Some(set)
            }
        };
        if matches!(*this.css_names.borrow(), DslValue::Null) {
            // `cssNames::add` on a null receiver
            return Err(Throwable::Exception {
                class: "java.lang.NullPointerException".into(),
                message: None,
            });
        }
        Ok((renaming_map, this.css_names.clone(), skiplist))
    })?;
    let pass = ReplaceCssNames::new(
        &mut compiler.borrow_mut(),
        renaming_map,
        Box::new(move |css_name: &JsString| {
            add_css_name(&mut css_names.borrow_mut(), css_name);
        }),
        skiplist,
    );
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeReplaceCssNames(pass),
    ))))
}

fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
