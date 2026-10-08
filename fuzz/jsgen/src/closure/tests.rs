//! Unit tests for the Closure builder (`closure/`).

use super::program::{FileKind, chunk2, chunk3, generate_closure_nth, may_import};
use crate::{Config, generate_nth};

fn balanced(s: &str) -> bool {
    // Rough structural check that ignores string contents and comments' braces in types:
    // generated JSDoc braces are always balanced too, so a plain count works.
    let mut depth: i64 = 0;
    for c in s.chars() {
        match c {
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return false;
        }
    }
    depth == 0
}

#[test]
fn chunk_layout_matches_case_args() {
    // case_args._chunk_flags: chunks2 = ceil_half; chunks3 = fanout_thirds.
    for n in 2..=8usize {
        let c2: Vec<usize> = (0..n).map(|i| chunk2(i, n)).collect();
        let first = n.div_ceil(2);
        assert_eq!(c2.iter().filter(|&&c| c == 0).count(), first, "n={n}");
        let k0 = n.div_ceil(3);
        let k1 = (n - k0).div_ceil(2);
        let c3: Vec<usize> = (0..n).map(|i| chunk3(i, n)).collect();
        assert_eq!(c3.iter().filter(|&&c| c == 0).count(), k0, "n={n}");
        assert_eq!(c3.iter().filter(|&&c| c == 1).count(), k1, "n={n}");
        assert_eq!(c3.iter().filter(|&&c| c == 2).count(), n - k0 - k1, "n={n}");
    }
}

#[test]
fn may_import_respects_all_layouts() {
    for n in 2..=8usize {
        for i in 0..n {
            for j in 0..n {
                if may_import(i, j, n) {
                    assert!(j < i);
                    assert!(chunk2(j, n) <= chunk2(i, n));
                    let (cj, ci) = (chunk3(j, n), chunk3(i, n));
                    assert!(cj == 0 || cj == ci, "n={n} i={i} j={j}");
                }
            }
        }
    }
    // c2 may never import c1 under chunks3 (n = 6: c0 = {0,1}, c1 = {2,3}, c2 = {4,5}).
    assert!(!may_import(4, 2, 6));
    assert!(may_import(4, 1, 6));
    assert!(may_import(5, 4, 6));
}

#[test]
fn multi_file_programs_are_deterministic_and_legal() {
    let cfg = Config::default();
    let mut kinds = std::collections::BTreeSet::new();
    for i in 0..300u64 {
        let p = generate_closure_nth(77, i, &cfg);
        assert_eq!(p, generate_closure_nth(77, i, &cfg), "index {i}");
        let n = p.files.len();
        assert!(n >= 2, "index {i}: {n} files");
        assert_eq!(p.deps.len(), n);
        kinds.insert(format!("{:?}", p.kind));
        let mut names: Vec<&str> = p.files.iter().map(|(f, _)| f.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), n, "duplicate file names");
        for (k, (name, src)) in p.files.iter().enumerate() {
            for &j in &p.deps[k] {
                assert!(may_import(k, j, n), "index {i}: {k} imports {j} of {n}");
                let target = &p.files[j].0;
                let stem = target.trim_end_matches(".js");
                assert!(
                    src.contains(&format!("'./{target}'"))
                        || src.contains(&format!("'gen.{stem}'")),
                    "index {i}: {name} does not import {target}"
                );
            }
        }
        match p.kind {
            FileKind::GoogModule | FileKind::GoogProvide => {
                assert_eq!(p.files[0].0, "base.js");
                assert!(p.files[0].1.contains("@provideGoog"));
                for (_, src) in &p.files[1..] {
                    assert!(src.starts_with("goog.module(") || src.starts_with("goog.provide("));
                }
            }
            FileKind::CommonJs => {
                assert_eq!(
                    p.extra_flags,
                    vec!["--process_common_js_modules".to_string()]
                );
            }
            _ => assert!(p.extra_flags.is_empty()),
        }
        // Every non-first module imports something: chunk boundaries get crossed.
        let first_module = usize::from(p.files[0].0 == "base.js");
        for k in (first_module + 1)..n {
            assert!(!p.deps[k].is_empty(), "index {i}: file {k} imports nothing");
        }
    }
    assert_eq!(kinds.len(), 4, "all four module systems appear: {kinds:?}");
}

#[test]
fn single_file_closure_idioms_appear() {
    let cfg = Config::default();
    let mut all = String::new();
    for i in 0..400u64 {
        all.push_str(&generate_nth(4242, i, &cfg));
    }
    for needle in [
        "@define",
        "@constructor",
        ".prototype.",
        "@enum",
        "@template",
        "@implements",
        "@override",
        "@record",
        "@typedef",
        "@private",
        "@struct",
        "@nocollapse",
        "window['",
        "/** @const */\nvar ns",
    ] {
        assert!(all.contains(needle), "no `{needle}` in 400 programs");
    }
}

