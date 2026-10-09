/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ijs/PotentialDeclaration.java.

//! Port of `com.google.javascript.jscomp.ijs.PotentialDeclaration`: encapsulates something that
//! could be a declaration.
//!
//! This includes: var/let/const declarations, function/class declarations, method declarations,
//! assignments, goog.define calls, and even valueless property accesses (e.g.
//! `/** @type {number} */ Foo.prototype.bar`)
//!
//! Java's abstract class and its private subclasses become one struct with the common fields and
//! a `PotentialDeclarationKind` naming the subclass; overridden methods dispatch on the kind, and
//! Java `super.` calls invoke the base bodies (`*_base`) directly.

use crate::{
    abstract_compiler::AbstractCompiler, ijs::class_util::ClassUtil, ijs::jsdoc_util::JsdocUtil,
    node_util::NodeUtil,
};
use closure_rhino::{
    check_argument, check_not_null, check_state,
    ir::IR,
    js_string::JsString,
    js_type_expression::JSTypeExpression,
    jsdoc_info::JSDocInfo,
    node::{Ast, NodeId},
    qualified_name::QualifiedName,
    token::Token,
};
use std::sync::{Arc, LazyLock};

// The threshold for trunctating string enum items. Any string literal longer than this will get
// truncated in the generated IJS file. This value is decided by the length of the longest
// security-sensitive attribute name.
// port: PotentialDeclaration#STRING_ENUM_RETAIN_CAP
const STRING_ENUM_RETAIN_CAP: usize = 10;

// port: PotentialDeclaration#GOOG_ABSTRACTMETHOD
static GOOG_ABSTRACTMETHOD: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.abstractMethod"));
// port: PotentialDeclaration#GOOG_REQUIRE
static GOOG_REQUIRE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.require"));
// port: PotentialDeclaration#GOOG_REQUIRETYPE
static GOOG_REQUIRETYPE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.requireType"));
// port: PotentialDeclaration#GOOG_FORWARDDECLARE
static GOOG_FORWARDDECLARE: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.forwardDeclare"));
// port: PotentialDeclaration#MODULE_EXPORTS
static MODULE_EXPORTS: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("module.exports"));
// port: PotentialDeclaration#GOOG_MODULE_GET
static GOOG_MODULE_GET: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.module.get"));

/// The Java subclass of a `PotentialDeclaration`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PotentialDeclarationKind {
    /// A potential declaration that has a fully qualified name to describe it. This includes
    /// things like: var/let/const/function/class declarations, assignments to a fully qualified
    /// name, and goog.module exports This is the most common type of potential declaration.
    NameDeclaration,
    /// A declaration of a property on `this` inside a constructor.
    ThisPropDeclaration {
        insertion_point: Option<NodeId>,
    },
    /// A declaration declared by a call to `goog.define`. Note that a let, const, or var
    /// declaration annotated with @define in its JSDoc and no 'goog.define' would be a
    /// NameDeclaration instead.
    DefineDeclaration,
    /// A declaration of a method defined using the ES6 method syntax. Note that a method defined
    /// as an assignment to a prototype property would be a NameDeclaration instead.
    MethodDeclaration,
    MemberFieldDeclaration,
    StringKeyDeclaration,
    /// Polymer Behaviors are mixin-like objects used in Polymer 1 for multiple inheritance. For
    /// incremental compilation, it is important that the "properties" object is preserved,
    /// because the PolymerPass injects the properties declared there onto the prototypes of the
    /// Polymer elements that apply that behavior.
    PolymerBehaviorPropertiesDeclaration,
    AliasDeclaration,
}
use PotentialDeclarationKind as Kind;

#[derive(Clone, Debug)]
pub struct PotentialDeclaration {
    // The fully qualified name of the declaration.
    fully_qualified_name: JsString,
    // The LHS node of the declaration.
    lhs: NodeId,
    // The RHS node of the declaration, if it exists.
    rhs: Option<NodeId>,
    kind: PotentialDeclarationKind,
}

impl PotentialDeclaration {
    // port: PotentialDeclaration#PotentialDeclaration
    fn new(
        fully_qualified_name: Option<JsString>,
        lhs: Option<NodeId>,
        rhs: Option<NodeId>,
        kind: PotentialDeclarationKind,
    ) -> Self {
        Self {
            fully_qualified_name: check_not_null!(fully_qualified_name),
            lhs: check_not_null!(lhs),
            rhs,
            kind,
        }
    }

    pub fn get_kind(&self) -> PotentialDeclarationKind {
        self.kind
    }

