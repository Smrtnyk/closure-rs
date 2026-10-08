//! Operators, references, calls, member access, optional chaining, object/array literals,
//! assignment (incl. logical and destructuring), `new`, spread, `yield`/`await`, and
//! constant-foldable shapes that give the peephole passes work.

use super::{dialect, util};
use crate::engine::{Binding, Gen, Production, always};

const BINOPS: &[&str] = &[
    "+",
    "+",
    "-",
    "*",
    "/",
    "%",
    "**",
    "<<",
    ">>",
    ">>>",
    "&",
    "|",
    "^",
    "&&",
    "||",
    "??",
    "==",
    "!=",
    "===",
    "!==",
    "<",
    "<=",
    ">",
    ">=",
    "in",
    "instanceof",
    ",",
];
// Unary `+` is only applied to non-literal operands below: `+123n` is rejected at parse time
// ("Cannot convert a BigInt value to a number").
const UNOPS: &[&str] = &["-", "!", "~", "typeof ", "void ", "!!"];
const ASSIGN_OPS: &[&str] = &[
    "=", "=", "+=", "-=", "*=", "/=", "%=", "**=", "<<=", ">>=", ">>>=", "&=", "|=", "^=", "&&=",
    "||=", "??=",
];
pub const PROPS: &[&str] = &[
    "length",
    "x",
    "y",
    "a",
    "b",
    "toString",
    "valueOf",
    "constructor",
    "size",
    "value",
    "done",
    "0",
];

fn prop(g: &mut Gen) -> &'static str {
    let p = *g.rng.pick(PROPS);
    if p == "0" { "x" } else { p }
}

fn reference(g: &mut Gen) {
    util::reference(g);
}

fn binary(g: &mut Gen) {
    let op = *g.rng.pick(BINOPS);
    // The comma operator is wrapped so it never splits declarators, arguments or properties.
    if op == "," {
        g.w("(");
    }
    g.pexpr();
    g.w(" ");
    g.w(op);
    g.w(" ");
    if op == "in" {
        if g.rng.chance(1, 2) {
            g.w("globalThis");
        } else {
            g.w("{x: 1, a: 2}");
        }
    } else if op == "instanceof" {
        let c = g
            .visible(|b| b == Binding::Class || b == Binding::Function)
            .unwrap_or_else(|| "Object".to_string());
        g.w(&c);
    } else {
        g.pexpr();
    }
    if op == "," {
        g.w(")");
    }
}

fn unary(g: &mut Gen) {
    if g.rng.chance(1, 6) {
        // `+ref` / `+'3'` (never a BigInt literal operand).
        g.w("+");
        if g.rng.chance(1, 2) {
            util::reference(g);
        } else {
            let s = *g
                .rng
                .pick(&["'3'", "''", "'0x10'", "true", "null", "[]", "'1e3'"]);
            g.w(s);
        }
        return;
    }
    let op = *g.rng.pick(UNOPS);
    g.w(op);
    g.pexpr();
}

fn delete_expr(g: &mut Gen) {
    if dialect::sloppy() && !g.ctx.in_class_method && g.rng.chance(1, 3) {
        // `delete identifier` is a strict-mode error; sloppy only. Only declared names:
        // Closure rejects `delete this` ("Invalid delete operand").
        if let Some(n) = g.visible(|b| !matches!(b, Binding::Class)) {
            g.w("delete ");
            g.w(&n);
            return;
        }
    }
    g.w("delete ");
    g.pexpr();
    if g.rng.chance(1, 2) {
        g.w(".");
        let p = prop(g);
        g.w(p);
    } else {
        g.w("[");
        g.expr();
        g.w("]");
    }
}

fn conditional(g: &mut Gen) {
    g.pexpr();
    g.w(" ? ");
    g.pexpr();
    g.w(" : ");
    g.pexpr();
}

