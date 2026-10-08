## PolymerIntegrationTest (post-call assertions, gate 0.2 (a))

Methods of `com.google.javascript.jscomp.integration.PolymerIntegrationTest` recorded through `compile(options, ...)` (api `compile`, always notCaptured). Their assertions are on values the record holds in `observed` (errors, warnings, printed output), which replay compares JSON-equal, but the postcall via schema has no entry for them; the last one also reads the warning's node. Port each as a Rust integration test: compile the record's input with its options and assert the listed fact.

