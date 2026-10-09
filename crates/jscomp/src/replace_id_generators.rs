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
//   src/com/google/javascript/jscomp/ReplaceIdGenerators.java.

//! Port of `ReplaceIdGenerators.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::default_name_generator::DefaultNameGenerator;
use crate::diagnostic_type::DiagnosticType;
use crate::id_mapping_util::{BiMap, IdMappingUtil};
use crate::js_error::JSError;
use crate::name_generator::NameGenerator;
use crate::node_traversal::{
    AbstractPostOrderCallback, AbstractPostOrderCallbackInterface, NodeTraversal,
};
use crate::node_util::NodeUtil;
use crate::renaming_map::RenamingMap;
use crate::renaming_token::RenamingToken;
use crate::xid::{HashFunction, Xid};
use closure_rhino::check_state;
use closure_rhino::fx_hash::{IndexMap, IndexSet};
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::node::NodeId;
use closure_rhino::qualified_name::QualifiedName;
use closure_sourcemap::base64::Base64;
use std::sync::{Arc, LazyLock, RwLock};

// port: ReplaceIdGenerators#NON_GLOBAL_ID_GENERATOR_CALL
pub static NON_GLOBAL_ID_GENERATOR_CALL: DiagnosticType = DiagnosticType::error(
    "JSC_NON_GLOBAL_ID_GENERATOR_CALL",
    "Id generator call must be in the global scope",
);

// port: ReplaceIdGenerators#CONDITIONAL_ID_GENERATOR_CALL
pub static CONDITIONAL_ID_GENERATOR_CALL: DiagnosticType = DiagnosticType::error(
    "JSC_CONDITIONAL_ID_GENERATOR_CALL",
    "Id generator call must be unconditional",
);

// port: ReplaceIdGenerators#MISSING_NAME_MAP_FOR_GENERATOR
pub static MISSING_NAME_MAP_FOR_GENERATOR: DiagnosticType = DiagnosticType::warning(
    "JSC_MISSING_NAME_MAP_FOR_GENERATOR",
    "The mapped id generator, does not have a renaming map supplied.",
);

// port: ReplaceIdGenerators#INVALID_GENERATOR_PARAMETER
pub static INVALID_GENERATOR_PARAMETER: DiagnosticType = DiagnosticType::warning(
    "JSC_INVALID_GENERATOR_PARAMETER",
    "An id generator must be called with a literal.",
);

// port: ReplaceIdGenerators#INVALID_TEMPLATE_LITERAL_PARAMETER
pub static INVALID_TEMPLATE_LITERAL_PARAMETER: DiagnosticType = DiagnosticType::warning(
    "JSC_INVALID_GENERATOR_PARAMETER",
    "An id generator must be called with a template literals with no invalid escape sequences.",
);

// port: ReplaceIdGenerators#SHORTHAND_FUNCTION_NOT_SUPPORTED_IN_ID_GEN
pub static SHORTHAND_FUNCTION_NOT_SUPPORTED_IN_ID_GEN: DiagnosticType = DiagnosticType::error(
    "JSC_SHORTHAND_FUNCTION_NOT_SUPPORTED_IN_ID_GEN",
    "Object literal shorthand functions is not allowed in the arguments of an id generator",
);

// port: ReplaceIdGenerators#COMPUTED_PROP_NOT_SUPPORTED_IN_ID_GEN
pub static COMPUTED_PROP_NOT_SUPPORTED_IN_ID_GEN: DiagnosticType = DiagnosticType::error(
    "JSC_COMPUTED_PROP_NOT_SUPPORTED_IN_ID_GEN",
    "Object literal computed property name is not allowed in the arguments of an id generator",
);

// port: ReplaceIdGenerators#CREATE_TEMPLATE_TAG_FIRST_ARG
static CREATE_TEMPLATE_TAG_FIRST_ARG: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("$jscomp.createTemplateTagFirstArg"));

/// A generator name: Java's `String` keys of these maps may be null (a JSDoc'd node that is
/// neither an assignment, a declaration nor a function, or a call target that is not a qualified
/// name).
type GeneratorName = Option<JsString>;

