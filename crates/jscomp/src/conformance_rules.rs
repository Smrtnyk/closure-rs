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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/CheckConformance.java,
//   src/com/google/javascript/jscomp/ConformanceRules.java.

//! Port of `ConformanceRules.java`: standard conformance rules.

use crate::{
    AbstractCompiler,
    check_conformance::{InvalidRequirementSpec, Precondition, Rule},
    closure_rewrite_module::ClosureRewriteModule,
    coding_convention::AssertionFunctionLookup,
    compiler_input::CompilerInput,
    conformance_config::{
        LibraryLevelNonAllowlistedConformanceViolationsBehavior, Requirement,
        RequirementScopeEntry,
        requirement::{self, Severity},
    },
    diagnostic_type::DiagnosticType,
    error_manager::ErrorManager,
    global_namespace::GlobalNamespace,
    js_error::JSError,
    node_traversal::NodeTraversal,
    node_util::NodeUtil,
    scope::ScopeId,
    scoped_aliases::ScopedAliases,
    source_file::SourceFile,
    template_ast_matcher::TemplateAstMatcher,
    type_matching_strategy::TypeMatchingStrategy,
};
use closure_jstype::{prelude::*, property::Property};
use closure_parsing::js_doc_info_parser::JsDocInfoParser;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::{
    check_state,
    java_lang::regex::Pattern,
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jscomp_base::LinkedIdentityHashSet,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId, Prop},
    qualified_name::QualifiedName,
    token::Token,
};
use std::sync::{Arc, LazyLock};

// port: ConformanceRules#ALL_TS_ALLOWLIST
static ALL_TS_ALLOWLIST: LazyLock<AllowList> = LazyLock::new(create_ts_allowlist);

// port: ConformanceRules#createTsAllowlist
fn create_ts_allowlist() -> AllowList {
    match AllowList::new::<&str, &str>(&[], &[".*\\.closure\\.js", ".*\\.tsx\\.cl\\.js"]) {
        Ok(allowlist) => allowlist,
        Err(t) => panic!("AssertionError: {t:?}"),
    }
}

/// Classes extending AbstractRule must return ConformanceResult from their checkConformance
/// implementation. For simple rules, the constants CONFORMANCE, POSSIBLE_VIOLATION, VIOLATION are
/// sufficient. However, for some rules additional clarification specific to the violation
/// instance is helpful, for that, an instance of this class can be created to associate a note
/// with the violation.
// port: ConformanceRules.ConformanceResult
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConformanceResult {
    pub level: ConformanceLevel,
    pub note: String,
}

impl ConformanceResult {
    // port: ConformanceRules.ConformanceResult#ConformanceResult(ConformanceLevel)
    pub fn new(level: ConformanceLevel) -> Self {
        Self::with_note(level, "")
    }

    // port: ConformanceRules.ConformanceResult#ConformanceResult(ConformanceLevel,String)
    pub fn with_note(level: ConformanceLevel, note: impl Into<String>) -> Self {
        Self {
            level,
            note: note.into(),
        }
    }

    // For CONFORMANCE rules that don't generate notes:
    // port: ConformanceRules.ConformanceResult#CONFORMANCE
    pub fn conformance() -> Self {
        Self::new(ConformanceLevel::CONFORMANCE)
    }
    // port: ConformanceRules.ConformanceResult#POSSIBLE_VIOLATION
    pub fn possible_violation() -> Self {
        Self::new(ConformanceLevel::POSSIBLE_VIOLATION)
    }
    // port: ConformanceRules.ConformanceResult#POSSIBLE_VIOLATION_DUE_TO_LOOSE_TYPES
    #[allow(dead_code)] // used by the type-restriction rules (still being ported)
    pub(crate) fn possible_violation_due_to_loose_types() -> Self {
        Self::with_note(
            ConformanceLevel::POSSIBLE_VIOLATION,
            "The type information available for this expression is too loose to ensure \
             conformance.",
        )
    }
    // port: ConformanceRules.ConformanceResult#VIOLATION
    pub fn violation() -> Self {
        Self::new(ConformanceLevel::VIOLATION)
    }
}

/// Possible check check results
// port: ConformanceRules.ConformanceLevel
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConformanceLevel {
    /// Nothing interesting detected.
    CONFORMANCE,
    /// In the optionally typed world of the Closure Compiler type system it is possible that
    /// detect patterns that match with looser types that the target pattern.
    POSSIBLE_VIOLATION,
    /// Definitely a violation.
    VIOLATION,
}

// port: ConformanceRules#buildPattern
fn build_pattern<S: AsRef<str>>(
    req_patterns: &[S],
) -> Result<Option<Pattern>, InvalidRequirementSpec> {
    if req_patterns.is_empty() {
        return Ok(None);
    }

    // validate the patterns
    for req_pattern in req_patterns {
        if Pattern::try_compile(req_pattern.as_ref()).is_err() {
            return Err(InvalidRequirementSpec::with_cause("invalid regex pattern"));
        }
    }

    let joint_reg_exp = format!(
        "({})",
        req_patterns
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>()
            .join("|")
    );
    let pattern = match Pattern::try_compile(&joint_reg_exp) {
        Ok(pattern) => pattern,
        Err(e) => panic!("bad joined regexp: {e}"),
    };
    Ok(Some(pattern))
}

// port: ConformanceRules.AllowList
#[derive(Clone, Debug)]
pub(crate) struct AllowList {
    prefixes: Option<Vec<String>>,
    regexp: Option<Pattern>,
    allowlist_entry: Option<RequirementScopeEntry>,
}

impl AllowList {
    // port: ConformanceRules.AllowList#AllowList(List,List)
    fn new<S: AsRef<str>, R: AsRef<str>>(
        prefixes: &[S],
        regexps: &[R],
    ) -> Result<Self, InvalidRequirementSpec> {
        Ok(Self {
            prefixes: Some(prefixes.iter().map(|p| p.as_ref().to_string()).collect()),
            regexp: build_pattern(regexps)?,
            allowlist_entry: None,
        })
    }

    // port: ConformanceRules.AllowList#AllowList(RequirementScopeEntry)
    fn from_entry(allowlist_entry: &RequirementScopeEntry) -> Result<Self, InvalidRequirementSpec> {
        Ok(Self {
            prefixes: Some(allowlist_entry.get_prefix_list().to_vec()),
            regexp: build_pattern(allowlist_entry.get_regexp_list())?,
            allowlist_entry: Some(allowlist_entry.clone()),
        })
    }

    /// Returns true if the given path matches one of the prefixes or regexps, and false otherwise
    // port: ConformanceRules.AllowList#matches
    fn matches(&self, path: &str) -> bool {
        // If the path ends with .closure.js or .tsx.cl.js, it is probably a tsickle-generated
        // file, and there may be entries in the allow list for the TypeScript path
        let ts_path = if let Some(stem) = path.strip_suffix(".closure.js") {
            Some(format!("{stem}.ts"))
        } else if path.ends_with(".tsx.cl.js") {
            // TSX
            Some(path[..path.len() - ".cl.js".len()].to_string())
        } else {
            None
        };
        if let Some(prefixes) = &self.prefixes {
            for prefix in prefixes {
                if !path.is_empty()
                    && (path.starts_with(prefix.as_str())
                        || ts_path
                            .as_ref()
                            .is_some_and(|ts_path| ts_path.starts_with(prefix.as_str())))
                {
                    return true;
                }
            }
        }

        self.regexp.as_ref().is_some_and(|regexp| {
            regexp.matcher(path).find()
                || ts_path
                    .as_ref()
                    .is_some_and(|ts_path| regexp.matcher(ts_path.as_str()).find())
        })
    }
}

/// A conformance rule implementation to support things common to all rules such as
/// allowlisting and reporting (the fields and final methods of Java's
/// `ConformanceRules.AbstractRule`; subclasses embed it and implement [`AbstractRuleImpl`]).
// port: ConformanceRules.AbstractRule
pub struct AbstractRule {
    pub(crate) message: String,
    pub(crate) severity: Severity,
    allowlists: Vec<Arc<AllowList>>,
    only_apply_to: Option<AllowList>,
    #[allow(dead_code)] // read by the type-restriction rules (still being ported)
    pub(crate) report_loose_type_violations: bool,
    #[allow(dead_code)] // read by the type-restriction rules (still being ported)
    pub(crate) type_matching_strategy: TypeMatchingStrategy,
    pub(crate) requirement: Arc<Requirement>,
}

impl AbstractRule {
    // port: ConformanceRules.AbstractRule#AbstractRule
    pub fn new(
        _compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        Self::new_with_ts_allowlisted(_compiler, requirement, false)
    }

    /// The constructor with the result of the overridable `tsIsAllowlisted()` (which Java calls
    /// from the constructor on the subclass instance).
    // port: ConformanceRules.AbstractRule#AbstractRule
    pub fn new_with_ts_allowlisted(
        _compiler: &mut AbstractCompiler,
        requirement: &Requirement,
        ts_is_allowlisted: bool,
    ) -> Result<Self, InvalidRequirementSpec> {
        if !requirement.has_error_message() {
            return Err(InvalidRequirementSpec::new("missing message"));
        }
        let mut message = requirement.get_error_message().to_string();
        if requirement.get_config_file_count() > 0 {
            message.push_str("\n  defined in ");
            message.push_str(&requirement.get_config_file_list().join("\n  extended by "));
        }
        let severity = if requirement.get_severity() == Severity::UNSPECIFIED {
            Severity::WARNING
        } else {
            requirement.get_severity()
        };

        // build allowlists
        let mut allowlists_builder: Vec<Arc<AllowList>> = Vec::new();
        for entry in requirement.get_whitelist_entry_list() {
            allowlists_builder.push(Arc::new(AllowList::from_entry(entry)?));
        }
        for entry in requirement.get_allowlist_entry_list() {
            allowlists_builder.push(Arc::new(AllowList::from_entry(entry)?));
        }

        if ts_is_allowlisted {
            allowlists_builder.push(Arc::new((*ALL_TS_ALLOWLIST).clone()));
        }

        if requirement.get_whitelist_count() > 0 || requirement.get_whitelist_regexp_count() > 0 {
            let allowlist = AllowList::new(
                requirement.get_whitelist_list(),
                requirement.get_whitelist_regexp_list(),
            )?;
            allowlists_builder.push(Arc::new(allowlist));
        }
        if requirement.get_allowlist_count() > 0 || requirement.get_allowlist_regexp_count() > 0 {
            let allowlist = AllowList::new(
                requirement.get_allowlist_list(),
                requirement.get_allowlist_regexp_list(),
            )?;
            allowlists_builder.push(Arc::new(allowlist));
        }

        let only_apply_to = if requirement.get_only_apply_to_count() > 0
            || requirement.get_only_apply_to_regexp_count() > 0
        {
            Some(AllowList::new(
                requirement.get_only_apply_to_list(),
                requirement.get_only_apply_to_regexp_list(),
            )?)
        } else {
            None
        };
        Ok(Self {
            message,
            severity,
            allowlists: allowlists_builder,
            only_apply_to,
            report_loose_type_violations: requirement.get_report_loose_type_violations(),
            type_matching_strategy: Self::get_type_matching_strategy(requirement),
            requirement: Arc::new(requirement.clone()),
        })
    }

    // port: ConformanceRules.AbstractRule#getTypeMatchingStrategy
    fn get_type_matching_strategy(requirement: &Requirement) -> TypeMatchingStrategy {
        use requirement::TypeMatchingStrategy as Proto;
        match requirement.get_type_matching_strategy() {
            Proto::LOOSE => TypeMatchingStrategy::LOOSE,
            Proto::STRICT_NULLABILITY => TypeMatchingStrategy::STRICT_NULLABILITY,
            Proto::SUBTYPES => TypeMatchingStrategy::SUBTYPES,
            Proto::EXACT => TypeMatchingStrategy::EXACT,
            #[allow(unreachable_patterns)] // Java's default branch
            _ => panic!("IllegalStateException: Unknown TypeMatchingStrategy"),
        }
    }

    /// Returns the first AllowList entry that matches the given path, and None otherwise.
    // port: ConformanceRules.AbstractRule#findAllowListForPath
    fn find_allow_list_for_path(
        &self,
        compiler: &AbstractCompiler,
        path: &str,
    ) -> Option<Arc<AllowList>> {
        let mut path = path.to_string();
        if let Some(path_regex) = compiler
            .get_options()
            .get_conformance_remove_regex_from_path()
        {
            path = path_regex.matcher(&path).replace_first("");
        }

        self.allowlists
            .iter()
            .find(|allowlist| allowlist.matches(&path))
            .cloned()
    }

    /// Report a conformance warning for the given node.
    // port: ConformanceRules.AbstractRule#report
    pub fn report(
        &self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        result: &ConformanceResult,
        behavior: LibraryLevelNonAllowlistedConformanceViolationsBehavior,
    ) {
        let msg: &'static DiagnosticType = if self.severity == Severity::ERROR {
            // Always report findings that are errors, even if the types are too loose to be
            // certain.
            &crate::check_conformance::CONFORMANCE_ERROR
        } else if result.level == ConformanceLevel::VIOLATION {
            &crate::check_conformance::CONFORMANCE_VIOLATION
        } else {
            &crate::check_conformance::CONFORMANCE_POSSIBLE_VIOLATION
        };
        let separator = if result.note.is_empty() { "" } else { "\n" };
        let err = JSError::make_with_requirement(
            t,
            self.requirement.clone(),
            n,
            msg,
            &[&self.message, separator, &result.note],
        );

        let path = NodeUtil::get_source_name(t, n);
        let compiler = t.get_compiler();
        let allowlist = path
            .as_deref()
            .and_then(|path| self.find_allow_list_for_path(compiler, path));
        let is_allowlisted = !(allowlist.is_none()
            && self
                .only_apply_to
                .as_ref()
                .is_none_or(|only_apply_to| only_apply_to.matches(path.as_deref().unwrap())));
        let should_report = compiler
            .get_error_manager()
            // returns true even if the violation is allowlisted
            .should_report_conformance_violation(
                &self.requirement,
                allowlist
                    .as_ref()
                    .and_then(|allowlist| allowlist.allowlist_entry.as_ref()),
                &err,
                behavior,
                is_allowlisted,
            );

        // if the violation is not in a gencode or we're not in library level reporting mode,
        // determined by `shouldReport` above, then check the allowlists to decide whether to
        // actually report the violation or not.
        if should_report && !is_allowlisted {
            compiler.report(err);
        }
    }
}

/// The overridable part of Java's `AbstractRule`: `checkConformance` (abstract) and
/// `getPrecondition`. `check` is final in Java and implemented once for every rule.
pub trait AbstractRuleImpl {
    fn base(&self) -> &AbstractRule;

    /// Returns whether the code represented by the Node conforms to the rule.
    // port: ConformanceRules.AbstractRule#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult;

    // port: CheckConformance.Rule#getPrecondition
    fn get_precondition(&self) -> Precondition {
        Precondition::CHECK_ALL
    }
}

impl<T: AbstractRuleImpl> Rule for T {
    fn get_precondition(&self) -> Precondition {
        AbstractRuleImpl::get_precondition(self)
    }

    // port: ConformanceRules.AbstractRule#check
    fn check(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        behavior: LibraryLevelNonAllowlistedConformanceViolationsBehavior,
    ) {
        let result = self.check_conformance(t, n);
        if result.level != ConformanceLevel::CONFORMANCE {
            self.base().report(t, n, &result, behavior);
        }
    }
}

/// No-op rule that never reports any violations.
///
/// This exists so that, if a requirement becomes obsolete but is extended by other requirements
/// that can't all be simultaneously deleted, it can be changed to this rule, allowing it to be
/// effectively removed without breaking downstream builds.
// port: ConformanceRules.NoOp
pub struct NoOp {
    base: AbstractRule,
}

impl NoOp {
    // port: ConformanceRules.NoOp#NoOp
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        Ok(Self {
            base: AbstractRule::new(compiler, requirement)?,
        })
    }
}

impl AbstractRuleImpl for NoOp {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.NoOp#checkConformance
    fn check_conformance(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId) -> ConformanceResult {
        ConformanceResult::conformance()
    }
}

/// Banned dependency rule
// port: ConformanceRules.BannedDependency
pub struct BannedDependency {
    base: AbstractRule,
    paths: Vec<String>,
}

impl BannedDependency {
    // port: ConformanceRules.BannedDependency#BannedDependency
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;
        let paths = requirement.get_value_list().to_vec();
        if paths.is_empty() {
            return Err(InvalidRequirementSpec::new("missing value"));
        }
        Ok(Self { base, paths })
    }
}

impl AbstractRuleImpl for BannedDependency {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.BannedDependency#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if n.is_script(t) {
            let src_file = n.get_source_file_name(t).unwrap();
            for path in &self.paths {
                if src_file.starts_with(path.as_str()) {
                    return ConformanceResult::violation();
                }
            }
        }
        ConformanceResult::conformance()
    }
}

/// Banned dependency via regex rule
// port: ConformanceRules.BannedDependencyRegex
pub struct BannedDependencyRegex {
    base: AbstractRule,
    path_regexp: Option<Pattern>,
}

