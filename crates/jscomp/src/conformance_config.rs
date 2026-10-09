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

// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/conformance/conformance.proto.

//! Port of the protobuf messages generated from `jscomp/conformance/conformance.proto`
//! (`java_package = "com.google.javascript.jscomp"`, `java_multiple_files = true`):
//! `ConformanceConfig`, `Requirement`, `RequirementScopeEntry` and `RequirementScope`.
//!
//! The generated Java classes are proto2 messages: singular fields carry a has-bit (`Option`
//! here), repeated fields are lists, and getters return the declared default when unset. The
//! descriptors (`FieldDescriptor`) drive `TextFormat` parsing and printing
//! (`crate::protobuf::text_format`). Fields are listed in field-number order, the order
//! `Message#getAllFields` and `TextFormat.printer()` use.

use crate::protobuf::text_format::{
    FieldDescriptor, FieldKind, FieldValue, Message, ParseException,
};

/// An enum value descriptor: Java's `EnumValueDescriptor` name and number.
pub type EnumValues = &'static [(&'static str, i32)];

macro_rules! proto_enum {
    ($(#[$m:meta])* $name:ident, $full:literal, [$($v:ident = $n:literal),+ $(,)?]) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $($v),+
        }
        impl $name {
            /// The enum's full proto name (`EnumDescriptor#getFullName`).
            pub const FULL_NAME: &'static str = $full;
            /// Every value in declaration order (`EnumDescriptor#getValues`).
            pub const VALUES: EnumValues = &[$((stringify!($v), $n)),+];
            // port: ProtocolMessageEnum#getNumber
            pub fn get_number(self) -> i32 {
                match self {
                    $(Self::$v => $n),+
                }
            }
            // port: <generated enum>#forNumber
            pub fn for_number(value: i32) -> Option<Self> {
                match value {
                    $($n => Some(Self::$v),)+
                    _ => None,
                }
            }
            // port: Enum#name
            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$v => stringify!($v)),+
                }
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.name())
            }
        }
    };
}

proto_enum!(
    /// Port of `ConformanceConfig.LibraryLevelNonAllowlistedConformanceViolationsBehavior`.
    LibraryLevelNonAllowlistedConformanceViolationsBehavior,
    "jscomp.ConformanceConfig.LibraryLevelNonAllowlistedConformanceViolationsBehavior",
    [UNSPECIFIED = 0, REPORT_AS_BUILD_ERROR = 1, RECORD_ONLY = 2]
);

proto_enum!(
    /// Port of `Requirement.Type`.
    Type,
    "jscomp.Requirement.Type",
    [
        CUSTOM = 1,
        NO_OP = 15,
        BANNED_DEPENDENCY = 2,
        BANNED_DEPENDENCY_REGEX = 14,
        BANNED_ENHANCE = 16,
        BANNED_NAME = 3,
        BANNED_PROPERTY = 4,
        BANNED_PROPERTY_READ = 5,
        BANNED_PROPERTY_WRITE = 6,
        RESTRICTED_NAME_CALL = 7,
        RESTRICTED_METHOD_CALL = 8,
        BANNED_CODE_PATTERN = 9,
        BANNED_PROPERTY_CALL = 10,
        BANNED_PROPERTY_NON_CONSTANT_WRITE = 11,
        BANNED_NAME_CALL = 12,
        RESTRICTED_PROPERTY_WRITE = 13,
        BANNED_STRING_REGEX = 17,
        BANNED_MODS_REGEX = 18,
    ]
);

proto_enum!(
    /// Port of `Requirement.TypeMatchingStrategy`.
    TypeMatchingStrategy,
    "jscomp.Requirement.TypeMatchingStrategy",
    [UNKNOWN = 0, LOOSE = 1, STRICT_NULLABILITY = 2, SUBTYPES = 3, EXACT = 4]
);

proto_enum!(
    /// Port of `Requirement.Severity`.
    Severity,
    "jscomp.Requirement.Severity",
    [UNSPECIFIED = 0, WARNING = 1, ERROR = 2]
);

proto_enum!(
    /// Port of `RequirementScopeEntry.Reason`.
    Reason,
    "jscomp.RequirementScopeEntry.Reason",
    [UNSPECIFIED = 0, LEGACY = 1, OUT_OF_SCOPE = 2, MANUALLY_REVIEWED = 3]
);

/// Java's `Requirement.Type` etc. are nested in the message class; the Rust module keeps them at
/// module level and exposes the Java paths as aliases.
pub mod requirement {
    pub use super::{Severity, Type, TypeMatchingStrategy};
    /// `Requirement.Builder`.
    pub type Builder = super::Requirement;
}
pub mod requirement_scope_entry {
    pub use super::Reason;
    /// `RequirementScopeEntry.Builder`.
    pub type Builder = super::RequirementScopeEntry;
}

fn enum_or_default<E: Copy>(v: Option<i32>, for_number: fn(i32) -> Option<E>, default: E) -> E {
    v.and_then(for_number).unwrap_or(default)
}

