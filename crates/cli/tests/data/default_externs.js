var arguments;
/**
 * @constructor
 * @param {...*} var_args
 * @throws {Error}
 */
function Function(var_args) {}
/**
 * @param {...*} var_args
 * @return {*}
 */
Function.prototype.call = function(var_args) {};
/**
 * @constructor
 * @param {...*} var_args
 * @return {!Array}
 */
function Array(var_args) {}
/**
 * @param {*=} opt_begin
 * @param {*=} opt_end
 * @return {!Array}
 * @this {Object}
 */
Array.prototype.slice = function(opt_begin, opt_end) {};
/** @constructor */ function Window() {}
/** @type {string} */ Window.prototype.name;
/** @type {Window} */ var window;
/** @constructor */ function Element() {}
Element.prototype.offsetWidth;
/** @nosideeffects */ function noSideEffects() {}
/** @param {...*} x */ function alert(x) {}
function Symbol() {}