    // port: PotentialDeclaration#fromName
    pub fn from_name(ast: &Ast, name_node: NodeId) -> Self {
        check_argument!(
            name_node.is_qualified_name(ast),
            "{}",
            name_node.to_string(ast)
        );
        let rhs = NodeUtil::get_r_value_of_l_value(ast, name_node);
        if ClassUtil::is_this_prop_inside_class_with_name(ast, name_node) {
            let name = ClassUtil::get_fully_qualified_name_of_this_prop(ast, name_node);
            return Self::new_this_prop_declaration(ast, name, name_node, rhs);
        }
        Self::new(
            name_node.get_qualified_name(ast),
            Some(name_node),
            rhs,
            Kind::NameDeclaration,
        )
    }

    // port: PotentialDeclaration#fromMemberFieldDef
    pub fn from_member_field_def(ast: &Ast, field_node: NodeId) -> Self {
        check_argument!(field_node.is_member_field_def(ast));
        let rhs = NodeUtil::get_r_value_of_l_value(ast, field_node);
        let name = ClassUtil::get_fully_qualified_name_of_member_field_def(ast, field_node);
        // lhs is MEMBER_FIELD_DEF  rhs is the value assigned to the field if any.
        Self::new(
            Some(name),
            Some(field_node),
            rhs,
            Kind::MemberFieldDeclaration,
        )
    }

    // port: PotentialDeclaration#fromMethod
    pub fn from_method(ast: &Ast, function_node: NodeId) -> Self {
        check_argument!(ClassUtil::is_class_method(ast, function_node));
        let name = ClassUtil::get_fully_qualified_name_of_method(ast, function_node);
        // port: PotentialDeclaration.MethodDeclaration#MethodDeclaration
        Self::new(
            Some(name),
            function_node.get_parent(ast),
            Some(function_node),
            Kind::MethodDeclaration,
        )
    }

    // port: PotentialDeclaration#fromStringKey
    pub fn from_string_key(ast: &Ast, string_key_node: NodeId) -> Self {
        check_argument!(string_key_node.is_string_key(ast));
        check_argument!(string_key_node.get_parent(ast).unwrap().is_object_lit(ast));
        let name = JsString::from("this.").concat(&string_key_node.get_string(ast));
        if string_key_node.get_string_ref(ast) == "properties" {
            let obj_lit_js_doc =
                NodeUtil::get_best_jsdoc_info(ast, string_key_node.get_parent(ast).unwrap());
            if obj_lit_js_doc.is_some_and(|d| d.is_polymer_behavior()) {
                // port: PotentialDeclaration.PolymerBehaviorPropertiesDeclaration#PolymerBehaviorPropertiesDeclaration
                return Self::new(
                    Some(name),
                    Some(string_key_node),
                    string_key_node.get_last_child(ast),
                    Kind::PolymerBehaviorPropertiesDeclaration,
                );
            }
        }
        // port: PotentialDeclaration.StringKeyDeclaration#StringKeyDeclaration
        Self::new(
            Some(name),
            Some(string_key_node),
            string_key_node.get_last_child(ast),
            Kind::StringKeyDeclaration,
        )
    }

    // port: PotentialDeclaration#fromDefine
    pub fn from_define(ast: &Ast, call_node: NodeId) -> Self {
        check_argument!(NodeUtil::is_call_to(ast, call_node, "goog.define"));
        Self::define_declaration_from(ast, call_node)
    }

    // port: PotentialDeclaration#fromAlias
    pub fn from_alias(ast: &Ast, name_node: NodeId) -> Self {
        check_argument!(
            name_node.is_qualified_name(ast),
            "{}",
            name_node.to_string(ast)
        );
        // port: PotentialDeclaration.AliasDeclaration#AliasDeclaration
        Self::new(
            name_node.get_qualified_name(ast),
            Some(name_node),
            None,
            Kind::AliasDeclaration,
        )
    }

    // port: PotentialDeclaration#getFullyQualifiedName
    pub fn get_fully_qualified_name(&self) -> &JsString {
        &self.fully_qualified_name
    }

    // port: PotentialDeclaration#getLhs
    pub fn get_lhs(&self) -> NodeId {
        self.lhs
    }

    // port: PotentialDeclaration#getRhs
    pub fn get_rhs(&self) -> Option<NodeId> {
        self.rhs
    }

    // port: PotentialDeclaration#getJsDoc
    pub fn get_js_doc(&self, ast: &Ast) -> Option<Arc<JSDocInfo>> {
        NodeUtil::get_best_jsdoc_info(ast, self.lhs)
    }

