/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ExtractPrototypeMemberDeclarations.java,
//   src/com/google/javascript/jscomp/NodeTraversal.java.

//! Port of ExtractPrototypeMemberDeclarations.java.
//!
//! When there are multiple prototype member declarations to the same class, use a temp variable
//! to alias the prototype object.
//!
//! Example:
//!
//! ```text
//! function B() { ... }                 \
//! B.prototype.foo = function() { ... }  \___ {@link ExtractionInstance}
//! ...                                   /
//! B.prototype.bar = function() { ... } /
//!          ^---------------------------------{@link PrototypeMemberDeclaration}
//! ```
//!
//! becomes
//!
//! ```text
//! function B() { ... }
//! x = B.prototype;
//! x.foo = function() { ... }
//! ...
//! x.bar = function() { ... }
//! ```
//!
//! Works almost like a redundant load elimination but limited to only recognizing the class
//! prototype declaration idiom. First it only works within a basic block because we avoided
//! `DataFlowAnalysis` for compilation performance. Secondly, we can avoid having to compute how
//! long to sub-expressing has to be. Example:
//!
//! ```text
//! a.b.c.d = ...
//! a.b.c = ...
//! a.b = ...
//! a.b.c = ...
//! ```
//!
//! Further more, we only introduce one temp variable to hold a single prototype at a time. So all
//! the `PrototypeMemberDeclaration` to be extracted must be in a single line. We call this a
//! single `ExtractionInstance`.
//!
//! Alternatively, for users who do not want a global variable to be introduced, we will create an
//! anonymous function instead.
//!
//! ```text
//! function B() { ... }
//! (function (x) {
//!   x.foo = function() { ... }
//!   ...
//!   x.bar = function() { ... }
//! )(B.prototype)
//! ```
//!
//! The RHS of the declarations can have side effects, however, one good way to break this is the
//! following:
//!
//! ```text
//! function B() { ... }
//! B.prototype.foo = (function() { B.prototype = somethingElse(); return 0 })();
//! ...
//! ```
//!
//! Such logic is highly unlikely and we will assume that it never occurs.
use crate::{
    AbstractCompiler,
    compiler_pass::CompilerPass,
    js_chunk::JSChunk,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{
    check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
};
use indexmap::IndexMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pattern {
    USE_GLOBAL_TEMP,
    USE_CHUNK_TEMP,
    USE_IIFE,
}

impl Pattern {
    // port: ExtractPrototypeMemberDeclarations.Pattern#globalOverhead
    fn global_overhead(self) -> i32 {
        self.overheads().0
    }

    // port: ExtractPrototypeMemberDeclarations.Pattern#perExtractionOverhead
    fn per_extraction_overhead(self) -> i32 {
        self.overheads().1
    }

    // port: ExtractPrototypeMemberDeclarations.Pattern#perMemberOverhead
    fn per_member_overhead(self) -> i32 {
        self.overheads().2
    }

    /// The constructor arguments of each Java enum constant: (globalOverHead,
    /// perExtractionOverhead, perMemberOverhead).
    // port: ExtractPrototypeMemberDeclarations.Pattern#Pattern
    const fn overheads(self) -> (i32, i32, i32) {
        match self {
            Self::USE_GLOBAL_TEMP => (
                // Global Overhead.
                // We need a temp variable to hold all the prototype.
                "var t;".len() as i32,
                // Per Extract overhead:
                // Every extraction instance must first use the temp variable to point
                // to the prototype object.
                "t=y.prototype;".len() as i32,
                // TODO(user): Check to to see if AliasExterns is on
                // The gain we get per prototype declaration. Assuming it can be
                // aliased.
                "t.y=".len() as i32 - "x[p].y=".len() as i32,
            ),
            Self::USE_CHUNK_TEMP => (
                // Per Chunk Overhead.
                // We need a temp variable to hold all the prototype.
                "var t;".len() as i32,
                // Per Extract overhead:
                // Every extraction instance must first use the temp variable to point
                // to the prototype object.
                "t=y.prototype;".len() as i32,
                // TODO(user): Check to to see if AliasExterns is on
                // The gain we get per prototype declaration. Assuming it can be
                // aliased.
                "t.y=".len() as i32 - "x[p].y=".len() as i32,
            ),
            Self::USE_IIFE => (
                // Global Overhead:
                0,
                // Per-extraction overhead:
                // This is the cost of a single anoynmous function.
                "(function(t){})(y.prototype);".len() as i32,
                // Per-prototype member declaration overhead:
                // Here we assumes that they don't have AliasExterns on (in SIMPLE mode).
                "t.y=".len() as i32 - "x.prototype.y=".len() as i32,
            ),
        }
    }
}

pub struct ExtractPrototypeMemberDeclarations {
    pattern: Pattern,
}

impl ExtractPrototypeMemberDeclarations {
    // The name of variable that will temporary hold the pointer to the prototype
    // object. Of course, we assume that it'll be renamed by RenameVars.
    // port: ExtractPrototypeMemberDeclarations#PROTOTYPE_ALIAS
    const PROTOTYPE_ALIAS: &'static str = "JSCompiler_prototypeAlias";

    // port: ExtractPrototypeMemberDeclarations#ExtractPrototypeMemberDeclarations
    pub fn new(pattern: Pattern) -> Self {
        Self { pattern }
    }

    // port: ExtractPrototypeMemberDeclarations#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        let mut extraction_info = GatherExtractionInfo {
            pattern: self.pattern,
            instances_by_chunk: IndexMap::new(),
        };
        NodeTraversal::traverse(compiler, root, &mut extraction_info);
        self.maybe_do_extraction(compiler, &extraction_info);
    }

    /// Declares the temp variable to point to prototype objects and iterates through all
    /// ExtractInstance and performs extraction there.
    // port: ExtractPrototypeMemberDeclarations#maybeDoExtraction
    fn maybe_do_extraction(&self, compiler: &mut AbstractCompiler, info: &GatherExtractionInfo) {
        if (self.pattern == Pattern::USE_IIFE || self.pattern == Pattern::USE_GLOBAL_TEMP)
            && !info.should_extract_global()
        {
            return;
        }
        if self.pattern == Pattern::USE_GLOBAL_TEMP {
            let injection_point = compiler.get_node_for_code_insertion(None);

            let var = NodeUtil::new_var_node(compiler, Self::PROTOTYPE_ALIAS, None)
                .srcref_tree_if_missing(compiler, injection_point);

            injection_point.add_child_to_front(compiler, var);
            compiler.report_change_to_enclosing_scope(var);
        }
        // Go through all extraction instances and extract each of them.
        for (chunk, instance_info) in &info.instances_by_chunk {
            let mut alias = JsString::from(Self::PROTOTYPE_ALIAS);
            if self.pattern == Pattern::USE_CHUNK_TEMP {
                // Rather than a truly global variable, use a unique variable per output chunk.
                // This prevents RescopeGlobalSymbolNames from converting these references to
                // namespace properties which reduces the benefit of the alias.
                if info.should_extract_chunk(chunk.as_ref()) {
                    let injection_point = compiler.get_node_for_code_insertion(chunk.as_ref());
                    alias = JsString::from(format!(
                        "{}{}",
                        Self::PROTOTYPE_ALIAS,
                        chunk.as_ref().unwrap().get_index()
                    ));
                    let var = NodeUtil::new_var_node(compiler, alias.clone(), None)
                        .srcref_tree_if_missing(compiler, injection_point);

                    injection_point.add_child_to_front(compiler, var);
                    compiler.report_change_to_enclosing_scope(var);
                } else {
                    continue;
                }
            }

            for instance in &instance_info.instances {
                self.extract_instance(compiler, instance, &alias);
            }
        }
    }

    /// At a given ExtractionInstance, stores and prototype object in the temp variable and rewrite
    /// each member declaration to assign to the temp variable instead.
    // port: ExtractPrototypeMemberDeclarations#extractInstance
    fn extract_instance(
        &self,
        compiler: &mut AbstractCompiler,
        instance: &ExtractionInstance,
        alias: &JsString,
    ) {
        let first = &instance.declarations[0];
        let class_name = first.qualified_class_name.clone();
        if self.pattern == Pattern::USE_GLOBAL_TEMP || self.pattern == Pattern::USE_CHUNK_TEMP {
            // Use the temp variable to hold the prototype.
            let class_name_node = NodeUtil::new_qname(compiler, class_name);
            class_name_node.put_boolean_prop(compiler, NodeId::IS_CONSTANT_NAME, first.constant);
            let alias_name = IR::name(compiler, alias.clone());
            let prototype = IR::getprop(compiler, class_name_node, "prototype");
            let assign = IR::assign(compiler, alias_name, prototype);
            let stmt =
                IR::expr_result(compiler, assign).srcref_tree_if_missing(compiler, first.node);

            stmt.insert_before(compiler, first.node);
            compiler.report_change_to_enclosing_scope(stmt);
        } else if self.pattern == Pattern::USE_IIFE {
            let block = IR::block(compiler);
            let fn_name = IR::name(compiler, "");
            let param = IR::name(compiler, alias.clone());
            let params = IR::param_list(compiler, &[param]);
            let func = IR::function(compiler, fn_name, params, block);

            let prototype_name = class_name.concat(&JsString::from(".prototype"));
            let prototype = NodeUtil::new_qname_with_basis(
                compiler,
                prototype_name.clone(),
                instance.parent,
                prototype_name,
            );
            let call = IR::call(compiler, func, &[prototype]);
            call.put_int_prop(compiler, NodeId::FREE_CALL, 1);

            let stmt = IR::expr_result(compiler, call);
            stmt.srcref_tree_if_missing(compiler, first.node);
            stmt.insert_before(compiler, first.node);
            compiler.report_change_to_enclosing_scope(stmt);
            for declar in &instance.declarations {
                compiler.report_change_to_enclosing_scope(declar.node);
                let detached = declar.node.detach(compiler);
                block.add_child_to_back(compiler, detached);
            }
        }
        // Go through each member declaration and replace it with an assignment
        // to the prototype variable.
        for declar in &instance.declarations {
            self.replace_prototype_member_declaration(compiler, declar, alias);
        }
    }

    /// Replaces a member declaration to an assignment to the temp prototype object.
    // port: ExtractPrototypeMemberDeclarations#replacePrototypeMemberDeclaration
    fn replace_prototype_member_declaration(
        &self,
        compiler: &mut AbstractCompiler,
        declar: &PrototypeMemberDeclaration,
        alias: &JsString,
    ) {
        // x.prototype.y = ...  ->  t.y = ...
        let assignment = declar.node.get_first_child(compiler).unwrap();
        let lhs = assignment.get_first_child(compiler).unwrap();
        let name = NodeUtil::new_qname_with_basis(
            compiler,
            alias
                .concat(&JsString::from("."))
                .concat(&declar.member_name),
            declar.node,
            declar.member_name.clone(),
        );

        // Save the full prototype path on the left hand side of the assignment for debugging
        // purposes.
        // declar.lhs = x.prototype.y so first child of the first child is 'x'.
        let access_node = declar.lhs.get_first_first_child(compiler).unwrap();
        let original_name = access_node.get_original_name(compiler);
        let class_name = match original_name {
            Some(original_name) => original_name,
            None => JsString::from("?"),
        };
        name.get_first_child(compiler)
            .unwrap()
            .srcref_tree(compiler, lhs);
        let is_constant_name = lhs.get_boolean_prop(compiler, NodeId::IS_CONSTANT_NAME);
        name.put_boolean_prop(compiler, NodeId::IS_CONSTANT_NAME, is_constant_name);
        name.get_first_child(compiler).unwrap().set_original_name(
            compiler,
            Some(class_name.concat(&JsString::from(".prototype"))),
        );

        lhs.replace_with(compiler, name);
        compiler.report_change_to_enclosing_scope(name);
    }
}

