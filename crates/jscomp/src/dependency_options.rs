/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/DependencyOptions.java.

use crate::{flag_usage_exception::FlagUsageException, module_identifier::ModuleIdentifier};
use closure_rhino::jscomp_base::check_state;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DependencyMode {
    NONE,
    SORT_ONLY,
    PRUNE_LEGACY,
    PRUNE,
    PRUNE_ALLOW_NO_ENTRY_POINTS,
}
impl DependencyMode {
    pub const VALUES: &'static [Self] = &[
        Self::NONE,
        Self::SORT_ONLY,
        Self::PRUNE_LEGACY,
        Self::PRUNE,
        Self::PRUNE_ALLOW_NO_ENTRY_POINTS,
    ];
    // port: DependencyOptions.DependencyMode#valueOf
    pub fn value_of(s: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == s)
    }
}
impl std::fmt::Display for DependencyMode {
    // port: DependencyOptions.DependencyMode#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DependencyOptions {
    mode: DependencyMode,
    entry_points: Vec<ModuleIdentifier>,
}
impl DependencyOptions {
    // port: DependencyOptions#DependencyOptions
    pub fn new(mode: DependencyMode, entry_points: Vec<ModuleIdentifier>) -> Self {
        Self { mode, entry_points }
    }
    // port: DependencyOptions#mode
    pub fn mode(&self) -> DependencyMode {
        self.mode
    }
    // port: DependencyOptions#entryPoints
    pub fn entry_points(&self) -> &[ModuleIdentifier] {
        &self.entry_points
    }
    // port: DependencyOptions#getMode
    pub fn get_mode(&self) -> DependencyMode {
        self.mode()
    }
    // port: DependencyOptions#getEntryPoints
    pub fn get_entry_points(&self) -> &[ModuleIdentifier] {
        self.entry_points()
    }
    // port: DependencyOptions#needsManagement
    pub fn needs_management(&self) -> bool {
        self.mode() != DependencyMode::NONE
    }
    // port: DependencyOptions#shouldSort
    pub fn should_sort(&self) -> bool {
        self.mode() != DependencyMode::NONE
    }
    // port: DependencyOptions#shouldPrune
    pub fn should_prune(&self) -> bool {
        matches!(
            self.mode(),
            DependencyMode::PRUNE_LEGACY
                | DependencyMode::PRUNE
                | DependencyMode::PRUNE_ALLOW_NO_ENTRY_POINTS
        )
    }
    // port: DependencyOptions#shouldDropMoochers
    pub fn should_drop_moochers(&self) -> bool {
        matches!(
            self.mode(),
            DependencyMode::PRUNE | DependencyMode::PRUNE_ALLOW_NO_ENTRY_POINTS
        )
    }
    // port: DependencyOptions#none
    pub fn none() -> Self {
        Self::new(DependencyMode::NONE, Vec::new())
    }
    // port: DependencyOptions#sortOnly
    pub fn sort_only() -> Self {
        Self::new(DependencyMode::SORT_ONLY, Vec::new())
    }
    // port: DependencyOptions#pruneLegacyForEntryPoints
    pub fn prune_legacy_for_entry_points(
        entry_points: impl IntoIterator<Item = ModuleIdentifier>,
    ) -> Self {
        Self::new(
            DependencyMode::PRUNE_LEGACY,
            entry_points.into_iter().collect(),
        )
    }
    // port: DependencyOptions#pruneAllowNoEntryPoints
    pub fn prune_allow_no_entry_points(
        entry_points: impl IntoIterator<Item = ModuleIdentifier>,
    ) -> Self {
        Self::new(
            DependencyMode::PRUNE_ALLOW_NO_ENTRY_POINTS,
            entry_points.into_iter().collect(),
        )
    }
    // port: DependencyOptions#pruneForEntryPoints
    pub fn prune_for_entry_points(
        entry_points: impl IntoIterator<Item = ModuleIdentifier>,
    ) -> Self {
        let entry_points: Vec<_> = entry_points.into_iter().collect();
        check_state!(
            !entry_points.is_empty(),
            "DependencyMode.PRUNE requires at least one entry point"
        );
        Self::new(DependencyMode::PRUNE, entry_points)
    }
    // port: DependencyOptions#fromFlags
    pub fn from_flags(
        dependency_mode_flag: Option<DependencyMode>,
        entry_point_flag: &[String],
        closure_entry_point_flag: &[String],
        common_js_entry_module_flag: Option<&str>,
        manage_closure_dependencies_flag: bool,
        only_closure_dependencies_flag: bool,
    ) -> Result<Option<Self>, FlagUsageException> {
        let has_entry_point = common_js_entry_module_flag.is_some()
            || !entry_point_flag.is_empty()
            || !closure_entry_point_flag.is_empty();
        if !has_entry_point && only_closure_dependencies_flag {
            return Err(FlagUsageException::new(
                "--only_closure_dependencies requires --entry_point.",
            ));
        }
        if !has_entry_point && dependency_mode_flag == Some(DependencyMode::PRUNE) {
            return Err(FlagUsageException::new(
                "--dependency_mode=PRUNE requires --entry_point.",
            ));
        }
        if has_entry_point
            && matches!(
                dependency_mode_flag,
                Some(DependencyMode::NONE | DependencyMode::SORT_ONLY)
            )
        {
            return Err(FlagUsageException::new(format!(
                "--dependency_mode={} cannot be used with --entry_point, --closure_entry_point or --common_js_entry_module.",
                dependency_mode_flag.unwrap()
            )));
        }
        if !entry_point_flag.is_empty() && !closure_entry_point_flag.is_empty() {
            return Err(FlagUsageException::new(
                "--closure_entry_point cannot be used with --entry_point.",
            ));
        }
        if common_js_entry_module_flag.is_some()
            && (!entry_point_flag.is_empty() || !closure_entry_point_flag.is_empty())
        {
            return Err(FlagUsageException::new(
                "--common_js_entry_module cannot be used with either --entry_point or --closure_entry_point.",
            ));
        }
        if manage_closure_dependencies_flag && only_closure_dependencies_flag {
            return Err(FlagUsageException::new(
                "--only_closure_dependencies cannot be used with --manage_closure_dependencies.",
            ));
        }
        if manage_closure_dependencies_flag && dependency_mode_flag.is_some() {
            return Err(FlagUsageException::new(
                "--manage_closure_dependencies cannot be used with --dependency_mode.",
            ));
        }
        if only_closure_dependencies_flag && dependency_mode_flag.is_some() {
            return Err(FlagUsageException::new(
                "--only_closure_dependencies cannot be used with --dependency_mode.",
            ));
        }
        let dependency_mode = if dependency_mode_flag == Some(DependencyMode::PRUNE)
            || only_closure_dependencies_flag
        {
            DependencyMode::PRUNE
        } else if dependency_mode_flag == Some(DependencyMode::PRUNE_ALLOW_NO_ENTRY_POINTS) {
            DependencyMode::PRUNE_ALLOW_NO_ENTRY_POINTS
        } else if dependency_mode_flag == Some(DependencyMode::PRUNE_LEGACY)
            || manage_closure_dependencies_flag
            || has_entry_point
        {
            DependencyMode::PRUNE_LEGACY
        } else if let Some(mode) = dependency_mode_flag {
            mode
        } else {
            return Ok(None);
        };
        let mut entry_points_builder = Vec::new();
        if let Some(module) = common_js_entry_module_flag {
            entry_points_builder.push(ModuleIdentifier::for_file(module));
        }
        for entry_point in entry_point_flag {
            entry_points_builder.push(ModuleIdentifier::for_flag_value(entry_point));
        }
        for closure_entry_point in closure_entry_point_flag {
            entry_points_builder.push(ModuleIdentifier::for_closure(closure_entry_point));
        }
        Ok(Some(match dependency_mode {
            DependencyMode::NONE => Self::none(),
            DependencyMode::SORT_ONLY => Self::sort_only(),
            DependencyMode::PRUNE_LEGACY => {
                Self::prune_legacy_for_entry_points(entry_points_builder)
            }
            DependencyMode::PRUNE_ALLOW_NO_ENTRY_POINTS => {
                Self::prune_allow_no_entry_points(entry_points_builder)
            }
            DependencyMode::PRUNE => Self::prune_for_entry_points(entry_points_builder),
        }))
    }
}
impl std::fmt::Display for DependencyOptions {
    // port: DependencyOptions#toString (record)
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DependencyOptions[mode={}, entryPoints=[", self.mode)?;
        for (i, entry) in self.entry_points.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{entry}")?;
        }
        f.write_str("]]")
    }
}
