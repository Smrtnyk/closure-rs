/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/GatherGetterAndSetterProperties.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of `GatherGetterAndSetterProperties.java`: finds all properties defined by getters or
//! setters and records them in the compiler's `AccessorSummary`.

use crate::{
    AbstractCompiler,
    accessor_summary::{AccessorSummary, PropertyAccessKind},
    compiler_pass::CompilerPass,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    check_state,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};
use std::sync::Arc;

/// Finds all getters and setters in the compilation (Java `CompilerPass`).
pub struct GatherGetterAndSetterProperties;

impl GatherGetterAndSetterProperties {
    // port: GatherGetterAndSetterProperties#GatherGetterAndSetterProperties
    pub fn new() -> Self {
        Self
    }

    // port: GatherGetterAndSetterProperties#update
    pub fn update(compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        // Java scans the live externs and sources anew, including for callers that provided
        // different roots or changed a descriptor between optimizer passes.
        check_state!(externs.get_parent(compiler) == root.get_parent(compiler));
        let accessor_summary = if compiler
            .get_options()
            .get_assume_properties_are_statically_analyzable()
        {
            let parent = externs.get_parent(compiler).unwrap();
            AccessorSummary::create(Self::gather(compiler, parent))
        } else {
            AccessorSummary::create_assuming_always_getter_and_setter()
        };
        compiler.set_accessor_summary(Arc::new(accessor_summary));
    }

    // port: GatherGetterAndSetterProperties#gather
    pub fn gather(
        compiler: &mut AbstractCompiler,
        root: NodeId,
    ) -> IndexMap<JsString, PropertyAccessKind> {
        const TOKENS: &[Token] = &[
            Token::SCRIPT,
            Token::FUNCTION,
            Token::CLASS,
            Token::CLASS_MEMBERS,
            Token::GETTER_DEF,
            Token::SETTER_DEF,
            Token::GETPROP,
        ];
        let mut gather_callback = if root.is_root(compiler)
            && compiler
                .get_options_opt()
                .is_some_and(|options| options.get_synthetic_block_start_marker().is_none())
        {
            compiler.with_token_candidates(TOKENS, GatherCallback::from_candidates)
        } else {
            // A subtree request is already small. A traversal with synthetic blocks enabled also
            // reads each BLOCK's SYNTHETIC property, which can throw on a malformed public AST.
            GatherCallback::default()
        };
        NodeTraversal::traverse(compiler, root, &mut gather_callback);
        gather_callback.properties
    }
}

impl Default for GatherGetterAndSetterProperties {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for GatherGetterAndSetterProperties {
    // port: GatherGetterAndSetterProperties#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        Self::update(compiler, externs, root);
    }
}

// port: GatherGetterAndSetterProperties.GatherCallback
#[derive(Default)]
struct GatherCallback {
    // An empty mask requests the ordinary traversal.
    paths: Vec<u64>,
    properties: IndexMap<JsString, PropertyAccessKind>,
}

impl GatherCallback {
    /// Mark the paths to observable visits. NodeTraversal supplies Java's order even within
    /// classes with computed keys. Paths are rebuilt from live links and values on each request.
    fn from_candidates(ast: &Ast, candidates: &[NodeId]) -> Self {
        let mut paths = vec![0u64; (ast.node_count() as usize).div_ceil(64)];
        for &n in candidates {
            match n.get_token(ast) {
                // The SCRIPT handler checks for an interrupt before asking whether to descend.
                Token::SCRIPT => {
                    if let Some(parent) = n.get_parent(ast) {
                        Self::mark_path(ast, &mut paths, parent);
                    }
                    continue;
                }
                // The function handler visits its first three children and inspects a declaration's
                // name. A well-formed function without a marked descendant needs no visit.
                Token::FUNCTION => {
                    if n.get_first_child(ast).is_some_and(|name| {
                        (!name.is_name(ast) || name.string_payload(ast).is_some())
                            && name
                                .get_next(ast)
                                .and_then(|params| params.get_next(ast))
                                .is_some()
                    }) {
                        continue;
                    }
                }
                Token::CLASS | Token::CLASS_MEMBERS | Token::GETTER_DEF | Token::SETTER_DEF => {}
                Token::GETPROP => {
                    let Some(call) = n.get_parent(ast) else {
                        continue;
                    };
                    if !call.is_call(ast) || call.get_first_child(ast) != Some(n) {
                        continue;
                    }
                    if n.string_payload(ast).is_some_and(|name| {
                        name != "create" && name != "defineProperty" && name != "defineProperties"
                    }) {
                        continue;
                    }
                    Self::mark_path(ast, &mut paths, call);
                    continue;
                }
                _ => continue,
            }
            Self::mark_path(ast, &mut paths, n);
        }
        Self {
            paths,
            properties: IndexMap::<_, _>::default(),
        }
    }