fn array(g: &mut Gen) {
    g.w("[");
    let n = g.rng.range(0, 4);
    for i in 0..n {
        if i > 0 {
            g.w(", ");
        }
        match g.rng.below(12) {
            0 => {
                g.w("...");
                g.pexpr();
            }
            1 => {} // hole
            _ => g.expr(),
        }
    }
    if n > 0 && g.rng.chance(1, 8) {
        g.w(",");
    }
    g.w("]");
}

/// The property name a non-computed object-literal key denotes (`'a-b'` -> `a-b`,
/// `0x10` -> `16`), for duplicate detection.
fn key_name(k: &str) -> String {
    match k {
        "'quoted'" => "quoted".into(),
        "'a-b'" => "a-b".into(),
        "'\\u0041'" => "A".into(),
        "1e3" => "1000".into(),
        "0x10" => "16".into(),
        _ => k.trim_matches('\'').to_string(),
    }
}

/// Claims key `k` in `keys`, or returns a fresh key when `k` is already taken.
fn claim(g: &mut Gen, keys: &mut Vec<String>, k: &str) -> String {
    let name = key_name(k);
    if keys.contains(&name) {
        let f = g.fresh("k");
        keys.push(f.clone());
        f
    } else {
        keys.push(name);
        k.to_string()
    }
}

/// Object literal: data, shorthand, computed, quoted/numeric keys, methods (plain, generator,
/// async, async generator), getters/setters, spread, `__proto__` and `super` in methods.
/// Every non-computed key occurs at most once (a get/set pair counts as one key): Closure
/// rejects a duplicate member (JSC_DUPLICATE_MEMBER), which stops every non-WHITESPACE
/// compile before optimization. A key that is already taken is replaced by
/// a fresh name.
fn object(g: &mut Gen) {
    g.w("{");
    let n = g.rng.range(0, 4);
    let mut proto = false;
    let mut keys: Vec<String> = vec![];
    for i in 0..n {
        if i > 0 {
            g.w(", ");
        }
        match g.rng.below(12) {
            0 => {
                g.w("[");
                g.expr();
                g.w("]: ");
                g.expr();
            }
            1 => {
                let p = prop(g);
                let p = claim(g, &mut keys, p);
                g.w(&p);
                g.w("() { return ");
                g.with_ctx(|c| util::enter_fn(c, false, false), |g| g.expr());
                g.w("; }");
            }
            2 => {
                let p = *g
                    .rng
                    .pick(&["'quoted'", "1", "'a-b'", "0.5", "1e3", "0x10", "'\\u0041'"]);
                let p = claim(g, &mut keys, p);
                g.w(&p);
                g.w(": ");
                g.expr();
            }
            3 => match g.visible(|b| !matches!(b, Binding::Class)) {
                Some(v) if !keys.contains(&v) => {
                    keys.push(v.clone());
                    g.w(&v);
                }
                Some(v) => {
                    // Already a key: a fresh key with the binding as its value.
                    let k = claim(g, &mut keys, &v);
                    g.w(&format!("{k}: {v}"));
                }
                None => {
                    let k = claim(g, &mut keys, "x");
                    g.w(&format!("{k}: 0"));
                }
            },
            4 => {
                g.w("...");
                g.pexpr();
            }
            5 => {
                let p = prop(g);
                let p = claim(g, &mut keys, p);
                let v = g.fresh("v");
                g.w(&format!("get {p}() {{ return "));
                g.with_ctx(|c| util::enter_fn(c, false, false), |g| g.expr());
                g.w(&format!("; }}, set {p}({v}) {{ this._{p} = {v}; }}"));
            }
            6 => {
                let (kw, gen_, asy) = *g.rng.pick(&[
                    ("*", true, false),
                    ("async ", false, true),
                    ("async *", true, true),
                ]);
                let m = g.fresh("m");
                keys.push(m.clone());
                g.w(&format!("{kw}{m}() "));
                g.with_ctx(|c| util::enter_fn(c, gen_, asy), |g| g.block(2));
            }
            7 => {
                let k = claim(g, &mut keys, "toString");
                if dialect::low_target() {
                    // ES5 output cannot transpile `super` in object-literal methods
                    // (JSC_CANNOT_CONVERT); low_target also covers ES2015.
                    g.w(&format!(
                        "{k}() {{ return 'o:' + Object.prototype.toString.call(this); }}"
                    ));
                } else {
                    g.w(&format!("{k}() {{ return 'o:' + super.toString(); }}"));
                }
            }
            8 if !proto => {
                proto = true;
                g.w("__proto__: ");
                let s = *g.rng.pick(&["null", "Object.prototype", "{}"]);
                g.w(s);
            }
            _ => {
                let p = prop(g);
                let p = claim(g, &mut keys, p);
                g.w(&p);
                g.w(": ");
                g.expr();
            }
        }
    }
    g.w("}");
}

