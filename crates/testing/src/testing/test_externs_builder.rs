/*
 * Copyright 2018 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/testing/TestExternsBuilder.java.
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java.

//! Standard test externs, ported from the pinned TestExternsBuilder.
use crate::jscomp_api::SourceFile;
use closure_rhino::js_string::JsString;
const CLOSURE_EXTERNS: &str = r###"/** @const */ var goog = {};
goog.module = function(ns) {};
goog.module.declareLegacyNamespace = function() {};
goog.module.preventModuleExportSealing = function() {};
/** @return {?} */
goog.module.get = function(ns) {};
goog.provide = function(ns) {};
/** @return {?} */
goog.require = function(ns) {};
/** @return {?} */
goog.requireType = function(ns) {};
goog.requireDynamic = function(ns) {};
goog.loadModule = function(ns) {};
/** @return {?} */
goog.forwardDeclare = function(ns) {};
goog.setTestOnly = function() {};
goog.scope = function(fn) {};
goog.declareModuleId = function(ns) {};
/**
 * @param {string} name
 * @param {T} defaultValue
 * @return {T}
 * @template T
 */
goog.define = function(name, defaultValue) {};
goog.exportSymbol = function(publicName, symbol) {};
goog.inherits = function(childCtor, parentCtor) {
  childCtor.superClass_ = parentCtor.prototype;
};
goog.getMsg = function(str) {};
/**
 * @param {T} symbol
 * @return {T|undefined}
 * @template T
 * @noinline
 */
goog.weakUsage = function(symbol) {};
"###;

const JSCOMP_LIBRARIES: &str = r###"/** @const */
var $jscomp = {};
/**
 * @param {function(new: ?)} subclass
 * @param {function(new: ?)} superclass
 */
$jscomp.inherits = function(subclass, superclass) {};
/**
 * @param {!Iterable<T>} iterable
 * @return {!Array<T>}
 * @template T
 */
$jscomp.arrayFromIterable = function(iterable) {};
/**
 * @param {string|!Iterable<T>|!IteratorLike<T>|!Arguments} iterable
 * @return {!Iterator<T>}
 * @template T
 */
$jscomp.makeIterator = function(iterable) {};
$jscomp.makeAsyncIterator = function(asyncIterable) {};
/**
 * @param {?} iterator
 * @param {?} iterResult
 * @return {void}
 */
$jscomp.iteratorClose = function(iterator, iterResult) {};
/**
 * @param {!IteratorLike<T>} iterator
 * @return {!Array<T>}
 * @template T
 */
$jscomp.arrayFromIterator = function(iterator) {};
/**
 * @param {function(): !Generator<?>} generatorFunction
 * @return {!Promise<?>}
 */
$jscomp.asyncExecutePromiseGeneratorFunction = function(generatorFunction) {};
/**
* @type {!Global}
*/
var globalThis;
/** @type {!Global} */
$jscomp.global = globalThis;
/** @const {typeof Reflect.construct} */
$jscomp.construct = Reflect.construct;
/** @constructor */
$jscomp.AsyncGeneratorWrapper = function(generator) {};
/** @constructor */
$jscomp.AsyncGeneratorWrapper$ActionRecord = function(action, value) {};
/** @enum {number} */
$jscomp.AsyncGeneratorWrapper$ActionEnum = {
  YIELD_VALUE: 0,
  YIELD_STAR: 1,
  AWAIT_VALUE: 2,
};"###;

const BIGINT_EXTERNS: &str = r###"/**
 * @constructor
 * @param {bigint|number|string} arg
 * @return {bigint}
 */
function BigInt(arg) {}

/**
 * @this {BigInt|bigint}
 * @param {number} width
 * @param {bigint} bigint
 * @return {bigint}
 */
BigInt.asIntN = function(width, bigint) {};

/**
 * @this {BigInt|bigint}
 * @param {number} width
 * @param {bigint} bigint
 * @return {bigint}
 */
BigInt.asUintN = function(width, bigint) {};

/**
 * @param {string|Array<string>=} locales
 * @param {?Object=} options
 * @return {string}
 */
BigInt.prototype.toLocaleString = function(locales, options) {};

/**
 * @this {BigInt|bigint}
 * @param {number=} radix
 * @return {string}
 */
BigInt.prototype.toString = function(radix) {};

/**
 * @return {bigint}
 */
BigInt.prototype.valueOf = function() {};
"###;

const ITERABLE_EXTERNS: &str = r###"// Symbol is needed for Symbol.iterator
/**
 * @constructor
 * @param {*=} opt_description
 * @return {symbol}
 * @nosideeffects
 */
function Symbol(opt_description) {}

/** @const {!symbol} */ Symbol.iterator;

/**
 * @record
 * @template TYield
 */
function IIterableResult() {};
/** @type {boolean} */
IIterableResult.prototype.done;
/** @type {TYield} */
IIterableResult.prototype.value;

/**
 * @record
 * @template T, TReturn, TNext
 */
function IteratorLike() {}
/**
 * @param {T=} value
 * @return {!IIterableResult<T>}
 */
IteratorLike.prototype.next;
/**
 * @type {((function(T=): !IIterableResult<T>)|undefined)}
 */
