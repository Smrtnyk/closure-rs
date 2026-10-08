/**
 * @fileoverview Externs for the three.js benchmark (bench/README.md): the one WebXR constructor
 * three.js uses that Closure Compiler's default externs do not declare. Without it, ADVANCED stops
 * at JSC_UNDEFINED_VARIABLE (an error) before optimizing anything.
 * @externs
 */

/**
 * @constructor
 * @param {*} session
 * @param {*} context
 * @param {*=} layerInit
 */
function XRWebGLLayer(session, context, layerInit) {}

/**
 * @param {*} session
 * @return {number}
 */
XRWebGLLayer.getNativeFramebufferScaleFactor = function(session) {};
