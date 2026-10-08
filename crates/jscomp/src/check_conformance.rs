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
//   src/com/google/javascript/jscomp/CheckConformance.java,
//   src/com/google/javascript/jscomp/ConformanceRules.java.

//! Port of `CheckConformance.java`.
//!
//! Provides a framework for checking code against a set of user configured conformance
//! requirements (`Requirement`). The requirements are specified by the ConformanceConfig proto,
//! which allows for both standard checks (forbidden properties, variables, or dependencies) and
//! allow for more complex checks using requirements of CUSTOM type.
//!
//! The requirements are translated into rules (`Rule`) which are then checked by this pass.
//!
//! Conformance violations are both reported as compiler errors, and are also reported separately
//! to the `ErrorManager`.
//!
//! Iteration order (DECISIONS): Java groups the rules in a `HashMultimap<Precondition, Rule>`
//! (`initRules`, CheckConformance.java:211). Every `Precondition` and every `Rule` uses identity
//! hashing, so the order of the categories and of the rules inside one category depends on the
//! JVM run. In the port
//! the categories use the fixed order of [`Precondition::canonical_rank`], chosen to reproduce the
//! order observed in the oracle corpus (every CheckConformanceTest processor dump with two
//! categories lists `BannedProperty.RequirementPrecondition.BANNED_PROPERTY` before the
//! `BannedName` lambda). The rules inside one category keep first-insertion order: the corpus
//! shows both orders for the same configuration shape (see
//! `build/questions/conformance-rule-order-within-category.md`).

use crate::{
    AbstractCompiler,
    compiler_options::ConformanceReportingMode,
    compiler_pass::CompilerPass,
    conformance_config::{
        ConformanceConfig, LibraryLevelNonAllowlistedConformanceViolationsBehavior, Requirement,
        requirement::Type,
    },
    conformance_rules::{self, RequirementPrecondition},
    diagnostic_type::DiagnosticType,
    js_error::JSError,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
    protobuf::text_format,
};
use closure_rhino::{
    check_not_null, check_state, node::NodeId, static_source_file::StaticSourceFile, token::Token,
};
use indexmap::{IndexMap, IndexSet};

use LibraryLevelNonAllowlistedConformanceViolationsBehavior::{RECORD_ONLY, UNSPECIFIED};

// port: CheckConformance#CONFORMANCE_ERROR
pub static CONFORMANCE_ERROR: DiagnosticType =
    DiagnosticType::error("JSC_CONFORMANCE_ERROR", "Violation: {0}{1}{2}");

// port: CheckConformance#CONFORMANCE_VIOLATION
pub static CONFORMANCE_VIOLATION: DiagnosticType =
    DiagnosticType::warning("JSC_CONFORMANCE_VIOLATION", "Violation: {0}{1}{2}");

// port: CheckConformance#CONFORMANCE_POSSIBLE_VIOLATION
pub static CONFORMANCE_POSSIBLE_VIOLATION: DiagnosticType = DiagnosticType::warning(
    "JSC_CONFORMANCE_POSSIBLE_VIOLATION",
    "Possible violation: {0}{1}{2}",
);

// port: CheckConformance#INVALID_REQUIREMENT_SPEC
pub static INVALID_REQUIREMENT_SPEC: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_REQUIREMENT_SPEC",
    "Invalid requirement. Reason: {0}\nRequirement spec:\n{1}",
);

/// A rule that can be checked for conformance (Java `CheckConformance.Rule`).
pub trait Rule {
    /// Return a precondition for this rule.
    ///
    /// This method will only be called once (per rule) during the creation of the
    /// CheckConformance pass. Therefore, the return must be constant.
    // port: CheckConformance.Rule#getPrecondition
    fn get_precondition(&self) -> Precondition {
        Precondition::CHECK_ALL
    }

    /// Perform conformance check
    // port: CheckConformance.Rule#check
    fn check(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        behavior: LibraryLevelNonAllowlistedConformanceViolationsBehavior,
    );
}