IteratorLike.prototype.return;
/**
 * @type {((function(?=): !IIterableResult<T>)|undefined)}
 */
IteratorLike.prototype.throw;

/**
 * @interface
 * @template T, TReturn, TNext
 */
function Iterable() {}

/**
 * @return {!IteratorLike<T, ?, *>}
 * @suppress {externsValidation}
 */
Iterable.prototype[Symbol.iterator] = function() {};

/**
 * @interface
 * @extends {IteratorLike<T, ?, *>}
 * @extends {Iterable<T, ?, *>}
 * @template T, TReturn, TNext
 */
function IteratorIterable() {}

/**
 * @constructor
 * @abstract
 * @implements {IteratorIterable<T, TReturn, TNext>}
 * @template T, TReturn, TNext
 * @return {?}
 */
function Iterator() {}
/**
 * @param {string|!Iterable<T>|!IteratorLike<T>} iterable
 * @return {!Iterator<T>}
 * @template T
 */
Iterator.from = function(iterable) {};
/**
 * @override
 * @param {?=} value
 * @return {!IIterableResult<T>}
 */
Iterator.prototype.next;
/**
 * @type {((function(T=): !IIterableResult<T>)|undefined)}
 */
Iterator.prototype.return;
/**
 * @type {((function(?=): !IIterableResult<T>)|undefined)}
 */
Iterator.prototype.throw;
/**
 * @override
 * @return {!Iterator<T, TReturn, TNext>}
 * @suppress {externsValidation}
 */
Iterator.prototype[Symbol.iterator] = function() {};
/**
 * @template U
 * @param {function(T, number): U} mapperFn
 * @return {!Iterator<U>}
 */
Iterator.prototype.map;
/**
 * @param {function(T, number): *} filtererFn
 * @return {!Iterator<T>}
 */
Iterator.prototype.filter;
/**
 * @param {number} limit
 * @return {!Iterator<T>}
 */
Iterator.prototype.take;
/**
 * @param {number} limit
 * @return {!Iterator<T>}
 */
Iterator.prototype.drop;
/**
 * @template U
 * @param {function(T, number): (!Iterable<U>|!IteratorLike<U>|!Iterator<U>)} mapperFn
 * @return {!Iterator<U>}
 */
Iterator.prototype.flatMap;
/**
 * @template U
 * @param {function(U, T, number): U} reducerFn
 * @param {U=} initialValue
 * @return {U}
 */
Iterator.prototype.reduce;
/**
 * @return {!Array<T>}
 */
Iterator.prototype.toArray;
/**
 * @param {function(T, number): *} callbackFn
 * @return {void}
 */
Iterator.prototype.forEach;
/**
 * @param {function(T, number): *} predicate
 * @return {boolean}
 */
Iterator.prototype.some;
/**
 * @param {function(T, number): *} predicate
 * @return {boolean}
 */
Iterator.prototype.every;
/**
 * @param {function(T, number): *} predicate
 * @return {T|undefined}
 */
Iterator.prototype.find;

/**
 * @interface
 * @extends {IteratorIterable<T, ?, *>}
 * @template T, TReturn, TNext
 */
function Generator() {}
/**
 * @param {?=} opt_value
 * @return {!IIterableResult<T>}
 * @override
 */
Generator.prototype.next = function(opt_value) {};
/**
 * @param {T=} value
 * @return {!IIterableResult<T>}
 */
Generator.prototype.return = function(value) {};
/**
 * @param {?=} exception
 * @return {!IIterableResult<T>}
 */
Generator.prototype.throw = function(exception) {};
"###;

const STRING_EXTERNS: &str = r###"/**
 * @constructor
 * @implements {Iterable<string>}
 * @param {*=} arg
 * @return {string}
 */
function String(arg) {}
/** @override @return {!Iterator<string>} */
String.prototype[Symbol.iterator] = function() {};
/** @type {number} */
String.prototype.length;
/** @param {number} sliceArg */
String.prototype.slice = function(sliceArg) {};
/**
 * @this {string|!String}
 * @param {*=} opt_separator
 * @param {number=} opt_limit
 * @return {!Array<string>}
 */
String.prototype.split = function(opt_separator, opt_limit) {};
/**
 * @this {string|!String}
 * @param {string} search_string
 * @param {number=} opt_position
 * @return {boolean}
 * @nosideeffects
 */
String.prototype.startsWith = function(search_string, opt_position) {};
/**
 * @this {!String|string}
 * @param {?} regex
 * @param {?} str
 * @param {string=} opt_flags
 * @return {string}
 */
String.prototype.replace = function(regex, str, opt_flags) {};
/**
 * @this {!String|string}
 * @param {number} index
 * @return {string}
 */
String.prototype.charAt = function(index) {};
/**
 * @this {!String|string}
 * @param {*} regexp
 * @return {!Array<string>}
 */
String.prototype.match = function(regexp) {};
/**
 * @this {!String|string}
 * @return {string}
 */
String.prototype.toLowerCase = function() {};

/**
 * @param {number} count
 * @this {!String|string}
 * @return {string}
 * @nosideeffects
 */
String.prototype.repeat = function(count) {};