    // port: PotentialDeclaration#isDetached
    pub fn is_detached(&self, ast: &Ast) -> bool {
        let mut current = Some(self.lhs);
        while let Some(c) = current {
            if c.is_script(ast) {
                return false;
            }
            current = c.get_parent(ast);
        }
        true
    }

    // port: PotentialDeclaration#getRemovableNode
    pub fn get_removable_node(&self, ast: &Ast) -> Option<NodeId> {
        match self.kind {
            // port: PotentialDeclaration.MethodDeclaration#getRemovableNode
            Kind::MethodDeclaration
            // port: PotentialDeclaration.MemberFieldDeclaration#getRemovableNode
            | Kind::MemberFieldDeclaration
            // port: PotentialDeclaration.StringKeyDeclaration#getRemovableNode
            | Kind::StringKeyDeclaration
            // port: PotentialDeclaration.PolymerBehaviorPropertiesDeclaration#getRemovableNode
            | Kind::PolymerBehaviorPropertiesDeclaration => Some(self.get_lhs()),
            Kind::AliasDeclaration => Some(self.alias_get_removable_node(ast)),
            _ => NodeUtil::get_enclosing_statement(ast, self.lhs),
        }
    }

    /// Remove this "potential declaration" completely. Usually, this is because the same symbol
    /// has already been declared in this file.
    // port: PotentialDeclaration#remove
    pub fn remove(&self, compiler: &mut AbstractCompiler) {
        if self.is_detached(compiler) {
            return;
        }
        let statement = self.get_removable_node(compiler).unwrap();
        NodeUtil::delete_node(compiler, statement);
        statement.remove_children(compiler);
    }

    /// Simplify this declaration to only include what's necessary for typing. Usually, this means
    /// removing the RHS and leaving a type annotation.
    // port: PotentialDeclaration#simplify
    pub fn simplify(&self, compiler: &mut AbstractCompiler) {
        match self.kind {
            Kind::NameDeclaration => self.name_simplify(compiler),
            Kind::ThisPropDeclaration { insertion_point } => {
                self.this_prop_simplify(compiler, insertion_point)
            }
            Kind::DefineDeclaration => self.define_simplify(compiler),
            // port: PotentialDeclaration.MethodDeclaration#simplify
            Kind::MethodDeclaration => {}
            Kind::MemberFieldDeclaration => self.member_field_simplify(compiler),
            Kind::StringKeyDeclaration => self.string_key_simplify(compiler),
            Kind::PolymerBehaviorPropertiesDeclaration => {
                self.polymer_behavior_properties_simplify(compiler)
            }
            // port: PotentialDeclaration.AliasDeclaration#simplify
            Kind::AliasDeclaration => {
                // Does not simplify
            }
        }
    }

    /// Breaks down this declaration if it's a destructuring LHS by replacing it with a VAR node
    /// and removing the RHS.
    ///
    /// Returns true if the declaration is broken down. Otherwise, returns false.
    // port: PotentialDeclaration#breakDownDestructure
    pub fn break_down_destructure(&self, compiler: &mut AbstractCompiler) -> bool {
        match self.kind {
            Kind::NameDeclaration => self.name_break_down_destructure(compiler),
            _ => false,
        }
    }

    // port: PotentialDeclaration.NameDeclaration#simplifyNamespace
    fn name_simplify_namespace(&self, compiler: &mut AbstractCompiler) {
        let rhs = self.get_rhs().unwrap();
        if rhs.is_or(compiler) {
            let obj_lit = rhs.get_last_child(compiler).unwrap().detach(compiler);
            rhs.replace_with(compiler, obj_lit);
            compiler.report_change_to_enclosing_scope(self.get_lhs());
        }
    }

    // port: PotentialDeclaration.NameDeclaration#simplifySymbol
    fn name_simplify_symbol(&self, compiler: &mut AbstractCompiler) {
        check_argument!(NodeUtil::is_call_to(
            compiler,
            self.get_rhs().unwrap(),
            "Symbol"
        ));
        let call_node = self.get_rhs().unwrap();
        while call_node.has_more_than_one_child(compiler) {
            let last = call_node.get_last_child(compiler).unwrap();
            NodeUtil::delete_node(compiler, last);
        }
    }