/// A condition that must be true for a rule to possibly match a node (Java
/// `CheckConformance.Precondition`).
///
/// Instances are used as keys to group rules with common preconditions. Grouping allows shared
/// computation to be done only once per node, which is a substantial performance improvement.
///
/// Java's preconditions are singletons compared by identity: the two constants of the interface,
/// the lambda `BannedName.IS_CANDIDATE_NODE`, the enum `BannedProperty.RequirementPrecondition`
/// and the `Node::isScript` method references of `BannedEnhance` and `BannedModsRegex` (one
/// instance per call site). Each is a variant here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Precondition {
    // port: CheckConformance.Precondition#CHECK_ALL
    CHECK_ALL,
    // port: CheckConformance.Precondition#IS_CONSTRUCTOR_OR_CLASS
    IS_CONSTRUCTOR_OR_CLASS,
    // port: ConformanceRules.BannedName#IS_CANDIDATE_NODE
    BannedNameIsCandidateNode,
    // port: ConformanceRules.BannedProperty.RequirementPrecondition
    BannedProperty(RequirementPrecondition),
    // port: ConformanceRules.BannedEnhance#getPrecondition (Node::isScript)
    BannedEnhanceIsScript,
    // port: ConformanceRules.BannedModsRegex#getPrecondition (Node::isScript)
    BannedModsRegexIsScript,
}

impl Precondition {
    /// The fixed position of this precondition's category in `CheckConformance#categories`
    /// (replaces Java's identity-hash order; see the module documentation).
    fn canonical_rank(self) -> u8 {
        match self {
            Precondition::BannedProperty(RequirementPrecondition::BANNED_PROPERTY) => 0,
            Precondition::BannedProperty(RequirementPrecondition::BANNED_PROPERTY_WRITE) => 1,
            Precondition::BannedProperty(
                RequirementPrecondition::BANNED_PROPERTY_NON_CONSTANT_WRITE,
            ) => 2,
            Precondition::BannedProperty(RequirementPrecondition::BANNED_PROPERTY_READ) => 3,
            Precondition::BannedProperty(RequirementPrecondition::BANNED_PROPERTY_CALL) => 4,
            Precondition::BannedNameIsCandidateNode => 5,
            Precondition::CHECK_ALL => 6,
            Precondition::IS_CONSTRUCTOR_OR_CLASS => 7,
            Precondition::BannedEnhanceIsScript => 8,
            Precondition::BannedModsRegexIsScript => 9,
        }
    }

    // port: CheckConformance.Precondition#shouldCheck
    pub fn should_check(self, t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        match self {
            Precondition::CHECK_ALL => true,
            Precondition::IS_CONSTRUCTOR_OR_CLASS => {
                n.is_class(t) || {
                    let compiler = t.get_compiler();
                    let (registry, ast) = compiler.get_type_registry_and_ast();
                    NodeUtil::is_constructor(ast, Some(n), registry)
                }
            }
            Precondition::BannedNameIsCandidateNode => match n.get_token(t) {
                Token::GETPROP => n.get_first_child(t).unwrap().is_qualified_name(t),
                Token::NAME => !n.get_string(t).is_empty(),
                _ => false,
            },
            Precondition::BannedProperty(p) => p.should_check(t, n),
            Precondition::BannedEnhanceIsScript | Precondition::BannedModsRegexIsScript => {
                n.is_script(t)
            }
        }
    }
}

// port: CheckConformance.Category
struct Category {
    precondition: Precondition,
    /// Indices into `CheckConformance::rules`.
    rules: Vec<usize>,
}

impl Category {
    // port: CheckConformance.Category#Category
    fn new(precondition: Precondition, rules: Vec<usize>) -> Self {
        Self {
            precondition,
            rules,
        }
    }
}

/// Port of `CheckConformance` (a `NodeTraversal.Callback` and `CompilerPass`).
pub struct CheckConformance {
    categories: Vec<Category>,
    /// The rule instances (Java keeps them inside the categories; they are referenced by index
    /// here so that `ruleToBehavior` can be keyed by rule identity).
    rules: Vec<Box<dyn Rule>>,