/**
 * @param {string} searchString
 * @param {number=} position
 * @return {boolean}
 * @nosideeffects
 */
String.prototype.includes = function(searchString, position) {};
"###;

const FUNCTION_EXTERNS: &str = r###"/**
 * @constructor
 * @param {...*} var_args
 */
function Function(var_args) {}
/** @type {!Function} */ Function.prototype.apply;
/** @type {!Function} */ Function.prototype.bind;
/** @type {!Function} */ Function.prototype.call;
/** @type {number} */
Function.prototype.length;
/** @type {string} */
Function.prototype.name;
"###;

const OBJECT_EXTERNS: &str = r###"/**
 * @record
 * @template THIS
 */
function ObjectPropertyDescriptor() {}
/** @type {(*|undefined)} */
ObjectPropertyDescriptor.prototype.value;
/** @type {(function(this: THIS):?)|undefined} */
ObjectPropertyDescriptor.prototype.get;

/** @type {(function(this: THIS, ?):void)|undefined} */
ObjectPropertyDescriptor.prototype.set;

/** @type {boolean|undefined} */
ObjectPropertyDescriptor.prototype.writable;

/** @type {boolean|undefined} */
ObjectPropertyDescriptor.prototype.enumerable;

/** @type {boolean|undefined} */
ObjectPropertyDescriptor.prototype.configurable;

/**
 * @constructor
 * @param {*=} opt_value
 * @return {!Object}
 * @template K,V
 */
function Object(opt_value) {}

/** @type {?Object} */ Object.prototype.__proto__;
/** @return {string} */
Object.prototype.toString = function() {};
/**
 * @param {*} propertyName
 * @return {boolean}
 */
Object.prototype.hasOwnProperty = function(propertyName) {};
/** @type {?Function} */ Object.prototype.constructor;
/** @return {*} */
Object.prototype.valueOf = function() {};
/**
 * @param {!Object} obj
 * @param {string} prop
 * @return {!ObjectPropertyDescriptor|undefined}
 * @nosideeffects
 */
Object.getOwnPropertyDescriptor = function(obj, prop) {};
/**
 * @param {!Object} obj
 * @param {string | symbol} prop
 * @param {!ObjectPropertyDescriptor} descriptor
 * @return {!Object}
 */
Object.defineProperty = function(obj, prop, descriptor) {};

/**
 * @template T
 * @param {T} obj
 * @param {!Object<string|symbol, !ObjectPropertyDescriptor<T>>} props
 * @return {T}
 */
Object.defineProperties = function(obj, props) {};

/**
 * @param {?Object} proto
 * @param {?Object=} opt_properties
 * @return {!Object}
 */
Object.create = function(proto, opt_properties) {};
/**
 * @param {!Object} obj
 * @param {?} proto
 * @return {!Object}
 */
Object.setPrototypeOf = function(obj, proto) {};
/**
 * @param {!Object} obj
 * @return {?Object}
 * @nosideeffects
 */
Object.getPrototypeOf = function(obj) {};

/**
 * @param {!Object} target
 * @param {...(Object|null|undefined)} var_args
 * @return {!Object}
 */
Object.assign = function(target, var_args) {};

/**
 * @param {T} obj
 * @return {T}
 * @template T
 */
Object.seal = function(obj) {}
"###;

const REFLECT_EXTERNS: &str = r###"/** @const */
var Reflect = {}

/**
 * @param {function(new: ?, ...?)} targetConstructorFn
 * @param {!Array<?>} argList
 * @param {function(new: TARGET, ...?)=} opt_newTargetConstructorFn
 * @return {TARGET}
 * @template TARGET
 * @nosideeffects
 */
Reflect.construct = function(
    targetConstructorFn, argList, opt_newTargetConstructorFn) {};

/**
 * @param {!Object} target
 * @param {?Object} proto
 * @return {boolean}
 */
Reflect.setPrototypeOf = function(target, proto) {};
"###;

const ARRAY_EXTERNS: &str = r###"/**
 * @interface
 * @template KEY1, VALUE1
 */
function IObject() {};

/**
 * @record
 * @extends IObject<number, VALUE2>
 * @template VALUE2
 */
function IArrayLike() {};

/** @type {number} */
IArrayLike.prototype.length;

/**
 * @template T
 * @record
 * @extends {IArrayLike<T>}
 * @extends {Iterable<T>}
 */
function ReadonlyArray(var_args) {}
/** @type {number} */ ReadonlyArray.prototype.length;
/**
 * @param {?function(this:S, T, number, !Array<T>): ?} callback
 * @param {S=} opt_thisobj
 * @this {!IArrayLike<T>|string}
 * @template T,S
 * @return {undefined}
 */
ReadonlyArray.prototype.forEach;
/**
 * @param {?function(this:S, T, number, !Array<T>): ?} callback
 * @param {S=} opt_thisobj
 * @return {!Array<T>}
 * @this {!IArrayLike<T>|string}
 * @template T,S
 */
ReadonlyArray.prototype.filter;
/**
 * @param {...*} var_args
 * @return {!Array<?>}
 * @this {*}
 */