    // port: PotentialDeclaration.NameDeclaration#simplify
    fn name_simplify(&self, compiler: &mut AbstractCompiler) {
        if self.get_rhs().is_none() || self.should_preserve(compiler) {
            return;
        }
        let rhs = self.get_rhs().unwrap();
        let name_node = self.get_lhs();
        let mut jsdoc = self.get_js_doc(compiler);
        if jsdoc.as_ref().is_some_and(|j| j.has_enum_parameter_type()) {
            self.simplify_enum_values(compiler);
            return;
        }
        if NodeUtil::is_namespace_decl(compiler, name_node) {
            self.name_simplify_namespace(compiler);
            return;
        }
        if name_node.matches_name(compiler, "exports") {
            // Replace the RHS of a default goog.module export with Unknown
            Self::replace_rhs_with_unknown(compiler, rhs);
            compiler.report_change_to_enclosing_scope(name_node);
            return;
        }
        if NodeUtil::is_call_to(compiler, rhs, "Symbol") {
            self.name_simplify_symbol(compiler);
            return;
        }
        if self
            .get_lhs()
            .get_parent(compiler)
            .unwrap()
            .is_const(compiler)
        {
            jsdoc = JsdocUtil::mark_constant(jsdoc.as_deref());
        }
        // Just completely remove the RHS, and replace with a getprop.
        let qualified_name = name_node.get_qualified_name(compiler).unwrap();
        let new_statement =
            NodeUtil::new_qname_declaration_var(compiler, qualified_name, None, jsdoc);
        new_statement.srcref_tree_if_missing(compiler, name_node);
        let old_statement = self.get_removable_node(compiler).unwrap();
        NodeUtil::delete_children(compiler, old_statement);
        if old_statement.is_export(compiler) {
            old_statement.add_child_to_back(compiler, new_statement);
        } else {
            old_statement.replace_with(compiler, new_statement);
        }
        compiler.report_change_to_enclosing_scope(new_statement);
    }

    // port: PotentialDeclaration.NameDeclaration#breakDownDestructure
    fn name_break_down_destructure(&self, compiler: &mut AbstractCompiler) -> bool {
        if !NodeUtil::is_lhs_by_destructuring(compiler, self.get_lhs()) {
            return false;
        }

        let root_target = NodeUtil::get_root_target(compiler, self.get_lhs());

        check_state!(
            root_target
                .get_parent(compiler)
                .unwrap()
                .is_destructuring_lhs(compiler)
        );
        if !NodeUtil::is_name_declaration(compiler, root_target.get_grandparent(compiler)) {
            return false;
        }

        let definition_node = root_target.get_grandparent(compiler).unwrap();

        let mut prev: Option<NodeId> = None;

        let mut lhs_nodes: Vec<NodeId> = Vec::new();
        let assigning_parent = root_target.get_parent(compiler).unwrap();
        NodeUtil::visit_lhs_nodes_in_node(compiler, assigning_parent, &mut |_, n| {
            lhs_nodes.push(n)
        });
        for n in lhs_nodes {
            n.detach(compiler);
            let temp = IR::var(compiler, n);
            match prev {
                None => {
                    definition_node.replace_with(compiler, temp);
                    compiler.report_change_to_enclosing_scope(temp);
                }
                Some(prev) => {
                    temp.insert_after(compiler, prev);
                    compiler.report_change_to_enclosing_scope(temp);
                    temp.srcref_tree(compiler, prev);
                }
            }
            prev = Some(temp);
        }
        true
    }

    // port: PotentialDeclaration.NameDeclaration#replaceRhsWithUnknown
    fn replace_rhs_with_unknown(compiler: &mut AbstractCompiler, rhs: NodeId) {
        let zero = IR::number(compiler, 0.0);
        let qmark = JsdocUtil::get_qmark_type_jsdoc(compiler);
        let cast = IR::cast(compiler, zero, qmark).srcref_tree(compiler, rhs);
        rhs.replace_with(compiler, cast);
    }

    // port: PotentialDeclaration.NameDeclaration#shouldPreserve
    fn name_should_preserve(&self, ast: &Ast) -> bool {
        let rhs = self.get_rhs();
        let name_node = self.get_lhs();
        let jsdoc = self.get_js_doc(ast);
        let is_export = Self::is_export_lhs(ast, name_node);
        self.should_preserve_base(ast)
            || Self::is_import_rhs(ast, rhs)
            || (is_export
                && rhs.is_some_and(|rhs| rhs.is_qualified_name(ast) || rhs.is_object_lit(ast)))
            || (jsdoc.as_ref().is_some_and(|j| j.is_constructor())
                && rhs.is_some_and(|rhs| rhs.is_qualified_name(ast)))
            || (rhs.is_some_and(|rhs| rhs.is_object_lit(ast) && !rhs.has_children(ast))
                && (jsdoc.is_none() || !JsdocUtil::has_annotated_type(jsdoc.as_deref())))
            || rhs.is_some_and(|rhs| NodeUtil::is_call_to(ast, rhs, "Polymer"))
            || self.is_polymer_behavior_alias_or_array(ast)
    }