impl BannedDependencyRegex {
    // port: ConformanceRules.BannedDependencyRegex#BannedDependencyRegex
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;
        let path_regexp_list = requirement.get_value_list();
        if path_regexp_list.is_empty() {
            return Err(InvalidRequirementSpec::new(
                "missing value (no banned dependency regexps)",
            ));
        }
        let path_regexp = build_pattern(path_regexp_list)?;
        Ok(Self { base, path_regexp })
    }
}

impl AbstractRuleImpl for BannedDependencyRegex {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.BannedDependencyRegex#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if n.is_script(t) {
            let src_file = n.get_source_file_name(t).unwrap();
            if self
                .path_regexp
                .as_ref()
                .is_some_and(|p| p.matcher(src_file.as_str()).find())
            {
                return ConformanceResult::violation();
            }
        }
        ConformanceResult::conformance()
    }
}

/// Banned string regex rule
// port: ConformanceRules.BannedStringRegex
pub struct BannedStringRegex {
    base: AbstractRule,
    string_pattern: Pattern,
}

impl BannedStringRegex {
    // port: ConformanceRules.BannedStringRegex#BannedStringRegex
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;

        if requirement.get_value_count() == 0 {
            return Err(InvalidRequirementSpec::new("missing value"));
        }

        let mut builder: Vec<String> = Vec::new();
        for value in requirement.get_value_list() {
            if java_trim(value).is_empty() {
                return Err(InvalidRequirementSpec::new(
                    "empty strings or whitespace are not allowed",
                ));
            }
            builder.push(value.clone());
        }

        let values = builder;
        if values.is_empty() {
            return Err(InvalidRequirementSpec::new("missing value"));
        }
        let string_regex = build_pattern(&values)?.unwrap();
        Ok(Self {
            base,
            string_pattern: string_regex,
        })
    }
}

/// Java `String#trim`: strips code units `<= ' '` from both ends.
fn java_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c <= ' ')
}

impl AbstractRuleImpl for BannedStringRegex {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.BannedStringRegex#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, node: NodeId) -> ConformanceResult {
        if node.is_string_lit(t) {
            if self
                .string_pattern
                .matcher(node.get_string(t).to_string())
                .matches()
            {
                return ConformanceResult::violation();
            }
        } else if node.is_template_lit_string(t) {
            let cooked_string = node.get_cooked_string(t);
            if let Some(cooked_string) = cooked_string {
                if self
                    .string_pattern
                    .matcher(cooked_string.to_string())
                    .matches()
                {
                    return ConformanceResult::violation();
                }
            } else {
                check_state!(
                    is_tagged_template_lit_string(t, node),
                    "Found untagged template literal with invalid (uncookable) escape sequence: %s",
                    node.get_raw_string(t)
                );
                let raw_string = node.get_raw_string(t);
                if self
                    .string_pattern
                    .matcher(raw_string.to_string())
                    .matches()
                {
                    return ConformanceResult::violation();
                }
            }
        }
        ConformanceResult::conformance()
    }
}

/// Is this template literal string a tagged template literal string?
// port: ConformanceRules#isTaggedTemplateLitString
fn is_tagged_template_lit_string(ast: &Ast, node: NodeId) -> bool {
    check_state!(node.is_template_lit_string(ast));
    node.get_grandparent(ast)
        .is_some_and(|g| g.is_tagged_template_lit(ast))
}

/// Checks that file does not include an @enhance annotation for a banned namespace.
// port: ConformanceRules.BannedEnhance
pub struct BannedEnhance {
    base: AbstractRule,
    banned_enhanced_namespaces: Vec<String>,
}

impl BannedEnhance {
    // port: ConformanceRules.BannedEnhance#BannedEnhance
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;
        if requirement.get_value_count() == 0 {
            return Err(InvalidRequirementSpec::new("missing value"));
        }
        // ImmutableSet.copyOf: first occurrence order, no duplicates.
        let mut banned_enhanced_namespaces: Vec<String> = Vec::new();
        for value in requirement.get_value_list() {
            if !banned_enhanced_namespaces.contains(value) {
                banned_enhanced_namespaces.push(value.clone());
            }
        }
        Ok(Self {
            base,
            banned_enhanced_namespaces,
        })
    }
}

impl AbstractRuleImpl for BannedEnhance {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.BannedEnhance#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        let Some(doc_info) = n.get_jsdoc_info(t) else {
            return ConformanceResult::conformance();
        };
        if !doc_info.has_enhance() {
            return ConformanceResult::conformance();
        }

        for banned in &self.banned_enhanced_namespaces {
            if doc_info.get_enhance().is_some_and(|e| e == banned.as_str()) {
                return ConformanceResult::with_note(
                    ConformanceLevel::VIOLATION,
                    format!("The enhanced namespace \"{banned}\""),
                );
            }
        }
        ConformanceResult::conformance()
    }

    // port: ConformanceRules.BannedEnhance#getPrecondition
    fn get_precondition(&self) -> Precondition {
        Precondition::BannedEnhanceIsScript
    }
}

/// Checks that file does not include an @mods annotation for a banned namespace regex.
// port: ConformanceRules.BannedModsRegex
pub struct BannedModsRegex {
    base: AbstractRule,
    banned_mods_regex: Pattern,
}

impl BannedModsRegex {
    // port: ConformanceRules.BannedModsRegex#BannedModsRegex
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;

        let banned_mods_regex_list = requirement.get_value_list();
        if requirement.get_value_count() == 0 {
            return Err(InvalidRequirementSpec::new("missing value"));
        }
        let banned_mods_regex = build_pattern(banned_mods_regex_list)?.unwrap();
        Ok(Self {
            base,
            banned_mods_regex,
        })
    }
}

impl AbstractRuleImpl for BannedModsRegex {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.BannedModsRegex#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        let Some(doc_info) = n.get_jsdoc_info(t) else {
            return ConformanceResult::conformance();
        };
        if !doc_info.has_mods() {
            return ConformanceResult::conformance();
        }

        if self
            .banned_mods_regex
            .matcher(doc_info.get_mods().unwrap_or_default().to_string())
            .find()
        {
            return ConformanceResult::violation();
        }
        ConformanceResult::conformance()
    }

    // port: ConformanceRules.BannedModsRegex#getPrecondition
    fn get_precondition(&self) -> Precondition {
        Precondition::BannedModsRegexIsScript
    }
}

// ---------------------------------------------------------------------------------------------
// ConformanceUtil

// port: ConformanceRules.ConformanceUtil
pub(crate) struct ConformanceUtil;

impl ConformanceUtil {
    // port: ConformanceRules.ConformanceUtil#isCallTarget
    pub(crate) fn is_call_target(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast).unwrap();
        (parent.is_call(ast) || parent.is_new(ast)) && parent.get_first_child(ast) == Some(n)
    }

    // port: ConformanceRules.ConformanceUtil#isLooseType
    pub(crate) fn is_loose_type(registry: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> bool {
        type_.is_unknown_type(registry, ast)
            || type_.is_no_resolved_type(registry)
            || type_.is_all_type(registry)
    }

    // port: ConformanceRules.ConformanceUtil#evaluateTypeString
    pub(crate) fn evaluate_type_string(
        compiler: &mut AbstractCompiler,
        expression: &str,
    ) -> Result<TypeId, InvalidRequirementSpec> {
        let type_nodes = JsDocInfoParser::parse_type_string(compiler, expression);
        let Some(type_nodes) = type_nodes else {
            return Err(InvalidRequirementSpec::new("bad type expression"));
        };
        let type_expr = JSTypeExpression::new(type_nodes, "conformance");
        let (registry, ast) = compiler.get_type_registry_and_ast();
        Ok(registry.evaluate_type_expression_in_global_scope(ast, &type_expr))
    }

    /// Validate the parameters and the 'this' type, of a new or call.
    ///
    /// @see TypeCheck#visitParameterList
    // port: ConformanceRules.ConformanceUtil#validateCall
    pub(crate) fn validate_call(
        compiler: &mut AbstractCompiler,
        call_or_new: NodeId,
        function_type: TypeId,
        is_call_invocation: bool,
    ) -> bool {
        check_state!(call_or_new.is_call(compiler) || call_or_new.is_new(compiler));

        Self::validate_parameter_list(compiler, call_or_new, function_type, is_call_invocation)
            && Self::validate_this(compiler, call_or_new, function_type, is_call_invocation)
    }

    // port: ConformanceRules.ConformanceUtil#validateThis
    fn validate_this(
        compiler: &mut AbstractCompiler,
        call_or_new: NodeId,
        function_type: TypeId,
        is_call_invocation: bool,
    ) -> bool {
        if call_or_new.is_new(compiler) {
            return true;
        }

        let (registry, ast) = compiler.get_type_registry_and_ast();
        let this_type = function_type.get_type_of_this(registry);
        let Some(this_type) = this_type else {
            return true;
        };
        if this_type.is_unknown_type(registry, ast) {
            return true;
        }

        let this_node = if is_call_invocation {
            call_or_new.get_second_child(ast)
        } else {
            call_or_new.get_first_first_child(ast)
        }
        .expect("NullPointerException");
        let this_node_type = this_node
            .get_jstype(ast)
            .expect("NullPointerException")
            .restrict_by_not_null_or_undefined(registry, ast);
        this_node_type.is_subtype_of(registry, ast, this_type)
    }

    // port: ConformanceRules.ConformanceUtil#validateParameterList
    fn validate_parameter_list(
        compiler: &mut AbstractCompiler,
        call_or_new: NodeId,
        function_type: TypeId,
        is_call_invocation: bool,
    ) -> bool {
        let (registry, ast) = compiler.get_type_registry_and_ast();
        let mut arg = call_or_new.get_second_child(ast); // skip the function name
        if is_call_invocation && let Some(a) = arg {
            arg = a.get_next(ast);
        }

        // Get all the annotated types of the argument nodes
        let mut argument_types: Vec<TypeId> = Vec::new();
        while let Some(a) = arg {
            let arg_type = a
                .get_jstype(ast)
                .unwrap_or_else(|| registry.get_native_type(JSTypeNative::UNKNOWN_TYPE));
            argument_types.push(arg_type);
            arg = a.get_next(ast);
        }
        function_type.accepts_arguments(registry, ast, &argument_types)
    }

    /// Java `Splitter.on(".prototype.").splitToList(specName)`.
    fn split_on_prototype(spec_name: &str) -> Vec<&str> {
        spec_name.split(ON_PROTOTYPE).collect()
    }

    /// Extracts the method name from a provided name.
    // port: ConformanceRules.ConformanceUtil#getPropertyFromDeclarationName
    fn get_property_from_declaration_name(spec_name: &str) -> Option<String> {
        let parts = Self::split_on_prototype(spec_name);
        check_state!(parts.len() == 1 || parts.len() == 2);
        if parts.len() == 2 {
            return Some(parts[1].to_string());
        }
        None
    }

    /// Extracts the class name from a provided name.
    // port: ConformanceRules.ConformanceUtil#getClassFromDeclarationName
    fn get_class_from_declaration_name(spec_name: &str) -> Option<String> {
        let tmp = spec_name;
        let parts = Self::split_on_prototype(tmp);
        check_state!(parts.len() == 1 || parts.len() == 2);
        if parts.len() == 2 {
            return Some(parts[0].to_string());
        }
        None
    }

    // port: ConformanceRules.ConformanceUtil#removeTypeDecl
    fn remove_type_decl(spec_name: &str) -> Result<String, InvalidRequirementSpec> {
        let index = java_index_of_char(spec_name, ':');
        if index < 1 {
            return Err(InvalidRequirementSpec::new(
                "value should be in the form NAME:TYPE",
            ));
        }
        Ok(spec_name[..index as usize].to_string())
    }

    // port: ConformanceRules.ConformanceUtil#getTypeFromValue
    fn get_type_from_value(spec_name: &str) -> Option<String> {
        let index = java_index_of_char(spec_name, ':');
        if index < 1 {
            return None;
        }
        Some(spec_name[index as usize + 1..].to_string())
    }

    // port: ConformanceRules.ConformanceUtil#isAnnotatedAsConst
    fn is_annotated_as_const(jsdoc: Option<&JSDocInfo>) -> bool {
        jsdoc.is_some_and(|jsdoc| jsdoc.is_constant())
    }

    // port: ConformanceRules.ConformanceUtil#getValueFromGlobalName
    fn get_value_from_global_name(
        compiler: &mut AbstractCompiler,
        scope: Option<ScopeId>,
        gns: &mut GlobalNamespace,
        qualified_name: &JsString,
    ) -> Option<JsString> {
        let gn = gns.get_own_slot(compiler, qualified_name)?;
        if gn.get_global_sets(gns) != 1
            || gn.get_total_sets(gns) != 1
            || !Self::is_annotated_as_const(gn.get_jsdoc_info(gns).as_deref())
        {
            return None;
        }

        let declaration_node = gn
            .get_declaration(gns)
            .expect("NullPointerException")
            .get_node(gns)
            .expect("NullPointerException");
        let r_value = NodeUtil::get_r_value_of_l_value(compiler, declaration_node);
        Self::infer_string_value(compiler, scope, r_value, &mut FixedGlobalNamespace(gns))
    }

    // port: ConformanceRules.ConformanceUtil#inferStringValue
    pub(crate) fn infer_string_value(
        compiler: &mut AbstractCompiler,
        scope: Option<ScopeId>,
        node: Option<NodeId>,
        global_namespace_supplier: &mut dyn GlobalNamespaceSupplier,
    ) -> Option<JsString> {
        let node = node?;

        match node.get_token(compiler) {
            Token::STRING_KEY
            | Token::STRINGLIT
            | Token::TEMPLATELIT
            | Token::TEMPLATELIT_STRING => {
                return NodeUtil::get_string_value(compiler, node);
            }
            Token::NAME => {
                let scope = scope?;
                let name = node.get_string(compiler);
                let var = scope.get_var(compiler, &name)?;
                if !var.is_const(compiler)
                    && !Self::is_annotated_as_const(var.get_jsdoc_info(compiler).as_deref())
                {
                    return None;
                }
                let initial_value = var.get_initial_value(compiler);
                let var_scope = Some(var.get_scope(compiler));
                return Self::infer_string_value(
                    compiler,
                    var_scope,
                    initial_value,
                    global_namespace_supplier,
                );
            }
            Token::GETPROP => {
                let type_ = node.get_jstype(compiler)?;

                // The JS style guide requires enums to be effectively immutable and all enum items
                // should be statically known. See go/js-style#features-objects-enums.
                let (registry, ast) = compiler.get_type_registry_and_ast();
                if type_.is_enum_element_type(registry) {
                    let enum_type = type_
                        .to_maybe_enum_element_type(registry)
                        .unwrap()
                        .get_enum_type(registry);
                    let enum_source = EnumType::get_source(enum_type, registry);
                    let enum_source = enum_source?;
                    if !enum_source.is_object_lit(ast) && !enum_source.is_class_members(ast) {
                        return None;
                    }
                    let key = NodeUtil::get_first_prop_matching_key(
                        ast,
                        enum_source,
                        &node.get_string(ast),
                    );
                    return Self::infer_string_value(
                        compiler,
                        None,
                        key,
                        global_namespace_supplier,
                    );
                } else if type_.is_string(registry, ast) {
                    let qualified_name = node.get_qualified_name(compiler).unwrap();
                    let gns = global_namespace_supplier.get(compiler)?;
                    return Self::get_value_from_global_name(compiler, scope, gns, &qualified_name);
                }
                return None;
            }
            _ => {
                // Do nothing
            }
        }
        None
    }

    // port: ConformanceRules.ConformanceUtil#isXid
    pub(crate) fn is_xid(registry: &JSTypeRegistry, type_: Option<TypeId>) -> bool {
        let Some(type_) = type_ else {
            return false;
        };
        let enum_el_ty = type_.to_maybe_enum_element_type(registry);
        if let Some(enum_el_ty) = enum_el_ty
            && enum_el_ty
                .get_enum_type(registry)
                .get_reference_name(registry)
                .expect("NullPointerException")
                == "enum{xid.String}"
        {
            return true;
        }
        false
    }

    // port: ConformanceRules.ConformanceUtil#isEventHandlerAttrName
    pub(crate) fn is_event_handler_attr_name(attr: &str) -> bool {
        attr != "on" && attr.starts_with("on")
    }
}

// port: ConformanceRules#ON_PROTOTYPE
const ON_PROTOTYPE: &str = ".prototype.";
// port: ConformanceRules#ON_DOT
const ON_DOT: &str = ".";

/// Java `String#indexOf(char)` (in UTF-16 units; the spec strings searched here are compared and
/// sliced around an ASCII `:` so the byte index is equivalent for the slicing below).
fn java_index_of_char(s: &str, c: char) -> i32 {
    s.find(c).map_or(-1, |i| i as i32)
}

/// Java `Supplier<GlobalNamespace>` as used by `ConformanceUtil#inferStringValue`
/// (`this::getGlobalNamespace` of the attribute rules).
pub trait GlobalNamespaceSupplier {
    /// Java `Supplier#get`; `None` stands for Java's `null`.
    fn get(&mut self, compiler: &mut AbstractCompiler) -> Option<&mut GlobalNamespace>;
}

