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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/deps/ClosureBundler.java.

use super::{dependency_info::DependencyInfo, source_code_escapers::SourceCodeEscapers};
use crate::transpile::{
    base_transpiler::LATEST_TRANSPILER,
    transpiler::{NULL, Transpiler},
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{
    java_lang::{charset::Charset, uri::URI},
    js_string::JsString,
};
use std::{
    any::Any,
    fmt, io,
    path::Path,
    sync::{Arc, Mutex},
};
/// Java Appendable, with a UTF-16 destination available for source text containing lone surrogates.
pub trait Appendable {
    // port: Appendable#append(CharSequence)
    fn append(&mut self, s: &JsString) -> fmt::Result;
    // port: Appendable#append(CharSequence)
    fn append_str(&mut self, s: &str) -> fmt::Result {
        self.append(&JsString::from(s))
    }
}
impl Appendable for JsString {
    // port: StringBuilder#append(CharSequence)
    fn append(&mut self, s: &JsString) -> fmt::Result {
        *self = self.concat(s);
        Ok(())
    }
}
impl Appendable for String {
    // port: StringBuilder#append(CharSequence) (Unicode scalar destination)
    fn append(&mut self, s: &JsString) -> fmt::Result {
        self.push_str(&String::from_utf16(s.as_units()).map_err(|_| fmt::Error)?);
        Ok(())
    }
}
#[derive(Clone)]
pub struct ClosureBundler {
    transpiler: Arc<dyn Transpiler>,
    es6_module_transpiler: Arc<dyn Transpiler>,
    mode: EvalMode,
    source_url: Option<String>,
    path: String,
    embed_sourcemap: bool,
    source_map_cache: Arc<Mutex<IndexMap<String, JsString>>>,
    minifier: Option<Arc<dyn Any + Send + Sync>>,
}
impl ClosureBundler {
    // port: ClosureBundler#ClosureBundler()
    pub fn new_default() -> Self {
        Self::new_with_transpiler(NULL.clone())
    }
    // port: ClosureBundler#ClosureBundler(Transpiler)
    pub fn new_with_transpiler(transpiler: Arc<dyn Transpiler>) -> Self {
        Self::new(transpiler, LATEST_TRANSPILER.clone())
    }
    // port: ClosureBundler#ClosureBundler(Transpiler,Transpiler)
    pub fn new(
        transpiler: Arc<dyn Transpiler>,
        es6_module_transpiler: Arc<dyn Transpiler>,
    ) -> Self {
        Self::new_with_options(
            transpiler,
            es6_module_transpiler,
            EvalMode::NORMAL,
            None,
            "unknown_source".into(),
            None,
            Arc::new(Mutex::new(IndexMap::<_, _>::default())),
            false,
        )
    }
    // port: ClosureBundler#ClosureBundler (private constructor)
    #[allow(clippy::too_many_arguments)] // The Java constructor's fields and parameter order.
    fn new_with_options(
        transpiler: Arc<dyn Transpiler>,
        es6_module_transpiler: Arc<dyn Transpiler>,
        mode: EvalMode,
        source_url: Option<String>,
        path: String,
        minifier: Option<Arc<dyn Any + Send + Sync>>,
        source_map_cache: Arc<Mutex<IndexMap<String, JsString>>>,
        embed_sourcemap: bool,
    ) -> Self {
        Self {
            transpiler,
            es6_module_transpiler,
            mode,
            source_url,
            path,
            minifier,
            source_map_cache,
            embed_sourcemap,
        }
    }
    // port: ClosureBundler#withTranspilers
    pub fn with_transpilers(
        &self,
        new_transpiler: Arc<dyn Transpiler>,
        new_es6_module_transpiler: Arc<dyn Transpiler>,
    ) -> Self {
        Self::new_with_options(
            new_transpiler,
            new_es6_module_transpiler,
            self.mode,
            self.source_url.clone(),
            self.path.clone(),
            self.minifier.clone(),
            self.source_map_cache.clone(),
            self.embed_sourcemap,
        )
    }
    // port: ClosureBundler#withTranspiler
    pub fn with_transpiler(&self, new_transpiler: Arc<dyn Transpiler>) -> Self {
        self.with_transpilers(new_transpiler, self.es6_module_transpiler.clone())
    }
    // port: ClosureBundler#withEs6ModuleTranspiler
    pub fn with_es6_module_transpiler(&self, new_transpiler: Arc<dyn Transpiler>) -> Self {
        self.with_transpilers(self.transpiler.clone(), new_transpiler)
    }
    // port: ClosureBundler#disableJ2clMinifier
    pub fn disable_j2cl_minifier(&self) -> Self {
        Self::new_with_options(
            self.transpiler.clone(),
            self.es6_module_transpiler.clone(),
            self.mode,
            self.source_url.clone(),
            self.path.clone(),
            None,
            self.source_map_cache.clone(),
            self.embed_sourcemap,
        )
    }
    // port: ClosureBundler#useEval
    pub fn use_eval(&self, use_eval: bool) -> Self {
        let new_mode = if use_eval {
            EvalMode::EVAL
        } else {
            EvalMode::NORMAL
        };
        Self::new_with_options(
            self.transpiler.clone(),
            self.es6_module_transpiler.clone(),
            new_mode,
            self.source_url.clone(),
            self.path.clone(),
            self.minifier.clone(),
            self.source_map_cache.clone(),
            self.embed_sourcemap,
        )
    }
    // port: ClosureBundler#withSourceUrl
    pub fn with_source_url(&self, new_source_url: Option<&str>) -> Self {
        Self::new_with_options(
            self.transpiler.clone(),
            self.es6_module_transpiler.clone(),
            self.mode,
            new_source_url.map(str::to_owned),
            self.path.clone(),
            self.minifier.clone(),
            self.source_map_cache.clone(),
            self.embed_sourcemap,
        )
    }
    // port: ClosureBundler#withPath
    pub fn with_path(&self, new_path: &str) -> Self {
        Self::new_with_options(
            self.transpiler.clone(),
            self.es6_module_transpiler.clone(),
            self.mode,
            self.source_url.clone(),
            new_path.into(),
            self.minifier.clone(),
            self.source_map_cache.clone(),
            self.embed_sourcemap,
        )
    }
    // port: ClosureBundler#embedSourcemap
    pub fn embed_sourcemap(&self) -> Self {
        Self::new_with_options(
            self.transpiler.clone(),
            self.es6_module_transpiler.clone(),
            self.mode,
            self.source_url.clone(),
            self.path.clone(),
            self.minifier.clone(),
            self.source_map_cache.clone(),
            true,
        )
    }
    /// Append the contents of the string to the supplied appendable.
    // port: ClosureBundler#appendInput
    pub fn append_input(
        out: &mut dyn Appendable,
        info: &dyn DependencyInfo,
        contents: impl Into<JsString>,
    ) -> fmt::Result {
        ClosureBundler::new_default().append_to(out, info, contents)
    }
    // port: ClosureBundler#appendTo(Appendable,DependencyInfo,String), appendTo(Appendable,DependencyInfo,CharSource)
    pub fn append_to(
        &self,
        out: &mut dyn Appendable,
        info: &dyn DependencyInfo,
        content: impl Into<JsString>,
    ) -> fmt::Result {
        let code = content.into();
        if info.is_goog_module() {
            self.mode
                .append_goog_module(&self.transpile(&code), out, self.source_url.as_deref())
        } else if info.is_es6_module() && Arc::ptr_eq(&self.transpiler, &NULL) {
            self.mode.append_traditional(
                &self.transpile_es6_module(&code),
                out,
                self.source_url.as_deref(),
            )
        } else {
            self.mode
                .append_traditional(&self.transpile(&code), out, self.source_url.as_deref())
        }
    }
    // port: ClosureBundler#appendTo(Appendable,DependencyInfo,File,Charset)
    pub fn append_to_file(
        &self,
        out: &mut dyn Appendable,
        info: &dyn DependencyInfo,
        content: impl AsRef<Path>,
        content_charset: &str,
    ) -> io::Result<()> {
        let bytes = std::fs::read(content)?;
        let code = Charset::for_name(content_charset)
            .decode(&bytes, true)
            .unwrap();
        self.append_to(out, info, code).map_err(io::Error::other)
    }
    // port: ClosureBundler#appendRuntimeTo
    pub fn append_runtime_to(&self, out: &mut dyn Appendable) -> fmt::Result {
        let runtime = self.transpiler.runtime();
        if !runtime.is_empty() {
            self.mode.append_traditional(&runtime, out, None)?;
        }
        if Arc::ptr_eq(&self.transpiler, &NULL) {
            self.mode
                .append_traditional(&self.es6_module_transpiler.runtime(), out, None)?;
        }
        self.mode.append_traditional(
            &JsString::from("this.CLOSURE_EVAL_PREFILTER = function(s) { return s; };"),
            out,
            None,
        )?;
        self.mode
            .append_traditional(&JsString::from("(function(thisValue){"), out, None)?;
        self.mode.append_traditional(
            &JsString::from(concat!(
                "var isChrome87 = false; try {isChrome87 =  eval(trustedTypes.emptyScript) !==",
                " trustedTypes.emptyScript } catch (e) {} if (typeof trustedTypes !==",
                " 'undefined' && trustedTypes.createPolicy &&isChrome87 ) {"
            )),
            out,
            None,
        )?;
        self.mode.append_traditional(&JsString::from(concat!("  var policy = trustedTypes.createPolicy('goog#devserver',{ createScript: function(s){"," return s; }});")),out,None)?;
        self.mode.append_traditional(
            &JsString::from(
                "  thisValue.CLOSURE_EVAL_PREFILTER = policy.createScript.bind(policy);",
            ),
            out,
            None,
        )?;
        self.mode
            .append_traditional(&JsString::from("}"), out, None)?;
        self.mode
            .append_traditional(&JsString::from("})(this);"), out, None)
    }
    // port: ClosureBundler#getSourceMap
    pub fn get_source_map(&self, path: &str) -> JsString {
        self.source_map_cache
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .unwrap_or_else(|| JsString::from(""))
    }
    // port: ClosureBundler#transpile(String,Transpiler)
    fn transpile_with(&self, s: &JsString, t: &dyn Transpiler) -> JsString {
        let uri =
            URI::new(&self.path).unwrap_or_else(|e| panic!("java.net.URISyntaxException: {e}"));
        let result = t.transpile(uri, s);
        self.source_map_cache
            .lock()
            .unwrap()
            .insert(self.path.clone(), result.source_map().clone());
        if self.embed_sourcemap {
            result.embed_sourcemap_base64().transpiled().clone()
        } else {
            result.transpiled().clone()
        }
    }
    // port: ClosureBundler#transpile(String)
    fn transpile(&self, s: &JsString) -> JsString {
        self.transpile_with(s, &*self.transpiler)
    }
    // port: ClosureBundler#transpileEs6Module
    fn transpile_es6_module(&self, s: &JsString) -> JsString {
        self.transpile_with(s, &*self.es6_module_transpiler)
    }
    // port: ClosureBundler#appendSourceUrl
    fn append_source_url(
        out: &mut dyn Appendable,
        mode: EscapeMode,
        source_url: Option<&str>,
    ) -> fmt::Result {
        let Some(source_url) = source_url else {
            return Ok(());
        };
        let to_append = JsString::from(format!("\n//# sourceURL={source_url}\n"));
        mode.append(&to_append, out)
    }
}
#[derive(Clone, Copy)]
pub enum EvalMode {
    EVAL,
    NORMAL,
}
impl EvalMode {
    // port: ClosureBundler.EvalMode#appendTraditional
    pub fn append_traditional(
        self,
        s: &JsString,
        out: &mut dyn Appendable,
        source_url: Option<&str>,
    ) -> fmt::Result {
        match self {
            Self::EVAL => {
                out.append_str("eval(this.CLOSURE_EVAL_PREFILTER(\"")?;
                EscapeMode::ESCAPED.append(s, out)?;
                ClosureBundler::append_source_url(out, EscapeMode::ESCAPED, source_url)?;
                out.append_str("\"));\n")
            }
            Self::NORMAL => {
                EscapeMode::NORMAL.append(s, out)?;
                ClosureBundler::append_source_url(out, EscapeMode::NORMAL, source_url)
            }
        }
    }
    // port: ClosureBundler.EvalMode#appendGoogModule
    pub fn append_goog_module(
        self,
        s: &JsString,
        out: &mut dyn Appendable,
        source_url: Option<&str>,
    ) -> fmt::Result {
        match self {
            Self::EVAL => {
                out.append_str("goog.loadModule(\"")?;
                EscapeMode::ESCAPED.append(s, out)?;
                ClosureBundler::append_source_url(out, EscapeMode::ESCAPED, source_url)?;
                out.append_str("\");\n")
            }
            Self::NORMAL => {
                out.append_str("goog.loadModule(function(exports) {'use strict';")?;
                EscapeMode::NORMAL.append(s, out)?;
                out.append_str("\n;return exports;});\n")?;
                ClosureBundler::append_source_url(out, EscapeMode::NORMAL, source_url)
            }
        }
    }
}
#[derive(Clone, Copy)]
pub enum EscapeMode {
    ESCAPED,
    NORMAL,
}
impl EscapeMode {
    // port: ClosureBundler.EscapeMode#append
    pub fn append(self, s: &JsString, out: &mut dyn Appendable) -> fmt::Result {
        match self {
            Self::ESCAPED => {
                SourceCodeEscapers::append_with_javascript_escaper(s, &mut AppendableWriter(out))
            }
            Self::NORMAL => out.append(s),
        }
    }
}
struct AppendableWriter<'a>(&'a mut dyn Appendable);
impl fmt::Write for AppendableWriter<'_> {
    // port: Appendable#append(CharSequence)
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.0.append_str(s)
    }
    // port: Appendable#append(char)
    fn write_char(&mut self, c: char) -> fmt::Result {
        self.0.append(&JsString::from(c.to_string()))
    }
}
#[cfg(test)]
mod tests;