    /// Polymer Behaviors can take 3 forms:
    ///
    /// ```text
    /// 1) /** @polymerBehavior */ export const MyBehavior = { ... };
    /// 2) /** @polymerBehavior */ export const MyBehaviorAlias = MyBehavior;
    /// 3) /** @polymerBehavior */ export const MyBehaviorArray = [Behavior1, Behavior2];
    /// ```
    ///
    /// Form #1 will be simplified by PolymerBehaviorPropertiesDeclaration. Forms #2 and #3 need to
    /// be preserved here as-is so that the PolymerPass can follow the name references. Other forms
    /// annotated with @polymerBehavior are invalid and can be simplified or removed like any other
    /// variable.
    // port: PotentialDeclaration.NameDeclaration#isPolymerBehaviorAliasOrArray
    fn is_polymer_behavior_alias_or_array(&self, ast: &Ast) -> bool {
        let jsdoc = self.get_js_doc(ast);
        let rhs = self.get_rhs();
        jsdoc.is_some_and(|j| j.is_polymer_behavior())
            && rhs.is_some_and(|rhs| rhs.is_name(ast) || rhs.is_array_lit(ast))
    }

    // port: PotentialDeclaration.ThisPropDeclaration#ThisPropDeclaration
    fn new_this_prop_declaration(
        ast: &Ast,
        fully_qualified_name: JsString,
        lhs: NodeId,
        rhs: Option<NodeId>,
    ) -> Self {
        let this_prop_definition = NodeUtil::get_enclosing_statement(ast, lhs).unwrap();
        let insertion_point =
            NodeUtil::get_enclosing_statement(ast, this_prop_definition.get_parent(ast).unwrap());
        Self::new(
            Some(fully_qualified_name),
            Some(lhs),
            rhs,
            Kind::ThisPropDeclaration { insertion_point },
        )
    }

    // port: PotentialDeclaration.ThisPropDeclaration#simplify
    fn this_prop_simplify(&self, compiler: &mut AbstractCompiler, insertion_point: Option<NodeId>) {
        if self.should_preserve(compiler) {
            return;
        }
        // Just completely remove the RHS, if present, and replace with a getprop.
        let jsdoc = self.get_js_doc(compiler);
        let new_statement = NodeUtil::new_qname_declaration_var(
            compiler,
            self.get_fully_qualified_name().clone(),
            None,
            jsdoc,
        );
        new_statement.srcref_tree_if_missing(compiler, self.get_lhs());
        let removable = self.get_removable_node(compiler).unwrap();
        NodeUtil::delete_node(compiler, removable);
        let insertion_point = insertion_point.unwrap();
        if insertion_point.has_parent(compiler) {
            new_statement.insert_after(compiler, insertion_point);
            compiler.report_change_to_enclosing_scope(new_statement);
        }
    }

