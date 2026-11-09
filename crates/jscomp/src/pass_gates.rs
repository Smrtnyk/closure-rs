/*
 * Copyright 2026 The closure-rs Authors.
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

//! Rust-only, not in Java (crates/DESIGN.md, "Performance divergences"): passes that cannot find
//! anything to do are not run.
//!
//! Many passes only react to Closure Library idioms (`goog.provide`, `goog.getCssName`, ...),
//! message names (`MSG_*`), JSDoc flags (`@export`) or node kinds (CAST). Java runs them on every
//! input, and on code without those idioms each walks the whole AST without effect (only the J2CL
//! passes ask `J2clSourceFileChecker` first). Here `PhaseOptimizer.NamedPass#process` asks
//! `may_have_work` before it creates and runs a pass; everything around the run (precondition
//! checks, tracer, performance tracker, `beforePass`/`afterPass`, validity checks) happens either
//! way.
//!
//! The answer comes from the node arena's presence summary (`Ast::has_seen_token`,
//! `Ast::has_seen_name`, `Ast::has_seen_jsdoc_bit`: what has ever existed in the arena), the
//! options and O(1) facts of the compiler. Each gate below is proven to skip only runs that would
//! change nothing: no tree, JSDoc, type or color, no diagnostic, no compiler state other than
//! state whose contents are unobservable by construction (see "Lemmas"). A pass without a gate
//! always runs.
//!
//! Lemmas the gates rely on:
//!
//! - L1 (traversal): `NodeTraversal` itself writes nothing outside the traversal object: with a
//!   callback that changes nothing, a traversal changes nothing (`node_traversal.rs`:
//!   `traverse_branch`, `push_scope`, `init_traversal`, `get_input`, `get_chunk` only read the
//!   compiler; a scope is made only when a callback asks for it).
//! - L2 (syntactic scopes): asking a traversal for a syntactic scope (`get_scope`) or a scope for
//!   a var (`get_var`, which may allocate an implicit var) is unobservable: the syntactic scope
//!   cache hands out a scope again only when a new scan would make the same one
//!   (`syntactic_scope_cache.rs`), so whether an earlier pass made a scope only changes which
//!   handles exist, and handles are compared, never ordered or printed.
//! - L3 (detached nodes): making nodes that are never attached or stored (patterns to match
//!   against) is unobservable: every later node gets the next free id either way, so any two
//!   nodes compare in creation order either way, and node ids are never printed.
//! - L4 (registry errors): `Compiler::create_ast_factory` and `get_type_registry` report the
//!   errors queued by the type registry, which the compiler otherwise reports before it next
//!   reports or reads any error (`report_queued_type_registry_errors`): the errors reach the error
//!   manager in the same order either way.
//! - L5 (module callbacks): `AbstractModuleCallback` panics when a script has no input or no
//!   module metadata. Neither happens after GatherModuleMetadata: its factory first makes the
//!   synthetic externs script, it records metadata by input path for every script of both roots
//!   (failing itself for a script without input), and later scripts come only from the
//!   synthetic code input (added before it) or the type summary input (made by runtime library
//!   injection, after the checks).
//!
//! `CLOSURE_RS_PASS_GATES=off` runs every pass; `CLOSURE_RS_PASS_GATES=verify` runs every pass
//! too, and around each run that a gate would skip compares a fingerprint of the trees (tokens,
//! strings, positions, properties, JSDoc and its type expressions, types or colors), the error and
//! warning counts, the change stamp and a few other compiler fields, and panics, naming the pass,
//! on a difference. `CLOSURE_RS_PASS_GATES=log` skips as by default and names each skipped run
//! on stderr.

use crate::abstract_compiler::AbstractCompiler;
use closure_rhino::jsdoc_info::Bit;
use closure_rhino::node::{NodeId, WatchedName};
use closure_rhino::token::Token;
use std::sync::{Arc, LazyLock};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    On,
    Off,
    Verify,
    Log,
}

static MODE: LazyLock<Mode> =
    LazyLock::new(|| match std::env::var("CLOSURE_RS_PASS_GATES").as_deref() {
        Ok("off") => Mode::Off,
        Ok("verify") => Mode::Verify,
        Ok("log") => Mode::Log,
        _ => Mode::On,
    });

/// What `PhaseOptimizer.NamedPass#process` does with one run of a pass.
pub(crate) enum Decision {
    /// Create and run the pass.
    Run,
    /// Neither create nor run it: it would change nothing.
    Skip,
    /// Create and run it, and check that it changed nothing (`run_verified`).
    Verify,
}

/// Rust-only (performance divergence): whether the pass named `name` must run now (see the module
/// documentation). Evaluated before every run of the pass.
pub(crate) fn decide(name: &str, compiler: &AbstractCompiler) -> Decision {
    let mode = *MODE;
    if mode == Mode::Off || may_have_work(name, compiler) {
        return Decision::Run;
    }
    match mode {
        Mode::Verify => Decision::Verify,
        Mode::Log => {
            eprintln!("CLOSURE_RS_PASS_GATES: skipped {name}");
            Decision::Skip
        }
        _ => Decision::Skip,
    }
}

/// The gate registry: false only when the pass named `name` provably has nothing to do. The proof
/// of each gate is the comment above its arm: (1) the factory, (2) every traversal and callback of
/// `process`, (3) every diagnostic, (4) every piece of compiler state written. "Needs goog" means
/// that a branch acts only on a NAME `goog` (a call or qualified name rooted in it, a goog.module
/// or goog.provide file, which the parser and GatherModuleMetadata recognise by such a call), so
/// it does nothing in an arena that never held the string `goog`.
fn may_have_work(name: &str, compiler: &AbstractCompiler) -> bool {
    let options = compiler.get_options();
    // A compilation restored from a TypedAST has colors and properties that were not made from
    // nodes of this arena: every pass runs.
    if options.get_merged_precompiled_libraries() {
        return true;
    }
    let seen = |watched: WatchedName| compiler.has_seen_name(watched);
    let goog = seen(WatchedName::Goog);
    match name {
        // removeCastNodes (remove_cast_nodes.rs). (1) `RemoveCastNodes::new`: unit struct.
        // (2) one post-order traversal of externs and sources (L1); `visit` acts only on a CAST
        // node. (3) none. (4) only in the CAST branch.
        "removeCastNodes" => compiler.has_seen_token(Token::CAST),

        // checkSuper (check_super.rs). (1) empty context lists. (2) one traversal of the sources
        // (L1). A constructor context needs a constructor in CLASS_MEMBERS (`is_es6_constructor`);
        // in every other context `visit_this` and `visit_return` do nothing and `visit` reports
        // only on a SUPER node. (3) every report needs a SUPER node or a constructor context; the
        // `check_state!`s hold for any valid tree (the ROOT context is pushed and popped once;
        // methods sit in object literals or class members). (4) none.
        "checkSuper" => {
            compiler.has_seen_token(Token::SUPER) || compiler.has_seen_token(Token::CLASS_MEMBERS)
        }

        // checkTypeImportCodeReferences (check_type_import_code_references.rs). (1) unit struct.
        // (2) one traversal of the sources (L1); for each NAME it asks the scope for the var (L2)
        // and reports only when the var is declared from a call to `goog.requireType` (needs
        // goog). (3) that report only. (4) scopes only (L2).
        "checkTypeImportCodeReferences" => goog,

        // checkGoogRequires (check_closure_imports.rs). (1) the factory panics without a module
        // metadata map (the gate then lets it run); `CheckClosureImports::new` stores the map.
        // (2) `AbstractModuleCallback` traversals of externs and sources (L1, L5); `enter_module`
        // collects the module's goog namespaces (none without goog: they come from
        // goog.provide/module/declareModuleId calls, gather_module_metadata.rs
        // `visit_goog_call`); `visit` acts on calls to goog.require/forwardDeclare/requireType/
        // requireDynamic/module.get (needs goog) and, for each NAME, asks the scope for its var
        // (L2) and returns unless the var is declared from such a call (needs goog). (3) every
        // report is in those goog branches. (4) scopes only (L2).
        "checkGoogRequires" => goog || compiler.get_module_metadata_map().is_none(),

        // closureCheckModule (closure_check_module.rs). (1) as checkGoogRequires. (2) an
        // `AbstractModuleCallback` traversal of the sources (L1, L5); `enter_module` keeps a
        // module info only for a goog module (needs goog); without one, `visit` only checks for
        // calls to goog.module, goog.module.declareLegacyNamespace and goog.provide (needs goog).
        // (3) all reports are in those branches. (4) none.
        "closureCheckModule" => goog || compiler.get_module_metadata_map().is_none(),

        // closureGoogScopeAliases (scoped_aliases.rs). (1) `maybe_initialize` writes the
        // preprocessor symbol table only when the options preserve detailed source info (then
        // the gate lets the pass run); `ScopedAliases::new` reads the module metadata map.
        // (2) one traversal of the sources (L1) with a scoped callback: `enter_scope` acts only in
        // a function that is the argument of a goog.scope call (needs goog); `visit` records the
        // input id in the pass, looks for goog.scope calls (needs goog) and returns outside a
        // goog.scope body. After the traversal the alias, definition and scope-call lists are
        // empty. (3) all reports are in goog.scope bodies. (4) none.
        "closureGoogScopeAliases" => goog || options.preserves_detailed_source_info(),

        // closureRewriteModule (closure_rewrite_module.rs). (1) `maybe_initialize` as above;
        // `ClosureRewriteModule::new` checks that the top scope is global (it is: `set_top_scope`
        // only takes a scope without parent), makes an AST factory (L4) and the nodes it matches
        // calls against (L3). (2) per script, `UnwrapGoogLoadModule` (acts on goog.loadModule
        // calls, needs goog), `ScriptPreprocessor` (goog.module files, and assignments to the
        // implicit `exports` var of a goog.module, needs goog; scope lookups, L2) and
        // `ScriptRecorder` (records goog calls, needs goog, and top-level names into the pass's
        // own script descriptions); then `ScriptUpdater` over externs and sources: scopes (L2);
        // a MODULE_BODY is entered only for a goog.module file; the name and JSDoc rewrites act
        // only on names recorded from goog calls, on top-level names of a module that is entered
        // (none), or on goog namespaces (none); `declare_synthetic_externs` and the goog.module.get
        // and goog.requireDynamic lists are empty. (3) every report is in a goog branch;
        // `report_unrecognized_requires` walks an empty list. (4) scopes (L2) and detached nodes
        // (L3) only.
        "closureRewriteModule" => goog || options.preserves_detailed_source_info(),

        // rewriteCallerCodeLocation (rewrite_caller_code_location.rs). (1) makes an AST factory
        // (L4). (2) `FindCallerLocationFunctions` over the whole tree (L1) acts only on uses of
        // goog.callerLocation (needs goog); the second traversal runs only when it found any.
        // (3) all reports are in those branches. (4) none.
        "rewriteCallerCodeLocation" => goog,

        // closureProvidesRequires (process_closure_provides_and_requires.rs). (1)
        // `maybe_initialize` as above; makes an AST factory (L4). (2) `CollectDefinitions` over
        // externs and sources (L1): the provided names are only `goog` itself (its ProvidedName
        // has no node); goog method calls need goog; a candidate definition or stub is recorded
        // only for a provided name, i.e. a name `goog` (needs goog); a legacy goog.module body
        // needs goog; `check_nested_typedef_provide` returns for a name that is not a provided
        // dotted name. `replace` of the `goog` name only sets its replacement node; the require
        // and forwardDeclare lists are empty. (3) all reports are in goog branches. (4) the
        // factory's closure adds the pass's exported names (empty) to the compiler: unchanged.
        "closureProvidesRequires" => goog || options.preserves_detailed_source_info(),

        // replaceMessages, the full replacement (replace_messages.rs `FullReplacementPass`; the
        // same name is used by the Chrome pass, which the gate lets run). (1) makes an AST
        // factory (L4) and reads the message bundle's id generator. (2) one `JsMessageVisitor`
        // traversal of the sources (L1): `collect_get_msg_call` needs a callee goog.getMsg or
        // goog.getMsgWithFallback (needs goog) or a qualified name ending in
        // `declareIcuTemplate`; `check_message_initialization` acts only when the key (the
        // string or original name of the NAME, GETPROP, STRING_KEY or MEMBER_FIELD_DEF) starts
        // with `MSG_` or `$jscomp$scope$` (its `check_state!` holds: a STRING_KEY with a value
        // outside an object pattern is in an object literal). (3) MESSAGE_NODE_IS_ORPHANED needs
        // a collected call; the others need a message key. (4) none.
        "replaceMessages" => {
            goog || seen(WatchedName::MsgPrefix)
                || seen(WatchedName::JscompScopePrefix)
                || seen(WatchedName::DeclareIcuTemplateSuffix)
                || options.should_run_replace_messages_for_chrome()
                || options.do_late_localization()
        }

        // closureReplaceGetCssName (default_pass_config.rs `ClosureReplaceGetCssNamePass`,
        // replace_css_names.rs). (1) unit struct. (2) `ReplaceCssNames::new` makes an AST
        // factory (L4); `FindSetCssNameTraversal` acts on goog.setCssNameMapping (needs goog);
        // `GatherCssNamesTraversal` reports a @sassGeneratedCssTs JSDoc in the wrong place and
        // collects goog.getCssName calls (needs goog); `CountCssNamesBySymbol` acts only on the
        // classes objects found by the collected calls (none). (3) only those. (4) the pass
        // stores its gathered CSS names, `None` unless the options gather them: the gate requires
        // that the options do not and that no names are stored yet.
        "closureReplaceGetCssName" => {
            goog || compiler.has_seen_jsdoc_bit(Bit::SASS_GENERATED_CSS_TS)
                || options.should_gather_css_names()
                || compiler.has_css_names()
        }

        // generateExports (default_pass_config.rs `GenerateExportsPass`, generate_exports.rs).
        // (1) reads the coding convention. (2) `FindExportableNodes` over the sources (L1)
        // returns for every node without an @export JSDoc; with no exports the pass adds nothing.
        // (3) all reports need an @export. (4) the exported names it adds to the compiler are
        // none.
        "generateExports" => compiler.has_seen_jsdoc_bit(Bit::EXPORT),

        // replaceToggles (replace_toggles.rs). (1) makes an AST factory (L4). (2) one traversal
        // of the sources (L1); `visit` acts on a declaration of `CLOSURE_TOGGLE_ORDINALS` and on
        // calls to goog.readToggleInternalDoNotCallDirectly (needs goog). (3) all reports are in
        // those branches. (4) none.
        "replaceToggles" => goog || seen(WatchedName::ClosureToggleOrdinals),

        // closureOptimizePrimitives (closure_optimize_primitives.rs). (1) reads an option.
        // (2) one traversal of the sources (L1); `visit` acts on calls of a property rename
        // function (the coding conventions name the NAME `JSCompiler_renameProperty`, which the
        // pass leaves as it is, goog.reflect.objectProperty and `goog$reflect$objectProperty`), of goog.object.create(Set) (needs goog) and of the NAMEs
        // `goog$object$create(Set)` and `module$contents$goog$object_create(Set)`. (3) only on
        // such calls. (4) none.
        "closureOptimizePrimitives" => {
            goog || seen(WatchedName::GoogDollarPrefix)
                || seen(WatchedName::ModuleContentsGoogPrefix)
        }

        _ => true,
    }
}

/// Everything a skipped run must leave as it is, for the verification mode.
#[derive(PartialEq, Eq, Debug)]
struct Fingerprint {
    tree: u64,
    errors: i32,
    warnings: i32,
    change_stamp: i32,
    exported_names: usize,
    module_metadata_map: Option<usize>,
    module_map: Option<usize>,
}

fn fingerprint(compiler: &AbstractCompiler, externs: NodeId, root: NodeId) -> Fingerprint {
    let mut state = std::hash::DefaultHasher::new();
    let tree_root = externs.get_parent(compiler).unwrap_or(root);
    compiler.hash_tree_for_verification(tree_root, &mut state);
    if externs.get_parent(compiler).is_none() {
        compiler.hash_tree_for_verification(externs, &mut state);
    }
    Fingerprint {
        tree: std::hash::Hasher::finish(&state),
        errors: compiler.get_error_count(),
        warnings: compiler.get_warning_count(),
        change_stamp: compiler.get_change_tracker_ref().get_change_stamp(),
        exported_names: compiler.get_exported_names().len(),
        module_metadata_map: compiler
            .get_module_metadata_map()
            .map(|m| Arc::as_ptr(m) as usize),
        module_map: compiler.get_module_map().map(|m| Arc::as_ptr(m) as usize),
    }
}

/// Runs `run` (the pass that `name`'s gate would skip) between two fingerprints and panics if they
/// differ. Afterwards the presence summary is the one from before the run: a skipped run adds
/// nothing to it, so the later decisions of this mode stay those of the normal mode.
pub(crate) fn run_verified(
    name: &str,
    compiler: &mut AbstractCompiler,
    externs: NodeId,
    root: NodeId,
    run: impl FnOnce(&mut AbstractCompiler),
) {
    let summary = compiler.presence_summary();
    let before = fingerprint(compiler, externs, root);
    run(compiler);
    let after = fingerprint(compiler, externs, root);
    if before != after {
        panic!(
            "CLOSURE_RS_PASS_GATES=verify: pass {name} changed the compilation although its gate \
             would skip it: before {before:?}, after {after:?}"
        );
    }
    compiler.restore_presence_summary_for_verification(summary);
}