/// The lambda `() -> gns` of `ConformanceUtil#getValueFromGlobalName`.
struct FixedGlobalNamespace<'a>(&'a mut GlobalNamespace);

impl GlobalNamespaceSupplier for FixedGlobalNamespace<'_> {
    fn get(&mut self, _compiler: &mut AbstractCompiler) -> Option<&mut GlobalNamespace> {
        Some(self.0)
    }
}

// ---------------------------------------------------------------------------------------------
// BannedProperty.RequirementPrecondition

// port: ConformanceRules.BannedProperty.RequirementPrecondition
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RequirementPrecondition {
    BANNED_PROPERTY,
    BANNED_PROPERTY_WRITE,
    BANNED_PROPERTY_NON_CONSTANT_WRITE,
    BANNED_PROPERTY_READ,
    BANNED_PROPERTY_CALL,
}

impl RequirementPrecondition {
    // port: ConformanceRules.BannedProperty.RequirementPrecondition#shouldCheck
    pub fn should_check(self, ast: &Ast, n: NodeId) -> bool {
        match self {
            RequirementPrecondition::BANNED_PROPERTY => matches!(
                n.get_token(ast),
                Token::STRING_KEY | Token::GETPROP | Token::GETELEM | Token::COMPUTED_PROP
            ),
            RequirementPrecondition::BANNED_PROPERTY_WRITE => NodeUtil::is_l_value(ast, n),
            RequirementPrecondition::BANNED_PROPERTY_NON_CONSTANT_WRITE => {
                if !NodeUtil::is_l_value(ast, n) {
                    return false;
                }
                if NodeUtil::is_lhs_of_assign(ast, n) && {
                    let next = n.get_next(ast).unwrap();
                    NodeUtil::is_literal_value(ast, next, /* includeFunctions= */ false)
                        || NodeUtil::is_some_compile_time_const_string_value(ast, next)
                } {
                    return false;
                }
                true
            }
            RequirementPrecondition::BANNED_PROPERTY_READ => {
                !NodeUtil::is_l_value(ast, n) && NodeUtil::is_expression_result_used(ast, n)
            }
            RequirementPrecondition::BANNED_PROPERTY_CALL => {
                ConformanceUtil::is_call_target(ast, n)
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// AbstractTypeRestrictionRule, InferredConstCheck

/// The fields and helpers of Java's `ConformanceRules.AbstractTypeRestrictionRule`; subclasses
/// embed it (it embeds their [`AbstractRule`]).
// port: ConformanceRules.AbstractTypeRestrictionRule
pub struct AbstractTypeRestrictionRule {
    pub(crate) base: AbstractRule,
    native_object_type: TypeId,
    allowlisted_types: Option<TypeId>,
    assertion_functions: AssertionFunctionLookup,
}

impl AbstractTypeRestrictionRule {
    // port: ConformanceRules.AbstractTypeRestrictionRule#AbstractTypeRestrictionRule
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        Self::new_with_ts_allowlisted(compiler, requirement, false)
    }

    /// The constructor with the result of the overridable `tsIsAllowlisted()`.
    // port: ConformanceRules.AbstractTypeRestrictionRule#AbstractTypeRestrictionRule
    pub fn new_with_ts_allowlisted(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
        ts_is_allowlisted: bool,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new_with_ts_allowlisted(compiler, requirement, ts_is_allowlisted)?;
        let native_object_type = compiler
            .get_type_registry()
            .get_native_type(JSTypeNative::OBJECT_TYPE);
        let allowlisted_type_names = requirement.get_value_list();
        let allowlisted_types = Self::union(compiler, allowlisted_type_names);

        let assertion_functions =
            AssertionFunctionLookup::of(compiler.get_coding_convention().get_assertion_functions());
        Ok(Self {
            base,
            native_object_type,
            allowlisted_types,
            assertion_functions,
        })
    }

    // port: ConformanceRules.AbstractTypeRestrictionRule#isAllowlistedType
    pub(crate) fn is_allowlisted_type(&self, t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        if let Some(allowlisted_types) = self.allowlisted_types
            && let Some(n_type) = n.get_jstype(t)
        {
            let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
            let target_type = n_type.restrict_by_not_null_or_undefined(registry, ast);
            if target_type.is_subtype_of(registry, ast, allowlisted_types) {
                return true;
            }
        }
        false
    }

    // port: ConformanceRules.AbstractTypeRestrictionRule#isKnown
    pub(crate) fn is_known(t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        !Self::is_unknown(t, n) && !Self::is_bottom(t, n) && !Self::is_template_type(t, n) // TODO(johnlenz): Remove this restriction
    }

    // port: ConformanceRules.AbstractTypeRestrictionRule#isNativeObjectType
    pub(crate) fn is_native_object_type(&self, t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        let n_type = n.get_jstype(t).expect("NullPointerException");
        let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
        let type_ = n_type.restrict_by_not_null_or_undefined(registry, ast);
        type_.equals(registry, ast, self.native_object_type)
    }

    // port: ConformanceRules.AbstractTypeRestrictionRule#isTop
    pub(crate) fn is_top(t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        let type_ = n.get_jstype(t);
        let registry = t.get_compiler().get_type_registry();
        type_.is_some_and(|type_| type_.is_all_type(registry))
    }

    // port: ConformanceRules.AbstractTypeRestrictionRule#isUnknown
    pub(crate) fn is_unknown(t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        let type_ = n.get_jstype(t);
        match type_ {
            None => true,
            Some(type_) => {
                let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
                type_.is_unknown_type(registry, ast)
            }
        }
    }

    // port: ConformanceRules.AbstractTypeRestrictionRule#isTemplateType
    pub(crate) fn is_template_type(t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        let type_ = n.get_jstype(t).expect("NullPointerException");
        let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
        if type_.is_union_type(registry) {
            let members = type_
                .get_union_members(registry, ast)
                .expect("NullPointerException");
            for member in members.iter() {
                if member.is_template_type(registry) {
                    return true;
                }
            }
        }
        type_.is_template_type(registry)
    }

    // port: ConformanceRules.AbstractTypeRestrictionRule#isBottom
    fn is_bottom(t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        let n_type = n.get_jstype(t).expect("NullPointerException");
        let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
        let type_ = n_type.restrict_by_not_null_or_undefined(registry, ast);
        type_.is_empty_type(registry)
    }

    // port: ConformanceRules.AbstractTypeRestrictionRule#union
    fn union(compiler: &mut AbstractCompiler, type_names: &[String]) -> Option<TypeId> {
        let (registry, ast) = compiler.get_type_registry_and_ast();
        let mut types: Vec<TypeId> = Vec::new();

        for type_name in type_names {
            let mut type_ = registry.get_global_type(ast, type_name.as_str());
            if type_.is_none() {
                // For a few types like `google3.javascript.apps.wiz.events.wiz_event.WizEvent`, we
                // need to resolve via closure namespace. This is because the class type WizEvent
                // is exported from a goog.module, so is not a global type name in the global type
                // registry.
                type_ = registry.resolve_via_closure_namespace(ast, type_name.as_str());
            }
            if let Some(type_) = type_ {
                types.push(type_);
            }
        }
        if types.is_empty() {
            None
        } else {
            Some(registry.create_union_type(ast, &types))
        }
    }

    // port: ConformanceRules.AbstractTypeRestrictionRule#isAssertionCall
    pub(crate) fn is_assertion_call(&self, t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        if n.is_call(t) && n.get_first_child(t).unwrap().is_qualified_name(t) {
            let target = n.get_first_child(t).unwrap();
            let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
            return self
                .assertion_functions
                .lookup_by_callee(ast, registry, target)
                .is_some();
        }
        false
    }

    // port: ConformanceRules.AbstractTypeRestrictionRule#isTypeImmediatelyTightened
    pub(crate) fn is_type_immediately_tightened(
        &self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
    ) -> bool {
        let parent = n.get_parent(t).unwrap();
        self.is_assertion_call(t, parent)
            || parent.is_type_of(t)
            || /* casted node */ n.get_jstype_before_cast(t).is_some()
    }

    // port: ConformanceRules.AbstractTypeRestrictionRule#isUsed
    pub(crate) fn is_used(ast: &Ast, n: NodeId) -> bool {
        let parent = n.get_parent(ast).unwrap();
        if parent.is_name(ast) || NodeUtil::is_lhs_by_destructuring(ast, n) {
            return false;
        }

        // Consider lvalues in assignment operations to be used iff the actual assignment
        // operation's result is used. e.g. for `a.b.c`:
        //     USED: `alert(x = a.b.c);`
        //   UNUSED: `x = a.b.c;`
        if NodeUtil::is_assignment_op(ast, parent) {
            return NodeUtil::is_expression_result_used(ast, parent);
        }

        NodeUtil::is_expression_result_used(ast, n)
    }
}

/// Check that variables annotated as @const have an inferred type, if there is no type given
/// explicitly.
// port: ConformanceRules.InferredConstCheck
pub struct InferredConstCheck {
    base: AbstractRule,
}

impl InferredConstCheck {
    // port: ConformanceRules.InferredConstCheck#InferredConstCheck
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        Ok(Self {
            base: AbstractRule::new(compiler, requirement)?,
        })
    }
}

impl AbstractRuleImpl for InferredConstCheck {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.InferredConstCheck#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        let mut n = n;
        let js_doc = n.get_jsdoc_info(t);
        if let Some(js_doc) = js_doc
            && js_doc.has_const_annotation()
            && js_doc.get_type().is_none()
        {
            if n.is_assign(t) {
                n = n.get_first_child(t).unwrap();
            }
            let type_ = n.get_jstype(t);
            if let Some(type_) = type_ {
                let is_unknown = {
                    let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
                    type_.is_unknown_type(registry, ast)
                };
                if is_unknown && !NodeUtil::is_namespace_decl(t, n) {
                    return ConformanceResult::violation();
                }
            }
        }
        ConformanceResult::conformance()
    }
}

// ---------------------------------------------------------------------------------------------
// BannedName

/// Banned name rule
// port: ConformanceRules.BannedName
pub struct BannedName {
    base: AbstractRule,
    requirement_type: requirement::Type,
    qualified_names: Vec<NodeId>,
    short_names: IndexSet<JsString>,
}

impl BannedName {
    // port: ConformanceRules.BannedName#BannedName
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;
        if requirement.get_value_count() == 0 {
            return Err(InvalidRequirementSpec::new("missing value"));
        }
        let requirement_type = requirement.get_type();

        let mut qualified_builder: Vec<NodeId> = Vec::new();
        let mut short_builder: IndexSet<JsString> = IndexSet::<_>::default();
        for name in requirement.get_value_list() {
            let qualified_name = NodeUtil::new_qname(compiler, name.as_str());
            qualified_builder.push(qualified_name);
            short_builder.insert(qualified_name.get_string(compiler));
        }
        Ok(Self {
            base,
            requirement_type,
            qualified_names: qualified_builder,
            short_names: short_builder,
        })
    }

    // port: ConformanceRules.BannedName#isRootOfQualifiedNameGlobal
    fn is_root_of_qualified_name_global(t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        let root_name = NodeUtil::get_root_of_qualified_name(t, n)
            .get_qualified_name(t)
            .expect("NullPointerException");
        let scope = t.get_scope();
        let compiler = t.get_compiler();
        let v = scope.get_var(compiler, &root_name);
        // TODO(b/189382837): Turn the nullness check back into an assertion once the bug is fixed.
        v.is_some_and(|v| v.is_global(compiler))
    }
}

impl AbstractRuleImpl for BannedName {
    fn base(&self) -> &AbstractRule {
        &self.base
    }

    // port: ConformanceRules.BannedName#getPrecondition
    fn get_precondition(&self) -> Precondition {
        Precondition::BannedNameIsCandidateNode
    }

    // port: ConformanceRules.BannedName#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if self.requirement_type == requirement::Type::BANNED_NAME_CALL
            && !ConformanceUtil::is_call_target(t, n)
        {
            return ConformanceResult::conformance();
        }

        // Efficiently filter out nearly all candidates.
        //
        // Ideally we could use a hashset containing the qualified names, but it turns out that
        // creating wrapper a object for every Node is more expensive than the loop below.
        if !self.short_names.contains(&n.get_string(t)) {
            return ConformanceResult::conformance();
        }

        // Defer expensive infrequent checks.
        //
        // These could be a precondition, but they don't usually need to be checked. Since they're
        // kind of expensive, it's cheaper overall to defer them.
        if NodeUtil::is_in_synthetic_script(t, n) || !Self::is_root_of_qualified_name_global(t, n) {
            return ConformanceResult::conformance();
        }

        for i in (0..self.qualified_names.len()).rev() {
            if n.matches_qualified_name_node(t, self.qualified_names[i]) {
                return ConformanceResult::violation();
            }
        }

        ConformanceResult::conformance()
    }
}

// ---------------------------------------------------------------------------------------------
// BannedProperty

/// Banned property rule
// port: ConformanceRules.BannedProperty
pub struct BannedProperty {
    base: AbstractRule,
    /// Java `ImmutableSetMultimap<String, JSType>`: keys and the values of each key in insertion
    /// order, values deduplicated by `JSType#equals`.
    props: IndexMap<JsString, Vec<TypeId>>,
    requirement_precondition: RequirementPrecondition,
}

impl BannedProperty {
    // port: ConformanceRules.BannedProperty#BannedProperty
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;

        if requirement.get_value_count() == 0 {
            return Err(InvalidRequirementSpec::new("missing value"));
        }

        use requirement::Type;
        let requirement_precondition = match requirement.get_type() {
            Type::BANNED_PROPERTY => RequirementPrecondition::BANNED_PROPERTY,
            Type::BANNED_PROPERTY_READ => RequirementPrecondition::BANNED_PROPERTY_READ,
            Type::BANNED_PROPERTY_WRITE => RequirementPrecondition::BANNED_PROPERTY_WRITE,
            Type::BANNED_PROPERTY_NON_CONSTANT_WRITE => {
                RequirementPrecondition::BANNED_PROPERTY_NON_CONSTANT_WRITE
            }
            Type::BANNED_PROPERTY_CALL => RequirementPrecondition::BANNED_PROPERTY_CALL,
            other => panic!("AssertionError: {other:?}"),
        };

        let (registry, ast) = compiler.get_type_registry_and_ast();