    // port: PotentialDeclaration.DefineDeclaration#simplify
    fn define_simplify(&self, compiler: &mut AbstractCompiler) {
        let info = self.get_js_doc(compiler);
        if let Some(r#type) = info.as_ref().and_then(|info| info.get_type()) {
            let new_rhs = Self::make_empty_value_node(compiler, &r#type);
            if let Some(new_rhs) = new_rhs {
                self.get_rhs().unwrap().replace_with(compiler, new_rhs);
                compiler.report_change_to_enclosing_scope(new_rhs);
                return;
            }
        }
        let removable = self.get_removable_node(compiler).unwrap();
        NodeUtil::delete_node(compiler, removable);
    }

    // port: PotentialDeclaration.DefineDeclaration#from
    fn define_declaration_from(ast: &Ast, call_node: NodeId) -> Self {
        // Match a few different forms, depending on the call node's parent:
        //   1. EXPR_RESULT: goog.define('foo', 1);
        //   2. ASSIGN: a.b = goog.define('c', 2);
        //   3. NAME: var x = goog.define('d', 3);
        let parent = call_node.get_parent(ast).unwrap();
        match parent.get_token(ast) {
            Token::EXPR_RESULT => Self::new(
                Some(call_node.get_second_child(ast).unwrap().get_string(ast)),
                Some(call_node),
                call_node.get_last_child(ast),
                Kind::DefineDeclaration,
            ),
            Token::ASSIGN => {
                let previous = call_node.get_previous(ast).unwrap();
                Self::new(
                    previous.get_qualified_name(ast),
                    Some(previous),
                    call_node.get_last_child(ast),
                    Kind::DefineDeclaration,
                )
            }
            Token::NAME => Self::new(
                Some(parent.get_string(ast)),
                Some(parent),
                call_node.get_last_child(ast),
                Kind::DefineDeclaration,
            ),
            token => panic!("Unexpected parent: {token}"),
        }
    }

    // port: PotentialDeclaration.DefineDeclaration#makeEmptyValueNode
    fn make_empty_value_node(ast: &mut Ast, r#type: &JSTypeExpression) -> Option<NodeId> {
        let mut n = Some(r#type.get_root());
        while let Some(cur) = n {
            if cur.is_string_lit(ast) || cur.is_name(ast) {
                break;
            }
            n = cur.get_first_child(ast);
        }
        let s = match n {
            Some(n) => n.get_string(ast),
            None => JsString::from(""),
        };
        if s == "boolean" {
            Some(ast.new_node(Token::FALSE))
        } else if s == "number" {
            Some(ast.new_number(0.0))
        } else if s == "string" {
            Some(ast.new_string(""))
        } else {
            None
        }
    }

    // port: PotentialDeclaration.MemberFieldDeclaration#simplify
    fn member_field_simplify(&self, compiler: &mut AbstractCompiler) {
        if let Some(rhs) = self.get_rhs() {
            NodeUtil::delete_node(compiler, rhs);
        }
    }

    // port: PotentialDeclaration.StringKeyDeclaration#simplify
    fn string_key_simplify(&self, compiler: &mut AbstractCompiler) {
        if self.should_preserve(compiler) {
            return;
        }
        let jsdoc = self.get_js_doc(compiler);
        if jsdoc.as_ref().is_some_and(|j| j.has_enum_parameter_type()) {
            self.simplify_enum_values(compiler);
            return;
        }
        let key = self.get_lhs();
        Self::remove_string_key_value(compiler, key);
        compiler.report_change_to_enclosing_scope(key);
        if jsdoc.as_ref().is_none_or(|j| !j.contains_declaration())
            || self.is_const_to_be_inferred(compiler)
        {
            let unusable = JsdocUtil::get_unusable_type_jsdoc(compiler, jsdoc.as_deref());
            key.set_jsdoc_info(compiler, unusable);
        }
    }

    // port: PotentialDeclaration.StringKeyDeclaration#shouldPreserve
    fn string_key_should_preserve(&self, ast: &Ast) -> bool {
        self.is_detached(ast)
            || self.should_preserve_base(ast)
            || !self.string_key_is_in_namespace(ast)
    }

    // port: PotentialDeclaration.StringKeyDeclaration#isInNamespace
    fn string_key_is_in_namespace(&self, ast: &Ast) -> bool {
        let string_key = self.get_lhs();
        let obj_lit = string_key.get_parent(ast).unwrap();
        let Some(lvalue) = NodeUtil::get_best_l_value(ast, obj_lit) else {
            return false;
        };
        let jsdoc = NodeUtil::get_best_jsdoc_info(ast, lvalue);
        !Self::is_export_lhs(ast, lvalue)
            && !JsdocUtil::has_annotated_type(jsdoc.as_deref())
            && NodeUtil::is_namespace_decl(ast, lvalue)
    }

    // port: PotentialDeclaration.PolymerBehaviorPropertiesDeclaration#simplify
    fn polymer_behavior_properties_simplify(&self, compiler: &mut AbstractCompiler) {
        if self.is_detached(compiler) {
            return;
        }
        let properties_object = self.get_rhs().unwrap();
        if !properties_object.is_object_lit(compiler) || !properties_object.has_children(compiler) {
            return;
        }
        let mut prop_key = properties_object.get_first_child(compiler);
        while let Some(pk) = prop_key {
            let prop_def = pk.get_only_child(compiler);
            // A property definition is either a function reference (e.g. String, Number), or
            // another object literal. If it's an object literal, only the "type" and "readOnly"
            // sub-properties matters for type checking, so we can delete everything else (which
            // may include e.g. a "value" sub-property with a function expression).
            if prop_def.is_object_lit(compiler) {
                let mut sub_prop = prop_def.get_first_child(compiler);
                while let Some(sp) = sub_prop {
                    let next = sp.get_next(compiler);
                    if sp.get_string_ref(compiler) != "type"
                        && sp.get_string_ref(compiler) != "readOnly"
                    {
                        NodeUtil::delete_node(compiler, sp);
                    }
                    sub_prop = next;
                }
            }
            prop_key = pk.get_next(compiler);
        }
    }

    /// If the declaration is a destructuring declaration: 1) If the lhs's destructuring pattern
    /// parent has only one child, e.g. const {Foo} = x; returns the enclosing statement to remove
    /// the entire statement. 2) If the parent has more than one children, e.g. const {Foo, Bar} =
    /// x; returns the lhs so that when Foo is removed, const {Foo, Bar} = x; becomes const {Bar} =
    /// x; Otherwise, returns the enclosing statement.
    // port: PotentialDeclaration.AliasDeclaration#getRemovableNode
    fn alias_get_removable_node(&self, ast: &Ast) -> NodeId {
        let lhs = self.get_lhs();
        let parent = lhs.get_parent(ast).unwrap();
        if parent.is_array_pattern(ast) && parent.has_more_than_one_child(ast) {
            return lhs;
        }
        let grandparent = lhs.get_grandparent(ast).unwrap();
        if grandparent.is_object_pattern(ast) && grandparent.has_more_than_one_child(ast) {
            return parent;
        }
        NodeUtil::get_enclosing_statement(ast, lhs).unwrap()
    }

    /// Remove values from enums
    // port: PotentialDeclaration#simplifyEnumValues
    fn simplify_enum_values(&self, compiler: &mut AbstractCompiler) {
        let rhs = self.get_rhs().unwrap();
        if rhs.is_object_lit(compiler) && rhs.has_children(compiler) {
            let mut changed = false;
            let mut key = rhs.get_first_child(compiler);
            while let Some(k) = key {
                let value = k.get_only_child(compiler);
                if !value.is_string_lit(compiler) {
                    Self::remove_string_key_value(compiler, k);
                    changed = true;
                } else {
                    let content = value.get_string(compiler);
                    if content.length() > STRING_ENUM_RETAIN_CAP {
                        Self::truncate_string_key_value(compiler, k);
                        changed = true;
                    }
                }
                key = k.get_next(compiler);
            }
            if changed {
                compiler.report_change_to_enclosing_scope(rhs);
            }
        }
    }

    // port: PotentialDeclaration#isDefiniteDeclaration
    pub fn is_definite_declaration(&self, compiler: &mut AbstractCompiler) -> bool {
        if let Kind::AliasDeclaration = self.kind {
            // port: PotentialDeclaration.AliasDeclaration#isDefiniteDeclaration
            return true;
        }
        let ast: &Ast = compiler;
        let parent = self.get_lhs().get_parent(ast).unwrap();
        let token = parent.get_token(ast);
        match token {
            Token::DEFAULT_VALUE | Token::COMPUTED_PROP | Token::STRING_KEY => {
                if token == Token::DEFAULT_VALUE
                    && !parent.get_parent(ast).unwrap().is_string_key(ast)
                {
                    return false;
                }
                // fall through
                if NodeUtil::is_lhs_by_destructuring(ast, self.get_lhs()) {
                    let root_target = NodeUtil::get_root_target(ast, self.get_lhs());
                    check_state!(
                        root_target
                            .get_parent(ast)
                            .unwrap()
                            .is_destructuring_lhs(ast)
                    );
                    if NodeUtil::is_name_declaration(ast, root_target.get_grandparent(ast)) {
                        return true;
                    }
                }
                false
            }
            Token::VAR | Token::LET | Token::CONST | Token::CLASS | Token::FUNCTION => true,
            _ => {
                Self::is_export_lhs(ast, self.get_lhs())
                    || self
                        .get_js_doc(ast)
                        .is_some_and(|j| j.contains_declaration())
                    || self
                        .get_rhs()
                        .is_some_and(|rhs| PotentialDeclaration::is_typed_rhs(ast, rhs))
            }
        }
    }

    // port: PotentialDeclaration#shouldPreserve
    pub fn should_preserve(&self, ast: &Ast) -> bool {
        match self.kind {
            Kind::NameDeclaration => self.name_should_preserve(ast),
            Kind::StringKeyDeclaration => self.string_key_should_preserve(ast),
            // port: PotentialDeclaration.PolymerBehaviorPropertiesDeclaration#shouldPreserve
            // port: PotentialDeclaration.AliasDeclaration#shouldPreserve
            Kind::PolymerBehaviorPropertiesDeclaration | Kind::AliasDeclaration => true,
            _ => self.should_preserve_base(ast),
        }
    }

    // port: PotentialDeclaration#shouldPreserve (the base body, Java's `super.shouldPreserve()`)
    fn should_preserve_base(&self, ast: &Ast) -> bool {
        self.get_rhs()
            .is_some_and(|rhs| Self::is_typed_rhs(ast, rhs))
    }

    // port: PotentialDeclaration#isConstToBeInferred()
    pub fn is_const_to_be_inferred(&self, ast: &Ast) -> bool {
        Self::is_const_to_be_inferred_node(ast, self.get_lhs())
    }

    // port: PotentialDeclaration#isConstToBeInferred(Node)
    pub fn is_const_to_be_inferred_node(ast: &Ast, name_node: NodeId) -> bool {
        let jsdoc = NodeUtil::get_best_jsdoc_info(ast, name_node);
        let is_const = name_node.get_parent(ast).unwrap().is_const(ast)
            || Self::is_export_lhs(ast, name_node)
            || jsdoc.as_ref().is_some_and(|j| j.is_constant());
        is_const
            && !JsdocUtil::has_annotated_type(jsdoc.as_deref())
            && !NodeUtil::is_namespace_decl(ast, name_node)
    }

    // port: PotentialDeclaration#isTypedRhs
    fn is_typed_rhs(ast: &Ast, rhs: NodeId) -> bool {
        rhs.is_function(ast)
            || rhs.is_class(ast)
            || (rhs.is_qualified_name(ast) && GOOG_ABSTRACTMETHOD.matches(ast, rhs))
    }

    // port: PotentialDeclaration#isExportLhs
    fn is_export_lhs(ast: &Ast, lhs: NodeId) -> bool {
        (lhs.is_name(ast) && lhs.matches_name(ast, "exports"))
            || (lhs.is_get_prop(ast)
                && lhs
                    .get_first_child(ast)
                    .unwrap()
                    .matches_name(ast, "exports"))
            || MODULE_EXPORTS.matches(ast, lhs)
    }

    // port: PotentialDeclaration#isImportRhs
    pub fn is_import_rhs(ast: &Ast, rhs: Option<NodeId>) -> bool {
        let Some(rhs) = rhs.filter(|rhs| rhs.is_call(ast)) else {
            return false;
        };
        let callee = rhs.get_first_child(ast).unwrap();
        GOOG_REQUIRE.matches(ast, callee)
            || GOOG_REQUIRETYPE.matches(ast, callee)
            || GOOG_FORWARDDECLARE.matches(ast, callee)
            || callee.matches_name(ast, "require")
    }

    // port: PotentialDeclaration#isAliasDeclaration
    pub fn is_alias_declaration(ast: &Ast, lhs: NodeId, rhs: Option<NodeId>) -> bool {
        !ClassUtil::is_this_prop_inside_class_with_name(ast, lhs)
            && Self::is_const_to_be_inferred_node(ast, lhs)
            && rhs.is_some_and(|rhs| Self::is_qualified_alias_expression(ast, rhs))
    }

    /// Returns whether a node corresponds to a simple or a qualified name, such as `x` or `a.b.c`
    /// or `this.a`.
    // port: PotentialDeclaration#isQualifiedAliasExpression
    pub fn is_qualified_alias_expression(ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::NAME => !n.get_string_ref(ast).is_empty(),
            Token::THIS | Token::SUPER => true,
            Token::GETPROP => {
                Self::is_qualified_alias_expression(ast, n.get_first_child(ast).unwrap())
            }
            Token::CALL => {
                if GOOG_MODULE_GET.matches(ast, n.get_first_child(ast).unwrap()) {
                    return true;
                }
                false
            }
            // MEMBER_FUNCTION_DEF: these are explicitly *not* qualified name components.
            _ => false,
        }
    }

    // port: PotentialDeclaration#removeStringKeyValue
    fn remove_string_key_value(ast: &mut Ast, string_key: NodeId) {
        let value = string_key.get_only_child(ast);
        let replacement_value = IR::number(ast, 0.0).srcref_tree(ast, value);
        value.replace_with(ast, replacement_value);
    }

    // port: PotentialDeclaration#truncateStringKeyValue
    fn truncate_string_key_value(ast: &mut Ast, string_key: NodeId) {
        let value = string_key.get_only_child(ast);
        let truncated = value
            .get_string(ast)
            .substring(0, STRING_ENUM_RETAIN_CAP - 2)
            .concat(&JsString::from(".."));
        let replacement_value = IR::string(ast, truncated).srcref_tree(ast, value);
        value.replace_with(ast, replacement_value);
    }
}
