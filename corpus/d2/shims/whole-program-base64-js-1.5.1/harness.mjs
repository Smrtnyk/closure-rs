// closure-rs D2 corpus, collector "whole-program": test harness.
//
// Master copy: corpus/d2/candidates/whole-program.harness.mjs. whole-program.gen.mjs copies it
// unchanged to harness.mjs in every corpus/d2/shims/whole-program-*/ directory; the sha256 of
// each copy is recorded in corpus/d2/candidates/whole-program.lock.json.
//
// It runs a library's own, unmodified test files (fetched into corpus-cache/) against either
//   - a compiled output file (the normal case: `node run.mjs <compiled.js>`), or
//   - the original sources, loaded through the case's shim (`node run.mjs --original`).
//
// Both modes end with the shim's export on globalThis; the test files then import the library
// through virtual modules that read that global. There is no third-party code: the test
// framework APIs the suites use (node:test subset, mocha, tape, uvu, ava, expect.js) are small
// re-implementations on top of one deterministic, sequential executor. The process exits 0 only
// when every test passed and the number of tests equals the case's `expectTests`.
//
// The compiled file is evaluated in this realm as a classic script wrapped in a function
// (`(function(){ ... }).call(globalThis)`), so top-level names of the compiled code cannot
// collide with globals that test files create; the output only communicates through the
// properties it writes to globalThis.

import { registerHooks, createRequire } from 'node:module';
import { pathToFileURL, fileURLToPath } from 'node:url';
import * as path from 'node:path';
import * as fs from 'node:fs';
import * as vm from 'node:vm';
import nodeAssert from 'node:assert';
import { inspect } from 'node:util';

const HARNESS_KEY = Symbol.for('closure-rs.d2.whole-program.harness');
const VIRTUAL = 'closure-rs-wp:';
const DEFAULT_TIMEOUT_MS = 120000;

// ---------------------------------------------------------------------------------------------
// Deep equality used by the tape / uvu / ava adapters.
//   prim: how primitives (and identical references) compare: 'strict' (===), 'is' (Object.is)
//   or 'loose' (==).
//   proto: whether both objects must have the same [[Prototype]] (tape 5, ava) or the same
//   `constructor` (uvu's dequal); 'none' skips the check (tape 4 strict mode).

function makeDeepEqual(prim, proto) {
  const primEq =
    prim === 'is' ? Object.is : prim === 'loose' ? (a, b) => a == b : (a, b) => a === b;
  function eq(a, b, seen) {
    if (primEq(a, b)) return true;
    if (a === null || b === null || typeof a !== 'object' || typeof b !== 'object') {
      return false;
    }
    if (proto === 'proto' && Object.getPrototypeOf(a) !== Object.getPrototypeOf(b)) return false;
    if (proto === 'ctor' && a.constructor !== b.constructor) return false;
    if (Array.isArray(a) !== Array.isArray(b)) return false;
    const pairKey = seen.get(a);
    if (pairKey && pairKey.has(b)) return true;
    if (pairKey) pairKey.add(b);
    else seen.set(a, new Set([b]));
    if (a instanceof Date || b instanceof Date) {
      return a instanceof Date && b instanceof Date && a.getTime() === b.getTime();
    }
    if (a instanceof RegExp || b instanceof RegExp) {
      return a instanceof RegExp && b instanceof RegExp && String(a) === String(b);
    }
    if (ArrayBuffer.isView(a) || ArrayBuffer.isView(b)) {
      if (!ArrayBuffer.isView(a) || !ArrayBuffer.isView(b) || a.length !== b.length) return false;
      for (let i = 0; i < a.length; i++) if (!primEq(a[i], b[i])) return false;
      return true;
    }
    if (a instanceof Map || b instanceof Map) {
      if (!(a instanceof Map && b instanceof Map) || a.size !== b.size) return false;
      for (const [k, v] of a) if (!b.has(k) || !eq(v, b.get(k), seen)) return false;
      return true;
    }
    if (a instanceof Set || b instanceof Set) {
      if (!(a instanceof Set && b instanceof Set) || a.size !== b.size) return false;
      for (const v of a) if (!b.has(v)) return false;
      return true;
    }
    const ka = Object.keys(a);
    const kb = Object.keys(b);
    if (ka.length !== kb.length) return false;
    for (const k of ka) {
      if (!Object.prototype.hasOwnProperty.call(b, k) || !eq(a[k], b[k], seen)) return false;
    }
    return true;
  }
  return (a, b) => eq(a, b, new Map());
}

const show = (v) => inspect(v, { depth: 6, breakLength: 120 });

// ---------------------------------------------------------------------------------------------
// Executor: a tree of suites holding tests and hooks, run sequentially in registration order.

function newSuite(name, parent, skip) {
  return { name, parent, items: [], before: [], after: [], beforeEach: [], afterEach: [], skip };
}

class Executor {
  constructor() {
    this.root = newSuite('', null, false);
    this.current = this.root;
    this.results = [];
    this.currentReject = null;
  }