macro_rules! repeated_string_accessors {
    ($field:ident, $get_list:ident, $get_count:ident, $get:ident, $add:ident, $add_all:ident, $clear:ident) => {
        // port: <generated>#get<Field>List
        pub fn $get_list(&self) -> &[String] {
            &self.$field
        }
        // port: <generated>#get<Field>Count
        pub fn $get_count(&self) -> i32 {
            self.$field.len() as i32
        }
        // port: <generated>#get<Field>(int)
        pub fn $get(&self, index: usize) -> &str {
            &self.$field[index]
        }
        // port: <generated>.Builder#add<Field>
        pub fn $add(&mut self, value: impl Into<String>) -> &mut Self {
            self.$field.push(value.into());
            self
        }
        // port: <generated>.Builder#addAll<Field>
        pub fn $add_all<I, S>(&mut self, values: I) -> &mut Self
        where
            I: IntoIterator<Item = S>,
            S: Into<String>,
        {
            self.$field.extend(values.into_iter().map(Into::into));
            self
        }
        // port: <generated>.Builder#clear<Field>
        pub fn $clear(&mut self) -> &mut Self {
            self.$field.clear();
            self
        }
    };
}

macro_rules! optional_string_accessors {
    ($field:ident, $has:ident, $get:ident, $set:ident, $clear:ident) => {
        // port: <generated>#has<Field>
        pub fn $has(&self) -> bool {
            self.$field.is_some()
        }
        // port: <generated>#get<Field>
        pub fn $get(&self) -> &str {
            self.$field.as_deref().unwrap_or("")
        }
        // port: <generated>.Builder#set<Field>
        pub fn $set(&mut self, value: impl Into<String>) -> &mut Self {
            self.$field = Some(value.into());
            self
        }
        // port: <generated>.Builder#clear<Field>
        pub fn $clear(&mut self) -> &mut Self {
            self.$field = None;
            self
        }
    };
}

// ---------------------------------------------------------------------------------------------
// ConformanceConfig

/// Port of the generated message `ConformanceConfig` (also its `Builder`: Java's
/// `toBuilder()`/`build()` are clones).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ConformanceConfig {
    pub requirement: Vec<Requirement>,
    pub library_level_non_allowlisted_conformance_violations_behavior: Option<i32>,
}

static CONFORMANCE_CONFIG_FIELDS: &[FieldDescriptor] = &[
    FieldDescriptor::new("requirement", 1, FieldKind::Message, true),
    FieldDescriptor::new(
        "library_level_non_allowlisted_conformance_violations_behavior",
        2,
        FieldKind::Enum(
            LibraryLevelNonAllowlistedConformanceViolationsBehavior::FULL_NAME,
            LibraryLevelNonAllowlistedConformanceViolationsBehavior::VALUES,
        ),
        false,
    ),
];

impl ConformanceConfig {
    // port: ConformanceConfig#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: ConformanceConfig#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    // port: ConformanceConfig#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: ConformanceConfig.Builder#build
    pub fn build(&self) -> Self {
        self.clone()
    }
    // port: ConformanceConfig#getRequirementList
    pub fn get_requirement_list(&self) -> &[Requirement] {
        &self.requirement
    }
    // port: ConformanceConfig#getRequirementCount
    pub fn get_requirement_count(&self) -> i32 {
        self.requirement.len() as i32
    }
    // port: ConformanceConfig#getRequirement
    pub fn get_requirement(&self, index: usize) -> &Requirement {
        &self.requirement[index]
    }
    // port: ConformanceConfig.Builder#addRequirement
    pub fn add_requirement(&mut self, value: Requirement) -> &mut Self {
        self.requirement.push(value);
        self
    }
    // port: ConformanceConfig.Builder#addAllRequirement
    pub fn add_all_requirement(
        &mut self,
        values: impl IntoIterator<Item = Requirement>,
    ) -> &mut Self {
        self.requirement.extend(values);
        self
    }
    // port: ConformanceConfig.Builder#clearRequirement
    pub fn clear_requirement(&mut self) -> &mut Self {
        self.requirement.clear();
        self
    }
    // port: ConformanceConfig#hasLibraryLevelNonAllowlistedConformanceViolationsBehavior
    pub fn has_library_level_non_allowlisted_conformance_violations_behavior(&self) -> bool {
        self.library_level_non_allowlisted_conformance_violations_behavior
            .is_some()
    }
    // port: ConformanceConfig#getLibraryLevelNonAllowlistedConformanceViolationsBehavior
    pub fn get_library_level_non_allowlisted_conformance_violations_behavior(
        &self,
    ) -> LibraryLevelNonAllowlistedConformanceViolationsBehavior {
        enum_or_default(
            self.library_level_non_allowlisted_conformance_violations_behavior,
            LibraryLevelNonAllowlistedConformanceViolationsBehavior::for_number,
            LibraryLevelNonAllowlistedConformanceViolationsBehavior::UNSPECIFIED,
        )
    }
    // port: ConformanceConfig.Builder#setLibraryLevelNonAllowlistedConformanceViolationsBehavior
    pub fn set_library_level_non_allowlisted_conformance_violations_behavior(
        &mut self,
        value: LibraryLevelNonAllowlistedConformanceViolationsBehavior,
    ) -> &mut Self {
        self.library_level_non_allowlisted_conformance_violations_behavior =
            Some(value.get_number());
        self
    }
}