        let mut builder: IndexMap<JsString, Vec<TypeId>> = IndexMap::<_, _>::default();
        for value in requirement.get_value_list() {
            let typename = ConformanceUtil::get_class_from_declaration_name(value);
            let property = ConformanceUtil::get_property_from_declaration_name(value);
            let (Some(typename), Some(property)) = (typename, property) else {
                return Err(InvalidRequirementSpec::new("bad prop value"));
            };

            let mut type_ = registry.get_global_type(ast, typename.as_str());
            if type_.is_none() {
                type_ = registry.resolve_via_closure_namespace(ast, typename.as_str());
            }

            // If type doesn't exist in the copmilation, it can't be a violation. Also, bottom
            // types match everything, which is almost surely not what the check is intended to
            // check against.
            let Some(type_) = type_ else {
                continue;
            };
            if type_.is_unknown_type(registry, ast) || type_.is_empty_type(registry) {
                continue;
            }

            let values = builder
                .entry(JsString::from(property.as_str()))
                .or_default();
            let mut duplicate = false;
            for &existing in values.iter() {
                if existing.equals(registry, ast, type_) {
                    duplicate = true;
                    break;
                }
            }
            if !duplicate {
                values.push(type_);
            }
        }
        Ok(Self {
            base,
            props: builder,
            requirement_precondition,
        })
    }

    // port: ConformanceRules.BannedProperty#matchTypes
    fn match_types(
        &self,
        registry: &mut JSTypeRegistry,
        ast: &Ast,
        found_type: TypeId,
        check_type: TypeId,
    ) -> ConformanceResult {
        let mut found_type = found_type;
        if let Some(found_obj) = found_type.to_maybe_object_type(registry) {
            if found_obj.is_function_prototype_type(registry) {
                let owner_fun = found_obj
                    .get_owner_function(registry)
                    .expect("NullPointerException");
                if owner_fun.is_constructor(registry) {
                    found_type = owner_fun
                        .get_instance_type(registry)
                        .expect("NullPointerException");
                }
            } else if found_obj.is_templatized_type(registry) {
                found_type = found_obj.get_raw_type(registry);
            }
        }

        if found_type.is_unknown_type(registry, ast)
            || found_type.is_template_type(registry)
            || found_type.is_empty_type(registry)
            || found_type.is_all_type(registry)
            || is_loose_object(registry, ast, found_type)
        {
            if self.base.report_loose_type_violations {
                return ConformanceResult::possible_violation_due_to_loose_types();
            }
        } else if found_type.is_subtype_of(registry, ast, check_type) {
            return ConformanceResult::violation();
        } else if check_type.is_subtype_without_structural_typing(registry, ast, found_type) {
            if Self::matches_prototype(registry, ast, check_type, found_type) {
                return ConformanceResult::violation();
            } else if self.base.report_loose_type_violations {
                // Access of a banned property through a super class may be a violation
                return ConformanceResult::possible_violation_due_to_loose_types();
            }
        }

        ConformanceResult::conformance()
    }

    // port: ConformanceRules.BannedProperty#matchesPrototype
    fn matches_prototype(
        registry: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
        maybe_prototype: TypeId,
    ) -> bool {
        let method_class_object_type = type_.to_maybe_object_type(registry);
        if let Some(method_class_object_type) = method_class_object_type {
            let implicit_prototype = method_class_object_type
                .get_implicit_prototype(registry, ast)
                .expect("NullPointerException");
            if implicit_prototype.equals(registry, ast, maybe_prototype) {
                return true;
            }
        }
        false
    }

    // port: ConformanceRules.BannedProperty#extractType
    fn extract_type(ast: &Ast, n: NodeId) -> Option<TypeId> {
        match n.get_token(ast) {
            Token::GETELEM | Token::GETPROP => n.get_first_child(ast).unwrap().get_jstype(ast),
            Token::STRING_KEY | Token::COMPUTED_PROP => {
                let parent = n.get_parent(ast).unwrap();
                match parent.get_token(ast) {
                    Token::OBJECT_PATTERN | Token::OBJECTLIT => parent.get_jstype(ast),
                    Token::CLASS_MEMBERS => None,
                    _ => panic!("AssertionError"),
                }
            }
            _ => None,
        }
    }

    // port: ConformanceRules.BannedProperty#extractName
    fn extract_name(ast: &Ast, n: NodeId) -> Option<JsString> {
        match n.get_token(ast) {
            Token::GETPROP | Token::STRING_KEY => Some(n.get_string(ast)),
            Token::GETELEM => {
                let string = n.get_second_child(ast).unwrap();
                if string.is_string_lit(ast) {
                    Some(string.get_string(ast))
                } else {
                    None
                }
            }
            Token::COMPUTED_PROP => {
                let string = n.get_first_child(ast).unwrap();
                if string.is_string_lit(ast) {
                    Some(string.get_string(ast))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl AbstractRuleImpl for BannedProperty {
    fn base(&self) -> &AbstractRule {
        &self.base
    }

    // port: ConformanceRules.BannedProperty#getPrecondition
    fn get_precondition(&self) -> Precondition {
        Precondition::BannedProperty(self.requirement_precondition)
    }

    // port: ConformanceRules.BannedProperty#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        // ImmutableSetMultimap#get(null) returns the empty set.
        let check_types: &[TypeId] = match Self::extract_name(t, n) {
            Some(name) => self.props.get(&name).map_or(&[], Vec::as_slice),
            None => &[],
        };
        if check_types.is_empty() {
            return ConformanceResult::conformance();
        }

        // Avoid type operations when possible.
        //
        // checkTypes is almost always empty, and operations on unions can be expensive.
        let Some(found_type) = Self::extract_type(t, n) else {
            return ConformanceResult::conformance();
        };
        let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
        let found_type = found_type.restrict_by_not_null_or_undefined(registry, ast);

        for &check_type in check_types {
            let result = self.match_types(registry, ast, found_type, check_type);
            if result.level != ConformanceLevel::CONFORMANCE {
                return result;
            }
        }

        ConformanceResult::conformance()
    }
}

// port: ConformanceRules#isLooseObject
fn is_loose_object(registry: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) -> bool {
    let object_type = registry.get_native_type(JSTypeNative::OBJECT_TYPE);
    type_.equals(registry, ast, object_type)
}

// ---------------------------------------------------------------------------------------------
// RestrictedNameCall, RestrictedMethodCall, RestrictedPropertyWrite

// port: ConformanceRules.RestrictedNameCall.Restriction
struct RestrictedNameCallRestriction {
    name: NodeId,
    restricted_call_type: TypeId,
}

/// Restricted name call rule
// port: ConformanceRules.RestrictedNameCall
pub struct RestrictedNameCall {
    base: AbstractRule,
    restrictions: Vec<RestrictedNameCallRestriction>,
}

impl RestrictedNameCall {
    // port: ConformanceRules.RestrictedNameCall#RestrictedNameCall
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;
        if requirement.get_value_count() == 0 {
            return Err(InvalidRequirementSpec::new("missing value"));
        }

        let mut builder: Vec<RestrictedNameCallRestriction> = Vec::new();
        for value in requirement.get_value_list() {
            // NodeUtil.newQName(compiler, null) throws NullPointerException in Java.
            let name = NodeUtil::new_qname(
                compiler,
                Self::get_name_from_value(value).expect("NullPointerException"),
            );
            let restricted_decl = ConformanceUtil::get_type_from_value(value);
            let Some(restricted_decl) = restricted_decl else {
                return Err(InvalidRequirementSpec::new("bad prop value"));
            };

            let restricted_type =
                ConformanceUtil::evaluate_type_string(compiler, &restricted_decl)?;
            let restricted_call_type =
                restricted_type.to_maybe_function_type(compiler.get_type_registry());
            let Some(restricted_call_type) = restricted_call_type else {
                return Err(InvalidRequirementSpec::new("invalid conformance type"));
            };
            builder.push(RestrictedNameCallRestriction {
                name,
                restricted_call_type,
            });
        }
        Ok(Self {
            base,
            restrictions: builder,
        })
    }

    // port: ConformanceRules.RestrictedNameCall#getNameFromValue
    fn get_name_from_value(spec_name: &str) -> Option<String> {
        let index = java_index_of_char(spec_name, ':');
        if index < 1 {
            return None;
        }
        Some(spec_name[..index as usize].to_string())
    }
}

impl AbstractRuleImpl for RestrictedNameCall {
    fn base(&self) -> &AbstractRule {
        &self.base
    }

    // port: ConformanceRules.RestrictedNameCall#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if ConformanceUtil::is_call_target(t, n) && n.is_qualified_name(t) {
            // TODO(johnlenz): restrict to global names
            for i in 0..self.restrictions.len() {
                let r = &self.restrictions[i];

                if n.matches_qualified_name_node(t, r.name) {
                    let parent = n.get_parent(t).unwrap();
                    if !ConformanceUtil::validate_call(
                        t.get_compiler(),
                        parent,
                        r.restricted_call_type,
                        false,
                    ) {
                        return ConformanceResult::violation();
                    }
                } else if n.is_get_prop(t)
                    && n.get_string_ref(t) == "call"
                    && n.get_first_child(t)
                        .unwrap()
                        .matches_qualified_name_node(t, r.name)
                {
                    let parent = n.get_parent(t).unwrap();
                    if !ConformanceUtil::validate_call(
                        t.get_compiler(),
                        parent,
                        r.restricted_call_type,
                        true,
                    ) {
                        return ConformanceResult::violation();
                    }
                }
            }
        }
        ConformanceResult::conformance()
    }
}

// port: ConformanceRules.RestrictedMethodCall.Restriction
struct RestrictedMethodCallRestriction {
    type_: Option<TypeId>,
    property: String,
    restricted_call_type: TypeId,
}

/// Banned property call rule
// port: ConformanceRules.RestrictedMethodCall
pub struct RestrictedMethodCall {
    base: AbstractRule,
    restrictions: Vec<RestrictedMethodCallRestriction>,
}

impl RestrictedMethodCall {
    // port: ConformanceRules.RestrictedMethodCall#RestrictedMethodCall
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;

        if requirement.get_value_count() == 0 {
            return Err(InvalidRequirementSpec::new("missing value"));
        }

        let mut builder: Vec<RestrictedMethodCallRestriction> = Vec::new();
        for value in requirement.get_value_list() {
            let type_ = ConformanceUtil::get_class_from_declaration_name(
                &ConformanceUtil::remove_type_decl(value)?,
            );
            let property = ConformanceUtil::get_property_from_declaration_name(
                &ConformanceUtil::remove_type_decl(value)?,
            );
            let restricted_decl = ConformanceUtil::get_type_from_value(value);
            let (Some(type_), Some(property), Some(restricted_decl)) =
                (type_, property, restricted_decl)
            else {
                return Err(InvalidRequirementSpec::new("bad prop value"));
            };

            let restricted_type =
                ConformanceUtil::evaluate_type_string(compiler, &restricted_decl)?;
            let restricted_call_type =
                restricted_type.to_maybe_function_type(compiler.get_type_registry());
            let Some(restricted_call_type) = restricted_call_type else {
                return Err(InvalidRequirementSpec::new("invalid conformance type"));
            };
            let (registry, ast) = compiler.get_type_registry_and_ast();
            builder.push(RestrictedMethodCallRestriction {
                type_: registry.get_global_type(ast, type_.as_str()),
                property,
                restricted_call_type,
            });
        }

        Ok(Self {
            base,
            restrictions: builder,
        })
    }

    // port: ConformanceRules.RestrictedMethodCall#checkConformance(NodeTraversal,Node,Restriction,boolean)
    fn check_conformance_restriction(
        &self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        r: &RestrictedMethodCallRestriction,
        is_call_invocation: bool,
    ) -> ConformanceResult {
        let method_class_type = r.type_;
        let lhs = if is_call_invocation {
            n.get_first_first_child(t)
        } else {
            n.get_first_child(t)
        }
        .unwrap();
        if let Some(method_class_type) = method_class_type
            && let Some(lhs_type) = lhs.get_jstype(t)
        {
            let parent = n.get_parent(t).unwrap();
            let compiler = t.get_compiler();
            let (registry, ast) = compiler.get_type_registry_and_ast();
            let target_type = lhs_type.restrict_by_not_null_or_undefined(registry, ast);
            if ConformanceUtil::is_loose_type(registry, ast, target_type)
                || is_loose_object(registry, ast, target_type)
            {
                if self.base.report_loose_type_violations
                    && !ConformanceUtil::validate_call(
                        compiler,
                        parent,
                        r.restricted_call_type,
                        is_call_invocation,
                    )
                {
                    return ConformanceResult::possible_violation_due_to_loose_types();
                }
            } else if target_type.is_subtype_of(registry, ast, method_class_type)
                && !ConformanceUtil::validate_call(
                    compiler,
                    parent,
                    r.restricted_call_type,
                    is_call_invocation,
                )
            {
                return ConformanceResult::violation();
            }
        }
        ConformanceResult::conformance()
    }

    // port: ConformanceRules.RestrictedMethodCall#matchesProp
    fn matches_prop(ast: &Ast, n: NodeId, r: &RestrictedMethodCallRestriction) -> bool {
        n.is_get_prop(ast) && n.get_string(ast) == r.property.as_str()
    }
}

impl AbstractRuleImpl for RestrictedMethodCall {
    fn base(&self) -> &AbstractRule {
        &self.base
    }

    // port: ConformanceRules.RestrictedMethodCall#checkConformance(NodeTraversal,Node)
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if !n.is_get_prop(t) || !ConformanceUtil::is_call_target(t, n) {
            return ConformanceResult::conformance();
        }

        for i in 0..self.restrictions.len() {
            let r = &self.restrictions[i];
            let mut result = ConformanceResult::conformance();

            if Self::matches_prop(t, n, r) {
                result = self.check_conformance_restriction(t, n, r, false);
            } else if n.get_string_ref(t) == "call"
                && Self::matches_prop(t, n.get_first_child(t).unwrap(), r)
            {
                // handle .call invocation
                result = self.check_conformance_restriction(t, n, r, true);
            }

            // TODO(johnlenz): should "apply" always be a possible violation?
            if result.level != ConformanceLevel::CONFORMANCE {
                return result;
            }
        }

        ConformanceResult::conformance()
    }
}

// port: ConformanceRules.RestrictedPropertyWrite.Restriction
struct RestrictedPropertyWriteRestriction {
    type_: Option<TypeId>,
    property: String,
    restricted_type: TypeId,
}

/// Restricted property write.
// port: ConformanceRules.RestrictedPropertyWrite
pub struct RestrictedPropertyWrite {
    base: AbstractRule,
    restrictions: Vec<RestrictedPropertyWriteRestriction>,
}

impl RestrictedPropertyWrite {
    // port: ConformanceRules.RestrictedPropertyWrite#RestrictedPropertyWrite
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;

        if requirement.get_value_count() == 0 {
            return Err(InvalidRequirementSpec::new("missing value"));
        }

        let mut builder: Vec<RestrictedPropertyWriteRestriction> = Vec::new();
        for value in requirement.get_value_list() {
            let type_ = ConformanceUtil::get_class_from_declaration_name(
                &ConformanceUtil::remove_type_decl(value)?,
            );
            let property = ConformanceUtil::get_property_from_declaration_name(
                &ConformanceUtil::remove_type_decl(value)?,
            );
            let restricted_decl = ConformanceUtil::get_type_from_value(value);
            let (Some(type_), Some(property), Some(restricted_decl)) =
                (type_, property, restricted_decl)
            else {
                return Err(InvalidRequirementSpec::new("bad prop value"));
            };
            let restricted_type =
                ConformanceUtil::evaluate_type_string(compiler, &restricted_decl)?;
            let (registry, ast) = compiler.get_type_registry_and_ast();
            builder.push(RestrictedPropertyWriteRestriction {
                type_: registry.get_global_type(ast, type_.as_str()),
                property,
                restricted_type,
            });
        }

        Ok(Self {
            base,
            restrictions: builder,
        })
    }
}

impl AbstractRuleImpl for RestrictedPropertyWrite {
    fn base(&self) -> &AbstractRule {
        &self.base
    }

    // port: ConformanceRules.RestrictedPropertyWrite#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if n.is_get_prop(t) && NodeUtil::is_lhs_of_assign(t, n) {
            let rhs_type = n.get_next(t).unwrap().get_jstype(t);
            let target_type = n.get_first_child(t).unwrap().get_jstype(t);
            let name = n.get_string(t);
            if let (Some(rhs_type), Some(target_type)) = (rhs_type, target_type) {
                let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
                let mut target_not_null_type: Option<TypeId> = None;
                for r in &self.restrictions {
                    if name == r.property.as_str()
                        && !rhs_type.is_subtype_of(registry, ast, r.restricted_type)
                    {
                        if ConformanceUtil::is_loose_type(registry, ast, target_type) {
                            if self.base.report_loose_type_violations {
                                return ConformanceResult::possible_violation_due_to_loose_types();
                            }
                        } else {
                            let target_not_null = match target_not_null_type {
                                Some(target_not_null) => target_not_null,
                                None => {
                                    let restricted = target_type
                                        .restrict_by_not_null_or_undefined(registry, ast);
                                    target_not_null_type = Some(restricted);
                                    restricted
                                }
                            };
                            // Java: `isSubtypeOf(null)` throws NullPointerException.
                            let r_type = r.type_.expect("NullPointerException");
                            if target_not_null.is_subtype_of(registry, ast, r_type) {
                                return ConformanceResult::violation();
                            }
                        }
                    }
                }
            }
        }
        ConformanceResult::conformance()
    }
}

// ---------------------------------------------------------------------------------------------
// BanForOf ... BanGlobalVars

/// Banned for/of loops
// port: ConformanceRules.BanForOf
pub struct BanForOf {
    base: AbstractRule,
}

impl BanForOf {
    // port: ConformanceRules.BanForOf#BanForOf
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        Ok(Self {
            base: AbstractRule::new(compiler, requirement)?,
        })
    }
}

impl AbstractRuleImpl for BanForOf {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.BanForOf#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if n.is_for_of(t) || n.is_for_await_of(t) {
            return ConformanceResult::violation();
        }
        ConformanceResult::conformance()
    }
}

/// Require "use strict" rule
// port: ConformanceRules.RequireUseStrict
pub struct RequireUseStrict {
    base: AbstractRule,
}

impl RequireUseStrict {
    // port: ConformanceRules.RequireUseStrict#RequireUseStrict
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;
        if !requirement.get_value_list().is_empty() {
            return Err(InvalidRequirementSpec::new("invalid value"));
        }
        Ok(Self { base })
    }
}

impl AbstractRuleImpl for RequireUseStrict {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.RequireUseStrict#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if n.is_script(t) && !n.is_use_strict(t) {
            ConformanceResult::violation()
        } else {
            ConformanceResult::conformance()
        }
    }
}

/// Banned throw of non-error object types.
// port: ConformanceRules.BanThrowOfNonErrorTypes
pub struct BanThrowOfNonErrorTypes {
    base: AbstractRule,
    error_obj_type: Option<TypeId>,
}

impl BanThrowOfNonErrorTypes {
    // port: ConformanceRules.BanThrowOfNonErrorTypes#BanThrowOfNonErrorTypes
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;
        let (registry, ast) = compiler.get_type_registry_and_ast();
        let error_obj_type = registry.get_global_type(ast, "Error");
        Ok(Self {
            base,
            error_obj_type,
        })
    }
}

impl AbstractRuleImpl for BanThrowOfNonErrorTypes {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.BanThrowOfNonErrorTypes#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if let Some(error_obj_type) = self.error_obj_type
            && n.is_throw(t)
        {
            let thrown = n.get_first_child(t).unwrap().get_jstype(t);
            if let Some(thrown) = thrown {
                let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
                // Allow vague types, as is typical of re-throws of exceptions
                if !thrown.is_unknown_type(registry, ast)
                    && !thrown.is_all_type(registry)
                    && !thrown.is_empty_type(registry)
                    && !thrown.is_subtype_of(registry, ast, error_obj_type)
                {
                    return ConformanceResult::violation();
                }
            }
        }
        ConformanceResult::conformance()
    }
}