ReadonlyArray.prototype.concat;
/**
 * @param {?number=} begin Zero-based index at which to begin extraction.
 * @param {?number=} end Zero-based index at which to end extraction.  slice
 * extracts up to but not including end.
 * @return {!Array<T>}
 * @this {!IArrayLike<T>|string}
 * @template T
 * @nosideeffects
 */
ReadonlyArray.prototype.slice;

/**
 * @return {!IteratorIterable<T>}
 */
ReadonlyArray.prototype.values;

/**
 * @param {T} searchElement
 * @param {number=} fromIndex
 * @return {boolean}
 * @this {!IArrayLike<T>|string}
 * @template T
 * @nosideeffects
 */
ReadonlyArray.prototype.includes;
/**
 * @template T
 * @constructor
 * @implements {ReadonlyArray<T>}
 * @implements {IArrayLike<T>}
 * @implements {Iterable<T>}
 * @param {...*} var_args
 * @return {!Array<?>}
 */
function Array(var_args) {}
/** @override */
Array.prototype[Symbol.iterator] = function() {};
/** @override @type {number} */ Array.prototype.length;
/**
 * @param {*} arr
 * @return {boolean}
 */
Array.isArray = function(arr) {};

/**
 * @param {string|!IArrayLike<T>|!Iterable<T>} arrayLike
 * @param {function(this:S, (string|T), number): R=} mapFn
 * @param {S=} thisObj
 * @return {!Array<R>}
 * @template T,S,R
 */
Array.from = function(arrayLike, mapFn, thisObj) {}

/**
 * @param {...T} var_args
 * @return {number} The new length of the array.
 * @this {!IArrayLike<T>}
 * @template T
 * @modifies {this}
 */
Array.prototype.push = function(var_args) {};
/**
 * @this {!IArrayLike<T>}
 * @return {T}
 * @template T
 */
Array.prototype.shift = function() {};
/**
 * @override
 * @param {?function(this:S, T, number, !Array<T>): ?} callback
 * @param {S=} opt_thisobj
 * @this {!IArrayLike<T>|string}
 * @template T,S
 * @return {undefined}
 */
Array.prototype.forEach = function(callback, opt_thisobj) {};
/**
 * @override
 * @param {?function(this:S, T, number, !Array<T>): ?} callback
 * @param {S=} opt_thisobj
 * @return {!Array<T>}
 * @this {!IArrayLike<T>|string}
 * @template T,S
 */
Array.prototype.filter = function(callback, opt_thisobj) {};
/**
 * @override
 * @param {...*} var_args
 * @return {!Array<?>}
 * @this {*}
 */
Array.prototype.concat = function(var_args) {};
/**
 * @override
 * @param {?number=} begin Zero-based index at which to begin extraction.
 * @param {?number=} end Zero-based index at which to end extraction.  slice
 * extracts up to but not including end.
 * @return {!Array<T>}
 * @this {!IArrayLike<T>|string}
 * @template T
 * @nosideeffects
 */
Array.prototype.slice = function(begin, end) {};

/**
 * @override
 * @return {!IteratorIterable<T>}
 */
Array.prototype.values;

/**
 * @override
 * @param {T} searchElement
 * @param {number=} fromIndex
 * @return {boolean}
 * @this {!IArrayLike<T>|string}
 * @template T
 * @nosideeffects
 */
Array.prototype.includes = function(searchElement, fromIndex) {};
"###;

const MAP_EXTERNS: &str = r###"/**
 * @interface
 * @extends {Iterable<K|V>}
 * @template K, V
 */
function ReadonlyMap() {}
/**
 * @return {!IteratorIterable<K|V>}
 */
ReadonlyMap.prototype.entries = function() {};
/**
 * @constructor @struct
 * @param {?Iterable<!Array<K|V>>|!Array<!Array<K|V>>=} opt_iterable
 * @implements {ReadonlyMap<K, V>}
 * @template K, V
 */
function Map(opt_iterable) {}
/** @override */
Map.prototype[Symbol.iterator] = function() {};
/**
 * @override
 * @return {!IteratorIterable<K|V>}
 */
Map.prototype.entries = function() {};"###;

const ARGUMENTS_EXTERNS: &str = r###"/**
 * @constructor
 * @implements {IArrayLike<?>}
 * @implements {Iterable<?>}
 */
function Arguments() {}

/** @type {number} */
Arguments.prototype.length;
/** @override */
Arguments.prototype[Symbol.iterator] = function() {};

/** @type {!Arguments} */
var arguments;
"###;

const CONSOLE_EXTERNS: &str = r###"/** @constructor */
function Console() {};

/**
 * @param {...*} var_args
 * @return {undefined}
 */
Console.prototype.log = function(var_args) {};

/** @const {!Console} */
var console;
"###;

const ALERT_EXTERNS: &str = r###"/**
 * @param {*} message
 * @return {undefined}
 */
function alert(message) {}
"###;

const UNDEFINED_EXTERNS: &str = r###"/** @type {undefined} */
var undefined;
"###;

const INFINITY_EXTERNS: &str = r###"/**
 * @type {number}
 * @see http://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Infinity
 * @const
 */
var Infinity;
"###;