    fn mark_path(ast: &Ast, paths: &mut [u64], n: NodeId) {
        let mut current = Some(n);
        while let Some(node) = current {
            let slot = node.arena_index();
            let mask = 1u64 << (slot % 64);
            if paths[slot / 64] & mask != 0 {
                break;
            }
            paths[slot / 64] |= mask;
            current = node.get_parent(ast);
        }
    }

    // port: GatherGetterAndSetterProperties.GatherCallback#record
    fn record(&mut self, property: JsString, kind: PropertyAccessKind) {
        // Java: properties.merge(property, kind, PropertyAccessKind::unionWith)
        match self.properties.get_mut(&property) {
            Some(existing) => *existing = existing.union_with(kind),
            None => {
                self.properties.insert(property, kind);
            }
        }
    }

    // port: GatherGetterAndSetterProperties.GatherCallback#recordGetterDef
    fn record_getter_def(&mut self, ast: &Ast, getter_def: NodeId) {
        check_state!(getter_def.is_getter_def(ast));
        let name = getter_def.get_string(ast);
        self.record(name, PropertyAccessKind::GETTER_ONLY);
    }

    // port: GatherGetterAndSetterProperties.GatherCallback#recordSetterDef
    fn record_setter_def(&mut self, ast: &Ast, setter_def: NodeId) {
        check_state!(setter_def.is_setter_def(ast));
        let name = setter_def.get_string(ast);
        self.record(name, PropertyAccessKind::SETTER_ONLY);
    }

    // port: GatherGetterAndSetterProperties.GatherCallback#visitDescriptor
    fn visit_descriptor(&mut self, ast: &Ast, property_name: &JsString, descriptor: NodeId) {
        let mut key = descriptor.get_first_child(ast);
        while let Some(k) = key {
            if k.is_string_key(ast) || k.is_member_function_def(ast) {
                if k.get_string_ref(ast) == "get" {
                    self.record(property_name.clone(), PropertyAccessKind::GETTER_ONLY);
                } else if k.get_string_ref(ast) == "set" {
                    self.record(property_name.clone(), PropertyAccessKind::SETTER_ONLY);
                }
            }
            key = k.get_next(ast);
        }
    }

    // port: GatherGetterAndSetterProperties.GatherCallback#visitDefineProperty
    fn visit_define_property(&mut self, ast: &Ast, define_property_call: NodeId) {
        let property_name_node = define_property_call.get_child_at_index(ast, 2).unwrap();
        let descriptor = define_property_call.get_child_at_index(ast, 3).unwrap();
        if !property_name_node.is_string_lit(ast) || !descriptor.is_object_lit(ast) {
            return;
        }
        let property_name = property_name_node.get_string(ast);
        self.visit_descriptor(ast, &property_name, descriptor);
    }

    // port: GatherGetterAndSetterProperties.GatherCallback#visitDefineProperties
    fn visit_define_properties(&mut self, ast: &Ast, define_properties_call: NodeId) {
        let props = define_properties_call.get_child_at_index(ast, 2).unwrap();
        if !props.is_object_lit(ast) {
            return;
        }
        let mut prop = props.get_first_child(ast);
        while let Some(p) = prop {
            if p.is_string_key(ast)
                && p.has_one_child(ast)
                && p.get_first_child(ast).unwrap().is_object_lit(ast)
            {
                let property_name = p.get_string(ast);
                let descriptor = p.get_first_child(ast).unwrap();
                self.visit_descriptor(ast, &property_name, descriptor);
            }
            prop = p.get_next(ast);
        }
    }
}