/// Banned dereferencing null or undefined types.
// port: ConformanceRules.BanNullDeref
pub struct BanNullDeref {
    base: AbstractTypeRestrictionRule,
}

impl BanNullDeref {
    // port: ConformanceRules.BanNullDeref#BanNullDeref
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        Ok(Self {
            base: AbstractTypeRestrictionRule::new(compiler, requirement)?,
        })
    }

    // port: ConformanceRules.BanNullDeref#report
    fn report(&self, t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        n.get_jstype(t).is_some()
            && AbstractTypeRestrictionRule::is_known(t, n)
            && Self::invalid_deref(t, n)
            && !self.base.is_allowlisted_type(t, n)
    }

    /// Whether the type is known to be invalid to dereference.
    // port: ConformanceRules.BanNullDeref#invalidDeref
    fn invalid_deref(t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        let type_ = n.get_jstype(t).expect("NullPointerException");
        let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
        // TODO(johnlenz): top type should not be allowed here
        !type_.is_all_type(registry)
            && (type_.is_nullable(registry, ast) || type_.is_voidable(registry, ast))
    }
}

impl AbstractRuleImpl for BanNullDeref {
    fn base(&self) -> &AbstractRule {
        &self.base.base
    }
    // port: ConformanceRules.BanNullDeref#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        let violation = match n.get_token(t) {
            Token::GETPROP | Token::GETELEM | Token::NEW | Token::CALL => {
                let first = n.get_first_child(t).unwrap();
                self.report(t, first)
            }
            Token::IN => {
                let last = n.get_last_child(t).unwrap();
                self.report(t, last)
            }
            _ => false,
        };

        if violation {
            ConformanceResult::violation()
        } else {
            ConformanceResult::conformance()
        }
    }
}

/// Banned unknown "this" types.
// port: ConformanceRules.BanUnknownThis
pub struct BanUnknownThis {
    base: AbstractTypeRestrictionRule,
    reports: LinkedIdentityHashSet<NodeId>,
}

impl BanUnknownThis {
    // port: ConformanceRules.BanUnknownThis#BanUnknownThis
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        Ok(Self {
            base: AbstractTypeRestrictionRule::new_with_ts_allowlisted(
                compiler,
                requirement,
                Self::ts_is_allowlisted(),
            )?,
            reports: LinkedIdentityHashSet::default(),
        })
    }

    // port: ConformanceRules.BanUnknownThis#tsIsAllowlisted
    fn ts_is_allowlisted() -> bool {
        // We expect TypeScript to already check it.
        // LINT.IfChange
        true
        // LINT.ThenChange(//depot/google3/java/com/google/javascript/modules/librarydepsconformancechecker/conformance_checker.go)
    }
}

impl AbstractRuleImpl for BanUnknownThis {
    fn base(&self) -> &AbstractRule {
        &self.base.base
    }
    // port: ConformanceRules.BanUnknownThis#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if n.is_this(t)
            && let Some(type_) = n.get_jstype(t)
        {
            let is_unknown = {
                let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
                type_.is_unknown_type(registry, ast)
            };
            if is_unknown && !self.base.is_type_immediately_tightened(t, n) {
                let root = t.get_scope_root().expect("NullPointerException");
                if !self.reports.contains(&root) {
                    self.reports.add(root);
                    return ConformanceResult::violation();
                }
            }
        }
        ConformanceResult::conformance()
    }
}

/// Banned unknown type references of the form "this.prop" unless
/// - it is immediately cast,
/// - it is a @template type (until template type restricts are enabled) or
/// - the value is unused.
/// - the "this" type is unknown (as this is expected to be used with BanUnknownThis which would
///   have already reported the root cause).
// port: ConformanceRules.BanUnknownDirectThisPropsReferences
pub struct BanUnknownDirectThisPropsReferences {
    base: AbstractTypeRestrictionRule,
}

impl BanUnknownDirectThisPropsReferences {
    // port: ConformanceRules.BanUnknownDirectThisPropsReferences#BanUnknownDirectThisPropsReferences
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        Ok(Self {
            base: AbstractTypeRestrictionRule::new(compiler, requirement)?,
        })
    }

    // port: ConformanceRules.BanUnknownDirectThisPropsReferences#isKnownThis
    fn is_known_this(t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        n.is_this(t) && !AbstractTypeRestrictionRule::is_unknown(t, n)
    }

    // port: ConformanceRules.BanUnknownDirectThisPropsReferences#isExplicitlyUnknown
    fn is_explicitly_unknown(t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        let first_type = n.get_first_child(t).unwrap().get_jstype(t);
        let name = n.get_string(t);
        let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
        let owner = closure_jstype::object_type::cast(registry, first_type);
        let prop = owner.and_then(|owner| owner.get_slot(registry, ast, &name));
        prop.is_some_and(|prop| !prop.is_type_inferred(registry))
    }
}

impl AbstractRuleImpl for BanUnknownDirectThisPropsReferences {
    fn base(&self) -> &AbstractRule {
        &self.base.base
    }
    // port: ConformanceRules.BanUnknownDirectThisPropsReferences#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if n.is_get_prop(t)
            && Self::is_known_this(t, n.get_first_child(t).unwrap()) // not a cascading unknown
            && AbstractTypeRestrictionRule::is_unknown(t, n)
            && !AbstractTypeRestrictionRule::is_template_type(t, n)
            && AbstractTypeRestrictionRule::is_used(t, n) // skip most assignments, etc
            && !self.base.is_type_immediately_tightened(t, n)
            && !Self::is_explicitly_unknown(t, n)
        {
            return ConformanceResult::violation();
        }
        ConformanceResult::conformance()
    }
}

/// Banned non-literal arguments passed to `goog.string.Const.from`. Possible calls to
/// goog.string.Const in user code could look like:
///
/// ```text
/// `goog.string.Const.from('foo');`
/// `const alias = goog.string.Const.from; alias(foo);`
/// `const {from} = goog.require('goog.string.Const'); from(foo);`
/// `const alias = goog.require('goog.string.Const'); alias.from(foo);`
/// ```
// port: ConformanceRules.BanNonLiteralArgsToGoogStringConstFrom
pub struct BanNonLiteralArgsToGoogStringConstFrom {
    base: AbstractRule,
}

// port: ConformanceRules.BanNonLiteralArgsToGoogStringConstFrom#GOOG_STRING_CONST_FROM
static GOOG_STRING_CONST_FROM: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.string.Const.from"));

impl BanNonLiteralArgsToGoogStringConstFrom {
    // port: ConformanceRules.BanNonLiteralArgsToGoogStringConstFrom#BanNonLiteralArgsToGoogStringConstFrom
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        Ok(Self {
            base: AbstractRule::new(compiler, requirement)?,
        })
    }

    // port: ConformanceRules.BanNonLiteralArgsToGoogStringConstFrom#isAllowed
    fn is_allowed(ast: &Ast, argument: NodeId) -> bool {
        argument.is_string_lit(ast)
            || (argument.is_template_lit(ast) && argument.has_one_child(ast))
    }
}

impl AbstractRuleImpl for BanNonLiteralArgsToGoogStringConstFrom {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.BanNonLiteralArgsToGoogStringConstFrom#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, node: NodeId) -> ConformanceResult {
        if node.is_call(t) {
            let mut name = node.get_first_child(t).unwrap();
            let Some(argument) = name.get_next(t) else {
                return ConformanceResult::conformance();
            };

            // If the name is a qualified name, it must be goog.string.Const.from or an alias of it.
            if name.is_name(t) {
                let scope = t.get_scope();
                let var_name = name.get_string(t);
                let compiler = t.get_compiler();
                let Some(var) = scope.get_var(compiler, &var_name) else {
                    return ConformanceResult::conformance();
                };
                let Some(initial_value) = var.get_initial_value(compiler) else {
                    return ConformanceResult::conformance();
                };
                name = initial_value;
            }

            if GOOG_STRING_CONST_FROM.matches(t, name) && !Self::is_allowed(t, argument) {
                return ConformanceResult::violation();
            }
        }
        ConformanceResult::conformance()
    }
}

/// Banned unknown type references of the form "instance.prop" unless
/// - (a) it is immediately cast/asserted, or
/// - (b) it is a @template type (until template type restrictions are enabled), or
/// - (c) the value is unused, or
/// - (d) the source object type is unknown (to avoid error cascades)
// port: ConformanceRules.BanUnknownTypedClassPropsReferences
pub struct BanUnknownTypedClassPropsReferences {
    base: AbstractTypeRestrictionRule,
}

impl BanUnknownTypedClassPropsReferences {
    // port: ConformanceRules.BanUnknownTypedClassPropsReferences#BanUnknownTypedClassPropsReferences
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        Ok(Self {
            base: AbstractTypeRestrictionRule::new_with_ts_allowlisted(
                compiler,
                requirement,
                Self::ts_is_allowlisted(),
            )?,
        })
    }

    // port: ConformanceRules.BanUnknownTypedClassPropsReferences#tsIsAllowlisted
    fn ts_is_allowlisted() -> bool {
        // LINT.IfChange
        true
        // LINT.ThenChange(//depot/google3/java/com/google/javascript/modules/librarydepsconformancechecker/conformance_checker.go)
    }

    // port: ConformanceRules.BanUnknownTypedClassPropsReferences#isCheckablePropertySource
    fn is_checkable_property_source(&self, t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        AbstractTypeRestrictionRule::is_known(t, n)
            && !AbstractTypeRestrictionRule::is_top(t, n)
            && Self::is_class_type(t, n)
            && !self.base.is_native_object_type(t, n)
            && !self.base.is_allowlisted_type(t, n)
    }

    // port: ConformanceRules.BanUnknownTypedClassPropsReferences#isClassType
    fn is_class_type(t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        let n_type = n.get_jstype(t).expect("NullPointerException");
        let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
        let type_ = n_type
            .restrict_by_not_null_or_undefined(registry, ast)
            .to_maybe_object_type(registry);
        let Some(type_) = type_ else {
            return false;
        };
        if !type_.is_instance_type(registry) {
            return false;
        }

        let Some(ctor) = type_.get_constructor(registry) else {
            return false;
        };

        let info = JSType::get_jsdoc_info(ctor, registry);
        let source = FunctionType::get_source(ctor, registry);

        info.is_some_and(|info| info.is_constructor_or_interface())
            || source.is_some_and(|source| source.is_class(ast))
    }

    // port: ConformanceRules.BanUnknownTypedClassPropsReferences#isDeclaredUnknown
    fn is_declared_unknown(t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        let target = n.get_first_child(t).unwrap();
        let target_jstype = target.get_jstype(t).expect("NullPointerException");
        let name = n.get_string(t);
        let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
        let target_type = target_jstype
            .restrict_by_not_null_or_undefined(registry, ast)
            .to_maybe_object_type(registry);
        let Some(target_type) = target_type else {
            return false;
        };

        let info = target_type.get_property_jsdoc_info(registry, ast, &name);
        let Some(info) = info else {
            return false;
        };
        if !info.has_type() {
            return false;
        }

        let expr = info.get_type().unwrap();
        let type_expr_node = expr.get_root();
        if type_expr_node.get_token(ast) == Token::QMARK && !type_expr_node.has_children(ast) {
            return true;
        } else if type_expr_node.get_token(ast) == Token::PIPE {
            // Might be a union type including ? that's collapsed during checking.
            let mut child = type_expr_node.get_first_child(ast);
            while let Some(c) = child {
                if c.get_token(ast) == Token::QMARK {
                    return true;
                }
                child = c.get_next(ast);
            }
        }

        false
    }
}

impl AbstractRuleImpl for BanUnknownTypedClassPropsReferences {
    fn base(&self) -> &AbstractRule {
        &self.base.base
    }
    // port: ConformanceRules.BanUnknownTypedClassPropsReferences#checkConformance
    fn check_conformance(
        &mut self,
        t: &mut NodeTraversal<'_>,
        getprop: NodeId,
    ) -> ConformanceResult {
        if getprop.is_get_prop(t)
            && AbstractTypeRestrictionRule::is_unknown(t, getprop)
            && AbstractTypeRestrictionRule::is_used(t, getprop) // skip most assignments, etc
            && !self.base.is_type_immediately_tightened(t, getprop)
            && self.is_checkable_property_source(t, getprop.get_first_child(t).unwrap()) // not a cascading unknown
            && !AbstractTypeRestrictionRule::is_template_type(t, getprop)
            && !Self::is_declared_unknown(t, getprop)
        {
            let prop_name = getprop.get_string(t);
            let first_type = getprop
                .get_first_child(t)
                .unwrap()
                .get_jstype(t)
                .expect("NullPointerException");
            let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
            let type_name = first_type.to_string(registry, ast);
            return ConformanceResult::with_note(
                ConformanceLevel::VIOLATION,
                format!("The property \"{prop_name}\" on type \"{type_name}\""),
            );
        }
        ConformanceResult::conformance()
    }
}

/// Banned accessing properties from objects that are unresolved forward-declared type names. For
/// legacy reasons this is allowed but causes unexpected weaknesses in the type inference.
// port: ConformanceRules.BanUnresolvedType
pub struct BanUnresolvedType {
    base: AbstractTypeRestrictionRule,
}

impl BanUnresolvedType {
    // port: ConformanceRules.BanUnresolvedType#BanUnresolvedType
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        Ok(Self {
            base: AbstractTypeRestrictionRule::new(compiler, requirement)?,
        })
    }

    // port: ConformanceRules.BanUnresolvedType#getNonConformingPart
    fn get_non_conforming_part(
        registry: &mut JSTypeRegistry,
        ast: &Ast,
        type_: Option<TypeId>,
    ) -> Option<String> {
        let type_ = type_?;
        if type_.is_union_type(registry) {
            // unwrap union types which might contain unresolved type name
            // references for example {Foo|undefined}
            let mut non_conforming_parts: Option<Vec<String>> = None;
            let members = type_
                .get_union_members(registry, ast)
                .expect("NullPointerException");
            for &part in members.iter() {
                let non_conforming_part = Self::get_non_conforming_part(registry, ast, Some(part));
                if let Some(non_conforming_part) = non_conforming_part {
                    non_conforming_parts
                        .get_or_insert_with(Vec::new)
                        .push(non_conforming_part);
                }
            }
            if let Some(non_conforming_parts) = non_conforming_parts {
                return Some(non_conforming_parts.join("|"));
            }
        } else if type_.is_no_resolved_type(registry) {
            let no_resolved_type = type_
                .to_object_type(registry)
                .expect("NullPointerException");
            // Java string concatenation prints a null reference name as "null".
            return Some(
                no_resolved_type
                    .get_reference_name(registry)
                    .map_or_else(|| "null".to_string(), |name| name.to_string()),
            );
        }
        None
    }
}

impl AbstractRuleImpl for BanUnresolvedType {
    fn base(&self) -> &AbstractRule {
        &self.base.base
    }
    // port: ConformanceRules.BanUnresolvedType#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if n.is_get_prop(t) {
            let target = n.get_first_child(t).unwrap();
            let type_ = target.get_jstype(t);
            let non_conforming_part = {
                let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
                Self::get_non_conforming_part(registry, ast, type_)
            };
            if let Some(non_conforming_part) = non_conforming_part
                && !self.base.is_type_immediately_tightened(t, n)
            {
                return ConformanceResult::with_note(
                    ConformanceLevel::VIOLATION,
                    format!("Reference to type '{non_conforming_part}' never resolved."),
                );
            }
        }
        ConformanceResult::conformance()
    }
}

/// Ban any use of unresolved forward-declared types
// port: ConformanceRules.StrictBanUnresolvedType
pub struct StrictBanUnresolvedType {
    base: AbstractTypeRestrictionRule,
}

impl StrictBanUnresolvedType {
    // port: ConformanceRules.StrictBanUnresolvedType#StrictBanUnresolvedType
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        Ok(Self {
            base: AbstractTypeRestrictionRule::new(compiler, requirement)?,
        })
    }
}

impl AbstractRuleImpl for StrictBanUnresolvedType {
    fn base(&self) -> &AbstractRule {
        &self.base.base
    }
    // port: ConformanceRules.StrictBanUnresolvedType#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        let type_ = n.get_jstype(t);
        let non_conforming_part = {
            let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
            BanUnresolvedType::get_non_conforming_part(registry, ast, type_)
        };
        if let Some(non_conforming_part) = non_conforming_part
            && !self.base.is_type_immediately_tightened(t, n)
        {
            return ConformanceResult::with_note(
                ConformanceLevel::VIOLATION,
                format!("Reference to type '{non_conforming_part}' never resolved."),
            );
        }
        ConformanceResult::conformance()
    }
}

/// Banned global var declarations.
// port: ConformanceRules.BanGlobalVars
pub struct BanGlobalVars {
    base: AbstractRule,
    allowlisted_names: IndexSet<String>,
}

impl BanGlobalVars {
    // port: ConformanceRules.BanGlobalVars#BanGlobalVars
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;

