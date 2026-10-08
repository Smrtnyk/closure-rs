## TypeCheckTest (com.google.javascript.jscomp.TypeCheckTest)

Post-call assertions classified `rust_unit_test` in `corpus/unit/postcall/TypeCheckTest.json` (D-015 (a)).
Each method below makes one hooked call, `parseAndTypeCheck(...)` or `parseAndTypeCheckWithScope(...)`
(record kind `type_check`, api `parseAndTypeCheckWithScope`, comparison `observed_only`; HARNESS.md
"parseAndTypeCheckWithScope"), and then inspects the Java objects the call returns: `JSType`s on nodes of
the type-checked AST and in the global `TypedScope`. Replay checks the call itself (outcome, no error
after InferConsts, errors and warnings of TypeCheck, the AstValidator JSTYPE check); the records carry no
post-call snapshot of type-registry state (`postCall` and `testFieldsAfter` exist only for
`compiler_test_case` records), so these assertions are ported as Rust unit tests next to the TypeCheck /
TypedScopeCreator / TypeInference implementation. Each record is already excluded from gate (a) and counted
in (e) as `observed_only` notCaptured; this classification does not add an exclusion.

The inputs are each method's record in `corpus/unit/records/TypeCheckTest.jsonl.gz` (`inputs`: externs and the
`[testcode]` source; `options`). Notation: `root` = `TypeCheckResult.root` (the first SCRIPT of the JS
root), `scope` = `TypeCheckResult.scope` (the global TypedScope that `processForTesting` returns);
`instanceType(root)` = `getInstanceType`: `root.getFirstChild().getJSType()` is non-null, a constructor
`FunctionType`, and its instance type is used; `checkObjectType(t, p, T)` = `t.hasProperty(p)` and
`t.getPropertyType(p)` equals `T`; `morePropsThanObject(t, n)` = `assertHasXMorePropertiesThanNativeObject(t, n)`;
"equals" = `assertTypeEquals` (JSType equality); `x == "..."` compares strings.

