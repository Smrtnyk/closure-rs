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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/WarningLevel.java.

use crate::{
    check_level::CheckLevel, compiler_options::CompilerOptions, diagnostic_groups,
    show_by_path_warnings_guard::ShowByPathWarningsGuard,
};
use std::sync::Arc;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WarningLevel {
    QUIET,
    DEFAULT,
    VERBOSE,
}
impl std::fmt::Display for WarningLevel {
    // port: WarningLevel#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl WarningLevel {
    pub const VALUES: &'static [Self] = &[Self::QUIET, Self::DEFAULT, Self::VERBOSE];
    // port: WarningLevel#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
    // port: WarningLevel#setOptionsForWarningLevel
    pub fn set_options_for_warning_level(self, options: &mut CompilerOptions) {
        match self {
            Self::QUIET => Self::silence_all_warnings(options),
            Self::DEFAULT => Self::add_default_warnings(options),
            Self::VERBOSE => Self::add_verbose_warnings(options),
        }
    }
    // port: WarningLevel#silenceAllWarnings
    pub fn silence_all_warnings(options: &mut CompilerOptions) {
        options.add_warnings_guard(Arc::new(ShowByPathWarningsGuard::new(
            "the_longest_path_that_cannot_be_expressed_as_a_string",
        )));

        options.set_warning_level(diagnostic_groups::MISSING_PROVIDE.clone(), CheckLevel::OFF);
        options.set_check_types(false);
        options.set_warning_level(diagnostic_groups::CHECK_TYPES.clone(), CheckLevel::OFF);
        options.set_warning_level(
            diagnostic_groups::CHECK_USELESS_CODE.clone(),
            CheckLevel::OFF,
        );
        options.set_warning_level(diagnostic_groups::MISSING_RETURN.clone(), CheckLevel::OFF);
        options.set_warning_level(diagnostic_groups::ACCESS_CONTROLS.clone(), CheckLevel::OFF);
        options.set_warning_level(diagnostic_groups::CONST.clone(), CheckLevel::OFF);
        options.set_warning_level(
            diagnostic_groups::CONSTANT_PROPERTY.clone(),
            CheckLevel::OFF,
        );
        options.set_check_suspicious_code(false);
        options.set_warning_level(diagnostic_groups::GLOBAL_THIS.clone(), CheckLevel::OFF);
        options.set_warning_level(diagnostic_groups::GLOBAL_THIS.clone(), CheckLevel::OFF);
        options.set_warning_level(diagnostic_groups::ES5_STRICT.clone(), CheckLevel::OFF);

        options.set_warning_level(
            diagnostic_groups::NON_STANDARD_JSDOC.clone(),
            CheckLevel::OFF,
        );
    }
    // port: WarningLevel#addDefaultWarnings
    pub fn add_default_warnings(options: &mut CompilerOptions) {
        options.set_check_suspicious_code(true);

        options.set_warning_level(
            diagnostic_groups::NON_STANDARD_JSDOC.clone(),
            CheckLevel::OFF,
        );
    }
    // port: WarningLevel#addVerboseWarnings
    pub fn add_verbose_warnings(options: &mut CompilerOptions) {
        Self::add_default_warnings(options);

        options.set_check_suspicious_code(true);
        options.set_warning_level(diagnostic_groups::GLOBAL_THIS.clone(), CheckLevel::WARNING);
        options.set_check_symbols(true);

        options.set_check_types(true);
        options.set_warning_level(
            diagnostic_groups::MISSING_PROPERTIES.clone(),
            CheckLevel::WARNING,
        );
        options.set_warning_level(diagnostic_groups::DEPRECATED.clone(), CheckLevel::WARNING);
        options.set_warning_level(diagnostic_groups::ES5_STRICT.clone(), CheckLevel::WARNING);
        options.set_warning_level(diagnostic_groups::VISIBILITY.clone(), CheckLevel::WARNING);
        options.set_warning_level(diagnostic_groups::CONST.clone(), CheckLevel::WARNING);
        options.set_warning_level(diagnostic_groups::CHECK_REGEXP.clone(), CheckLevel::WARNING);
        options.set_warning_level(
            diagnostic_groups::STRICT_MODULE_DEP_CHECK.clone(),
            CheckLevel::WARNING,
        );
        options.set_warning_level(
            diagnostic_groups::MISSING_RETURN.clone(),
            CheckLevel::WARNING,
        );

        options.set_warning_level(
            diagnostic_groups::NON_STANDARD_JSDOC.clone(),
            CheckLevel::WARNING,
        );
    }
}