const NAN_EXTERNS: &str = r###"/**
 * @type {number}
 * @see http://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/NaN
 * @const
 */
var NaN;
"###;

const PROMISE_EXTERNS: &str = r###"
/**
 * @typedef {{then: ?}}
 */
var Thenable;


/**
 * @interface
 * @template TYPE
 */
function IThenable() {}


/**
 * @param {?(function(TYPE):VALUE)=} opt_onFulfilled
 * @param {?(function(*): *)=} opt_onRejected
 * @return {RESULT}
 * @template VALUE
 *
 * @template RESULT := type('IThenable',
 * cond(isUnknown(VALUE), unknown(),
 * mapunion(VALUE, (V) =>
 * cond(isTemplatized(V) && sub(rawTypeOf(V), 'IThenable'),
 * templateTypeOf(V, 0),
 * cond(sub(V, 'Thenable'),
 * unknown(),
 * V)))))
 * =:
 */
IThenable.prototype.then = function(opt_onFulfilled, opt_onRejected) {};


/**
 * @param {function(
 * function((TYPE|IThenable<TYPE>|Thenable|null)=),
 * function(*=))} resolver
 * @constructor
 * @implements {IThenable<TYPE>}
 * @template TYPE
 */
function Promise(resolver) {}


/**
 * @param {VALUE=} opt_value
 * @return {RESULT}
 * @template VALUE
 * @template RESULT := type('Promise',
 * cond(isUnknown(VALUE), unknown(),
 * mapunion(VALUE, (V) =>
 * cond(isTemplatized(V) && sub(rawTypeOf(V), 'IThenable'),
 * templateTypeOf(V, 0),
 * cond(sub(V, 'Thenable'),
 * unknown(),
 * V)))))
 * =:
 */
Promise.resolve = function(opt_value) {};


/**
 * @param {*=} opt_error
 * @return {!Promise<?>}
 */
Promise.reject = function(opt_error) {};


/**
 * @param {!Iterable<VALUE>} iterable
 * @return {!Promise<!Array<RESULT>>}
 * @template VALUE
 * @template RESULT := mapunion(VALUE, (V) =>
 * cond(isUnknown(V),
 * unknown(),
 * cond(isTemplatized(V) && sub(rawTypeOf(V), 'IThenable'),
 * templateTypeOf(V, 0),
 * cond(sub(V, 'Thenable'), unknown(), V))))
 * =:
 */
Promise.all = function(iterable) {};


/**
 * @param {!Iterable<VALUE>} iterable
 * @return {!Promise<RESULT>}
 * @template VALUE
 * @template RESULT := mapunion(VALUE, (V) =>
 * cond(isUnknown(V),
 * unknown(),
 * cond(isTemplatized(V) && sub(rawTypeOf(V), 'IThenable'),
 * templateTypeOf(V, 0),
 * cond(sub(V, 'Thenable'), unknown(), V))))
 * =:
 */
Promise.race = function(iterable) {};


/**
 * @param {?(function(this:void, TYPE):VALUE)=} opt_onFulfilled
 * @param {?(function(this:void, *): *)=} opt_onRejected
 * @return {RESULT}
 * @template VALUE
 *
 * @template RESULT := type('Promise',
 * cond(isUnknown(VALUE), unknown(),
 * mapunion(VALUE, (V) =>
 * cond(isTemplatized(V) && sub(rawTypeOf(V), 'IThenable'),
 * templateTypeOf(V, 0),
 * cond(sub(V, 'Thenable'),
 * unknown(),
 * V)))))
 * =:
 * @override
 */
Promise.prototype.then = function(opt_onFulfilled, opt_onRejected) {};


/**
 * @param {function(*): RESULT} onRejected
 * @return {!Promise<RESULT>}
 * @template RESULT
 */
Promise.prototype.catch = function(onRejected) {};


/**
 * @param {function()} callback
 * @return {!Promise<TYPE>}
 */
Promise.prototype.finally = function(callback) {};
"###;

const I_TEMPLATE_ARRAY_EXTERNS: &str = r###"/**
 * @constructor
 * @extends {Array<string>}
 */
function ITemplateArray() {}"###;

const ASYNC_ITERABLE_EXTERNS: &str = r###"/**
 * @const {symbol}
 */
Symbol.asyncIterator;
/**
 * @interface
 * @template T, TReturn, TNext
 */
function AsyncIterator() {}
/**
 * @param {?=} opt_value
 * @return {!Promise<!IIterableResult<T>>}
 */
AsyncIterator.prototype.next;
/**
 * @interface
 * @template T, TReturn, TNext
 */
function AsyncIterable() {}
/**
 * @return {!AsyncIterator<T,?,*>}
 */
AsyncIterable.prototype[Symbol.asyncIterator] = function() {};
/**
 * @interface
 * @extends {AsyncIterator<T,?,*>}
 * @extends {AsyncIterable<T,?,*>}
 * @template T, TReturn, TNext
 */
function AsyncIteratorIterable() {}
/**
 * @interface
 * @extends {AsyncIteratorIterable<T,?,*>}
 * @template T, TReturn, TNext
 */
