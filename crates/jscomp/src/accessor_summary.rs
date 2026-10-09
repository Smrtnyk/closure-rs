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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/AccessorSummary.java.

//! Port of `AccessorSummary.java`: a summary of the properties that are defined by getters or
//! setters in a compilation.

use crate::compiler_state_proto::{
    AccessorSummaryEntryProto, AccessorSummaryProto, PropertyAccessKindProto,
};
use crate::serialization::protobuf::encode_utf8_java;
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::{check_state, js_string::JsString};

// port: AccessorSummary.PropertyAccessKind
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PropertyAccessKind {
    // To save space properties without getters or setters won't appear
    // in the maps at all, but NORMAL will be returned by some methods.
    NORMAL,
    GETTER_ONLY,
    SETTER_ONLY,
    GETTER_AND_SETTER,
}

impl PropertyAccessKind {
    // port: AccessorSummary.PropertyAccessKind#flags
    pub const fn flags(self) -> i8 {
        match self {
            Self::NORMAL => 0,
            Self::GETTER_ONLY => 1,
            Self::SETTER_ONLY => 2,
            Self::GETTER_AND_SETTER => 3,
        }
    }

    // port: AccessorSummary.PropertyAccessKind#hasGetter
    pub fn has_getter(self) -> bool {
        (self.flags() & 1) != 0
    }

    // port: AccessorSummary.PropertyAccessKind#hasSetter
    pub fn has_setter(self) -> bool {
        (self.flags() & 2) != 0
    }

    // port: AccessorSummary.PropertyAccessKind#hasGetterOrSetter
    pub fn has_getter_or_setter(self) -> bool {
        (self.flags() & 3) != 0
    }

    // used to combine information from externs and from sources
    // port: AccessorSummary.PropertyAccessKind#unionWith
    pub fn union_with(self, other: PropertyAccessKind) -> PropertyAccessKind {
        let combined_flags = self.flags() | other.flags();
        match combined_flags {
            0 => Self::NORMAL,
            1 => Self::GETTER_ONLY,
            2 => Self::SETTER_ONLY,
            3 => Self::GETTER_AND_SETTER,
            _ => panic!("unexpected value: {combined_flags}"),
        }
    }
}

impl std::fmt::Display for PropertyAccessKind {
    // port: AccessorSummary.PropertyAccessKind#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Records information about functions and classes that is used to inform optimizations.
///
/// The summary is immutable (Java `@Immutable`).
#[derive(Debug, PartialEq, Eq)]
pub struct AccessorSummary {
    assume_always_getter_and_setter: bool,
    accessors: IndexMap<JsString, PropertyAccessKind>,
}

impl AccessorSummary {
    // port: AccessorSummary#create
    pub fn create(accessors: IndexMap<JsString, PropertyAccessKind>) -> AccessorSummary {
        // TODO(nickreid): Efficiently verify that no entry in `accessor` is `NORMAL`.
        AccessorSummary::new_with_accessors(accessors)
    }

    // port: AccessorSummary#AccessorSummary(ImmutableMap)
    fn new_with_accessors(accessors: IndexMap<JsString, PropertyAccessKind>) -> AccessorSummary {
        AccessorSummary {
            accessors,
            assume_always_getter_and_setter: false,
        }
    }

    // port: AccessorSummary#AccessorSummary(boolean)
    fn new_assuming(assume_always_getter_and_setter: bool) -> AccessorSummary {
        AccessorSummary {
            accessors: IndexMap::<_, _>::default(),
            assume_always_getter_and_setter,
        }
    }

    // port: AccessorSummary#getAccessors
    pub fn get_accessors(&self) -> &IndexMap<JsString, PropertyAccessKind> {
        &self.accessors
    }

    // port: AccessorSummary#getKind
    pub fn get_kind(&self, name: &JsString) -> PropertyAccessKind {
        if self.assume_always_getter_and_setter {
            return PropertyAccessKind::GETTER_AND_SETTER;
        }
        self.accessors
            .get(name)
            .copied()
            .unwrap_or(PropertyAccessKind::NORMAL)
    }

    // port: AccessorSummary#createAssumingAlwaysGetterAndSetter
    pub fn create_assuming_always_getter_and_setter() -> AccessorSummary {
        AccessorSummary::new_assuming(true)
    }

    // port: AccessorSummary#toProto()
    pub fn to_proto(&self) -> AccessorSummaryProto {
        let mut builder = AccessorSummaryProto::new_builder()
            .set_assume_always_getter_and_setter(self.assume_always_getter_and_setter);
        if self.assume_always_getter_and_setter {
            check_state!(self.accessors.is_empty());
        } else {
            for (key, value) in &self.accessors {
                builder = builder.add_accessors(
                    AccessorSummaryEntryProto::new_builder()
                        .set_name(String::from_utf8(encode_utf8_java(key)).unwrap())
                        .set_kind(Self::kind_to_proto(*value))
                        .build(),
                );
            }
        }
        builder.build()
    }

    // port: AccessorSummary#toProto(PropertyAccessKind)
    fn kind_to_proto(kind: PropertyAccessKind) -> PropertyAccessKindProto {
        match kind {
            PropertyAccessKind::NORMAL => PropertyAccessKindProto::KIND_NORMAL,
            PropertyAccessKind::GETTER_ONLY => PropertyAccessKindProto::KIND_GETTER_ONLY,
            PropertyAccessKind::SETTER_ONLY => PropertyAccessKindProto::KIND_SETTER_ONLY,
            PropertyAccessKind::GETTER_AND_SETTER => {
                PropertyAccessKindProto::KIND_GETTER_AND_SETTER
            }
        }
    }

    // port: AccessorSummary#fromProto(AccessorSummaryProto)
    pub fn from_proto(proto: &AccessorSummaryProto) -> AccessorSummary {
        if proto.get_assume_always_getter_and_setter() {
            return Self::create_assuming_always_getter_and_setter();
        }
        // Java: ImmutableMap.Builder, then buildOrThrow (which rejects duplicate keys).
        let mut builder: IndexMap<JsString, PropertyAccessKind> = IndexMap::<_, _>::default();
        for entry in proto.get_accessors_list() {
            let key = JsString::from(entry.get_name());
            let value = Self::kind_from_proto(entry.get_kind());
            if let Some(previous) = builder.get(&key) {
                panic!(
                    "Multiple entries with same key: {}={} and {}={}",
                    key.to_string_lossy(),
                    value,
                    key.to_string_lossy(),
                    previous
                );
            }
            builder.insert(key, value);
        }
        Self::create(builder)
    }

    // port: AccessorSummary#fromProto(PropertyAccessKindProto)
    fn kind_from_proto(kind: PropertyAccessKindProto) -> PropertyAccessKind {
        match kind {
            PropertyAccessKindProto::KIND_NORMAL => PropertyAccessKind::NORMAL,
            PropertyAccessKindProto::KIND_GETTER_ONLY => PropertyAccessKind::GETTER_ONLY,
            PropertyAccessKindProto::KIND_SETTER_ONLY => PropertyAccessKind::SETTER_ONLY,
            PropertyAccessKindProto::KIND_GETTER_AND_SETTER => {
                PropertyAccessKind::GETTER_AND_SETTER
            }
            _ => panic!("Unknown kind: {kind}"),
        }
    }
}