/// Replaces calls to id generators with ids.
///
/// Use this to get unique and short ids.
pub struct ReplaceIdGenerators {
    template_literals_are_transpiled: bool,
    /// A `None` value is a DISABLE'd generator (Java's null map value).
    name_generators: IndexMap<GeneratorName, Option<Box<dyn NameSupplier>>>,
    consist_name_map: IndexMap<GeneratorName, IndexMap<JsString, JsString>>,

    id_generator_maps: IndexMap<GeneratorName, IndexMap<JsString, JsString>>,
    previous_map: IndexMap<JsString, BiMap>,

    generate_pseudo_names: bool,
    xid_hash_function: Option<Arc<dyn HashFunction + Send + Sync>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
pub enum RenameStrategy {
    CONSISTENT,
    INCONSISTENT,
    MAPPED,
    STABLE,
    XID,
}

trait NameSupplier {
    // port: ReplaceIdGenerators.NameSupplier#getName
    fn get_name(&mut self, id: &JsString, name: &JsString) -> JsString;

    // port: ReplaceIdGenerators.NameSupplier#getRenameStrategy
    fn get_rename_strategy(&self) -> RenameStrategy;
}

struct ObfuscatedNameSupplier {
    generator: DefaultNameGenerator,
    previous_mappings: IndexMap<JsString, JsString>,
    rename_strategy: RenameStrategy,
}

impl ObfuscatedNameSupplier {
    // port: ReplaceIdGenerators.ObfuscatedNameSupplier#ObfuscatedNameSupplier
    fn new(rename_strategy: RenameStrategy, previous_mappings: &BiMap) -> Self {
        Self {
            previous_mappings: previous_mappings.inverse(),
            generator: DefaultNameGenerator::with_reserved_characters(
                Arc::new(RwLock::new(previous_mappings.key_set())),
                JsString::from(""),
                &IndexSet::<_>::default(),
            ),
            rename_strategy,
        }
    }
}

impl NameSupplier for ObfuscatedNameSupplier {
    // port: ReplaceIdGenerators.ObfuscatedNameSupplier#getName
    fn get_name(&mut self, id: &JsString, _name: &JsString) -> JsString {
        let new_name = self.previous_mappings.get(id).cloned();
        match new_name {
            Some(new_name) => new_name,
            None => self.generator.generate_next_name(),
        }
    }

    // port: ReplaceIdGenerators.ObfuscatedNameSupplier#getRenameStrategy
    fn get_rename_strategy(&self) -> RenameStrategy {
        self.rename_strategy
    }
}

struct PseudoNameSupplier {
    counter: i32,
    rename_strategy: RenameStrategy,
}

impl PseudoNameSupplier {
    // port: ReplaceIdGenerators.PseudoNameSupplier#PseudoNameSupplier
    fn new(rename_strategy: RenameStrategy) -> Self {
        Self {
            counter: 0,
            rename_strategy,
        }
    }
}

impl NameSupplier for PseudoNameSupplier {
    // port: ReplaceIdGenerators.PseudoNameSupplier#getName
    fn get_name(&mut self, _id: &JsString, name: &JsString) -> JsString {
        if self.rename_strategy == RenameStrategy::INCONSISTENT {
            let counter = self.counter;
            self.counter += 1;
            return name.concat(&JsString::from(format!("${counter}")));
        }
        name.concat(&JsString::from("$0"))
    }

    // port: ReplaceIdGenerators.PseudoNameSupplier#getRenameStrategy
    fn get_rename_strategy(&self) -> RenameStrategy {
        self.rename_strategy
    }
}

struct StableNameSupplier;

impl NameSupplier for StableNameSupplier {
    // port: ReplaceIdGenerators.StableNameSupplier#getName
    fn get_name(&mut self, _id: &JsString, name: &JsString) -> JsString {
        Base64::base64_encode_int(name.hash_code())
    }

