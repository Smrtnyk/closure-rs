## ColorGraphBuilderTest (post-call assertions, gate 0.2 (a))

Test class `com.google.javascript.jscomp.disambiguate.ColorGraphBuilderTest` (reference commit bb8c8e7). Each record is the single hooked `testSame` inside `createBuilderIncludingCode`: its processor is a test-local lambda that collects a `ColorGraphNode` for every NAME starting with `test` and a label -> `ColorId` map. Replay compares those side outputs (`testFieldsAfter.labelToId`, `testFieldsAfter.graphNodeFactory`), so the 12 records stay counted. The assertions come after the call: the test builds a `ColorGraphBuilder` (with a stub `LowestCommonAncestorFinder` for the union tests), calls `build()`, and checks edges of the resulting `DiGraph<ColorGraphNode, Object>` as (source ColorId, destination ColorId, EdgeReason) cells. The `@After` methods also check that UNKNOWN is the only root and that the graph has no self edges and no parallel edges. Port these as Rust unit tests of the color graph builder (`disambiguate/ColorGraphBuilder`): build the colors from the same source, run the builder with the same LCA stubs, and check the same cells and graph properties.

- ColorGraphBuilderTest#constructorDef_includesPrototypeAndInstanceType_evenIfUnused: a constructor reference adds FOO_PROTOTYPE and FOO with TOP_OBJECT -> FOO_PROTOTYPE -> FOO, CAN_HOLD (lines 255-277); records [6]
- ColorGraphBuilderTest#prototypeChain_canBranch: FOO branches to BAR_PROTOTYPE -> BAR and QUX_PROTOTYPE -> QUX, CAN_HOLD (lines 223-253); records [11]
- ColorGraphBuilderTest#prototypeChain_connectsInterfaces: edge IFOO_PROTOTYPE -> IFOO, CAN_HOLD (lines 279-298); records [2]
- ColorGraphBuilderTest#prototypeChain_connectsSubclasses_viaClassSideInheritance: class-side chain TOP_OBJECT -> FOO0_CTOR -> FOO1_CTOR -> FOO2_CTOR, CAN_HOLD (lines 300-324); records [3]
- ColorGraphBuilderTest#prototypeChain_isInserted: edges TOP_OBJECT -> FOO_PROTOTYPE and FOO_PROTOTYPE -> FOO, CAN_HOLD (lines 166-187); records [9]
- ColorGraphBuilderTest#prototypeChain_isInserted_throughExtends: the prototype chain TOP_OBJECT -> FOO_PROTOTYPE -> FOO -> BAR_PROTOTYPE, CAN_HOLD (lines 189-221); records [8]
- ColorGraphBuilderTest#top_isAboveInterface: edges TOP_OBJECT -> IFOO_PROTOTYPE and IFOO_PROTOTYPE -> IFOO, both CAN_HOLD (test lines 141-164); records [0]
- ColorGraphBuilderTest#unions_connectBelowLac_whichHasSameDescendantCount: edge KIF -> (FOO_PROTOTYPE|BAR_PROTOTYPE), ALGEBRAIC (lines 484-511); records [5]
- ColorGraphBuilderTest#unions_connectBelowLca: the stubbed LCA color 100 gets an ALGEBRAIC edge to the union (FOO|BAR|QUX) (lines 380-410); records [4]
- ColorGraphBuilderTest#unions_connectBelowLca_whichIsAlsoUnion: LCA 100 -> (FOO|BAR|QUX) -> (FOO|BAR), ALGEBRAIC, with two stubs (lines 447-482); records [1]
- ColorGraphBuilderTest#unions_connectBelowLca_withMultipleLcas: both stubbed LCAs 100 and 101 get ALGEBRAIC edges to the union (lines 412-445); records [7]
- ColorGraphBuilderTest#unions_connectedAboveMembers: the union (FOO|BAR|QUX) has ALGEBRAIC edges to FOO, BAR and QUX, with a StubLcaFinder stub (lines 347-378); records [10]
