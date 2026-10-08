/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/deps/ModuleLoader.java.

use super::{
    browser_module_resolver::BrowserModuleResolver, dependency_info::DependencyInfo,
    module_names::ModuleNames, module_resolver::ModuleResolver,
};
use crate::{
    check_level::CheckLevel, diagnostic_type::DiagnosticType, error_handler::ErrorHandler,
    js_error::JSError,
};
use closure_rhino::{
    check_not_null, check_state,
    java_lang::{string_compare_to, unix_path::UnixPath},
};
use indexmap::{IndexMap, IndexSet};
use std::{
    cmp::Ordering,
    fmt,
    sync::{Arc, LazyLock, Mutex},
};
pub static MODULE_CONFLICT: DiagnosticType = DiagnosticType::warning(
    "JSC_MODULE_CONFLICT",
    "File cannot be a combination of goog.provide, goog.module, and/or ES6 module: {0}",
);
pub static LOAD_WARNING: DiagnosticType = DiagnosticType::error(
    "JSC_JS_MODULE_LOAD_WARNING",
    "Failed to load module \"{0}\"",
);
pub static INVALID_MODULE_PATH: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_MODULE_PATH",
    "Invalid module path \"{0}\" for resolution mode \"{1}\"",
);
pub type SharedErrorHandler = Arc<Mutex<dyn ErrorHandler + Send>>;
struct NoopErrorHandler;
impl ErrorHandler for NoopErrorHandler {
    // port: ModuleLoader#NOOP_ERROR_HANDER
    fn report(&mut self, _level: CheckLevel, _error: JSError) {}
}
pub static NOOP_ERROR_HANDER: LazyLock<SharedErrorHandler> =
    LazyLock::new(|| Arc::new(Mutex::new(NoopErrorHandler)));
