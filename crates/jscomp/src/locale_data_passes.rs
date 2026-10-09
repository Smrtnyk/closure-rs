/*
 * Copyright 2021 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/LocaleDataPasses.java.

//! Port of LocaleDataPasses.java.
//!
//! Contains compiler passes to protect `goog.LOCALE` from optimization during the main
//! optimizations phase of compilation, then replace it with the specific destination locale near
//! the end of compilation.
use crate::abstract_compiler::AbstractCompiler;
use crate::ast_factory::AstFactory;
use crate::compiler_pass::CompilerPass;
use crate::node_traversal::{
    AbstractPostOrderCallback, AbstractPostOrderCallbackInterface, NodeTraversal,
};
use crate::node_util::NodeUtil;
use closure_rhino::check_not_null;
use closure_rhino::ir::IR;
use closure_rhino::node::{Ast, NodeId, Prop};

/// port: LocaleDataPasses
pub struct LocaleDataPasses;

impl LocaleDataPasses {
    /// Replacements for values that needed to be protected from optimizations.
    pub const GOOG_LOCALE_REPLACEMENT: &'static str = "__JSC_LOCALE__";

    /// Java's `private static final Node QNAME_FOR_GOOG_LOCALE = IR.getprop(IR.name("goog"),
    /// "LOCALE")` ("matching against an actual Node is faster than matching against the string
    /// "goog.LOCALE""). Nodes live in an arena here, so the detached template is built in the
    /// compiler's arena by the callback that uses it.
    // port: LocaleDataPasses#QNAME_FOR_GOOG_LOCALE
    fn qname_for_goog_locale(ast: &mut Ast) -> NodeId {
        let goog = IR::name(ast, "goog");
        IR::getprop(ast, goog, "LOCALE")
    }

    // port: LocaleDataPasses#isGoogDotLocaleReference
    fn is_goog_dot_locale_reference(ast: &Ast, qname_for_goog_locale: NodeId, n: NodeId) -> bool {
        // NOTE: Theoretically there could be a local variable named `goog`, but it's not worth
        // checking for that.
        n.matches_qualified_name_node(ast, qname_for_goog_locale)
    }
}

/// port: LocaleDataPasses.ProtectGoogLocale
pub struct ProtectGoogLocale;

impl ProtectGoogLocale {
    // port: LocaleDataPasses.ProtectGoogLocale#<init>
    pub fn new(_compiler: &mut AbstractCompiler) -> Self {
        Self
    }
}

impl CompilerPass for ProtectGoogLocale {
    // port: LocaleDataPasses.ProtectGoogLocale#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        // Create the extern symbols
        NodeUtil::create_synthesized_externs_symbol(
            compiler,
            LocaleDataPasses::GOOG_LOCALE_REPLACEMENT,
        );
        let protect_locale_callback = ProtectCurrentLocale::new(compiler);
        NodeTraversal::traverse(
            compiler,
            root,
            &mut AbstractPostOrderCallback::new(protect_locale_callback),
        );
    }
}

/// port: LocaleDataPasses.ProtectCurrentLocale
///
/// Protect `goog.LOCALE` by replacing it with an extern-defined name.
struct ProtectCurrentLocale {
    ast_factory: AstFactory,
    qname_for_goog_locale: NodeId,
}

impl ProtectCurrentLocale {
    // port: LocaleDataPasses.ProtectCurrentLocale#<init>
    fn new(compiler: &mut AbstractCompiler) -> Self {
        let ast_factory = compiler.create_ast_factory();
        Self {
            ast_factory,
            qname_for_goog_locale: LocaleDataPasses::qname_for_goog_locale(compiler),
        }
    }
}

impl AbstractPostOrderCallbackInterface for ProtectCurrentLocale {
    // port: LocaleDataPasses.ProtectCurrentLocale#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, parent: Option<NodeId>) {
        if LocaleDataPasses::is_goog_dot_locale_reference(t, self.qname_for_goog_locale, n) {
            // We will replace the RHS of `goog.LOCALE = goog.define(...);`, but everywhere
            // else we will replace `goog.LOCALE` itself, so we don't have to waste time
            // inlining it later.
            let node_to_replace = if NodeUtil::is_lhs_of_assign(t, n) {
                n.get_next(t).unwrap()
            } else {
                n
            };
            let replacement = self.ast_factory.create_name(
                t.get_compiler(),
                LocaleDataPasses::GOOG_LOCALE_REPLACEMENT,
                AstFactory::type_node(node_to_replace),
            );
            replacement.put_boolean_prop(t, Prop::IS_CONSTANT_NAME, true);
            node_to_replace.replace_with(t, replacement);
            t.get_compiler()
                .report_change_to_enclosing_scope(parent.unwrap());
        }
    }
}

/// port: LocaleDataPasses.LocaleSubstitutions
///
/// This class replaces `__JSC_LOCALE__` with the actual locale string.
pub struct LocaleSubstitutions {
    qname_for_locale: NodeId,
    ast_factory: AstFactory,
    locale: Option<String>,
}

impl LocaleSubstitutions {
    const DEFAULT_LOCALE: &'static str = "en";

    // port: LocaleDataPasses.LocaleSubstitutions#<init>
    pub fn new(compiler: &mut AbstractCompiler, locale: Option<&str>) -> Self {
        let qname_for_locale = IR::name(compiler, LocaleDataPasses::GOOG_LOCALE_REPLACEMENT);
        let ast_factory = compiler.create_ast_factory();
        Self {
            qname_for_locale,
            ast_factory,
            // Use the "default" locale if not otherwise set.
            locale: Some(locale.unwrap_or(Self::DEFAULT_LOCALE).to_string()),
        }
    }
}

impl CompilerPass for LocaleSubstitutions {
    // port: LocaleDataPasses.LocaleSubstitutions#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        // Create the extern symbol
        NodeTraversal::traverse(
            compiler,
            root,
            &mut AbstractPostOrderCallback::new(LocaleSubstitutionsCallback { owner: self }),
        );
    }
}

/// Rust-only: Java's LocaleSubstitutions is both the pass and its own traversal callback.
struct LocaleSubstitutionsCallback<'a> {
    owner: &'a mut LocaleSubstitutions,
}

impl AbstractPostOrderCallbackInterface for LocaleSubstitutionsCallback<'_> {
    // port: LocaleDataPasses.LocaleSubstitutions#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        if n.matches_name_node(t, self.owner.qname_for_locale) {
            let locale = check_not_null!(self.owner.locale.clone());
            let replacement = self
                .owner
                .ast_factory
                .create_string(t.get_compiler(), locale.as_str());
            replacement.srcref(t, n);
            n.replace_with(t, replacement);
            t.get_compiler()
                .report_change_to_enclosing_scope(replacement);
        }
    }
}
