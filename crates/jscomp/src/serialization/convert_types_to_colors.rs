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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 143e31c:
//   src/com/google/javascript/jscomp/serialization/ConvertTypesToColors.java.

//! Port of serialization/ConvertTypesToColors.java.
use super::color_pool::{ColorPool, ShardView};
use super::jsdoc_serializer::JSDocSerializer;
use super::serialization_options::SerializationOptions;
use super::serialize_types_to_pointers::SerializeTypesToPointers;
use super::string_pool::StringPool;
use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use closure_rhino::check_state;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::jsdoc_info::{JSDocInfo, OptimizationTagsKey};
use closure_rhino::jstype::TypeId;
use closure_rhino::node::{Ast, NodeId};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// port: ConvertTypesToColors
///
/// Pass to convert JSType objects from TypeChecking that are attached to the AST into Color
/// objects whose sole use is to enable running optimizations and delete all other references to
/// JSTypes.
///
/// This pass is also responsible for logging debug information that needs to know about both
/// JSType objects and their corresponding colors, and for telling the compiler to prepare for
/// reading runtime libraries from precompiled TypedASTs instead of source.
pub struct ConvertTypesToColors {
    serialization_options: SerializationOptions,
}

impl ConvertTypesToColors {
    // port: ConvertTypesToColors#<init>
    pub fn new(serialization_options: SerializationOptions) -> Self {
        Self {
            serialization_options,
        }
    }
}

/// port: ConvertTypesToColors.RemoveTypes
struct RemoveTypes;

impl RemoveTypes {
    // port: ConvertTypesToColors.RemoveTypes#visit
    #[inline]
    fn visit_node(ast: &mut Ast, n: NodeId, jsdoc_cache: &mut IndexMap<u32, Arc<JSDocInfo>>) {
        use closure_rhino::node::Prop;
        const TYPE_DECL_MASK: u64 = (1 << Prop::TYPE_BEFORE_CAST as u8)
            | (1 << Prop::DECLARED_TYPE_EXPR as u8)
            | (1 << Prop::TYPEDEF_TYPE as u8);
        n.set_jstype(ast, None);
        if ast.node_has_any_prop(n, TYPE_DECL_MASK) {
            n.clear_type_declaration_props(ast);
        }

        if let Some(jsdoc) = n.get_jsdoc_info_ref(ast) {
            // Rust-only (performance divergence): drop JSDocs with no optimization-relevant
            // fields without cloning or round-tripping, and share identical immutable
            // optimization-only `Arc<JSDocInfo>` instances by their tag bitmask.
            match jsdoc.optimization_tags_key() {
                OptimizationTagsKey::Empty => {
                    n.set_jsdoc_info(ast, None);
                }
                OptimizationTagsKey::TagsOnly(mask) => {
                    let converted = if let Some(cached) = jsdoc_cache.get(&mask) {
                        Some(Arc::clone(cached))
                    } else {
                        let jsdoc = n.get_jsdoc_info(ast);
                        let converted = JSDocSerializer::convert_jsdoc_info_for_optimizations(
                            ast,
                            jsdoc.as_deref(),
                        );
                        if let Some(ref info) = converted {
                            jsdoc_cache.insert(mask, Arc::clone(info));
                        }
                        converted
                    };
                    n.set_jsdoc_info(ast, converted);
                }
                OptimizationTagsKey::HasPayload => {
                    let jsdoc = n.get_jsdoc_info(ast);
                    let converted = JSDocSerializer::convert_jsdoc_info_for_optimizations(
                        ast,
                        jsdoc.as_deref(),
                    );
                    n.set_jsdoc_info(ast, converted);
                }
            }
        }
    }

    /// Rust-only (performance divergence): walk `n` directly in post-order without building a
    /// `NodeTraversal`, skipping nodes with no types, type-declaration props, or JSDoc via one
    /// dense bitmask check when the arena is untyped.
    fn traverse(ast: &mut Ast, n: NodeId) {
        let mut jsdoc_cache = IndexMap::default();
        if ast.has_types() {
            Self::traverse_with_cache(ast, n, &mut jsdoc_cache);
        } else {
            Self::traverse_untyped_with_cache(ast, n, &mut jsdoc_cache);
        }
    }

    fn traverse_untyped_with_cache(
        ast: &mut Ast,
        n: NodeId,
        jsdoc_cache: &mut IndexMap<u32, Arc<JSDocInfo>>,
    ) {
        use closure_rhino::node::Prop;
        const RELEVANT_MASK: u64 = (1 << Prop::JSDOC_INFO as u8)
            | (1 << Prop::TYPE_BEFORE_CAST as u8)
            | (1 << Prop::DECLARED_TYPE_EXPR as u8)
            | (1 << Prop::TYPEDEF_TYPE as u8);
        let mut child = n.get_first_child(ast);
        while let Some(c) = child {
            let next = c.get_next(ast);
            Self::traverse_untyped_with_cache(ast, c, jsdoc_cache);
            child = next;
        }
        if ast.node_has_any_prop(n, RELEVANT_MASK) {
            Self::visit_node(ast, n, jsdoc_cache);
        }
    }