  suite(name, body, opts = {}) {
    const s = newSuite(String(name), this.current, !!opts.skip || this.current.skip);
    this.current.items.push({ kind: 'suite', suite: s });
    const prev = this.current;
    this.current = s;
    try {
      const r = body ? body.call(opts.thisArg) : undefined;
      if (r && typeof r.then === 'function') {
        throw new Error(`suite "${name}": asynchronous suite bodies are not supported`);
      }
    } finally {
      this.current = prev;
    }
  }

  // `run` is a zero-argument function returning a value or a promise.
  test(name, run, opts = {}) {
    this.current.items.push({
      kind: 'test',
      name: String(name),
      run,
      skip: !!opts.skip || this.current.skip,
      timeout: opts.timeout || DEFAULT_TIMEOUT_MS,
    });
  }

  hook(kind, run) {
    this.current[kind].push(run);
  }

  count(s = this.root) {
    let n = 0;
    for (const it of s.items) n += it.kind === 'suite' ? this.count(it.suite) : 1;
    return n;
  }

  async guarded(run, timeout) {
    let timer;
    try {
      await new Promise((resolve, reject) => {
        this.currentReject = reject;
        timer = setTimeout(() => reject(new Error(`timed out after ${timeout} ms`)), timeout);
        Promise.resolve()
          .then(run)
          .then(resolve, reject);
      });
    } finally {
      clearTimeout(timer);
      this.currentReject = null;
    }
  }

  record(title, status, error) {
    const n = this.results.length + 1;
    this.results.push({ title, status, error });
    if (status === 'pass') console.log(`ok ${n} - ${title}`);
    else if (status === 'skip') console.log(`ok ${n} - ${title} # SKIP`);
    else {
      console.log(`not ok ${n} - ${title}`);
      const text = error && error.stack ? error.stack : String(error);
      console.log(text.split('\n').map((l) => '  # ' + l).join('\n'));
    }
  }

  async runSuite(s, chain, prefix, inheritedFailure) {
    const title = (name) => (prefix ? prefix + ' > ' : '') + name;
    let failure = inheritedFailure;
    if (!failure && !s.skip) {
      for (const h of s.before) {
        try {
          await this.guarded(h, DEFAULT_TIMEOUT_MS);
        } catch (e) {
          failure = e;
          break;
        }
      }
    }
    const fullChain = chain.concat([s]);
    for (const it of s.items) {
      if (it.kind === 'suite') {
        await this.runSuite(it.suite, fullChain, title(it.suite.name), failure);
        continue;
      }
      const t = title(it.name);
      if (failure) {
        this.record(t, 'fail', new Error('a before() hook failed: ' + (failure.stack || failure)));
        continue;
      }
      if (it.skip) {
        this.record(t, 'skip');
        continue;
      }
      let error = null;
      try {
        for (const c of fullChain) for (const h of c.beforeEach) await this.guarded(h, it.timeout);
        await this.guarded(it.run, it.timeout);
      } catch (e) {
        error = e || new Error('test failed with a falsy rejection');
      }
      try {
        for (const c of fullChain.slice().reverse()) {
          for (const h of c.afterEach) await this.guarded(h, it.timeout);
        }
      } catch (e) {
        error = error || e;
      }
      if (error && error.wpSkip) this.record(t, 'skip');
      else this.record(t, error ? 'fail' : 'pass', error);
    }
    if (!s.skip && !inheritedFailure) {
      for (const h of s.after) {
        try {
          await this.guarded(h, DEFAULT_TIMEOUT_MS);
        } catch (e) {
          this.record(title('after() hook'), 'fail', e);
        }
      }
    }
  }

  async run() {
    const onError = (e) => {
      if (this.currentReject) this.currentReject(e);
      else console.log('# uncaught outside of a test: ' + (e && e.stack ? e.stack : e));
    };
    process.on('uncaughtException', onError);
    process.on('unhandledRejection', onError);
    try {
      await this.runSuite(this.root, [], '', null);
    } finally {
      process.off('uncaughtException', onError);
      process.off('unhandledRejection', onError);
    }
    return this.results;
  }
}

const skipSignal = () => Object.assign(new Error('skipped'), { wpSkip: true });

// ---------------------------------------------------------------------------------------------
// node:test subset (describe/it/test/hooks; test functions get a minimal context `t` and, when
// they declare two parameters, a `done` callback).

