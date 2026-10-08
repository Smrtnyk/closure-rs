## ReferenceCollectorTest (post-call assertions, gate 0.2 (a))

Methods of `com.google.javascript.jscomp.ReferenceCollectorTest` whose assertions around the hooked call inspect Java-internal objects (the scan treats hook-reaching helpers as hooked calls). The records still replay but are excluded from gate (a) counting under D-015; the checks are ported as Rust unit tests.

- ReferenceCollectorTest#nullishCoalesce: asserts Reference.getBasicBlock().getRoot() (BasicBlock graph objects) inside the Behavior callback; postCall.pass.referenceMap encodes each reference's node and declaration/lvalue flags but not its BasicBlock, so replay does not check it
- ReferenceCollectorTest#optChain: asserts Reference.getBasicBlock().getRoot() (BasicBlock graph objects) inside the Behavior callback; postCall.pass.referenceMap encodes each reference's node and declaration/lvalue flags but not its BasicBlock, so replay does not check it
- ReferenceCollectorTest#testBasicBlocks: asserts Reference.getBasicBlock().getRoot() (BasicBlock graph objects) inside the Behavior callback; postCall.pass.referenceMap encodes each reference's node and declaration/lvalue flags but not its BasicBlock, so replay does not check it
- ReferenceCollectorTest#testBasicBlocksInConditionals: asserts Reference.getBasicBlock().getRoot() (BasicBlock graph objects) inside the Behavior callback; postCall.pass.referenceMap encodes each reference's node and declaration/lvalue flags but not its BasicBlock, so replay does not check it
- ReferenceCollectorTest#testThis: asserts ReferenceCollection.isEscaped() inside the Behavior callback; postCall.pass.referenceMap encodes isAssignedOnceInLifetime, isWellDefined and per-reference declaration/lvalue flags but not isEscaped, so replay does not check it