impl Message for ConformanceConfig {
    fn full_name(&self) -> &'static str {
        "jscomp.ConformanceConfig"
    }
    fn fields(&self) -> &'static [FieldDescriptor] {
        CONFORMANCE_CONFIG_FIELDS
    }
    fn get_field(&self, number: i32) -> Vec<FieldValue<'_>> {
        match number {
            1 => self
                .requirement
                .iter()
                .map(|m| FieldValue::Message(m))
                .collect(),
            2 => self
                .library_level_non_allowlisted_conformance_violations_behavior
                .map(FieldValue::Enum)
                .into_iter()
                .collect(),
            _ => unreachable!(),
        }
    }
    fn set_enum(&mut self, number: i32, value: i32) {
        match number {
            2 => self.library_level_non_allowlisted_conformance_violations_behavior = Some(value),
            _ => unreachable!(),
        }
    }
    fn merge_message_field(
        &mut self,
        number: i32,
        fill: &mut dyn FnMut(&mut dyn Message) -> Result<(), ParseException>,
    ) -> Result<(), ParseException> {
        match number {
            1 => {
                let mut sub_field = Requirement::default();
                fill(&mut sub_field)?;
                self.requirement.push(sub_field);
            }
            _ => unreachable!(),
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// Requirement

/// Port of the generated message `Requirement` (also its `Builder`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Requirement {
    pub error_message: Option<String>,
    pub whitelist: Vec<String>,
    pub whitelist_regexp: Vec<String>,
    pub allowlist: Vec<String>,
    pub allowlist_regexp: Vec<String>,
    pub whitelist_entry: Vec<RequirementScopeEntry>,
    pub allowlist_entry: Vec<RequirementScopeEntry>,
    pub only_apply_to: Vec<String>,
    pub only_apply_to_regexp: Vec<String>,
    pub r#type: Option<i32>,
    pub value: Vec<String>,
    pub allow_extending_value: Option<bool>,
    pub type_matching_strategy: Option<i32>,
    pub java_class: Option<String>,
    pub rule_id: Option<String>,
    pub extends: Option<String>,
    pub report_loose_type_violations: Option<bool>,
    pub severity: Option<i32>,
    pub config_file: Vec<String>,
}

static REQUIREMENT_FIELDS: &[FieldDescriptor] = &[
    FieldDescriptor::new("error_message", 1, FieldKind::String, false),
    FieldDescriptor::new("whitelist", 2, FieldKind::String, true),
    FieldDescriptor::new("whitelist_regexp", 3, FieldKind::String, true),
    FieldDescriptor::new("only_apply_to", 4, FieldKind::String, true),
    FieldDescriptor::new("only_apply_to_regexp", 5, FieldKind::String, true),
    FieldDescriptor::new(
        "type",
        6,
        FieldKind::Enum(Type::FULL_NAME, Type::VALUES),
        false,
    ),
    FieldDescriptor::new("value", 7, FieldKind::String, true),
    FieldDescriptor::new("java_class", 8, FieldKind::String, false),
    FieldDescriptor::new("rule_id", 9, FieldKind::String, false),
    FieldDescriptor::new("extends", 10, FieldKind::String, false),
    FieldDescriptor::new("report_loose_type_violations", 11, FieldKind::Bool, false),
    FieldDescriptor::new(
        "severity",
        12,
        FieldKind::Enum(Severity::FULL_NAME, Severity::VALUES),
        false,
    ),
    FieldDescriptor::new(
        "type_matching_strategy",
        13,
        FieldKind::Enum(
            TypeMatchingStrategy::FULL_NAME,
            TypeMatchingStrategy::VALUES,
        ),
        false,
    ),
    FieldDescriptor::new("whitelist_entry", 14, FieldKind::Message, true),
    FieldDescriptor::new("allow_extending_value", 15, FieldKind::Bool, false),
    FieldDescriptor::new("config_file", 16, FieldKind::String, true),
    FieldDescriptor::new("allowlist", 17, FieldKind::String, true),
    FieldDescriptor::new("allowlist_regexp", 18, FieldKind::String, true),
    FieldDescriptor::new("allowlist_entry", 19, FieldKind::Message, true),
];

impl Requirement {
    // port: Requirement#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: Requirement#getDefaultInstance
    pub fn get_default_instance() -> Self {
        Self::default()
    }
    // port: Requirement#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: Requirement.Builder#build
    pub fn build(&self) -> Self {
        self.clone()
    }

    optional_string_accessors!(
        error_message,
        has_error_message,
        get_error_message,
        set_error_message,
        clear_error_message
    );
    repeated_string_accessors!(
        whitelist,
        get_whitelist_list,
        get_whitelist_count,
        get_whitelist,
        add_whitelist,
        add_all_whitelist,
        clear_whitelist
    );
    repeated_string_accessors!(
        whitelist_regexp,
        get_whitelist_regexp_list,
        get_whitelist_regexp_count,
        get_whitelist_regexp,
        add_whitelist_regexp,
        add_all_whitelist_regexp,
        clear_whitelist_regexp
    );
    repeated_string_accessors!(
        allowlist,
        get_allowlist_list,
        get_allowlist_count,
        get_allowlist,
        add_allowlist,
        add_all_allowlist,
        clear_allowlist
    );
    repeated_string_accessors!(
        allowlist_regexp,
        get_allowlist_regexp_list,
        get_allowlist_regexp_count,
        get_allowlist_regexp,
        add_allowlist_regexp,
        add_all_allowlist_regexp,
        clear_allowlist_regexp
    );
    repeated_string_accessors!(
        only_apply_to,
        get_only_apply_to_list,
        get_only_apply_to_count,
        get_only_apply_to,
        add_only_apply_to,
        add_all_only_apply_to,
        clear_only_apply_to
    );
    repeated_string_accessors!(
        only_apply_to_regexp,
        get_only_apply_to_regexp_list,
        get_only_apply_to_regexp_count,
        get_only_apply_to_regexp,
        add_only_apply_to_regexp,
        add_all_only_apply_to_regexp,
        clear_only_apply_to_regexp
    );
    repeated_string_accessors!(
        value,
        get_value_list,
        get_value_count,
        get_value,
        add_value,
        add_all_value,
        clear_value
    );
    repeated_string_accessors!(
        config_file,
        get_config_file_list,
        get_config_file_count,
        get_config_file,
        add_config_file,
        add_all_config_file,
        clear_config_file
    );
    optional_string_accessors!(
        java_class,
        has_java_class,
        get_java_class,
        set_java_class,
        clear_java_class
    );
    optional_string_accessors!(
        rule_id,
        has_rule_id,
        get_rule_id,
        set_rule_id,
        clear_rule_id
    );
    optional_string_accessors!(
        extends,
        has_extends,
        get_extends,
        set_extends,
        clear_extends
    );

    // port: Requirement#getWhitelistEntryList
    pub fn get_whitelist_entry_list(&self) -> &[RequirementScopeEntry] {
        &self.whitelist_entry
    }
    // port: Requirement.Builder#addWhitelistEntry
    pub fn add_whitelist_entry(&mut self, value: RequirementScopeEntry) -> &mut Self {
        self.whitelist_entry.push(value);
        self
    }
    // port: Requirement.Builder#addAllWhitelistEntry
    pub fn add_all_whitelist_entry(
        &mut self,
        values: impl IntoIterator<Item = RequirementScopeEntry>,
    ) -> &mut Self {
        self.whitelist_entry.extend(values);
        self
    }
    // port: Requirement.Builder#clearWhitelistEntry
    pub fn clear_whitelist_entry(&mut self) -> &mut Self {
        self.whitelist_entry.clear();
        self
    }
    // port: Requirement#getAllowlistEntryList
    pub fn get_allowlist_entry_list(&self) -> &[RequirementScopeEntry] {
        &self.allowlist_entry
    }
    // port: Requirement.Builder#addAllowlistEntry
    pub fn add_allowlist_entry(&mut self, value: RequirementScopeEntry) -> &mut Self {
        self.allowlist_entry.push(value);
        self
    }
    // port: Requirement.Builder#addAllAllowlistEntry
    pub fn add_all_allowlist_entry(
        &mut self,
        values: impl IntoIterator<Item = RequirementScopeEntry>,
    ) -> &mut Self {
        self.allowlist_entry.extend(values);
        self
    }
    // port: Requirement.Builder#clearAllowlistEntry
    pub fn clear_allowlist_entry(&mut self) -> &mut Self {
        self.allowlist_entry.clear();
        self
    }

    // port: Requirement#hasType
    pub fn has_type(&self) -> bool {
        self.r#type.is_some()
    }
    // port: Requirement#getType
    pub fn get_type(&self) -> Type {
        enum_or_default(self.r#type, Type::for_number, Type::CUSTOM)
    }
    // port: Requirement.Builder#setType
    pub fn set_type(&mut self, value: Type) -> &mut Self {
        self.r#type = Some(value.get_number());
        self
    }
    // port: Requirement#hasAllowExtendingValue
    pub fn has_allow_extending_value(&self) -> bool {
        self.allow_extending_value.is_some()
    }
    // port: Requirement#getAllowExtendingValue
    pub fn get_allow_extending_value(&self) -> bool {
        self.allow_extending_value.unwrap_or(false)
    }
    // port: Requirement.Builder#setAllowExtendingValue
    pub fn set_allow_extending_value(&mut self, value: bool) -> &mut Self {
        self.allow_extending_value = Some(value);
        self
    }
    // port: Requirement#hasTypeMatchingStrategy
    pub fn has_type_matching_strategy(&self) -> bool {
        self.type_matching_strategy.is_some()
    }
    // port: Requirement#getTypeMatchingStrategy
    pub fn get_type_matching_strategy(&self) -> TypeMatchingStrategy {
        enum_or_default(
            self.type_matching_strategy,
            TypeMatchingStrategy::for_number,
            TypeMatchingStrategy::LOOSE,
        )
    }
    // port: Requirement.Builder#setTypeMatchingStrategy
    pub fn set_type_matching_strategy(&mut self, value: TypeMatchingStrategy) -> &mut Self {
        self.type_matching_strategy = Some(value.get_number());
        self
    }
    // port: Requirement#hasReportLooseTypeViolations
    pub fn has_report_loose_type_violations(&self) -> bool {
        self.report_loose_type_violations.is_some()
    }
    // port: Requirement#getReportLooseTypeViolations
    pub fn get_report_loose_type_violations(&self) -> bool {
        self.report_loose_type_violations.unwrap_or(true)
    }
    // port: Requirement.Builder#setReportLooseTypeViolations
    pub fn set_report_loose_type_violations(&mut self, value: bool) -> &mut Self {
        self.report_loose_type_violations = Some(value);
        self
    }
    // port: Requirement#hasSeverity
    pub fn has_severity(&self) -> bool {
        self.severity.is_some()
    }
    // port: Requirement#getSeverity
    pub fn get_severity(&self) -> Severity {
        enum_or_default(self.severity, Severity::for_number, Severity::WARNING)
    }
    // port: Requirement.Builder#setSeverity
    pub fn set_severity(&mut self, value: Severity) -> &mut Self {
        self.severity = Some(value.get_number());
        self
    }

    /// The names of the fields `Message#getAllFields` returns (set singular fields and non-empty
    /// repeated fields, in field-number order).
    // port: AbstractMessage#getAllFields
    pub fn get_all_fields(&self) -> Vec<&'static FieldDescriptor> {
        crate::protobuf::text_format::get_all_fields(self)
    }
}

impl Message for Requirement {
    fn full_name(&self) -> &'static str {
        "jscomp.Requirement"
    }
    fn fields(&self) -> &'static [FieldDescriptor] {
        REQUIREMENT_FIELDS
    }
    fn get_field(&self, number: i32) -> Vec<FieldValue<'_>> {
        fn strs(v: &[String]) -> Vec<FieldValue<'_>> {
            v.iter().map(|s| FieldValue::String(s)).collect()
        }
        fn opt_str(v: &Option<String>) -> Vec<FieldValue<'_>> {
            v.iter().map(|s| FieldValue::String(s)).collect()
        }
        fn entries(v: &[RequirementScopeEntry]) -> Vec<FieldValue<'_>> {
            v.iter().map(|m| FieldValue::Message(m)).collect()
        }
        match number {
            1 => opt_str(&self.error_message),
            2 => strs(&self.whitelist),
            3 => strs(&self.whitelist_regexp),
            4 => strs(&self.only_apply_to),
            5 => strs(&self.only_apply_to_regexp),
            6 => self.r#type.map(FieldValue::Enum).into_iter().collect(),
            7 => strs(&self.value),
            8 => opt_str(&self.java_class),
            9 => opt_str(&self.rule_id),
            10 => opt_str(&self.extends),
            11 => self
                .report_loose_type_violations
                .map(FieldValue::Bool)
                .into_iter()
                .collect(),
            12 => self.severity.map(FieldValue::Enum).into_iter().collect(),
            13 => self
                .type_matching_strategy
                .map(FieldValue::Enum)
                .into_iter()
                .collect(),
            14 => entries(&self.whitelist_entry),
            15 => self
                .allow_extending_value
                .map(FieldValue::Bool)
                .into_iter()
                .collect(),
            16 => strs(&self.config_file),
            17 => strs(&self.allowlist),
            18 => strs(&self.allowlist_regexp),
            19 => entries(&self.allowlist_entry),
            _ => unreachable!(),
        }
    }
    fn set_string(&mut self, number: i32, value: String) {
        match number {
            1 => self.error_message = Some(value),
            2 => self.whitelist.push(value),
            3 => self.whitelist_regexp.push(value),
            4 => self.only_apply_to.push(value),
            5 => self.only_apply_to_regexp.push(value),
            7 => self.value.push(value),
            8 => self.java_class = Some(value),
            9 => self.rule_id = Some(value),
            10 => self.extends = Some(value),
            16 => self.config_file.push(value),
            17 => self.allowlist.push(value),
            18 => self.allowlist_regexp.push(value),
            _ => unreachable!(),
        }
    }
    fn set_bool(&mut self, number: i32, value: bool) {
        match number {
            11 => self.report_loose_type_violations = Some(value),
            15 => self.allow_extending_value = Some(value),
            _ => unreachable!(),
        }
    }
    fn set_enum(&mut self, number: i32, value: i32) {
        match number {
            6 => self.r#type = Some(value),
            12 => self.severity = Some(value),
            13 => self.type_matching_strategy = Some(value),
            _ => unreachable!(),
        }
    }
    fn merge_message_field(
        &mut self,
        number: i32,
        fill: &mut dyn FnMut(&mut dyn Message) -> Result<(), ParseException>,
    ) -> Result<(), ParseException> {
        let mut sub_field = RequirementScopeEntry::default();
        fill(&mut sub_field)?;
        match number {
            14 => self.whitelist_entry.push(sub_field),
            19 => self.allowlist_entry.push(sub_field),
            _ => unreachable!(),
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// RequirementScopeEntry

/// Port of the generated message `RequirementScopeEntry` (also its `Builder`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RequirementScopeEntry {
    pub reason: Option<i32>,
    pub prefix: Vec<String>,
    pub regexp: Vec<String>,
    pub explanation: Option<String>,
    pub comment: Vec<String>,
    pub automatically_prune: Option<bool>,
    pub do_not_record_violations_in_summary_because_this_is_gencode_and_always_safe: Option<bool>,
}

static REQUIREMENT_SCOPE_ENTRY_FIELDS: &[FieldDescriptor] = &[
    FieldDescriptor::new(
        "reason",
        1,
        FieldKind::Enum(Reason::FULL_NAME, Reason::VALUES),
        false,
    ),
    FieldDescriptor::new("prefix", 2, FieldKind::String, true),
    FieldDescriptor::new("regexp", 3, FieldKind::String, true),
    FieldDescriptor::new("explanation", 4, FieldKind::String, false),
    FieldDescriptor::new("comment", 5, FieldKind::String, true),
    FieldDescriptor::new("automatically_prune", 6, FieldKind::Bool, false),
    FieldDescriptor::new(
        "do_not_record_violations_in_summary_because_this_is_gencode_and_always_safe",
        7,
        FieldKind::Bool,
        false,
    ),
];

impl RequirementScopeEntry {
    // port: RequirementScopeEntry#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: RequirementScopeEntry#toBuilder
    pub fn to_builder(&self) -> Self {
        self.clone()
    }
    // port: RequirementScopeEntry.Builder#build
    pub fn build(&self) -> Self {
        self.clone()
    }
    // port: RequirementScopeEntry#hasReason
    pub fn has_reason(&self) -> bool {
        self.reason.is_some()
    }
    // port: RequirementScopeEntry#getReason
    pub fn get_reason(&self) -> Reason {
        enum_or_default(self.reason, Reason::for_number, Reason::UNSPECIFIED)
    }
    // port: RequirementScopeEntry.Builder#setReason
    pub fn set_reason(&mut self, value: Reason) -> &mut Self {
        self.reason = Some(value.get_number());
        self
    }
    repeated_string_accessors!(
        prefix,
        get_prefix_list,
        get_prefix_count,
        get_prefix,
        add_prefix,
        add_all_prefix,
        clear_prefix
    );
    repeated_string_accessors!(
        regexp,
        get_regexp_list,
        get_regexp_count,
        get_regexp,
        add_regexp,
        add_all_regexp,
        clear_regexp
    );
    optional_string_accessors!(
        explanation,
        has_explanation,
        get_explanation,
        set_explanation,
        clear_explanation
    );
    repeated_string_accessors!(
        comment,
        get_comment_list,
        get_comment_count,
        get_comment,
        add_comment,
        add_all_comment,
        clear_comment
    );
    // port: RequirementScopeEntry#getAutomaticallyPrune
    pub fn get_automatically_prune(&self) -> bool {
        self.automatically_prune.unwrap_or(false)
    }
    // port: RequirementScopeEntry.Builder#setAutomaticallyPrune
    pub fn set_automatically_prune(&mut self, value: bool) -> &mut Self {
        self.automatically_prune = Some(value);
        self
    }
}

impl Message for RequirementScopeEntry {
    fn full_name(&self) -> &'static str {
        "jscomp.RequirementScopeEntry"
    }
    fn fields(&self) -> &'static [FieldDescriptor] {
        REQUIREMENT_SCOPE_ENTRY_FIELDS
    }
    fn get_field(&self, number: i32) -> Vec<FieldValue<'_>> {
        fn strs(v: &[String]) -> Vec<FieldValue<'_>> {
            v.iter().map(|s| FieldValue::String(s)).collect()
        }
        match number {
            1 => self.reason.map(FieldValue::Enum).into_iter().collect(),
            2 => strs(&self.prefix),
            3 => strs(&self.regexp),
            4 => self
                .explanation
                .iter()
                .map(|s| FieldValue::String(s))
                .collect(),
            5 => strs(&self.comment),
            6 => self
                .automatically_prune
                .map(FieldValue::Bool)
                .into_iter()
                .collect(),
            7 => self
                .do_not_record_violations_in_summary_because_this_is_gencode_and_always_safe
                .map(FieldValue::Bool)
                .into_iter()
                .collect(),
            _ => unreachable!(),
        }
    }
    fn set_string(&mut self, number: i32, value: String) {
        match number {
            2 => self.prefix.push(value),
            3 => self.regexp.push(value),
            4 => self.explanation = Some(value),
            5 => self.comment.push(value),
            _ => unreachable!(),
        }
    }
    fn set_bool(&mut self, number: i32, value: bool) {
        match number {
            6 => self.automatically_prune = Some(value),
            7 => {
                self.do_not_record_violations_in_summary_because_this_is_gencode_and_always_safe =
                    Some(value)
            }
            _ => unreachable!(),
        }
    }
    fn set_enum(&mut self, number: i32, value: i32) {
        match number {
            1 => self.reason = Some(value),
            _ => unreachable!(),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// RequirementScope

/// Port of the generated message `RequirementScope` (also its `Builder`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RequirementScope {
    pub rule_id: Option<String>,
    pub allowlist: Vec<RequirementScopeEntry>,
    pub only_apply_to: Vec<RequirementScopeEntry>,
}

static REQUIREMENT_SCOPE_FIELDS: &[FieldDescriptor] = &[
    FieldDescriptor::new("rule_id", 1, FieldKind::String, false),
    FieldDescriptor::new("allowlist", 2, FieldKind::Message, true),
    FieldDescriptor::new("only_apply_to", 3, FieldKind::Message, true),
];

impl RequirementScope {
    // port: RequirementScope#newBuilder
    pub fn new_builder() -> Self {
        Self::default()
    }
    // port: RequirementScope.Builder#build
    pub fn build(&self) -> Self {
        self.clone()
    }
    optional_string_accessors!(
        rule_id,
        has_rule_id,
        get_rule_id,
        set_rule_id,
        clear_rule_id
    );
    // port: RequirementScope#getAllowlistList
    pub fn get_allowlist_list(&self) -> &[RequirementScopeEntry] {
        &self.allowlist
    }
    // port: RequirementScope#getOnlyApplyToList
    pub fn get_only_apply_to_list(&self) -> &[RequirementScopeEntry] {
        &self.only_apply_to
    }
}

impl Message for RequirementScope {
    fn full_name(&self) -> &'static str {
        "jscomp.RequirementScope"
    }
    fn fields(&self) -> &'static [FieldDescriptor] {
        REQUIREMENT_SCOPE_FIELDS
    }
    fn get_field(&self, number: i32) -> Vec<FieldValue<'_>> {
        match number {
            1 => self.rule_id.iter().map(|s| FieldValue::String(s)).collect(),
            2 => self
                .allowlist
                .iter()
                .map(|m| FieldValue::Message(m))
                .collect(),
            3 => self
                .only_apply_to
                .iter()
                .map(|m| FieldValue::Message(m))
                .collect(),
            _ => unreachable!(),
        }
    }
    fn set_string(&mut self, number: i32, value: String) {
        match number {
            1 => self.rule_id = Some(value),
            _ => unreachable!(),
        }
    }
    fn merge_message_field(
        &mut self,
        number: i32,
        fill: &mut dyn FnMut(&mut dyn Message) -> Result<(), ParseException>,
    ) -> Result<(), ParseException> {
        let mut sub_field = RequirementScopeEntry::default();
        fill(&mut sub_field)?;
        match number {
            2 => self.allowlist.push(sub_field),
            3 => self.only_apply_to.push(sub_field),
            _ => unreachable!(),
        }
        Ok(())
    }
}