function AsyncGenerator() {}
/**
 * @param {?=} opt_value
 * @return {!Promise<!IIterableResult<T>>}
 * @override
 */
AsyncGenerator.prototype.next = function(opt_value) {};
/**
 * @param {T} value
 * @return {!Promise<!IIterableResult<T>>}
 */
AsyncGenerator.prototype.return = function(value) {};
/**
 * @param {?} exception
 * @return {!Promise<!IIterableResult<T>>}
 */
AsyncGenerator.prototype.throw = function(exception) {};"###;

const ES6_CLASS_TRANSPILATION_EXTERNS: &str = r###"var $jscomp = {};

/**
 * @param {?} subClass
 * @param {?} superClass
 * @return {?} newClass
 */
$jscomp.inherits = function(subClass, superClass) {};
"###;

const MATH_EXTERNS: &str = r###"
/** @const */
var Math = {};

/**
 * @return {number}
 * @nosideeffects
 */
Math.random = function() {};

/**
 * @param {number} a
 * @param {number} b
 * @return {number}
 * @nosideeffects
 */
Math.max = function(a, b) {};

/**
 * @param {number} a
 * @param {number} b
 * @return {number}
 * @nosideeffects
 */
Math.min = function(a, b) {};

/**
 * @param {number} a
 * @param {number} b
 * @return {number}
 * @nosideeffects
 */
Math.pow = function(a, b) {};
"###;

const REG_EXP_EXTERNS: &str = r###"/**
 * @constructor
 * @param {*=} opt_pattern
 * @param {*=} opt_flags
 * @return {!RegExp}
 * @throws {SyntaxError} if opt_pattern is an invalid pattern.
 */
function RegExp(opt_pattern, opt_flags) {}
/**
 * @param {*} str The string to search.
 * @return {?RegExpResult}
 */
RegExp.prototype.exec = function(str) {};
/**
 * @constructor
 * @extends {Array<string>}
 */
var RegExpResult = function() {};
/** @type {string} */
RegExp.$1;"###;

const HTML_ELEMENT_EXTERNS: &str = r###"/** @constructor */
function Node() {}
/** @constructor @extends {Node} */
function Element() {}
/** @constructor @extends {Element} */
function HTMLElement() {}"###;

const POLYMER_EXTERNS: &str = r###"
/**
 * @param {!Object} init
 * @return {!function(new:HTMLElement)}
 */
function Polymer(init) {}

Polymer.ElementMixin = function(mixin) {}

/** @typedef {!Object} */
var PolymerElementProperties;

/** @interface */
function Polymer_ElementMixin() {}
/** @type {string} */
Polymer_ElementMixin.prototype._importPath;

/**
* @interface
* @extends {Polymer_ElementMixin}
*/
function Polymer_LegacyElementMixin(){}
/** @type {boolean} */
Polymer_LegacyElementMixin.prototype.isAttached;
/**
 * @constructor
 * @extends {HTMLElement}
 * @implements {Polymer_LegacyElementMixin}
 */
var PolymerElement = function() {};  // Polymer 1

/** @constructor @extends {HTMLElement} */
Polymer.Element = function() {};  // Polymer 2
"###;

