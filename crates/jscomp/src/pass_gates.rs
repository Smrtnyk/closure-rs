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
#[derive(Debug, PartialEq, Eq)]
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
    decide_in(*MODE, name, compiler)
}

fn decide_in(mode: Mode, name: &str, compiler: &AbstractCompiler) -> Decision {
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
/// that a branch acts only on Closure Library code: a NAME `goog` (a call or qualified name rooted
/// in it, a goog.module or goog.provide file, which the parser and GatherModuleMetadata recognise
/// by such a call), so it does nothing in an arena that never held the string `goog`. A file can
/// also be Closure code without that string: through a @provideGoog JSDoc (its input then
/// provides `goog`) or an ES import or export of a `goog:` module specifier. `goog` below is true
/// in those cases too, so every goog gate opens for them without a proof per pass.
fn may_have_work(name: &str, compiler: &AbstractCompiler) -> bool {
    let options = compiler.get_options();
    // A compilation restored from a TypedAST has colors and properties that were not made from
    // nodes of this arena: every pass runs.
    if options.get_merged_precompiled_libraries() {
        return true;
    }
    let seen = |watched: WatchedName| compiler.has_seen_name(watched);
    let goog = seen(WatchedName::Goog)
        || seen(WatchedName::GoogColonPrefix)
        || compiler.has_seen_jsdoc_bit(Bit::PROVIDE_GOOG);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::Compiler;
    use crate::compiler_pass::CompilerPass;
    use crate::pass_factory::PassFactory;
    use crate::phase_optimizer::PhaseOptimizer;
    use closure_rhino::ir::IR;
    use closure_rhino::jsdoc_info::JSDocInfo;
    use std::sync::Mutex;

    /// A compiler with options and a tree `ROOT(ROOT(SCRIPT), ROOT(SCRIPT))`; returns it with the
    /// externs root, the sources root and the source script.
    fn setup() -> (Compiler, NodeId, NodeId, NodeId) {
        let mut compiler = Compiler::new();
        compiler.init_compiler_options_if_testing();
        let externs_script = IR::script(&mut compiler);
        let script = IR::script(&mut compiler);
        let externs = IR::root(&mut compiler, &[externs_script]);
        let root = IR::root(&mut compiler, &[script]);
        IR::root(&mut compiler, &[externs, root]);
        (compiler, externs, root, script)
    }

    const GATED: [&str; 14] = [
        "removeCastNodes",
        "checkSuper",
        "checkTypeImportCodeReferences",
        "checkGoogRequires",
        "closureCheckModule",
        "closureGoogScopeAliases",
        "closureRewriteModule",
        "rewriteCallerCodeLocation",
        "closureProvidesRequires",
        "replaceMessages",
        "closureReplaceGetCssName",
        "generateExports",
        "replaceToggles",
        "closureOptimizePrimitives",
    ];

    /// The gated passes whose gate is open in `compiler`.
    fn open_gates(compiler: &Compiler) -> Vec<&'static str> {
        GATED
            .into_iter()
            .filter(|name| may_have_work(name, compiler))
            .collect()
    }

    /// The passes a goog idiom opens (checkGoogRequires and closureCheckModule are also open while
    /// there is no module metadata map, as in `setup`).
    const GOOG_GATES: [&str; 10] = [
        "checkTypeImportCodeReferences",
        "checkGoogRequires",
        "closureCheckModule",
        "closureGoogScopeAliases",
        "closureRewriteModule",
        "rewriteCallerCodeLocation",
        "closureProvidesRequires",
        "replaceMessages",
        "closureReplaceGetCssName",
        "replaceToggles",
    ];
    const NO_METADATA: [&str; 2] = ["checkGoogRequires", "closureCheckModule"];

    /// Makes nodes in a compiler's arena.
    type MakeNodes = fn(&mut Compiler);

    #[test]
    fn gates_are_closed_without_triggers_and_ungated_passes_run() {
        let (compiler, ..) = setup();
        assert_eq!(open_gates(&compiler), NO_METADATA);
        assert!(may_have_work("inlineVariables", &compiler));
        for rejected in [
            "rewriteGoogJsImports",
            "closurePrimitives",
            "closureCodeRemoval",
            "j2clChecksPass",
            "stripSideEffectProtection",
            "replaceIdGenerators",
        ] {
            assert!(may_have_work(rejected, &compiler), "{rejected}");
        }
    }

    #[test]
    fn goog_idioms_open_the_goog_gates() {
        // A detached node is enough: the summary covers every node ever made in the arena.
        for make in [
            (|c: &mut Compiler| {
                IR::name(c, "goog");
            }) as MakeNodes,
            |c| {
                IR::string(c, "goog:foo.Bar");
            },
            |c| {
                let n = IR::name(c, "x");
                n.set_original_name(c, Some("goog".into()));
            },
            |c| {
                let mut builder = JSDocInfo::builder();
                builder.record_provide_goog();
                let script = IR::script(c);
                script.set_jsdoc_info(c, builder.build());
            },
        ] {
            let (mut compiler, ..) = setup();
            make(&mut compiler);
            let open = open_gates(&compiler);
            for name in GOOG_GATES {
                assert!(open.contains(&name), "{name} is closed");
            }
        }
    }

    #[test]
    fn each_gate_opens_for_its_own_trigger() {
        let cases: [(&str, MakeNodes); 8] = [
            ("removeCastNodes", |c| {
                let x = IR::name(c, "x");
                IR::cast(c, x, None);
            }),
            ("checkSuper", |c| {
                c.new_node(Token::SUPER);
            }),
            ("checkSuper", |c| {
                c.new_node(Token::CLASS_MEMBERS);
            }),
            ("replaceMessages", |c| {
                IR::name(c, "MSG_HELLO");
            }),
            ("replaceMessages", |c| {
                IR::string(c, "i18n.declareIcuTemplate");
            }),
            ("generateExports", |c| {
                let mut builder = JSDocInfo::builder();
                builder.record_export();
                let n = IR::name(c, "x");
                n.set_jsdoc_info(c, builder.build());
            }),
            ("replaceToggles", |c| {
                IR::name(c, "CLOSURE_TOGGLE_ORDINALS");
            }),
            ("closureOptimizePrimitives", |c| {
                IR::name(c, "goog$object$create");
            }),
        ];
        for (name, make) in cases {
            let (mut compiler, ..) = setup();
            make(&mut compiler);
            let mut expected: Vec<&str> = NO_METADATA.to_vec();
            if !expected.contains(&name) {
                expected.push(name);
            }
            let mut open = open_gates(&compiler);
            open.sort_unstable();
            expected.sort_unstable();
            assert_eq!(open, expected, "{name}");
        }
    }

    #[test]
    fn a_typed_ast_compilation_runs_every_pass() {
        let (mut compiler, ..) = setup();
        compiler
            .get_options_mut()
            .set_merged_precompiled_libraries(true);
        assert_eq!(open_gates(&compiler), GATED);
    }

    #[test]
    fn modes() {
        let (mut compiler, ..) = setup();
        let decisions = |compiler: &Compiler| {
            [Mode::On, Mode::Off, Mode::Verify, Mode::Log]
                .map(|mode| decide_in(mode, "removeCastNodes", compiler))
        };
        assert_eq!(
            decisions(&compiler),
            [
                Decision::Skip,
                Decision::Run,
                Decision::Verify,
                Decision::Skip
            ]
        );
        let x = IR::name(&mut compiler, "x");
        IR::cast(&mut compiler, x, None);
        assert_eq!(
            decisions(&compiler),
            [Decision::Run, Decision::Run, Decision::Run, Decision::Run]
        );
        // An ungated pass always runs.
        assert_eq!(
            decide_in(Mode::On, "inlineVariables", &compiler),
            Decision::Run
        );
    }

    #[test]
    fn verified_run_that_changes_nothing_keeps_the_summary() {
        let (mut compiler, externs, root, _) = setup();
        // A pass that only makes a detached pattern node (as ClosureRewriteModule does).
        run_verified("p", &mut compiler, externs, root, |c| {
            let goog = IR::name(c, "goog");
            IR::getprop(c, goog, "module");
        });
        assert!(!compiler.has_seen_name(WatchedName::Goog));
    }

    fn verify_message(change: fn(&mut Compiler, NodeId)) -> String {
        let (mut compiler, externs, root, script) = setup();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_verified("somePass", &mut compiler, externs, root, |c| {
                change(c, script)
            });
        }));
        let error = result.expect_err("verify mode did not see the change");
        error.downcast_ref::<String>().cloned().unwrap_or_default()
    }

    #[test]
    fn verified_run_that_changes_something_panics() {
        let changes: [fn(&mut Compiler, NodeId); 4] = [
            |c, script| {
                let x = IR::name(c, "x");
                let statement = IR::expr_result(c, x);
                script.add_child_to_back(c, statement);
            },
            |c, script| {
                script.set_original_name(c, Some("x".into()));
            },
            |c, script| {
                script.put_boolean_prop(c, closure_rhino::node::Prop::IS_NAMESPACE, true);
            },
            |c, _| {
                c.get_change_tracker().increment_change_stamp();
            },
        ];
        for change in changes {
            let message = verify_message(change);
            assert!(message.contains("pass somePass changed"), "{message}");
        }
    }

    #[test]
    fn phase_optimizer_skips_a_pass_whose_gate_is_closed() {
        let runs = Arc::new(Mutex::new(0));
        let factory = {
            let runs = runs.clone();
            PassFactory::builder()
                .set_name("removeCastNodes")
                .set_internal_factory(Arc::new(move |_| {
                    let runs = runs.clone();
                    Box::new(move |_: &mut Compiler, _, _| *runs.lock().unwrap() += 1)
                }))
                .build()
        };
        let (mut compiler, externs, root, _) = setup();
        let run_once = |compiler: &mut Compiler| {
            let mut optimizer = PhaseOptimizer::new(compiler, None);
            optimizer.add_one_time_pass(factory.clone());
            optimizer.process(compiler, externs, root);
        };
        run_once(&mut compiler);
        let skipped = matches!(*MODE, Mode::On | Mode::Log);
        assert_eq!(*runs.lock().unwrap(), if skipped { 0 } else { 1 });
        let x = IR::name(&mut compiler, "x");
        IR::cast(&mut compiler, x, None);
        run_once(&mut compiler);
        assert_eq!(*runs.lock().unwrap(), if skipped { 1 } else { 2 });
    }
}
