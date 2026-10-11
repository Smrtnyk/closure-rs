// Property names of every shape: a lone high or low surrogate, a high surrogate at the end,
// a low one before a high one, and a valid pair.
/** @type {number} */ var a = {'\uD800': 1, '\uDC00': 'x', 'a\uDBFF': null, '\uDC00\uD800': true, '😀': 2};
// Record properties sort by their UTF-16 code units.
/** @type {number} */ var b = {'\uD800': 1, '': 2, 'A': 3, '\uDC00': 4, '?': 5, '�': 6};
// Union members sort by their type strings, also in UTF-16 code units.
/** @type {number} */ var c = Math.random() ? {'\uD800': 1} : Math.random() ? {A: 1} : {'': 1};
// Found and required differ only in an unpaired surrogate (or a surrogate and a real '?'): the
// type strings are not equal, so the message keeps the multiline form.
var o = {'\uD800': 1, a: 2};
var p = {'\uD801': 1, a: 2};
var q = {'?': 1, a: 2};
/** @type {typeof o} */ var t = p;
/** @type {typeof o} */ var u = q;
// Nested records and a record with more than ten properties.
/** @type {number} */ var d = {a: {'\uD800': {'\uDC00': [1]}}, b: function(/** {'\uDBFF': number} */ x) {}};
/** @type {number} */ var e = {k0: 0, k1: 1, k2: 2, k3: 3, k4: 4, k5: 5, k6: 6, k7: 7, k8: 8, k9: 9, 'k\uD800': 10, 'k\uDC00': 11};
alert(a, b, c, d, e, t, u);
