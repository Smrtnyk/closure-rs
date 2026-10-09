/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Norris Boyd
 *   Roger Lawrence
 *   Mike McCabe
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/rhino/Node.java.

// Keep the Java control flow visible for side-by-side port review.
#![allow(
    clippy::collapsible_if,
    clippy::collapsible_match,
    clippy::if_same_then_else,
    clippy::question_mark
)]
//! Rhino's Node API, with object identity represented by arena handles.
use crate::input_id::InputId;
use crate::ir::IR;
use crate::java_lang::double_to_string;
use crate::js_string::{JsStrLike, JsString};
use crate::js_type_expression::JSTypeExpression;
use crate::jscomp_base::JSCompDoubles;
use crate::jscomp_colors::Color;
use crate::jscomp_serialization::node_property::NodeProperty;
use crate::jsdoc_info::JSDocInfo;
use crate::jstype::TypeId;
use crate::non_jsdoc_comment::NonJSDocComment;
use crate::prop_translator::PropTranslator;
use crate::qualified_name::QualifiedName;
use crate::rhino_string_pool::{LazyInternedStringList, RhinoStringPool};
use crate::simple_source_file::SimpleSourceFile;
use crate::static_source_file::{SourceKind, StaticSourceFile};
use crate::token::Token;
use crate::{check_argument, check_not_null, check_state};
use num_bigint::{BigInt, Sign};
use std::{
    any::Any,
    fmt,
    num::NonZeroU32,
    ops::{Index, IndexMut},
    sync::Arc,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Prop {
    IS_PARENTHESIZED,
    NON_JSDOC_COMMENT,
    TRAILING_NON_JSDOC_COMMENT,
    JSDOC_INFO,
    INCRDECR,
    QUOTED,
    SYNTHETIC,
    ADDED_BLOCK,
    SIDE_EFFECT_FLAGS,
    IS_CONSTANT_NAME,
    IS_NAMESPACE,
    USE_STRICT,
    DIRECT_EVAL,
    FREE_CALL,
    SOURCE_FILE,
    INPUT_ID,
    CHANGE_TIME,
    REFLECTED_OBJECT,
    STATIC_MEMBER,
    GENERATOR_FN,
    ARROW_FN,
    ASYNC_FN,
    YIELD_ALL,
    EXPORT_DEFAULT,
    EXPORT_ALL_FROM,
    CONSTANT_VAR_FLAGS,
    IS_GENERATOR_MARKER,
    IS_GENERATOR_SAFE,
    COMPUTED_PROP_METHOD,
    COMPUTED_PROP_GETTER,
    COMPUTED_PROP_SETTER,
    COMPUTED_PROP_VARIABLE,
    DECLARED_TYPE_EXPR,
    TYPE_BEFORE_CAST,
    COLOR_FROM_CAST,
    OPT_ES6_TYPED,
    GENERIC_TYPE,
    IMPLEMENTS,
    CONSTRUCT_SIGNATURE,
    ACCESS_MODIFIER,
    NON_INDEXABLE,
    GOOG_MODULE,
    FEATURE_SET,
    IS_TYPESCRIPT_ABSTRACT,
    DELETED,
    MODULE_ALIAS,
    IS_UNUSED_PARAMETER,
    MODULE_EXPORT,
    IS_SHORTHAND_PROPERTY,
    ES6_MODULE,
    TYPEDEF_TYPE,
    START_OF_OPT_CHAIN,
    TRAILING_COMMA,
    SYNTHESIZED_UNFULFILLED_NAME_DECLARATION,
    CLOSURE_UNAWARE_SHADOW,
    PRIVATE_IDENTIFIER,
    IS_IN_CLOSURE_UNAWARE_SUBTREE,
}
impl Prop {
    pub const VALUES: [Self; 57] = [
        Self::IS_PARENTHESIZED,
        Self::NON_JSDOC_COMMENT,
        Self::TRAILING_NON_JSDOC_COMMENT,
        Self::JSDOC_INFO,
        Self::INCRDECR,
        Self::QUOTED,
        Self::SYNTHETIC,
        Self::ADDED_BLOCK,
        Self::SIDE_EFFECT_FLAGS,
        Self::IS_CONSTANT_NAME,
        Self::IS_NAMESPACE,
        Self::USE_STRICT,
        Self::DIRECT_EVAL,
        Self::FREE_CALL,
        Self::SOURCE_FILE,
        Self::INPUT_ID,
        Self::CHANGE_TIME,
        Self::REFLECTED_OBJECT,
        Self::STATIC_MEMBER,
        Self::GENERATOR_FN,
        Self::ARROW_FN,
        Self::ASYNC_FN,
        Self::YIELD_ALL,
        Self::EXPORT_DEFAULT,
        Self::EXPORT_ALL_FROM,
        Self::CONSTANT_VAR_FLAGS,
        Self::IS_GENERATOR_MARKER,
        Self::IS_GENERATOR_SAFE,
        Self::COMPUTED_PROP_METHOD,
        Self::COMPUTED_PROP_GETTER,
        Self::COMPUTED_PROP_SETTER,
        Self::COMPUTED_PROP_VARIABLE,
        Self::DECLARED_TYPE_EXPR,
        Self::TYPE_BEFORE_CAST,
        Self::COLOR_FROM_CAST,
        Self::OPT_ES6_TYPED,
        Self::GENERIC_TYPE,
        Self::IMPLEMENTS,
        Self::CONSTRUCT_SIGNATURE,
        Self::ACCESS_MODIFIER,
        Self::NON_INDEXABLE,
        Self::GOOG_MODULE,
        Self::FEATURE_SET,
        Self::IS_TYPESCRIPT_ABSTRACT,
        Self::DELETED,
        Self::MODULE_ALIAS,
        Self::IS_UNUSED_PARAMETER,
        Self::MODULE_EXPORT,
        Self::IS_SHORTHAND_PROPERTY,
        Self::ES6_MODULE,
        Self::TYPEDEF_TYPE,
        Self::START_OF_OPT_CHAIN,
        Self::TRAILING_COMMA,
        Self::SYNTHESIZED_UNFULFILLED_NAME_DECLARATION,
        Self::CLOSURE_UNAWARE_SHADOW,
        Self::PRIVATE_IDENTIFIER,
        Self::IS_IN_CLOSURE_UNAWARE_SUBTREE,
    ];
}
pub trait OpaqueProp: Any + Send + Sync + fmt::Display + fmt::Debug {
    fn as_any(&self) -> &dyn Any;
}
impl<T: Any + Send + Sync + fmt::Display + fmt::Debug> OpaqueProp for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
#[derive(Clone, Debug)]
pub enum ObjectProp {
    NonJSDocComment(Arc<NonJSDocComment>),
    JSDocInfo(Arc<JSDocInfo>),
    StaticSourceFile(Arc<dyn StaticSourceFile>),
    InputId(Arc<InputId>),
    Node(NodeId),
    JSType(TypeId),
    Opaque(Arc<dyn OpaqueProp>),
}
/// Prints a JSType the way Java's `JSType#toString` does. Rhino has no access to the
/// `JSTypeRegistry` (crates/jstype), so the registry-aware printing of `Node#toString`,
/// `Node#toStringTree` and `ObjectPropListItem#toString` takes it from the caller.
pub type JSTypePrinter<'p> = dyn FnMut(TypeId) -> JsString + 'p;

/// The printer of the registry-free entry points: Java has none, its JSType prints itself.
pub(crate) fn no_registry_jstype_printer(_: TypeId) -> JsString {
    panic!("Type printing requires JSTypeRegistry or ColorRegistry")
}

impl ObjectProp {
    // port: ObjectPropListItem#toString
    fn to_string_utf16(&self, ast: &Ast, type_printer: &mut JSTypePrinter<'_>) -> JsString {
        match self {
            Self::NonJSDocComment(c) => {
                JsString::from("NonJSDocComment : ").concat(&c.get_comment_string())
            }
            Self::JSDocInfo(i) => i.to_string_with_types_utf16(ast, type_printer),
            Self::StaticSourceFile(f) => f.to_string().into(),
            Self::InputId(i) => i.to_string().into(),
            Self::Node(n) => {
                n.to_string_with_options_and_types_utf16(ast, true, true, true, type_printer)
            }
            Self::JSType(t) => type_printer(*t),
            Self::Opaque(o) => o.to_string().into(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum PropValue {
    Int(i32),
    Object(ObjectProp),
}
#[derive(Debug)]
pub struct PropListItem {
    pub prop_type: u8,
    pub value: PropValue,
    pub next: Option<Arc<PropListItem>>,
}
impl PropListItem {
    // port: PropListItem#PropListItem
    fn new(prop_type: u8, value: PropValue, next: Option<Arc<Self>>) -> Arc<Self> {
        Arc::new(Self {
            prop_type,
            value,
            next,
        })
    }
    // port: ObjectPropListItem#ObjectPropListItem
    fn object(prop_type: u8, object_value: ObjectProp, next: Option<Arc<Self>>) -> Arc<Self> {
        Self::new(prop_type, PropValue::Object(object_value), next)
    }
    // port: IntPropListItem#IntPropListItem
    fn int(prop_type: u8, int_value: i32, next: Option<Arc<Self>>) -> Arc<Self> {
        check_state!(int_value != 0);
        Self::new(prop_type, PropValue::Int(int_value), next)
    }
    // port: PropListItem#getIntValue
    // port: IntPropListItem#getIntValue
    // port: ObjectPropListItem#getIntValue
    pub fn get_int_value(&self) -> i32 {
        match self.value {
            PropValue::Int(v) => v,
            _ => panic!("UnsupportedOperationException"),
        }
    }
    // port: PropListItem#getObjectValue
    // port: IntPropListItem#getObjectValue
    // port: ObjectPropListItem#getObjectValue
    pub fn get_object_value(&self) -> ObjectProp {
        match &self.value {
            PropValue::Object(v) => v.clone(),
            _ => panic!("UnsupportedOperationException"),
        }
    }
    // port: ObjectPropListItem#toString
    // port: IntPropListItem#toString
    pub fn to_string(&self, ast: &Ast) -> String {
        crate::java_lang::charset::utf8_encoded_text(self.to_string_utf16(ast).as_units())
    }
    // port: ObjectPropListItem#toString
    // port: IntPropListItem#toString
    pub fn to_string_utf16(&self, ast: &Ast) -> JsString {
        self.to_string_with_types_utf16(ast, &mut no_registry_jstype_printer)
    }
    // port: ObjectPropListItem#toString
    // port: IntPropListItem#toString
    /// `toString` with a registry-aware JSType printer (see [`JSTypePrinter`]).
    pub fn to_string_with_types_utf16(
        &self,
        ast: &Ast,
        type_printer: &mut JSTypePrinter<'_>,
    ) -> JsString {
        match &self.value {
            PropValue::Int(v) => v.to_string().into(),
            PropValue::Object(v) => v.to_string_utf16(ast, type_printer),
        }
    }
    // port: PropListItem#chain
    // port: ObjectPropListItem#chain
    // port: IntPropListItem#chain
    pub fn chain(&self, next: Option<Arc<Self>>) -> Arc<Self> {
        match &self.value {
            PropValue::Int(v) => Self::int(self.prop_type, *v, next),
            PropValue::Object(v) => Self::object(self.prop_type, v.clone(), next),
        }
    }
}
#[derive(Clone, Debug)]
pub enum NodeKind {
    Node,
    Number { number: f64 },
    BigInt { bigint: Arc<BigInt> },
    String { str: JsString },
    // Boxed (rare) so that the node stays small (D-025).
    TemplateLiteralSubstring(Box<TemplateLiteralStrings>),
}
#[derive(Clone, Debug)]
pub struct TemplateLiteralStrings {
    cooked: Option<JsString>,
    raw: JsString,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JSTypeOrColor {
    JSType(TypeId),
    Color(Color),
}
/// Rust-only (D-025): the tree links and token of a node, the fields every traversal reads.
/// They live in their own dense array (`Ast::links`), apart from the rest of the node, so that
/// walking the tree touches 20 bytes per node instead of the whole node.
#[derive(Debug, Clone, Copy)]
pub struct NodeLinks {
    token: Token,
    parent: Option<NodeId>,
    first: Option<NodeId>,
    next: Option<NodeId>,
    previous: Option<NodeId>,
}
/// Rust-only: indexes `Ast` for a node's `NodeLinks` (`ast[L(n)].next`).
#[derive(Clone, Copy)]
struct L(NodeId);
#[derive(Debug)]
pub struct NodeData {
    lineno_charno: i32,
    length: i32,
    jstype_or_color: Option<JSTypeOrColor>,
    original_name: Option<JsString>,
    prop_list_head: Option<Arc<PropListItem>>,
    kind: NodeKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(NonZeroU32);
/// Rust-only: how `Ast::append_preparsed` renumbered the nodes of a preparse arena.
#[derive(Clone, Copy, Debug)]
pub struct PreparsedIdMap {
    base: u32,
    bound_at: Option<u32>,
}
impl PreparsedIdMap {
    /// The id in the target arena of node `n` of the preparse arena.
    pub fn map(&self, n: NodeId) -> NodeId {
        let local = n.0.get();
        let shift = u32::from(self.bound_at.is_some_and(|at| local > at));
        NodeId(NonZeroU32::new(self.base + local + shift).unwrap())
    }
}
/// Rust-only: rewrites the property lists of appended preparse nodes for their new ids,
/// keeping the sharing of lists, JSDoc infos and type expressions (memoized by identity).
struct PropRemapper<'m> {
    map: &'m PreparsedIdMap,
    placeholder: Arc<JSTypeExpression>,
    bound: Option<Arc<JSTypeExpression>>,
    items: crate::fast_hash::IndexMap<usize, Arc<PropListItem>>,
    infos: crate::fast_hash::IndexMap<usize, Arc<JSDocInfo>>,
    exprs: crate::fast_hash::IndexMap<usize, Arc<JSTypeExpression>>,
}
impl PropRemapper<'_> {
    /// The list `item` for the new ids: the same list when no item refers to a node.
    fn list(&mut self, item: Arc<PropListItem>) -> Arc<PropListItem> {
        if Self::refers_to_nodes(&item) {
            self.rebuild(&item)
        } else {
            item
        }
    }
    fn refers_to_nodes(item: &PropListItem) -> bool {
        let mut x = Some(item);
        while let Some(item) = x {
            if matches!(
                item.value,
                PropValue::Object(ObjectProp::Node(_) | ObjectProp::JSDocInfo(_))
            ) {
                return true;
            }
            x = item.next.as_deref();
        }
        false
    }
    fn rebuild(&mut self, item: &Arc<PropListItem>) -> Arc<PropListItem> {
        let key = Arc::as_ptr(item) as usize;
        if let Some(done) = self.items.get(&key) {
            return done.clone();
        }
        let next = match &item.next {
            Some(next) if Self::refers_to_nodes(next) => Some(self.rebuild(next)),
            next => next.clone(),
        };
        let value = match &item.value {
            PropValue::Object(ObjectProp::Node(n)) => {
                PropValue::Object(ObjectProp::Node(self.map.map(*n)))
            }
            PropValue::Object(ObjectProp::JSDocInfo(info)) => {
                PropValue::Object(ObjectProp::JSDocInfo(self.info(info)))
            }
            value => value.clone(),
        };
        let done = Arc::new(PropListItem {
            prop_type: item.prop_type,
            value,
            next,
        });
        self.items.insert(key, done.clone());
        done
    }
    fn info(&mut self, info: &Arc<JSDocInfo>) -> Arc<JSDocInfo> {
        let key = Arc::as_ptr(info) as usize;
        if let Some(done) = self.infos.get(&key) {
            return done.clone();
        }
        let map = *self.map;
        let done = Arc::new(info.map_nodes(&mut |e| self.expr(e), &|n| map.map(n)));
        self.infos.insert(key, done.clone());
        done
    }
    fn expr(&mut self, expr: &Arc<JSTypeExpression>) -> Arc<JSTypeExpression> {
        if Arc::ptr_eq(expr, &self.placeholder) {
            return self.bound.clone().expect("implicit template bound");
        }
        let key = Arc::as_ptr(expr) as usize;
        if let Some(done) = self.exprs.get(&key) {
            return done.clone();
        }
        let done = Arc::new(expr.with_root(self.map.map(expr.get_root())));
        self.exprs.insert(key, done.clone());
        done
    }
}
#[derive(Default, Debug)]
pub struct Ast {
    nodes: Vec<NodeData>,
    links: Vec<NodeLinks>,
    pub(crate) implicit_template_bound: Option<Arc<JSTypeExpression>>,
    /// port: JSDocSerializer#placeholderType (a Java static holding nodes, so one per arena; the
    /// serialization code in closure-jscomp creates it on first use).
    pub jsdoc_serializer_placeholder_type: Option<Arc<JSTypeExpression>>,
    /// Rust-only: this arena parses one input apart from the compiler's arena (see
    /// `Ast::new_for_preparse`).
    pub(crate) preparse: bool,
    /// Rust-only: in a preparse arena, the node count when the parse first asked for the
    /// implicit template bound.
    pub(crate) preparse_bound_first_use: Option<u32>,
}
impl Index<NodeId> for Ast {
    type Output = NodeData;
    fn index(&self, n: NodeId) -> &NodeData {
        &self.nodes[n.0.get() as usize - 1]
    }
}
impl IndexMut<NodeId> for Ast {
    fn index_mut(&mut self, n: NodeId) -> &mut NodeData {
        &mut self.nodes[n.0.get() as usize - 1]
    }
}
impl Index<L> for Ast {
    type Output = NodeLinks;
    #[inline]
    fn index(&self, n: L) -> &NodeLinks {
        &self.links[n.0.0.get() as usize - 1]
    }
}
impl IndexMut<L> for Ast {
    #[inline]
    fn index_mut(&mut self, n: L) -> &mut NodeLinks {
        &mut self.links[n.0.0.get() as usize - 1]
    }
}
impl Ast {
    pub fn new() -> Self {
        Self::default()
    }
    /// Rust-only, not in Java (D-025): an arena in which a worker thread parses one input
    /// (Java parses into the one shared object graph). `append_preparsed` then moves the result
    /// into the compiler's arena with exactly the node ids, and the same object sharing, that
    /// parsing directly into that arena at that point would have produced. The one piece of
    /// arena state a parse reads is the lazily created implicit template bound (Java's
    /// `JSTypeExpression.IMPLICIT_TEMPLATE_BOUND`); a preparse arena hands out a placeholder for
    /// it and records when it was first asked for, so that the append can create the real one at
    /// the same point if the target arena does not have it yet.
    pub fn new_for_preparse() -> Self {
        Self {
            implicit_template_bound: Some(Arc::new(JSTypeExpression::new(
                NodeId(NonZeroU32::MAX),
                crate::js_type_expression::IMPLICIT_TEMPLATE_BOUND_SOURCE,
            ))),
            preparse: true,
            ..Self::default()
        }
    }
    /// Rust-only: the number of nodes in the arena.
    pub fn node_count(&self) -> u32 {
        u32::try_from(self.nodes.len()).unwrap()
    }
    /// Rust-only (D-025): moves the nodes of `other`, a `new_for_preparse` arena holding one
    /// parsed input, to the end of this arena; see `new_for_preparse`. Returns how the ids of
    /// `other` map to ids of this arena.
    pub fn append_preparsed(&mut self, other: Ast) -> PreparsedIdMap {
        assert!(other.preparse && !self.preparse);
        let placeholder = other.implicit_template_bound.clone().unwrap();
        let map = PreparsedIdMap {
            base: u32::try_from(self.nodes.len()).unwrap(),
            // Parsing into this arena would have created the bound (one node) at that point.
            bound_at: other
                .preparse_bound_first_use
                .filter(|_| self.implicit_template_bound.is_none()),
        };
        let Ast { nodes, links, .. } = other;
        let mut heads = Vec::with_capacity(nodes.len());
        for (i, (mut data, link)) in nodes.into_iter().zip(links).enumerate() {
            if map.bound_at == Some(u32::try_from(i).unwrap()) {
                crate::js_type_expression::JSTypeExpression::implicit_template_bound(self);
            }
            heads.push(data.prop_list_head.take());
            self.links.push(NodeLinks {
                token: link.token,
                parent: link.parent.map(|n| map.map(n)),
                first: link.first.map(|n| map.map(n)),
                next: link.next.map(|n| map.map(n)),
                previous: link.previous.map(|n| map.map(n)),
            });
            self.nodes.push(data);
        }
        if map.bound_at == Some(u32::try_from(heads.len()).unwrap()) {
            crate::js_type_expression::JSTypeExpression::implicit_template_bound(self);
        }
        // The properties last: they may refer to the bound, which now exists.
        let bound = self.implicit_template_bound.clone();
        let mut remapper = PropRemapper {
            map: &map,
            placeholder,
            bound,
            items: Default::default(),
            infos: Default::default(),
            exprs: Default::default(),
        };
        for (i, head) in heads.into_iter().enumerate() {
            if let Some(head) = head {
                let local = NodeId(NonZeroU32::new(u32::try_from(i + 1).unwrap()).unwrap());
                let id = map.map(local);
                self[id].prop_list_head = Some(remapper.list(head));
            }
        }
        map
    }
    // port: Node#Node(Token)
    pub fn new_node(&mut self, token: Token) -> NodeId {
        let id = NodeId(
            NonZeroU32::new(
                u32::try_from(self.nodes.len() + 1).expect("Node arena capacity exceeded"),
            )
            .unwrap(),
        );
        self.links.push(NodeLinks {
            token,
            parent: None,
            first: None,
            next: None,
            previous: None,
        });
        self.nodes.push(NodeData {
            lineno_charno: -1,
            length: 0,
            jstype_or_color: None,
            original_name: None,
            prop_list_head: None,
            kind: NodeKind::Node,
        });
        id
    }
    // port: Node#Node(Token, Node)
    pub fn new_node_with_child(&mut self, token: Token, child: NodeId) -> NodeId {
        let n = self.new_node(token);
        self[L(n)].first = Some(child);
        child.check_detached(self);
        self[L(child)].previous = Some(child);
        self[L(child)].parent = Some(n);
        n
    }
    // port: Node#Node(Token, Node, Node)
    pub fn new_node_with_children2(&mut self, token: Token, left: NodeId, right: NodeId) -> NodeId {
        let n = self.new_node(token);
        self[L(n)].first = Some(left);
        left.check_detached(self);
        self[L(left)].next = Some(right);
        self[L(left)].previous = Some(right);
        self[L(left)].parent = Some(n);
        right.check_detached(self);
        self[L(right)].previous = Some(left);
        self[L(right)].parent = Some(n);
        n
    }
    // port: Node#Node(Token, Node, Node, Node)
    pub fn new_node_with_children3(
        &mut self,
        token: Token,
        left: NodeId,
        mid: NodeId,
        right: NodeId,
    ) -> NodeId {
        let n = self.new_node(token);
        self[L(n)].first = Some(left);
        left.check_detached(self);
        self[L(left)].next = Some(mid);
        self[L(left)].previous = Some(right);
        self[L(left)].parent = Some(n);
        mid.check_detached(self);
        self[L(mid)].next = Some(right);
        self[L(mid)].previous = Some(left);
        self[L(mid)].parent = Some(n);
        right.check_detached(self);
        self[L(right)].previous = Some(mid);
        self[L(right)].parent = Some(n);
        n
    }
    // port: NumberNode#NumberNode(double)
    // port: Node#newNumber
    pub fn new_number(&mut self, number: f64) -> NodeId {
        let n = self.new_node(Token::NUMBER);
        self[n].kind = NodeKind::Number { number: 0.0 };
        n.set_double(self, number);
        n
    }
    // port: BigIntNode#BigIntNode(BigInteger)
    // port: Node#newBigInt
    pub fn new_big_int(&mut self, bigint: impl Into<Arc<BigInt>>) -> NodeId {
        let n = self.new_node(Token::BIGINT);
        self[n].kind = NodeKind::BigInt {
            bigint: Arc::new(BigInt::from(0)),
        };
        n.set_big_int(self, bigint);
        n
    }
    // port: Node#newString(String)
    pub fn new_string(&mut self, str: impl Into<JsString>) -> NodeId {
        self.new_string_with_token(Token::STRINGLIT, str)
    }
    // port: StringNode#StringNode(Token)
    fn new_string_node(&mut self, token: Token) -> NodeId {
        let n = self.new_node(token);
        self[n].kind = NodeKind::String {
            str: JsString::default(),
        };
        n
    }
    // port: StringNode#StringNode(Token, String)
    // port: Node#newString(Token, String)
    pub fn new_string_with_token(&mut self, token: Token, str: impl Into<JsString>) -> NodeId {
        let n = self.new_string_node(token);
        n.set_string(self, str);
        n
    }
    // port: StringNode#StringNode(Token, LazyInternedStringList, int)
    // port: Node#newString(Token, LazyInternedStringList, int)
    pub fn new_string_from_string_pool(
        &mut self,
        token: Token,
        string_pool: &LazyInternedStringList,
        offset: i32,
    ) -> NodeId {
        let n = self.new_string_node(token);
        n.set_string_from_string_pool(self, string_pool, offset);
        n
    }
    // port: TemplateLiteralSubstringNode#TemplateLiteralSubstringNode(String, String)
    // port: Node#newTemplateLitString(String, String)
    pub fn new_template_lit_string(
        &mut self,
        cooked: Option<JsString>,
        raw: impl Into<JsString>,
    ) -> NodeId {
        let n = self.new_node(Token::TEMPLATELIT_STRING);
        self[n].kind = NodeKind::TemplateLiteralSubstring(Box::new(TemplateLiteralStrings {
            cooked: cooked.map(RhinoStringPool::add_or_get),
            raw: RhinoStringPool::add_or_get(raw),
        }));
        n
    }
    // port: TemplateLiteralSubstringNode#TemplateLiteralSubstringNode(LazyInternedStringList, int, int)
    // port: Node#newTemplateLitString(LazyInternedStringList, int, int)
    pub fn new_template_lit_string_from_string_pool(
        &mut self,
        pool: &LazyInternedStringList,
        cooked_offset_or_negative_one: i32,
        raw_offset: i32,
    ) -> NodeId {
        let n = self.new_node(Token::TEMPLATELIT_STRING);
        self[n].kind = NodeKind::TemplateLiteralSubstring(Box::new(TemplateLiteralStrings {
            cooked: if cooked_offset_or_negative_one == -1 {
                None
            } else {
                Some(pool.get(cooked_offset_or_negative_one))
            },
            raw: pool.get(raw_offset),
        }));
        n
    }
}
impl NodeId {
    // port: Object#getClass (the runtime classes represented by NodeKind)
    pub fn get_class(self, ast: &Ast) -> &'static str {
        match &ast[self].kind {
            NodeKind::Node => "com.google.javascript.rhino.Node",
            NodeKind::Number { .. } => "com.google.javascript.rhino.Node$NumberNode",
            NodeKind::BigInt { .. } => "com.google.javascript.rhino.Node$BigIntNode",
            NodeKind::String { .. } => "com.google.javascript.rhino.Node$StringNode",
            NodeKind::TemplateLiteralSubstring(_) => {
                "com.google.javascript.rhino.Node$TemplateLiteralSubstringNode"
            }
        }
    }
    pub const INCRDECR_PROP: Prop = Prop::INCRDECR;
    pub const QUOTED_PROP: Prop = Prop::QUOTED;
    pub const IS_CONSTANT_NAME: Prop = Prop::IS_CONSTANT_NAME;
    pub const IS_NAMESPACE: Prop = Prop::IS_NAMESPACE;
    pub const DIRECT_EVAL: Prop = Prop::DIRECT_EVAL;
    pub const FREE_CALL: Prop = Prop::FREE_CALL;
    pub const REFLECTED_OBJECT: Prop = Prop::REFLECTED_OBJECT;
    pub const STATIC_MEMBER: Prop = Prop::STATIC_MEMBER;
    pub const GENERATOR_FN: Prop = Prop::GENERATOR_FN;
    pub const YIELD_ALL: Prop = Prop::YIELD_ALL;
    pub const EXPORT_DEFAULT: Prop = Prop::EXPORT_DEFAULT;
    pub const EXPORT_ALL_FROM: Prop = Prop::EXPORT_ALL_FROM;
    pub const COMPUTED_PROP_METHOD: Prop = Prop::COMPUTED_PROP_METHOD;
    pub const COMPUTED_PROP_GETTER: Prop = Prop::COMPUTED_PROP_GETTER;
    pub const COMPUTED_PROP_SETTER: Prop = Prop::COMPUTED_PROP_SETTER;
    pub const COMPUTED_PROP_VARIABLE: Prop = Prop::COMPUTED_PROP_VARIABLE;
    pub const OPT_ES6_TYPED: Prop = Prop::OPT_ES6_TYPED;
    pub const GENERIC_TYPE_LIST: Prop = Prop::GENERIC_TYPE;
    pub const IMPLEMENTS: Prop = Prop::IMPLEMENTS;
    pub const CONSTRUCT_SIGNATURE: Prop = Prop::CONSTRUCT_SIGNATURE;
    pub const ACCESS_MODIFIER: Prop = Prop::ACCESS_MODIFIER;
    pub const GOOG_MODULE: Prop = Prop::GOOG_MODULE;
    pub const FEATURE_SET: Prop = Prop::FEATURE_SET;
    pub const IS_TYPESCRIPT_ABSTRACT: Prop = Prop::IS_TYPESCRIPT_ABSTRACT;
    pub const MODULE_ALIAS: Prop = Prop::MODULE_ALIAS;
    pub const MODULE_EXPORT: Prop = Prop::MODULE_EXPORT;
    pub const IS_SHORTHAND_PROPERTY: Prop = Prop::IS_SHORTHAND_PROPERTY;
    pub const ES6_MODULE: Prop = Prop::ES6_MODULE;
    pub const CHARNO_BITS: u32 = 12;
    pub const MAX_COLUMN_NUMBER: i32 = (1 << Self::CHARNO_BITS) - 1;
    // port: Node#getNonJSDocCommentString
    pub fn get_non_jsdoc_comment_string(self, ast: &Ast) -> JsString {
        self.get_non_jsdoc_comment(ast)
            .map_or_else(JsString::default, |c| c.get_comment_string())
    }
    // port: Node#getNonJSDocComment
    pub fn get_non_jsdoc_comment(self, ast: &Ast) -> Option<Arc<NonJSDocComment>> {
        self.get_prop(ast, Prop::NON_JSDOC_COMMENT)
            .map(|v| match v {
                ObjectProp::NonJSDocComment(c) => c,
                _ => panic!("ClassCastException"),
            })
    }
    // port: Node#getTrailingNonJSDocCommentString
    pub fn get_trailing_non_jsdoc_comment_string(self, ast: &Ast) -> JsString {
        self.get_trailing_non_jsdoc_comment(ast)
            .map_or_else(JsString::default, |c| c.get_comment_string())
    }
    // port: Node#getTrailingNonJSDocComment
    pub fn get_trailing_non_jsdoc_comment(self, ast: &Ast) -> Option<Arc<NonJSDocComment>> {
        self.get_prop(ast, Prop::TRAILING_NON_JSDOC_COMMENT)
            .map(|v| match v {
                ObjectProp::NonJSDocComment(c) => c,
                _ => panic!("ClassCastException"),
            })
    }
    // port: Node#setNonJSDocComment
    pub fn set_non_jsdoc_comment(
        self,
        ast: &mut Ast,
        comment: Option<Arc<NonJSDocComment>>,
    ) -> Self {
        self.put_prop(
            ast,
            Prop::NON_JSDOC_COMMENT,
            comment.map(ObjectProp::NonJSDocComment),
        );
        self
    }
    // port: Node#setTrailingNonJSDocComment
    pub fn set_trailing_non_jsdoc_comment(
        self,
        ast: &mut Ast,
        comment: Option<Arc<NonJSDocComment>>,
    ) -> Self {
        self.put_prop(
            ast,
            Prop::TRAILING_NON_JSDOC_COMMENT,
            comment.map(ObjectProp::NonJSDocComment),
        );
        self
    }
    // port: Node#setIsParenthesized
    pub fn set_is_parenthesized(self, ast: &mut Ast, b: bool) {
        check_state!(IR::may_be_expression(ast, self));
        self.put_boolean_prop(ast, Prop::IS_PARENTHESIZED, b);
    }
    // port: Node#getIsParenthesized
    pub fn get_is_parenthesized(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::IS_PARENTHESIZED)
    }
    // port: Node#getToken
    pub fn get_token(self, ast: &Ast) -> Token {
        ast[L(self)].token
    }
    // port: Node#setToken
    pub fn set_token(self, ast: &mut Ast, token: Token) {
        ast[L(self)].token = token;
    }
    // port: Node#hasChildren
    pub fn has_children(self, ast: &Ast) -> bool {
        ast[L(self)].first.is_some()
    }
    // port: Node#getOnlyChild
    pub fn get_only_child(self, ast: &Ast) -> NodeId {
        check_state!(self.has_one_child(ast));
        ast[L(self)].first.unwrap()
    }
    // port: Node#getFirstChild
    pub fn get_first_child(self, ast: &Ast) -> Option<NodeId> {
        ast[L(self)].first
    }
    // port: Node#getFirstFirstChild
    pub fn get_first_first_child(self, ast: &Ast) -> Option<NodeId> {
        ast[L(ast[L(self)].first.unwrap())].first
    }
    // port: Node#getSecondChild
    pub fn get_second_child(self, ast: &Ast) -> Option<NodeId> {
        ast[L(ast[L(self)].first.unwrap())].next
    }
    // port: Node#getLastChild
    pub fn get_last_child(self, ast: &Ast) -> Option<NodeId> {
        ast[L(self)].first.and_then(|f| ast[L(f)].previous)
    }
    // port: Node#getNext
    pub fn get_next(self, ast: &Ast) -> Option<NodeId> {
        ast[L(self)].next
    }
    // port: Node#getPrevious
    pub fn get_previous(self, ast: &Ast) -> Option<NodeId> {
        if ast[L(ast[L(self)].parent.unwrap())].first == Some(self) {
            None
        } else {
            ast[L(self)].previous
        }
    }
    // port: Node#setClosureUnawareShadow
    pub fn set_closure_unaware_shadow(self, ast: &mut Ast, shadow_root: Option<NodeId>) {
        check_state!(
            ast[L(self)].first.is_none(),
            "Cannot set shadow root on a node with children"
        );
        check_state!(
            ast[L(self)].token == Token::NAME,
            "Only NAME nodes can be used as shadows"
        );
        self.put_prop(
            ast,
            Prop::CLOSURE_UNAWARE_SHADOW,
            shadow_root.map(ObjectProp::Node),
        );
    }
    // port: Node#getClosureUnawareShadow
    pub fn get_closure_unaware_shadow(self, ast: &Ast) -> Option<NodeId> {
        if ast[L(self)].token != Token::NAME {
            return None;
        }
        self.get_prop(ast, Prop::CLOSURE_UNAWARE_SHADOW)
            .map(|v| match v {
                ObjectProp::Node(n) => n,
                _ => panic!("ClassCastException"),
            })
    }
    // port: Node#setIsInClosureUnawareSubtree
    pub fn set_is_in_closure_unaware_subtree(self, ast: &mut Ast, value: bool) {
        check_argument!(
            ast[self]
                .prop_list_head
                .as_ref()
                .is_some_and(|h| h.prop_type == Prop::SOURCE_FILE as u8 && h.next.is_none()),
            "Cannot set IS_IN_CLOSURE_UNAWARE_SUBTREE property on a node which proplist is not a single SOURCE_FILE prop: %s %s",
            self.to_string(ast),
            self.get_prop_list_debug_string(ast)
        );
        self.put_boolean_prop(ast, Prop::IS_IN_CLOSURE_UNAWARE_SUBTREE, value);
    }
    // port: Node#getIsInClosureUnawareSubtree
    pub fn get_is_in_closure_unaware_subtree(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::IS_IN_CLOSURE_UNAWARE_SUBTREE)
    }
    // port: Node#getChildAtIndex
    pub fn get_child_at_index(self, ast: &Ast, mut i: i32) -> Option<NodeId> {
        let mut n = ast[L(self)].first;
        while i > 0 {
            n = ast[L(n.unwrap())].next;
            i -= 1;
        }
        n
    }
    // port: Node#getIndexOfChild
    pub fn get_index_of_child(self, ast: &Ast, child: NodeId) -> i32 {
        let mut n = ast[L(self)].first;
        let mut i = 0;
        while let Some(cur) = n {
            if child == cur {
                return i;
            }
            n = ast[L(cur)].next;
            i += 1;
        }
        -1
    }
    // port: Node#addChildToFront
    pub fn add_child_to_front(self, ast: &mut Ast, child: NodeId) {
        check_argument!(ast[L(child)].parent.is_none());
        check_argument!(ast[L(child)].next.is_none());
        check_argument!(ast[L(child)].previous.is_none());
        ast[L(child)].parent = Some(self);
        ast[L(child)].next = ast[L(self)].first;
        if let Some(first) = ast[L(self)].first {
            let last = ast[L(first)].previous;
            ast[L(child)].previous = last;
            ast[L(child)].next = Some(first);
            ast[L(first)].previous = Some(child);
        } else {
            ast[L(child)].previous = Some(child);
        }
        ast[L(self)].first = Some(child);
    }
    // port: Node#addChildToBack
    pub fn add_child_to_back(self, ast: &mut Ast, child: NodeId) {
        check_argument!(
            ast[L(child)].parent.is_none(),
            "Cannot add already-owned child node.\nChild: %s\nExisting parent: %s\nNew parent: %s",
            child.to_string(ast),
            ast[L(child)]
                .parent
                .map_or_else(|| "null".into(), |p| p.to_string(ast)),
            self.to_string(ast)
        );
        check_argument!(ast[L(child)].next.is_none());
        check_argument!(ast[L(child)].previous.is_none());
        if let Some(first) = ast[L(self)].first {
            let last = ast[L(first)].previous.unwrap();
            ast[L(last)].next = Some(child);
            ast[L(child)].previous = Some(last);
            ast[L(first)].previous = Some(child);
        } else {
            ast[L(child)].previous = Some(child);
            ast[L(self)].first = Some(child);
        }
        ast[L(child)].parent = Some(self);
    }
    // port: Node#addChildrenToFront
    pub fn add_children_to_front(self, ast: &mut Ast, children: Option<NodeId>) {
        let Some(children) = children else {
            return;
        };
        check_not_null!(ast[L(children)].previous, "%s", children.to_string(ast));
        let mut child = Some(children);
        while let Some(c) = child {
            check_argument!(ast[L(c)].parent.is_none());
            ast[L(c)].parent = Some(self);
            child = ast[L(c)].next;
        }
        let last_sib = ast[L(children)].previous.unwrap();
        if let Some(first) = ast[L(self)].first {
            let last = ast[L(first)].previous;
            ast[L(children)].previous = last;
            ast[L(last_sib)].next = Some(first);
            ast[L(first)].previous = Some(last_sib);
        }
        ast[L(self)].first = Some(children);
    }
    // port: Node#addChildrenToBack
    pub fn add_children_to_back(self, ast: &mut Ast, children: Option<NodeId>) {
        self.add_children_after(ast, children, self.get_last_child(ast));
    }
    // port: Node#insertAfter
    pub fn insert_after(self, ast: &mut Ast, existing: NodeId) {
        existing.check_attached(ast);
        self.check_detached(ast);
        let existing_parent = ast[L(existing)].parent.unwrap();
        let existing_next = ast[L(existing)].next;
        ast[L(self)].parent = Some(existing_parent);
        ast[L(existing)].next = Some(self);
        ast[L(self)].previous = Some(existing);
        if let Some(next) = existing_next {
            ast[L(next)].previous = Some(self);
            ast[L(self)].next = Some(next);
        } else {
            let first = ast[L(existing_parent)].first.unwrap();
            ast[L(first)].previous = Some(self);
        }
    }
    // port: Node#insertBefore
    pub fn insert_before(self, ast: &mut Ast, existing: NodeId) {
        existing.check_attached(ast);
        self.check_detached(ast);
        let existing_parent = ast[L(existing)].parent.unwrap();
        let existing_previous = ast[L(existing)].previous.unwrap();
        ast[L(self)].parent = Some(existing_parent);
        ast[L(self)].next = Some(existing);
        ast[L(existing)].previous = Some(self);
        ast[L(self)].previous = Some(existing_previous);
        if ast[L(existing_previous)].next.is_none() {
            ast[L(existing_parent)].first = Some(self);
        } else {
            ast[L(existing_previous)].next = Some(self);
        }
    }
    // port: Node#addChildrenAfter
    pub fn add_children_after(self, ast: &mut Ast, children: Option<NodeId>, node: Option<NodeId>) {
        let Some(children) = children else {
            return;
        };
        check_argument!(node.is_none_or(|n| ast[L(n)].parent == Some(self)));
        check_not_null!(ast[L(children)].previous, "%s", children.to_string(ast));
        let Some(node) = node else {
            self.add_children_to_front(ast, Some(children));
            return;
        };
        let mut child = Some(children);
        while let Some(c) = child {
            check_argument!(ast[L(c)].parent.is_none());
            ast[L(c)].parent = Some(self);
            child = ast[L(c)].next;
        }
        let last_sibling = ast[L(children)].previous.unwrap();
        let node_after = ast[L(node)].next;
        ast[L(last_sibling)].next = node_after;
        if let Some(after) = node_after {
            ast[L(after)].previous = Some(last_sibling);
        } else {
            let first = ast[L(self)].first.unwrap();
            ast[L(first)].previous = Some(last_sibling);
        }
        ast[L(node)].next = Some(children);
        ast[L(children)].previous = Some(node);
    }
    // port: Node#replaceWith
    pub fn replace_with(self, ast: &mut Ast, replacement: NodeId) {
        self.check_attached(ast);
        replacement.check_detached(ast);
        let existing_parent = ast[L(self)].parent.unwrap();
        let existing_next = ast[L(self)].next;
        let existing_previous = ast[L(self)].previous.unwrap();
        replacement.srcref_if_missing(ast, self);
        ast[L(self)].parent = None;
        ast[L(replacement)].parent = Some(existing_parent);
        ast[L(self)].previous = None;
        ast[L(replacement)].previous = Some(existing_previous);
        if ast[L(existing_previous)].next.is_none() {
            ast[L(existing_parent)].first = Some(replacement);
        } else {
            ast[L(existing_previous)].next = Some(replacement);
        }
        if let Some(next) = existing_next {
            ast[L(self)].next = None;
            ast[L(next)].previous = Some(replacement);
            ast[L(replacement)].next = Some(next);
        } else {
            let first = ast[L(existing_parent)].first.unwrap();
            ast[L(first)].previous = Some(replacement);
        }
    }
    // port: Node#detach
    pub fn detach(self, ast: &mut Ast) -> Self {
        self.check_attached(ast);
        let existing_parent = ast[L(self)].parent.unwrap();
        let existing_next = ast[L(self)].next;
        let existing_previous = ast[L(self)].previous.unwrap();
        ast[L(self)].parent = None;
        if let Some(next) = existing_next {
            ast[L(self)].next = None;
            ast[L(next)].previous = Some(existing_previous);
        } else {
            let first = ast[L(existing_parent)].first.unwrap();
            ast[L(first)].previous = Some(existing_previous);
        }
        ast[L(self)].previous = None;
        if ast[L(existing_previous)].next.is_none() {
            ast[L(existing_parent)].first = existing_next;
        } else {
            ast[L(existing_previous)].next = existing_next;
        }
        self
    }
    // port: Node#checkAttached
    fn check_attached(self, ast: &Ast) {
        check_state!(
            ast[L(self)].parent.is_some(),
            "Has no parent: %s",
            self.to_string(ast)
        );
    }
    // port: Node#checkDetached
    fn check_detached(self, ast: &Ast) {
        check_state!(
            ast[L(self)].parent.is_none(),
            "Has parent: %s",
            self.to_string(ast)
        );
        check_state!(
            ast[L(self)].next.is_none(),
            "Has next: %s",
            self.to_string(ast)
        );
        check_state!(
            ast[L(self)].previous.is_none(),
            "Has previous: %s",
            self.to_string(ast)
        );
    }
    // port: Node#removeFirstChild
    pub fn remove_first_child(self, ast: &mut Ast) -> Option<NodeId> {
        let child = ast[L(self)].first;
        if let Some(child) = child {
            child.detach(ast);
        }
        child
    }
    // port: Node#removeChildren
    pub fn remove_children(self, ast: &mut Ast) -> Option<NodeId> {
        let children = ast[L(self)].first;
        let mut child = children;
        while let Some(c) = child {
            ast[L(c)].parent = None;
            child = ast[L(c)].next;
        }
        ast[L(self)].first = None;
        children
    }
    // port: Node#detachChildren
    pub fn detach_children(self, ast: &mut Ast) {
        let mut child = ast[L(self)].first;
        while let Some(c) = child {
            let next_child = ast[L(c)].next;
            ast[L(c)].parent = None;
            ast[L(c)].next = None;
            ast[L(c)].previous = None;
            child = next_child;
        }
        ast[L(self)].first = None;
    }
    // port: Node#lookupProperty
    pub fn lookup_property(self, ast: &Ast, prop: Prop) -> Option<Arc<PropListItem>> {
        self.lookup_property_ref(ast, prop).cloned()
    }
    // Java walks the list by reference; walking with borrows avoids an Arc clone (two atomic
    // operations) per list item.
    fn lookup_property_ref(self, ast: &Ast, prop: Prop) -> Option<&Arc<PropListItem>> {
        let prop_type = prop as u8;
        let mut x = ast[self].prop_list_head.as_ref();
        while let Some(item) = x {
            if item.prop_type == prop_type {
                return Some(item);
            }
            x = item.next.as_ref();
        }
        None
    }
    // port: Node#clonePropsFrom
    pub fn clone_props_from(self, ast: &mut Ast, other: NodeId) -> Self {
        check_state!(
            ast[self].prop_list_head.is_none(),
            "Node has existing properties."
        );
        ast[self].prop_list_head = ast[other].prop_list_head.clone();
        self
    }
    // port: Node#validateProperties
    pub fn validate_properties(self, ast: &Ast, mut consumer: impl FnMut(String)) {
        if ast[self].prop_list_head.is_none() {
            return;
        }
        if ast[L(self)].token == Token::ROOT {
            consumer("ROOT has properties".into());
        }
        let mut item = ast[self].prop_list_head.clone();
        while let Some(p) = item {
            let prop = Prop::VALUES[p.prop_type as usize];
            check_state!(
                prop as u8 == p.prop_type,
                "ordinal doesn't match: %s",
                format!("{prop:?}")
            );
            match prop {
                Prop::IS_PARENTHESIZED => {
                    if !IR::may_be_expression(ast, self) {
                        consumer("non-expression is parenthesized".into());
                    }
                }
                Prop::ARROW_FN => {
                    if !self.is_function(ast) {
                        consumer("invalid ARROW_FN prop".into());
                    }
                }
                Prop::ASYNC_FN => {
                    if !self.is_function(ast) {
                        consumer("invalid ASYNC_FN prop".into());
                    }
                }
                Prop::SYNTHETIC => {
                    if !self.is_block(ast) {
                        consumer("invalid SYNTHETIC prop".into());
                    }
                }
                Prop::COLOR_FROM_CAST => {
                    if self.get_color(ast).is_none() {
                        consumer("COLOR_FROM_CAST with no Color".into());
                    }
                }
                Prop::START_OF_OPT_CHAIN => {
                    if !(self.is_opt_chain_call(ast)
                        || self.is_opt_chain_get_elem(ast)
                        || self.is_opt_chain_get_prop(ast))
                    {
                        consumer("START_OF_OPT_CHAIN on non-optional Node".into());
                    }
                }
                Prop::CONSTANT_VAR_FLAGS => {
                    if !(self.is_name(ast) || self.is_import_star(ast)) {
                        consumer("invalid CONST_VAR_FLAGS".into());
                    }
                }
                Prop::SYNTHESIZED_UNFULFILLED_NAME_DECLARATION => {
                    if !self.is_var(ast) {
                        consumer(
                            "Expected all synthetic unfulfilled declarations to be `var <name>`"
                                .into(),
                        );
                    }
                }
                Prop::CLOSURE_UNAWARE_SHADOW => {
                    if !matches!(
                        self.lookup_property(ast, prop).unwrap().value,
                        PropValue::Object(ObjectProp::Node(_))
                    ) {
                        consumer("CLOSURE_UNAWARE_SHADOW property must point to a Node".into());
                    }
                }
                _ => {}
            }
            item = p.next.clone();
        }
    }
    // port: Node#rebuildListWithoutProp
    fn rebuild_list_without_prop(
        item: Option<Arc<PropListItem>>,
        prop: Prop,
    ) -> Option<Arc<PropListItem>> {
        let Some(item) = item else {
            return None;
        };
        if item.prop_type == prop as u8 {
            item.next.clone()
        } else {
            let result = Self::rebuild_list_without_prop(item.next.clone(), prop);
            if arc_option_ptr_eq(&result, &item.next) {
                Some(item)
            } else {
                Some(item.chain(result))
            }
        }
    }
    // port: Node#getProp
    pub fn get_prop(self, ast: &Ast, prop_type: Prop) -> Option<ObjectProp> {
        self.lookup_property_ref(ast, prop_type)
            .map(|i| i.get_object_value())
    }
    // port: Node#getBooleanProp
    pub fn get_boolean_prop(self, ast: &Ast, prop_type: Prop) -> bool {
        self.get_int_prop(ast, prop_type) != 0
    }
    // port: Node#getIntProp
    pub fn get_int_prop(self, ast: &Ast, prop_type: Prop) -> i32 {
        self.lookup_property_ref(ast, prop_type)
            .map_or(0, |i| i.get_int_value())
    }
    // port: Node#putProp
    pub fn put_prop(self, ast: &mut Ast, prop: Prop, value: Option<ObjectProp>) {
        self.remove_prop(ast, prop);
        if let Some(value) = value {
            let head = ast[self].prop_list_head.take();
            ast[self].prop_list_head = Some(PropListItem::object(prop as u8, value, head));
        }
    }
    // port: Node#putBooleanProp
    pub fn put_boolean_prop(self, ast: &mut Ast, prop_type: Prop, value: bool) {
        self.put_int_prop(ast, prop_type, if value { 1 } else { 0 });
    }
    // port: Node#putIntProp
    pub fn put_int_prop(self, ast: &mut Ast, prop: Prop, value: i32) {
        self.remove_prop(ast, prop);
        if value != 0 {
            let head = ast[self].prop_list_head.take();
            ast[self].prop_list_head = Some(PropListItem::int(prop as u8, value, head));
        }
    }
    // port: Node#removeProp
    pub fn remove_prop(self, ast: &mut Ast, prop: Prop) {
        // Rust-only: rebuildListWithoutProp returns the list itself when `prop` is absent; skip
        // the walk that copies references then (D-025).
        if self.lookup_property_ref(ast, prop).is_none() {
            return;
        }
        let head = ast[self].prop_list_head.take();
        ast[self].prop_list_head = Self::rebuild_list_without_prop(head, prop);
    }
    // port: Node#nodePropertyToBit
    pub fn node_property_to_bit(prop: NodeProperty) -> i64 {
        1i64.wrapping_shl(prop.get_number() as u32)
    }
    // port: Node#setNodePropertyBit
    pub fn set_node_property_bit(bitset: i64, prop: NodeProperty) -> i64 {
        bitset | Self::node_property_to_bit(prop)
    }
    // port: Node#removeNodePropertyBit
    pub fn remove_node_property_bit(bitset: i64, prop: NodeProperty) -> i64 {
        bitset & !Self::node_property_to_bit(prop)
    }
    // port: Node#hasNodePropertyBitSet
    pub fn has_node_property_bit_set(bitset: i64, prop: NodeProperty) -> bool {
        bitset & Self::node_property_to_bit(prop) != 0
    }
    // port: Node#hasBitSet
    pub fn has_bit_set(bitset: i64, bit: i32) -> bool {
        bitset & 1i64.wrapping_shl(bit as u32) != 0
    }
    // port: Node#serializeProperties
    pub fn serialize_properties(self, ast: &Ast) -> i64 {
        let mut prop_set = 0;
        let mut item = ast[self].prop_list_head.clone();
        while let Some(p) = item {
            let prop = Prop::VALUES[p.prop_type as usize];
            match prop {
                Prop::TYPE_BEFORE_CAST => {
                    prop_set = Self::set_node_property_bit(prop_set, NodeProperty::COLOR_FROM_CAST)
                }
                Prop::CONSTANT_VAR_FLAGS => {
                    let int_val = p.get_int_value();
                    if Self::any_bit_set(int_val, ConstantVarFlags::INFERRED) {
                        prop_set = Self::set_node_property_bit(
                            prop_set,
                            NodeProperty::IS_INFERRED_CONSTANT,
                        );
                    }
                    if Self::any_bit_set(int_val, ConstantVarFlags::DECLARED) {
                        prop_set = Self::set_node_property_bit(
                            prop_set,
                            NodeProperty::IS_DECLARED_CONSTANT,
                        );
                    }
                }
                Prop::SIDE_EFFECT_FLAGS => {
                    prop_set = self.set_node_property_side_effect_flags(prop_set, p.get_int_value())
                }
                Prop::CLOSURE_UNAWARE_SHADOW => {
                    prop_set =
                        Self::set_node_property_bit(prop_set, NodeProperty::CLOSURE_UNAWARE_SHADOW)
                }
                _ => {
                    if matches!(p.value, PropValue::Int(_)) {
                        if let Some(node_property) = PropTranslator::serialize(prop) {
                            prop_set = Self::set_node_property_bit(prop_set, node_property);
                        }
                    }
                }
            }
            item = p.next.clone();
        }
        prop_set
    }
    // port: Node#setNodePropertySideEffectFlags
    fn set_node_property_side_effect_flags(self, mut prop_set: i64, side_effect_flags: i32) -> i64 {
        if Self::any_bit_set(side_effect_flags, SideEffectFlags::MUTATES_GLOBAL_STATE) {
            prop_set = Self::set_node_property_bit(prop_set, NodeProperty::MUTATES_GLOBAL_STATE);
        }
        if Self::any_bit_set(side_effect_flags, SideEffectFlags::MUTATES_THIS) {
            prop_set = Self::set_node_property_bit(prop_set, NodeProperty::MUTATES_THIS);
        }
        if Self::any_bit_set(side_effect_flags, SideEffectFlags::MUTATES_ARGUMENTS) {
            prop_set = Self::set_node_property_bit(prop_set, NodeProperty::MUTATES_ARGUMENTS);
        }
        if Self::any_bit_set(side_effect_flags, SideEffectFlags::THROWS) {
            prop_set = Self::set_node_property_bit(prop_set, NodeProperty::THROWS);
        }
        prop_set
    }
    // port: Node#getPropListDebugString
    pub fn get_prop_list_debug_string(self, ast: &Ast) -> String {
        let mut item = ast[self].prop_list_head.clone();
        let mut repr = Vec::new();
        while let Some(p) = item {
            repr.push(format!("{:?}", Prop::VALUES[p.prop_type as usize]));
            item = p.next.clone();
        }
        repr.join("->")
    }
    // port: Node#deserializeProperties
    pub fn deserialize_properties(
        self,
        ast: &mut Ast,
        prop_set: i64,
        is_in_closure_unaware_subtree: bool,
    ) {
        if self.is_root(ast) {
            check_state!(
                ast[self].prop_list_head.is_none(),
                "%s",
                ast[self].prop_list_head.as_ref().unwrap().to_string(ast)
            );
        } else {
            check_state!(
                self.validate_prop_list_tail_ordering(ast, is_in_closure_unaware_subtree),
                "Node not ready to receive properties"
            );
        }
        let mut constant_var_flags = 0;
        let mut side_effect_flags = 0;
        for i in 0..63 {
            if !Self::has_bit_set(prop_set, i) {
                continue;
            }
            let node_property = NodeProperty::for_number(i).expect("NullPointerException");
            match node_property {
                NodeProperty::IS_DECLARED_CONSTANT => {
                    constant_var_flags |= ConstantVarFlags::DECLARED
                }
                NodeProperty::IS_INFERRED_CONSTANT => {
                    constant_var_flags |= ConstantVarFlags::INFERRED
                }
                NodeProperty::MUTATES_GLOBAL_STATE => {
                    side_effect_flags |= SideEffectFlags::MUTATES_GLOBAL_STATE
                }
                NodeProperty::MUTATES_THIS => side_effect_flags |= SideEffectFlags::MUTATES_THIS,
                NodeProperty::MUTATES_ARGUMENTS => {
                    side_effect_flags |= SideEffectFlags::MUTATES_ARGUMENTS
                }
                NodeProperty::THROWS => side_effect_flags |= SideEffectFlags::THROWS,
                NodeProperty::CLOSURE_UNAWARE_SHADOW => {}
                _ => {
                    let prop = PropTranslator::deserialize(node_property)
                        .unwrap_or_else(|| panic!("Can not translate {node_property} to AST Prop"));
                    ast[self].prop_list_head = Some(PropListItem::int(
                        prop as u8,
                        1,
                        ast[self].prop_list_head.clone(),
                    ));
                }
            }
        }
        if constant_var_flags != 0 {
            ast[self].prop_list_head = Some(PropListItem::int(
                Prop::CONSTANT_VAR_FLAGS as u8,
                constant_var_flags,
                ast[self].prop_list_head.clone(),
            ));
        }
        if side_effect_flags != 0 {
            ast[self].prop_list_head = Some(PropListItem::int(
                Prop::SIDE_EFFECT_FLAGS as u8,
                side_effect_flags,
                ast[self].prop_list_head.clone(),
            ));
        }
        self.validate_properties(ast, |error_message| {
            panic!(
                "deserialize error: {error_message}: {}",
                self.to_string(ast)
            )
        });
    }
    // port: Node#setDeclaredTypeExpression
    pub fn set_declared_type_expression(self, ast: &mut Ast, type_expression: Option<NodeId>) {
        self.put_prop(
            ast,
            Prop::DECLARED_TYPE_EXPR,
            type_expression.map(ObjectProp::Node),
        );
    }
    // port: Node#getDeclaredTypeExpression
    pub fn get_declared_type_expression(self, ast: &Ast) -> Option<NodeId> {
        self.get_prop(ast, Prop::DECLARED_TYPE_EXPR)
            .map(|v| match v {
                ObjectProp::Node(n) => n,
                _ => panic!("ClassCastException"),
            })
    }
    // port: Node#setJSTypeBeforeCast
    pub fn set_jstype_before_cast(self, ast: &mut Ast, t: Option<TypeId>) {
        self.put_prop(ast, Prop::TYPE_BEFORE_CAST, t.map(ObjectProp::JSType));
    }
    // port: Node#getJSTypeBeforeCast
    pub fn get_jstype_before_cast(self, ast: &Ast) -> Option<TypeId> {
        self.get_prop(ast, Prop::TYPE_BEFORE_CAST).map(|v| match v {
            ObjectProp::JSType(t) => t,
            _ => panic!("ClassCastException"),
        })
    }
    // port: Node#setColorFromTypeCast
    pub fn set_color_from_type_cast(self, ast: &mut Ast) {
        check_state!(
            self.get_color(ast).is_some(),
            "Only use on nodes with colors present"
        );
        self.put_boolean_prop(ast, Prop::COLOR_FROM_CAST, true);
    }
    // port: Node#isColorFromTypeCast
    pub fn is_color_from_type_cast(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::COLOR_FROM_CAST)
    }
    // port: Node#getSortedPropTypes
    fn get_sorted_prop_types(self, ast: &Ast) -> Vec<u8> {
        let mut item = ast[self].prop_list_head.clone();
        let mut keys = Vec::new();
        while let Some(p) = item {
            keys.push(p.prop_type);
            item = p.next.clone();
        }
        keys.sort_unstable();
        keys
    }
    // port: Node#getDouble
    pub fn get_double(self, ast: &Ast) -> f64 {
        match ast[self].kind {
            NodeKind::Number { number } => number,
            _ => panic!("ClassCastException"),
        }
    }
    // port: Node#setDouble
    pub fn set_double(self, ast: &mut Ast, x: f64) {
        check_state!(!x.is_nan(), "%s", double_to_string(x));
        check_state!(JSCompDoubles::is_positive(x), "%s", double_to_string(x));
        match &mut ast[self].kind {
            NodeKind::Number { number } => *number = x,
            _ => panic!("ClassCastException"),
        }
    }
    // port: Node#getBigInt
    pub fn get_big_int(self, ast: &Ast) -> Arc<BigInt> {
        match &ast[self].kind {
            NodeKind::BigInt { bigint } => bigint.clone(),
            _ => panic!("ClassCastException"),
        }
    }
    // port: Node#setBigInt
    pub fn set_big_int(self, ast: &mut Ast, number: impl Into<Arc<BigInt>>) {
        let number = number.into();
        check_state!(number.sign() != Sign::Minus, "%s", number);
        match &mut ast[self].kind {
            NodeKind::BigInt { bigint } => *bigint = number,
            _ => panic!("ClassCastException"),
        }
    }
    // port: Node#getString
    pub fn get_string(self, ast: &Ast) -> JsString {
        self.get_string_ref(ast).clone()
    }
    // port: Node#getString
    /// Rust-only: `get_string` by reference, for readers that need no owned copy (saves the
    /// reference-count traffic of a clone).
    pub fn get_string_ref(self, ast: &Ast) -> &JsString {
        match &ast[self].kind {
            NodeKind::String { str } => str,
            _ => panic!("ClassCastException"),
        }
    }
    // port: Node#setString
    pub fn set_string(self, ast: &mut Ast, s: impl Into<JsString>) {
        match &mut ast[self].kind {
            NodeKind::String { str } => *str = RhinoStringPool::add_or_get(s),
            _ => panic!("ClassCastException"),
        }
    }
    // port: Node#setStringFromStringPool
    pub fn set_string_from_string_pool(
        self,
        ast: &mut Ast,
        pool: &LazyInternedStringList,
        offset: i32,
    ) {
        match &mut ast[self].kind {
            NodeKind::String { str } => *str = pool.get(offset),
            _ => panic!("ClassCastException"),
        }
    }
    // port: Node#getRawString
    pub fn get_raw_string(self, ast: &Ast) -> JsString {
        match &ast[self].kind {
            NodeKind::TemplateLiteralSubstring(strings) => strings.raw.clone(),
            _ => panic!("ClassCastException"),
        }
    }
    // port: Node#getCookedString
    pub fn get_cooked_string(self, ast: &Ast) -> Option<JsString> {
        match &ast[self].kind {
            NodeKind::TemplateLiteralSubstring(strings) => strings.cooked.clone(),
            _ => panic!("ClassCastException"),
        }
    }
    // port: Node#toString()
    pub fn to_string(self, ast: &Ast) -> String {
        self.to_string_with_options(ast, true, true, true)
    }
    // port: Node#toString(boolean, boolean, boolean)
    pub fn to_string_with_options(
        self,
        ast: &Ast,
        print_source: bool,
        print_annotations: bool,
        print_type: bool,
    ) -> String {
        crate::java_lang::charset::utf8_encoded_text(
            self.to_string_with_options_utf16(ast, print_source, print_annotations, print_type)
                .as_units(),
        )
    }
    // port: Node#toString
    pub fn to_string_utf16(self, ast: &Ast) -> JsString {
        self.to_string_with_options_utf16(ast, true, true, true)
    }
    // port: Node#toString(boolean, boolean, boolean)
    pub fn to_string_with_options_utf16(
        self,
        ast: &Ast,
        print_source: bool,
        print_annotations: bool,
        print_type: bool,
    ) -> JsString {
        self.to_string_with_options_and_types_utf16(
            ast,
            print_source,
            print_annotations,
            print_type,
            &mut no_registry_jstype_printer,
        )
    }
    // port: Node#toString(boolean, boolean, boolean)
    /// `toString(printSource, printAnnotations, printType)` with a registry-aware JSType
    /// printer (see [`JSTypePrinter`]).
    pub fn to_string_with_options_and_types_utf16(
        self,
        ast: &Ast,
        print_source: bool,
        print_annotations: bool,
        print_type: bool,
        type_printer: &mut JSTypePrinter<'_>,
    ) -> JsString {
        let mut sb = crate::java_lang::string_builder::StringBuilder::new();
        self.to_string_into(
            ast,
            &mut sb,
            print_source,
            print_annotations,
            print_type,
            type_printer,
        );
        sb.to_js_string()
    }
    // port: Node#toString(StringBuilder, boolean, boolean, boolean)
    fn to_string_into(
        self,
        ast: &Ast,
        sb: &mut crate::java_lang::string_builder::StringBuilder,
        print_source: bool,
        print_annotations: bool,
        print_type: bool,
        type_printer: &mut JSTypePrinter<'_>,
    ) {
        use fmt::Write;
        write!(sb, "{}", ast[L(self)].token).unwrap();
        if matches!(ast[self].kind, NodeKind::String { .. }) {
            sb.write_char(' ').unwrap();
            sb.append(&self.get_string(ast));
        } else if ast[L(self)].token == Token::FUNCTION {
            sb.write_char(' ').unwrap();
            if ast[L(self)]
                .first
                .is_none_or(|f| ast[L(f)].token != Token::NAME)
            {
                sb.write_str("<invalid>").unwrap();
            } else {
                sb.append(&ast[L(self)].first.unwrap().get_string(ast));
            }
        } else if ast[L(self)].token == Token::NUMBER {
            write!(sb, " {}", double_to_string(self.get_double(ast))).unwrap();
        }
        if print_source {
            let lineno = self.get_lineno(ast);
            if lineno != -1 {
                write!(sb, " {lineno}:{} ", self.get_charno(ast)).unwrap();
            }
            if ast[self].length != 0 {
                write!(sb, " [length: {}]", ast[self].length).unwrap();
            }
        }
        if print_annotations {
            for key in self.get_sorted_prop_types(ast) {
                let prop = Prop::VALUES[key as usize];
                if prop == Prop::CLOSURE_UNAWARE_SHADOW {
                    sb.write_str(" [is_shadow_host]").unwrap();
                    continue;
                }
                let x = self.lookup_property(ast, prop).unwrap();
                write!(sb, " [{}: ", format!("{prop:?}").to_ascii_lowercase()).unwrap();
                sb.append(&x.to_string_with_types_utf16(ast, type_printer));
                sb.write_char(']').unwrap();
            }
            if let Some(original_name) = &ast[self].original_name {
                sb.write_str(" [original_name: ").unwrap();
                sb.append(original_name);
                sb.write_char(']').unwrap();
            }
        }
        if print_type && let Some(ty) = &ast[self].jstype_or_color {
            match ty {
                JSTypeOrColor::Color(color) => write!(sb, " : {color}").unwrap(),
                JSTypeOrColor::JSType(t) => {
                    let type_string = type_printer(*t);
                    sb.write_str(" : ").unwrap();
                    sb.append(&type_string);
                }
            }
        }
    }
    // port: Node#createJsonPair
    fn create_json_pair(name: &str, value: &JsString) -> JsString {
        JsString::from(format!("\"{name}\":\""))
            .concat(value)
            .concat(&"\"".into())
    }
    // port: Node#createJsonPairRawValue
    fn create_json_pair_raw_value(name: &str, value: &str) -> String {
        format!("\"{name}\":{value}")
    }
    // port: Node#toJson
    fn to_json(
        self,
        ast: &Ast,
        sb: &mut crate::java_lang::string_builder::StringBuilder,
        type_printer: &mut JSTypePrinter<'_>,
    ) -> fmt::Result {
        use fmt::Write;
        sb.write_char('{')?;
        sb.append(&Self::create_json_pair(
            "token",
            &ast[L(self)].token.to_string().into(),
        ));
        if matches!(ast[self].kind, NodeKind::String { .. }) {
            sb.write_char(',')?;
            sb.append(&Self::create_json_pair("string", &self.get_string(ast)));
        } else if ast[L(self)].token == Token::FUNCTION {
            sb.write_char(',')?;
            let name = if ast[L(self)]
                .first
                .is_none_or(|f| ast[L(f)].token != Token::NAME)
            {
                "<invalid>".into()
            } else {
                ast[L(self)].first.unwrap().get_string(ast)
            };
            sb.append(&Self::create_json_pair("functionName", &name));
        } else if ast[L(self)].token == Token::NUMBER {
            sb.write_char(',')?;
            sb.append(&Self::create_json_pair(
                "number",
                &double_to_string(self.get_double(ast)).into(),
            ));
        }
        let lineno = self.get_lineno(ast);
        if lineno != -1 {
            sb.write_str(",\"sourceLocation\":{")?;
            sb.write_str(&Self::create_json_pair_raw_value(
                "line",
                &lineno.to_string(),
            ))?;
            sb.write_char(',')?;
            sb.write_str(&Self::create_json_pair_raw_value(
                "col",
                &self.get_charno(ast).to_string(),
            ))?;
            if ast[self].length != 0 {
                sb.write_char(',')?;
                sb.write_str(&Self::create_json_pair_raw_value(
                    "length",
                    &ast[self].length.to_string(),
                ))?;
            }
            sb.write_char('}')?;
        }
        if let Some(name) = &ast[self].original_name {
            sb.write_char(',')?;
            sb.append(&Self::create_json_pair("original_name", name));
        }
        let keys = self.get_sorted_prop_types(ast);
        if !keys.is_empty() {
            sb.write_str(",\"props\":{")?;
            for (i, key) in keys.iter().enumerate() {
                let prop = Prop::VALUES[*key as usize];
                let x = self.lookup_property(ast, prop).unwrap();
                sb.append(&Self::create_json_pair(
                    &format!("{prop:?}").to_ascii_lowercase(),
                    &x.to_string_with_types_utf16(ast, type_printer)
                        .replace(&"\n".into(), &"\\n".into())
                        .replace(&"\"".into(), &"\\\"".into()),
                ));
                if i + 1 < keys.len() {
                    sb.write_char(',')?;
                }
            }
            sb.write_char('}')?;
        }
        if let Some(ty) = &ast[self].jstype_or_color {
            match ty {
                JSTypeOrColor::Color(color) => {
                    sb.write_char(',')?;
                    sb.append(&Self::create_json_pair(
                        "typeString",
                        &color.to_string().replace('"', "\\\"").into(),
                    ));
                }
                JSTypeOrColor::JSType(t) => {
                    let type_string = type_printer(*t);
                    sb.write_char(',')?;
                    sb.append(&Self::create_json_pair(
                        "typeString",
                        &type_string.replace(&"\"".into(), &"\\\"".into()),
                    ));
                }
            }
        }
        if ast[L(self)].first.is_some() {
            sb.write_str(",\"children\":[")?;
            let mut child = ast[L(self)].first;
            while let Some(c) = child {
                c.to_json(ast, sb, type_printer)?;
                if ast[L(c)].next.is_some() {
                    sb.write_char(',')?;
                }
                child = ast[L(c)].next;
            }
            sb.write_char(']')?;
        }
        sb.write_char('}')
    }
    // port: Node#toStringTree
    pub fn to_string_tree(self, ast: &Ast) -> String {
        self.to_string_tree_impl(ast)
    }
    // port: Node#toStringTreeImpl
    fn to_string_tree_impl(self, ast: &Ast) -> String {
        crate::java_lang::charset::utf8_encoded_text(self.to_string_tree_utf16(ast).as_units())
    }
    // port: Node#toStringTree
    pub fn to_string_tree_utf16(self, ast: &Ast) -> JsString {
        self.to_string_tree_with_types_utf16(ast, &mut no_registry_jstype_printer)
    }
    // port: Node#toStringTree
    /// `toStringTree` with a registry-aware JSType printer (see [`JSTypePrinter`]).
    pub fn to_string_tree_with_types(
        self,
        ast: &Ast,
        type_printer: &mut JSTypePrinter<'_>,
    ) -> String {
        crate::java_lang::charset::utf8_encoded_text(
            self.to_string_tree_with_types_utf16(ast, type_printer)
                .as_units(),
        )
    }
    // port: Node#toStringTree
    /// `toStringTree` with a registry-aware JSType printer (see [`JSTypePrinter`]).
    pub fn to_string_tree_with_types_utf16(
        self,
        ast: &Ast,
        type_printer: &mut JSTypePrinter<'_>,
    ) -> JsString {
        let mut sb = crate::java_lang::string_builder::StringBuilder::new();
        Self::to_string_tree_helper(ast, self, 0, &mut sb, type_printer)
            .unwrap_or_else(|e| panic!("Should not happen\n{e}"));
        sb.to_js_string()
    }
    // port: Node#appendStringTree
    pub fn append_string_tree(self, ast: &Ast, appendable: &mut impl fmt::Write) -> fmt::Result {
        appendable.write_str(&crate::java_lang::charset::utf8_encoded_text(
            self.to_string_tree_utf16(ast).as_units(),
        ))
    }
    // port: Node#toStringTreeHelper
    fn to_string_tree_helper(
        ast: &Ast,
        n: NodeId,
        level: i32,
        sb: &mut crate::java_lang::string_builder::StringBuilder,
        type_printer: &mut JSTypePrinter<'_>,
    ) -> fmt::Result {
        use fmt::Write;
        for _ in 0..level {
            sb.write_str("    ")?;
        }
        sb.append(&n.to_string_with_options_and_types_utf16(ast, true, true, true, type_printer));
        sb.write_char('\n')?;
        let mut cursor = ast[L(n)].first;
        while let Some(c) = cursor {
            Self::to_string_tree_helper(ast, c, level + 1, sb, type_printer)?;
            cursor = ast[L(c)].next;
        }
        if let Some(shadow) = n.get_closure_unaware_shadow(ast) {
            Self::to_string_tree_helper(ast, shadow, level + 1, sb, type_printer)?;
        }
        Ok(())
    }
    // port: Node#appendJsonTree
    pub fn append_json_tree(self, ast: &Ast, appendable: &mut impl fmt::Write) -> fmt::Result {
        appendable.write_str(&crate::java_lang::charset::utf8_encoded_text(
            self.to_json_tree_utf16(ast).as_units(),
        ))
    }
    // port: Node#appendJsonTree
    pub fn to_json_tree_utf16(self, ast: &Ast) -> JsString {
        self.to_json_tree_with_types_utf16(ast, &mut no_registry_jstype_printer)
    }
    // port: Node#appendJsonTree
    /// `appendJsonTree` with a registry-aware JSType printer (see [`JSTypePrinter`]).
    pub fn to_json_tree_with_types_utf16(
        self,
        ast: &Ast,
        type_printer: &mut JSTypePrinter<'_>,
    ) -> JsString {
        let mut sb = crate::java_lang::string_builder::StringBuilder::new();
        Self::to_json_tree_helper(ast, self, &mut sb, type_printer).unwrap();
        sb.to_js_string()
    }
    // port: Node#toJsonTreeHelper
    fn to_json_tree_helper(
        ast: &Ast,
        n: NodeId,
        sb: &mut crate::java_lang::string_builder::StringBuilder,
        type_printer: &mut JSTypePrinter<'_>,
    ) -> fmt::Result {
        n.to_json(ast, sb, type_printer)
    }
    // port: Node#setStaticSourceFileFrom
    pub fn set_static_source_file_from(self, ast: &mut Ast, other: NodeId) {
        if ast[other].prop_list_head.is_some()
            && ast[self]
                .prop_list_head
                .as_ref()
                .is_none_or(|p| p.prop_type == Prop::SOURCE_FILE as u8 && p.next.is_none())
        {
            let mut tail = ast[other].prop_list_head.clone().unwrap();
            while let Some(next) = &tail.next {
                tail = next.clone();
            }
            if tail.prop_type == Prop::SOURCE_FILE as u8 {
                ast[self].prop_list_head = Some(tail);
                return;
            }
        }
        self.set_static_source_file(ast, other.get_static_source_file(ast));
    }
    // port: Node#setStaticSourceFile
    pub fn set_static_source_file(
        self,
        ast: &mut Ast,
        file: Option<Arc<dyn StaticSourceFile>>,
    ) -> Self {
        self.put_prop(
            ast,
            Prop::SOURCE_FILE,
            file.map(ObjectProp::StaticSourceFile),
        );
        self
    }
    // port: Node#setSourceFileForTesting
    pub fn set_source_file_for_testing(self, ast: &mut Ast, name: impl Into<String>) {
        self.put_prop(
            ast,
            Prop::SOURCE_FILE,
            Some(ObjectProp::StaticSourceFile(Arc::new(
                SimpleSourceFile::new(name, SourceKind::STRONG),
            ))),
        );
    }
    // port: Node#getSourceFileName
    pub fn get_source_file_name(self, ast: &Ast) -> Option<String> {
        self.get_static_source_file(ast)
            .map(|f| f.get_name().into())
    }
    // port: Node#getStaticSourceFile
    pub fn get_static_source_file(self, ast: &Ast) -> Option<Arc<dyn StaticSourceFile>> {
        self.get_prop(ast, Prop::SOURCE_FILE).map(|v| match v {
            ObjectProp::StaticSourceFile(f) => f,
            _ => panic!("ClassCastException"),
        })
    }
    // port: Node#setInputId
    pub fn set_input_id(self, ast: &mut Ast, input_id: Option<Arc<InputId>>) {
        self.put_prop(ast, Prop::INPUT_ID, input_id.map(ObjectProp::InputId));
    }
    // port: Node#getInputId
    pub fn get_input_id(self, ast: &Ast) -> Option<Arc<InputId>> {
        self.get_prop(ast, Prop::INPUT_ID).map(|v| match v {
            ObjectProp::InputId(i) => i,
            _ => panic!("ClassCastException"),
        })
    }
    // port: Node#getOriginalName
    pub fn get_original_name(self, ast: &Ast) -> Option<JsString> {
        ast[self].original_name.clone()
    }
    // port: Node#setOriginalName
    pub fn set_original_name(self, ast: &mut Ast, s: Option<JsString>) {
        ast[self].original_name = s.map(RhinoStringPool::add_or_get);
    }
    // port: Node#setOriginalNameFromStringPool
    pub fn set_original_name_from_string_pool(
        self,
        ast: &mut Ast,
        pool: &LazyInternedStringList,
        offset: i32,
    ) {
        ast[self].original_name = Some(pool.get(offset));
    }
    // port: Node#setOriginalNameFromName
    pub fn set_original_name_from_name(self, ast: &mut Ast, name: NodeId) {
        ast[self].original_name = Some(name.get_string(ast));
    }
    // port: Node#isIndexable
    pub fn is_indexable(self, ast: &Ast) -> bool {
        !self.get_boolean_prop(ast, Prop::NON_INDEXABLE)
    }
    // port: Node#makeNonIndexable
    pub fn make_non_indexable(self, ast: &mut Ast) {
        self.put_boolean_prop(ast, Prop::NON_INDEXABLE, true);
    }
    // port: Node#makeNonIndexableRecursive
    pub fn make_non_indexable_recursive(self, ast: &mut Ast) {
        self.make_non_indexable(ast);
        let mut child = ast[L(self)].first;
        while let Some(c) = child {
            c.make_non_indexable_recursive(ast);
            child = ast[L(c)].next;
        }
    }
    // port: Node#isFromExterns
    pub fn is_from_externs(self, ast: &Ast) -> bool {
        self.get_static_source_file(ast)
            .is_some_and(|f| f.is_extern())
    }
    // port: Node#isClosureUnawareCode
    pub fn is_closure_unaware_code(self, ast: &Ast) -> bool {
        self.get_static_source_file(ast)
            .is_some_and(|f| f.is_closure_unaware_code())
    }
    // port: Node#getLength
    pub fn get_length(self, ast: &Ast) -> i32 {
        ast[self].length
    }
    // port: Node#setLength
    pub fn set_length(self, ast: &mut Ast, length: i32) {
        ast[self].length = length;
    }
    // port: Node#getLineno
    pub fn get_lineno(self, ast: &Ast) -> i32 {
        if ast[self].lineno_charno == -1 {
            -1
        } else {
            (ast[self].lineno_charno as u32 >> Self::CHARNO_BITS) as i32
        }
    }
    // port: Node#getCharno
    pub fn get_charno(self, ast: &Ast) -> i32 {
        if ast[self].lineno_charno == -1 {
            -1
        } else {
            ast[self].lineno_charno & Self::MAX_COLUMN_NUMBER
        }
    }
    // port: Node#getLocation
    pub fn get_location(self, ast: &Ast) -> String {
        format!(
            "{}:{}:{}",
            self.get_source_file_name(ast)
                .unwrap_or_else(|| "null".into()),
            self.get_lineno(ast),
            self.get_charno(ast)
        )
    }
    // port: Node#getSourceOffset
    pub fn get_source_offset(self, ast: &Ast) -> i32 {
        let Some(file) = self.get_static_source_file(ast) else {
            return -1;
        };
        let lineno = self.get_lineno(ast);
        if lineno == -1 {
            return -1;
        }
        file.get_line_offset(lineno)
            .wrapping_add(self.get_charno(ast))
    }
    // port: Node#getSourcePosition
    pub fn get_source_position(self, ast: &Ast) -> i32 {
        ast[self].lineno_charno
    }
    // port: Node#setLinenoCharno
    pub fn set_lineno_charno(self, ast: &mut Ast, lineno: i32, mut charno: i32) -> Self {
        if lineno < 0 || charno < 0 {
            ast[self].lineno_charno = -1;
            return self;
        }
        if charno > Self::MAX_COLUMN_NUMBER {
            charno = Self::MAX_COLUMN_NUMBER;
        }
        ast[self].lineno_charno = lineno.wrapping_shl(Self::CHARNO_BITS) | charno;
        self
    }
    // port: Node#children
    pub fn children(self, ast: &Ast) -> SiblingNodeIterator<'_> {
        SiblingNodeIterable::new(ast[L(self)].first).iterator(ast)
    }
    pub fn children_cursor(self, ast: &Ast) -> SiblingNodeCursor {
        SiblingNodeCursor::new(ast[L(self)].first)
    }
    // port: Node#getPropListHeadForTesting
    pub fn get_prop_list_head_for_testing(self, ast: &Ast) -> Option<Arc<PropListItem>> {
        ast[self].prop_list_head.clone()
    }
    // port: Node#setPropListHead
    pub fn set_prop_list_head(self, ast: &mut Ast, head: Option<Arc<PropListItem>>) {
        ast[self].prop_list_head = head;
    }
    // port: Node#getParent
    pub fn get_parent(self, ast: &Ast) -> Option<NodeId> {
        ast[L(self)].parent
    }
    // port: Node#hasParent
    pub fn has_parent(self, ast: &Ast) -> bool {
        ast[L(self)].parent.is_some()
    }
    // port: Node#getGrandparent
    pub fn get_grandparent(self, ast: &Ast) -> Option<NodeId> {
        ast[L(self)].parent.and_then(|p| ast[L(p)].parent)
    }
    // port: Node#getAncestor
    pub fn get_ancestor(self, ast: &Ast, mut level: i32) -> Option<NodeId> {
        check_argument!(level >= 0);
        let mut node = Some(self);
        while node.is_some() && level > 0 {
            level -= 1;
            node = ast[L(node.unwrap())].parent;
        }
        node
    }
    // port: Node#isDescendantOf
    pub fn is_descendant_of(self, ast: &Ast, node: NodeId) -> bool {
        let mut n = Some(self);
        while let Some(c) = n {
            if c == node {
                return true;
            }
            n = ast[L(c)].parent;
        }
        false
    }
    // port: Node#isOnlyChildOf
    pub fn is_only_child_of(self, ast: &Ast, possible_parent: Option<NodeId>) -> bool {
        possible_parent == self.get_parent(ast)
            && self.get_previous(ast).is_none()
            && self.get_next(ast).is_none()
    }
    // port: Node#isFirstChildOf
    pub fn is_first_child_of(self, ast: &Ast, possible_parent: Option<NodeId>) -> bool {
        possible_parent == self.get_parent(ast) && self.get_previous(ast).is_none()
    }
    // port: Node#isSecondChildOf
    pub fn is_second_child_of(self, ast: &Ast, possible_parent: Option<NodeId>) -> bool {
        self.get_previous(ast)
            .is_some_and(|p| p.is_first_child_of(ast, possible_parent))
    }
    // port: Node#getAncestors
    pub fn get_ancestors(self, ast: &Ast) -> AncestorIterable<'_> {
        AncestorIterable::new(ast, self.get_parent(ast))
    }
    pub fn ancestors(self, ast: &Ast) -> AncestorIterable<'_> {
        self.get_ancestors(ast)
    }
    pub fn ancestors_cursor(self, ast: &Ast) -> AncestorCursor {
        AncestorCursor {
            cur: self.get_parent(ast),
        }
    }
    // port: Node#hasOneChild
    pub fn has_one_child(self, ast: &Ast) -> bool {
        ast[L(self)].first.is_some_and(|f| ast[L(f)].next.is_none())
    }
    // port: Node#hasTwoChildren
    pub fn has_two_children(self, ast: &Ast) -> bool {
        ast[L(self)]
            .first
            .is_some_and(|f| ast[L(f)].next.is_some() && ast[L(f)].next == self.get_last_child(ast))
    }
    // port: Node#hasZeroOrOneChild
    pub fn has_zero_or_one_child(self, ast: &Ast) -> bool {
        ast[L(self)].first == self.get_last_child(ast)
    }
    // port: Node#hasMoreThanOneChild
    pub fn has_more_than_one_child(self, ast: &Ast) -> bool {
        ast[L(self)].first.is_some_and(|f| ast[L(f)].next.is_some())
    }
    // port: Node#hasXChildren
    pub fn has_x_children(self, ast: &Ast, x: i32) -> bool {
        let mut c = 0;
        let mut n = ast[L(self)].first;
        while let Some(cur) = n {
            if c > x {
                break;
            }
            c += 1;
            n = ast[L(cur)].next;
        }
        c == x
    }
    // port: Node#getChildCount
    pub fn get_child_count(self, ast: &Ast) -> i32 {
        let mut c = 0;
        let mut n = ast[L(self)].first;
        while let Some(cur) = n {
            c += 1;
            n = ast[L(cur)].next;
        }
        c
    }
    // port: Node#hasChild
    pub fn has_child(self, ast: &Ast, child: NodeId) -> bool {
        let mut n = ast[L(self)].first;
        while let Some(cur) = n {
            if child == cur {
                return true;
            }
            n = ast[L(cur)].next;
        }
        false
    }
    // port: Node#isEquivalentToShallow
    pub fn is_equivalent_to_shallow(self, ast: &Ast, node: NodeId) -> bool {
        self.is_equivalent_to_with_options(
            ast,
            node,
            RecursionMode::SHALLOW,
            TypeComparison::IGNORE,
            JsDocComparison::IGNORE,
            SideEffectComparison::IGNORE,
        )
    }
    // port: Node#isEquivalentWithSideEffectsTo
    pub fn is_equivalent_with_side_effects_to(self, ast: &Ast, node: NodeId) -> bool {
        self.is_equivalent_to_with_options(
            ast,
            node,
            RecursionMode::DEEP,
            TypeComparison::IGNORE,
            JsDocComparison::IGNORE,
            SideEffectComparison::COMPARE,
        )
    }
    // port: Node#isEquivalentWithSideEffectsToShallow
    pub fn is_equivalent_with_side_effects_to_shallow(self, ast: &Ast, node: NodeId) -> bool {
        self.is_equivalent_to_with_options(
            ast,
            node,
            RecursionMode::SHALLOW,
            TypeComparison::IGNORE,
            JsDocComparison::IGNORE,
            SideEffectComparison::COMPARE,
        )
    }
    // port: Node#isEquivalentToTyped
    pub fn is_equivalent_to_typed(self, ast: &Ast, node: NodeId) -> bool {
        self.is_equivalent_to_with_options(
            ast,
            node,
            RecursionMode::DEEP,
            TypeComparison::COMPARE,
            JsDocComparison::COMPARE,
            SideEffectComparison::IGNORE,
        )
    }
    // port: Node#isEquivalentTo(Node)
    pub fn is_equivalent_to(self, ast: &Ast, node: NodeId) -> bool {
        self.is_equivalent_to_with_options(
            ast,
            node,
            RecursionMode::DEEP,
            TypeComparison::IGNORE,
            JsDocComparison::IGNORE,
            SideEffectComparison::IGNORE,
        )
    }
    // port: Node#getSecondToLastPropListItem
    fn get_second_to_last_prop_list_item(self, ast: &Ast) -> Option<Arc<PropListItem>> {
        let mut p = ast[self].prop_list_head.clone()?;
        p.next.as_ref()?;
        while p.next.as_ref().unwrap().next.is_some() {
            p = p.next.clone().unwrap();
        }
        Some(p)
    }
    // port: Node#getLastPropListItem
    fn get_last_prop_list_item(self, ast: &Ast) -> Option<Arc<PropListItem>> {
        let mut p = ast[self].prop_list_head.clone()?;
        while let Some(next) = &p.next {
            p = next.clone();
        }
        Some(p)
    }
    // port: Node#validatePropListTailOrdering
    fn validate_prop_list_tail_ordering(
        self,
        ast: &Ast,
        should_node_be_closure_unaware: bool,
    ) -> bool {
        let mut prev = None;
        let Some(mut curr) = ast[self].prop_list_head.clone() else {
            return false;
        };
        while let Some(next) = &curr.next {
            prev = Some(curr.clone());
            curr = next.clone();
        }
        if curr.prop_type != Prop::SOURCE_FILE as u8 {
            return false;
        }
        if should_node_be_closure_unaware
            && prev.is_none_or(|p| p.prop_type != Prop::IS_IN_CLOSURE_UNAWARE_SUBTREE as u8)
        {
            return false;
        }
        true
    }
    // port: Node#validateMemorySensitivePropertyGuarantees
    pub fn validate_memory_sensitive_property_guarantees(
        ast: &Ast,
        node1: NodeId,
        should_node1_be_closure_unaware: bool,
        node2: NodeId,
        should_node2_be_closure_unaware: bool,
    ) -> bool {
        if !node1.validate_prop_list_tail_ordering(ast, should_node1_be_closure_unaware) {
            return false;
        }
        if !node2.validate_prop_list_tail_ordering(ast, should_node2_be_closure_unaware) {
            return false;
        }
        if !arc_option_ptr_eq(
            &node1.get_last_prop_list_item(ast),
            &node2.get_last_prop_list_item(ast),
        ) {
            return false;
        }
        if should_node1_be_closure_unaware
            && should_node2_be_closure_unaware
            && !arc_option_ptr_eq(
                &node1.get_second_to_last_prop_list_item(ast),
                &node2.get_second_to_last_prop_list_item(ast),
            )
        {
            return false;
        }
        true
    }
    // port: Node#isEquivalentTo(Node, boolean, boolean, boolean)
    pub fn is_equivalent_to_with_flags(
        self,
        ast: &Ast,
        node: NodeId,
        compare_type: bool,
        recurse: bool,
        js_doc: bool,
    ) -> bool {
        self.is_equivalent_to_with_side_effect_flags(
            ast,
            node,
            compare_type,
            recurse,
            js_doc,
            false,
        )
    }
    // port: Node#isEquivalentTo(Node, boolean, boolean, boolean, boolean)
    pub fn is_equivalent_to_with_side_effect_flags(
        self,
        ast: &Ast,
        node: NodeId,
        compare_type: bool,
        recurse: bool,
        js_doc: bool,
        side_effect: bool,
    ) -> bool {
        self.is_equivalent_to_with_options(
            ast,
            node,
            if recurse {
                RecursionMode::DEEP
            } else {
                RecursionMode::SHALLOW
            },
            if compare_type {
                TypeComparison::COMPARE
            } else {
                TypeComparison::IGNORE
            },
            if js_doc {
                JsDocComparison::COMPARE
            } else {
                JsDocComparison::IGNORE
            },
            if side_effect {
                SideEffectComparison::COMPARE
            } else {
                SideEffectComparison::IGNORE
            },
        )
    }
    // port: Node#isEquivalentTo(Node, RecursionMode, TypeComparison, JsDocComparison, SideEffectComparison)
    // port: NumberNode#isEquivalentTo
    // port: BigIntNode#isEquivalentTo
    // port: StringNode#isEquivalentTo
    // port: TemplateLiteralSubstringNode#isEquivalentTo
    pub fn is_equivalent_to_with_options(
        self,
        ast: &Ast,
        node: NodeId,
        recursion: RecursionMode,
        type_config: TypeComparison,
        js_doc_config: JsDocComparison,
        side_effect_config: SideEffectComparison,
    ) -> bool {
        self.is_equivalent_to_across_with_options(
            ast,
            ast,
            node,
            recursion,
            type_config,
            js_doc_config,
            side_effect_config,
        )
    }
    // port: Node#isEquivalentTo(Node) (nodes owned by separate compilers)
    pub fn is_equivalent_to_across(self, ast: &Ast, ast_b: &Ast, node: NodeId) -> bool {
        self.is_equivalent_to_across_with_options(
            ast,
            ast_b,
            node,
            RecursionMode::DEEP,
            TypeComparison::IGNORE,
            JsDocComparison::IGNORE,
            SideEffectComparison::IGNORE,
        )
    }
    // port: Node#isEquivalentTo(Node, RecursionMode, TypeComparison, JsDocComparison, SideEffectComparison)
    #[expect(
        clippy::too_many_arguments,
        reason = "Java equivalence flags plus the two owning arenas"
    )]
    pub fn is_equivalent_to_across_with_options(
        self,
        ast: &Ast,
        ast_b: &Ast,
        node: NodeId,
        recursion: RecursionMode,
        type_config: TypeComparison,
        js_doc_config: JsDocComparison,
        side_effect_config: SideEffectComparison,
    ) -> bool {
        if ast[L(self)].token != ast_b[L(node)].token
            || self.get_child_count(ast) != node.get_child_count(ast_b)
            || std::mem::discriminant(&ast[self].kind) != std::mem::discriminant(&ast_b[node].kind)
        {
            return false;
        }
        if type_config == TypeComparison::COMPARE
            && ast[self].jstype_or_color != ast_b[node].jstype_or_color
        {
            return false;
        }
        if js_doc_config == JsDocComparison::COMPARE
            && !JSDocInfo::are_equivalent_across(
                ast,
                ast_b,
                self.get_jsdoc_info(ast).as_deref(),
                node.get_jsdoc_info(ast_b).as_deref(),
            )
        {
            return false;
        }
        let this_dte = self.get_declared_type_expression(ast);
        let that_dte = node.get_declared_type_expression(ast_b);
        if (this_dte.is_none() && that_dte.is_none())
            || (std::ptr::eq(ast, ast_b) && this_dte == that_dte)
        {
        } else if this_dte.is_none() || that_dte.is_none() {
            return false;
        } else if !this_dte.unwrap().is_equivalent_to_across_with_options(
            ast,
            ast_b,
            that_dte.unwrap(),
            recursion,
            type_config,
            js_doc_config,
            side_effect_config,
        ) {
            return false;
        }
        let mut prop_set = [false; Prop::VALUES.len()];
        for (arena, n) in [(ast, self), (ast_b, node)] {
            let mut item = arena[n].prop_list_head.clone();
            while let Some(p) = item {
                prop_set[p.prop_type as usize] = true;
                item = p.next.clone();
            }
        }
        for prop in Prop::VALUES {
            if !prop_set[prop as usize] {
                continue;
            }
            let same = match prop {
                Prop::ARROW_FN => self.is_arrow_function(ast) == node.is_arrow_function(ast_b),
                Prop::ASYNC_FN => self.is_async_function(ast) == node.is_async_function(ast_b),
                Prop::GENERATOR_FN => {
                    self.is_generator_function(ast) == node.is_generator_function(ast_b)
                }
                Prop::START_OF_OPT_CHAIN => {
                    self.is_optional_chain_start(ast) == node.is_optional_chain_start(ast_b)
                }
                Prop::STATIC_MEMBER => self.is_static_member(ast) == node.is_static_member(ast_b),
                Prop::YIELD_ALL => self.is_yield_all(ast) == node.is_yield_all(ast_b),
                Prop::EXPORT_DEFAULT | Prop::EXPORT_ALL_FROM | Prop::INCRDECR | Prop::QUOTED => {
                    self.get_int_prop(ast, prop) == node.get_int_prop(ast_b, prop)
                }
                Prop::FREE_CALL
                | Prop::DIRECT_EVAL
                | Prop::COMPUTED_PROP_METHOD
                | Prop::COMPUTED_PROP_GETTER
                | Prop::COMPUTED_PROP_SETTER => {
                    self.get_boolean_prop(ast, prop) == node.get_boolean_prop(ast_b, prop)
                }
                _ => true,
            };
            if !same {
                return false;
            }
        }
        if side_effect_config == SideEffectComparison::COMPARE {
            if self.get_side_effect_flags(ast) != node.get_side_effect_flags(ast_b) {
                return false;
            }
            if self.is_unused_parameter(ast) != node.is_unused_parameter(ast_b) {
                return false;
            }
        }
        let this_shadow = self.get_closure_unaware_shadow(ast);
        let node_shadow = node.get_closure_unaware_shadow(ast_b);
        if this_shadow.is_some() != node_shadow.is_some() {
            return false;
        }
        if recursion.recurse_shadow_ast() {
            if let (Some(a), Some(b)) = (this_shadow, node_shadow) {
                if !a.is_equivalent_to_across_with_options(
                    ast,
                    ast_b,
                    b,
                    recursion,
                    type_config,
                    js_doc_config,
                    side_effect_config,
                ) {
                    return false;
                }
            }
        }
        if recursion.recurse_children() {
            let mut n = ast[L(self)].first;
            let mut n2 = ast_b[L(node)].first;
            while let Some(cur) = n {
                let cur2 = n2.unwrap();
                if !cur.is_equivalent_to_across_with_options(
                    ast,
                    ast_b,
                    cur2,
                    recursion,
                    type_config,
                    js_doc_config,
                    side_effect_config,
                ) {
                    return false;
                }
                n = ast[L(cur)].next;
                n2 = ast_b[L(cur2)].next;
            }
        }
        match (&ast[self].kind, &ast_b[node].kind) {
            (NodeKind::Number { number: a }, NodeKind::Number { number: b }) => a == b,
            (NodeKind::BigInt { bigint: a }, NodeKind::BigInt { bigint: b }) => a == b,
            (NodeKind::String { str: a }, NodeKind::String { str: b }) => {
                RhinoStringPool::unchecked_equals(a, b)
            }
            (NodeKind::TemplateLiteralSubstring(x), NodeKind::TemplateLiteralSubstring(y)) => {
                let (a, c, b, d) = (&x.raw, &x.cooked, &y.raw, &y.cooked);
                RhinoStringPool::unchecked_equals(a, b)
                    && match (c, d) {
                        (Some(c), Some(d)) => RhinoStringPool::unchecked_equals(c, d),
                        (None, None) => true,
                        _ => false,
                    }
            }
            _ => true,
        }
    }
    // port: Node#getQualifiedName
    pub fn get_qualified_name(self, ast: &Ast) -> Option<JsString> {
        match ast[L(self)].token {
            Token::NAME => {
                let name = self.get_string_ref(ast);
                if name.is_empty() {
                    None
                } else {
                    Some(name.clone())
                }
            }
            Token::GETPROP => self
                .get_qualified_name_for_get_prop(ast, 0)
                .map(JsString::from_units),
            Token::THIS => Some("this".into()),
            Token::SUPER => Some("super".into()),
            _ => None,
        }
    }
    // port: Node#getQualifiedNameObject
    pub fn get_qualified_name_object(self, ast: &Ast) -> Option<QualifiedName> {
        if self.is_qualified_name(ast) {
            Some(QualifiedName::from_node(self))
        } else {
            None
        }
    }
    // port: Node#getQualifiedNameForGetProp
    fn get_qualified_name_for_get_prop(self, ast: &Ast, mut reserve: i32) -> Option<Vec<u16>> {
        let prop_name = self.get_string_ref(ast);
        reserve = reserve
            .wrapping_add(1)
            .wrapping_add(prop_name.length() as i32);
        let first = ast[L(self)].first.unwrap();
        let mut builder;
        if first.is_get_prop(ast) {
            builder = first.get_qualified_name_for_get_prop(ast, reserve)?;
        } else {
            // getQualifiedName on the left side, read in place (no JsString copy).
            let left: &[u16] = match ast[L(first)].token {
                Token::NAME => {
                    let name = first.get_string_ref(ast);
                    if name.is_empty() {
                        return None;
                    }
                    name.as_units()
                }
                Token::THIS => &[116, 104, 105, 115],
                Token::SUPER => &[115, 117, 112, 101, 114],
                _ => return None,
            };
            builder = Vec::with_capacity(
                usize::try_from((left.len() as i32).wrapping_add(reserve)).unwrap(),
            );
            builder.extend_from_slice(left);
        }
        builder.push(b'.' as u16);
        builder.extend_from_slice(prop_name.as_units());
        Some(builder)
    }
    // port: Node#getOriginalQualifiedName
    pub fn get_original_qualified_name(self, ast: &Ast) -> Option<JsString> {
        if ast[L(self)].token == Token::NAME {
            let name = self
                .get_original_name(ast)
                .unwrap_or_else(|| self.get_string(ast));
            if name.is_empty() { None } else { Some(name) }
        } else if ast[L(self)].token == Token::GETPROP {
            let left = ast[L(self)]
                .first
                .unwrap()
                .get_original_qualified_name(ast)?;
            let right = self
                .get_original_name(ast)
                .unwrap_or_else(|| self.get_string(ast));
            Some(left.concat(&".".into()).concat(&right))
        } else if ast[L(self)].token == Token::THIS {
            Some("this".into())
        } else if ast[L(self)].token == Token::SUPER {
            Some("super".into())
        } else {
            None
        }
    }
    // port: Node#isQualifiedName
    pub fn is_qualified_name(self, ast: &Ast) -> bool {
        match self.get_token(ast) {
            Token::NAME => !self.get_string_ref(ast).is_empty(),
            Token::THIS | Token::SUPER => true,
            Token::GETPROP => ast[L(self)].first.unwrap().is_qualified_name(ast),
            _ => false,
        }
    }
    // port: Node#matchesName(String)
    pub fn matches_name(self, ast: &Ast, name: impl JsStrLike) -> bool {
        if ast[L(self)].token != Token::NAME {
            return false;
        }
        let internal_string = self.get_string_ref(ast);
        !internal_string.is_empty() && name.with_units(|name| name == internal_string.as_units())
    }
    // port: Node#matchesName(Node)
    pub fn matches_name_node(self, ast: &Ast, n: NodeId) -> bool {
        if ast[L(self)].token != Token::NAME || ast[L(n)].token != Token::NAME {
            return false;
        }
        let internal_string = self.get_string_ref(ast);
        !internal_string.is_empty()
            && RhinoStringPool::unchecked_equals(internal_string, n.get_string_ref(ast))
    }
    // port: Node#matchesQualifiedName(String)
    pub fn matches_qualified_name(self, ast: &Ast, name: impl JsStrLike) -> bool {
        name.with_units(|name| self.matches_qualified_name_to_index(ast, name, name.len()))
    }
    // port: Node#matchesQualifiedName(String, int)
    fn matches_qualified_name_to_index(self, ast: &Ast, qname: &[u16], end_index: usize) -> bool {
        let start = qname[..end_index]
            .iter()
            .rposition(|c| *c == b'.' as u16)
            .map_or(0, |i| i + 1);
        match self.get_token(ast) {
            Token::NAME | Token::IMPORT_STAR => {
                let name = self.get_string_ref(ast);
                start == 0
                    && !name.is_empty()
                    && name.length() == end_index
                    && qname.starts_with(name.as_units())
            }
            Token::THIS => start == 0 && end_index == 4 && qname.starts_with(&[116, 104, 105, 115]),
            Token::SUPER => {
                start == 0 && end_index == 5 && qname.starts_with(&[115, 117, 112, 101, 114])
            }
            Token::GETPROP => {
                let prop = self.get_string_ref(ast);
                start > 1
                    && prop.length() == end_index - start
                    && prop.as_units() == &qname[start..end_index]
                    && ast[L(self)].first.unwrap().matches_qualified_name_to_index(
                        ast,
                        qname,
                        start - 1,
                    )
            }
            _ => false,
        }
    }
    // port: Node#matchesQualifiedName(Node)
    pub fn matches_qualified_name_node(self, ast: &Ast, n: NodeId) -> bool {
        if ast[L(n)].token != ast[L(self)].token {
            return false;
        }
        match ast[L(self)].token {
            Token::NAME => self.matches_name_node(ast, n),
            Token::THIS | Token::SUPER => true,
            Token::GETPROP => {
                RhinoStringPool::unchecked_equals(self.get_string_ref(ast), n.get_string_ref(ast))
                    && ast[L(self)]
                        .first
                        .unwrap()
                        .matches_qualified_name_node(ast, ast[L(n)].first.unwrap())
            }
            _ => false,
        }
    }
    // port: Node#isUnscopedQualifiedName
    pub fn is_unscoped_qualified_name(self, ast: &Ast) -> bool {
        match self.get_token(ast) {
            Token::NAME => !self.get_string_ref(ast).is_empty(),
            Token::GETPROP => ast[L(self)].first.unwrap().is_unscoped_qualified_name(ast),
            _ => false,
        }
    }
    // port: Node#isValidAssignmentTarget
    pub fn is_valid_assignment_target(self, ast: &Ast) -> bool {
        matches!(
            self.get_token(ast),
            Token::NAME
                | Token::GETPROP
                | Token::GETELEM
                | Token::ARRAY_PATTERN
                | Token::OBJECT_PATTERN
        )
    }
    // port: Node#clone
    #[allow(clippy::should_implement_trait)]
    pub fn clone(self, _ast: &Ast) -> ! {
        panic!("Did you mean cloneNode?")
    }
    // port: Node#cloneNode()
    pub fn clone_node(self, ast: &mut Ast) -> NodeId {
        self.clone_node_with_type_exprs(ast, false)
    }
    // port: Node#cloneNode(boolean)
    // port: NumberNode#cloneNode
    // port: BigIntNode#cloneNode
    // port: StringNode#cloneNode
    // port: TemplateLiteralSubstringNode#cloneNode
    pub fn clone_node_with_type_exprs(self, ast: &mut Ast, clone_type_exprs: bool) -> NodeId {
        let token = ast[L(self)].token;
        let clone = match ast[self].kind.clone() {
            NodeKind::Node => ast.new_node(token),
            NodeKind::Number { number } => ast.new_number(number),
            NodeKind::BigInt { bigint } => ast.new_big_int(bigint),
            NodeKind::String { str } => {
                let n = ast.new_string_node(token);
                ast[n].kind = NodeKind::String { str };
                n
            }
            NodeKind::TemplateLiteralSubstring(strings) => {
                ast.new_template_lit_string(strings.cooked, strings.raw)
            }
        };
        Self::copy_base_node_fields(ast, self, clone, clone_type_exprs);
        clone
    }
    // port: Node#copyBaseNodeFields
    fn copy_base_node_fields(ast: &mut Ast, source: NodeId, dest: NodeId, clone_type_exprs: bool) {
        ast[dest].lineno_charno = ast[source].lineno_charno;
        ast[dest].length = ast[source].length;
        ast[dest].jstype_or_color = ast[source].jstype_or_color.clone();
        ast[dest].original_name = ast[source].original_name.clone();
        ast[dest].prop_list_head = ast[source].prop_list_head.clone();
        if clone_type_exprs {
            if let Some(info) = source.get_jsdoc_info(ast) {
                let cloned = info.clone_with_type_nodes(ast, true);
                dest.set_jsdoc_info(ast, Some(Arc::new(cloned)));
            }
        }
    }
    // port: Node#cloneTree()
    pub fn clone_tree(self, ast: &mut Ast) -> NodeId {
        self.clone_tree_with_type_exprs(ast, false)
    }
    // port: Node#cloneTree(boolean)
    pub fn clone_tree_with_type_exprs(self, ast: &mut Ast, clone_type_exprs: bool) -> NodeId {
        let result = self.clone_node_with_type_exprs(ast, clone_type_exprs);
        let mut first_child = None;
        let mut last_child = None;
        if self.has_children(ast) {
            let mut n2 = self.get_first_child(ast);
            while let Some(n) = n2 {
                let clone = n.clone_tree_with_type_exprs(ast, clone_type_exprs);
                ast[L(clone)].parent = Some(result);
                if first_child.is_none() {
                    first_child = Some(clone);
                    last_child = first_child;
                } else {
                    let last = last_child.unwrap();
                    ast[L(last)].next = Some(clone);
                    ast[L(clone)].previous = Some(last);
                    last_child = Some(clone);
                }
                n2 = ast[L(n)].next;
            }
            ast[L(first_child.unwrap())].previous = last_child;
            ast[L(last_child.unwrap())].next = None;
            ast[L(result)].first = first_child;
        }
        if let Some(shadow) = self.get_closure_unaware_shadow(ast) {
            let cloned = shadow.clone_tree_with_type_exprs(ast, clone_type_exprs);
            result.set_closure_unaware_shadow(ast, Some(cloned));
        }
        result
    }
    // port: Node#srcref
    pub fn srcref(self, ast: &mut Ast, other: NodeId) -> Self {
        self.set_static_source_file_from(ast, other);
        ast[self].original_name = ast[other].original_name.clone();
        ast[self].lineno_charno = ast[other].lineno_charno;
        ast[self].length = ast[other].length;
        self
    }
    // port: Node#srcrefTree
    pub fn srcref_tree(self, ast: &mut Ast, other: NodeId) -> Self {
        self.srcref(ast, other);
        let mut child = ast[L(self)].first;
        while let Some(c) = child {
            c.srcref_tree(ast, other);
            child = ast[L(c)].next;
        }
        self
    }
    // port: Node#srcrefIfMissing
    pub fn srcref_if_missing(self, ast: &mut Ast, other: NodeId) -> Self {
        if self.get_static_source_file(ast).is_none() {
            self.set_static_source_file_from(ast, other);
            ast[self].lineno_charno = ast[other].lineno_charno;
            ast[self].length = ast[other].length;
        }
        if ast[self].original_name.is_none() {
            ast[self].original_name = ast[other].original_name.clone();
        }
        self
    }
    // port: Node#srcrefTreeIfMissing
    pub fn srcref_tree_if_missing(self, ast: &mut Ast, other: NodeId) -> Self {
        self.srcref_if_missing(ast, other);
        let mut child = ast[L(self)].first;
        while let Some(c) = child {
            c.srcref_tree_if_missing(ast, other);
            child = ast[L(c)].next;
        }
        self
    }
    // port: Node#getJSType
    pub fn get_jstype(self, ast: &Ast) -> Option<TypeId> {
        match &ast[self].jstype_or_color {
            Some(JSTypeOrColor::JSType(t)) => Some(*t),
            _ => None,
        }
    }
    // port: Node#getJSTypeRequired
    pub fn get_jstype_required(self, ast: &Ast) -> TypeId {
        check_not_null!(
            self.get_jstype(ast),
            "no jstypeOrColor: %s",
            self.to_string(ast)
        )
    }
    // port: Node#setJSType
    pub fn set_jstype(self, ast: &mut Ast, x: Option<TypeId>) -> Self {
        check_state!(
            ast[self].jstype_or_color.is_none()
                || matches!(ast[self].jstype_or_color, Some(JSTypeOrColor::JSType(_))),
            "%s",
            self.to_string(ast)
        );
        ast[self].jstype_or_color = x.map(JSTypeOrColor::JSType);
        self
    }
    // port: Node#getColor
    pub fn get_color(self, ast: &Ast) -> Option<Color> {
        match &ast[self].jstype_or_color {
            Some(JSTypeOrColor::Color(c)) => Some(c.clone()),
            _ => None,
        }
    }
    // port: Node#setColor
    pub fn set_color(self, ast: &mut Ast, x: Option<Color>) -> Self {
        check_state!(
            ast[self].jstype_or_color.is_none()
                || matches!(ast[self].jstype_or_color, Some(JSTypeOrColor::Color(_))),
            "%s",
            self.to_string(ast)
        );
        ast[self].jstype_or_color = x.map(JSTypeOrColor::Color);
        self
    }
    // port: Node#copyTypeFrom
    pub fn copy_type_from(self, ast: &mut Ast, other: NodeId) -> Self {
        ast[self].jstype_or_color = ast[other].jstype_or_color.clone();
        self
    }
    // port: Node#getJSDocInfo
    pub fn get_jsdoc_info(self, ast: &Ast) -> Option<Arc<JSDocInfo>> {
        self.get_jsdoc_info_ref(ast).cloned()
    }
    // port: Node#getJSDocInfo
    /// Rust-only: `get_jsdoc_info` by reference (no reference-count traffic).
    pub fn get_jsdoc_info_ref(self, ast: &Ast) -> Option<&Arc<JSDocInfo>> {
        self.lookup_property_ref(ast, Prop::JSDOC_INFO)
            .map(|item| match &item.value {
                PropValue::Object(ObjectProp::JSDocInfo(info)) => info,
                PropValue::Object(_) => panic!("ClassCastException"),
                PropValue::Int(_) => panic!("UnsupportedOperationException"),
            })
    }
    // port: Node#setJSDocInfo
    pub fn set_jsdoc_info(self, ast: &mut Ast, info: Option<Arc<JSDocInfo>>) -> Self {
        self.put_prop(ast, Prop::JSDOC_INFO, info.map(ObjectProp::JSDocInfo));
        self
    }
    // port: Node#setChangeTime
    pub fn set_change_time(self, ast: &mut Ast, time: i32) {
        self.put_int_prop(ast, Prop::CHANGE_TIME, time);
    }
    // port: Node#getChangeTime
    pub fn get_change_time(self, ast: &Ast) -> i32 {
        self.get_int_prop(ast, Prop::CHANGE_TIME)
    }
    // port: Node#setDeleted
    pub fn set_deleted(self, ast: &mut Ast, deleted: bool) {
        self.put_boolean_prop(ast, Prop::DELETED, deleted);
    }
    // port: Node#isDeleted
    pub fn is_deleted(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::DELETED)
    }
    // port: Node#setTypedefTypeProp
    pub fn set_typedef_type_prop(self, ast: &mut Ast, t: Option<TypeId>) {
        self.put_prop(ast, Prop::TYPEDEF_TYPE, t.map(ObjectProp::JSType));
    }
    // port: Node#getTypedefTypeProp
    pub fn get_typedef_type_prop(self, ast: &Ast) -> Option<TypeId> {
        self.get_prop(ast, Prop::TYPEDEF_TYPE).map(|v| match v {
            ObjectProp::JSType(t) => t,
            _ => panic!("ClassCastException"),
        })
    }
    // port: Node#setUnusedParameter
    pub fn set_unused_parameter(self, ast: &mut Ast, unused: bool) {
        self.put_boolean_prop(ast, Prop::IS_UNUSED_PARAMETER, unused);
    }
    // port: Node#isUnusedParameter
    pub fn is_unused_parameter(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::IS_UNUSED_PARAMETER)
    }
    // port: Node#setShorthandProperty
    pub fn set_shorthand_property(self, ast: &mut Ast, shorthand: bool) {
        self.put_boolean_prop(ast, Prop::IS_SHORTHAND_PROPERTY, shorthand);
    }
    // port: Node#isShorthandProperty
    pub fn is_shorthand_property(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::IS_SHORTHAND_PROPERTY)
    }
    // port: Node#isOptionalEs6Typed
    pub fn is_optional_es6_typed(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::OPT_ES6_TYPED)
    }
    // port: Node#setIsSyntheticBlock
    pub fn set_is_synthetic_block(self, ast: &mut Ast, val: bool) {
        check_state!(ast[L(self)].token == Token::BLOCK);
        self.put_boolean_prop(ast, Prop::SYNTHETIC, val);
    }
    // port: Node#isSyntheticBlock
    pub fn is_synthetic_block(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::SYNTHETIC)
    }
    // port: Node#setIsSynthesizedUnfulfilledNameDeclaration
    pub fn set_is_synthesized_unfulfilled_name_declaration(self, ast: &mut Ast, val: bool) {
        check_state!(
            ast[L(self)].token == Token::VAR
                && self.has_one_child(ast)
                && self.get_first_child(ast).unwrap().is_name(ast),
            "Expected all synthetic unfulfilled declarations to be `var <name>`, found %s",
            self.to_string(ast)
        );
        self.put_boolean_prop(ast, Prop::SYNTHESIZED_UNFULFILLED_NAME_DECLARATION, val);
    }
    // port: Node#isSynthesizedUnfulfilledNameDeclaration
    pub fn is_synthesized_unfulfilled_name_declaration(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::SYNTHESIZED_UNFULFILLED_NAME_DECLARATION)
    }
    // port: Node#setUseStrict
    pub fn set_use_strict(self, ast: &mut Ast, x: bool) {
        self.put_boolean_prop(ast, Prop::USE_STRICT, x)
    }
    // port: Node#isUseStrict
    pub fn is_use_strict(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::USE_STRICT)
    }
    // port: Node#setIsAddedBlock
    pub fn set_is_added_block(self, ast: &mut Ast, val: bool) {
        self.put_boolean_prop(ast, Prop::ADDED_BLOCK, val)
    }
    // port: Node#isAddedBlock
    pub fn is_added_block(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::ADDED_BLOCK)
    }
    // port: Node#setStaticMember
    pub fn set_static_member(self, ast: &mut Ast, is_static: bool) {
        self.put_boolean_prop(ast, Prop::STATIC_MEMBER, is_static)
    }
    // port: Node#isStaticMember
    pub fn is_static_member(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::STATIC_MEMBER)
    }
    // port: Node#setIsGeneratorFunction
    pub fn set_is_generator_function(self, ast: &mut Ast, is_generator: bool) {
        self.put_boolean_prop(ast, Prop::GENERATOR_FN, is_generator)
    }
    // port: Node#isGeneratorFunction
    pub fn is_generator_function(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::GENERATOR_FN)
    }
    // port: Node#setGeneratorMarker
    pub fn set_generator_marker(self, ast: &mut Ast, is_generator_marker: bool) {
        self.put_boolean_prop(ast, Prop::IS_GENERATOR_MARKER, is_generator_marker)
    }
    // port: Node#isGeneratorMarker
    pub fn is_generator_marker(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::IS_GENERATOR_MARKER)
    }
    // port: Node#setGeneratorSafe
    pub fn set_generator_safe(self, ast: &mut Ast, is_generator_safe: bool) {
        self.put_boolean_prop(ast, Prop::IS_GENERATOR_SAFE, is_generator_safe)
    }
    // port: Node#isGeneratorSafe
    pub fn is_generator_safe(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::IS_GENERATOR_SAFE)
    }
    // port: Node#setIsOptionalChainStart
    pub fn set_is_optional_chain_start(self, ast: &mut Ast, is_optional_chain_start: bool) {
        check_state!(
            !is_optional_chain_start
                || self.is_opt_chain_get_elem(ast)
                || self.is_opt_chain_get_prop(ast)
                || self.is_opt_chain_call(ast),
            "cannot make a non-optional node the start of an optional chain."
        );
        self.put_boolean_prop(ast, Prop::START_OF_OPT_CHAIN, is_optional_chain_start);
    }
    // port: Node#isOptionalChainStart
    pub fn is_optional_chain_start(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::START_OF_OPT_CHAIN)
    }
    // port: Node#setIsArrowFunction
    pub fn set_is_arrow_function(self, ast: &mut Ast, is_arrow: bool) {
        check_state!(self.is_function(ast));
        self.put_boolean_prop(ast, Prop::ARROW_FN, is_arrow)
    }
    // port: Node#isArrowFunction
    pub fn is_arrow_function(self, ast: &Ast) -> bool {
        self.is_function(ast) && self.get_boolean_prop(ast, Prop::ARROW_FN)
    }
    // port: Node#setIsAsyncFunction
    pub fn set_is_async_function(self, ast: &mut Ast, is_async: bool) {
        check_state!(self.is_function(ast));
        self.put_boolean_prop(ast, Prop::ASYNC_FN, is_async)
    }
    // port: Node#isAsyncFunction
    pub fn is_async_function(self, ast: &Ast) -> bool {
        self.is_function(ast) && self.get_boolean_prop(ast, Prop::ASYNC_FN)
    }
    // port: Node#isAsyncGeneratorFunction
    pub fn is_async_generator_function(self, ast: &Ast) -> bool {
        self.is_async_function(ast) && self.is_generator_function(ast)
    }
    // port: Node#setYieldAll
    pub fn set_yield_all(self, ast: &mut Ast, is_generator: bool) {
        self.put_boolean_prop(ast, Prop::YIELD_ALL, is_generator)
    }
    // port: Node#isYieldAll
    pub fn is_yield_all(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::YIELD_ALL)
    }
    // port: Node#setTrailingComma
    pub fn set_trailing_comma(self, ast: &mut Ast, has_trailing_comma: bool) {
        self.put_boolean_prop(ast, Prop::TRAILING_COMMA, has_trailing_comma)
    }
    // port: Node#hasTrailingComma
    pub fn has_trailing_comma(self, ast: &Ast) -> bool {
        self.get_boolean_prop(ast, Prop::TRAILING_COMMA)
    }
    // port: Node#setSideEffectFlags(int)
    pub fn set_side_effect_flags(self, ast: &mut Ast, flags: i32) {
        check_state!(
            self.is_call(ast)
                || self.is_opt_chain_call(ast)
                || self.is_new(ast)
                || self.is_tagged_template_lit(ast),
            "Side-effect flags can only be set on invocation nodes; got %s",
            self.to_string(ast)
        );
        self.put_int_prop(
            ast,
            Prop::SIDE_EFFECT_FLAGS,
            !flags & SideEffectFlags::USED_BITS_MASK,
        );
    }
    // port: Node#setSideEffectFlags(SideEffectFlags)
    pub fn set_side_effect_flags_from_flags(self, ast: &mut Ast, flags: &SideEffectFlags) {
        self.set_side_effect_flags(ast, flags.value_of());
    }
    // port: Node#getSideEffectFlags
    pub fn get_side_effect_flags(self, ast: &Ast) -> i32 {
        !self.get_int_prop(ast, Prop::SIDE_EFFECT_FLAGS) & SideEffectFlags::USED_BITS_MASK
    }
    // port: Node#isOnlyModifiesThisCall
    pub fn is_only_modifies_this_call(self, ast: &Ast) -> bool {
        let side_effects_besides_mutates_this =
            self.get_side_effect_flags(ast) & !SideEffectFlags::MUTATES_THIS;
        side_effects_besides_mutates_this == SideEffectFlags::NO_SIDE_EFFECTS
    }
    // port: Node#isOnlyModifiesArgumentsCall
    pub fn is_only_modifies_arguments_call(self, ast: &Ast) -> bool {
        let side_effects_besides_mutates_arguments =
            self.get_side_effect_flags(ast) & !SideEffectFlags::MUTATES_ARGUMENTS;
        side_effects_besides_mutates_arguments == SideEffectFlags::NO_SIDE_EFFECTS
    }
    // port: Node#isNoSideEffectsCall
    pub fn is_no_side_effects_call(self, ast: &Ast) -> bool {
        self.get_side_effect_flags(ast) == SideEffectFlags::NO_SIDE_EFFECTS
    }
    // port: Node#mayMutateArguments
    pub fn may_mutate_arguments(self, ast: &Ast) -> bool {
        Self::all_bits_set(
            self.get_side_effect_flags(ast),
            SideEffectFlags::MUTATES_ARGUMENTS,
        )
    }
    // port: Node#mayMutateGlobalStateOrThrow
    pub fn may_mutate_global_state_or_throw(self, ast: &Ast) -> bool {
        Self::any_bit_set(
            self.get_side_effect_flags(ast),
            SideEffectFlags::MUTATES_GLOBAL_STATE | SideEffectFlags::THROWS,
        )
    }
    // port: Node#allBitsSet
    fn all_bits_set(value: i32, mask: i32) -> bool {
        value & mask == mask
    }
    // port: Node#anyBitSet
    fn any_bit_set(value: i32, mask: i32) -> bool {
        value & mask != 0
    }
    // port: Node#getConstantVarFlags
    fn get_constant_var_flags(self, ast: &Ast) -> i32 {
        self.get_int_prop(ast, Prop::CONSTANT_VAR_FLAGS)
    }
    // port: Node#setConstantVarFlag
    fn set_constant_var_flag(self, ast: &mut Ast, flag: i32, value: bool) {
        let mut flags = self.get_constant_var_flags(ast);
        if value {
            flags |= flag;
        } else {
            flags &= !flag;
        }
        self.put_int_prop(ast, Prop::CONSTANT_VAR_FLAGS, flags);
    }
    // port: Node#isDeclaredConstantVar
    pub fn is_declared_constant_var(self, ast: &Ast) -> bool {
        check_state!(
            self.is_name(ast) || self.is_import_star(ast),
            "Should only be called on name or import * nodes. Found %s",
            self.to_string(ast)
        );
        Self::any_bit_set(self.get_constant_var_flags(ast), ConstantVarFlags::DECLARED)
    }
    // port: Node#setDeclaredConstantVar
    pub fn set_declared_constant_var(self, ast: &mut Ast, value: bool) {
        check_state!(
            self.is_name(ast) || self.is_import_star(ast),
            "Should only be called on name or import * nodes. Found %s",
            self.to_string(ast)
        );
        self.set_constant_var_flag(ast, ConstantVarFlags::DECLARED, value);
    }
    // port: Node#isInferredConstantVar
    pub fn is_inferred_constant_var(self, ast: &Ast) -> bool {
        check_state!(
            self.is_name(ast) || self.is_import_star(ast),
            "Should only be called on name or import * nodes. Found %s",
            self.to_string(ast)
        );
        Self::any_bit_set(self.get_constant_var_flags(ast), ConstantVarFlags::INFERRED)
    }
    // port: Node#setInferredConstantVar
    pub fn set_inferred_constant_var(self, ast: &mut Ast, value: bool) {
        check_state!(
            self.is_name(ast) || self.is_import_star(ast),
            "Should only be called on name or import * nodes. Found %s",
            self.to_string(ast)
        );
        self.set_constant_var_flag(ast, ConstantVarFlags::INFERRED, value);
    }
    // port: Node#isQuotedStringKey
    pub fn is_quoted_string_key(self, ast: &Ast) -> bool {
        matches!(ast[self].kind, NodeKind::String { .. })
            && self.get_boolean_prop(ast, Prop::QUOTED)
    }
    // port: Node#setQuotedStringKey
    pub fn set_quoted_string_key(self, ast: &mut Ast) {
        check_state!(
            matches!(ast[self].kind, NodeKind::String { .. }),
            "%s",
            self.to_string(ast)
        );
        self.put_boolean_prop(ast, Prop::QUOTED, true);
    }
    // port: Node#isPrivateIdentifier
    pub fn is_private_identifier(self, ast: &Ast) -> bool {
        matches!(ast[self].kind, NodeKind::String { .. })
            && self.get_boolean_prop(ast, Prop::PRIVATE_IDENTIFIER)
    }
    // port: Node#setPrivateIdentifier
    pub fn set_private_identifier(self, ast: &mut Ast) {
        check_state!(
            matches!(ast[self].kind, NodeKind::String { .. }),
            "%s",
            self.to_string(ast)
        );
        check_state!(self.get_string_ref(ast).starts_with("#"));
        self.put_boolean_prop(ast, Prop::PRIVATE_IDENTIFIER, true);
    }
    // port: Node#isAdd
    pub fn is_add(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::ADD
    }
    // port: Node#isSub
    pub fn is_sub(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::SUB
    }
    // port: Node#isAnd
    pub fn is_and(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::AND
    }
    // port: Node#isAssignAnd
    pub fn is_assign_and(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::ASSIGN_AND
    }
    // port: Node#isArrayLit
    pub fn is_array_lit(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::ARRAYLIT
    }
    // port: Node#isArrayPattern
    pub fn is_array_pattern(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::ARRAY_PATTERN
    }
    // port: Node#isAssign
    pub fn is_assign(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::ASSIGN
    }
    // port: Node#isAssignAdd
    pub fn is_assign_add(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::ASSIGN_ADD
    }
    // port: Node#isNormalBlock
    pub fn is_normal_block(self, ast: &Ast) -> bool {
        self.is_block(ast)
    }
    // port: Node#isBlock
    pub fn is_block(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::BLOCK
    }
    // port: Node#isRoot
    pub fn is_root(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::ROOT
    }
    // port: Node#isAwait
    pub fn is_await(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::AWAIT
    }
    // port: Node#isBigInt
    pub fn is_big_int(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::BIGINT
    }
    // port: Node#isBitNot
    pub fn is_bit_not(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::BITNOT
    }
    // port: Node#isBreak
    pub fn is_break(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::BREAK
    }
    // port: Node#isCall
    pub fn is_call(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::CALL
    }
    // port: Node#isCase
    pub fn is_case(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::CASE
    }
    // port: Node#isCast
    pub fn is_cast(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::CAST
    }
    // port: Node#isCatch
    pub fn is_catch(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::CATCH
    }
    // port: Node#isClass
    pub fn is_class(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::CLASS
    }
    // port: Node#isClassMembers
    pub fn is_class_members(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::CLASS_MEMBERS
    }
    // port: Node#isComma
    pub fn is_comma(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::COMMA
    }
    // port: Node#isComputedProp
    pub fn is_computed_prop(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::COMPUTED_PROP
    }
    // port: Node#isContinue
    pub fn is_continue(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::CONTINUE
    }
    // port: Node#isConst
    pub fn is_const(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::CONST
    }
    // port: Node#isDebugger
    pub fn is_debugger(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::DEBUGGER
    }
    // port: Node#isDec
    pub fn is_dec(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::DEC
    }
    // port: Node#isDefaultCase
    pub fn is_default_case(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::DEFAULT_CASE
    }
    // port: Node#isDefaultValue
    pub fn is_default_value(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::DEFAULT_VALUE
    }
    // port: Node#isDelProp
    pub fn is_del_prop(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::DELPROP
    }
    // port: Node#isDestructuringLhs
    pub fn is_destructuring_lhs(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::DESTRUCTURING_LHS
    }
    // port: Node#isDestructuringPattern
    pub fn is_destructuring_pattern(self, ast: &Ast) -> bool {
        self.is_object_pattern(ast) || self.is_array_pattern(ast)
    }
    // port: Node#isDo
    pub fn is_do(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::DO
    }
    // port: Node#isEmpty
    pub fn is_empty(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::EMPTY
    }
    // port: Node#isExponent
    pub fn is_exponent(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::EXPONENT
    }
    // port: Node#isAssignExponent
    pub fn is_assign_exponent(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::ASSIGN_EXPONENT
    }
    // port: Node#isExport
    pub fn is_export(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::EXPORT
    }
    // port: Node#isExportSpec
    pub fn is_export_spec(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::EXPORT_SPEC
    }
    // port: Node#isExportSpecs
    pub fn is_export_specs(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::EXPORT_SPECS
    }
    // port: Node#isExprResult
    pub fn is_expr_result(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::EXPR_RESULT
    }
    // port: Node#isFalse
    pub fn is_false(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::FALSE
    }
    // port: Node#isVanillaFor
    pub fn is_vanilla_for(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::FOR
    }
    // port: Node#isForIn
    pub fn is_for_in(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::FOR_IN
    }
    // port: Node#isForOf
    pub fn is_for_of(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::FOR_OF
    }
    // port: Node#isForAwaitOf
    pub fn is_for_await_of(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::FOR_AWAIT_OF
    }
    // port: Node#isFunction
    pub fn is_function(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::FUNCTION
    }
    // port: Node#isGetterDef
    pub fn is_getter_def(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::GETTER_DEF
    }
    // port: Node#isGetElem
    pub fn is_get_elem(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::GETELEM
    }
    // port: Node#isGetProp
    pub fn is_get_prop(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::GETPROP
    }
    // port: Node#isHook
    pub fn is_hook(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::HOOK
    }
    // port: Node#isIf
    pub fn is_if(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::IF
    }
    // port: Node#isImport
    pub fn is_import(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::IMPORT
    }
    // port: Node#isImportMeta
    pub fn is_import_meta(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::IMPORT_META
    }
    // port: Node#isImportStar
    pub fn is_import_star(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::IMPORT_STAR
    }
    // port: Node#isImportSpec
    pub fn is_import_spec(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::IMPORT_SPEC
    }
    // port: Node#isImportSpecs
    pub fn is_import_specs(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::IMPORT_SPECS
    }
    // port: Node#isIn
    pub fn is_in(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::IN
    }
    // port: Node#isInc
    pub fn is_inc(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::INC
    }
    // port: Node#isInstanceOf
    pub fn is_instance_of(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::INSTANCEOF
    }
    // port: Node#isInterface
    pub fn is_interface(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::INTERFACE
    }
    // port: Node#isInterfaceMembers
    pub fn is_interface_members(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::INTERFACE_MEMBERS
    }
    // port: Node#isRecordType
    pub fn is_record_type(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::RECORD_TYPE
    }
    // port: Node#isCallSignature
    pub fn is_call_signature(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::CALL_SIGNATURE
    }
    // port: Node#isIndexSignature
    pub fn is_index_signature(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::INDEX_SIGNATURE
    }
    // port: Node#isLabel
    pub fn is_label(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::LABEL
    }
    // port: Node#isLabelName
    pub fn is_label_name(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::LABEL_NAME
    }
    // port: Node#isLet
    pub fn is_let(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::LET
    }
    // port: Node#isMemberFunctionDef
    pub fn is_member_function_def(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::MEMBER_FUNCTION_DEF
    }
    // port: Node#isMemberVariableDef
    pub fn is_member_variable_def(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::MEMBER_VARIABLE_DEF
    }
    // port: Node#isMemberFieldDef
    pub fn is_member_field_def(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::MEMBER_FIELD_DEF
    }
    // port: Node#isComputedFieldDef
    pub fn is_computed_field_def(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::COMPUTED_FIELD_DEF
    }
    // port: Node#isModuleBody
    pub fn is_module_body(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::MODULE_BODY
    }
    // port: Node#isName
    pub fn is_name(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::NAME
    }
    // port: Node#isNE
    pub fn is_ne(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::NE
    }
    // port: Node#isSHNE
    pub fn is_shne(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::SHNE
    }
    // port: Node#isEQ
    pub fn is_eq(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::EQ
    }
    // port: Node#isSHEQ
    pub fn is_sheq(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::SHEQ
    }
    // port: Node#isNeg
    pub fn is_neg(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::NEG
    }
    // port: Node#isNew
    pub fn is_new(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::NEW
    }
    // port: Node#isNot
    pub fn is_not(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::NOT
    }
    // port: Node#isNull
    pub fn is_null(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::NULL
    }
    // port: Node#isNullishCoalesce
    pub fn is_nullish_coalesce(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::COALESCE
    }
    // port: Node#isAssignNullishCoalesce
    pub fn is_assign_nullish_coalesce(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::ASSIGN_COALESCE
    }
    // port: Node#isNumber
    pub fn is_number(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::NUMBER
    }
    // port: Node#isObjectLit
    pub fn is_object_lit(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::OBJECTLIT
    }
    // port: Node#isObjectPattern
    pub fn is_object_pattern(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::OBJECT_PATTERN
    }
    // port: Node#isOptChainCall
    pub fn is_opt_chain_call(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::OPTCHAIN_CALL
    }
    // port: Node#isOptChainGetElem
    pub fn is_opt_chain_get_elem(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::OPTCHAIN_GETELEM
    }
    // port: Node#isOptChainGetProp
    pub fn is_opt_chain_get_prop(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::OPTCHAIN_GETPROP
    }
    // port: Node#isOr
    pub fn is_or(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::OR
    }
    // port: Node#isAssignOr
    pub fn is_assign_or(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::ASSIGN_OR
    }
    // port: Node#isParamList
    pub fn is_param_list(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::PARAM_LIST
    }
    // port: Node#isPos
    pub fn is_pos(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::POS
    }
    // port: Node#isRegExp
    pub fn is_reg_exp(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::REGEXP
    }
    // port: Node#isRest
    pub fn is_rest(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::ITER_REST || ast[L(self)].token == Token::OBJECT_REST
    }
    // port: Node#isObjectRest
    pub fn is_object_rest(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::OBJECT_REST
    }
    // port: Node#isReturn
    pub fn is_return(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::RETURN
    }
    // port: Node#isScript
    pub fn is_script(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::SCRIPT
    }
    // port: Node#isSetterDef
    pub fn is_setter_def(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::SETTER_DEF
    }
    // port: Node#isSpread
    pub fn is_spread(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::ITER_SPREAD || ast[L(self)].token == Token::OBJECT_SPREAD
    }
    // port: Node#isString
    pub fn is_string(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::STRINGLIT
    }
    // port: Node#isStringKey
    pub fn is_string_key(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::STRING_KEY
    }
    // port: Node#isStringLit
    pub fn is_string_lit(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::STRINGLIT
    }
    // port: Node#isSuper
    pub fn is_super(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::SUPER
    }
    // port: Node#isSwitch
    pub fn is_switch(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::SWITCH
    }
    // port: Node#isSwitchBody
    pub fn is_switch_body(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::SWITCH_BODY
    }
    // port: Node#isTaggedTemplateLit
    pub fn is_tagged_template_lit(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::TAGGED_TEMPLATELIT
    }
    // port: Node#isTemplateLit
    pub fn is_template_lit(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::TEMPLATELIT
    }
    // port: Node#isTemplateLitString
    pub fn is_template_lit_string(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::TEMPLATELIT_STRING
    }
    // port: Node#isTemplateLitSub
    pub fn is_template_lit_sub(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::TEMPLATELIT_SUB
    }
    // port: Node#isThis
    pub fn is_this(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::THIS
    }
    // port: Node#isThrow
    pub fn is_throw(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::THROW
    }
    // port: Node#isTrue
    pub fn is_true(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::TRUE
    }
    // port: Node#isTry
    pub fn is_try(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::TRY
    }
    // port: Node#isTypeOf
    pub fn is_type_of(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::TYPEOF
    }
    // port: Node#isVar
    pub fn is_var(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::VAR
    }
    // port: Node#isVoid
    pub fn is_void(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::VOID
    }
    // port: Node#isWhile
    pub fn is_while(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::WHILE
    }
    // port: Node#isWith
    pub fn is_with(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::WITH
    }
    // port: Node#isYield
    pub fn is_yield(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::YIELD
    }
    // port: Node#isDeclare
    pub fn is_declare(self, ast: &Ast) -> bool {
        ast[L(self)].token == Token::DECLARE
    }
}
fn arc_option_ptr_eq<T: ?Sized>(a: &Option<Arc<T>>, b: &Option<Arc<T>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        _ => false,
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecursionMode {
    SHALLOW,
    DEEP_NO_SHADOW,
    DEEP,
}
impl RecursionMode {
    // port: RecursionMode#recurseShadowAst
    pub fn recurse_shadow_ast(self) -> bool {
        self == Self::DEEP
    }
    // port: RecursionMode#recurseChildren
    pub fn recurse_children(self) -> bool {
        self == Self::DEEP || self == Self::DEEP_NO_SHADOW
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeComparison {
    IGNORE,
    COMPARE,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsDocComparison {
    IGNORE,
    COMPARE,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SideEffectComparison {
    IGNORE,
    COMPARE,
}
#[derive(Debug)]
pub struct SiblingNodeIterable {
    start: Option<NodeId>,
}
impl SiblingNodeIterable {
    // port: SiblingNodeIterable#SiblingNodeIterable
    fn new(start: Option<NodeId>) -> Self {
        Self { start }
    }
    // port: SiblingNodeIterable#iterator
    pub fn iterator<'a>(&self, ast: &'a Ast) -> SiblingNodeIterator<'a> {
        SiblingNodeIterator::new(ast, self.start)
    }
}
pub struct SiblingNodeIterator<'a> {
    ast: &'a Ast,
    current: Option<NodeId>,
}
impl<'a> SiblingNodeIterator<'a> {
    // port: SiblingNodeIterator#SiblingNodeIterator
    fn new(ast: &'a Ast, start: Option<NodeId>) -> Self {
        Self {
            ast,
            current: start,
        }
    }
    // port: SiblingNodeIterator#hasNext
    pub fn has_next(&self) -> bool {
        self.current.is_some()
    }
    // port: SiblingNodeIterator#remove
    pub fn remove(&mut self) -> ! {
        panic!("UnsupportedOperationException")
    }
}
impl Iterator for SiblingNodeIterator<'_> {
    type Item = NodeId;
    // port: SiblingNodeIterator#next
    fn next(&mut self) -> Option<NodeId> {
        let n = self.current?;
        self.current = n.get_next(self.ast);
        Some(n)
    }
}
pub struct SiblingNodeCursor {
    current: Option<NodeId>,
}
impl SiblingNodeCursor {
    // port: SiblingNodeIterator#SiblingNodeIterator
    fn new(start: Option<NodeId>) -> Self {
        Self { current: start }
    }
    // port: SiblingNodeIterator#hasNext
    pub fn has_next(&self) -> bool {
        self.current.is_some()
    }
    // port: SiblingNodeIterator#next
    pub fn next(&mut self, ast: &Ast) -> Option<NodeId> {
        let n = self.current?;
        self.current = n.get_next(ast);
        Some(n)
    }
    // port: SiblingNodeIterator#remove
    pub fn remove(&mut self) -> ! {
        panic!("UnsupportedOperationException")
    }
}
pub struct AncestorIterable<'a> {
    ast: &'a Ast,
    cur: Option<NodeId>,
}
impl<'a> AncestorIterable<'a> {
    // port: AncestorIterable#AncestorIterable
    fn new(ast: &'a Ast, cur: Option<NodeId>) -> Self {
        Self { ast, cur }
    }
    // port: AncestorIterable#iterator
    pub fn iterator(&mut self) -> &mut Self {
        self
    }
    // port: AncestorIterable#hasNext
    pub fn has_next(&self) -> bool {
        self.cur.is_some()
    }
    // port: AncestorIterable#remove
    pub fn remove(&mut self) -> ! {
        panic!("UnsupportedOperationException")
    }
}
impl Iterator for AncestorIterable<'_> {
    type Item = NodeId;
    // port: AncestorIterable#next
    fn next(&mut self) -> Option<NodeId> {
        let n = self.cur?;
        self.cur = n.get_parent(self.ast);
        Some(n)
    }
}
pub struct AncestorCursor {
    cur: Option<NodeId>,
}
impl AncestorCursor {
    // port: AncestorIterable#hasNext
    pub fn has_next(&self) -> bool {
        self.cur.is_some()
    }
    // port: AncestorIterable#next
    pub fn next(&mut self, ast: &Ast) -> Option<NodeId> {
        let n = self.cur?;
        self.cur = n.get_parent(ast);
        Some(n)
    }
    // port: AncestorIterable#remove
    pub fn remove(&mut self) -> ! {
        panic!("UnsupportedOperationException")
    }
}
#[derive(Debug, Clone, Copy)]
pub struct SideEffectFlags {
    value: i32,
}
impl SideEffectFlags {
    pub const MUTATES_GLOBAL_STATE: i32 = 1;
    pub const MUTATES_THIS: i32 = 2;
    pub const MUTATES_ARGUMENTS: i32 = 4;
    pub const THROWS: i32 = 8;
    pub const USED_BITS_MASK: i32 = (1 << 4) - 1;
    pub const NO_SIDE_EFFECTS: i32 = 0;
    pub const ALL_SIDE_EFFECTS: i32 =
        Self::MUTATES_GLOBAL_STATE | Self::MUTATES_THIS | Self::MUTATES_ARGUMENTS | Self::THROWS;
    // port: SideEffectFlags#SideEffectFlags()
    pub fn new() -> Self {
        Self {
            value: Self::ALL_SIDE_EFFECTS,
        }
    }
    // port: SideEffectFlags#SideEffectFlags(int)
    pub fn with_value(value: i32) -> Self {
        Self { value }
    }
    // port: SideEffectFlags#valueOf
    pub fn value_of(&self) -> i32 {
        self.value
    }
    // port: SideEffectFlags#setAllFlags
    pub fn set_all_flags(&mut self) -> &mut Self {
        self.value = Self::ALL_SIDE_EFFECTS;
        self
    }
    // port: SideEffectFlags#clearAllFlags
    pub fn clear_all_flags(&mut self) -> &mut Self {
        self.value = Self::NO_SIDE_EFFECTS;
        self
    }
    // port: SideEffectFlags#setMutatesGlobalState
    pub fn set_mutates_global_state(&mut self) -> &mut Self {
        self.value |= Self::MUTATES_GLOBAL_STATE | Self::MUTATES_ARGUMENTS | Self::MUTATES_THIS;
        self
    }
    // port: SideEffectFlags#setThrows
    pub fn set_throws(&mut self) -> &mut Self {
        self.value |= Self::THROWS;
        self
    }
    // port: SideEffectFlags#setMutatesThis
    pub fn set_mutates_this(&mut self) -> &mut Self {
        self.value |= Self::MUTATES_THIS;
        self
    }
    // port: SideEffectFlags#setMutatesArguments
    pub fn set_mutates_arguments(&mut self) -> &mut Self {
        self.value |= Self::MUTATES_ARGUMENTS;
        self
    }
}
impl Default for SideEffectFlags {
    fn default() -> Self {
        Self::new()
    }
}
impl fmt::Display for SideEffectFlags {
    // port: SideEffectFlags#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Side effects: ")?;
        if self.value & Self::MUTATES_THIS != 0 {
            f.write_str("this ")?;
        }
        if self.value & Self::MUTATES_GLOBAL_STATE != 0 {
            f.write_str("global ")?;
        }
        if self.value & Self::THROWS != 0 {
            f.write_str("throw ")?;
        }
        if self.value & Self::MUTATES_ARGUMENTS != 0 {
            f.write_str("args ")?;
        }
        Ok(())
    }
}
struct ConstantVarFlags;
impl ConstantVarFlags {
    const DECLARED: i32 = 1;
    const INFERRED: i32 = 2;
    // port: ConstantVarFlags#ConstantVarFlags
    fn new() -> Self {
        Self
    }
}
impl Default for ConstantVarFlags {
    fn default() -> Self {
        Self::new()
    }
}
impl crate::java_lang::JavaHashCode for NodeId {
    // port: Object#hashCode
    // Node's Java identity hash varies per JVM. Use deterministic arena identity.
    fn hash_code(&self) -> i32 {
        self.0.get() as i32
    }
}