function makeNodeTest(ex) {
  const parse = (args) => {
    let [name, opts, fn] = args;
    if (typeof name === 'function') [name, opts, fn] = [name.name || '<anonymous>', {}, name];
    if (typeof opts === 'function') [opts, fn] = [{}, opts];
    return { name, opts: opts || {}, fn };
  };
  const makeIt = (extra) =>
    function it(...args) {
      const { name, opts, fn } = parse(args);
      const skip = extra.skip || !!opts.skip || !!opts.todo || !fn;
      ex.test(
        name,
        () => {
          const t = {
            name,
            assert: nodeAssert,
            diagnostic() {},
            skip() {
              throw skipSignal();
            },
            todo() {},
            plan() {},
          };
          if (fn.length >= 2) {
            return new Promise((resolve, reject) => {
              fn.call(t, t, (err) => (err ? reject(err) : resolve()));
            });
          }
          return fn.call(t, t);
        },
        { skip, timeout: opts.timeout },
      );
    };
  const makeDescribe = (extra) =>
    function describe(...args) {
      const { name, opts, fn } = parse(args);
      ex.suite(name, fn, { skip: extra.skip || !!opts.skip || !!opts.todo });
    };
  const it = Object.assign(makeIt({}), {
    skip: makeIt({ skip: true }),
    todo: makeIt({ skip: true }),
    only: makeIt({}),
  });
  const describe = Object.assign(makeDescribe({}), {
    skip: makeDescribe({ skip: true }),
    todo: makeDescribe({ skip: true }),
    only: makeDescribe({}),
  });
  const hook = (kind) => (fn) =>
    ex.hook(kind, () =>
      fn.length >= 2
        ? new Promise((res, rej) => fn({}, (e) => (e ? rej(e) : res())))
        : fn({}),
    );
  return {
    describe,
    suite: describe,
    it,
    test: it,
    before: hook('before'),
    after: hook('after'),
    beforeEach: hook('beforeEach'),
    afterEach: hook('afterEach'),
  };
}

// ---------------------------------------------------------------------------------------------
// mocha (BDD globals). Test functions with a parameter receive mocha's `done` callback.

function makeMocha(ex) {
  const ctx = {
    timeout() {
      return ctx;
    },
    slow() {
      return ctx;
    },
    retries() {
      return ctx;
    },
    skip() {
      throw skipSignal();
    },
  };
  const call = (fn) =>
    fn.length >= 1
      ? new Promise((resolve, reject) => {
          fn.call(ctx, (err) => (err ? reject(err) : resolve()));
        })
      : fn.call(ctx);
  const makeIt = (skip) =>
    function it(name, fn) {
      ex.test(name, () => call(fn), { skip: skip || !fn });
    };
  const makeDescribe = (skip) =>
    function describe(name, fn) {
      ex.suite(name, fn, { skip, thisArg: ctx });
    };
  const it = Object.assign(makeIt(false), { skip: makeIt(true), only: makeIt(false) });
  const describe = Object.assign(makeDescribe(false), {
    skip: makeDescribe(true),
    only: makeDescribe(false),
  });
  const hook = (kind) => (name, fn) => ex.hook(kind, () => call(typeof name === 'function' ? name : fn));
  return {
    describe,
    context: describe,
    it,
    specify: it,
    xdescribe: describe.skip,
    xcontext: describe.skip,
    xit: it.skip,
    xspecify: it.skip,
    before: hook('before'),
    after: hook('after'),
    beforeEach: hook('beforeEach'),
    afterEach: hook('afterEach'),
  };
}

// ---------------------------------------------------------------------------------------------
// tape (4.x: equal is ===; 5.x: equal is Object.is). Assertions record failures and the test
// finishes on t.end(), when the plan is reached, or when an async callback settles.