    fn traverse_with_cache(
        ast: &mut Ast,
        n: NodeId,
        jsdoc_cache: &mut IndexMap<u32, Arc<JSDocInfo>>,
    ) {
        let mut child = n.get_first_child(ast);
        while let Some(c) = child {
            let next = c.get_next(ast);
            Self::traverse_with_cache(ast, c, jsdoc_cache);
            child = next;
        }
        Self::visit_node(ast, n, jsdoc_cache);
    }
}

/// port: ConvertTypesToColors.RemoveTypesAndApplyColors
struct RemoveTypesAndApplyColors {
    // TypeId is an index in this compiler's type registry. Resolve the immutable shard
    // once per observed type, including colors shared by many nodes.
    colors: Vec<Option<closure_rhino::jscomp_colors::color::Color>>,
    jsdoc_cache: IndexMap<u32, Arc<JSDocInfo>>,
}

impl RemoveTypesAndApplyColors {
    fn new(shard: &ShardView, pointers: &IndexMap<TypeId, i32>) -> Self {
        let len = pointers
            .keys()
            .map(|id| id.0 as usize + 1)
            .max()
            .unwrap_or(0);
        let mut colors = vec![None; len];
        for (id, &pointer) in pointers {
            colors[id.0 as usize] = Some(shard.get_color(pointer));
        }
        Self {
            colors,
            jsdoc_cache: IndexMap::default(),
        }
    }

    // port: ConvertTypesToColors.RemoveTypesAndApplyColors#visit
    /// Rust-only (performance divergence): walk `n` directly in post-order without building a
    /// `NodeTraversal`.
    fn traverse(&mut self, ast: &mut Ast, n: NodeId) {
        let mut child = n.get_first_child(ast);
        while let Some(c) = child {
            let next = c.get_next(ast);
            self.traverse(ast, c);
            child = next;
        }
        let old_type = n.get_jstype(ast);
        let old_type_before_cast = n.get_jstype_before_cast(ast);

        RemoveTypes::visit_node(ast, n, &mut self.jsdoc_cache);

        if let Some(color) = old_type
            .and_then(|id| self.colors.get(id.0 as usize))
            .and_then(Option::as_ref)
        {
            // Rust-only (performance divergence): the color is lent (see `NodeId::set_color_ref`).
            n.set_color_ref(ast, Some(color));
        }

        if old_type_before_cast.is_some() {
            // used by FunctionInjector and InlineVariables when inlining as a hint that a node has
            // a more specific color
            n.set_color_from_type_cast(ast);
        }
    }
}

impl CompilerPass for ConvertTypesToColors {
    // port: ConvertTypesToColors#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        if compiler
            .get_life_cycle_stage()
            .has_color_and_simplified_jsdoc()
        {
            // Pass is a no-op if we already have optimization colors or have finished the checks
            // phase
            return;
        }
        // NOTE(lharker): this pass could probably safely run after normalization, except that the
        // LifeCycleStage enum assumes that normalization implies "colors + simplified JSDoc"
        check_state!(
            !compiler.get_life_cycle_stage().is_normalized(),
            "Not expected to run after normalization"
        );

        let externs_and_js_root = root.get_parent(compiler).unwrap();

        if !compiler.has_type_checking_run() {
            RemoveTypes::traverse(compiler, externs_and_js_root);
            compiler.clear_js_type_registry();
            compiler.init_runtime_library_typed_asts(None);
            return;
        }

        let string_pool_builder = Rc::new(RefCell::new(StringPool::builder()));
        let mut serialize_jstypes = SerializeTypesToPointers::create(
            compiler,
            &string_pool_builder,
            self.serialization_options.clone(),
        );
        serialize_jstypes.gather_types_on_ast(compiler, externs_and_js_root);

        let (type_pool, color_ids, pointers) = serialize_jstypes.into_optimization_shard();
        let strings = Rc::into_inner(string_pool_builder)
            .expect("the completed serializer released the string pool")
            .into_inner()
            .into_interned_strings();

        let mut color_pool_builder = ColorPool::builder();
        compiler.init_runtime_library_typed_asts(Some(&mut color_pool_builder));

        let shard = color_pool_builder.add_optimization_shard(type_pool, color_ids, strings);
        let color_pool = color_pool_builder.build();
        let mut callback = RemoveTypesAndApplyColors::new(&shard, &pointers);
        callback.traverse(compiler, externs_and_js_root);

        compiler.clear_js_type_registry();
        compiler.set_color_registry(Arc::clone(color_pool.get_registry()));
    }
}