        let allowlisted_names = requirement.get_value_list().iter().cloned().collect();
        Ok(Self {
            base,
            allowlisted_names,
        })
    }

    // port: ConformanceRules.BanGlobalVars#isAllowlisted
    fn is_allowlisted(&self, compiler: &mut AbstractCompiler, n: NodeId) -> bool {
        if n.is_from_externs(compiler) {
            return true;
        }

        if n.is_function(compiler) {
            let name = n.get_first_child(compiler).unwrap().get_string(compiler);
            return self.is_allowlisted_name(&name);
        }

        if NodeUtil::is_name_declaration(compiler, Some(n)) {
            let mut allowlisted = true; // for lambda access
            NodeUtil::visit_lhs_nodes_in_node(compiler, n, &mut |compiler, name| {
                if !self.is_allowlisted_name(&name.get_string(compiler)) {
                    allowlisted = false;
                }
            });
            return allowlisted;
        }

        false
    }

    // port: ConformanceRules.BanGlobalVars#isAllowlistedName
    fn is_allowlisted_name(&self, name: &JsString) -> bool {
        let name_string = name.to_string();
        if self.allowlisted_names.contains(&name_string) {
            return true;
        }
        // names created by JSCompiler internals
        name_string == "$jscomp"
            || name_string.starts_with("$jscomp$compprop")
            || ClosureRewriteModule::is_module_content(name)
            || ClosureRewriteModule::is_module_export(name)
            || ScopedAliases::is_scoped_aliases(&name_string)
    }
}

impl AbstractRuleImpl for BanGlobalVars {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.BanGlobalVars#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if t.in_global_scope()
            && NodeUtil::is_declaration(t, n)
            && !n.get_boolean_prop(t, Prop::IS_NAMESPACE)
            && !self.is_allowlisted(t.get_compiler(), n)
        {
            let compiler = t.get_compiler();
            let enclosing_script = NodeUtil::get_enclosing_script(compiler, n);
            if let Some(enclosing_script) = enclosing_script
                && (enclosing_script.get_boolean_prop(compiler, Prop::GOOG_MODULE)
                    || enclosing_script.get_boolean_prop(compiler, Prop::ES6_MODULE)
                    || *enclosing_script
                        .get_input_id(compiler)
                        .expect("NullPointerException")
                        == *compiler.get_synthetic_code_input_id())
            {
                return ConformanceResult::conformance();
            }
            return ConformanceResult::violation();
        }
        ConformanceResult::conformance()
    }
}

// ---------------------------------------------------------------------------------------------
// BanCreateElement ... BanStaticThis

// port: ConformanceRules#GOOG_DOM
static GOOG_DOM: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.dom"));
// port: ConformanceRules#GOOG_DOM_TAGNAME
static GOOG_DOM_TAGNAME: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.dom.TagName"));

/// Java `String#toLowerCase(Locale.ROOT)`.
fn to_lower_case_root(s: &str) -> String {
    s.to_lowercase()
}

/// Bans `document.createElement` and similar methods with string literal parameter specified in
/// `value`, e.g. `value: 'script'`. The purpose of banning these is that they don't provide the
/// type information which hinders other rules. Authors should use e.g.
/// `goog.dom.createElement(goog.dom.TagName.SCRIPT)` which returns HTMLScriptElement.
// port: ConformanceRules.BanCreateElement
pub struct BanCreateElement {
    base: AbstractRule,
    banned_tags: IndexSet<String>,
    dom_helper_type: Option<TypeId>,
    document_type: Option<TypeId>,
}

impl BanCreateElement {
    // port: ConformanceRules.BanCreateElement#BanCreateElement
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;
        let mut banned_tags: IndexSet<String> = IndexSet::<_>::default();
        for value in requirement.get_value_list() {
            // Ascii.toLowerCase
            banned_tags.insert(value.to_ascii_lowercase());
        }
        if banned_tags.is_empty() {
            return Err(InvalidRequirementSpec::new("Specify one or more values."));
        }
        let (registry, ast) = compiler.get_type_registry_and_ast();
        let dom_helper_type = registry.get_global_type(ast, "goog.dom.DomHelper");
        let document_type = registry.get_global_type(ast, "Document");
        Ok(Self {
            base,
            banned_tags,
            dom_helper_type,
            document_type,
        })
    }

    // port: ConformanceRules.BanCreateElement#checkCreateElement
    fn check_create_element(&self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        let target = n.get_first_child(t).unwrap();
        if !target.is_get_prop(t) {
            return ConformanceResult::conformance();
        }
        let function_name = target.get_string(t);
        if function_name != "createElement" && function_name != "createDom" {
            return ConformanceResult::conformance();
        }

        let src_obj = target.get_first_child(t).unwrap();
        if GOOG_DOM.matches(t, src_obj) {
            return ConformanceResult::violation();
        }
        let type_ = src_obj.get_jstype(t);
        let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
        let Some(type_) =
            type_.filter(|&type_| !ConformanceUtil::is_loose_type(registry, ast, type_))
        else {
            return if self.base.report_loose_type_violations {
                ConformanceResult::possible_violation_due_to_loose_types()
            } else {
                ConformanceResult::conformance()
            };
        };
        if self
            .dom_helper_type
            .is_some_and(|dom_helper_type| dom_helper_type.is_subtype_of(registry, ast, type_))
            || self
                .document_type
                .is_some_and(|document_type| document_type.is_subtype_of(registry, ast, type_))
        {
            return ConformanceResult::violation();
        }
        ConformanceResult::conformance()
    }
}

impl AbstractRuleImpl for BanCreateElement {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.BanCreateElement#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if n.is_call(t) {
            let tag = n.get_second_child(t);
            if let Some(tag) = tag
                && tag.is_string_lit(t)
                && self
                    .banned_tags
                    .contains(&tag.get_string(t).to_string().to_ascii_lowercase())
            {
                return self.check_create_element(t, n);
            }
        }
        ConformanceResult::conformance()
    }
}

/// Ban `goog.dom.createDom` and `goog.dom.DomHelper#createDom` with parameters specified in
/// `value` in the format tagname.attribute, e.g. `value: 'iframe.src'`. Tag name might be also
/// `*` to ban the attribute in any tag. Note that string literal values assigned to banned
/// attributes are allowed as they couldn't be attacker controlled.
// port: ConformanceRules.BanCreateDom
pub struct BanCreateDom {
    base: AbstractRule,
    banned_tag_attrs: Vec<[String; 2]>,
    dom_helper_type: Option<TypeId>,
    class_name_types: TypeId,
}

// port: ConformanceRules.BanCreateDom#ELEMENT_TAG_NAMES
const ELEMENT_TAG_NAMES: &[(&str, &str)] = &[
    ("HTMLAnchorElement", "a"),
    ("HTMLAppletElement", "applet"),
    ("HTMLAreaElement", "area"),
    ("HTMLAudioElement", "audio"),
    ("HTMLBRElement", "br"),
    ("HTMLBaseElement", "base"),
    ("HTMLBaseFontElement", "basefont"),
    ("HTMLBodyElement", "body"),
    ("HTMLButtonElement", "button"),
    ("HTMLCanvasElement", "canvas"),
    ("HTMLDListElement", "dl"),
    ("HTMLDataListElement", "datalist"),
    ("HTMLDetailsElement", "details"),
    ("HTMLDialogElement", "dialog"),
    ("HTMLDirectoryElement", "dir"),
    ("HTMLDivElement", "div"),
    ("HTMLEmbedElement", "embed"),
    ("HTMLFieldSetElement", "fieldset"),
    ("HTMLFontElement", "font"),
    ("HTMLFormElement", "form"),
    ("HTMLFrameElement", "frame"),
    ("HTMLFrameSetElement", "frameset"),
    ("HTMLHRElement", "hr"),
    ("HTMLHeadElement", "head"),
    ("HTMLHeadingElement", "h1"),
    ("HTMLHeadingElement", "h2"),
    ("HTMLHeadingElement", "h3"),
    ("HTMLHeadingElement", "h4"),
    ("HTMLHeadingElement", "h5"),
    ("HTMLHeadingElement", "h6"),
    ("HTMLHtmlElement", "html"),
    ("HTMLIFrameElement", "iframe"),
    ("HTMLImageElement", "img"),
    ("HTMLInputElement", "input"),
    ("HTMLIsIndexElement", "isindex"),
    ("HTMLLIElement", "li"),
    ("HTMLLabelElement", "label"),
    ("HTMLLegendElement", "legend"),
    ("HTMLLinkElement", "link"),
    ("HTMLMapElement", "map"),
    ("HTMLMenuElement", "menu"),
    ("HTMLMetaElement", "meta"),
    ("HTMLMeterElement", "meter"),
    ("HTMLModElement", "del"),
    ("HTMLModElement", "ins"),
    ("HTMLOListElement", "ol"),
    ("HTMLObjectElement", "object"),
    ("HTMLOptGroupElement", "optgroup"),
    ("HTMLOptionElement", "option"),
    ("HTMLOutputElement", "output"),
    ("HTMLParagraphElement", "p"),
    ("HTMLParamElement", "param"),
    ("HTMLPreElement", "pre"),
    ("HTMLProgressElement", "progress"),
    ("HTMLQuoteElement", "blockquote"),
    ("HTMLQuoteElement", "q"),
    ("HTMLScriptElement", "script"),
    ("HTMLSelectElement", "select"),
    ("HTMLSourceElement", "source"),
    ("HTMLSpanElement", "span"),
    ("HTMLStyleElement", "style"),
    ("HTMLTableCaptionElement", "caption"),
    ("HTMLTableCellElement", "td"),
    ("HTMLTableCellElement", "th"),
    ("HTMLTableColElement", "col"),
    ("HTMLTableColElement", "colgroup"),
    ("HTMLTableElement", "table"),
    ("HTMLTableRowElement", "tr"),
    ("HTMLTableSectionElement", "tbody"),
    ("HTMLTableSectionElement", "tfoot"),
    ("HTMLTableSectionElement", "thead"),
    ("HTMLTemplateElement", "template"),
    ("HTMLTextAreaElement", "textarea"),
    ("HTMLTitleElement", "title"),
    ("HTMLTrackElement", "track"),
    ("HTMLUListElement", "ul"),
    ("HTMLVideoElement", "video"),
];

impl BanCreateDom {
    // port: ConformanceRules.BanCreateDom#BanCreateDom
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;
        let mut banned_tag_attrs: Vec<[String; 2]> = Vec::new();
        for value in requirement.get_value_list() {
            // ON_DOT.splitToList(value)
            let tag_attr: Vec<&str> = value.split(ON_DOT).collect();
            if tag_attr.len() != 2 || tag_attr[0].is_empty() || tag_attr[1].is_empty() {
                return Err(InvalidRequirementSpec::new(
                    "Values must be in the format tagname.attribute.",
                ));
            }
            let lowercased_tag = to_lower_case_root(tag_attr[0]);
            banned_tag_attrs.push([lowercased_tag, tag_attr[1].to_string()]);
        }
        if banned_tag_attrs.is_empty() {
            return Err(InvalidRequirementSpec::new("Specify one or more values."));
        }
        let (registry, ast) = compiler.get_type_registry_and_ast();
        let dom_helper_type = registry.get_global_type(ast, "goog.dom.DomHelper");
        let class_name_types = registry.create_union_type(
            ast,
            &[
                registry.get_native_type(JSTypeNative::STRING_TYPE),
                registry.get_native_type(JSTypeNative::ARRAY_TYPE),
                registry.get_native_type(JSTypeNative::NULL_TYPE),
                registry.get_native_type(JSTypeNative::VOID_TYPE),
            ],
        );
        Ok(Self {
            base,
            banned_tag_attrs,
            dom_helper_type,
            class_name_types,
        })
    }

    // port: ConformanceRules.BanCreateDom#getTagNames
    #[allow(clippy::if_same_then_else)] // the two branches are separate in Java too
    fn get_tag_names(t: &mut NodeTraversal<'_>, tag: NodeId) -> Option<Vec<String>> {
        if tag.is_string_lit(t) {
            return Some(vec![to_lower_case_root(&tag.get_string(t).to_string())]);
        } else if tag.is_get_prop(t) && GOOG_DOM_TAGNAME.matches(t, tag.get_first_child(t).unwrap())
        {
            return Some(vec![to_lower_case_root(&tag.get_string(t).to_string())]);
        }
        // TODO(jakubvrana): Support union, e.g. {!TagName<!HTMLDivElement>|!TagName<!HTMLBRElement>}.
        let type_ = tag.get_jstype(t)?;
        let registry = t.get_compiler().get_type_registry();
        if !type_.is_templatized_type(registry) {
            return None;
        }
        let type_as_obj = type_.to_maybe_object_type(registry).unwrap();
        let raw_display_name = type_as_obj
            .get_raw_type(registry)
            .get_display_name(registry)
            .expect("NullPointerException");
        if raw_display_name == "goog.dom.TagName" {
            let template_types = type_as_obj.get_template_types(registry).unwrap_or_default();
            // Iterables.getOnlyElement
            assert!(
                template_types.len() == 1,
                "IllegalArgumentException: expected one element but was: {}",
                template_types.len()
            );
            let tag_type = template_types[0];
            let display_name = tag_type.get_display_name(registry);
            // ImmutableMultimap#get: the values of the key in insertion order (empty if absent).
            return Some(
                ELEMENT_TAG_NAMES
                    .iter()
                    .filter(|(key, _)| display_name.as_deref() == Some(*key))
                    .map(|(_, value)| (*value).to_string())
                    .collect(),
            );
        }
        None
    }

    // port: ConformanceRules.BanCreateDom#isCreateDomCall
    fn is_create_dom_call(&self, t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        if !n.is_call(t) {
            return false;
        }
        let target = n.get_first_child(t).unwrap();
        if !target.is_get_prop(t) {
            return false;
        }
        if target.get_string_ref(t) != "createDom" {
            return false;
        }

        let src_obj = target.get_first_child(t).unwrap();
        if GOOG_DOM.matches(t, src_obj) {
            return true;
        }
        let Some(type_) = src_obj.get_jstype(t) else {
            return false;
        };
        let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
        if type_.equals(registry, ast, self.dom_helper_type) {
            return true;
        }
        false
    }
}

impl AbstractRuleImpl for BanCreateDom {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.BanCreateDom#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if !self.is_create_dom_call(t, n) {
            return ConformanceResult::conformance();
        }
        if n.get_child_count(t) < 3 {
            // goog.dom.createDom('iframe') is fine.
            return ConformanceResult::conformance();
        }

        let tag_names = Self::get_tag_names(t, n.get_second_child(t).unwrap());
        let attrs = n.get_child_at_index(t, 2).unwrap();
        let attrs_type = attrs.get_jstype(t);

        // TODO(tbreisacher): Remove this after the typechecker understands ES6.
        let Some(attrs_type) = attrs_type else {
            // Type information is not available; don't run this check.
            return ConformanceResult::conformance();
        };

        let is_void_type;
        // String or array attribute sets the class.
        let is_class_name = {
            let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
            is_void_type = attrs_type.is_void_type(registry);
            !attrs_type.is_unknown_type(registry, ast)
                && attrs_type.is_subtype_of(registry, ast, self.class_name_types)
        };

        if attrs.is_null(t) || is_void_type {
            // goog.dom.createDom('iframe', null) is fine.
            return ConformanceResult::conformance();
        }

        for tag_attr in &self.banned_tag_attrs {
            if tag_names
                .as_ref()
                .is_some_and(|tag_names| !tag_names.contains(&tag_attr[0]))
                && tag_attr[0] != "*"
            {
                continue;
            }
            let violation = if tag_names.is_some() || tag_attr[0] == "*" {
                ConformanceResult::violation()
            } else if self.base.report_loose_type_violations {
                ConformanceResult::possible_violation()
            } else {
                ConformanceResult::conformance()
            };
            if is_class_name {
                if tag_attr[1] != "class" {
                    continue;
                }
                return violation;
            }
            if tag_attr[1] == "textContent"
                && n.get_child_count(t) > 3
                && violation != ConformanceResult::conformance()
            {
                return violation;
            }
            if !attrs.is_object_lit(t) {
                // Attrs is not an object literal and tagName matches or is unknown.
                return if self.base.report_loose_type_violations {
                    ConformanceResult::possible_violation()
                } else {
                    ConformanceResult::conformance()
                };
            }
            let prop = NodeUtil::get_first_prop_matching_key(
                t,
                attrs,
                &JsString::from(tag_attr[1].as_str()),
            );
            if let Some(prop) = prop {
                if NodeUtil::is_some_compile_time_const_string_value(t, prop) {
                    // Ignore string literal values.
                    continue;
                }
                return violation;
            }

            let mut attr = attrs.get_first_child(t);
            while let Some(a) = attr {
                if a.is_computed_prop(t) {
                    // We don't know if the computed property matches 'src' or not
                    return if self.base.report_loose_type_violations {
                        ConformanceResult::possible_violation_due_to_loose_types()
                    } else {
                        ConformanceResult::conformance()
                    };
                }
                attr = a.get_next(t);
            }
        }

        ConformanceResult::conformance()
    }
}

/// `() -> null`: the supplier of the `SecuritySensitiveAttributes` constructors without one.
pub struct NullGlobalNamespaceSupplier;

