/*
 * Copyright 2016 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CheckSuper.java.

#![allow(clippy::collapsible_match)] // Retain the Java switches and their nested branches.
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
    node_util::NodeUtil,
};
use closure_rhino::{check_argument, check_not_null, check_state, node::NodeId, token::Token};

// port: CheckSuper#MISSING_CALL_TO_SUPER
pub static MISSING_CALL_TO_SUPER: DiagnosticType = DiagnosticType::error(
    "JSC_MISSING_CALL_TO_SUPER",
    "constructor is missing a call to super()",
);

// port: CheckSuper#THIS_BEFORE_SUPER
pub static THIS_BEFORE_SUPER: DiagnosticType = DiagnosticType::error(
    "JSC_THIS_BEFORE_SUPER",
    "cannot access this before calling super()",
);

// port: CheckSuper#SUPER_ACCESS_BEFORE_SUPER_CONSTRUCTOR
pub static SUPER_ACCESS_BEFORE_SUPER_CONSTRUCTOR: DiagnosticType = DiagnosticType::error(
    "JSC_SUPER_ACCESS_BEFORE_SUPER_CONSTRUCTOR",
    "cannot access super properties before calling super()",
);

// port: CheckSuper#INVALID_SUPER_CALL
pub static INVALID_SUPER_CALL: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_SUPER_CALL",
    "super() not allowed except in the constructor of a subclass",
);

// The JS spec allows calls to `super()` in an arrow function within a constructor,
// as long as `super()` executes exactly once before any references to `this` or `super.prop`.
// However, doing that makes it very hard to statically determine whether `super()` is being
// called when it should be.
// There's really no good reason to call `super()` in an arrow function.
// It indicates that your code is overly complicated and you should refactor, so we will not
// allow it.
// port: CheckSuper#SUPER_CALL_IN_ARROW
pub static SUPER_CALL_IN_ARROW: DiagnosticType = DiagnosticType::error(
    "JSC_SUPER_CALL_IN_ARROW",
    "closure-compiler does not allow calls to `super()` in arrow functions",
);

// port: CheckSuper#INVALID_SUPER_USAGE
pub static INVALID_SUPER_USAGE: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_SUPER_USAGE",
    "''super'' may only be used in a call or property access",
);

// port: CheckSuper#INVALID_SUPER_ACCESS
pub static INVALID_SUPER_ACCESS: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_SUPER_ACCESS",
    "''super'' may only be accessed within a method",
);

// port: CheckSuper#INVALID_SUPER_CALL_WITH_SUGGESTION
pub static INVALID_SUPER_CALL_WITH_SUGGESTION: DiagnosticType = DiagnosticType::error(
    "JSC_INVALID_SUPER_CALL_WITH_SUGGESTION",
    "super() not allowed here. Did you mean super.{0}?",
);

/// Check for errors related to the `super` keyword.
pub struct CheckSuper {
    // Java context objects are identified by reference (`newContext != currentContext`, and a
    // ConstructorArrowContext refers to its ConstructorContext); here they live in this arena and
    // are identified by index.
    contexts: Vec<Context>,
    context_stack: Vec<ContextId>,
}

type ContextId = usize;

impl CheckSuper {
    // port: CheckSuper#CheckSuper
    pub fn new() -> Self {
        Self {
            contexts: Vec::new(),
            context_stack: Vec::new(),
        }
    }

    fn new_context(&mut self, context: Context) -> ContextId {
        self.contexts.push(context);
        self.contexts.len() - 1
    }

    // port: CheckSuper#getContextForFunctionNode
    fn get_context_for_function_node(
        &mut self,
        t: &NodeTraversal<'_>,
        current_context: ContextId,
        fn_: NodeId,
    ) -> ContextId {
        if NodeUtil::is_method_declaration(t, fn_) {
            if NodeUtil::is_es6_constructor(t, fn_) {
                let context = Context::new_constructor_context(t, fn_);
                self.new_context(context)
            } else {
                let context = Context::new_method_context(t, fn_);
                self.new_context(context)
            }
        } else {
            // Arrow function context varies depending on the actual context, but
            // super is never allowed in normal functions.
            if fn_.is_arrow_function(t) {
                self.get_context_for_arrow_function_node(current_context, fn_)
            } else {
                self.new_context(Context::new(fn_, ContextKind::SuperNotAllowed))
            }
        }
    }

    // port: CheckSuper#isSuperConstructorCall
    fn is_super_constructor_call(t: &NodeTraversal<'_>, super_node: NodeId) -> bool {
        check_state!(super_node.is_super(t), "%s", super_node.to_string(t));
        let parent = super_node.get_parent(t).unwrap();
        parent.is_call(t) && super_node.is_first_child_of(t, Some(parent))
    }

    // port: CheckSuper#isSuperPropertyAccess
    fn is_super_property_access(t: &NodeTraversal<'_>, super_node: NodeId) -> bool {
        check_state!(super_node.is_super(t), "%s", super_node.to_string(t));
        let parent = super_node.get_parent(t).unwrap();
        NodeUtil::is_normal_get(t, parent) && super_node.is_first_child_of(t, Some(parent))
    }

    // port: CheckSuper#createMemberFieldDefContext
    fn create_member_field_def_context(&mut self, field_node: NodeId) -> ContextId {
        self.new_context(Context::new(
            field_node,
            ContextKind::SuperPropertyAccessAllowed,
        ))
    }

    // port: CheckSuper#createStaticBlockContext
    fn create_static_block_context(&mut self, static_block: NodeId) -> ContextId {
        self.new_context(Context::new(
            static_block,
            ContextKind::SuperPropertyAccessAllowed,
        ))
    }

    /// The correct context for an arrow function depends on the enclosing context.
    // port: CheckSuper.Context#getContextForArrowFunctionNode
    fn get_context_for_arrow_function_node(
        &mut self,
        context: ContextId,
        arrow_fn: NodeId,
    ) -> ContextId {
        match self.contexts[context].kind {
            // port: CheckSuper.SuperPropertyAccessAllowedContext#getContextForArrowFunctionNode
            ContextKind::SuperPropertyAccessAllowed => context,
            // port: CheckSuper.SuperNotAllowedContext#getContextForArrowFunctionNode
            // Since super isn't allowed in this context, it isn't allowed in any of its arrow
            // functions either.
            ContextKind::SuperNotAllowed => context,
            // port: CheckSuper.MethodContext#getContextForArrowFunctionNode
            // In a method an arrow function keeps the same context as the method, but a
            // non-arrow function creates a new non-method context.
            ContextKind::Method { .. } => context,
            // port: CheckSuper.ConstructorContext#getContextForArrowFunctionNode
            // Unlike normal methods, we need to create a separate context for arrow functions in
            // constructors. We need to distinguish between return statements and `super()` calls
            // that appear directly in the constructor and those that appear in arrow functions.
            ContextKind::Constructor(_) => {
                let context = Context::new_constructor_arrow_context(context, arrow_fn);
                self.new_context(context)
            }
            // port: CheckSuper.ConstructorArrowContext#getContextForArrowFunctionNode
            // Arrow functions within arrow functions share the same context behavior.
            ContextKind::ConstructorArrow { .. } => context,
        }
    }

    /// Handle a super constructor call.
    // port: CheckSuper.Context#visitSuperConstructorCall
    fn visit_super_constructor_call(
        &mut self,
        context: ContextId,
        t: &mut NodeTraversal<'_>,
        super_node: NodeId,
    ) {
        match &mut self.contexts[context].kind {
            // port: CheckSuper.SuperPropertyAccessAllowedContext#visitSuperConstructorCall
            ContextKind::SuperPropertyAccessAllowed => {
                // Super constructor calls are only allowed in constructor() methods.
                t.report(super_node, &INVALID_SUPER_CALL, &[]);
            }
            // port: CheckSuper.SuperNotAllowedContext#visitSuperConstructorCall
            ContextKind::SuperNotAllowed => {
                // We're not within a constructor method.
                t.report(super_node, &INVALID_SUPER_CALL, &[]);
            }
            // port: CheckSuper.MethodContext#visitSuperConstructorCall
            ContextKind::Method { is_class_method } => {
                if *is_class_method {
                    // super() - not allowed in non-constructor methods.
                    let prop_name = self.contexts[context].get_property_name(t);
                    if let Some(prop_name) = prop_name {
                        // Maybe the user was confused by other languages where super() invokes
                        // the same method in the parent class?
                        t.report(
                            super_node,
                            &INVALID_SUPER_CALL_WITH_SUGGESTION,
                            &[prop_name.as_str()],
                        );
                    } else {
                        t.report(super_node, &INVALID_SUPER_CALL, &[]);
                    }
                } else {
                    // object literal methods cannot contain super() calls
                    t.report(super_node, &INVALID_SUPER_CALL, &[]);
                }
            }
            // port: CheckSuper.ConstructorContext#visitSuperConstructorCall
            ContextKind::Constructor(constructor) => {
                if constructor.first_super_call.is_none() {
                    constructor.first_super_call = Some(super_node);
                }
            }
            // port: CheckSuper.ConstructorArrowContext#visitSuperConstructorCall
            ContextKind::ConstructorArrow {
                constructor_context,
            } => {
                let constructor_context = *constructor_context;
                t.report(super_node, &SUPER_CALL_IN_ARROW, &[]);
                // Tell the constructor context about this `super()` call in order to avoid
                // confusing and likely redundant error messages such as "missing super call" or
                // "`this` accessed before super()".
                self.visit_super_constructor_call(constructor_context, t, super_node);
            }
        }
    }

    /// Handle a super property reference. (`super.prop` or `super[expr]`)
    // port: CheckSuper.Context#visitSuperPropertyAccess
    fn visit_super_property_access(
        &mut self,
        context: ContextId,
        t: &mut NodeTraversal<'_>,
        super_node: NodeId,
    ) {
        match &mut self.contexts[context].kind {
            // port: CheckSuper.SuperPropertyAccessAllowedContext#visitSuperPropertyAccess
            ContextKind::SuperPropertyAccessAllowed => {
                // Super property should be allowed on a public field and inside static blocks.
            }
            // port: CheckSuper.SuperNotAllowedContext#visitSuperPropertyAccess
            ContextKind::SuperNotAllowed => {
                // We're not within a method.
                t.report(super_node, &INVALID_SUPER_ACCESS, &[]);
            }
            // port: CheckSuper.MethodContext#visitSuperPropertyAccess
            ContextKind::Method { .. } => {
                // super property access in a method is perfectly valid, so do not report an
                // error.
            }
            // port: CheckSuper.ConstructorContext#visitSuperPropertyAccess
            ContextKind::Constructor(constructor) => {
                if constructor.first_super_call.is_none() {
                    constructor.super_property_accessed_before_super_call = Some(super_node);
                }
            }
            // port: CheckSuper.ConstructorArrowContext#visitSuperPropertyAccess
            ContextKind::ConstructorArrow {
                constructor_context,
            } => {
                // We will pretend the arrow function is immediately called, so it's as if the
                // super property reference appeared directly in the constructor.
                // The result is that you get an error for code like the following, even though
                // it isn't technically a JS error.
                // ```
                // class Sub extends Base {
                //   constructor() {
                //     let arrow = () => super.prop; // ERROR: super.prop comes before super()
                //     super();
                //     arrow(); // not really executed until here, though
                //   }
                // }
                // ```
                // This behavior is consistent with TypeScript.
                // In general there's no good reason to declare such arrow functions before
                // calling `super()`.
                let constructor_context = *constructor_context;
                self.visit_super_property_access(constructor_context, t, super_node);
            }
        }
    }

    // port: CheckSuper.Context#visitThis
    fn visit_this(&mut self, context: ContextId, _t: &mut NodeTraversal<'_>, this_node: NodeId) {
        match &mut self.contexts[context].kind {
            // port: CheckSuper.ConstructorContext#visitThis
            ContextKind::Constructor(constructor) => {
                if constructor.first_super_call.is_none() {
                    constructor.this_accessed_before_super = Some(this_node);
                }
            }
            _ => {
                // Ignored except in constructor methods.
            }
        }
    }

    // port: CheckSuper.Context#visitReturn
    fn visit_return(&mut self, context: ContextId, t: &mut NodeTraversal<'_>, return_node: NodeId) {
        match &mut self.contexts[context].kind {
            // port: CheckSuper.ConstructorContext#visitReturn
            ContextKind::Constructor(constructor) => {
                if return_node.has_children(t) {
                    constructor.returns_a_value = true;
                }
            }
            _ => {
                // Ignored except in constructor methods.
            }
        }
    }

    // port: CheckSuper.Context#visitContextNode
    fn visit_context_node(&mut self, context: ContextId, t: &mut NodeTraversal<'_>) {
        let context_node = self.contexts[context].get_context_node();
        match &self.contexts[context].kind {
            // port: CheckSuper.ConstructorContext#visitContextNode
            ContextKind::Constructor(constructor) => {
                // We've now visited the entire constructor, so we can decide whether to report
                // errors.
                if !constructor.has_parent_class {
                    if let Some(first_super_call) = constructor.first_super_call {
                        // calling `super()` only makes sense when there is a super class.
                        t.report(first_super_call, &INVALID_SUPER_CALL, &[]);
                    }
                } else {
                    // There is a parent class, so a call to `super()` is required unless the
                    // constructor returns a value.
                    if constructor.first_super_call.is_none() && !constructor.returns_a_value {
                        // The context node is the function itself , but ErrorToFixMapper expects
                        // the error to be reported on the MEMBER_FUNCTION_DEF that is its parent.
                        t.report(
                            context_node.get_parent(t).unwrap(),
                            &MISSING_CALL_TO_SUPER,
                            &[],
                        );
                    }
                    if let Some(this_accessed_before_super) = constructor.this_accessed_before_super
                    {
                        t.report(this_accessed_before_super, &THIS_BEFORE_SUPER, &[]);
                    }
                    if let Some(super_property_accessed_before_super_call) =
                        constructor.super_property_accessed_before_super_call
                    {
                        t.report(
                            super_property_accessed_before_super_call,
                            &SUPER_ACCESS_BEFORE_SUPER_CONSTRUCTOR,
                            &[],
                        );
                    }
                }
            }
            _ => {
                // Called when visiting the root node of this context after all of its children
                // have been visited.
            }
        }
    }
}

impl Default for CheckSuper {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerPass for CheckSuper {
    // port: CheckSuper#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
        check_state!(
            self.context_stack.is_empty(),
            "%s",
            format!("{:?}", self.context_stack)
        );
    }
}

impl Callback for CheckSuper {
    // port: CheckSuper#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::ROOT => {
                check_state!(self.context_stack.is_empty());
                let context = self.new_context(Context::new(n, ContextKind::SuperNotAllowed));
                self.context_stack.push(context);
            }
            Token::FUNCTION => {
                let current_context = *self.context_stack.last().unwrap();
                let new_context = self.get_context_for_function_node(t, current_context, n);
                if new_context != current_context {
                    self.context_stack.push(new_context);
                }
            }
            Token::MEMBER_FIELD_DEF => {
                let field_context = self.create_member_field_def_context(n);
                self.context_stack.push(field_context);
            }
            // For class static blocks
            Token::BLOCK => {
                if NodeUtil::is_class_static_block(t, n) {
                    let new_context = self.create_static_block_context(n);
                    self.context_stack.push(new_context);
                }
            }
            // TODO (user): For class fields
            _ => {}
        }

        true
    }

    // port: CheckSuper#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        let current_context = *check_not_null!(self.context_stack.last());

        match n.get_token(t) {
            Token::SUPER => {
                if Self::is_super_constructor_call(t, n) {
                    // Note: defer recording the call in `currentContext.visitSuperConstructorCall`
                    // until visitng the parent CALL node. This ensures any this/super references
                    // in the call arguments are treated as invalid, pre-super() call, references.
                } else if Self::is_super_property_access(t, n) {
                    self.visit_super_property_access(current_context, t, n);
                } else {
                    // super used some way other than `super()`, `super.prop`, or `super[expr]`.
                    t.report(n, &INVALID_SUPER_USAGE, &[]);
                }
            }
            Token::CALL => {
                let callee = n.get_first_child(t).unwrap();
                if callee.is_super(t) {
                    self.visit_super_constructor_call(current_context, t, callee);
                }
            }
            Token::THIS => self.visit_this(current_context, t, n),
            Token::RETURN => self.visit_return(current_context, t, n),
            Token::ROOT => check_state!(self.context_stack.len() == 1),
            _ => {}
        }

        if n == self.contexts[current_context].get_context_node() {
            self.visit_context_node(current_context, t);
            self.context_stack.pop();
        }
    }
}

/// Tracks lexical context and determines correct traversal behavior based on it.
// port: CheckSuper.Context
struct Context {
    context_node: NodeId,
    kind: ContextKind,
}

enum ContextKind {
    // port: CheckSuper.SuperPropertyAccessAllowedContext
    SuperPropertyAccessAllowed,
    /// Lexical context when not within a method, class, or object literal.
    // port: CheckSuper.SuperNotAllowedContext
    SuperNotAllowed,
    /// Lexicial context within a non-constructor method of either a class or an object literal.
    // port: CheckSuper.MethodContext
    Method {
        is_class_method: bool,
    },
    /// Lexical context within a class constructor method.
    // port: CheckSuper.ConstructorContext
    Constructor(ConstructorContext),
    /// Lexical context within an arrow function enclosed by a class constructor method.
    // port: CheckSuper.ConstructorArrowContext
    ConstructorArrow {
        constructor_context: ContextId,
    },
}

struct ConstructorContext {
    has_parent_class: bool,
    // Will be set to the first `super()` call that appears lexically, if any.
    first_super_call: Option<NodeId>,
    // Call to super() isn't required if the constructor returns a value.
    returns_a_value: bool,
    this_accessed_before_super: Option<NodeId>,
    super_property_accessed_before_super_call: Option<NodeId>,
}

impl Context {
    // port: CheckSuper.Context#Context
    fn new(context_node: NodeId, kind: ContextKind) -> Self {
        Self { context_node, kind }
    }

    // port: CheckSuper.Context#getContextNode
    fn get_context_node(&self) -> NodeId {
        self.context_node
    }

    // port: CheckSuper.MethodContext#MethodContext
    fn new_method_context(t: &NodeTraversal<'_>, function_node: NodeId) -> Self {
        check_argument!(
            function_node.is_function(t) && NodeUtil::is_method_declaration(t, function_node),
            "%s",
            function_node.to_string(t)
        );
        let obj_lit_or_class_members = check_not_null!(function_node.get_grandparent(t));
        let is_class_method = if obj_lit_or_class_members.is_object_lit(t) {
            false
        } else {
            check_state!(
                obj_lit_or_class_members.is_class_members(t),
                "%s",
                obj_lit_or_class_members.to_string(t)
            );
            true
        };
        Self::new(function_node, ContextKind::Method { is_class_method })
    }

    /// Return the property name associated with the method, if any, otherwise NULL.
    // port: CheckSuper.MethodContext#getPropertyName
    fn get_property_name(&self, t: &NodeTraversal<'_>) -> Option<String> {
        let parent = check_not_null!(self.get_context_node().get_parent(t));
        // ```
        // class X {
        //   propertyName() {}
        //   get propertyName() {}
        //   set propertyName(value) {}
        //   [expression]() {} // no name for this one
        // }
        // ```
        if parent.is_member_function_def(t) || parent.is_getter_def(t) || parent.is_setter_def(t) {
            Some(parent.get_string(t).to_string())
        } else {
            None
        }
    }

    // port: CheckSuper.ConstructorContext#ConstructorContext
    fn new_constructor_context(t: &NodeTraversal<'_>, context_node: NodeId) -> Self {
        check_argument!(
            NodeUtil::is_es6_constructor(t, context_node),
            "%s",
            context_node.to_string(t)
        );
        let class_node = NodeUtil::get_enclosing_class(t, context_node).unwrap();
        let has_parent_class = !class_node.get_second_child(t).unwrap().is_empty(t);
        Self::new(
            context_node,
            ContextKind::Constructor(ConstructorContext {
                has_parent_class,
                first_super_call: None,
                returns_a_value: false,
                this_accessed_before_super: None,
                super_property_accessed_before_super_call: None,
            }),
        )
    }

    // port: CheckSuper.ConstructorArrowContext#ConstructorArrowContext
    fn new_constructor_arrow_context(constructor_context: ContextId, arrow_fn: NodeId) -> Self {
        Self::new(
            arrow_fn,
            ContextKind::ConstructorArrow {
                constructor_context,
            },
        )
    }
}