function makeTape(ex, major) {
  const primEq = major >= 5 ? Object.is : (a, b) => a === b;
  const deepStrict = makeDeepEqual(major >= 5 ? 'is' : 'strict', major >= 5 ? 'proto' : 'none');
  const deepLoose = makeDeepEqual('loose', 'none');

  function makeT(name, finish) {
    let count = 0;
    let plan;
    let ended = false;
    const failures = [];
    const done = (err) => {
      if (ended) return;
      ended = true;
      if (err) failures.push('error: ' + (err.stack || err));
      if (plan !== undefined && count !== plan) failures.push(`plan != count: ${plan} != ${count}`);
      finish(failures);
    };
    const ok = (cond, msg, detail) => {
      if (ended) {
        failures.push(`assertion after end: ${msg || ''}`);
        return;
      }
      count++;
      if (!cond) failures.push(`#${count} ${msg || 'assertion'}${detail ? ': ' + detail : ''}`);
      if (plan !== undefined && count === plan) setImmediate(() => done());
    };
    const t = {
      plan(n) {
        plan = n;
      },
      end(err) {
        if (err) failures.push('end(err): ' + err);
        done();
      },
      comment() {},
      timeoutAfter() {},
      skip() {},
      pass(msg) {
        ok(true, msg);
      },
      fail(msg) {
        ok(false, msg || 'fail');
      },
      ok(v, msg) {
        ok(!!v, msg || 'should be truthy', show(v));
      },
      notOk(v, msg) {
        ok(!v, msg || 'should be falsy', show(v));
      },
      error(err, msg) {
        ok(!err, msg || String(err), show(err));
      },
      equal(a, b, msg) {
        ok(primEq(a, b), msg || 'should be equal', `${show(a)} vs ${show(b)}`);
      },
      notEqual(a, b, msg) {
        ok(!primEq(a, b), msg || 'should not be equal', `${show(a)} vs ${show(b)}`);
      },
      looseEqual(a, b, msg) {
        ok(a == b, msg || 'should be loosely equal', `${show(a)} vs ${show(b)}`);
      },
      notLooseEqual(a, b, msg) {
        ok(a != b, msg || 'should not be loosely equal', `${show(a)} vs ${show(b)}`);
      },
      deepEqual(a, b, msg) {
        ok(deepStrict(a, b), msg || 'should be deeply equal', `${show(a)} vs ${show(b)}`);
      },
      notDeepEqual(a, b, msg) {
        ok(!deepStrict(a, b), msg || 'should not be deeply equal', `${show(a)} vs ${show(b)}`);
      },
      deepLooseEqual(a, b, msg) {
        ok(deepLoose(a, b), msg || 'should be loosely deeply equal', `${show(a)} vs ${show(b)}`);
      },
      throws(fn, expected, msg) {
        if (typeof expected === 'string') [expected, msg] = [undefined, expected];
        let caught = null;
        try {
          fn();
        } catch (e) {
          caught = { error: e };
        }
        let passed = !!caught;
        if (caught && expected instanceof RegExp) passed = expected.test(String(caught.error));
        else if (caught && typeof expected === 'function' && expected.prototype !== undefined) {
          passed = caught.error instanceof expected;
        } else if (caught && typeof expected === 'function') passed = !!expected(caught.error);
        ok(passed, msg || 'should throw', caught ? show(caught.error) : 'did not throw');
      },
      doesNotThrow(fn, expected, msg) {
        if (typeof expected === 'string') msg = expected;
        let caught = null;
        try {
          fn();
        } catch (e) {
          caught = e;
        }
        ok(!caught, msg || 'should not throw', caught ? show(caught) : '');
      },
      match(s, re, msg) {
        ok(re.test(s), msg || 'should match', `${show(s)} vs ${re}`);
      },
      doesNotMatch(s, re, msg) {
        ok(!re.test(s), msg || 'should not match', `${show(s)} vs ${re}`);
      },
      _autoEnd() {
        if (plan === undefined) done();
      },
      _fail(e) {
        failures.push('error: ' + (e && e.stack ? e.stack : e));
        done();
      },
    };
    const aliases = {
      true: 'ok',
      assert: 'ok',
      false: 'notOk',
      notok: 'notOk',
      ifError: 'error',
      ifErr: 'error',
      iferror: 'error',
      equals: 'equal',
      isEqual: 'equal',
      is: 'equal',
      strictEqual: 'equal',
      strictEquals: 'equal',
      notEquals: 'notEqual',
      isNotEqual: 'notEqual',
      doesNotEqual: 'notEqual',
      isInequal: 'notEqual',
      notStrictEqual: 'notEqual',
      notStrictEquals: 'notEqual',
      isNot: 'notEqual',
      not: 'notEqual',
      looseEquals: 'looseEqual',
      notLooseEquals: 'notLooseEqual',
      deepEquals: 'deepEqual',
      isEquivalent: 'deepEqual',
      same: 'deepEqual',
      notDeepEquals: 'notDeepEqual',
      notEquivalent: 'notDeepEqual',
      notDeeply: 'notDeepEqual',
      notSame: 'notDeepEqual',
      isNotDeepEqual: 'notDeepEqual',
      isNotDeeply: 'notDeepEqual',
      isNotEquivalent: 'notDeepEqual',
      isInequivalent: 'notDeepEqual',
      throw: 'throws',
    };
    for (const [alias, target] of Object.entries(aliases)) t[alias] = t[target];
    return t;
  }

  const register = (skip) =>
    function test(name, opts, cb) {
      if (typeof name === 'function') [name, opts, cb] = [name.name || '(anonymous)', {}, name];
      if (typeof opts === 'function') [opts, cb] = [{}, opts];
      ex.test(
        name,
        () =>
          new Promise((resolve, reject) => {
            const t = makeT(name, (failures) =>
              failures.length ? reject(new Error(failures.join('\n'))) : resolve(),
            );
            try {
              const r = cb(t);
              if (r && typeof r.then === 'function') r.then(() => t._autoEnd(), (e) => t._fail(e));
            } catch (e) {
              t._fail(e);
            }
          }),
        { skip: skip || !!(opts && opts.skip) || !cb },
      );
    };
  return Object.assign(register(false), { skip: register(true), only: register(false) });
}

// ---------------------------------------------------------------------------------------------
// uvu: suites register their tests when `.run()` is called; assertions throw.