impl GlobalNamespaceSupplier for NullGlobalNamespaceSupplier {
    fn get(&mut self, _compiler: &mut AbstractCompiler) -> Option<&mut GlobalNamespace> {
        None
    }
}

/// Checks nodes for conformance with banning the setting of attributes that are on the blocklist.
///
/// Java keeps the `Supplier<GlobalNamespace>` in a field; the only non-null supplier is the
/// owning rule's `getGlobalNamespace`, so the port passes the supplier to the check methods
/// ([`NullGlobalNamespaceSupplier`] for the constructors without one).
// port: ConformanceRules.SecuritySensitiveAttributes
pub struct SecuritySensitiveAttributes {
    banned_atrrs: IndexSet<String>,
}

/// Security-sensitive attributes that are banned from being set.
///
/// Making updates to these attributes requires a new JSCompiler release. You must test the change
/// using a global presubmit "at head" and update any affected allowlists. See
/// go/jscompiler-global-presubmit and go/tsjs-conformance-team-docs.
// port: ConformanceRules.SecuritySensitiveAttributes#ALL_BANNED_ATTRS
pub const ALL_BANNED_ATTRS: &[&str] = &[
    "href",
    "rel",
    "src",
    "srcdoc",
    "action",
    "formaction",
    "sandbox",
    "icon",
    "codebase",
    "data",
];

impl Default for SecuritySensitiveAttributes {
    // port: ConformanceRules.SecuritySensitiveAttributes#SecuritySensitiveAttributes()
    fn default() -> Self {
        Self::new(ALL_BANNED_ATTRS)
    }
}

impl SecuritySensitiveAttributes {
    // port: ConformanceRules.SecuritySensitiveAttributes#SecuritySensitiveAttributes(Collection,Supplier)
    pub fn new<S: AsRef<str>>(banned_atrrs: &[S]) -> Self {
        Self {
            banned_atrrs: banned_atrrs
                .iter()
                .map(|s| to_lower_case_root(s.as_ref()))
                .collect(),
        }
    }

    /// Checks if a attribute name is on the security banlist. Callers should make sure the
    /// attribute name is lower-cased, as attribute names are case-insensitve in HTML.
    // port: ConformanceRules.SecuritySensitiveAttributes#contains
    pub fn contains(&self, attribute_name: &str) -> bool {
        self.banned_atrrs.contains(attribute_name)
    }

    /// Given a `NodeTraversal` and `Node`, check if the attribute violates conformance.
    ///
    /// A violation is returned if the attribute name cannot be determined (and it is not an xid),
    /// if the attribute is on a list of banned attributes, or if it begins with the letters "on".
    /// Otherwise, it is a conforming attribute.
    // port: ConformanceRules.SecuritySensitiveAttributes#checkConformanceForAttributeName
    pub fn check_conformance_for_attribute_name(
        &self,
        traversal: &mut NodeTraversal<'_>,
        attr_name: NodeId,
        global_namespace_supplier: &mut dyn GlobalNamespaceSupplier,
    ) -> ConformanceResult {
        let scope = traversal.get_scope();
        let literal_name = ConformanceUtil::infer_string_value(
            traversal.get_compiler(),
            Some(scope),
            Some(attr_name),
            global_namespace_supplier,
        );
        let Some(literal_name) = literal_name else {
            // xid() obfuscates attribute names, thus never clashing with security-sensitive
            // attributes.
            let attr_type = attr_name.get_jstype(traversal);
            return if ConformanceUtil::is_xid(
                traversal.get_compiler().get_type_registry(),
                attr_type,
            ) {
                ConformanceResult::conformance()
            } else {
                ConformanceResult::violation()
            };
        };

        let literal_name = to_lower_case_root(&literal_name.to_string());

        if self.banned_atrrs.contains(&literal_name)
            || ConformanceUtil::is_event_handler_attr_name(&literal_name)
        {
            return ConformanceResult::violation();
        }
        ConformanceResult::conformance()
    }

    /// Given a `NodeTraversal` and `Node`, check if the attribute violates conformance.
    ///
    /// A violation is returned only if the attribute name can be statically determined and is on
    /// the list of banned attributes.
    // port: ConformanceRules.SecuritySensitiveAttributes#checkConformanceForAttributeNameWithHighConfidence
    pub fn check_conformance_for_attribute_name_with_high_confidence(
        &self,
        traversal: &mut NodeTraversal<'_>,
        attr_name: NodeId,
        global_namespace_supplier: &mut dyn GlobalNamespaceSupplier,
    ) -> ConformanceResult {
        let scope = traversal.get_scope();
        let literal_name = ConformanceUtil::infer_string_value(
            traversal.get_compiler(),
            Some(scope),
            Some(attr_name),
            global_namespace_supplier,
        );
        let Some(literal_name) = literal_name else {
            return ConformanceResult::conformance();
        };
        if self.contains(&to_lower_case_root(&literal_name.to_string())) {
            ConformanceResult::violation()
        } else {
            ConformanceResult::conformance()
        }
    }
}

/// The lazily built namespace of `BanElementSetAttribute` (its fields
/// `performGlobalNamespaceAnalysis` and `globalNamespace`, and `getGlobalNamespace`).
struct BanElementSetAttributeGlobalNamespace {
    perform_global_namespace_analysis: bool,
    global_namespace: Option<GlobalNamespace>,
}

impl GlobalNamespaceSupplier for BanElementSetAttributeGlobalNamespace {
    /// Build GlobalNamespace starting from the Root node of the input, excluding externs. Skip
    /// the build if the input AST is too large.
    // port: ConformanceRules.BanElementSetAttribute#getGlobalNamespace
    fn get(&mut self, compiler: &mut AbstractCompiler) -> Option<&mut GlobalNamespace> {
        if self.perform_global_namespace_analysis && self.global_namespace.is_none() {
            let js_root = compiler.get_js_root().expect("NullPointerException");
            if NodeUtil::count_ast_size_up_to_limit(
                compiler,
                js_root,
                BanElementSetAttribute::GLOBAL_NAMESPACE_ANALYSIS_LIMIT,
            ) >= BanElementSetAttribute::GLOBAL_NAMESPACE_ANALYSIS_LIMIT
            {
                self.perform_global_namespace_analysis = false;
            } else {
                self.global_namespace =
                    Some(GlobalNamespace::new_without_externs(compiler, js_root));
            }
        }
        self.global_namespace.as_mut()
    }
}

/// Ban `Element#setAttribute` with attribute names specified in `value` or any dynamic string.
///
/// Using the element access syntax to set properties of `Element` is considered to be equivalent
/// to calling `Element#setAttribute`. E.g., `element[prop] = value` is treated as
/// `element.setAttribute(prop, value)` when `element` has the type `Element`.
// port: ConformanceRules.BanElementSetAttribute
pub struct BanElementSetAttribute {
    base: AbstractRule,
    global_namespace: BanElementSetAttributeGlobalNamespace,
    element_type: Option<TypeId>,
    security_sensitive_attributes: SecuritySensitiveAttributes,
    default_decision_for_uncertain_cases: ConformanceResult,
}

impl BanElementSetAttribute {
    // port: ConformanceRules.BanElementSetAttribute#ELEMENT_TYPE_NAME
    const ELEMENT_TYPE_NAME: &'static str = "Element";
    // port: ConformanceRules.BanElementSetAttribute#SET_ATTRIBUTE
    const SET_ATTRIBUTE: &'static str = "setAttribute";
    // port: ConformanceRules.BanElementSetAttribute#SET_ATTRIBUTE_NS
    const SET_ATTRIBUTE_NS: &'static str = "setAttributeNS";
    // port: ConformanceRules.BanElementSetAttribute#SET_ATTRIBUTE_NODE
    const SET_ATTRIBUTE_NODE: &'static str = "setAttributeNode";
    // port: ConformanceRules.BanElementSetAttribute#SET_ATTRIBUTE_NODE_NS
    const SET_ATTRIBUTE_NODE_NS: &'static str = "setAttributeNodeNS";
    // port: ConformanceRules.BanElementSetAttribute#BANNED_PROPERTIES
    const BANNED_PROPERTIES: &'static [&'static str] = &[
        Self::SET_ATTRIBUTE,
        Self::SET_ATTRIBUTE_NS,
        Self::SET_ATTRIBUTE_NODE,
        Self::SET_ATTRIBUTE_NODE_NS,
    ];

    /// The cap for script size, beyond which we give up performing global namespace analysis due
    /// to exccessive performance cost. The value is purely herustic and subject to future
    /// regulation.
    // port: ConformanceRules.BanElementSetAttribute#GLOBAL_NAMESPACE_ANALYSIS_LIMIT
    const GLOBAL_NAMESPACE_ANALYSIS_LIMIT: i32 = 10000;

    /// Create a custom checker to ban `Element#setAttribute` based on conformance a requirement
    /// spec.
    // port: ConformanceRules.BanElementSetAttribute#BanElementSetAttribute
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;
        let security_sensitive_attributes =
            SecuritySensitiveAttributes::new(requirement.get_value_list());
        let default_decision_for_uncertain_cases = if requirement.get_report_loose_type_violations()
        {
            ConformanceResult::violation()
        } else {
            ConformanceResult::conformance()
        };
        let (registry, ast) = compiler.get_type_registry_and_ast();
        let element_type = registry.get_global_type(ast, Self::ELEMENT_TYPE_NAME);
        Ok(Self {
            base,
            global_namespace: BanElementSetAttributeGlobalNamespace {
                perform_global_namespace_analysis: true,
                global_namespace: None,
            },
            element_type,
            security_sensitive_attributes,
            default_decision_for_uncertain_cases,
        })
    }

    // port: ConformanceRules.BanElementSetAttribute#checkConformanceOnPropertyCall
    fn check_conformance_on_property_call(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        call_node: NodeId,
    ) -> ConformanceResult {
        let called_property = self.get_banned_property_name(traversal, call_node);
        let Some(called_property) = called_property else {
            return ConformanceResult::conformance();
        };

        let report_loose_type_violations = self.base.requirement.get_report_loose_type_violations();
        if called_property == Self::SET_ATTRIBUTE {
            // If the setAttribute function is called with less than two arguments it's either a
            // false positive or uncompilable code.
            if call_node.get_child_count(traversal) < 3 {
                return ConformanceResult::conformance();
            }

            let second = call_node.get_second_child(traversal).unwrap();
            if report_loose_type_violations {
                return self
                    .security_sensitive_attributes
                    .check_conformance_for_attribute_name(
                        traversal,
                        second,
                        &mut self.global_namespace,
                    );
            } else {
                return self
                    .security_sensitive_attributes
                    .check_conformance_for_attribute_name_with_high_confidence(
                        traversal,
                        second,
                        &mut self.global_namespace,
                    );
            }
        }

        if called_property == Self::SET_ATTRIBUTE_NS {
            // If the setAttributeNS function is called with less than three arguments it's either
            // a false positive or uncompilable code.
            if call_node.get_child_count(traversal) < 4 {
                return ConformanceResult::conformance();
            }

            if call_node
                .get_second_child(traversal)
                .unwrap()
                .is_null(traversal)
            {
                let third = call_node.get_child_at_index(traversal, 2).unwrap();
                // A null namespace makes this equivalent to a regular setAttribute call.
                if report_loose_type_violations {
                    return self
                        .security_sensitive_attributes
                        .check_conformance_for_attribute_name(
                            traversal,
                            third,
                            &mut self.global_namespace,
                        );
                } else {
                    return self
                        .security_sensitive_attributes
                        .check_conformance_for_attribute_name_with_high_confidence(
                            traversal,
                            third,
                            &mut self.global_namespace,
                        );
                }
            }
        }

        // We haven't defined a security contract on setAttributeNode and setAttributeNodeNS yet, so
        // flag them as risky if report_loose_type_violations is set.
        self.default_decision_for_uncertain_cases.clone()
    }

    // port: ConformanceRules.BanElementSetAttribute#getBannedPropertyName
    fn get_banned_property_name(
        &self,
        t: &mut NodeTraversal<'_>,
        call_node: NodeId,
    ) -> Option<&'static str> {
        let target = call_node.get_first_child(t).unwrap();
        if !target.is_get_prop(t) {
            return None;
        }
        let property_name = target.get_string(t);
        let property_name = *Self::BANNED_PROPERTIES
            .iter()
            .find(|banned| property_name == **banned)?;
        let type_ = target.get_first_child(t).unwrap().get_jstype(t)?;
        let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
        if let Some(element_type) = self.element_type
            && type_
                .restrict_by_not_null_or_undefined(registry, ast)
                .is_subtype_of(registry, ast, element_type)
        {
            return Some(property_name);
        }
        None
    }

    /// Checks if there is any security implication in a GetElem node.
    ///
    /// Returns `true` if the checked `Node` is setting a security sensitive property on an
    /// `Element` object through the bracket syntax.
    // port: ConformanceRules.BanElementSetAttribute#checkConformanceOnGetElement
    fn check_conformance_on_get_element(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        get_element_node: NodeId,
    ) -> ConformanceResult {
        let key = get_element_node.get_second_child(traversal).unwrap();
        let scope = traversal.get_scope();
        let key_name = ConformanceUtil::infer_string_value(
            traversal.get_compiler(),
            Some(scope),
            Some(key),
            &mut self.global_namespace,
        );
        let report_loose_type_violations = self.base.requirement.get_report_loose_type_violations();
        if let Some(key_name) = key_name {
            let key_name = key_name.to_string();
            if (self
                .security_sensitive_attributes
                .contains(&to_lower_case_root(&key_name))
                || (report_loose_type_violations
                // innerHTML and outerHTML are not attributes but properties. For simplicity we
                // check them here instead of having another rule.
                && (key_name == "innerHTML" || key_name == "outerHTML")))
                && self.has_element_type(traversal, get_element_node)
            {
                return ConformanceResult::violation();
            }
            return ConformanceResult::conformance();
        }

        if !report_loose_type_violations {
            return ConformanceResult::conformance();
        }

        // We have seen code using other types of keys (e.g., numbers) for element access. Only
        // report violations if the key is explicitly typed as string or the union of string and
        // other types.
        let key_type = key.get_jstype(traversal);
        let Some(key_type) = key_type else {
            return ConformanceResult::conformance();
        };
        if ConformanceUtil::is_xid(traversal.get_compiler().get_type_registry(), Some(key_type)) {
            return ConformanceResult::conformance();
        }
        if self.has_element_type(traversal, get_element_node) {
            let (registry, ast) = traversal.get_compiler().get_type_registry_and_ast();
            if key_type.is_string(registry, ast) {
                return ConformanceResult::violation();
            } else if key_type.is_union_type(registry) {
                let alternates = key_type
                    .to_maybe_union_type(registry)
                    .unwrap()
                    .get_alternates(registry, ast);
                for &alternate in alternates.iter() {
                    if alternate.is_string(registry, ast) {
                        return ConformanceResult::violation();
                    }
                }
            }
        }

        ConformanceResult::conformance()
    }

    // port: ConformanceRules.BanElementSetAttribute#hasElementType
    fn has_element_type(&self, t: &mut NodeTraversal<'_>, get_element_node: NodeId) -> bool {
        let obj_type = get_element_node.get_first_child(t).unwrap().get_jstype(t);

        // Do not further check the node if there's no type information available.
        let (Some(obj_type), Some(element_type)) = (obj_type, self.element_type) else {
            return false;
        };
        let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
        let obj_type = obj_type.restrict_by_not_null_or_undefined(registry, ast);
        // If the type of the object is not known to be a subtype of Element, the code is not
        // equivalent to setAttribute. Without this check, we will get overwhelmed by false
        // positives.
        if obj_type.is_unknown_type(registry, ast)
            || obj_type.is_empty_type(registry)
            || !obj_type.is_subtype_of(registry, ast, element_type)
        {
            return false;
        }

        true
    }
}

impl AbstractRuleImpl for BanElementSetAttribute {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.BanElementSetAttribute#checkConformance
    fn check_conformance(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        node: NodeId,
    ) -> ConformanceResult {
        if node.is_call(traversal) {
            return self.check_conformance_on_property_call(traversal, node);
        } else if node.is_get_elem(traversal) && NodeUtil::is_l_value(traversal, node) {
            return self.check_conformance_on_get_element(traversal, node);
        }
        ConformanceResult::conformance()
    }
}

// port: ConformanceRules.BanSettingAttributes.AttributeSettingRestriction
struct AttributeSettingRestriction {
    type_: Option<TypeId>,
    property: String,
}

// port: ConformanceRules.BanSettingAttributes#BANNED_ATTRS
static BAN_SETTING_ATTRIBUTES_BANNED_ATTRS: LazyLock<SecuritySensitiveAttributes> =
    LazyLock::new(SecuritySensitiveAttributes::default);

/// Can be used to ban any function like `Element#setAttribute` which takes two parameters, an
/// attribute name and a value.
///
/// The specific attribute names that are banned are specified in
/// `SecuritySensitiveAttributes.BANNED_ATTRS`.
// port: ConformanceRules.BanSettingAttributes
pub struct BanSettingAttributes {
    base: AbstractRule,
    restrictions: Vec<AttributeSettingRestriction>,
}

