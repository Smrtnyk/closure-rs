# serialization (typed-ast) port notes

- The build does not use protoc (D-003). The proto messages of `rhino/typed_ast/*.proto` were generated
  once into `*_proto.rs` (by a one-time generator script that is not in the repository, then
  `cargo fmt`), following protoc's Java API shape: messages are their own
  builders (`new_builder()`, consuming `set_x`/`add_x`, `build()`), getters return Java's
  defaults for unset fields (`default_instance_ref()` for messages), oneofs are enums with an
  `X_NOT_SET` variant plus a `XCase` enum, enums keep `UNRECOGNIZED = -1`. Field `type` is
  `type_` in Rust.
- `protobuf.rs` ports the protobuf-java runtime subset they use (`CodedInputStream`,
  `CodedOutputStream`, `WireFormat`, `InvalidProtocolBufferException`, `Message`). Encoding writes
  fields in field-number order with proto3 default omission and packed repeated scalars, as
  protobuf-java does: `runtime_libs.typedast` re-serializes byte-identically
  (tests/typed_ast_proto_test.rs).
- `MalformedTypedAstException` is thrown with `panic_any(MalformedTypedAstException)` (Java
  unchecked exception, not control flow); tests downcast the payload.
- `compiler_state.proto` is generated into `crates/jscomp/src/compiler_state_proto.rs` (its
  `java_package` is jscomp). ViolationProto's field keeps Java's name `allowlist_entry`.
  `Requirement`/`RequirementScopeEntry` (conformance.proto) are the real types of
  `conformance_config.rs`, with their binary wire format.
- Proto enums also get Java's `Enum#name`/`valueOf` (`name()`, `value_of()`).
- `FastGzipOutputStream` writes Java's GZIP header itself (OS 255, XFL 0) and deflates at level 1;
  `tests/fast_gzip_output_stream_test.rs` pins bytes produced by the reference JDK.
- Every generated file also embeds the Java `getDescriptor()` data: `MESSAGE_DESCRIPTORS`
  (`Descriptor`/`FieldDescriptor`: full name, fields in declaration order with number, type,
  label, containing oneof and message/enum type) and `ENUM_DESCRIPTORS`. The unit recorder's
  neutral protobuf encoding (`crates/testing/src/proto_neutral.rs`, FORMAT.md "Neutral
  encodings") decodes a message's bytes against them, which yields exactly `getAllFields`
  (the fields protobuf-java serializes).
- `StringPool.Builder#max_length`/`#pool` and `ColorId#right_aligned` are Rust-only accessors of
  private fields for the unit recorder's object dumps.