function makeUvu(ex) {
  const dequal = makeDeepEqual('strict', 'ctor');
  class Assertion extends Error {}
  const check = (cond, msg, detail) => {
    if (!cond) throw new Assertion((msg || 'assertion failed') + (detail ? ': ' + detail : ''));
  };
  const assert = {
    Assertion,
    ok: (v, msg) => check(!!v, msg || 'Expected value to be truthy', show(v)),
    is: (a, b, msg) => check(a === b, msg || 'Expected values to be strictly equal', `${show(a)} vs ${show(b)}`),
    equal: (a, b, msg) => check(dequal(a, b), msg || 'Expected values to be deeply equal', `${show(a)} vs ${show(b)}`),
    type: (v, t, msg) => check(typeof v === t, msg || `Expected "${typeof v}" to be "${t}"`),
    instance: (v, c, msg) => check(v instanceof c, msg || `Expected value to be an instance of ${c && c.name}`),
    match: (v, e, msg) =>
      check(typeof e === 'string' ? String(v).includes(e) : e.test(v), msg || 'Expected value to match', `${show(v)} vs ${e}`),
    snapshot: (a, b, msg) => check(a === b, msg || 'Expected value to match snapshot'),
    fixture: (a, b, msg) => check(a === b, msg || 'Expected value to match fixture'),
    throws: (fn, exp, msg) => {
      let caught = null;
      try {
        fn();
      } catch (e) {
        caught = { e };
      }
      check(!!caught, msg || 'Expected function to throw');
      if (exp instanceof RegExp) check(exp.test(caught.e.message), msg || 'Expected error message to match');
      else if (typeof exp === 'function') check(!!exp(caught.e), msg || 'Expected function to throw matching exception');
    },
    unreachable: (msg) => check(false, msg || 'Expected not to be reached!'),
  };
  assert.not = (v, msg) => check(!v, msg || 'Expected value to be falsey', show(v));
  assert.not.ok = assert.not;
  assert.not.equal = (a, b, msg) => check(!dequal(a, b), msg || 'Expected values not to be deeply equal');
  assert.not.type = (v, t, msg) => check(typeof v !== t, msg || `Expected "${typeof v}" not to be "${t}"`);
  assert.not.instance = (v, c, msg) => check(!(v instanceof c), msg || 'Expected value not to be an instance');
  assert.not.match = (v, e, msg) =>
    check(!(typeof e === 'string' ? String(v).includes(e) : e.test(v)), msg || 'Expected value not to match');
  assert.not.snapshot = (a, b, msg) => check(a !== b, msg || 'Expected value not to match snapshot');
  assert.not.fixture = (a, b, msg) => check(a !== b, msg || 'Expected value not to match fixture');
  assert.not.throws = (fn, exp, msg) => {
    try {
      fn();
    } catch (e) {
      check(false, msg || 'Expected function not to throw', show(e));
    }
  };
  assert.is.not = (a, b, msg) => check(a !== b, msg || 'Expected values not to be strictly equal');

  // Like uvu, `run()` queues what was registered so far and resets the suite, so one suite
  // object (notably the shared default `test`) can be reused by several test files.
  function suite(name = '', state = {}) {
    let tests = [];
    let hooks = { before: [], after: [], beforeEach: [], afterEach: [] };
    const add = (skip) => (tname, fn) => tests.push({ tname, fn, skip });
    const t = Object.assign(add(false), { skip: add(true), only: add(false) });
    t.before = (fn) => hooks.before.push(fn);
    t.after = (fn) => hooks.after.push(fn);
    t.before.each = (fn) => hooks.beforeEach.push(fn);
    t.after.each = (fn) => hooks.afterEach.push(fn);
    t.run = () => {
      const [queued, queuedHooks] = [tests, hooks];
      tests = [];
      hooks = { before: [], after: [], beforeEach: [], afterEach: [] };
      const body = () => {
        for (const h of ['before', 'after', 'beforeEach', 'afterEach']) {
          for (const fn of queuedHooks[h]) ex.hook(h, () => fn(state));
        }
        for (const { tname, fn, skip } of queued) ex.test(tname, () => fn(state), { skip });
      };
      ex.suite(name, body);
    };
    return t;
  }
  return { uvu: { test: suite(), suite }, assert };
}

// ---------------------------------------------------------------------------------------------
// ava: assertions record failures (tests continue); a test without assertions fails
// (AVA's default failWithoutAssertions).