#[test]
fn items_are_balanced_in_every_style() {
    use super::items::{self, Style};
    use crate::engine::Gen;
    use crate::rng::Rng;
    let cfg = Config::default();
    let styles = [
        Style::Script,
        Style::GoogModule,
        Style::EsModule,
        Style::CommonJs,
        Style::Provide("gen.m1".into()),
    ];
    for s in 0..500u64 {
        for st in &styles {
            let mut g = Gen::new(Rng::new(s), &cfg);
            let it = items::declare(&mut g, st);
            g.nl();
            items::observe(&mut g, &|_| String::new(), std::slice::from_ref(&it));
            assert!(balanced(&g.out), "seed {s} {st:?}:\n{}", g.out);
            assert!(g.out.contains(it.name()));
        }
    }
}

#[test]
fn module_filler_avoids_module_illegal_tokens() {
    let cfg = Config::default();
    for i in 0..300u64 {
        let p = generate_closure_nth(5, i, &cfg);
        if p.kind == FileKind::GoogModule {
            for (_, src) in &p.files[1..] {
                for line in src.lines() {
                    // `this` only inside functions: indented, or a one-line method.
                    let ok = line.starts_with(' ') || line.contains("function");
                    assert!(ok || !line.contains("this"), "index {i}: {line}");
                }
            }
        }
    }
}

#[test]
fn define_only_at_top_level() {
    // A `@define` must be a global declaration: never indented (nested).
    let cfg = Config::default();
    for i in 0..400u64 {
        let p = generate_nth(99, i, &cfg);
        for line in p.lines() {
            if line.contains("@define") {
                assert!(
                    !line.starts_with(' '),
                    "nested @define in program {i}:\n{p}"
                );
            }
        }
    }
}

#[test]
fn pass_targeted_idioms_appear() {
    // passes.rs and the multi-file primitives (D-016 item 5): each idiom must be generated.
    let cfg = Config::default();
    let mut single = String::new();
    for i in 0..400u64 {
        single.push_str(&generate_nth(4243, i, &cfg));
    }
    for needle in [
        ".prototype.w",
        "arguments.length",
        "constructor(v) { super(v); }",
        "constructor() {}",
        "@nosideeffects",
        "@dict",
    ] {
        assert!(single.contains(needle), "no `{needle}` in 400 programs");
    }
    let mut multi = String::new();
    for i in 0..300u64 {
        for (_, src) in generate_closure_nth(4243, i, &cfg).files {
            multi.push_str(&src);
        }
    }
    for needle in [
        ".base(this, 'constructor', v)",
        "goog.forwardDeclare('gen.fwd.",
        "goog.addDependency('gen/",
        ".prototype.w",
    ] {
        assert!(
            multi.contains(needle),
            "no `{needle}` in 300 multi-file programs"
        );
    }
}

#[test]
fn wide_classes_are_not_used_by_their_own_file() {
    // crossChunkMethodMotion needs the methods referenced only by importers.
    let cfg = Config::default();
    for i in 0..300u64 {
        let p = generate_closure_nth(31, i, &cfg);
        for (name, src) in &p.files {
            // A wide class declared here never has its methods called here.
            for decl in src.match_indices("function W").map(|(k, _)| k) {
                let id: String = src[decl + 9..]
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric())
                    .collect();
                let low = id.to_lowercase();
                for m in 'a'..='j' {
                    assert!(
                        !src.contains(&format!(".{low}{m}()")),
                        "index {i}: {name} calls a method of its own wide class {id}"
                    );
                }
            }
        }
    }
}

#[test]
fn module_forms_appear() {
    // The main ClosureRewriteModule / ProcessCommonJSModules / Es6RewriteModules
    // forms, none of which the multi-file generator produced before.
    let cfg = Config::default();
    let mut all = String::new();
    for i in 0..400u64 {
        for (_, src) in generate_closure_nth(91, i, &cfg).files {
            all.push_str(&src);
            all.push('\n');
        }
    }
    let lines: Vec<&str> = all.lines().collect();
    let has = |f: &dyn Fn(&str) -> bool| lines.iter().any(|l| f(l));
    let checks: [(&str, bool); 11] = [
        (
            "const {..} = goog.require",
            has(&|l| l.starts_with("const {") && l.contains("= goog.require(")),
        ),
        ("goog.module.get", all.contains("goog.module.get('gen.")),
        ("exports = {..}", has(&|l| l.starts_with("exports = {"))),
        (
            "exports = X",
            has(&|l| l.starts_with("exports = ") && !l.starts_with("exports = {")),
        ),
        (
            "module.exports = {..}",
            has(&|l| l.starts_with("module.exports = {")),
        ),
        (
            "module.exports = X",
            has(&|l| l.starts_with("module.exports = ") && !l.starts_with("module.exports = {")),
        ),
        (
            "const {..} = require",
            has(&|l| l.starts_with("const {") && l.contains("= require(")),
        ),
        (
            "export let / export var",
            has(&|l| l.starts_with("export let ") || l.starts_with("export var ")),
        ),
        (
            "export default function|class",
            has(&|l| {
                l.starts_with("export default function ") || l.starts_with("export default class ")
            }),
        ),
        (
            "import {a as b} / export {x} from",
            has(&|l| l.starts_with("import {") && l.contains(" as "))
                && has(&|l| l.starts_with("export {") && l.contains("} from '")),
        ),
        ("import './m.js'", has(&|l| l.starts_with("import './m"))),
    ];
    for (name, ok) in checks {
        assert!(ok, "module form never generated: {name}");
    }
}