    // port: ReplaceIdGenerators.StableNameSupplier#getRenameStrategy
    fn get_rename_strategy(&self) -> RenameStrategy {
        RenameStrategy::STABLE
    }
}

struct XidNameSupplier {
    xid: Xid,
}

impl XidNameSupplier {
    // port: ReplaceIdGenerators.XidNameSupplier#XidNameSupplier
    fn new(hash_function: Option<&Arc<dyn HashFunction + Send + Sync>>) -> Self {
        Self {
            xid: match hash_function {
                None => Xid::new(),
                Some(hash_function) => Xid::with_hasher(hash_function.clone()),
            },
        }
    }
}

impl NameSupplier for XidNameSupplier {
    // port: ReplaceIdGenerators.XidNameSupplier#getName
    fn get_name(&mut self, _id: &JsString, name: &JsString) -> JsString {
        JsString::from(self.xid.get(name))
    }

    // port: ReplaceIdGenerators.XidNameSupplier#getRenameStrategy
    fn get_rename_strategy(&self) -> RenameStrategy {
        RenameStrategy::XID
    }
}

struct MappedNameSupplier {
    map: Arc<dyn RenamingMap + Send + Sync>,
}

impl NameSupplier for MappedNameSupplier {
    // port: ReplaceIdGenerators.MappedNameSupplier#getName
    fn get_name(&mut self, _id: &JsString, name: &JsString) -> JsString {
        // Java returns the map's (possibly null) result, which then fails in IR.string.
        self.map
            .get(name)
            .expect("NullPointerException: RenamingMap#get returned null")
    }

    // port: ReplaceIdGenerators.MappedNameSupplier#getRenameStrategy
    fn get_rename_strategy(&self) -> RenameStrategy {
        RenameStrategy::MAPPED
    }
}

impl ReplaceIdGenerators {
    // port: ReplaceIdGenerators#ReplaceIdGenerators
    pub fn new(
        _compiler: &AbstractCompiler,
        template_literals_are_transpiled: bool,
        id_gens: Option<&IndexMap<String, Arc<dyn RenamingMap + Send + Sync>>>,
        generate_pseudo_names: bool,
        previous_map_serialized: Option<&str>,
        xid_hash_function: Option<Arc<dyn HashFunction + Send + Sync>>,
    ) -> Self {
        let previous_map = IdMappingUtil::parse_serialized_id_mappings(previous_map_serialized);
        let mut this = Self {
            template_literals_are_transpiled,
            generate_pseudo_names,
            xid_hash_function,
            name_generators: IndexMap::<_, _>::default(),
            id_generator_maps: IndexMap::<_, _>::default(),
            consist_name_map: IndexMap::<_, _>::default(),
            previous_map,
        };

        if let Some(id_gens) = id_gens {
            for (name, map) in id_gens {
                let name = Some(JsString::from(name.as_str()));
                let map_any: &dyn std::any::Any = map.as_ref();
                if let Some(renaming_token) = map_any.downcast_ref::<RenamingToken>() {
                    match renaming_token {
                        RenamingToken::DISABLE => {
                            this.name_generators.insert(name, None);
                            continue;
                            // don't put an entry in idGeneratorsMap
                        }
                        RenamingToken::INCONSISTENT => {
                            let supplier = this.create_name_supplier(
                                RenameStrategy::INCONSISTENT,
                                this.previous_map_get(&name),
                            );
                            this.name_generators.insert(name.clone(), Some(supplier));
                        }
                        RenamingToken::STABLE => {
                            let supplier = this.create_name_supplier(
                                RenameStrategy::STABLE,
                                this.previous_map_get(&name),
                            );
                            this.name_generators.insert(name.clone(), Some(supplier));
                        }
                    }
                } else {
                    this.name_generators.insert(
                        name.clone(),
                        Some(Self::create_mapped_name_supplier(
                            RenameStrategy::MAPPED,
                            map.clone(),
                        )),
                    );
                }
                this.id_generator_maps
                    .insert(name, IndexMap::<_, _>::default());
            }
        }
        this
    }

    /// `previousMap.get(name)`.
    fn previous_map_get(&self, name: &GeneratorName) -> Option<&BiMap> {
        name.as_ref().and_then(|name| self.previous_map.get(name))
    }

