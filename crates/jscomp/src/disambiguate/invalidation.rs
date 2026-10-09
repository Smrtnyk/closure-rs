/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/disambiguate/Invalidation.java.

//! Port of `disambiguate/Invalidation.java`.

use crate::diagnostic::logs_gson::LogsGsonObject;
use closure_sourcemap::gson::stream::json_writer::JsonWriter;

/// Describes one way in which a property became invalidated.
///
/// This information is only used diagnostically and doesn't affect the outcome of optimizations.
/// Since diagnostic information isn't usually rendered, this class is designed to defer work as
/// much as possible.
///
/// Java's subclass `WithReceiverType` is the `receiver_type` field (present exactly when Java
/// creates a `WithReceiverType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Invalidation {
    reason: Reason,
    receiver_type: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reason {
    /// Certain well-known properties like "prototype" are always invalidated.
    WELL_KNOWN_PROPERTY,
    /// Properties accessed on invalidating types (like Object) are invalidated.
    INVALIDATING_TYPE,
    /// Properties accessed on types that don't declare them are invalidated.
    UNDECLARED_ACCESS,
}

impl Reason {
    // port: Invalidation.Reason#name
    fn name(self) -> &'static str {
        match self {
            Reason::WELL_KNOWN_PROPERTY => "WELL_KNOWN_PROPERTY",
            Reason::INVALIDATING_TYPE => "INVALIDATING_TYPE",
            Reason::UNDECLARED_ACCESS => "UNDECLARED_ACCESS",
        }
    }
}

// port: Invalidation#WELL_KNOWN_PROPERTY
const WELL_KNOWN_PROPERTY: Invalidation = Invalidation::new(Reason::WELL_KNOWN_PROPERTY);

impl Invalidation {
    // port: Invalidation#wellKnownProperty
    pub fn well_known_property() -> Invalidation {
        WELL_KNOWN_PROPERTY
    }

    // port: Invalidation#invalidatingType
    pub fn invalidating_type(receiver_type: i32) -> Invalidation {
        Self::with_receiver_type(Reason::INVALIDATING_TYPE, receiver_type)
    }

    // port: Invalidation#undeclaredAccess
    pub fn undeclared_access(receiver_type: i32) -> Invalidation {
        Self::with_receiver_type(Reason::UNDECLARED_ACCESS, receiver_type)
    }

    // port: Invalidation#Invalidation
    const fn new(reason: Reason) -> Self {
        Self {
            reason,
            receiver_type: None,
        }
    }

    // port: Invalidation.WithReceiverType#WithReceiverType
    fn with_receiver_type(reason: Reason, receiver_type: i32) -> Self {
        Self {
            reason,
            receiver_type: Some(receiver_type),
        }
    }

    /// Rust-only: `reason`'s enum constant name and, for a `WithReceiverType`, `receiverType`
    /// (the unit-record replay's field dumps).
    pub fn replay_fields(self) -> (&'static str, Option<i32>) {
        (self.reason.name(), self.receiver_type)
    }

    /// Gson's reflective serialization (DisambiguateProperties' renaming_index log): the subclass
    /// field `receiverType` first, then the superclass field `reason`.
    pub(crate) fn write_gson(self, writer: &mut JsonWriter) {
        writer.begin_object();
        if let Some(receiver_type) = self.receiver_type {
            writer.name("receiverType");
            receiver_type.write(writer);
        }
        writer.name("reason");
        self.reason.name().write(writer);
        writer.end_object();
    }
}