function makeAva(ex) {
  const deep = makeDeepEqual('is', 'proto');
  const like = (actual, selector) => {
    if (selector === null || typeof selector !== 'object') return deep(actual, selector);
    if (actual === null || typeof actual !== 'object') return false;
    return Object.keys(selector).every((k) => like(actual[k], selector[k]));
  };
  function makeT(title) {
    const state = { count: 0, failures: [], plan: undefined, teardowns: [] };
    const ok = (cond, msg, detail) => {
      state.count++;
      if (!cond) state.failures.push(`${msg}${detail ? ': ' + detail : ''}`);
      return !!cond;
    };
    const matchError = (err, exp) => {
      if (!exp) return true;
      if (exp.any !== true && !(err instanceof Error)) return false;
      if (exp.instanceOf && !(err instanceof exp.instanceOf)) return false;
      if (exp.is !== undefined && err !== exp.is) return false;
      if (exp.name !== undefined && err.name !== exp.name) return false;
      if (exp.code !== undefined && err.code !== exp.code) return false;
      if (typeof exp.message === 'string' && err.message !== exp.message) return false;
      if (exp.message instanceof RegExp && !exp.message.test(err.message)) return false;
      if (typeof exp.message === 'function' && !exp.message(err.message)) return false;
      return true;
    };
    const t = {
      title,
      context: {},
      is: (a, b, m) => ok(Object.is(a, b), m || 'Values are not the same', `${show(a)} vs ${show(b)}`),
      not: (a, b, m) => ok(!Object.is(a, b), m || 'Values are the same', show(a)),
      deepEqual: (a, b, m) => ok(deep(a, b), m || 'Values are not deeply equal', `${show(a)} vs ${show(b)}`),
      notDeepEqual: (a, b, m) => ok(!deep(a, b), m || 'Values are deeply equal', show(a)),
      like: (a, s, m) => ok(like(a, s), m || 'Value is not like selector', show(a)),
      true: (v, m) => ok(v === true, m || 'Value is not `true`', show(v)),
      false: (v, m) => ok(v === false, m || 'Value is not `false`', show(v)),
      truthy: (v, m) => ok(!!v, m || 'Value is not truthy', show(v)),
      falsy: (v, m) => ok(!v, m || 'Value is not falsy', show(v)),
      assert: (v, m) => ok(!!v, m || 'Assertion failed', show(v)),
      regex: (s, re, m) => ok(re.test(s), m || 'String does not match', `${show(s)} vs ${re}`),
      notRegex: (s, re, m) => ok(!re.test(s), m || 'String matches', `${show(s)} vs ${re}`),
      pass: () => ok(true),
      fail: (m) => ok(false, m || 'Test failed via `t.fail()`'),
      throws(fn, exp, m) {
        try {
          fn();
        } catch (e) {
          ok(matchError(e, exp), m || 'Function threw unexpected exception', show(e));
          return e;
        }
        ok(false, m || 'Function did not throw');
        return undefined;
      },
      notThrows(fn, m) {
        try {
          fn();
          ok(true);
        } catch (e) {
          ok(false, m || 'Function threw', show(e));
        }
      },
      async throwsAsync(fn, exp, m) {
        try {
          await (typeof fn === 'function' ? fn() : fn);
        } catch (e) {
          ok(matchError(e, exp), m || 'Promise rejected with unexpected exception', show(e));
          return e;
        }
        ok(false, m || 'Promise resolved');
        return undefined;
      },
      async notThrowsAsync(fn, m) {
        try {
          await (typeof fn === 'function' ? fn() : fn);
          ok(true);
        } catch (e) {
          ok(false, m || 'Promise rejected', show(e));
        }
      },
      snapshot: () => ok(false, 't.snapshot() is not supported by the closure-rs harness'),
      plan: (n) => {
        state.plan = n;
      },
      log() {},
      timeout() {},
      teardown: (fn) => state.teardowns.push(fn),
    };
    return { t, state };
  }
  const register = (mode) =>
    function test(title, impl, ...args) {
      if (typeof title === 'function') [title, impl, args] = ['', title, [impl, ...args]];
      const impls = Array.isArray(impl) ? impl : [impl];
      for (const fn of impls) {
        const name = fn && typeof fn.title === 'function' ? fn.title(title, ...args) : title;
        ex.test(
          name,
          async () => {
            const { t, state } = makeT(name);
            let err = null;
            try {
              await fn(t, ...args);
            } catch (e) {
              err = e;
            }
            for (const td of state.teardowns.reverse()) await td();
            const problems = state.failures.slice();
            if (err) problems.push('Error thrown in test: ' + (err.stack || err));
            if (state.plan !== undefined && state.plan !== state.count) {
              problems.push(`Planned for ${state.plan} assertions, but got ${state.count}`);
            }
            if (!err && state.count === 0) problems.push('Test finished without running any assertions');
            if (mode === 'failing') {
              if (!problems.length) throw new Error('Test was expected to fail, but succeeded');
              return;
            }
            if (problems.length) throw new Error(problems.join('\n'));
          },
          { skip: mode === 'skip' || mode === 'todo' || !fn },
        );
      }
    };
  const test = register('normal');
  Object.assign(test, {
    serial: Object.assign(register('normal'), { skip: register('skip'), only: register('normal') }),
    skip: register('skip'),
    only: register('normal'),
    failing: register('failing'),
    todo: (title) => register('todo')(title, undefined),
    before: (fn) => ex.hook('before', () => fn({ context: {} })),
    after: Object.assign((fn) => ex.hook('after', () => fn({ context: {} })), {
      always: (fn) => ex.hook('after', () => fn({ context: {} })),
    }),
    beforeEach: (fn) => ex.hook('beforeEach', () => fn({ context: {} })),
    afterEach: Object.assign((fn) => ex.hook('afterEach', () => fn({ context: {} })), {
      always: (fn) => ex.hook('afterEach', () => fn({ context: {} })),
    }),
    macro: (m) => m,
  });
  return test;
}

// ---------------------------------------------------------------------------------------------
// expect.js subset: to.be / to.equal / to.eql / to.be.a / to.throwError, with not.