    // port: ReplaceIdGenerators#createNameSupplier(RenameStrategy,BiMap)
    fn create_name_supplier(
        &self,
        rename_strategy: RenameStrategy,
        previous_mappings: Option<&BiMap>,
    ) -> Box<dyn NameSupplier> {
        let empty = BiMap::new();
        let previous_mappings = previous_mappings.unwrap_or(&empty);
        if rename_strategy == RenameStrategy::STABLE {
            Box::new(StableNameSupplier)
        } else if rename_strategy == RenameStrategy::XID {
            Box::new(XidNameSupplier::new(self.xid_hash_function.as_ref()))
        } else if self.generate_pseudo_names {
            Box::new(PseudoNameSupplier::new(rename_strategy))
        } else {
            Box::new(ObfuscatedNameSupplier::new(
                rename_strategy,
                previous_mappings,
            ))
        }
    }

    // port: ReplaceIdGenerators#createNameSupplier(RenameStrategy,RenamingMap)
    fn create_mapped_name_supplier(
        rename_strategy: RenameStrategy,
        mappings: Arc<dyn RenamingMap + Send + Sync>,
    ) -> Box<dyn NameSupplier> {
        check_state!(rename_strategy == RenameStrategy::MAPPED);
        Box::new(MappedNameSupplier { map: mappings })
    }

    /// Returns the serialize map of generators and their ids and their replacements.
    // port: ReplaceIdGenerators#getSerializedIdMappings
    pub fn get_serialized_id_mappings(&self) -> String {
        IdMappingUtil::generate_serialized_id_mappings(&self.id_generator_maps)
    }

    // port: ReplaceIdGenerators#getIdForGeneratorNode
    fn get_id_for_generator_node(
        ast: &closure_rhino::node::Ast,
        consistent: bool,
        n: NodeId,
        name: &JsString,
    ) -> JsString {
        if consistent {
            name.clone()
        } else {
            JsString::from(format!(
                "{}:{}:{}",
                n.get_source_file_name(ast)
                    .unwrap_or_else(|| "null".to_string()),
                n.get_lineno(ast),
                n.get_charno(ast)
            ))
        }
    }
}

impl CompilerPass for ReplaceIdGenerators {
    // port: ReplaceIdGenerators#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(
            compiler,
            root,
            &mut AbstractPostOrderCallback::new(GatherGenerators { outer: self }),
        );
        if !self.name_generators.is_empty() {
            NodeTraversal::traverse(
                compiler,
                root,
                &mut AbstractPostOrderCallback::new(ReplaceGenerators { outer: self }),
            );
        }
    }
}

struct GatherGenerators<'a> {
    outer: &'a mut ReplaceIdGenerators,
}

impl AbstractPostOrderCallbackInterface for GatherGenerators<'_> {
    // port: ReplaceIdGenerators.GatherGenerators#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let Some(doc) = n.get_jsdoc_info(t) else {
            return;
        };
        if !doc.is_any_id_generator() {
            return;
        }

        let mut name: GeneratorName = None;
        if n.is_assign(t) {
            name = n.get_first_child(t).unwrap().get_qualified_name(t);
        } else if NodeUtil::is_name_declaration(t, Some(n)) {
            name = Some(n.get_first_child(t).unwrap().get_string(t));
        } else if n.is_function(t) {
            let function_name = n.get_first_child(t).unwrap().get_string(t);
            if function_name.is_empty() {
                return;
            }
            name = Some(function_name);
        }
        let outer = &mut *self.outer;
        if outer.name_generators.contains_key(&name) {
            // This generator is already registered from our constructor.
            // Don't override it.
            return;
        }
        if doc.is_consistent_id_generator() {
            outer
                .consist_name_map
                .insert(name.clone(), IndexMap::<_, _>::default());
            let supplier = outer
                .create_name_supplier(RenameStrategy::CONSISTENT, outer.previous_map_get(&name));
            outer.name_generators.insert(name.clone(), Some(supplier));
        } else if doc.is_stable_id_generator() {
            let supplier =
                outer.create_name_supplier(RenameStrategy::STABLE, outer.previous_map_get(&name));
            outer.name_generators.insert(name.clone(), Some(supplier));
        } else if doc.is_xid_generator() {
            let supplier =
                outer.create_name_supplier(RenameStrategy::XID, outer.previous_map_get(&name));
            outer.name_generators.insert(name.clone(), Some(supplier));
        } else if doc.is_id_generator() {
            let supplier = outer
                .create_name_supplier(RenameStrategy::INCONSISTENT, outer.previous_map_get(&name));
            outer.name_generators.insert(name.clone(), Some(supplier));
        } else if doc.is_mapped_id_generator() {
            let supplier = outer.name_generators.get(&name).and_then(Option::as_ref);
            if supplier.is_none_or(|s| s.get_rename_strategy() != RenameStrategy::MAPPED) {
                let compiler = t.get_compiler();
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &MISSING_NAME_MAP_FOR_GENERATOR,
                    &[],
                ));
                // skip registering the name in the list of Generators if there no
                // mapping.
                return;
            }
        } else {
            panic!("IllegalStateException: unexpected");
        }
        outer
            .id_generator_maps
            .insert(name, IndexMap::<_, _>::default());
    }
}

