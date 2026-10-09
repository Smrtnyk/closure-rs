/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/J2clSuppressWarningsGuard.java.

//! Port of `J2clSuppressWarningsGuard.java`.
use crate::{
    check_level::CheckLevel,
    diagnostic_group::DiagnosticGroup,
    diagnostic_groups::{self as groups, DiagnosticGroups},
    j2cl_source_utils::J2clSourceUtils,
    js_error::JSError,
    warnings_guard::{Priority, WarningsGuard},
};
use std::{any::Any, fmt, sync::LazyLock};

// TODO(b/128554878): Cleanup and document all file level suppressions for J2CL generated code.
// port: J2clSuppressWarningsGuard#DEFAULT_J2CL_SUPRRESIONS
static DEFAULT_J2CL_SUPRRESIONS: LazyLock<DiagnosticGroup> = LazyLock::new(|| {
    DiagnosticGroup::new_named_from_groups(
        "j2clIncomaptible",
        &[
            // Do not warn when static overrides do not match. This is safe, because J2CL
            // generated code directly points to declaration and this is also required with
            // collapse properties pass, which does not support dynamic dispatch for static
            // methods.
            groups::CHECK_STATIC_OVERRIDES.clone(),
            // Do not warn on valid Java constructs like "if(false) {...}" and other situations
            // that may arise from transormation of complex Kotlin constructs.
            groups::CHECK_USELESS_CODE.clone(),
            groups::CONST.clone(),
            groups::EXTRA_REQUIRE.clone(),
            // Kotlin allows provably invalid casts so long as it's via a safe cast. This causes
            // J2CL to generate code of the form:
            //   Foo.$isInstance(value) ? /**@type {!Foo}*/ (value) : null
            // However, if value is obviously never a instance of Foo then this will cause invalid
            // cast error. At runtime it's guarded so it would be safe regardless.
            // This also suppresses casts from non-lambda JsFunction implementations to functions.
            // This particular feature is deprecated and will be removed (b/159954752).
            groups::INVALID_CASTS.clone(),
            groups::LATE_PROVIDE.clone(),
            groups::MISSING_OVERRIDE.clone(),
            groups::MISSING_REQUIRE.clone(),
            groups::STRICT_MODULE_DEP_CHECK.clone(),
            groups::SUSPICIOUS_CODE.clone(),
            groups::UNUSED_LOCAL_VARIABLE.clone(),
            // TODO(b/78521031): J2CL targets are not strict missing property compatible.
            groups::STRICT_MISSING_PROPERTIES.clone(),
            DiagnosticGroups::for_name("transitionalSuspiciousCodeWarnings").unwrap(),
        ],
    )
});

/// A warnings guard that suppresses some warnings incompatible with J2CL.
#[derive(Debug, Default)]
pub struct J2clSuppressWarningsGuard;

impl J2clSuppressWarningsGuard {
    pub fn new() -> Self {
        Self
    }
}

impl WarningsGuard for J2clSuppressWarningsGuard {
    // port: J2clSuppressWarningsGuard#level
    fn level(&self, error: &JSError) -> Option<CheckLevel> {
        if !J2clSourceUtils::is_j2cl_source(error.source_name()) {
            return None;
        }

        // Do not warn about deprecated forwardDeclare usages in J2CL generated code as this
        // particular use-case is still valid, but not elsewhere.
        if groups::DEPRECATED.matches(error) && error.description().contains("forwardDeclare") {
            return Some(CheckLevel::OFF);
        }

        if DEFAULT_J2CL_SUPRRESIONS.matches(error) {
            Some(CheckLevel::OFF)
        } else {
            None
        }
    }

    // port: J2clSuppressWarningsGuard#getPriority
    fn get_priority(&self) -> i32 {
        Priority::MAX.get_value()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl fmt::Display for J2clSuppressWarningsGuard {
    // port: Object#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "com.google.javascript.jscomp.J2clSuppressWarningsGuard@{:x}",
            self as *const Self as usize
        )
    }
}
