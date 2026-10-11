/** @unrestricted */
class A {
  /** @return {number} */
  get '\uD800'() { return 1; }
  /** @param {string} x */
  set '\uD800'(x) {}
}
var CLOSURE_TOGGLE_ORDINALS = {'\uD800': 1, 'x\uDBFF': 2, 'x\uDBFF': 3};
alert(new A());