impl BanSettingAttributes {
    /// Create a custom checker to ban a function like `Element#setAttribute` based on a
    /// conformance requirement spec.
    ///
    /// Names in `value` fields indicate the functions that should be blocked.
    // port: ConformanceRules.BanSettingAttributes#BanSettingAttributes
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;

        if requirement.get_value_list().is_empty() {
            return Err(InvalidRequirementSpec::new("missing value"));
        }

        let (registry, ast) = compiler.get_type_registry_and_ast();
        let mut builder: Vec<AttributeSettingRestriction> = Vec::new();
        for value in requirement.get_value_list() {
            let type_ = ConformanceUtil::get_class_from_declaration_name(value);
            let property = ConformanceUtil::get_property_from_declaration_name(value);
            let (Some(type_), Some(property)) = (type_, property) else {
                return Err(InvalidRequirementSpec::new("bad prop value"));
            };
            builder.push(AttributeSettingRestriction {
                type_: registry.get_global_type(ast, type_.as_str()),
                property,
            });
        }

        Ok(Self {
            base,
            restrictions: builder,
        })
    }

    // port: ConformanceRules.BanSettingAttributes#checkConformanceOnPropertyCall
    fn check_conformance_on_property_call(
        &self,
        traversal: &mut NodeTraversal<'_>,
        node: NodeId,
    ) -> ConformanceResult {
        let called_property = self.get_banned_property_name(traversal, node);
        if called_property.is_none() {
            return ConformanceResult::conformance();
        }

        // This node is conformant if it has less than three children (the function and two
        // arguments). It's either reading the attribute, a false positive, or uncompilable code.
        if node.get_child_count(traversal) < 3 {
            return ConformanceResult::conformance();
        }

        let second = node.get_second_child(traversal).unwrap();
        BAN_SETTING_ATTRIBUTES_BANNED_ATTRS.check_conformance_for_attribute_name(
            traversal,
            second,
            &mut NullGlobalNamespaceSupplier,
        )
    }

    // port: ConformanceRules.BanSettingAttributes#getBannedPropertyName
    fn get_banned_property_name(
        &self,
        t: &mut NodeTraversal<'_>,
        node: NodeId,
    ) -> Option<JsString> {
        let target = node.get_first_child(t).unwrap();
        if !target.is_get_prop(t) {
            return None;
        }
        let type_ = target.get_first_child(t).unwrap().get_jstype(t)?;
        let property_name = target.get_string(t);
        let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
        for restricted in &self.restrictions {
            if property_name == restricted.property.as_str()
                && let Some(restricted_type) = restricted.type_
                && type_.is_subtype_of(registry, ast, restricted_type)
            {
                return Some(property_name);
            }
        }
        None
    }
}

impl AbstractRuleImpl for BanSettingAttributes {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.BanSettingAttributes#checkConformance
    fn check_conformance(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        node: NodeId,
    ) -> ConformanceResult {
        if node.is_call(traversal) {
            return self.check_conformance_on_property_call(traversal, node);
        }
        ConformanceResult::conformance()
    }
}

/// Ban `Document#execCommand` with command names specified in `value` or any dynamic string.
// port: ConformanceRules.BanExecCommand
pub struct BanExecCommand {
    base: AbstractRule,
    banned_attrs: Vec<String>,
}

impl BanExecCommand {
    /// Creates a custom checker to ban `Document#execCommand` based on a conformance requirement
    /// spec. Names in `value` fields indicate the attribute names that should be blocked.
    // port: ConformanceRules.BanExecCommand#BanExecCommand
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;
        let banned_attrs = requirement
            .get_value_list()
            .iter()
            .map(|v| to_lower_case_root(v))
            .collect();
        Ok(Self { base, banned_attrs })
    }

    // port: ConformanceRules.BanExecCommand#isBannedProperty
    fn is_banned_property(t: &mut NodeTraversal<'_>, node: NodeId) -> bool {
        if !node.is_call(t) {
            return false;
        }
        let target = node.get_first_child(t).unwrap();
        if !target.is_get_prop(t) {
            return false;
        }
        let property_name = target.get_string(t);
        if property_name != "execCommand" {
            return false;
        }
        let Some(type_) = target.get_first_child(t).unwrap().get_jstype(t) else {
            return false;
        };
        let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
        let document_type = registry.get_global_type(ast, "Document");
        let Some(document_type) = document_type else {
            return false;
        };
        if !type_
            .restrict_by_not_null_or_undefined(registry, ast)
            .is_subtype_of(registry, ast, document_type)
        {
            return false;
        }
        true
    }
}

impl AbstractRuleImpl for BanExecCommand {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.BanExecCommand#checkConformance
    fn check_conformance(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        node: NodeId,
    ) -> ConformanceResult {
        if !Self::is_banned_property(traversal, node) {
            return ConformanceResult::conformance();
        }

        // If the function is called without arguments then it's either a false positive or
        // uncompilable code.
        if node.get_child_count(traversal) < 2 {
            return ConformanceResult::conformance();
        }

        let attr = node.get_second_child(traversal).unwrap();
        if !attr.is_string_lit(traversal) {
            return ConformanceResult::violation();
        }

        let attr_name = attr.get_string(traversal).to_string();
        if self.banned_attrs.contains(&to_lower_case_root(&attr_name)) {
            return ConformanceResult::violation();
        }
        ConformanceResult::conformance()
    }
}

/// Checks that `this` is not being referenced directly within a static member function.
// port: ConformanceRules.BanStaticThis
pub struct BanStaticThis {
    base: AbstractTypeRestrictionRule,
}

impl BanStaticThis {
    // port: ConformanceRules.BanStaticThis#BanStaticThis
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        Ok(Self {
            base: AbstractTypeRestrictionRule::new(compiler, requirement)?,
        })
    }

    // port: ConformanceRules.BanStaticThis#isStaticMethod
    fn is_static_method(ast: &Ast, n: NodeId) -> bool {
        closure_rhino::check_argument!(n.is_function(ast));
        let parent = n.get_parent(ast);
        parent.is_some_and(|parent| {
            parent.is_member_function_def(ast) && parent.is_static_member(ast)
        })
    }
}

impl AbstractRuleImpl for BanStaticThis {
    fn base(&self) -> &AbstractRule {
        &self.base.base
    }
    // port: ConformanceRules.BanStaticThis#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        if !n.is_this(t) {
            return ConformanceResult::conformance();
        }

        let enclosing_function = NodeUtil::get_enclosing_non_arrow_function(t, n);
        if let Some(enclosing_function) = enclosing_function
            && Self::is_static_method(t, enclosing_function)
        {
            return ConformanceResult::violation();
        }
        ConformanceResult::conformance()
    }
}

// ---------------------------------------------------------------------------------------------
// BannedCodePattern

/// Banned Code Pattern rule
// port: ConformanceRules.BannedCodePattern
pub struct BannedCodePattern {
    base: AbstractRule,
    restrictions: Vec<TemplateAstMatcher>,
}

impl BannedCodePattern {
    // port: ConformanceRules.BannedCodePattern#BannedCodePattern
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        let base = AbstractRule::new(compiler, requirement)?;

        if requirement.get_value_count() == 0 {
            return Err(InvalidRequirementSpec::new("missing value"));
        }

        let mut builder: Vec<TemplateAstMatcher> = Vec::new();
        for value in requirement.get_value_list() {
            let parse_root =
                CompilerInput::new(SourceFile::from_code("<template>", value.as_str()))
                    .get_ast_root(compiler);
            if !parse_root.has_one_child(compiler)
                || !parse_root
                    .get_first_child(compiler)
                    .unwrap()
                    .is_function(compiler)
            {
                return Err(InvalidRequirementSpec::new(format!(
                    "invalid conformance template: {value}"
                )));
            }
            let template_root = parse_root.get_first_child(compiler).unwrap();
            let ast_matcher =
                TemplateAstMatcher::new(compiler, template_root, base.type_matching_strategy);
            builder.push(ast_matcher);
        }

        Ok(Self {
            base,
            restrictions: builder,
        })
    }
}

impl AbstractRuleImpl for BannedCodePattern {
    fn base(&self) -> &AbstractRule {
        &self.base
    }
    // port: ConformanceRules.BannedCodePattern#checkConformance
    fn check_conformance(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) -> ConformanceResult {
        let mut possible_violation = false;
        for i in 0..self.restrictions.len() {
            let matcher = &mut self.restrictions[i];
            if matcher.matches(t.get_compiler(), n) {
                if matcher.is_loose_match() {
                    possible_violation = true;
                } else {
                    return ConformanceResult::violation();
                }
            }
        }
        if possible_violation && self.base.report_loose_type_violations {
            ConformanceResult::possible_violation_due_to_loose_types()
        } else {
            ConformanceResult::conformance()
        }
    }
}

// ---------------------------------------------------------------------------------------------
// CustomRuleProxy

/// A custom rule proxy, for rules that we load dynamically.
// port: ConformanceRules.CustomRuleProxy
pub struct CustomRuleProxy {
    custom_rule: Box<dyn Rule>,
}

type RuleConstructor =
    fn(&mut AbstractCompiler, &Requirement) -> Result<Box<dyn Rule>, InvalidRequirementSpec>;

/// What `Class.forName(className)` finds for a class of the compiler (Java reflection; the port
/// knows the classes of `ConformanceRules`, the package `CustomRuleProxy` loads its rules from).
enum RuleClass {
    /// A `Rule` class with a public `(AbstractCompiler, Requirement)` constructor.
    Constructible(RuleConstructor),
    /// A `Rule` class without public constructors.
    NoPublicConstructor,
    /// An abstract `Rule` class: `Constructor#newInstance` throws `InstantiationException`.
    Abstract,
    /// A class that is not a `Rule`.
    NotRule,
}

macro_rules! rule_constructor {
    ($rule:ident) => {
        (|compiler, requirement| Ok(Box::new($rule::new(compiler, requirement)?) as Box<dyn Rule>))
            as RuleConstructor
    };
}

/// The binary name prefix of the nested classes of `ConformanceRules`.
const CONFORMANCE_RULES_CLASS_PREFIX: &str = "com.google.javascript.jscomp.ConformanceRules$";

impl CustomRuleProxy {
    // port: ConformanceRules.CustomRuleProxy#CustomRuleProxy
    pub fn new(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Self, InvalidRequirementSpec> {
        if !requirement.has_java_class() {
            return Err(InvalidRequirementSpec::new("missing java_class"));
        }
        let custom_rule = Self::create_rule(compiler, requirement)?;
        Ok(Self { custom_rule })
    }

    // port: ConformanceRules.CustomRuleProxy#createRule
    fn create_rule(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Result<Box<dyn Rule>, InvalidRequirementSpec> {
        let custom = Self::get_rule_class(requirement.get_java_class())?;
        let ctor = Self::get_rule_constructor(&custom)?;
        // ctor.newInstance(compiler, requirement): an InvalidRequirementSpec thrown by the
        // constructor is rethrown; InstantiationException becomes a RuntimeException.
        match ctor {
            Some(ctor) => ctor(compiler, requirement),
            None => panic!(
                "java.lang.RuntimeException: java.lang.InstantiationException: {}",
                requirement.get_java_class()
            ),
        }
    }

    /// Returns the public `(AbstractCompiler, Requirement)` constructor (`None` for an abstract
    /// class, whose constructor exists but cannot be instantiated).
    // port: ConformanceRules.CustomRuleProxy#getRuleConstructor
    fn get_rule_constructor(
        cls: &RuleClass,
    ) -> Result<Option<RuleConstructor>, InvalidRequirementSpec> {
        match cls {
            RuleClass::Constructible(ctor) => Ok(Some(*ctor)),
            RuleClass::Abstract => Ok(None),
            RuleClass::NoPublicConstructor | RuleClass::NotRule => Err(
                InvalidRequirementSpec::new("No valid class constructors found."),
            ),
        }
    }

    /// `Class.forName(className)` and the `Rule` supertype check.
    ///
    /// Java resolves any class on the classpath. The port knows the classes of
    /// `ConformanceRules`; a name without a package cannot name a class of the compiler or its
    /// libraries (none is in the default package), so it is not found. Any other class (a custom
    /// rule of the user, or of the Java tests) cannot be loaded by the port: Unported.
    // port: ConformanceRules.CustomRuleProxy#getRuleClass
    fn get_rule_class(class_name: &str) -> Result<RuleClass, InvalidRequirementSpec> {
        let custom_class =
            if let Some(nested) = class_name.strip_prefix(CONFORMANCE_RULES_CLASS_PREFIX) {
                Self::conformance_rules_class(nested)
            } else if class_name == "com.google.javascript.jscomp.ConformanceRules" {
                Some(RuleClass::NotRule)
            } else if !class_name.contains('.') {
                None
            } else {
                panic!("unported: ConformanceRules.CustomRuleProxy java_class {class_name}")
            };
        let Some(custom_class) = custom_class else {
            return Err(InvalidRequirementSpec::with_cause("JavaClass not found."));
        };
        if !matches!(custom_class, RuleClass::NotRule) {
            return Ok(custom_class);
        }
        Err(InvalidRequirementSpec::new("JavaClass is not a rule."))
    }

    /// The nested classes of `ConformanceRules` by their binary name after `ConformanceRules$`.
    fn conformance_rules_class(nested: &str) -> Option<RuleClass> {
        Some(match nested {
            "AbstractRule" | "AbstractTypeRestrictionRule" => RuleClass::Abstract,
            "NoOp"
            | "BannedDependency"
            | "BannedDependencyRegex"
            | "BannedName"
            | "BannedProperty"
            | "RestrictedNameCall"
            | "RestrictedMethodCall"
            | "RestrictedPropertyWrite"
            | "BannedCodePattern"
            | "CustomRuleProxy" => RuleClass::NoPublicConstructor,
            "InferredConstCheck" => RuleClass::Constructible(rule_constructor!(InferredConstCheck)),
            "BannedStringRegex" => RuleClass::Constructible(rule_constructor!(BannedStringRegex)),
            "BanForOf" => RuleClass::Constructible(rule_constructor!(BanForOf)),
            "RequireUseStrict" => RuleClass::Constructible(rule_constructor!(RequireUseStrict)),
            "BanThrowOfNonErrorTypes" => {
                RuleClass::Constructible(rule_constructor!(BanThrowOfNonErrorTypes))
            }
            "BanNullDeref" => RuleClass::Constructible(rule_constructor!(BanNullDeref)),
            "BanUnknownThis" => RuleClass::Constructible(rule_constructor!(BanUnknownThis)),
            "BanUnknownDirectThisPropsReferences" => {
                RuleClass::Constructible(rule_constructor!(BanUnknownDirectThisPropsReferences))
            }
            "BanNonLiteralArgsToGoogStringConstFrom" => {
                RuleClass::Constructible(rule_constructor!(BanNonLiteralArgsToGoogStringConstFrom))
            }
            "BanUnknownTypedClassPropsReferences" => {
                RuleClass::Constructible(rule_constructor!(BanUnknownTypedClassPropsReferences))
            }
            "BanUnresolvedType" => RuleClass::Constructible(rule_constructor!(BanUnresolvedType)),
            "StrictBanUnresolvedType" => {
                RuleClass::Constructible(rule_constructor!(StrictBanUnresolvedType))
            }
            "BanGlobalVars" => RuleClass::Constructible(rule_constructor!(BanGlobalVars)),
            "BannedEnhance" => RuleClass::Constructible(rule_constructor!(BannedEnhance)),
            "BannedModsRegex" => RuleClass::Constructible(rule_constructor!(BannedModsRegex)),
            "BanCreateElement" => RuleClass::Constructible(rule_constructor!(BanCreateElement)),
            "BanCreateDom" => RuleClass::Constructible(rule_constructor!(BanCreateDom)),
            "BanElementSetAttribute" => {
                RuleClass::Constructible(rule_constructor!(BanElementSetAttribute))
            }
            "BanSettingAttributes" => {
                RuleClass::Constructible(rule_constructor!(BanSettingAttributes))
            }
            "BanExecCommand" => RuleClass::Constructible(rule_constructor!(BanExecCommand)),
            "BanStaticThis" => RuleClass::Constructible(rule_constructor!(BanStaticThis)),
            "ConformanceResult"
            | "ConformanceLevel"
            | "AllowList"
            | "ConformanceUtil"
            | "SecuritySensitiveAttributes"
            | "BannedProperty$RequirementPrecondition"
            | "RestrictedNameCall$Restriction"
            | "RestrictedMethodCall$Restriction"
            | "RestrictedPropertyWrite$Restriction"
            | "BanSettingAttributes$AttributeSettingRestriction" => RuleClass::NotRule,
            _ => return None,
        })
    }
}

impl Rule for CustomRuleProxy {
    // port: ConformanceRules.CustomRuleProxy#getPrecondition
    fn get_precondition(&self) -> Precondition {
        self.custom_rule.get_precondition()
    }

    // port: ConformanceRules.CustomRuleProxy#check
    fn check(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        behavior: LibraryLevelNonAllowlistedConformanceViolationsBehavior,
    ) {
        self.custom_rule.check(t, n, behavior);
    }
}