    /// Map of root requirements to their behavior specified in their configs, or of their
    /// extending requirement's config. It is only populated if both of the following conditions
    /// are met:
    ///
    /// 1. the conformance reporting mode is RESPECT_LIBRARY_LEVEL_BEHAVIOR_SPECIFIED_IN_CONFIG,
    ///    i.e. the conformance checks are being run during the CheckJS action.
    /// 2. the requirement has a behavior that is different from the default UNSPECIFIED behavior
    ///    (i.e. RECORD_ONLY or REPORT_AS_BUILD_ERROR).
    ///
    /// The behavior of each library-level requirement gets stored in this map, and gets passed
    /// into the check() method of its corresponding rule to determine whether to record or report
    /// the conformance violations.
    merged_behaviors:
        IndexMap<Requirement, LibraryLevelNonAllowlistedConformanceViolationsBehavior>,

    /// Map of rules (by index into `rules`) to their behavior. Populated from the
    /// mergedBehaviors map. The behavior of each rule is passed into the check() method of the
    /// rule to determine whether to record or report the conformance violations.
    rule_to_behavior: IndexMap<usize, LibraryLevelNonAllowlistedConformanceViolationsBehavior>,
}

// port: CheckConformance#EXTENDABLE_FIELDS
const EXTENDABLE_FIELDS: &[&str] = &[
    "config_file",
    "extends",
    "only_apply_to",
    "only_apply_to_regexp",
    "whitelist",
    "whitelist_regexp",
    "allowlist",
    "allowlist_regexp",
    "value",
];

impl CheckConformance {
    // port: CheckConformance#CheckConformance(AbstractCompiler)
    pub fn new_empty(compiler: &mut AbstractCompiler) -> Self {
        Self::new(compiler, &[], None)
    }

    // port: CheckConformance#CheckConformance(AbstractCompiler,ImmutableList,ConformanceReportingMode)
    pub fn new(
        compiler: &mut AbstractCompiler,
        configs: &[ConformanceConfig],
        reporting_mode: Option<ConformanceReportingMode>,
    ) -> Self {
        let mut this = Self {
            categories: Vec::new(),
            rules: Vec::new(),
            merged_behaviors: IndexMap::new(),
            rule_to_behavior: IndexMap::new(),
        };
        // Initialize the map of functions to inspect for renaming candidates.
        this.categories = this.init_rules(compiler, configs, reporting_mode);
        this
    }

    // port: CheckConformance#isScriptOfInterest
    fn is_script_of_interest(sf: &dyn StaticSourceFile) -> bool {
        !sf.is_weak() && !sf.is_extern()
    }

    /// Build the data structures need by this pass from the provided configurations.
    // port: CheckConformance#initRules
    fn init_rules(
        &mut self,
        compiler: &mut AbstractCompiler,
        configs: &[ConformanceConfig],
        reporting_mode: Option<ConformanceReportingMode>,
    ) -> Vec<Category> {
        // Java: HashMultimap<Precondition, Rule> (identity-hashed keys and values); the rules of a
        // category keep insertion order, the categories get the fixed order below (module docs).
        let mut builder: IndexMap<Precondition, IndexSet<usize>> = IndexMap::new();

        let is_library_level_reporting_mode = reporting_mode
            == Some(ConformanceReportingMode::RESPECT_LIBRARY_LEVEL_BEHAVIOR_SPECIFIED_IN_CONFIG);
        if is_library_level_reporting_mode
            && !Self::validate_behavior_setting_of_configs(
                compiler,
                configs,
                reporting_mode.unwrap(),
            )
        {
            return Vec::new();
        }

        let merged_requirements =
            self.merge_requirements(compiler, configs, is_library_level_reporting_mode);
        for requirement in merged_requirements {
            let mut behavior = UNSPECIFIED;
            let rule = Self::init_rule(compiler, &requirement);
            if let Some(rule) = rule {
                let index = self.rules.len();
                builder
                    .entry(rule.get_precondition())
                    .or_default()
                    .insert(index);
                self.rules.push(rule);
                if is_library_level_reporting_mode
                    && self.merged_behaviors.contains_key(&requirement)
                {
                    behavior = *check_not_null!(
                        self.merged_behaviors.get(&requirement),
                        "The mergeBehaviors method inserted a null behavior in the mergedBehaviors map"
                    );
                }
                self.rule_to_behavior.insert(index, behavior);
            }
        }
        let mut categories: Vec<Category> = builder
            .into_iter()
            .map(|(precondition, rules)| Category::new(precondition, rules.into_iter().collect()))
            .collect();
        // Java: builder.asMap() iterates in identity-hash order; the port uses the fixed order
        // observed in the oracle corpus (module documentation).
        categories.sort_by_key(|category| category.precondition.canonical_rank());
        categories
    }