/// `Message#toString()` of a generated message is `TextFormat.printer().printToString(this)`.
impl std::fmt::Display for Requirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::protobuf::text_format::printer().print_to_string(self))
    }
}
impl std::fmt::Display for ConformanceConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::protobuf::text_format::printer().print_to_string(self))
    }
}
impl std::fmt::Display for RequirementScopeEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&crate::protobuf::text_format::printer().print_to_string(self))
    }
}

// ---------------------------------------------------------------------------------------------
// Binary wire format (compiler_state.proto's ViolationProto embeds `Requirement` and
// `RequirementScopeEntry`). Like the other ported messages (`compiler_state_proto`), fields the
// message does not declare, and enum numbers it does not declare, are skipped.

use crate::serialization::protobuf::{
    self as wire, CodedInputStream, CodedOutputStream, ProtoResult, WireFormat,
};

/// The generated `writeTo` of a message: every set field, in field-number order (`fields()` is in
/// field-number order), each value with its tag.
// port: GeneratedMessage#writeTo (generated code: fields sorted by number)
fn write_fields(message: &dyn Message, output: &mut CodedOutputStream) {
    for field in message.fields() {
        let number = field.get_number() as u32;
        for value in message.get_field(field.get_number()) {
            match value {
                FieldValue::String(s) => {
                    output.write_tag(number, WireFormat::WIRETYPE_LENGTH_DELIMITED);
                    output.write_byte_array_no_tag(s.as_bytes());
                }
                FieldValue::Bool(b) => {
                    output.write_tag(number, WireFormat::WIRETYPE_VARINT);
                    output.write_bool_no_tag(b);
                }
                FieldValue::Enum(e) => {
                    output.write_tag(number, WireFormat::WIRETYPE_VARINT);
                    output.write_int32_no_tag(e);
                }
                FieldValue::Message(m) => {
                    output.write_tag(number, WireFormat::WIRETYPE_LENGTH_DELIMITED);
                    output.write_uint32_no_tag(serialized_size(m) as i32);
                    write_fields(m, output);
                }
            }
        }
    }
}

