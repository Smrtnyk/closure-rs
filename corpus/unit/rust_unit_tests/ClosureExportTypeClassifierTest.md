## ClosureExportTypeClassifierTest (post-call assertions, gate 0.2 (a))

Method of `com.google.javascript.jscomp.ClosureExportTypeClassifierTest` (parameterized, one record per case) that, after the hooked `testSame`, calls the Java API `ClosureExportTypeClassifier.classify` on the root node of the goog.module's metadata; no snapshot holds its result, so the check is ported as a Rust test (crates/jscomp/tests/closure_export_type_classifier_test.rs).

- ClosureExportTypeClassifierTest#testExportType: for each of the 75 cases, the goog.module 'm' has module metadata and ClosureExportTypeClassifier.classify of its root node gives the expected NAMED or DEFAULT export type