impl CompilerPass for ExtractPrototypeMemberDeclarations {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        ExtractPrototypeMemberDeclarations::process(self, compiler, externs, root);
    }
}

/// Per-chunk info needed for prototype extraction
struct ExtractionInstanceInfo {
    instances: Vec<ExtractionInstance>,
    total_delta: i32,
}

impl ExtractionInstanceInfo {
    // port: ExtractPrototypeMemberDeclarations.ExtractionInstanceInfo#ExtractionInstanceInfo
    fn new() -> Self {
        Self {
            instances: Vec::new(),
            total_delta: 0,
        }
    }
}

/// Collects all the possible extraction instances in a node traversal.
struct GatherExtractionInfo {
    /// The enclosing instance's `pattern`.
    pattern: Pattern,
    instances_by_chunk: IndexMap<Option<JSChunk>, ExtractionInstanceInfo>,
}

impl GatherExtractionInfo {
    /// Returns `true` if the sum of all the extraction instance gain outweighs the overhead of the
    /// temp variable declaration.
    // port: ExtractPrototypeMemberDeclarations.GatherExtractionInfo#shouldExtractGlobal
    fn should_extract_global(&self) -> bool {
        let mut all_modules_delta = 0;
        for instance_info in self.instances_by_chunk.values() {
            all_modules_delta += instance_info.total_delta;
        }
        all_modules_delta + self.pattern.global_overhead() < 0
    }