fn member(g: &mut Gen) {
    if g.rng.chance(1, 2) {
        util::reference(g);
    } else {
        g.pexpr();
    }
    if g.rng.chance(1, 3) {
        g.w("[");
        g.expr();
        g.w("]");
    } else {
        let p = prop(g);
        g.w(".");
        g.w(p);
    }
}

/// Optional chains: `a?.b`, `a?.[k]`, `a?.b.c`, `a?.m?.()`, `a?.b?.c`.
fn optional_chain(g: &mut Gen) {
    util::reference(g);
    match g.rng.below(5) {
        0 => {
            let p = prop(g);
            g.w(&format!("?.{p}"));
        }
        1 => {
            g.w("?.[");
            g.expr();
            g.w("]");
        }
        2 => {
            let (p, q) = (prop(g), prop(g));
            g.w(&format!("?.{p}.{q}"));
        }
        3 => {
            g.w("?.toString?.()");
        }
        _ => {
            let (p, q) = (prop(g), prop(g));
            g.w(&format!("?.{p}?.{q}"));
        }
    }
}

fn args(g: &mut Gen, max: u32) {
    g.w("(");
    let n = g.rng.range(0, max);
    for i in 0..n {
        if i > 0 {
            g.w(", ");
        }
        if g.rng.chance(1, 10) {
            g.w("...");
            if g.rng.chance(1, 2) {
                g.w("[");
                g.expr();
                g.w("]");
            } else {
                g.pexpr();
            }
        } else {
            g.expr();
        }
    }
    g.w(")");
}

/// Calls of visible functions (the main way programs use what they declare).
fn call(g: &mut Gen) {
    let f = g
        .visible(|b| b == Binding::Function)
        .unwrap_or_else(|| "String".to_string());
    g.w(&f);
    match g.rng.below(10) {
        0 => {
            g.w("?.");
            args(g, 2);
        }
        1 => {
            g.w(".call(null");
            if g.rng.chance(1, 2) {
                g.w(", ");
                g.expr();
            }
            g.w(")");
        }
        2 => {
            g.w(".apply(null, [");
            g.expr();
            g.w("])");
        }
        _ => args(g, 3),
    }
}

/// Library calls, many of them foldable by the peephole passes.
fn method_call(g: &mut Gen) {
    const M1: &[&str] = &[
        "Math.max",
        "Math.min",
        "Math.floor",
        "Math.ceil",
        "Math.abs",
        "Math.round",
        "Math.sign",
        "String",
        "Number",
        "Boolean",
        "parseInt",
        "parseFloat",
        "isNaN",
        "isFinite",
        "Object.keys",
        "Object.freeze",
        "Array.isArray",
        "Array.from",
        "JSON.stringify",
        "Symbol",
        "Promise.resolve",
        "BigInt.asIntN.bind(null, 8)",
        "encodeURIComponent",
    ];
    let m = *g.rng.pick(M1);
    g.w(m);
    g.w("(");
    g.expr();
    g.w(")");
}

