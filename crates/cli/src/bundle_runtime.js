/*
 * Copyright 2012, 2016, 2017, 2018, 2020, 2026 The Closure Compiler Authors.
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

 *
 * Generated from Closure Compiler's js/modules.js and its runtime dependencies.
 * The original sources and their notices are in crates/resources/data/com/google/javascript/jscomp/js.
 * Regenerate with CLOSURE_RS_UPDATE_BUNDLE_RUNTIME=1 cargo test -p closure-cli --lib bundle_runtime_matches_compiler
 */
var $jscomp = $jscomp || {};
$jscomp.scope = {};
$jscomp.ASSUME_ES2020 = false;
$jscomp.ASSUME_ES6 = $jscomp.ASSUME_ES2020;
$jscomp.ASSUME_ES5 = $jscomp.ASSUME_ES6;
$jscomp.ASSUME_NO_NATIVE_MAP = false;
$jscomp.ASSUME_NO_NATIVE_SET = false;
$jscomp.ISOLATE_POLYFILLS = false;
$jscomp.FORCE_POLYFILL_PROMISE = false;
$jscomp.FORCE_POLYFILL_PROMISE_WHEN_NO_UNHANDLED_REJECTION = false;
$jscomp.INSTRUMENT_ASYNC_CONTEXT = true;
$jscomp.getGlobal = function(passedInThis) {
  var possibleGlobals = ["object" == typeof globalThis && globalThis, passedInThis, "object" == typeof window && window, "object" == typeof self && self, "object" == typeof global && global];
  var i = 0;
  for (; i < possibleGlobals.length; ++i) {
    var maybeGlobal = possibleGlobals[i];
    if (maybeGlobal && maybeGlobal["Math"] == Math) {
      return maybeGlobal;
    }
  }
  return {valueOf:function() {
    throw new Error("Cannot find global object");
  }}.valueOf();
};
$jscomp.global = $jscomp.ASSUME_ES2020 ? globalThis : $jscomp.getGlobal(this);
$jscomp.checkEs6ConformanceViaProxy = function() {
  try {
    var proxied = {};
    var proxy = Object.create(new $jscomp.global["Proxy"](proxied, {"get":function(target, key, receiver) {
      return target == proxied && key == "q" && receiver == proxy;
    }}));
    return proxy["q"] === true;
  } catch (err) {
    return false;
  }
};
$jscomp.USE_PROXY_FOR_ES6_CONFORMANCE_CHECKS = false;
$jscomp.ES6_CONFORMANCE = $jscomp.USE_PROXY_FOR_ES6_CONFORMANCE_CHECKS && $jscomp.checkEs6ConformanceViaProxy();
$jscomp.arrayIteratorImpl = function(array) {
  var index = 0;
  return function() {
    if (index < array.length) {
      return {done:false, value:array[index++]};
    } else {
      return {done:true};
    }
  };
};
$jscomp.arrayIterator = function(array) {
  return {next:$jscomp.arrayIteratorImpl(array)};
};
$jscomp.getIteratorPrototype = function() {
  return typeof Iterator != "undefined" && Iterator.prototype && !Iterator.prototype.next ? Iterator.prototype : null;
};
$jscomp.defineProperty = $jscomp.ASSUME_ES5 || typeof Object.defineProperties == "function" ? Object.defineProperty : function(target, property, descriptor) {
  if (target == Array.prototype || target == Object.prototype) {
    return target;
  }
  target[property] = descriptor.value;
  return target;
};
$jscomp.objectCreate = $jscomp.ASSUME_ES5 || typeof Object.create == "function" ? Object.create : function(prototype) {
  var ctor = function() {
  };
  ctor.prototype = prototype;
  return new ctor();
};
$jscomp.IS_SYMBOL_NATIVE = typeof Symbol === "function" && typeof Symbol("x") === "symbol";
$jscomp.TRUST_ES6_POLYFILLS = !$jscomp.ISOLATE_POLYFILLS || $jscomp.IS_SYMBOL_NATIVE;
$jscomp.polyfills = {};
$jscomp.propertyToPolyfillSymbol = {};
$jscomp.POLYFILL_PREFIX = "$jscp$";
var $jscomp$lookupPolyfilledValue = function(target, property, isOptionalAccess) {
  if (isOptionalAccess && target == null) {
    return undefined;
  }
  var obfuscatedName = $jscomp.propertyToPolyfillSymbol[property];
  if (obfuscatedName == null) {
    return target[property];
  }
  var polyfill = target[obfuscatedName];
  return polyfill !== undefined ? polyfill : target[property];
};
$jscomp.TYPED_ARRAY_CLASSES = function() {
  var classes = ["Int8", "Uint8", "Uint8Clamped", "Int16", "Uint16", "Int32", "Uint32", "Float32", "Float64"];
  if ($jscomp.global.BigInt64Array) {
    classes.push("BigInt64");
    classes.push("BigUint64");
  }
  return classes;
}();
$jscomp.polyfillTypedArrayMethod = function(methodName, polyfill, fromLang, toLang) {
  if (!polyfill) {
    return;
  }
  var i = 0;
  for (; i < $jscomp.TYPED_ARRAY_CLASSES.length; i++) {
    var target = $jscomp.TYPED_ARRAY_CLASSES[i] + "Array.prototype." + methodName;
    if ($jscomp.ISOLATE_POLYFILLS) {
      $jscomp.polyfillIsolated(target, polyfill, fromLang, toLang);
    } else {
      $jscomp.polyfillUnisolated(target, polyfill, fromLang, toLang);
    }
  }
};
$jscomp.polyfill = function(target, polyfill, fromLang, toLang) {
  if (!polyfill) {
    return;
  }
  if ($jscomp.ISOLATE_POLYFILLS) {
    $jscomp.polyfillIsolated(target, polyfill, fromLang, toLang);
  } else {
    $jscomp.polyfillUnisolated(target, polyfill, fromLang, toLang);
  }
};
$jscomp.polyfillUnisolated = function(target, polyfill, fromLang, toLang) {
  var obj = $jscomp.global;
  var split = target.split(".");
  var i = 0;
  for (; i < split.length - 1; i++) {
    var key = split[i];
    if (!(key in obj)) {
      return;
    }
    obj = obj[key];
  }
  var property = split[split.length - 1];
  var orig = obj[property];
  var impl = polyfill(orig);
  if (impl == orig || impl == null) {
    return;
  }
  $jscomp.defineProperty(obj, property, {configurable:true, writable:true, value:impl});
};
$jscomp.polyfillIsolated = function(target, polyfill, fromLang, toLang) {
  var split = target.split(".");
  var isSimpleName = split.length === 1;
  var root = split[0];
  var ownerObject;
  if (!isSimpleName && root in $jscomp.polyfills) {
    ownerObject = $jscomp.polyfills;
  } else {
    ownerObject = $jscomp.global;
  }
  var i = 0;
  for (; i < split.length - 1; i++) {
    var key = split[i];
    if (!(key in ownerObject)) {
      return;
    }
    ownerObject = ownerObject[key];
  }
  var property = split[split.length - 1];
  var nativeImpl = $jscomp.IS_SYMBOL_NATIVE && fromLang === "es6" ? ownerObject[property] : null;
  var impl = polyfill(nativeImpl);
  if (impl == null) {
    return;
  }
  if (isSimpleName) {
    $jscomp.defineProperty($jscomp.polyfills, property, {configurable:true, writable:true, value:impl});
  } else if (impl !== nativeImpl) {
    if ($jscomp.propertyToPolyfillSymbol[property] === undefined) {
      var BIN_ID = Math.random() * 1e9 >>> 0;
      $jscomp.propertyToPolyfillSymbol[property] = $jscomp.IS_SYMBOL_NATIVE ? $jscomp.global["Symbol"](property) : $jscomp.POLYFILL_PREFIX + BIN_ID + "$" + property;
    }
    var obfuscatedName = $jscomp.propertyToPolyfillSymbol[property];
    $jscomp.defineProperty(ownerObject, obfuscatedName, {configurable:true, writable:true, value:impl});
  }
};
$jscomp.initSymbol = function() {
};
$jscomp.polyfill("Symbol", function(orig) {
  if (orig) {
    return orig;
  }
  var SymbolClass = function(id, opt_description) {
    this.$jscomp$symbol$id_ = id;
    this.description;
    $jscomp.defineProperty(this, "description", {configurable:true, writable:true, value:opt_description});
  };
  SymbolClass.prototype.toString = function() {
    return this.$jscomp$symbol$id_;
  };
  var BIN_ID = Math.random() * 1e9 >>> 0;
  var SYMBOL_PREFIX = "jscomp_symbol_" + BIN_ID + "_";
  var counter = 0;
  var symbolPolyfill = function(opt_description) {
    if (this instanceof symbolPolyfill) {
      throw new TypeError("Symbol is not a constructor");
    }
    return new SymbolClass(SYMBOL_PREFIX + (opt_description || "") + "_" + counter++, opt_description);
  };
  return symbolPolyfill;
}, "es6", "es3");
$jscomp.polyfill("Symbol.iterator", function(orig) {
  if (orig) {
    return orig;
  }
  var symbolIterator = Symbol("Symbol.iterator");
  $jscomp.defineProperty(Array.prototype, symbolIterator, {configurable:true, writable:true, value:function() {
    return $jscomp.iteratorPrototype($jscomp.arrayIteratorImpl(this));
  }});
  return symbolIterator;
}, "es6", "es3");
$jscomp.polyfill("Symbol.asyncIterator", function(orig) {
  if (orig) {
    return orig;
  }
  return Symbol("Symbol.asyncIterator");
}, "es9", "es3");
$jscomp.polyfill("Symbol.toStringTag", function(orig) {
  if (orig) {
    return orig;
  }
  return Symbol("Symbol.toStringTag");
}, "es6", "es3");
$jscomp.iteratorPrototype = function(next) {
  var parentProto = $jscomp.getIteratorPrototype() || Object.prototype;
  var iterator = $jscomp.objectCreate(parentProto);
  iterator.next = next;
  if (!parentProto[Symbol.iterator]) {
    iterator[Symbol.iterator] = function() {
      return this;
    };
  }
  return iterator;
};
$jscomp.makeIterator = function(iterable) {
  var iteratorFunction = typeof Symbol != "undefined" && Symbol.iterator && iterable[Symbol.iterator];
  if (iteratorFunction) {
    return iteratorFunction.call(iterable);
  }
  if (typeof iterable["length"] == "number") {
    return $jscomp.arrayIterator(iterable);
  }
  throw new Error(String(iterable) + " is not an iterable or ArrayLike");
};
$jscomp.owns = function(obj, prop) {
  return Object.prototype.hasOwnProperty.call(obj, prop);
};
$jscomp.polyfill("WeakMap", function(NativeWeakMap) {
  function isConformant() {
    if (!NativeWeakMap || !Object.seal) {
      return false;
    }
    try {
      var x = Object.seal({});
      var y = Object.seal({});
      var map = new NativeWeakMap([[x, 2], [y, 3]]);
      if (map.get(x) != 2 || map.get(y) != 3) {
        return false;
      }
      map["delete"](x);
      map.set(y, 4);
      return !map.has(x) && map.get(y) == 4;
    } catch (err) {
      return false;
    }
  }
  function WeakMapMembership() {
  }
  function isValidKey(key) {
    var type = typeof key;
    return type === "object" && key !== null || type === "function";
  }
  function insert(target) {
    if (!$jscomp.owns(target, prop)) {
      var obj = new WeakMapMembership();
      $jscomp.defineProperty(target, prop, {value:obj});
    }
  }
  function patch(name) {
    if ($jscomp.ISOLATE_POLYFILLS) {
      return;
    }
    var prev = Object[name];
    if (prev) {
      Object[name] = function(target) {
        if (target instanceof WeakMapMembership) {
          return target;
        } else {
          if (Object.isExtensible(target)) {
            insert(target);
          }
          return prev(target);
        }
      };
    }
  }
  if ($jscomp.USE_PROXY_FOR_ES6_CONFORMANCE_CHECKS) {
    if (NativeWeakMap && $jscomp.ES6_CONFORMANCE) {
      return NativeWeakMap;
    }
  } else {
    if (isConformant()) {
      return NativeWeakMap;
    }
  }
  var prop = "$jscomp_hidden_" + Math.random();
  patch("freeze");
  patch("preventExtensions");
  patch("seal");
  var index = 0;
  var PolyfillWeakMap = function(opt_iterable) {
    this.id_ = (index = index + (Math.random() + 1)).toString();
    if (opt_iterable) {
      var iter = $jscomp.makeIterator(opt_iterable);
      var entry;
      for (; !(entry = iter.next()).done;) {
        var item = entry.value;
        this.set(item[0], item[1]);
      }
    }
  };
  PolyfillWeakMap.prototype.set = function(key, value) {
    if (!isValidKey(key)) {
      throw new Error("Invalid WeakMap key");
    }
    insert(key);
    if (!$jscomp.owns(key, prop)) {
      throw new Error("WeakMap key fail: " + key);
    }
    key[prop][this.id_] = value;
    return this;
  };
  PolyfillWeakMap.prototype.get = function(key) {
    return isValidKey(key) && $jscomp.owns(key, prop) ? key[prop][this.id_] : undefined;
  };
  PolyfillWeakMap.prototype.has = function(key) {
    return isValidKey(key) && $jscomp.owns(key, prop) && $jscomp.owns(key[prop], this.id_);
  };
  PolyfillWeakMap.prototype["delete"] = function(key) {
    if (!isValidKey(key) || !$jscomp.owns(key, prop) || !$jscomp.owns(key[prop], this.id_)) {
      return false;
    }
    return delete key[prop][this.id_];
  };
  return PolyfillWeakMap;
}, "es6", "es3");
$jscomp.MapEntry = function() {
  this.previous;
  this.next;
  this.head;
  this.key;
  this.value;
};
$jscomp.polyfill("Map", function(NativeMap) {
  function isConformant() {
    if ($jscomp.ASSUME_NO_NATIVE_MAP || !NativeMap || typeof NativeMap != "function" || !NativeMap.prototype.entries || typeof Object.seal != "function") {
      return false;
    }
    try {
      NativeMap = NativeMap;
      var key = Object.seal({x:4});
      var map = new NativeMap($jscomp.makeIterator([[key, "s"]]));
      if (map.get(key) != "s" || map.size != 1 || map.get({x:4}) || map.set({x:4}, "t") != map || map.size != 2) {
        return false;
      }
      var iter = map.entries();
      var item = iter.next();
      if (item.done || item.value[0] != key || item.value[1] != "s") {
        return false;
      }
      item = iter.next();
      if (item.done || item.value[0].x != 4 || item.value[1] != "t" || !iter.next().done) {
        return false;
      }
      return true;
    } catch (err) {
      return false;
    }
  }
  if ($jscomp.USE_PROXY_FOR_ES6_CONFORMANCE_CHECKS) {
    if (NativeMap && $jscomp.ES6_CONFORMANCE) {
      return NativeMap;
    }
  } else {
    if (isConformant()) {
      return NativeMap;
    }
  }
  var idMap = new WeakMap();
  var DATA = 0;
  var HEAD = 1;
  var PolyfillMap = function(opt_iterable) {
    this[DATA] = {};
    this[HEAD] = createHead();
    this.size = 0;
    if (opt_iterable) {
      var iter = $jscomp.makeIterator(opt_iterable);
      var entry;
      for (; !(entry = iter.next()).done;) {
        var item = entry.value;
        this.set(item[0], item[1]);
      }
    }
  };
  PolyfillMap.prototype.set = function(key, value) {
    key = key === 0 ? 0 : key;
    var r = maybeGetEntry(this, key);
    if (!r.list) {
      r.list = this[DATA][r.id] = [];
    }
    if (!r.entry) {
      r.entry = {next:this[HEAD], previous:this[HEAD].previous, head:this[HEAD], key:key, value:value};
      r.list.push(r.entry);
      this[HEAD].previous.next = r.entry;
      this[HEAD].previous = r.entry;
      this.size++;
    } else {
      r.entry.value = value;
    }
    return this;
  };
  PolyfillMap.prototype["delete"] = function(key) {
    var r = maybeGetEntry(this, key);
    if (r.entry && r.list) {
      r.list.splice(r.index, 1);
      if (!r.list.length) {
        delete this[DATA][r.id];
      }
      r.entry.previous.next = r.entry.next;
      r.entry.next.previous = r.entry.previous;
      r.entry.head = null;
      this.size--;
      return true;
    }
    return false;
  };
  PolyfillMap.prototype.clear = function() {
    this[DATA] = {};
    this[HEAD] = this[HEAD].previous = createHead();
    this.size = 0;
  };
  PolyfillMap.prototype.has = function(key) {
    return !!maybeGetEntry(this, key).entry;
  };
  PolyfillMap.prototype.get = function(key) {
    var entry = maybeGetEntry(this, key).entry;
    return entry && entry.value;
  };
  PolyfillMap.prototype.entries = function() {
    return makeIterator(this, function(entry) {
      return [entry.key, entry.value];
    });
  };
  PolyfillMap.prototype.keys = function() {
    return makeIterator(this, function(entry) {
      return entry.key;
    });
  };
  PolyfillMap.prototype.values = function() {
    return makeIterator(this, function(entry) {
      return entry.value;
    });
  };
  PolyfillMap.prototype.forEach = function(callback, opt_thisArg) {
    var iter = this.entries();
    var item;
    for (; !(item = iter.next()).done;) {
      var entry = item.value;
      callback.call(opt_thisArg, entry[1], entry[0], this);
    }
  };
  PolyfillMap.prototype[Symbol.iterator] = PolyfillMap.prototype.entries;
  var maybeGetEntry = function(map, key) {
    var id = getId(key);
    var list = map[DATA][id];
    if (list && $jscomp.owns(map[DATA], id)) {
      var index = 0;
      for (; index < list.length; index++) {
        var entry = list[index];
        if (key !== key && entry.key !== entry.key || key === entry.key) {
          return {id:id, list:list, index:index, entry:entry};
        }
      }
    }
    return {id:id, list:list, index:-1, entry:undefined};
  };
  var makeIterator = function(map, func) {
    var entry = map[HEAD];
    return $jscomp.iteratorPrototype(function() {
      if (entry) {
        for (; entry.head != map[HEAD];) {
          entry = entry.previous;
        }
        for (; entry.next != entry.head;) {
          entry = entry.next;
          return {done:false, value:func(entry)};
        }
        entry = null;
      }
      return {done:true, value:void 0};
    });
  };
  var createHead = function() {
    var head = {};
    head.previous = head.next = head.head = head;
    return head;
  };
  var mapIndex = 0;
  var getId = function(obj) {
    var type = obj && typeof obj;
    if (type == "object" || type == "function") {
      obj = obj;
      if (!idMap.has(obj)) {
        var id = "" + ++mapIndex;
        idMap.set(obj, id);
        return id;
      }
      return idMap.get(obj);
    }
    return "p_" + obj;
  };
  return PolyfillMap;
}, "es6", "es3");
$jscomp.polyfill("Map.groupBy", function(orig) {
  if (orig) {
    return orig;
  }
  var polyfill = function(items, callbackFn) {
    if (typeof callbackFn !== "function") {
      throw new TypeError("callbackFn must be a function");
    }
    var result = new Map();
    var index = 0;
    var iter = $jscomp.makeIterator(items);
    var entry = iter.next();
    for (; !entry.done; entry = iter.next()) {
      var item = entry.value;
      var key = callbackFn(item, index++);
      var group = result.get(key);
      if (!group) {
        group = [];
        result.set(key, group);
      }
      group.push(item);
    }
    return result;
  };
  return polyfill;
}, "es_next", "es3");
$jscomp.polyfill("Set", function(NativeSet) {
  function isConformant() {
    if ($jscomp.ASSUME_NO_NATIVE_SET || !NativeSet || typeof NativeSet != "function" || !NativeSet.prototype.entries || typeof Object.seal != "function") {
      return false;
    }
    try {
      NativeSet = NativeSet;
      var value = Object.seal({x:4});
      var set = new NativeSet($jscomp.makeIterator([value]));
      if (!set.has(value) || set.size != 1 || set.add(value) != set || set.size != 1 || set.add({x:4}) != set || set.size != 2) {
        return false;
      }
      var iter = set.entries();
      var item = iter.next();
      if (item.done || item.value[0] != value || item.value[1] != value) {
        return false;
      }
      item = iter.next();
      if (item.done || item.value[0] == value || item.value[0].x != 4 || item.value[1] != item.value[0]) {
        return false;
      }
      return iter.next().done;
    } catch (err) {
      return false;
    }
  }
  if ($jscomp.USE_PROXY_FOR_ES6_CONFORMANCE_CHECKS) {
    if (NativeSet && $jscomp.ES6_CONFORMANCE) {
      return NativeSet;
    }
  } else {
    if (isConformant()) {
      return NativeSet;
    }
  }
  var PolyfillSet = function(opt_iterable) {
    this.map_ = new Map();
    if (opt_iterable) {
      var iter = $jscomp.makeIterator(opt_iterable);
      var entry;
      for (; !(entry = iter.next()).done;) {
        var item = entry.value;
        this.add(item);
      }
    }
    this.size = this.map_.size;
  };
  PolyfillSet.prototype.add = function(value) {
    value = value === 0 ? 0 : value;
    this.map_.set(value, value);
    this.size = this.map_.size;
    return this;
  };
  PolyfillSet.prototype["delete"] = function(value) {
    var result = this.map_["delete"](value);
    this.size = this.map_.size;
    return result;
  };
  PolyfillSet.prototype.clear = function() {
    this.map_.clear();
    this.size = 0;
  };
  PolyfillSet.prototype.has = function(value) {
    return this.map_.has(value);
  };
  PolyfillSet.prototype.entries = function() {
    return this.map_.entries();
  };
  PolyfillSet.prototype.values = function() {
    return this.map_.values();
  };
  PolyfillSet.prototype.keys = PolyfillSet.prototype.values;
  PolyfillSet.prototype[Symbol.iterator] = PolyfillSet.prototype.values;
  PolyfillSet.prototype.forEach = function(callback, opt_thisArg) {
    var set = this;
    this.map_.forEach(function(value) {
      return callback.call(opt_thisArg, value, value, set);
    });
  };
  return PolyfillSet;
}, "es6", "es3");
(function() {
  function callRequireCallback(callback, opt_module) {
    var oldPath = currentModulePath;
    try {
      if (opt_module) {
        currentModulePath = opt_module.id;
        callback.call(opt_module, createRequire(opt_module), opt_module.exports, opt_module);
      } else {
        callback($jscomp.require);
      }
    } finally {
      currentModulePath = oldPath;
    }
  }
  function normalizePath(path) {
    var components = path.split("/");
    var i = 0;
    for (; i < components.length;) {
      if (components[i] == ".") {
        components.splice(i, 1);
      } else if (i && components[i] == ".." && components[i - 1] && components[i - 1] != "..") {
        components.splice(--i, 2);
      } else {
        i++;
      }
    }
    return components.join("/");
  }
  function getCacheEntry(id) {
    var cacheEntry = moduleCache.get(id);
    if (cacheEntry === undefined) {
      throw new Error("Module " + id + " does not exist.");
    }
    return cacheEntry;
  }
  function maybeNormalizePath(root, absOrRelativePath) {
    if (absOrRelativePath.startsWith("./") || absOrRelativePath.startsWith("../")) {
      return normalizePath(root + "/../" + absOrRelativePath);
    } else {
      return absOrRelativePath;
    }
  }
  function createRequire(opt_module) {
    function require(absOrRelativePath) {
      var absPath = absOrRelativePath;
      if (opt_module) {
        absPath = maybeNormalizePath(opt_module.id, absPath);
      }
      return getCacheEntry(absPath).load();
    }
    function requireEnsure(requires, callback) {
      if (currentModulePath) {
        var i = 0;
        for (; i < requires.length; i++) {
          requires[i] = maybeNormalizePath(currentModulePath, requires[i]);
        }
      }
      var blockingRequires = [];
      i = 0;
      for (; i < requires.length; i++) {
        var required = moduleCache.get(requires[i]);
        if (!required || required.blockingDeps.size) {
          blockingRequires.push(requires[i]);
        }
      }
      if (blockingRequires.length) {
        var requireSet = new Set(blockingRequires);
        var callbackEntry = new CallbackEntry(requireSet, callback);
        requireSet.forEach(function(require) {
          var arr = ensureMap.get(require);
          if (!arr) {
            arr = [];
            ensureMap.set(require, arr);
          }
          arr.push(callbackEntry);
        });
      } else {
        callback(require);
      }
    }
    require.ensure = requireEnsure;
    return require;
  }
  function markAvailable(absModulePath) {
    var ensures = ensureMap.get(absModulePath);
    if (ensures) {
      var i = 0;
      for (; i < ensures.length; i++) {
        var entry = ensures[i];
        entry.requireSet["delete"](absModulePath);
        if (!entry.requireSet.size) {
          ensures.splice(i--, 1);
          callRequireCallback(entry.callback);
        }
      }
      if (!ensures.length) {
        ensureMap["delete"](absModulePath);
      }
    }
  }
  function getTransitiveBlockingDepsOf(moduleId) {
    var cacheEntry = moduleCache.get(moduleId);
    var blocking = new Set();
    if (cacheEntry) {
      cacheEntry.blockingDeps.forEach(function(dep) {
        getTransitiveBlockingDepsOf(dep).forEach(function(transitive) {
          blocking.add(transitive);
        });
      });
    } else {
      blocking.add(moduleId);
    }
    return blocking;
  }
  function addAsBlocking(blocked, blocker) {
    if (blocked.module.id != blocker) {
      var blockedModules = blockingModulePathToBlockedModules.get(blocker);
      if (!blockedModules) {
        blockedModules = new Set();
        blockingModulePathToBlockedModules.set(blocker, blockedModules);
      }
      blockedModules.add(blocked);
      blocked.blockingDeps.add(blocker);
    }
  }
  function removeAsBlocking(cacheEntry) {
    var blocked = blockingModulePathToBlockedModules.get(cacheEntry.module.id);
    if (blocked) {
      blockingModulePathToBlockedModules["delete"](cacheEntry.module.id);
      blocked.forEach(function(blockedCacheEntry) {
        blockedCacheEntry.blockingDeps["delete"](cacheEntry.module.id);
        cacheEntry.blockingDeps.forEach(function(blocker) {
          addAsBlocking(blockedCacheEntry, blocker);
        });
        if (!blockedCacheEntry.blockingDeps.size) {
          removeAsBlocking(blockedCacheEntry);
          markAvailable(blockedCacheEntry.module.id);
        }
      });
    }
  }
  var Module = function(id, opt_exports) {
    this.id = id;
    this.exports = opt_exports || {};
  };
  Module.prototype.exportAllFrom = function(other) {
    var module = this;
    var define = {};
    var key;
    for (key in other) {
      if (key == "default" || key in module.exports || key in define) {
        continue;
      }
      define[key] = {enumerable:true, get:function(key) {
        return function() {
          return other[key];
        };
      }(key)};
    }
    $jscomp.global.Object.defineProperties(module.exports, define);
  };
  var CacheEntry = function(def, module, path) {
    this.def = def;
    this.module = module;
    this.path = path;
    this.blockingDeps = new Set();
  };
  CacheEntry.prototype.load = function() {
    if (this.def) {
      var def = this.def;
      this.def = null;
      callRequireCallback(def, this.module);
    }
    return this.module.exports;
  };
  var moduleCache = new Map();
  var currentModulePath = "";
  $jscomp.getCurrentModulePath = function() {
    return currentModulePath;
  };
  var ensureMap = new Map();
  var CallbackEntry = function(requireSet, callback) {
    this.requireSet = requireSet;
    this.callback = callback;
  };
  $jscomp.require = createRequire();
  $jscomp.hasModule = function(id) {
    return moduleCache.has(id);
  };
  $jscomp.registerModule = function(moduleDef, absModulePath, opt_shallowDeps) {
    if (moduleCache.has(absModulePath)) {
      throw new Error("Module " + absModulePath + " has already been registered.");
    }
    if (currentModulePath) {
      throw new Error("Cannot nest modules.");
    }
    var shallowDeps = opt_shallowDeps || [];
    var i = 0;
    for (; i < shallowDeps.length; i++) {
      shallowDeps[i] = maybeNormalizePath(absModulePath, shallowDeps[i]);
    }
    var blockingDeps = new Set();
    i = 0;
    for (; i < shallowDeps.length; i++) {
      getTransitiveBlockingDepsOf(shallowDeps[i]).forEach(function(transitive) {
        blockingDeps.add(transitive);
      });
    }
    blockingDeps["delete"](absModulePath);
    var cacheEntry = new CacheEntry(moduleDef, new Module(absModulePath), absModulePath);
    moduleCache.set(absModulePath, cacheEntry);
    blockingDeps.forEach(function(blocker) {
      addAsBlocking(cacheEntry, blocker);
    });
    if (!blockingDeps.size) {
      markAvailable(cacheEntry.module.id);
    }
    removeAsBlocking(cacheEntry);
  };
  var blockingModulePathToBlockedModules = new Map();
  $jscomp.registerAndLoadModule = function(moduleDef, absModulePath, shallowDeps) {
    $jscomp.require.ensure([absModulePath], function(require) {
      require(absModulePath);
    });
    $jscomp.registerModule(moduleDef, absModulePath, shallowDeps);
  };
  $jscomp.clearModules = function() {
    moduleCache.clear();
  };
})();