    /// Returns `true` if the sum of all the extraction instance gain outweighs the overhead of the
    /// temp variable declaration.
    // port: ExtractPrototypeMemberDeclarations.GatherExtractionInfo#shouldExtractChunk
    fn should_extract_chunk(&self, chunk: Option<&JSChunk>) -> bool {
        let instance_info = self.instances_by_chunk.get(&chunk.cloned());
        let Some(instance_info) = instance_info else {
            return false;
        };
        instance_info.total_delta + self.pattern.global_overhead() < 0
    }
}

impl Callback for GatherExtractionInfo {
    // port: NodeTraversal.AbstractShallowCallback#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        // We do want to traverse the name of a named function, but we don't
        // want to traverse the arguments or body.
        match parent {
            None => true,
            Some(parent) => !parent.is_function(t) || Some(n) == parent.get_first_child(t),
        }
    }

    // port: ExtractPrototypeMemberDeclarations.GatherExtractionInfo#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if !n.is_script(t) && !n.is_block(t) {
            return;
        }

        let mut cur = n.get_first_child(t);
        while let Some(c) = cur {
            let prototype_member = PrototypeMemberDeclaration::extract_declaration(t, c);
            let Some(prototype_member) = prototype_member else {
                cur = c.get_next(t);
                continue;
            };

            // Found a good site here. The constructor will computes the chain of
            // declarations that is qualified for extraction.
            let instance = ExtractionInstance::new(t, self.pattern, prototype_member, n);
            let c = instance.declarations.last().unwrap().node;

            // Only add it to our work list if the extraction at this instance makes the code
            // smaller.
            if instance.is_favorable() {
                let chunk = t.get_chunk();
                self.instances_by_chunk
                    .entry(chunk.clone())
                    .or_insert_with(ExtractionInstanceInfo::new);
                let instance_info = self.instances_by_chunk.get_mut(&chunk).unwrap();
                instance_info.total_delta += instance.delta;
                instance_info.instances.push(instance);
            }
            cur = c.get_next(t);
        }
    }
}