/// Methods on values: `x.toString()`, `[..].map(cb)`, `'..'.slice(1)`.
fn value_method(g: &mut Gen) {
    match g.rng.below(6) {
        0 => {
            g.w("[");
            g.expr();
            g.w(", ");
            g.expr();
            let m = *g
                .rng
                .pick(&["map", "filter", "some", "every", "find", "forEach"]);
            let a = g.fresh("a");
            g.w(&format!("].{m}(({a}) => "));
            g.push_scope();
            g.declare(&a, Binding::Param);
            g.with_ctx(|c| util::enter_fn(c, false, false), |g| g.pexpr());
            g.pop_scope();
            g.w(")");
        }
        1 => {
            util::reference(g);
            let m = *g
                .rng
                .pick(&[".toString()", ".valueOf()", "?.toString()", ".constructor"]);
            g.w(m);
        }
        2 => {
            g.pexpr();
            let m = *g.rng.pick(&[
                ".toString()",
                ".toString(16)",
                ".valueOf()",
                ".hasOwnProperty('x')",
            ]);
            g.w(m);
        }
        3 => {
            super::literal::string_lit(g);
            let m = *g.rng.pick(&[
                ".length",
                ".charAt(0)",
                ".charCodeAt(1)",
                ".indexOf('a')",
                ".slice(1)",
                ".substring(0, 2)",
                ".toUpperCase()",
                ".split('')",
                ".concat('x')",
                ".trim()",
                ".codePointAt(0)",
                ".at(-1)",
            ]);
            g.w(m);
        }
        4 => {
            g.w("[");
            g.expr();
            g.w("]");
            let m = *g.rng.pick(&[
                ".length",
                ".join('-')",
                ".concat([1])",
                ".indexOf(1)",
                ".slice(0)",
                ".reverse()",
                ".flat()",
                ".includes(0)",
                "[0]",
            ]);
            g.w(m);
        }
        _ => {
            let c = *g.rng.pick(&[
                "new Map([[1, 'a']])",
                "new Set([1, 2, 2])",
                "new Date(0)",
                "new Error('e')",
                "new Array(3)",
                "new RegExp('a', 'g')",
                "new Uint8Array(4)",
                "new WeakMap()",
                "Symbol.iterator",
                "Object.create(null)",
            ]);
            g.w(c);
        }
    }
}

fn assign(g: &mut Gen) {
    match util::mutable(g) {
        Some(n) => {
            let op = *g.rng.pick(ASSIGN_OPS);
            g.w(&n);
            g.w(" ");
            g.w(op);
            g.w(" ");
            g.pexpr();
        }
        None => g.w("0"),
    }
}

/// Assignment to a member: `o.x = e`, `o[k] ??= e`, `o.y++`.
fn member_assign(g: &mut Gen) {
    match g.visible(|b| !matches!(b, Binding::Class)) {
        Some(n) => {
            g.w(&n);
            if g.rng.chance(1, 3) {
                g.w("[");
                g.expr();
                g.w("]");
            } else {
                let p = prop(g);
                g.w(".");
                g.w(p);
            }
            if g.rng.chance(1, 5) {
                g.w("++");
            } else {
                let op = *g.rng.pick(ASSIGN_OPS);
                g.w(" ");
                g.w(op);
                g.w(" ");
                g.pexpr();
            }
        }
        None => g.w("({}).x = 1"),
    }
}

fn update(g: &mut Gen) {
    match util::mutable(g) {
        Some(n) => {
            let pre = g.rng.chance(1, 2);
            let op = if g.rng.chance(1, 2) { "++" } else { "--" };
            if pre {
                g.w(op);
                g.w(&n);
            } else {
                g.w(&n);
                g.w(op);
            }
        }
        None => g.w("1"),
    }
}

fn new_expr(g: &mut Gen) {
    match g.visible(|b| b == Binding::Class || b == Binding::Function) {
        Some(c) if g.rng.chance(3, 4) => {
            g.w("new ");
            g.w(&c);
            if g.rng.chance(1, 5) {
                g.w("()");
            } else {
                args(g, 2);
            }
        }
        _ => {
            let c = *g
                .rng
                .pick(&["new Object()", "new Array(2)", "new Map", "new Set()"]);
            g.w(c);
        }
    }
}

