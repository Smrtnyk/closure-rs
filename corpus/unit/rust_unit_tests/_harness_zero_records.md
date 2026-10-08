## Harness-based test classes with zero hooked calls (gate 0.2 (c))

Each class below is a concrete test class deriving from CompilerTestCase, IntegrationTestCase or CompilerTypeTestCase (static class-hierarchy check of the pristine test/ tree, `harness_hierarchy_classes()` in gates/lib/unit_gate02.py), but none of its tests calls a hooked harness API, so it produced no record. It is counted in the gate (c) denominator (the harness-based classes) and its tests are ported as ordinary Rust unit tests next to the code they cover.

- com.google.javascript.jscomp.ClosureReverseAbstractInterpreterTest: unit tests of ClosureReverseAbstractInterpreter (no matching src class; see the test source)
- com.google.javascript.jscomp.LinkedFlowScopeTest: A flow scope that tries to store as little symbol information as possible, instead delegating to its parents. [src com/google/javascript/jscomp/LinkedFlowScope.java]
- com.google.javascript.jscomp.PolymerBehaviorExtractorTest: Finds the Polymer behavior definitions associated with Polymer element definitions. [src com/google/javascript/jscomp/PolymerBehaviorExtractor.java]
- com.google.javascript.jscomp.PolymerClassDefinitionTest: Parsed Polymer class (element) definition. [src com/google/javascript/jscomp/PolymerClassDefinition.java]
- com.google.javascript.jscomp.SemanticReverseAbstractInterpreterTest: unit tests of SemanticReverseAbstractInterpreter (no matching src class; see the test source)
- com.google.javascript.jscomp.TypeTransformationTest: A class for processing type transformation expressions [src com/google/javascript/jscomp/TypeTransformation.java]
- com.google.javascript.jscomp.integration.TypedAstIntegrationTest: Tests that run the optimizer over individual library TypedAST shards [test-class Javadoc]