- TypeCheckTest#testAddMethodsPrototypeTwoWays: t = instanceType(root); t.toString() == "A"; morePropsThanObject(t, 3); checkObjectType(t, m1, number), (t, m2, boolean), (t, m3, string)
- TypeCheckTest#testAddSingletonGetter: o = (ObjectType) root.getFirstChild().getJSType(); o.getPropertyType("getInstance").toString() == "function(): Foo"; o.getPropertyType("instance_").toString() == "Foo"
- TypeCheckTest#testAddingMethodsPrototypeIdiomAndObjectLiteralSimpleNamespace: t = instanceType(root); morePropsThanObject(t, 2); checkObjectType(t, m1, number), (t, m2, boolean)
- TypeCheckTest#testAddingMethodsUsingPrototypeIdiomComplexNamespace1: via the private helper testAddingMethodsUsingPrototypeIdiomComplexNamespace(p): goog = (ObjectType) scope.getVar("goog").getType(); morePropsThanObject(goog, 1); googA = goog.getPropertyType("A") is a non-null FunctionType; classA = its instance type; morePropsThanObject(classA, 1); checkObjectType(classA, m1, number)
- TypeCheckTest#testAddingMethodsUsingPrototypeIdiomComplexNamespace2: same helper and checks as ComplexNamespace1, on the `/** @constructor */goog.A = function() {}` input
- TypeCheckTest#testAddingMethodsUsingPrototypeIdiomSimpleNamespace: t = instanceType(root); morePropsThanObject(t, 1); checkObjectType(t, m1, number)
- TypeCheckTest#testAssignToUntypedProperty: node = root.getLastChild().getFirstChild() (the `(new Foo).a` GETPROP): its JSType is not unknown and isNumber()
- TypeCheckTest#testAssignToUntypedVariable: node = first child of the ASSIGN in `z = 1`: its JSType is not unknown and toString() == "number"
- TypeCheckTest#testCallArrayConstructorAsFunction: root.getFirstFirstChild() (the `Array()` CALL) has JSType equal to the native Array type
- TypeCheckTest#testCallDateConstructorAsFunction: root.getFirstFirstChild() (the `Date()` CALL) has JSType equal to the native string type
- TypeCheckTest#testCallErrorConstructorAsFunction: call = root.getFirstFirstChild() is a CALL; call.getJSType() equals the instance type of the callee's FunctionType (Error)
- TypeCheckTest#testCast4Types: the cast expression `new base()` under the CAST: getJSType().toString() == "derived" and getJSTypeBeforeCast().toString() == "base"
- TypeCheckTest#testComplexNamespace: googScope = scope.getVar("goog").getType() is an ObjectType with property foo and without bar; root.getFirstChild() is a VAR whose NAME has the same JSType instance; the `goog` NAMEs under both GETPROPs carry that same instance; googScope.getPropertyType("foo") is an ObjectType; the `goog.foo` GETPROP's JSType is an ObjectType without foo, with bar, and bar's type equals number
- TypeCheckTest#testConstructorType7: scope.getVar("A").getType() is a FunctionType with getReferenceName() == "A"
- TypeCheckTest#testDeclareBuiltInConstructor: root.getLastChild().getFirstChild() (the `.charAt(0)` CALL) has JSType equal to the native string type
- TypeCheckTest#testDeclaredNativeTypeEquality: root.getFirstChild() (the redeclared `function Object`) has JSType equal to registry.getNativeType(OBJECT_FUNCTION_TYPE)
- TypeCheckTest#testDontAddMethodsIfNoConstructor: root.getFirstChild().getJSType().toString() == "function(): undefined"; the native Function type's property types of m1 and m2 equal the unknown type
- TypeCheckTest#testEnum21: the returned NAME x in f (root.getLastChild().getLastChild().getLastChild().getLastChild()): its JSType (enum element !E) is not isObject() and not isNullable()
- TypeCheckTest#testExtendBuiltInType1: root.getLastChild().getFirstChild() (the `(new String("x")).substr(0,1)` CALL) has JSType equal to the native string type
- TypeCheckTest#testExtendBuiltInType2: root.getLastChild().getFirstChild() (the `"x".substr(0,1)` CALL) has JSType equal to the native string type
- TypeCheckTest#testExtendFunction1: root.getLastChild().getLastChild() (the `(new Function()).f()` CALL) has JSType equal to the native number type
- TypeCheckTest#testExtendFunction2: root.getLastChild().getLastChild() (the `(function() {}).f()` CALL) has JSType equal to the native number type
- TypeCheckTest#testFlowScopeBug1: the ADD node `i + a` in the for condition (root.getFirstChild().getLastChild().getLastChild().getFirstChild().getNext().getFirstChild()) has JSType equal to registry.getNativeType(NUMBER_TYPE)
- TypeCheckTest#testFlowScopeBug2: the `afoo` reference in the loop body (six getLastChild() steps from root) has JSType equal to registry.createNullableType(registry.getGlobalType("Foo"))
- TypeCheckTest#testGatherProperyWithoutAnnotation1: type = root.getLastChild().getLastChild().getJSType() (the trailing `t`): not unknown, an ObjectType, and hasProperty("x") is false
- TypeCheckTest#testGatherProperyWithoutAnnotation2: type = root.getLastChild().getLastChild().getJSType() (the trailing `t`): not unknown, equal to the native Object type, an ObjectType, and hasProperty("x") is false
- TypeCheckTest#testGoodExtends4: subTypeName = root.getLastChild().getLastChild().getFirstChild() has qualified name "goog.Derived"; its sibling's FunctionType has instance type "goog.Derived"; getPrototype().getImplicitPrototype().toString() == "goog.Base"
- TypeCheckTest#testNamespacedConstructor: root.getLastChild() (function foo) has a FunctionType whose return type is an ObjectType with getReferenceName() == "goog.MyClass"
- TypeCheckTest#testNew12: scope.getVar("a").getType() equals the native Array type
- TypeCheckTest#testNew13: scope.getVar("a").getType() is an ObjectType with toString() == "FooBar"
- TypeCheckTest#testNew14: scope.getVar("a").getType() is an ObjectType with toString() == "FooBar" (constructor declared as `var FooBar = function(){}`)
- TypeCheckTest#testNew15: scope.getVar("a").getType() is an ObjectType with toString() == "goog.A"
- TypeCheckTest#testNew6: scope.getVar("a").getType() is an ObjectType whose getConstructor().getReferenceName() == "A"
- TypeCheckTest#testObjectLiteral: nameNode = root.getFirstFirstChild() is a NAME and its child an OBJECTLIT; the OBJECTLIT's ObjectType has property types m1 = number, m2 = string; nameNode.getJSType() equals the OBJECTLIT's type
- TypeCheckTest#testPrototypePropertyReference: compiler error and warning counts are 0 (also compared by replay through observed.errors/warnings); scope.getVar("Foo").getType() is a FunctionType whose prototype's property type bar has toString() == "function(this:Foo, number): undefined"
- TypeCheckTest#testPrototypePropertyTypes: t = instanceType(root); morePropsThanObject(t, 6); checkObjectType(t, m1, string), (t, m2, (Object|null)), (t, m3, boolean), (t, m4, string), (t, m5, number), (t, m6, boolean)
- TypeCheckTest#testResolutionViaRegistry5: type = root.getLastChild().getLastChild().getJSType() (the trailing `u.T`): not unknown, a FunctionType whose instance type has getReferenceName() == "u.T"
- TypeCheckTest#testScoping10: the global scope declares a and does not declare b (assertScope); scope.getVar("a").getType().toString() == "function(): undefined"
- TypeCheckTest#testSheqRefinedScope: nodeC = the `b.p()` CALL inside the `if (a === b)` block (six getLastChild() steps from root): its JSType isNumber(); nodeC.getFirstFirstChild() (the refined `b`) has JSType toString() == "B"
- TypeCheckTest#testUndefinedVar: root.getFirstFirstChild() (the NAME undefined) has JSType equal to registry.getNativeType(VOID_TYPE)
- TypeCheckTest#testValueTypeBuiltInPrototypePropertyType: root.getFirstFirstChild() (the `"x".charAt(0)` CALL) has JSType equal to the native string type
- TypeCheckTest#testVar1: scope.getVar("a").getType() equals the union (string|null)
- TypeCheckTest#testVar3: scope.getVar("a").getType() equals the native number type
- TypeCheckTest#testVar4: scope.getVar("a").getType() equals the union (string|number)
