# serialization (typed-ast) port notes

- The build does not run protoc. The messages of Closure's
  `src/com/google/javascript/rhino/typed_ast/*.proto` are generated Rust in `*_proto.rs` (the
  generator is not part of the repository), following protoc's Java API shape: messages are their own
  builders (`new_builder()`, consuming `set_x`/`add_x`, `build()`), getters return Java's
  defaults for unset fields (`default_instance_ref()` for messages), oneofs are enums with an
  `X_NOT_SET` variant plus a `XCase` enum, enums keep `UNRECOGNIZED = -1`. Field `type` is
  `type_` in Rust.
- `protobuf.rs` ports the protobuf-java runtime subset they use (`CodedInputStream`,
  `CodedOutputStream`, `WireFormat`, `InvalidProtocolBufferException`, `Message`). Encoding writes
  fields in field-number order with proto3 default omission and packed repeated scalars, as
  protobuf-java does: the bundled `runtime_libs.typedast` re-serializes byte-identically
  (`crates/jscomp/tests/typed_ast_proto_test.rs`).
- `MalformedTypedAstException` is thrown with `panic_any(MalformedTypedAstException)` (Java
  unchecked exception, not control flow); tests downcast the payload.
- `compiler_state.proto` is generated into `crates/jscomp/src/compiler_state_proto.rs` (its
  `java_package` is jscomp). ViolationProto's field keeps Java's name `allowlist_entry`.
  `Requirement`/`RequirementScopeEntry` (conformance.proto) are the real types of
  `conformance_config.rs`, with their binary wire format.
- Proto enums also get Java's `Enum#name`/`valueOf` (`name()`, `value_of()`).
- `FastGzipOutputStream` writes Java's GZIP header itself (OS 255, XFL 0) and deflates at level 1;
  `crates/jscomp/tests/fast_gzip_output_stream_test.rs` pins bytes produced by the reference JDK.
- Every generated file also embeds the Java `getDescriptor()` data: `MESSAGE_DESCRIPTORS`
  (`Descriptor`/`FieldDescriptor`: full name, fields in declaration order with number, type,
  label, containing oneof and message/enum type) and `ENUM_DESCRIPTORS`. The unit recorder's
  neutral protobuf encoding (`crates/testing/src/proto_neutral.rs`, corpus/unit/FORMAT.md
  "Neutral encodings") decodes a message's bytes against them, which yields exactly `getAllFields`
  (the fields protobuf-java serializes).
- `StringPool.Builder#max_length`/`#pool` and `ColorId#right_aligned` are Rust-only accessors of
  private fields for the unit recorder's object dumps.