struct ReplaceGenerators<'a> {
    outer: &'a mut ReplaceIdGenerators,
}

impl AbstractPostOrderCallbackInterface for ReplaceGenerators<'_> {
    // port: ReplaceIdGenerators.ReplaceGenerators#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if !n.is_call(t) && !n.is_tagged_template_lit(t) {
            return;
        }

        let qname = NodeUtil::get_call_target_resolving_indirect_calls(t, n);
        let call_name: GeneratorName = qname.get_qualified_name(t);
        let Some(Some(name_generator)) = self.outer.name_generators.get(&call_name) else {
            return;
        };
        let strategy = name_generator.get_rename_strategy();

        if !t.in_global_hoist_scope() && strategy == RenameStrategy::INCONSISTENT {
            // Warn about calls not in the global scope.
            let compiler = t.get_compiler();
            compiler.report(JSError::make(
                compiler,
                n,
                &NON_GLOBAL_ID_GENERATOR_CALL,
                &[],
            ));
            return;
        }

        if strategy == RenameStrategy::INCONSISTENT {
            let is_conditional = n
                .get_ancestors(t)
                .any(|ancestor| NodeUtil::is_control_structure(t, ancestor));
            if is_conditional {
                // Warn about conditional calls.
                let compiler = t.get_compiler();
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &CONDITIONAL_ID_GENERATOR_CALL,
                    &[],
                ));
                return;
            }
        }

        if n.is_call(t) {
            self.maybe_replace_call(t, n, &call_name);
        } else {
            self.maybe_replace_tagged_template_lit(t, n, &call_name);
        }
    }
}