/// The generated `getSerializedSize` of a message.
// port: GeneratedMessage#getSerializedSize
fn serialized_size(message: &dyn Message) -> usize {
    let mut size = 0usize;
    for field in message.fields() {
        let tag_size = CodedOutputStream::compute_tag_size(field.get_number() as u32);
        for value in message.get_field(field.get_number()) {
            size += tag_size
                + match value {
                    FieldValue::String(s) => {
                        CodedOutputStream::compute_byte_array_size_no_tag(s.as_bytes())
                    }
                    FieldValue::Bool(_) => 1,
                    FieldValue::Enum(e) => CodedOutputStream::compute_int32_size_no_tag(e),
                    FieldValue::Message(m) => {
                        let inner = serialized_size(m);
                        CodedOutputStream::compute_uint32_size_no_tag(inner as i32) + inner
                    }
                };
        }
    }
    size
}

/// The generated parsing loop for the string, bool and enum fields of a message; `message_field`
/// reads the message-typed fields. Returns at the end tag (0) or an end-group tag.
// port: <generated message>.Builder#mergeFrom(CodedInputStream,ExtensionRegistryLite)
fn merge_fields(
    message: &mut dyn Message,
    input: &mut CodedInputStream<'_>,
    message_field: &mut dyn FnMut(&mut CodedInputStream<'_>, u32) -> ProtoResult<bool>,
) -> ProtoResult<()> {
    loop {
        let tag = input.read_tag()?;
        if tag == 0 {
            return Ok(());
        }
        let number = WireFormat::get_tag_field_number(tag) as i32;
        let wire_type = WireFormat::get_tag_wire_type(tag);
        let field = message.fields().iter().find(|f| f.get_number() == number);
        let handled = match field.map(FieldDescriptor::kind) {
            Some(FieldKind::String) if wire_type == WireFormat::WIRETYPE_LENGTH_DELIMITED => {
                // proto2 string fields keep the bytes; the getter decodes them as UTF-8
                // (ByteString#toStringUtf8 replaces malformed input).
                let bytes = input.read_bytes()?;
                message.set_string(number, String::from_utf8_lossy(&bytes).into_owned());
                true
            }
            Some(FieldKind::Bool) if wire_type == WireFormat::WIRETYPE_VARINT => {
                message.set_bool(number, input.read_bool()?);
                true
            }
            Some(FieldKind::Enum(_, values)) if wire_type == WireFormat::WIRETYPE_VARINT => {
                let raw = input.read_enum()?;
                if values.iter().any(|(_, n)| *n == raw) {
                    message.set_enum(number, raw);
                }
                true
            }
            Some(FieldKind::Message) if wire_type == WireFormat::WIRETYPE_LENGTH_DELIMITED => {
                message_field(input, number as u32)?
            }
            _ => false,
        };
        if !handled && !input.skip_field(tag)? {
            return Ok(());
        }
    }
}

impl wire::Message for RequirementScopeEntry {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        merge_fields(self, input, &mut |_, _| Ok(false))
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        write_fields(self, output);
    }
    fn get_serialized_size(&self) -> usize {
        serialized_size(self)
    }
}