struct ExtractionInstance {
    declarations: Vec<PrototypeMemberDeclaration>,
    delta: i32,
    parent: NodeId,
}

impl ExtractionInstance {
    // port: ExtractPrototypeMemberDeclarations.ExtractionInstance#ExtractionInstance
    fn new(ast: &Ast, pattern: Pattern, head: PrototypeMemberDeclaration, parent: NodeId) -> Self {
        let mut cur = head.node.get_next(ast);
        let mut this = Self {
            declarations: Vec::new(),
            delta: 0,
            parent,
        };
        this.delta = pattern.per_extraction_overhead() + pattern.per_member_overhead();
        this.declarations.push(head);

        while let Some(c) = cur {
            // We can skip over any named functions because they have no effect on
            // the control flow. In fact, they are lifted to the beginning of the
            // block. This happens a lot when devirtualization breaks the whole chain.
            if c.is_function(ast) {
                cur = c.get_next(ast);
                continue;
            }

            let prototype_member = PrototypeMemberDeclaration::extract_declaration(ast, c);
            let Some(prototype_member) = prototype_member else {
                break;
            };
            if !this.declarations[0].is_same_class(&prototype_member) {
                break;
            }
            this.declarations.push(prototype_member);
            this.delta += pattern.per_member_overhead();
            cur = c.get_next(ast);
        }
        this
    }

