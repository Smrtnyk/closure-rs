/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/TypeMatchingStrategy.java.

//! Port of `TypeMatchingStrategy.java`: the different strategies for matching the `JSType` of
//! nodes.

use closure_jstype::{JSTypeRegistry, prelude::*};
use closure_rhino::{jstype::TypeId, node::Ast};

/// Port of the enum `TypeMatchingStrategy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TypeMatchingStrategy {
    /// Matches type or any subtype. Matches types with different nullability/voidability. Allows
    /// loose matches.
    LOOSE,
    /// Matches type or any subtype. Does not match types with different nullability/voidability.
    /// Allows loose matches.
    STRICT_NULLABILITY,
    /// Matches type or any subtype. Does not match types with different nullability/voidability.
    /// Does not allow loose matches.
    SUBTYPES,
    /// Does not match subtypes. Does not match types with different nullability/voidability. Does
    /// not allow loose matches.
    EXACT,
}

impl TypeMatchingStrategy {
    // port: TypeMatchingStrategy#TypeMatchingStrategy (allowSubtypes, ignoreNullability,
    // allowLooseMatches)
    const fn fields(self) -> (bool, bool, bool) {
        match self {
            TypeMatchingStrategy::LOOSE => (true, true, true),
            TypeMatchingStrategy::STRICT_NULLABILITY => (true, false, true),
            TypeMatchingStrategy::SUBTYPES => (true, false, false),
            TypeMatchingStrategy::EXACT => (false, false, false),
        }
    }

    // port: TypeMatchingStrategy#match
    pub fn r#match(
        self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        template_type: TypeId,
        type_: Option<TypeId>,
    ) -> MatchResult {
        let (allow_subtypes, ignore_nullability, allow_loose_matches) = self.fields();
        if template_type.is_unknown_type(reg, ast) {
            // If the template type is '?', then any type is a match and this is not considered a
            // loose match.
            return MatchResult::MATCH;
        }

        let mut type_ = match type_ {
            Some(t) if !t.is_unknown_type(reg, ast) && !t.is_all_type(reg) => t,
            _ => {
                return if allow_loose_matches {
                    MatchResult::LOOSE_MATCH
                } else {
                    MatchResult::NO_MATCH
                };
            }
        };

        if allow_subtypes {
            if ignore_nullability {
                type_ = type_.restrict_by_not_null_or_undefined(reg, ast);
            }
            if type_.is_subtype_of(reg, ast, template_type) {
                return MatchResult::MATCH;
            }
        }

        let nullable_mismatch = template_type.is_nullable(reg, ast) != type_.is_nullable(reg, ast);
        let voidable_mismatch = template_type.is_voidable(reg, ast) != type_.is_voidable(reg, ast);
        if !ignore_nullability && (nullable_mismatch || voidable_mismatch) {
            return MatchResult::NO_MATCH;
        }

        if type_.equals(reg, ast, template_type) {
            MatchResult::MATCH
        } else {
            MatchResult::NO_MATCH
        }
    }
}

/// The result of comparing two different `JSType` instances (Java
/// `TypeMatchingStrategy.MatchResult`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MatchResult {
    MATCH,
    NO_MATCH,
    LOOSE_MATCH,
}

impl MatchResult {
    // port: TypeMatchingStrategy.MatchResult#isMatch
    pub fn is_match(self) -> bool {
        self != MatchResult::NO_MATCH
    }

    // port: TypeMatchingStrategy.MatchResult#isLooseMatch
    pub fn is_loose_match(self) -> bool {
        self == MatchResult::LOOSE_MATCH
    }
}