fn this_expr(g: &mut Gen) {
    let s = *g.rng.pick(&["this", "globalThis", "this?.x"]);
    g.w(s);
}

fn yield_expr(g: &mut Gen) {
    match g.rng.below(4) {
        0 => g.w("yield"),
        1 => {
            g.w("yield* [");
            g.expr();
            g.w("]");
        }
        _ => {
            g.w("yield ");
            g.pexpr();
        }
    }
}

fn await_expr(g: &mut Gen) {
    g.w("await ");
    if g.rng.chance(1, 3) {
        g.w("Promise.resolve(");
        g.expr();
        g.w(")");
    } else {
        g.pexpr();
    }
}

/// Constant-foldable shapes (PeepholeFoldConstants, PeepholeReplaceKnownMethods, ...).
const FOLD: &[&str] = &[
    "$n + $n",
    "$n * $n - $n",
    "$n / $n",
    "$n % $n",
    "$i ** $i",
    "1 << $i",
    "-1 >>> $i",
    "$n & $n",
    "$n | 0",
    "~$i",
    "0.1 + 0.2",
    "1 / 0",
    "-0 * 1",
    "$s + $s",
    "$s + $n",
    "$n + $s",
    "'abc'.length",
    "$s.length",
    "'abc'.charAt($i)",
    "'abc'.charCodeAt(0)",
    "'a,b,c'.split(',')",
    "'abc'.indexOf('b')",
    "'abcdef'.substring(1, 3)",
    "'ABC'.toLowerCase()",
    "[1, 2, 3].length",
    "[$n, $n].join('-')",
    "[1, 2, 3][$i]",
    "({a: $n, b: $s}).a",
    "typeof $n",
    "typeof $s",
    "typeof null",
    "typeof undefined",
    "typeof $r",
    "typeof $r === 'undefined'",
    "void 0",
    "!0",
    "!1",
    "!!$s",
    "-(-$i)",
    "true && $s",
    "null ?? $n",
    "0 || $s",
    "$n == $s",
    "null == undefined",
    "NaN === NaN",
    "'b' > 'a'",
    "[] + []",
    "Math.max($n, $n)",
    "Math.floor($n)",
    "Math.abs(-$i)",
    "Math.pow(2, $i)",
    "parseInt('12', 10)",
    "parseFloat('1.5')",
    "String($n)",
    "Number($s)",
    "Boolean($i)",
    "String.fromCharCode(65, 66)",
    "$r + 0",
    "$r * 1",
    "'' + $r",
    "$r | 0",
    "$r === $r",
    "true ? $r : $n",
    "false ? $n : $r",
    "(0, $r)",
    "$r && $r",
    "$r || $n",
    "$r ?? $s",
    "1 + 2 + $r",
    "$r + 1 + 2",
    "($i, $s, $r)",
    "`${$n}`",
    "`a${$s}b`",
];

fn fold(g: &mut Gen) {
    let t = *g.rng.pick(FOLD);
    util::expand(g, t);
}

/// Destructuring assignment: `[a, b] = [b, a]`, `({x: a, y: b = 1} = o)`.
fn destructuring_assign(g: &mut Gen) {
    let a = util::mutable(g);
    let b = util::mutable(g);
    match (a, b) {
        (Some(a), Some(b)) if a != b => match g.rng.below(4) {
            0 => g.w(&format!("[{a}, {b}] = [{b}, {a}]")),
            1 => {
                g.w(&format!("({{x: {a}, y: {b} = 1}} = {{x: "));
                g.expr();
                g.w("})");
            }
            2 => g.w(&format!("[{a}, , ...{b}] = [1, 2, 3, 4]")),
            _ => {
                g.w(&format!("({{{a}, ...{b}}} = {{{a}: "));
                g.expr();
                g.w(", q: 1})");
            }
        },
        (Some(a), _) => {
            g.w(&format!("[{a} = 2] = []"));
        }
        _ => g.w("[] = []"),
    }
}

