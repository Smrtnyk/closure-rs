/*
 * Copyright 2004 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/serialization/ConvertTypesToColors.java.

//! Port of serialization/ConvertTypesToColors.java.
use super::color_pool::{ColorPool, ShardView};
use super::jsdoc_serializer::JSDocSerializer;
use super::serialization_options::SerializationOptions;
use super::serialize_types_to_pointers::SerializeTypesToPointers;
use super::string_pool::StringPool;
use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::node_traversal::{Callback, NodeTraversal};
use closure_rhino::check_state;
use closure_rhino::jstype::TypeId;
use closure_rhino::node::{Ast, NodeId};
use indexmap::IndexMap;
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
    fn visit_node(ast: &mut Ast, n: NodeId) {
        n.set_jstype(ast, None);
        n.set_jstype_before_cast(ast, None);
        n.set_declared_type_expression(ast, None);
        n.set_typedef_type_prop(ast, None);

        let jsdoc = n.get_jsdoc_info(ast);
        if let Some(jsdoc) = jsdoc {
            let converted =
                JSDocSerializer::convert_jsdoc_info_for_optimizations(ast, Some(&jsdoc));
            n.set_jsdoc_info(ast, converted);
        }
    }
}

impl Callback for RemoveTypes {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ConvertTypesToColors.RemoveTypes#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let ast: &mut Ast = t.get_compiler();
        Self::visit_node(ast, n);
    }
}

/// port: ConvertTypesToColors.RemoveTypesAndApplyColors
struct RemoveTypesAndApplyColors<'a> {
    color_pool_shard: Arc<ShardView>,
    type_pointers_by_jstype: &'a IndexMap<TypeId, i32>,
}

impl<'a> RemoveTypesAndApplyColors<'a> {
    // port: ConvertTypesToColors.RemoveTypesAndApplyColors#<init>
    fn new(
        color_pool_shard: Arc<ShardView>,
        type_pointers_by_jstype: &'a IndexMap<TypeId, i32>,
    ) -> Self {
        Self {
            color_pool_shard,
            type_pointers_by_jstype,
        }
    }
}

impl Callback for RemoveTypesAndApplyColors<'_> {
    // port: NodeTraversal.AbstractPostOrderCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        _t: &mut NodeTraversal<'_>,
        _n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        true
    }

    // port: ConvertTypesToColors.RemoveTypesAndApplyColors#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let ast: &mut Ast = t.get_compiler();
        let old_type = n.get_jstype(ast);
        let old_type_before_cast = n.get_jstype_before_cast(ast);

        RemoveTypes::visit_node(ast, n);

        if let Some(old_type) = old_type
            && let Some(&pointer) = self.type_pointers_by_jstype.get(&old_type)
        {
            n.set_color(ast, Some(self.color_pool_shard.get_color(pointer)));
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
            NodeTraversal::traverse(compiler, externs_and_js_root, &mut RemoveTypes);
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

        let type_pool = Arc::new(serialize_jstypes.get_type_pool().unwrap().clone());
        let string_pool = Arc::new(string_pool_builder.borrow().build());

        let mut color_pool_builder = ColorPool::builder();
        compiler.init_runtime_library_typed_asts(Some(&mut color_pool_builder));

        let mut callback = RemoveTypesAndApplyColors::new(
            color_pool_builder.add_shard(type_pool, string_pool),
            serialize_jstypes.get_type_pointers_by_jstype(),
        );

        let color_pool = color_pool_builder.build();
        NodeTraversal::traverse(compiler, externs_and_js_root, &mut callback);

        compiler.clear_js_type_registry();
        compiler.set_color_registry(Arc::clone(color_pool.get_registry()));
    }
}
