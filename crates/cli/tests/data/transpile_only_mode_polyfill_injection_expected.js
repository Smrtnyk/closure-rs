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
$jscomp.arrayIteratorImpl = function(array) {
  var index = 0;
  return function() {
    if (index < array.length) {
      return {done: false, value: array[index++]};
    } else {
      return {done: true};
    }
  };
};
$jscomp.arrayIterator = function(array) {
  return {next: $jscomp.arrayIteratorImpl(array)};
};
$jscomp.getIteratorPrototype = function() {
  return typeof Iterator != 'undefined' &&
      Iterator.prototype &&
      !Iterator.prototype.next ?
      Iterator.prototype :
      null;
};
$jscomp.objectCreate =
    $jscomp.ASSUME_ES5 || typeof Object.create == 'function' ?
    Object.create :
    function(prototype) {
      var ctor = function() {};
      ctor.prototype = prototype;
      return new ctor;
    };
$jscomp.initSymbol = function() {};
$jscomp.polyfill('Symbol', function(orig) {
  if (orig) return orig;
  var SymbolClass = function(id, opt_description) {
    this.$jscomp$symbol$id_ = id;
    this.description;
    $jscomp.defineProperty(this, 'description', {
      configurable: true,
      writable: true,
      value: opt_description
    });
  };
  SymbolClass.prototype.toString = function() {
    return this.$jscomp$symbol$id_;
  };
  var BIN_ID = Math.random() * 1E9 >>> 0;
  var SYMBOL_PREFIX = 'jscomp_symbol_' + BIN_ID + '_';
  var counter = 0;
  var symbolPolyfill = function(opt_description) {
    if (this instanceof symbolPolyfill) {
      throw new TypeError('Symbol is not a constructor');
    }
    return new SymbolClass(
        SYMBOL_PREFIX + (opt_description || '') + '_' + counter++,
        opt_description);
  };
  return symbolPolyfill;
}, 'es6', 'es3');
$jscomp.polyfill('Symbol.iterator', function(orig) {
  if (orig) return orig;
  var symbolIterator = Symbol('Symbol.iterator');
  $jscomp.defineProperty(Array.prototype, symbolIterator, {
    configurable: true,
    writable: true,
    value: function() {
      return $jscomp.iteratorPrototype($jscomp.arrayIteratorImpl(this));
    }
  });
  return symbolIterator;
}, 'es6', 'es3');
$jscomp.polyfill('Symbol.toStringTag', function(orig) {
  if (orig) return orig;
  return Symbol('Symbol.toStringTag');
}, 'es6', 'es3');
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
$jscomp.polyfill('Iterator', function(localIterator) {
  var getPrototypeOf = Object.getPrototypeOf;
  var arrIterator = typeof Symbol != 'undefined' && Symbol.iterator && [][Symbol.iterator] ?
      [][Symbol.iterator]() :
      null;
  var ArrayIteratorProto = arrIterator && getPrototypeOf ? getPrototypeOf(arrIterator) : null;
  var IteratorPrototype = ArrayIteratorProto && getPrototypeOf ? getPrototypeOf(ArrayIteratorProto) : null;
  if (IteratorPrototype === Object.prototype) IteratorPrototype = null;
  if (localIterator && IteratorPrototype && localIterator.prototype === IteratorPrototype) return localIterator;
  var Iterator = function() {
    if (!(this instanceof Iterator) || this.constructor === Iterator) {
      throw new TypeError('Abstract Iterator not directly constructable');
    }
  };
  if (IteratorPrototype) Iterator.prototype = IteratorPrototype;
  $jscomp.defineProperty(Iterator.prototype, 'constructor', {
    configurable: true,
    writable: true,
    value: Iterator
  });
  if (!Iterator.prototype[Symbol.iterator]) {
    Iterator.prototype[Symbol.iterator] = function() {
      return this;
    };
  }
  return Iterator;
}, 'es_next', 'es3');
$jscomp.iteratorClose = function(iterator, iterResult) {
  var returnMethod;
  if (iterResult && !iterResult.done && (returnMethod = iterator['return'])) {
    returnMethod.call(iterator);
  }
};
$jscomp.checkIsObject = function(x) {
  if (Object(x) !== x) {
    throw new TypeError('Expected an object');
  }
  return x;
};
$jscomp.iteratorCloseAbrupt = function(iterator) {
  try {
    var returnMethod = iterator['return'];
    if (typeof returnMethod == 'function') {
      returnMethod.call(iterator);
    }
  } catch (ignored) {
  }
};
$jscomp.iteratorCloseNormal = function(iterator) {
  var returnMethod = iterator['return'];
  if (returnMethod != null) {
    if (typeof returnMethod != 'function') {
      throw new TypeError('Iterator return is not a function');
    }
    $jscomp.checkIsObject(returnMethod.call(iterator));
  }
};
$jscomp.iteratorResult = function(opt_value, opt_done) {
  if (opt_done == null) opt_done = true;
  return {value: opt_value, done: opt_done};
};
$jscomp.ITERATOR_STATE_SUSPENDED_START = 1;
$jscomp.ITERATOR_STATE_SUSPENDED_YIELD = 2;
$jscomp.ITERATOR_STATE_EXECUTING = 3;
$jscomp.ITERATOR_STATE_COMPLETED = 4;
$jscomp.getLazyIteratorState = function(iterator) {
  if (!iterator || iterator.self_ !== iterator) {
    throw new TypeError('Method called on incompatible object');
  }
  var state = iterator.state_;
  if (state === $jscomp.ITERATOR_STATE_EXECUTING) {
    throw new TypeError('Generator is already running');
  }
  return state;
};
$jscomp.lazyIteratorPrototype = null;
$jscomp.createLazyIterator = function(iterator, stepFn) {
  var prototype = $jscomp.lazyIteratorPrototype;
  if (!prototype) {
    var resume = function(helper, isReturn) {
      var state = $jscomp.getLazyIteratorState(helper);
      if (state != $jscomp.ITERATOR_STATE_COMPLETED) {
        helper.state_ =
            isReturn && state == $jscomp.ITERATOR_STATE_SUSPENDED_START ?
            $jscomp.ITERATOR_STATE_COMPLETED :
            $jscomp.ITERATOR_STATE_EXECUTING;
        try {
          if (isReturn) {
            $jscomp.iteratorCloseNormal(helper.iter_);
          } else {
            var res = helper.step_();
            if (res) {
              helper.state_ = $jscomp.ITERATOR_STATE_SUSPENDED_YIELD;
              return res;
            }
          }
        } finally {
          if (helper.state_ != $jscomp.ITERATOR_STATE_SUSPENDED_YIELD) {
            helper.state_ = $jscomp.ITERATOR_STATE_COMPLETED;
            helper.step_ = helper.iter_ = null;
          }
        }
      }
      return $jscomp.iteratorResult();
    };
    prototype = $jscomp.lazyIteratorPrototype =
        $jscomp.objectCreate(Iterator.prototype);
    prototype.next = function() {
      return resume(this, false);
    };
    prototype.return = function() {
      return resume(this, true);
    };
    prototype[Symbol.toStringTag] = 'Iterator Helper';
  }
  var lazyIterator = $jscomp.objectCreate(prototype);
  lazyIterator.self_ = lazyIterator;
  lazyIterator.state_ = $jscomp.ITERATOR_STATE_SUSPENDED_START;
  lazyIterator.iter_ = iterator;
  lazyIterator.step_ = stepFn;
  return lazyIterator;
};
$jscomp.validateIteratorCallback = function(iterator, callback) {
  $jscomp.checkIsObject(iterator);
  if (typeof callback != 'function') {
    $jscomp.iteratorCloseAbrupt(iterator);
    throw new TypeError('callback must be a function');
  }
};
$jscomp.iteratorDo = function(iterator, callback, step) {
  $jscomp.validateIteratorCallback(iterator, callback);
  var nextMethod = iterator.next;
  var index = 0;
  for (var nextResult;
      !(nextResult = $jscomp.checkIsObject(nextMethod.call(iterator))).done;) {
    try {
      var stop = step(nextResult.value, index++);
    } catch (e) {
      try {
        $jscomp.iteratorClose(iterator, nextResult);
      } catch (ignored) {
      }
      throw e;
    }
    if (stop) {
      var returnMethod = iterator['return'];
      if (returnMethod != null) {
        $jscomp.checkIsObject(returnMethod.call(iterator));
      }
      return;
    }
  }
};
$jscomp.polyfill('Iterator.prototype.find', function(orig) {
  return orig || function(fn) {
    var res;
    $jscomp.iteratorDo(this, fn, function(v, i) {
      if (fn(v, i)) {
        res = v;
        return true;
      }
    });
    return res;
  };
}, 'es_next', 'es3');
var arr = [1, 2, 3];
var found = arr.find(function(element) {
  return element > 10;
});
