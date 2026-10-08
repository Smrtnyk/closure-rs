/*
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2013 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CssRenamingMap.java,
//   src/com/google/javascript/jscomp/RenamingMap.java.

// port: CssRenamingMap.Style
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Style {
    BY_WHOLE,
    BY_PART,
}
impl Style {
    pub const VALUES: &'static [Self] = &[Self::BY_WHOLE, Self::BY_PART];
    // port: CssRenamingMap.Style#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl std::fmt::Display for Style {
    // port: CssRenamingMap.Style#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
use crate::renaming_map::RenamingMap;
use closure_rhino::js_string::JsString;
use std::sync::Arc;
pub trait CssRenamingMap: RenamingMap {
    // port: CssRenamingMap#get (inherited RenamingMap#get)
    // The redeclared Java method is provided by the RenamingMap supertrait.
    // port: CssRenamingMap#getStyle
    fn get_style(&self) -> Style;
}
pub struct ByPart {
    map: Arc<dyn RenamingMap>,
}
impl ByPart {
    // port: CssRenamingMap.ByPart#ByPart
    pub fn new(map: Arc<dyn RenamingMap>) -> Self {
        Self { map }
    }
}
impl RenamingMap for ByPart {
    // port: CssRenamingMap.ByPart#get
    fn get(&self, value: &JsString) -> Option<JsString> {
        self.map.get(value)
    }
}
impl CssRenamingMap for ByPart {
    // port: CssRenamingMap.ByPart#getStyle
    fn get_style(&self) -> Style {
        Style::BY_PART
    }
}
pub struct ByWhole {
    map: Arc<dyn RenamingMap>,
}
impl ByWhole {
    // port: CssRenamingMap.ByWhole#ByWhole
    pub fn new(map: Arc<dyn RenamingMap>) -> Self {
        Self { map }
    }
}
impl RenamingMap for ByWhole {
    // port: CssRenamingMap.ByWhole#get
    fn get(&self, value: &JsString) -> Option<JsString> {
        self.map.get(value)
    }
}
impl CssRenamingMap for ByWhole {
    // port: CssRenamingMap.ByWhole#getStyle
    fn get_style(&self) -> Style {
        Style::BY_WHOLE
    }
}