#[derive(Default)]
pub struct TestExternsBuilder {
    include_big_int_externs: bool,
    include_iterable_externs: bool,
    include_string_externs: bool,
    include_function_externs: bool,
    include_object_externs: bool,
    include_array_externs: bool,
    include_map_externs: bool,
    include_arguments_externs: bool,
    include_undefined_externs: bool,
    include_infinity_externs: bool,
    include_nan_externs: bool,
    include_i_template_array_externs: bool,
    include_console_externs: bool,
    include_alert_externs: bool,
    include_promise_externs: bool,
    include_async_iterable_externs: bool,
    include_es6_class_transpilation_externs: bool,
    include_reflect_externs: bool,
    include_closure_externs: bool,
    include_js_comp_libraries: bool,
    include_math_externs: bool,
    include_reg_exp_externs: bool,
    include_html_element_externs: bool,
    include_polymer_externs: bool,
    extra_externs: Vec<JsString>,
}
impl TestExternsBuilder {
    // port: TestExternsBuilder#TestExternsBuilder
    pub fn new() -> Self {
        Self::default()
    }
    // port: TestExternsBuilder#getClosureExternsAsSource
    pub fn get_closure_externs_as_source() -> JsString {
        CLOSURE_EXTERNS.into()
    }
    // port: TestExternsBuilder#addBigInt
    pub fn add_big_int(&mut self) -> &mut Self {
        self.include_big_int_externs = true;
        self
    }
    // port: TestExternsBuilder#addIterable
    pub fn add_iterable(&mut self) -> &mut Self {
        self.include_iterable_externs = true;
        self
    }
    // port: TestExternsBuilder#addString
    pub fn add_string(&mut self) -> &mut Self {
        self.include_string_externs = true;
        self.add_iterable();
        self
    }
    // port: TestExternsBuilder#addFunction
    pub fn add_function(&mut self) -> &mut Self {
        self.include_function_externs = true;
        self
    }
    // port: TestExternsBuilder#addObject
    pub fn add_object(&mut self) -> &mut Self {
        self.include_object_externs = true;
        self.add_function();
        self
    }
    // port: TestExternsBuilder#addArray
    pub fn add_array(&mut self) -> &mut Self {
        self.include_array_externs = true;
        self.add_iterable();
        self
    }
    // port: TestExternsBuilder#addMap
    pub fn add_map(&mut self) -> &mut Self {
        self.include_map_externs = true;
        self.add_array();
        self
    }
    // port: TestExternsBuilder#addArguments
    pub fn add_arguments(&mut self) -> &mut Self {
        self.include_arguments_externs = true;
        self.add_array();
        self.add_iterable();
        self
    }
    // port: TestExternsBuilder#addUndefined
    pub fn add_undefined(&mut self) -> &mut Self {
        self.include_undefined_externs = true;
        self
    }
    // port: TestExternsBuilder#addInfinity
    pub fn add_infinity(&mut self) -> &mut Self {
        self.include_infinity_externs = true;
        self
    }
    // port: TestExternsBuilder#addNaN
    pub fn add_nan(&mut self) -> &mut Self {
        self.include_nan_externs = true;
        self
    }
    // port: TestExternsBuilder#addITemplateArray
    pub fn add_i_template_array(&mut self) -> &mut Self {
        self.include_i_template_array_externs = true;
        self.add_array();
        self
    }
    // port: TestExternsBuilder#addPromise
    pub fn add_promise(&mut self) -> &mut Self {
        self.include_promise_externs = true;
        self.add_iterable();
        self
    }
    // port: TestExternsBuilder#addConsole
    pub fn add_console(&mut self) -> &mut Self {
        self.include_console_externs = true;
        self
    }
    // port: TestExternsBuilder#addAlert
    pub fn add_alert(&mut self) -> &mut Self {
        self.include_alert_externs = true;
        self
    }
    // port: TestExternsBuilder#addAsyncIterable
    pub fn add_async_iterable(&mut self) -> &mut Self {
        self.include_async_iterable_externs = true;
        self.add_iterable();
        self.add_promise();
        self
    }
    // port: TestExternsBuilder#addReflect
    pub fn add_reflect(&mut self) -> &mut Self {
        self.include_reflect_externs = true;
        self.add_object();
        self
    }
    // port: TestExternsBuilder#addEs6ClassTranspilationExterns
    pub fn add_es6_class_transpilation_externs(&mut self) -> &mut Self {
        self.include_es6_class_transpilation_externs = true;
        self.add_function();
        self
    }
    // port: TestExternsBuilder#addClosureExterns
    pub fn add_closure_externs(&mut self) -> &mut Self {
        self.include_closure_externs = true;
        self
    }
    // port: TestExternsBuilder#addJSCompLibraries
    pub fn add_js_comp_libraries(&mut self) -> &mut Self {
        self.include_js_comp_libraries = true;
        self.add_arguments();
        self.add_iterable();
        self.add_reflect();
        self
    }
    // port: TestExternsBuilder#addMath
    pub fn add_math(&mut self) -> &mut Self {
        self.include_math_externs = true;
        self
    }
    // port: TestExternsBuilder#addRegExp
    pub fn add_reg_exp(&mut self) -> &mut Self {
        self.add_array();
        self.include_reg_exp_externs = true;
        self
    }
    // port: TestExternsBuilder#addHtmlElement
    pub fn add_html_element(&mut self) -> &mut Self {
        self.include_html_element_externs = true;
        self
    }
    // port: TestExternsBuilder#addPolymer
    pub fn add_polymer(&mut self) -> &mut Self {
        self.add_html_element();
        self.include_polymer_externs = true;
        self
    }
    // port: TestExternsBuilder#addExtra
    pub fn add_extra(&mut self, lines: &[JsString]) -> &mut Self {
        self.extra_externs.extend_from_slice(lines);
        self
    }
    // port: TestExternsBuilder#build
    pub fn build(&self) -> JsString {
        let mut extern_sections: Vec<JsString> = vec![
            r###"/**
 * @fileoverview
 * @externs
 */
"###
            .into(),
        ];
        if self.include_big_int_externs {
            extern_sections.push(BIGINT_EXTERNS.into());
        }
        if self.include_iterable_externs {
            extern_sections.push(ITERABLE_EXTERNS.into());
        }
        if self.include_string_externs {
            extern_sections.push(STRING_EXTERNS.into());
        }
        if self.include_function_externs {
            extern_sections.push(FUNCTION_EXTERNS.into());
        }
        if self.include_object_externs {
            extern_sections.push(OBJECT_EXTERNS.into());
        }
        if self.include_array_externs {
            extern_sections.push(ARRAY_EXTERNS.into());
        }
        if self.include_map_externs {
            extern_sections.push(MAP_EXTERNS.into());
        }
        if self.include_reflect_externs {
            extern_sections.push(REFLECT_EXTERNS.into());
        }
        if self.include_arguments_externs {
            extern_sections.push(ARGUMENTS_EXTERNS.into());
        }
        if self.include_undefined_externs {
            extern_sections.push(UNDEFINED_EXTERNS.into());
        }
        if self.include_infinity_externs {
            extern_sections.push(INFINITY_EXTERNS.into());
        }
        if self.include_nan_externs {
            extern_sections.push(NAN_EXTERNS.into());
        }
        if self.include_i_template_array_externs {
            extern_sections.push(I_TEMPLATE_ARRAY_EXTERNS.into());
        }
        if self.include_console_externs {
            extern_sections.push(CONSOLE_EXTERNS.into());
        }
        if self.include_alert_externs {
            extern_sections.push(ALERT_EXTERNS.into());
        }
        if self.include_promise_externs {
            extern_sections.push(PROMISE_EXTERNS.into());
        }
        if self.include_async_iterable_externs {
            extern_sections.push(ASYNC_ITERABLE_EXTERNS.into());
        }
        if self.include_math_externs {
            extern_sections.push(MATH_EXTERNS.into());
        }
        if self.include_reg_exp_externs {
            extern_sections.push(REG_EXP_EXTERNS.into());
        }
        if self.include_es6_class_transpilation_externs {
            extern_sections.push(ES6_CLASS_TRANSPILATION_EXTERNS.into());
        }
        if self.include_closure_externs {
            extern_sections.push(CLOSURE_EXTERNS.into());
        }
        if self.include_js_comp_libraries {
            extern_sections.push(JSCOMP_LIBRARIES.into());
        }
        if self.include_html_element_externs {
            extern_sections.push(HTML_ELEMENT_EXTERNS.into());
        }
        if self.include_polymer_externs {
            extern_sections.push(POLYMER_EXTERNS.into());
        }
        extern_sections.extend_from_slice(&self.extra_externs);
        let mut units = Vec::new();
        for (i, section) in extern_sections.iter().enumerate() {
            if i != 0 {
                units.push(10);
            }
            units.extend_from_slice(section.as_units());
        }
        JsString::from_units(units)
    }
    // port: TestExternsBuilder#buildExternsFile
    pub fn build_externs_file(&self, file_path: &str) -> SourceFile {
        SourceFile::from_code(file_path, self.build())
    }
}