    // port: CheckConformance#validateBehaviorSettingOfConfigs
    fn validate_behavior_setting_of_configs(
        compiler: &mut AbstractCompiler,
        configs: &[ConformanceConfig],
        reporting_mode: ConformanceReportingMode,
    ) -> bool {
        // only validate if the behavior matters (i.e. if the compiler is running in library-level
        // conformance mode)
        check_state!(
            reporting_mode
                == ConformanceReportingMode::RESPECT_LIBRARY_LEVEL_BEHAVIOR_SPECIFIED_IN_CONFIG
        );
        let mut base_requirement_behaviors: IndexMap<
            String,
            LibraryLevelNonAllowlistedConformanceViolationsBehavior,
        > = IndexMap::new();
        for config in configs {
            if !config.has_library_level_non_allowlisted_conformance_violations_behavior() {
                // nothing to validate
                continue;
            }
            let behavior =
                config.get_library_level_non_allowlisted_conformance_violations_behavior();
            for requirement in config.get_requirement_list() {
                if requirement.has_rule_id() {
                    base_requirement_behaviors
                        .insert(requirement.get_rule_id().to_string(), behavior);
                }
            }
        }

        for config in configs {
            if !config.has_library_level_non_allowlisted_conformance_violations_behavior() {
                // nothing to validate
                continue;
            }
            for requirement in config.get_requirement_list() {
                if requirement.has_extends() {
                    let extending_behavior =
                        config.get_library_level_non_allowlisted_conformance_violations_behavior();
                    let base_rule_id = requirement.get_extends();
                    if let Some(&base_behavior) = base_requirement_behaviors.get(base_rule_id)
                        && base_behavior != extending_behavior
                    {
                        Self::report_invalid_requirement(
                            compiler,
                            requirement,
                            "extending rule's config may not specify a different value of \
                             'library_level_non_allowlisted_conformance_violations_behavior' than \
                             the base rule's config. Skipping all conformance checks.",
                        );
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Gets requirements from all configs, validates them, and merges any extending requirements
    /// into their respective requirements from which they're extending. This means merging the
    /// allowlists/whitelists of requirements with 'extends' equal to 'rule_id' of other rule.
    ///
    /// The requirements inheritance can not have a chain of more than 1 level (see the Java
    /// documentation of `CheckConformance#mergeRequirements` for the algorithm).
    ///
    /// Returns a list of root requirements of all configs after merging.
    // port: CheckConformance#mergeRequirements
    pub fn merge_requirements(
        &mut self,
        compiler: &mut AbstractCompiler,
        configs: &[ConformanceConfig],
        is_library_level_conformance_reporting_mode: bool,
    ) -> Vec<Requirement> {
        // Java keys these maps by `Requirement.Builder` identity; the builders live in `builders`
        // and the maps hold their indices.
        let mut builders: Vec<Requirement> = Vec::new();
        // Root requirements (i.e. requirements that have no 'extends' field).
        let mut root_requirements: IndexMap<
            usize,
            LibraryLevelNonAllowlistedConformanceViolationsBehavior,
        > = IndexMap::new();
        // Requirements that are extendable (i.e. have a 'rule_id' field).
        let mut extendable: IndexMap<String, usize> = IndexMap::new();

        // 1. Process the root requirements and add them to the rootRequirements list, and process
        // the extendable requirements and add them to the extendable map.
        for config in configs {
            for requirement in config.get_requirement_list() {
                let builder = builders.len();
                builders.push(requirement.to_builder());
                if requirement.has_rule_id() {
                    if !Self::validate_extendable_requirement(compiler, requirement, &extendable) {
                        continue;
                    }
                    // has a rule_id, so it's extendable
                    extendable.insert(requirement.get_rule_id().to_string(), builder);
                }
                let has_behavior =
                    config.has_library_level_non_allowlisted_conformance_violations_behavior();
                let behavior =
                    config.get_library_level_non_allowlisted_conformance_violations_behavior();
                if !requirement.has_extends() {
                    // does not extend anything, so it's a root requirement. May or may not have a
                    // rule_id.
                    if !is_library_level_conformance_reporting_mode {
                        root_requirements.insert(builder, UNSPECIFIED);
                    } else {
                        root_requirements
                            .insert(builder, if has_behavior { behavior } else { UNSPECIFIED });
                    }
                }
                // Enforce that requirements used in library-level conformance checks must have
                // their rule_id or extends fields set.
                let library_level_conformance_enabled = is_library_level_conformance_reporting_mode
                    && has_behavior
                    && behavior == RECORD_ONLY;
                if !requirement.has_rule_id()
                    && !requirement.has_extends()
                    && library_level_conformance_enabled
                {
                    Self::report_invalid_requirement(
                        compiler,
                        requirement,
                        "Library-level conformance requirements require their rule_id or extends \
                         field set.",
                    );
                }
            }
        }

        // 2. Process extending requirements and merge them into their respective extended
        // requirements.
        for config in configs {
            for requirement in config.get_requirement_list() {
                if requirement.has_extends() {
                    let extendable_requirement = extendable.get(requirement.get_extends()).copied();
                    if !Self::validate_extending_requirement(
                        compiler,
                        requirement,
                        extendable_requirement.map(|b| &builders[b]),
                    ) {
                        continue;
                    }
                    let extendable_requirement = extendable_requirement.unwrap();
                    if config.has_library_level_non_allowlisted_conformance_violations_behavior() {
                        // overwrite the behavior of the extendable requirement with the behavior
                        // of the extending requirement.
                        root_requirements.insert(
                            extendable_requirement,
                            config
                                .get_library_level_non_allowlisted_conformance_violations_behavior(
                                ),
                        );
                    }
                    Self::merge_extending_requirement_into_extended(
                        &mut builders[extendable_requirement],
                        requirement,
                    );
                }
            }
        }

        // 3. Remove duplicates from the allowlists/whitelists of the merged root requirements.
        let mut cleaned_up_root_requirements = Vec::with_capacity(root_requirements.len());
        for (builder, behavior) in root_requirements {
            Self::remove_duplicates(&mut builders[builder]);
            let requirement = builders[builder].build();
            if is_library_level_conformance_reporting_mode && behavior != UNSPECIFIED {
                self.merged_behaviors.insert(requirement.clone(), behavior);
            }
            cleaned_up_root_requirements.push(requirement);
        }

        cleaned_up_root_requirements
    }

    /// Merges the allowlists/whitelists of the extending requirement into the extended
    /// requirement.
    // port: CheckConformance#mergeExtendingRequirementIntoExtended
    fn merge_extending_requirement_into_extended(
        extended_requirement: &mut Requirement,
        extending_requirement: &Requirement,
    ) {
        check_state!(
            extending_requirement.get_extends() == extended_requirement.get_rule_id(),
            "Attempting to merge an requirement with the wrong extended requirement"
        );
        extended_requirement
            .add_all_whitelist(extending_requirement.get_whitelist_list().iter().cloned())
            .add_all_whitelist_regexp(
                extending_requirement
                    .get_whitelist_regexp_list()
                    .iter()
                    .cloned(),
            )
            .add_all_allowlist(extending_requirement.get_allowlist_list().iter().cloned())
            .add_all_allowlist_regexp(
                extending_requirement
                    .get_allowlist_regexp_list()
                    .iter()
                    .cloned(),
            )
            .add_all_only_apply_to(
                extending_requirement
                    .get_only_apply_to_list()
                    .iter()
                    .cloned(),
            )
            .add_all_only_apply_to_regexp(
                extending_requirement
                    .get_only_apply_to_regexp_list()
                    .iter()
                    .cloned(),
            )
            .add_all_whitelist_entry(extending_requirement.get_whitelist_entry_list().to_vec())
            .add_all_allowlist_entry(extending_requirement.get_allowlist_entry_list().to_vec())
            .add_all_value(extending_requirement.get_value_list().iter().cloned())
            .add_all_config_file(extending_requirement.get_config_file_list().iter().cloned());
    }

    /// Validates a extendable requirement.
    ///
    /// A valid extendable requirement is a requirement that has a unique, non-empty rule_id.
    // port: CheckConformance#validateExtendableRequirement
    fn validate_extendable_requirement(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
        extendable: &IndexMap<String, usize>,
    ) -> bool {
        check_state!(
            requirement.has_rule_id(),
            "Extendable requirement must have a rule_id"
        );

        if requirement.get_rule_id().is_empty() {
            Self::report_invalid_requirement(compiler, requirement, "empty rule_id");
            return false;
        }
        if extendable.contains_key(requirement.get_rule_id()) {
            Self::report_invalid_requirement(
                compiler,
                requirement,
                &format!(
                    "two requirements with the same rule_id: {}",
                    requirement.get_rule_id()
                ),
            );
            return false;
        }
        true
    }

    /// Validates an extending requirement is correctly extending an extendable requirement.
    // port: CheckConformance#validateExtendingRequirement
    fn validate_extending_requirement(
        compiler: &mut AbstractCompiler,
        extending_requirement: &Requirement,
        extendable_requirement: Option<&Requirement>,
    ) -> bool {
        check_state!(
            extending_requirement.has_extends(),
            "Extending requirement must have an extends"
        );
        let Some(extendable_requirement) = extendable_requirement else {
            Self::report_invalid_requirement(
                compiler,
                extending_requirement,
                &format!(
                    "no extendable requirement with rule_id: {}",
                    extending_requirement.get_extends()
                ),
            );
            return false;
        };
        for field in extending_requirement.get_all_fields() {
            if !EXTENDABLE_FIELDS.contains(&field.get_name()) {
                // Java: "..." + EXTENDABLE_FIELDS (ImmutableSet#toString)
                Self::report_invalid_requirement(
                    compiler,
                    extending_requirement,
                    &format!(
                        "extending rules allow only [{}]",
                        EXTENDABLE_FIELDS.join(", ")
                    ),
                );
            }
        }
        if extending_requirement.get_value_count() > 0
            && !extendable_requirement.get_allow_extending_value()
        {
            Self::report_invalid_requirement(
                compiler,
                extending_requirement,
                "extending rule may not specify 'value' if extendable rule does not allow it",
            );
        }
        true
    }

    // port: CheckConformance#removeDuplicates
    fn remove_duplicates(requirement: &mut Requirement) {
        let list1: IndexSet<String> = requirement.get_whitelist_list().iter().cloned().collect();
        requirement.clear_whitelist().add_all_whitelist(list1);

        let allowlist: IndexSet<String> =
            requirement.get_allowlist_list().iter().cloned().collect();
        requirement.clear_allowlist().add_all_allowlist(allowlist);

        let list2: IndexSet<String> = requirement
            .get_whitelist_regexp_list()
            .iter()
            .cloned()
            .collect();
        requirement
            .clear_whitelist_regexp()
            .add_all_whitelist_regexp(list2);

        let allowlist_regexp: IndexSet<String> = requirement
            .get_allowlist_regexp_list()
            .iter()
            .cloned()
            .collect();
        requirement
            .clear_allowlist_regexp()
            .add_all_allowlist_regexp(allowlist_regexp);

        let list3: IndexSet<String> = requirement
            .get_only_apply_to_list()
            .iter()
            .cloned()
            .collect();
        requirement
            .clear_only_apply_to()
            .add_all_only_apply_to(list3);

        let list4: IndexSet<String> = requirement
            .get_only_apply_to_regexp_list()
            .iter()
            .cloned()
            .collect();
        requirement
            .clear_only_apply_to_regexp()
            .add_all_only_apply_to_regexp(list4);
    }

    /// Translates a requirement specified in the conformance config file into a rule instance.
    ///
    /// Returns the initialized rule, or None if the requirement is invalid.
    // port: CheckConformance#initRule
    fn init_rule(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
    ) -> Option<Box<dyn Rule>> {
        use conformance_rules as cr;
        let result: Result<Box<dyn Rule>, InvalidRequirementSpec> = (|| {
            Ok(match requirement.get_type() {
                Type::CUSTOM => Box::new(cr::CustomRuleProxy::new(compiler, requirement)?) as _,
                Type::NO_OP => Box::new(cr::NoOp::new(compiler, requirement)?) as _,
                Type::BANNED_CODE_PATTERN => {
                    Box::new(cr::BannedCodePattern::new(compiler, requirement)?) as _
                }
                Type::BANNED_DEPENDENCY => {
                    Box::new(cr::BannedDependency::new(compiler, requirement)?) as _
                }
                Type::BANNED_DEPENDENCY_REGEX => {
                    Box::new(cr::BannedDependencyRegex::new(compiler, requirement)?) as _
                }
                Type::BANNED_ENHANCE => {
                    Box::new(cr::BannedEnhance::new(compiler, requirement)?) as _
                }
                Type::BANNED_MODS_REGEX => {
                    Box::new(cr::BannedModsRegex::new(compiler, requirement)?) as _
                }
                Type::BANNED_NAME | Type::BANNED_NAME_CALL => {
                    Box::new(cr::BannedName::new(compiler, requirement)?) as _
                }
                Type::BANNED_PROPERTY
                | Type::BANNED_PROPERTY_READ
                | Type::BANNED_PROPERTY_WRITE
                | Type::BANNED_PROPERTY_NON_CONSTANT_WRITE
                | Type::BANNED_PROPERTY_CALL => {
                    Box::new(cr::BannedProperty::new(compiler, requirement)?) as _
                }
                Type::RESTRICTED_NAME_CALL => {
                    Box::new(cr::RestrictedNameCall::new(compiler, requirement)?) as _
                }
                Type::RESTRICTED_METHOD_CALL => {
                    Box::new(cr::RestrictedMethodCall::new(compiler, requirement)?) as _
                }
                Type::RESTRICTED_PROPERTY_WRITE => {
                    Box::new(cr::RestrictedPropertyWrite::new(compiler, requirement)?) as _
                }
                Type::BANNED_STRING_REGEX => {
                    Box::new(cr::BannedStringRegex::new(compiler, requirement)?) as _
                }
            })
        })();
        match result {
            Ok(rule) => Some(rule),
            Err(e) => {
                Self::report_invalid_requirement(compiler, requirement, e.get_message());
                None
            }
        }
    }

    // port: CheckConformance#reportInvalidRequirement
    fn report_invalid_requirement(
        compiler: &mut AbstractCompiler,
        requirement: &Requirement,
        reason: &str,
    ) {
        compiler.report(JSError::make_without_location(
            &INVALID_REQUIREMENT_SPEC,
            &[reason, &text_format::printer().print_to_string(requirement)],
        ));
    }
}

impl CompilerPass for CheckConformance {
    // port: CheckConformance#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        if !self.categories.is_empty() {
            NodeTraversal::traverse_roots(compiler, self, externs, root);
        }
    }
}

impl Callback for CheckConformance {
    // port: CheckConformance#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        // Don't inspect extern files, *.tsmes.closure.js, weak sources, or @closureUnaware ASTs.
        !n.is_script(t)
            || (Self::is_script_of_interest(
                t.get_input()
                    .expect("NodeTraversal#getInput")
                    .get_source_file(),
            ) && !t
                .get_source_name()
                .unwrap_or_default()
                .ends_with("tsmes.closure.js"))
    }

    // port: CheckConformance#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        // Use counted loops and backward iteration for performance (as Java).
        for c in (0..self.categories.len()).rev() {
            let precondition = self.categories[c].precondition;
            if precondition.should_check(t, n) {
                for r in (0..self.categories[c].rules.len()).rev() {
                    let rule = self.categories[c].rules[r];
                    let behavior = *check_not_null!(Some(
                        self.rule_to_behavior.get(&rule).unwrap_or(&UNSPECIFIED)
                    ));
                    self.rules[rule].check(t, n, behavior);
                }
            }
        }
    }
}

/// Port of `CheckConformance.InvalidRequirementSpec` (a checked exception).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidRequirementSpec {
    message: String,
}

impl InvalidRequirementSpec {
    // port: CheckConformance.InvalidRequirementSpec#InvalidRequirementSpec(String)
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    // port: CheckConformance.InvalidRequirementSpec#InvalidRequirementSpec(String,Throwable)
    // (the cause is not observable: only the message is reported)
    pub fn with_cause(message: impl Into<String>) -> Self {
        Self::new(message)
    }

    // port: Throwable#getMessage
    pub fn get_message(&self) -> &str {
        &self.message
    }
}