impl ReplaceGenerators<'_> {
    // port: ReplaceIdGenerators.ReplaceGenerators#maybeReplaceTaggedTemplateLit
    fn maybe_replace_tagged_template_lit(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        call_name: &GeneratorName,
    ) {
        let arg = n.get_last_child(t);

        if arg.is_none_or(|arg| !arg.is_template_lit(t)) {
            panic!("IllegalStateException");
        }
        let arg = arg.unwrap();
        if arg.has_one_child(t) {
            let cooked = arg.get_first_child(t).unwrap().get_cooked_string(t);
            let Some(cooked) = cooked else {
                // We don't allow strings with odd escape sequences... We could but it doesn't seem
                // necessary
                let compiler = t.get_compiler();
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &INVALID_TEMPLATE_LITERAL_PARAMETER,
                    &[],
                ));
                return;
            };

            let rename = self.get_obfuscated_name(t, arg, call_name, &cooked);
            let replacement = IR::string(t, rename);
            n.replace_with(t, replacement);
            t.report_code_change();
        } else {
            // There is an alternating sequence of template literals and expressions, in this case
            // we need to preserve the function call but obfuscate the literals.
            let new_template_lit = IR::template_literal(t);
            let mut child = arg.get_first_child(t);
            while let Some(c) = child {
                // Read the next sibling first: a substitution's child is detached below.
                let next = c.get_next(t);
                if c.is_template_lit_string(t) {
                    let cooked = c.get_cooked_string(t);
                    let Some(cooked) = cooked else {
                        let compiler = t.get_compiler();
                        compiler.report(JSError::make(
                            compiler,
                            n,
                            &INVALID_TEMPLATE_LITERAL_PARAMETER,
                            &[],
                        ));
                        return;
                    };
                    let rename = self.get_obfuscated_name(t, c, call_name, &cooked);
                    let string = IR::template_literal_string(t, Some(rename.clone()), rename)
                        .srcref_if_missing(t, c);
                    new_template_lit.add_child_to_back(t, string);
                } else {
                    let removed = c.remove_first_child(t).unwrap();
                    let substitution =
                        IR::template_literal_substitution(t, removed).srcref_if_missing(t, c);
                    new_template_lit.add_child_to_back(t, substitution);
                }
                child = next;
            }
            arg.replace_with(t, new_template_lit);
            t.report_code_change_at_node(new_template_lit);
        }
    }

    // port: ReplaceIdGenerators.ReplaceGenerators#maybeReplaceCall
    fn maybe_replace_call(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        call_name: &GeneratorName,
    ) {
        let arg = n.get_second_child(t);
        let Some(arg) = arg else {
            let compiler = t.get_compiler();
            compiler.report(JSError::make(
                compiler,
                n,
                &INVALID_GENERATOR_PARAMETER,
                &[],
            ));
            return;
        };
        if arg.is_string_lit(t) {
            let string = arg.get_string(t);
            let rename = self.get_obfuscated_name(t, arg, call_name, &string);
            let replacement = IR::string(t, rename);
            n.replace_with(t, replacement);
            t.report_code_change();
        } else if arg.is_template_lit(t) && arg.has_one_child(t) {
            let cooked = arg.get_first_child(t).unwrap().get_cooked_string(t);
            match cooked {
                None => {
                    let compiler = t.get_compiler();
                    compiler.report(JSError::make(
                        compiler,
                        n,
                        &INVALID_GENERATOR_PARAMETER,
                        &[],
                    ));
                }
                Some(cooked) => {
                    let rename = self.get_obfuscated_name(t, arg, call_name, &cooked);
                    let replacement = IR::string(t, rename);
                    n.replace_with(t, replacement);
                    t.report_code_change();
                }
            }
        } else if arg.is_object_lit(t) {
            let mut key = arg.get_first_child(t);
            while let Some(k) = key {
                if k.is_member_function_def(t) {
                    let compiler = t.get_compiler();
                    compiler.report(JSError::make(
                        compiler,
                        n,
                        &SHORTHAND_FUNCTION_NOT_SUPPORTED_IN_ID_GEN,
                        &[],
                    ));
                    return;
                }
                if k.is_computed_prop(t) {
                    let compiler = t.get_compiler();
                    compiler.report(JSError::make(
                        compiler,
                        n,
                        &COMPUTED_PROP_NOT_SUPPORTED_IN_ID_GEN,
                        &[],
                    ));
                    return;
                }

                let string = k.get_string(t);
                let rename = self.get_obfuscated_name(t, k, call_name, &string);
                k.set_string(t, rename);
                // Prevent standard renaming by marking the key as quoted.
                k.put_boolean_prop(t, NodeId::QUOTED_PROP, true);
                key = k.get_next(t);
            }
            arg.detach(t);
            n.replace_with(t, arg);
            t.report_code_change();
        } else if self.outer.template_literals_are_transpiled && arg.is_name(t) {
            // This might be a transpiled template literal.  Find the definition.
            // The pass always injects it at the current script root, typically at the top, but not
            // always.
            let ttl_var = Self::find_ttl_var(t, arg);
            let Some(ttl_var) = ttl_var else {
                // This could be some other call to the ttl function not using ttl syntax.  These
                // are weird but not illegal.
                let compiler = t.get_compiler();
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &INVALID_GENERATOR_PARAMETER,
                    &[],
                ));
                return;
            };
            let ttl_function_call = ttl_var.get_first_first_child(t).unwrap();
            let params_array_literal = ttl_function_call.get_second_child(t);
            let Some(params_array_literal) = params_array_literal.filter(|p| p.is_array_lit(t))
            else {
                panic!("IllegalStateException: bad transpiled structure");
            };
            // 2 cases, if there is exactly 1 element we can just replace everything
            // otherwise we need to rewrite the array in place.
            if params_array_literal.get_child_count(t) == 1 {
                let original_name = params_array_literal
                    .get_first_child(t)
                    .unwrap()
                    .get_string(t);
                let rename = self.get_obfuscated_name(t, arg, call_name, &original_name);
                let replacement = IR::string(t, rename);
                n.replace_with(t, replacement);
                t.report_code_change();
                t.report_code_change_at_node(ttl_var);
                ttl_var.detach(t);
            } else {
                // We have parameters, so we need to rewrite the TTL array in place and leave the
                // function call.
                let mut param = params_array_literal.get_first_child(t);
                while let Some(p) = param {
                    let original_name = p.get_string(t);
                    let rename = self.get_obfuscated_name(t, arg, call_name, &original_name);
                    p.set_string(t, rename);
                    param = p.get_next(t);
                }
                t.report_code_change_at_node(params_array_literal);
            }
        } else {
            let compiler = t.get_compiler();
            compiler.report(JSError::make(
                compiler,
                n,
                &INVALID_GENERATOR_PARAMETER,
                &[],
            ));
        }
    }

    // port: ReplaceIdGenerators.ReplaceGenerators#findTtlVar
    fn find_ttl_var(t: &mut NodeTraversal<'_>, arg: NodeId) -> Option<NodeId> {
        let name = arg.get_string(t);
        let scope = t.get_scope();
        let var = scope
            .get_var(t.get_compiler(), name)
            .expect("NullPointerException: Scope#getVar");
        let ttl_var_name = var
            .get_node(t.get_compiler())
            .expect("NullPointerException: Var#getNode");
        let parent = ttl_var_name.get_parent(t).unwrap();
        // We are only interested in top level var definitions since that is what the transpiler
        // creates
        if !parent.is_var(t) || !parent.get_parent(t).unwrap().is_script(t) {
            return None;
        }
        // Because the AST is normalized there is only one child
        check_state!(
            ttl_var_name.get_parent(t).unwrap().has_one_child(t),
            "The AST is normalized so there should only be one name per var"
        );
        let call_node = ttl_var_name.get_first_child(t)?;
        if !call_node.is_call(t) {
            return None;
        }
        let call_expr = call_node.get_first_child(t).unwrap();
        if call_expr.matches_name(t, "$jscomp$createTemplateTagFirstArg")
            // The dot case is about our unit tests mostly.
            || CREATE_TEMPLATE_TAG_FIRST_ARG.matches(t, call_expr)
        {
            return ttl_var_name.get_parent(t);
        }

        None
    }

    // port: ReplaceIdGenerators.ReplaceGenerators#getObfuscatedName
    fn get_obfuscated_name(
        &mut self,
        t: &NodeTraversal<'_>,
        id: NodeId,
        call_name: &GeneratorName,
        name: &JsString,
    ) -> JsString {
        let outer = &mut *self.outer;
        let name_generator = outer
            .name_generators
            .get_mut(call_name)
            .and_then(Option::as_mut)
            .expect("ReplaceIdGenerators name generator");
        let strategy = name_generator.get_rename_strategy();
        let instance_id = ReplaceIdGenerators::get_id_for_generator_node(
            t,
            strategy != RenameStrategy::INCONSISTENT,
            id,
            name,
        );
        let rename = if strategy == RenameStrategy::CONSISTENT {
            let entry = outer
                .consist_name_map
                .get_mut(call_name)
                .expect("NullPointerException: consistNameMap entry");
            match entry.get(&instance_id) {
                Some(rename) => rename.clone(),
                None => {
                    let rename = name_generator.get_name(&instance_id, name);
                    entry.insert(instance_id.clone(), rename.clone());
                    rename
                }
            }
        } else {
            name_generator.get_name(&instance_id, name)
        };
        outer
            .id_generator_maps
            .get_mut(call_name)
            .expect("NullPointerException: idGeneratorMaps entry")
            .insert(rename.clone(), instance_id);
        rename
    }
}