struct LoaderData {
    error_handler: Mutex<SharedErrorHandler>,
    module_root_paths: Vec<String>,
    path_resolver: PathResolver,
    path_escaper: PathEscaper,
    module_resolver: Arc<dyn ModuleResolver>,
}
#[derive(Clone)]
pub struct ModuleLoader {
    data: Arc<LoaderData>,
}
impl ModuleLoader {
    pub const EMPTY: &'static LazyLock<Self> = &EMPTY;
    pub const MODULE_SLASH: &'static str = ModuleNames::MODULE_SLASH;
    pub const DEFAULT_FILENAME_PREFIX: &'static str = "./";
    pub const JSC_BROWSER_SKIPLISTED_MARKER: &'static str = "$jscomp$browser$skiplisted";
    // port: ModuleLoader#builder
    pub fn builder() -> Builder {
        Builder::new()
    }
    // port: ModuleLoader#ModuleLoader
    fn new(builder: Builder) -> Self {
        let roots = check_not_null!(builder.module_roots);
        let module_root_paths =
            Self::create_root_paths(roots, builder.path_resolver, builder.path_escaper);
        let inputs = check_not_null!(builder.inputs);
        let module_paths = Self::resolve_paths(
            inputs,
            &module_root_paths,
            builder.path_resolver,
            builder.path_escaper,
        );
        let error_handler = builder.error_handler;
        let module_resolver = check_not_null!(builder.factory).create(
            module_paths,
            module_root_paths.clone(),
            error_handler.clone(),
            builder.path_escaper,
        );
        Self {
            data: Arc::new(LoaderData {
                error_handler: Mutex::new(error_handler),
                module_root_paths,
                path_resolver: builder.path_resolver,
                path_escaper: builder.path_escaper,
                module_resolver,
            }),
        }
    }
    // port: ModuleLoader#getPackageJsonMainEntries
    pub fn get_package_json_main_entries(&self) -> IndexMap<String, String> {
        self.data.module_resolver.get_package_json_main_entries()
    }
    // port: ModuleLoader#resolve
    pub fn resolve(&self, path: &str) -> ModulePath {
        ModulePath::new(
            Self::normalize(
                &self
                    .data
                    .path_escaper
                    .escape(&self.data.path_resolver.apply(path)),
                &self.data.module_root_paths,
            ),
            self.data.module_resolver.clone(),
        )
    }
    // port: ModuleLoader#isRelativeIdentifier
    pub fn is_relative_identifier(name: &str) -> bool {
        name.starts_with("./") || name.starts_with("../") || name == "." || name == ".."
    }
    // port: ModuleLoader#isAbsoluteIdentifier
    pub fn is_absolute_identifier(name: &str) -> bool {
        name.starts_with('/')
    }
    // port: ModuleLoader#isAmbiguousIdentifier
    pub fn is_ambiguous_identifier(name: &str) -> bool {
        !Self::is_absolute_identifier(name) && !Self::is_relative_identifier(name)
    }
    // port: ModuleLoader#isPathIdentifier
    pub fn is_path_identifier(name: &str) -> bool {
        name.contains('/')
    }
    // port: ModuleLoader#createRootPaths
    fn create_root_paths(
        roots: Vec<String>,
        resolver: PathResolver,
        escaper: PathEscaper,
    ) -> Vec<String> {
        let mut roots: Vec<_> = roots
            .into_iter()
            .map(|n| escaper.escape(&resolver.apply(&n)))
            .map(|n| {
                if Self::is_ambiguous_identifier(&n) {
                    format!("/{n}")
                } else {
                    n
                }
            })
            .collect();
        roots.sort_by(|a, b| best_match_path_ordering(a, b));
        roots.dedup();
        roots
    }
    // port: ModuleLoader#resolvePaths
    fn resolve_paths(
        inputs: Vec<String>,
        roots: &[String],
        resolver: PathResolver,
        escaper: PathEscaper,
    ) -> IndexSet<String> {
        let mut paths: Vec<_> = inputs
            .into_iter()
            .map(|p| Self::normalize(&escaper.escape(&resolver.apply(&p)), roots))
            .map(|n| {
                if Self::is_ambiguous_identifier(&n) {
                    format!("/{n}")
                } else {
                    n
                }
            })
            .collect();
        paths.sort_by(|a, b| best_match_path_ordering(a, b));
        let total = paths.len();
        let mut dupe_module_paths: IndexMap<String, usize> = IndexMap::new();
        for p in paths {
            *dupe_module_paths.entry(p).or_default() += 1;
        }
        let entries: Vec<_> = dupe_module_paths
            .iter()
            .map(|(p, count)| {
                if *count == 1 {
                    p.clone()
                } else {
                    format!("{p} x {count}")
                }
            })
            .collect();
        check_state!(
            total == dupe_module_paths.len(),
            "Duplicate module paths after resolving: %s",
            format!("[{}]", entries.join(", "))
        );
        dupe_module_paths.into_keys().collect()
    }
    // port: ModuleLoader#normalize
    pub fn normalize(path: &str, module_root_paths: &[String]) -> String {
        let normalized_path = if Self::is_ambiguous_identifier(path) {
            format!("/{path}")
        } else {
            path.into()
        };
        for root in module_root_paths {
            if let Some(trailing) = normalized_path.strip_prefix(root)
                && let Some(trailing) = trailing.strip_prefix('/')
            {
                return trailing.into();
            }
        }
        path.into()
    }
    // port: ModuleLoader#relativePathFrom
    pub fn relative_path_from(from_uri_path: &str, to_uri_path: &str) -> String {
        Self::try_relative_path_from(from_uri_path, to_uri_path)
            .unwrap_or_else(|message| panic!("{message}"))
    }
    /// Rust-only: `relativePathFrom` with its `IllegalArgumentException` as an `Err` holding the
    /// message, for callers that catch it as control flow (RewriteDynamicImports).
    // port: ModuleLoader#relativePathFrom
    pub fn try_relative_path_from(
        from_uri_path: &str,
        to_uri_path: &str,
    ) -> Result<String, String> {
        let from_path = UnixPath::of(from_uri_path);
        let to_path = UnixPath::of(to_uri_path);
        let from_folder = from_path.get_parent();
        if from_folder.is_none() && to_path.get_parent().is_none() {
            return Ok(format!("./{to_uri_path}"));
        }
        if from_folder.is_none()
            && (to_uri_path.starts_with('.')
                || to_path.to_string().starts_with('/')
                || to_path.to_string().starts_with('\\'))
        {
            return Ok(to_uri_path.into());
        }
        let Some(from_folder) = from_folder else {
            return Err("Relative path between URIs cannot be calculated".into());
        };
        let calculated_path = from_folder.try_relativize(&to_path)?.to_string();
        if calculated_path.starts_with('.') || calculated_path.starts_with('/') {
            Ok(calculated_path)
        } else {
            Ok(format!("./{calculated_path}"))
        }
    }
    // port: ModuleLoader#setErrorHandler
    pub fn set_error_handler(&self, handler: Option<SharedErrorHandler>) {
        let handler = handler.unwrap_or_else(|| NOOP_ERROR_HANDER.clone());
        *self.data.error_handler.lock().unwrap() = handler.clone();
        self.data.module_resolver.set_error_handler(handler);
    }
    // port: ModuleLoader#getErrorHandler
    pub fn get_error_handler(&self) -> SharedErrorHandler {
        self.data.error_handler.lock().unwrap().clone()
    }
}
// port: ModuleLoader#BEST_MATCH_PATH_ORDERING
pub(crate) fn best_match_path_ordering(a: &str, b: &str) -> Ordering {
    b.encode_utf16()
        .count()
        .cmp(&a.encode_utf16().count())
        .then_with(|| string_compare_to(a, b).cmp(&0))
}
pub struct Builder {
    error_handler: SharedErrorHandler,
    module_roots: Option<Vec<String>>,
    inputs: Option<Vec<String>>,
    factory: Option<Arc<dyn ModuleResolverFactory>>,
    path_resolver: PathResolver,
    path_escaper: PathEscaper,
}
impl Builder {
    // port: ModuleLoader.Builder#Builder
    fn new() -> Self {
        Self {
            error_handler: NOOP_ERROR_HANDER.clone(),
            module_roots: None,
            inputs: None,
            factory: None,
            path_resolver: PathResolver::RELATIVE,
            path_escaper: PathEscaper::ESCAPE,
        }
    }
    // port: ModuleLoader.Builder#setErrorHandler
    pub fn set_error_handler(mut self, x: Option<SharedErrorHandler>) -> Self {
        if let Some(x) = x {
            self.error_handler = x;
        }
        self
    }
    // port: ModuleLoader.Builder#setModuleRoots
    pub fn set_module_roots(mut self, x: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.module_roots = Some(x.into_iter().map(Into::into).collect());
        self
    }
    // port: ModuleLoader.Builder#setInputs
    pub fn set_inputs<I: DependencyInfo>(mut self, x: impl IntoIterator<Item = I>) -> Self {
        self.inputs = Some(x.into_iter().map(|d| d.get_name().to_owned()).collect());
        self
    }
    // port: ModuleLoader.Builder#setFactory
    pub fn set_factory(mut self, x: Arc<dyn ModuleResolverFactory>) -> Self {
        self.factory = Some(x);
        self
    }
    // port: ModuleLoader.Builder#setPathResolver
    pub fn set_path_resolver(mut self, x: PathResolver) -> Self {
        self.path_resolver = x;
        self
    }
    // port: ModuleLoader.Builder#setPathEscaper
    pub fn set_path_escaper(mut self, x: PathEscaper) -> Self {
        self.path_escaper = x;
        self
    }
    // port: ModuleLoader.Builder#build
    pub fn build(self) -> ModuleLoader {
        ModuleLoader::new(self)
    }
}
#[derive(Clone)]
pub struct ModulePath {
    path: String,
    module_resolver: Arc<dyn ModuleResolver>,
}
impl ModulePath {
    // port: ModuleLoader.ModulePath#ModulePath
    fn new(path: String, module_resolver: Arc<dyn ModuleResolver>) -> Self {
        Self {
            path,
            module_resolver,
        }
    }
    // port: ModuleLoader.ModulePath#equalsIgnoreLeadingSlash
    pub fn equals_ignore_leading_slash(&self, other: Option<&Self>) -> bool {
        other.is_some_and(|o| self.to_module_name() == o.to_module_name())
    }
    // port: ModuleLoader.ModulePath#toModuleName
    pub fn to_module_name(&self) -> String {
        ModuleNames::to_module_name(&self.path)
    }
    // port: ModuleLoader.ModulePath#resolveJsModule
    pub fn resolve_js_module(
        &self,
        module_address: &str,
        sourcename: Option<&str>,
        lineno: i32,
        colno: i32,
    ) -> Option<Self> {
        self.module_resolver
            .resolve_js_module(&self.path, module_address, sourcename, lineno, colno)
            .map(|p| Self::new(p, self.module_resolver.clone()))
    }
    // port: ModuleLoader.ModulePath#resolveJsModuleSilently
    pub fn resolve_js_module_silently(&self, module_address: &str) -> Option<Self> {
        self.module_resolver
            .resolve_js_module_silently(&self.path, module_address)
            .map(|p| Self::new(p, self.module_resolver.clone()))
    }
    // port: ModuleLoader.ModulePath#resolveModuleAsPath
    pub fn resolve_module_as_path(&self, module_address: &str) -> Self {
        Self::new(
            self.module_resolver
                .resolve_module_as_path(&self.path, module_address),
            self.module_resolver.clone(),
        )
    }
}
impl fmt::Display for ModulePath {
    // port: ModuleLoader.ModulePath#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathEscaper {
    ESCAPE,
    CANONICALIZE_ONLY,
}
impl PathEscaper {
    pub const VALUES: &'static [Self] = &[Self::ESCAPE, Self::CANONICALIZE_ONLY];
    // port: ModuleLoader.PathEscaper#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
    // port: ModuleLoader.PathEscaper#escape
    pub fn escape(self, path: &str) -> String {
        match self {
            Self::ESCAPE => ModuleNames::escape_path(path),
            Self::CANONICALIZE_ONLY => ModuleNames::canonicalize_path(path),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum PathResolver {
    RELATIVE,
    ABSOLUTE,
}
impl PathResolver {
    // port: ModuleLoader.PathResolver#apply
    pub fn apply(self, path: &str) -> String {
        match self {
            Self::RELATIVE => path.into(),
            Self::ABSOLUTE => UnixPath::of(path).to_absolute_path().to_string(),
        }
    }
}
pub trait ModuleResolverFactory: Send + Sync {
    // port: ModuleLoader.ModuleResolverFactory#create
    fn create(
        &self,
        module_paths: IndexSet<String>,
        module_root_paths: Vec<String>,
        error_handler: SharedErrorHandler,
        path_escaper: PathEscaper,
    ) -> Arc<dyn ModuleResolver>;
}
pub static EMPTY: LazyLock<ModuleLoader> = LazyLock::new(|| {
    ModuleLoader::builder()
        .set_module_roots(Vec::<String>::new())
        .set_inputs(Vec::<super::simple_dependency_info::SimpleDependencyInfo>::new())
        .set_factory(Arc::clone(BrowserModuleResolver::FACTORY))
        .build()
});
impl fmt::Display for PathEscaper {
    // port: ModuleLoader.PathEscaper#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResolutionMode {
    BROWSER,
    BROWSER_WITH_TRANSFORMED_PREFIXES,
    NODE,
    WEBPACK,
}
impl ResolutionMode {
    pub const VALUES: &'static [Self] = &[
        Self::BROWSER,
        Self::BROWSER_WITH_TRANSFORMED_PREFIXES,
        Self::NODE,
        Self::WEBPACK,
    ];
    // port: ModuleLoader.ResolutionMode#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
}
impl fmt::Display for ResolutionMode {
    // port: ModuleLoader.ResolutionMode#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
#[cfg(test)]
mod tests;