impl Callback for GatherCallback {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        let slot = n.arena_index();
        self.paths.is_empty() || self.paths[slot / 64] & (1u64 << (slot % 64)) != 0
    }

    // port: GatherGetterAndSetterProperties.GatherCallback#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        match n.get_token(t) {
            Token::GETTER_DEF => self.record_getter_def(t, n),
            Token::SETTER_DEF => self.record_setter_def(t, n),
            Token::CALL => {
                if NodeUtil::is_object_define_property_definition(t, n) {
                    self.visit_define_property(t, n);
                } else if NodeUtil::is_object_define_properties_definition(t, n)
                    || NodeUtil::is_object_create_with_properties_definition(t, n)
                {
                    self.visit_define_properties(t, n);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Compiler;
    use closure_rhino::ir::IR;

    fn gather_both(compiler: &mut Compiler, root: NodeId) -> Vec<(String, PropertyAccessKind)> {
        let selected = GatherGetterAndSetterProperties::gather(compiler, root);
        let mut full = GatherCallback::default();
        NodeTraversal::traverse(compiler, root, &mut full);
        assert!(selected.iter().eq(full.properties.iter()));
        selected
            .into_iter()
            .map(|(name, kind)| (name.to_string(), kind))
            .collect()
    }

    #[test]
    fn indexed_refresh_keeps_computed_member_order() {
        let mut compiler = Compiler::new();
        let script = compiler.parse_test_code(
            r#"
          const o = {get first() {}};
          class C {
            [Object.defineProperty({}, 'key1', {get() {}})]() {
              Object.defineProperty({}, 'body', {set(v) {}});
            }
            [Object.defineProperty({}, 'key2', {get() {}})]() {}
          }
          Object.create(null, {first: {set(v) {}}, last: {get() {}}});
        "#,
        );
        let root = IR::root(&mut compiler, &[script]);
        use PropertyAccessKind::*;
        assert_eq!(
            gather_both(&mut compiler, root),
            [
                ("first".into(), GETTER_AND_SETTER),
                ("key1".into(), GETTER_ONLY),
                ("key2".into(), GETTER_ONLY),
                ("body".into(), SETTER_ONLY),
                ("last".into(), GETTER_ONLY)
            ]
        );
    }

    #[test]
    fn indexed_refresh_reads_unreported_edits_and_new_nodes() {
        let mut compiler = Compiler::new();
        let script = compiler.parse_test_code("Object.other({}, 'live', {get() {}});");
        let root = IR::root(&mut compiler, &[script]);
        let stmt = script.get_first_child(&compiler).unwrap();
        let call = stmt.get_first_child(&compiler).unwrap();
        let callee = call.get_first_child(&compiler).unwrap();
        let old_name = IR::name(&mut compiler, "before");
        let other = IR::script(&mut compiler);
        assert!(gather_both(&mut compiler, root).is_empty());
        callee.set_string(&mut compiler, "defineProperty");
        assert_eq!(
            gather_both(&mut compiler, root),
            [("live".into(), PropertyAccessKind::GETTER_ONLY)]
        );
        old_name.set_token(&mut compiler, Token::SETTER_DEF);
        other.add_child_to_back(&mut compiler, old_name);
        root.add_child_to_front(&mut compiler, other);
        assert_eq!(gather_both(&mut compiler, root).len(), 2);
        old_name.set_string(&mut compiler, "live");
        assert_eq!(
            gather_both(&mut compiler, root),
            [("live".into(), PropertyAccessKind::GETTER_AND_SETTER)]
        );
        stmt.detach(&mut compiler);
        let new_site = compiler.new_string_with_token(Token::GETTER_DEF, "new");
        script.add_child_to_back(&mut compiler, new_site);
        assert_eq!(
            gather_both(&mut compiler, root),
            [
                ("live".into(), PropertyAccessKind::SETTER_ONLY),
                ("new".into(), PropertyAccessKind::GETTER_ONLY)
            ]
        );
    }
}