pub static EXPRESSIONS: &[Production] = &[
    Production {
        name: "reference",
        weight: 34,
        leaf: true,
        when: always,
        emit: reference,
    },
    Production {
        name: "binary",
        weight: 22,
        leaf: false,
        when: always,
        emit: binary,
    },
    Production {
        name: "unary",
        weight: 7,
        leaf: false,
        when: always,
        emit: unary,
    },
    Production {
        name: "delete",
        weight: 1,
        leaf: false,
        when: always,
        emit: delete_expr,
    },
    Production {
        name: "conditional",
        weight: 5,
        leaf: false,
        when: always,
        emit: conditional,
    },
    Production {
        name: "array",
        weight: 5,
        leaf: false,
        when: always,
        emit: array,
    },
    Production {
        name: "object",
        weight: 5,
        leaf: false,
        when: always,
        emit: object,
    },
    Production {
        name: "member",
        weight: 8,
        leaf: false,
        when: always,
        emit: member,
    },
    Production {
        name: "optional_chain",
        weight: 3,
        leaf: false,
        when: always,
        emit: optional_chain,
    },
    Production {
        name: "call",
        weight: 14,
        leaf: false,
        when: always,
        emit: call,
    },
    Production {
        name: "method_call",
        weight: 4,
        leaf: false,
        when: always,
        emit: method_call,
    },
    Production {
        name: "value_method",
        weight: 4,
        leaf: false,
        when: always,
        emit: value_method,
    },
    Production {
        name: "assign",
        weight: 8,
        leaf: false,
        when: always,
        emit: assign,
    },
    Production {
        name: "member_assign",
        weight: 3,
        leaf: false,
        when: always,
        emit: member_assign,
    },
    Production {
        name: "destructuring_assign",
        weight: 1,
        leaf: false,
        when: always,
        emit: destructuring_assign,
    },
    Production {
        name: "update",
        weight: 4,
        leaf: true,
        when: always,
        emit: update,
    },
    Production {
        name: "new",
        weight: 3,
        leaf: false,
        when: always,
        emit: new_expr,
    },
    Production {
        name: "this",
        weight: 1,
        leaf: true,
        when: always,
        emit: this_expr,
    },
    Production {
        name: "fold",
        weight: 10,
        leaf: false,
        when: always,
        emit: fold,
    },
    Production {
        name: "yield",
        weight: 4,
        leaf: false,
        when: |c| c.in_generator,
        emit: yield_expr,
    },
    Production {
        name: "await",
        weight: 4,
        leaf: false,
        when: |c| c.in_async,
        emit: await_expr,
    },
];

#[cfg(test)]
mod dup_key_tests {
    use super::{claim, key_name};
    use crate::engine::Gen;
    use crate::rng::Rng;

    #[test]
    fn spellings_of_one_key_collide() {
        assert_eq!(key_name("'quoted'"), "quoted");
        assert_eq!(key_name("0x10"), "16");
        assert_eq!(key_name("1e3"), "1000");
        assert_eq!(key_name("'\\u0041'"), "A");
    }

    #[test]
    fn a_taken_key_is_replaced() {
        let cfg = crate::Config::default();
        let mut g = Gen::new(Rng::new(1), &cfg);
        let mut keys = vec![];
        assert_eq!(claim(&mut g, &mut keys, "toString"), "toString");
        let k = claim(&mut g, &mut keys, "toString");
        assert_ne!(k, "toString");
        assert_eq!(claim(&mut g, &mut keys, "v1"), "v1");
        assert_ne!(claim(&mut g, &mut keys, "v1"), "v1");
        let mut sorted = keys.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), keys.len(), "{keys:?}");
    }
}
