var $jscomp = $jscomp || {};
$jscomp.scope = {};
$jscomp.findInternal = function(array, callback, thisArg) {
  if (array instanceof String) array = String(array);
  var len = array.length;
  for (var i = 0; i < len; i++) {
    var value = array[i];
    if (callback.call(thisArg, value, i, array)) return {
        i: i, v: value
      }
  }
  return {
    i: -1, v: void 0
  }
};
$jscomp.ASSUME_ES2020 = false;
$jscomp.ASSUME_ES6 = $jscomp.ASSUME_ES2020;
$jscomp.ASSUME_ES5 = $jscomp.ASSUME_ES6;
$jscomp.ASSUME_NO_NATIVE_MAP = false;
$jscomp.ASSUME_NO_NATIVE_SET = false;
$jscomp.ISOLATE_POLYFILLS = false;
$jscomp.FORCE_POLYFILL_PROMISE = false;
$jscomp.FORCE_POLYFILL_PROMISE_WHEN_NO_UNHANDLED_REJECTION = false;
$jscomp.INSTRUMENT_ASYNC_CONTEXT = true;
$jscomp.defineProperty =
    $jscomp.ASSUME_ES5 || typeof Object.defineProperties == 'function' ?
    Object.defineProperty :
    function(target, property, descriptor) {
      if (target == Array.prototype || target == Object.prototype)
        return target;
      target[property] = descriptor.value;
      return target
    };
$jscomp.getGlobal = function(passedInThis) {
  var possibleGlobals = [
    'object' == typeof globalThis && globalThis, passedInThis,
    'object' == typeof window && window, 'object' == typeof self && self,
    'object' == typeof global && global
  ];
  for (var i = 0; i < possibleGlobals.length; ++i) {
    var maybeGlobal = possibleGlobals[i];
    if (maybeGlobal && maybeGlobal['Math'] == Math) return maybeGlobal
  }
  return {
    valueOf: function() {
      throw new Error('Cannot find global object');
    }
  }.valueOf()
};
$jscomp.global = $jscomp.ASSUME_ES2020 ? globalThis : $jscomp.getGlobal(this);
$jscomp.IS_SYMBOL_NATIVE =
    typeof Symbol === 'function' && typeof Symbol('x') === 'symbol';
$jscomp.TRUST_ES6_POLYFILLS =
    !$jscomp.ISOLATE_POLYFILLS || $jscomp.IS_SYMBOL_NATIVE;
$jscomp.polyfills = {};
$jscomp.propertyToPolyfillSymbol = {};
$jscomp.POLYFILL_PREFIX = '$jscp$';
var $jscomp$lookupPolyfilledValue = function(
    target, property, isOptionalAccess) {
  if (isOptionalAccess && target == null) return undefined;
  var obfuscatedName = $jscomp.propertyToPolyfillSymbol[property];
  if (obfuscatedName == null) return target[property];
  var polyfill = target[obfuscatedName];
  return polyfill !== undefined ? polyfill : target[property]
};
$jscomp.TYPED_ARRAY_CLASSES = function() {
  var classes = [
    'Int8', 'Uint8', 'Uint8Clamped', 'Int16', 'Uint16', 'Int32', 'Uint32',
    'Float32', 'Float64'
  ];
  if ($jscomp.global.BigInt64Array) {
    classes.push('BigInt64');
    classes.push('BigUint64')
  }
  return classes
}();
$jscomp.polyfillTypedArrayMethod = function(
    methodName, polyfill, fromLang, toLang) {
  if (!polyfill) return;
  for (var i = 0; i < $jscomp.TYPED_ARRAY_CLASSES.length; i++) {
    var target =
        $jscomp.TYPED_ARRAY_CLASSES[i] + 'Array.prototype.' + methodName;
    if ($jscomp.ISOLATE_POLYFILLS)
      $jscomp.polyfillIsolated(target, polyfill, fromLang, toLang);
    else
      $jscomp.polyfillUnisolated(target, polyfill, fromLang, toLang)
  }
};
$jscomp.polyfill = function(target, polyfill, fromLang, toLang) {
  if (!polyfill) return;
  if ($jscomp.ISOLATE_POLYFILLS)
    $jscomp.polyfillIsolated(target, polyfill, fromLang, toLang);
  else
    $jscomp.polyfillUnisolated(target, polyfill, fromLang, toLang)
};
$jscomp.polyfillUnisolated = function(target, polyfill, fromLang, toLang) {
  var obj = $jscomp.global;
  var split = target.split('.');
  for (var i = 0; i < split.length - 1; i++) {
    var key = split[i];
    if (!(key in obj)) return;
    obj = obj[key]
  }
  var property = split[split.length - 1];
  var orig = obj[property];
  var impl = polyfill(orig);
  if (impl == orig || impl == null) return;
  $jscomp.defineProperty(
      obj, property, {configurable: true, writable: true, value: impl})
};
$jscomp.polyfillIsolated = function(target, polyfill, fromLang, toLang) {
  var split = target.split('.');
  var isSimpleName = split.length === 1;
  var root = split[0];
  if (!isSimpleName && root in $jscomp.polyfills)
    var ownerObject = $jscomp.polyfills;
  else
    ownerObject = $jscomp.global;
  for (var i = 0; i < split.length - 1; i++) {
    var key = split[i];
    if (!(key in ownerObject)) return;
    ownerObject = ownerObject[key]
  }
  var property = split[split.length - 1];
  var nativeImpl = $jscomp.IS_SYMBOL_NATIVE && fromLang === 'es6' ?
      ownerObject[property] :
      null;
  var impl = polyfill(nativeImpl);
  if (impl == null) return;
  if (isSimpleName)
    $jscomp.defineProperty(
        $jscomp.polyfills, property,
        {configurable: true, writable: true, value: impl});
  else if (impl !== nativeImpl) {
    if ($jscomp.propertyToPolyfillSymbol[property] === undefined) {
      var BIN_ID = Math.random() * 1E9 >>> 0;
      $jscomp.propertyToPolyfillSymbol[property] = $jscomp.IS_SYMBOL_NATIVE ?
          $jscomp.global['Symbol'](property) :
          $jscomp.POLYFILL_PREFIX + BIN_ID + '$' + property
    }
    var obfuscatedName = $jscomp.propertyToPolyfillSymbol[property];
    $jscomp.defineProperty(
        ownerObject, obfuscatedName,
        {configurable: true, writable: true, value: impl})
  }
};
$jscomp.polyfill('Array.prototype.find', function(orig) {
  if (orig) return orig;
  var polyfill = function(callback, opt_thisArg) {
    return $jscomp.findInternal(this, callback, opt_thisArg).v
  };
  return polyfill
}, 'es6', 'es3');
var arr = [1, 2, 3];
var found = arr.find(function(element) {
  return element > 10
});