    /// Returns `true` if extracting all the declarations at this instance will overweight the
    /// overhead of aliasing the prototype object.
    // port: ExtractPrototypeMemberDeclarations.ExtractionInstance#isFavorable
    fn is_favorable(&self) -> bool {
        self.delta <= 0
    }
}

/// Abstraction for a prototype member declaration.
///
/// `a.b.c.prototype.d = ....`
struct PrototypeMemberDeclaration {
    member_name: JsString,
    node: NodeId,
    qualified_class_name: JsString,
    lhs: NodeId,
    constant: bool,
}

impl PrototypeMemberDeclaration {
    // port: ExtractPrototypeMemberDeclarations.PrototypeMemberDeclaration#PrototypeMemberDeclaration
    fn new(ast: &Ast, lhs: NodeId, node: NodeId) -> Self {
        check_state!(
            NodeUtil::is_expr_assign(ast, node),
            "%s",
            node.to_string(ast)
        );
        let member_name = NodeUtil::get_prototype_property_name(ast, lhs);
        let class_node = Self::get_prototype_class_name(ast, lhs).unwrap();
        let qualified_class_name = class_node.get_qualified_name(ast).unwrap();
        let constant = class_node.get_boolean_prop(ast, NodeId::IS_CONSTANT_NAME);
        Self {
            member_name,
            node,
            qualified_class_name,
            lhs,
            constant,
        }
    }

    // port: ExtractPrototypeMemberDeclarations.PrototypeMemberDeclaration#isSameClass
    fn is_same_class(&self, other: &PrototypeMemberDeclaration) -> bool {
        self.qualified_class_name == other.qualified_class_name
    }

    // port: ExtractPrototypeMemberDeclarations.PrototypeMemberDeclaration#getPrototypeClassName
    fn get_prototype_class_name(ast: &Ast, q_name: NodeId) -> Option<NodeId> {
        let mut cur = q_name;
        while cur.is_get_prop(ast) {
            if cur.get_string_ref(ast) == "prototype" {
                return cur.get_first_child(ast);
            } else {
                cur = cur.get_first_child(ast).unwrap();
            }
        }
        None
    }

    // port: ExtractPrototypeMemberDeclarations.PrototypeMemberDeclaration#isPrototypePropertyDeclaration
    fn is_prototype_property_declaration(ast: &Ast, n: NodeId) -> bool {
        if !NodeUtil::is_expr_assign(ast, n) {
            return false;
        }
        let lvalue = n.get_first_first_child(ast).unwrap();
        if lvalue.is_get_prop(ast) {
            let mut cur = lvalue.get_first_child(ast).unwrap();
            while cur.is_get_prop(ast) {
                if cur.get_string_ref(ast) == "prototype" {
                    return cur.is_qualified_name(ast);
                }
                cur = cur.get_first_child(ast).unwrap();
            }
        }
        false
    }

    /// Returns a prototype member declaration representation if there is one else it returns
    /// `null`.
    // port: ExtractPrototypeMemberDeclarations.PrototypeMemberDeclaration#extractDeclaration
    fn extract_declaration(ast: &Ast, n: NodeId) -> Option<PrototypeMemberDeclaration> {
        if !Self::is_prototype_property_declaration(ast, n) {
            return None;
        }
        let lhs = n.get_first_first_child(ast).unwrap();
        Some(PrototypeMemberDeclaration::new(ast, lhs, n))
    }
}