function makeExpect() {
  const deep = makeDeepEqual('loose', 'none');
  function expect(actual) {
    const chain = (negate) => {
      const check = (cond, msg) => {
        if (negate ? cond : !cond) throw new Error(`expected ${show(actual)} ${negate ? 'not ' : ''}${msg}`);
      };
      const be = (v) => check(actual === v, `to equal ${show(v)}`);
      be.a = be.an = (type) =>
        check(typeof type === 'string' ? typeof actual === type || (type === 'array' && Array.isArray(actual)) : actual instanceof type, `to be a ${type}`);
      be.ok = () => check(!!actual, 'to be truthy');
      be.empty = () => check(actual.length === 0, 'to be empty');
      const api = {
        be,
        equal: be,
        eql: (v) => check(deep(actual, v), `to sort of equal ${show(v)}`),
        a: be.a,
        an: be.a,
        ok: be.ok,
        contain: (v) => check(actual.indexOf(v) !== -1, `to contain ${show(v)}`),
        match: (re) => check(re.test(actual), `to match ${re}`),
        throwError: (fn) => {
          let thrown = false;
          let error;
          try {
            actual();
          } catch (e) {
            thrown = true;
            error = e;
          }
          if (negate) check(thrown, 'to throw an error');
          else {
            check(thrown, 'to throw an error');
            if (typeof fn === 'function') fn(error);
            else if (fn instanceof RegExp) check(fn.test(error.message), `to throw ${fn}`);
          }
        },
      };
      api.throwException = api.throwError;
      api.be.within = (a, b) => check(actual >= a && actual <= b, `to be within ${a}..${b}`);
      return api;
    };
    const pos = chain(false);
    const neg = chain(true);
    pos.not = neg;
    neg.not = pos;
    return { to: Object.assign(pos, { not: neg }), not: { to: neg, be: neg.be }, be: pos.be };
  }
  return expect;
}

// ---------------------------------------------------------------------------------------------
// Module redirection: specifiers written in test files are mapped to virtual modules that read
// the library's global (`lib:`) or a framework adapter (`fw:`).

const IDENT = /^[A-Za-z_$][A-Za-z0-9_$]*$/;

function libValue(state, spec) {
  let v = globalThis[state.cfg.global];
  for (const k of spec.path || []) v = v == null ? v : v[k];
  return v;
}

function installHooks(state) {
  const testDirUrl = pathToFileURL(state.testDir + path.sep).href;
  registerHooks({
    resolve(specifier, context, nextResolve) {
      const parent = context.parentURL || '';
      const fromTests = parent.startsWith(testDirUrl) || parent.startsWith(VIRTUAL);
      if (fromTests) {
        const fmt = context.conditions && context.conditions.includes('require') ? 'cjs' : 'esm';
        if (Object.prototype.hasOwnProperty.call(state.cfg.modules, specifier)) {
          return { url: `${VIRTUAL}lib/${fmt}/${encodeURIComponent(specifier)}`, format: fmt === 'cjs' ? 'commonjs' : 'module', shortCircuit: true };
        }
        if (Object.prototype.hasOwnProperty.call(state.frameworks, specifier)) {
          return { url: `${VIRTUAL}fw/${fmt}/${encodeURIComponent(specifier)}`, format: fmt === 'cjs' ? 'commonjs' : 'module', shortCircuit: true };
        }
      }
      return nextResolve(specifier, context);
    },
    load(url, context, nextLoad) {
      if (!url.startsWith(VIRTUAL)) {
        // Test files in a repository whose package.json has no "type" but that use ES module
        // syntax: name the format, as Node's own syntax detection would (minus its warning).
        if (url.startsWith(testDirUrl) && url.endsWith('.js') && context.format !== 'module' && isEsm(fileURLToPath(url))) {
          return nextLoad(url, { ...context, format: 'module' });
        }
        return nextLoad(url, context);
      }
      const [kind, fmt, encoded] = url.slice(VIRTUAL.length).split('/');
      const specifier = decodeURIComponent(encoded);
      const access =
        kind === 'lib'
          ? `globalThis[Symbol.for(${JSON.stringify(HARNESS_KEY.description)})].lib(${JSON.stringify(specifier)})`
          : `globalThis[Symbol.for(${JSON.stringify(HARNESS_KEY.description)})].frameworks[${JSON.stringify(specifier)}]`;
      if (fmt === 'cjs') {
        return { format: 'commonjs', source: `module.exports = ${access};`, shortCircuit: true };
      }
      const value = kind === 'lib' ? libValue(state, state.cfg.modules[specifier]) : state.frameworks[specifier];
      const isNamespace = kind === 'lib' ? !!state.cfg.modules[specifier].namespace : !!(value && value.__esModule);
      const names =
        value !== null && (typeof value === 'object' || typeof value === 'function')
          ? Object.keys(value).filter((k) => IDENT.test(k) && k !== 'default')
          : [];
      let source = `const m = ${access};\n`;
      source += isNamespace ? 'export default m.default;\n' : 'export default m;\n';
      for (const n of names) source += `export const ${n} = m[${JSON.stringify(n)}];\n`;
      return { format: 'module', source, shortCircuit: true };
    },
  });
}

// ---------------------------------------------------------------------------------------------
// Entry point used by every case's run.mjs.