impl crate::replay::replay_dsl::NativeObject for TestExternsBuilder {
    // port: TestExternsBuilder#TestExternsBuilder (native replay class)
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.testing.TestExternsBuilder"
    }
    // port: ReplayDsl#invoke (TestExternsBuilder method adapter)
    fn call(
        &mut self,
        method: &str,
        args: Vec<crate::replay::replay_dsl::DslValue>,
    ) -> Result<crate::replay::replay_dsl::DslValue, crate::throwable::Throwable> {
        use crate::{replay::replay_dsl::DslValue, throwable::Throwable};
        match method {
            "addBigInt" => {
                self.add_big_int();
            }
            "addIterable" => {
                self.add_iterable();
            }
            "addString" => {
                self.add_string();
            }
            "addFunction" => {
                self.add_function();
            }
            "addObject" => {
                self.add_object();
            }
            "addArray" => {
                self.add_array();
            }
            "addMap" => {
                self.add_map();
            }
            "addArguments" => {
                self.add_arguments();
            }
            "addUndefined" => {
                self.add_undefined();
            }
            "addInfinity" => {
                self.add_infinity();
            }
            "addNaN" => {
                self.add_nan();
            }
            "addITemplateArray" => {
                self.add_i_template_array();
            }
            "addPromise" => {
                self.add_promise();
            }
            "addConsole" => {
                self.add_console();
            }
            "addAlert" => {
                self.add_alert();
            }
            "addAsyncIterable" => {
                self.add_async_iterable();
            }
            "addReflect" => {
                self.add_reflect();
            }
            "addEs6ClassTranspilationExterns" => {
                self.add_es6_class_transpilation_externs();
            }
            "addClosureExterns" => {
                self.add_closure_externs();
            }
            "addJSCompLibraries" => {
                self.add_js_comp_libraries();
            }
            "addMath" => {
                self.add_math();
            }
            "addRegExp" => {
                self.add_reg_exp();
            }
            "addHtmlElement" => {
                self.add_html_element();
            }
            "addPolymer" => {
                self.add_polymer();
            }
            "addExtra" => {
                let items = if let [DslValue::Array { items, .. }] = args.as_slice() {
                    items.as_slice()
                } else {
                    args.as_slice()
                };
                let lines = items
                    .iter()
                    .map(|v| match v {
                        DslValue::String(s) => Ok(s.clone()),
                        _ => Err(Throwable::HarnessError(
                            "TestExternsBuilder.addExtra requires strings".into(),
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.add_extra(&lines);
            }
            "build" => return Ok(DslValue::String(self.build())),
            "buildExternsFile" => {
                if let [DslValue::String(path)] = args.as_slice() {
                    return Ok(DslValue::SourceFile(std::sync::Arc::new(
                        self.build_externs_file(&path.to_string_lossy()),
                    )));
                }
                return Err(Throwable::HarnessError(
                    "buildExternsFile requires a path".into(),
                ));
            }
            _ => {
                return Err(Throwable::HarnessError(format!(
                    "no TestExternsBuilder method {method}"
                )));
            }
        }
        Ok(DslValue::Null)
    }
    // port: ReplayDsl#invoke (native TestExternsBuilder receiver)
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