impl wire::Message for Requirement {
    fn merge_from(&mut self, input: &mut CodedInputStream<'_>) -> ProtoResult<()> {
        let mut whitelist_entry = std::mem::take(&mut self.whitelist_entry);
        let mut allowlist_entry = std::mem::take(&mut self.allowlist_entry);
        let result = merge_fields(self, input, &mut |input, number| {
            let list = match number {
                14 => &mut whitelist_entry,
                19 => &mut allowlist_entry,
                _ => return Ok(false),
            };
            let mut entry = RequirementScopeEntry::default();
            input.read_message(&mut entry)?;
            list.push(entry);
            Ok(true)
        });
        self.whitelist_entry = whitelist_entry;
        self.allowlist_entry = allowlist_entry;
        result
    }
    fn write_to(&self, output: &mut CodedOutputStream) {
        write_fields(self, output);
    }
    fn get_serialized_size(&self) -> usize {
        serialized_size(self)
    }
}

static REQUIREMENT_DEFAULT_INSTANCE: std::sync::LazyLock<Requirement> =
    std::sync::LazyLock::new(Requirement::default);
static REQUIREMENT_SCOPE_ENTRY_DEFAULT_INSTANCE: std::sync::LazyLock<RequirementScopeEntry> =
    std::sync::LazyLock::new(RequirementScopeEntry::default);

impl Requirement {
    // port: Requirement#getDefaultInstance
    pub fn default_instance_ref() -> &'static Self {
        &REQUIREMENT_DEFAULT_INSTANCE
    }
}

impl RequirementScopeEntry {
    // port: RequirementScopeEntry#getDefaultInstance
    pub fn default_instance_ref() -> &'static Self {
        &REQUIREMENT_SCOPE_ENTRY_DEFAULT_INSTANCE
    }
}