export async function runCase(runUrl, cfg) {
  const caseDir = path.dirname(fileURLToPath(runUrl));
  const repoRoot = path.resolve(caseDir, '..', '..', '..', '..');
  const arg = process.argv[2];
  if (!arg || process.argv.length !== 3) {
    console.error(`usage: node ${path.relative(repoRoot, fileURLToPath(runUrl))} <compiled.js | --original>`);
    process.exit(2);
  }
  const testDir = path.resolve(repoRoot, cfg.testDir);
  if (!fs.existsSync(testDir)) {
    console.error(`missing test files: ${cfg.testDir} (fetch them per corpus/d2/candidates/whole-program.lock.json)`);
    process.exit(2);
  }

  const ex = new Executor();
  const frameworks = {};
  const nodeTest = makeNodeTest(ex);
  frameworks['node:test'] = Object.assign({}, nodeTest, { default: nodeTest.test, __esModule: true });
  if (cfg.framework === 'tape') frameworks.tape = makeTape(ex, cfg.tapeMajor || 4);
  if (cfg.framework === 'uvu') {
    const u = makeUvu(ex);
    frameworks.uvu = u.uvu;
    frameworks['uvu/assert'] = u.assert;
  }
  if (cfg.framework === 'ava') frameworks.ava = makeAva(ex);
  if (cfg.framework === 'mocha') Object.assign(globalThis, makeMocha(ex));
  for (const extra of cfg.adapters || []) {
    if (extra === 'expect.js') frameworks['expect.js'] = makeExpect();
    else if (extra === 'deep-equal') {
      // deep-equal(a, b[, {strict}]) returns a boolean (it does not assert).
      frameworks['deep-equal'] = (a, b, opts) =>
        makeDeepEqual(opts && opts.strict ? 'strict' : 'loose', 'none')(a, b);
    } else throw new Error('unknown adapter ' + extra);
  }

  const state = { cfg, testDir, frameworks };
  globalThis[HARNESS_KEY] = {
    frameworks,
    lib: (specifier) => libValue(state, cfg.modules[specifier]),
  };
  installHooks(state);

  // Load the library: compiled output or original sources through the shim.
  try {
    if (arg === '--original') {
      console.log(`# ${cfg.id}: original sources via ${path.relative(repoRoot, path.join(caseDir, 'entry.js'))}`);
      await import(pathToFileURL(path.join(caseDir, 'entry.js')).href);
    } else {
      const compiled = path.resolve(process.cwd(), arg);
      console.log(`# ${cfg.id}: compiled output ${arg}`);
      const code = fs.readFileSync(compiled, 'utf8');
      vm.runInThisContext('(function(){' + code + '\n}).call(globalThis);', { filename: compiled });
    }
    if (globalThis[cfg.global] === undefined) {
      throw new Error(`globalThis[${JSON.stringify(cfg.global)}] was not set by the program`);
    }
  } catch (e) {
    console.log('not ok 1 - load the program');
    console.log(String(e && e.stack ? e.stack : e).split('\n').map((l) => '  # ' + l).join('\n'));
    console.log('# FAIL');
    process.exitCode = 1;
    return;
  }

  // Register the tests.
  const req = createRequire(path.join(testDir, 'noop.js'));
  const helpers = {
    ex,
    testDir,
    require: (rel) => req(path.join(testDir, rel)),
    import: (rel) => import(pathToFileURL(path.join(testDir, rel)).href),
    read: (rel) => fs.readFileSync(path.join(testDir, rel), 'utf8'),
    test: (name, run) => ex.test(name, run),
  };
  if (cfg.framework === 'custom') {
    await cfg.register(helpers);
  } else {
    for (const rel of cfg.files) {
      const file = path.join(testDir, rel);
      if (isEsm(file)) await import(pathToFileURL(file).href);
      else req(file);
    }
  }

  const registered = ex.count();
  const results = await ex.run();
  const pass = results.filter((r) => r.status === 'pass').length;
  const fail = results.filter((r) => r.status === 'fail').length;
  const skip = results.filter((r) => r.status === 'skip').length;
  console.log(`# tests ${results.length}, pass ${pass}, fail ${fail}, skip ${skip}`);
  let ok = fail === 0 && results.length > 0;
  if (cfg.expectTests !== undefined && registered !== cfg.expectTests) {
    console.log(`# expected ${cfg.expectTests} tests but ${registered} were registered`);
    ok = false;
  }
  console.log(ok ? '# PASS' : '# FAIL');
  process.exitCode = ok ? 0 : 1;
}

// Mirrors Node's own choice: .mjs/.cjs by extension, else the nearest package.json "type",
// else (no "type") module-syntax detection.
function isEsm(file) {
  if (file.endsWith('.mjs')) return true;
  if (file.endsWith('.cjs')) return false;
  for (let dir = path.dirname(file); ; dir = path.dirname(dir)) {
    const pj = path.join(dir, 'package.json');
    if (fs.existsSync(pj)) {
      const type = JSON.parse(fs.readFileSync(pj, 'utf8')).type;
      if (type === 'module') return true;
      if (type === 'commonjs') return false;
      break;
    }
    if (path.dirname(dir) === dir) break;
  }
  return /^\s*(import\s|import\{|export\s|export\{)/m.test(fs.readFileSync(file, 'utf8'));
}
