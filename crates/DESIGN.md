# crates/ design: how Java maps to Rust

These are the conventions of the port (docs/PORTING.md §5). They are deliberately Java-shaped: a
ported function must read line by line like its Java original. When this file and the code
disagree, fix the code.

## 1. Crates and modules

| Crate | Java package | Notes |
|---|---|---|
| `closure-rhino` (`crates/rhino`) | `com.google.javascript.rhino` (+ `rhino/dtoa`) | lowest crate; holds `TypeId` |
| `closure-jstype` (`crates/jstype`) | `com.google.javascript.rhino.jstype`, Rhino type testing helpers, `NominalTypeBuilder`, `JSTypeExpression#evaluate` | depends on rhino |
| `closure-parsing` (`crates/parsing`) | `com.google.javascript.jscomp.parsing` (+ `parsing/parser`) | depends on rhino |
| `closure-jscomp` (`crates/jscomp`) | `com.google.javascript.jscomp` and its subpackages | depends on both |

- One Rust module per Java file, snake_case: `Node.java` -> `node.rs`, `IR.java` -> `ir.rs`,
  `StaticSourceFile.java` -> `static_source_file.rs`. Java subpackages are directories
  (`rhino/jstype/JSType.java` -> `crates/jstype/src/js_type.rs`).
- Java nested classes stay in the file of their outer class (`Node.SideEffectFlags` ->
  `node.rs`, `pub struct SideEffectFlags`).
- **Classes from another package that rhino itself needs** (Java imports them into rhino) are
  ported into rhino under a module named after that package, and the owning crate re-exports
  them instead of porting them again:
  - `jscomp.base.*` (e.g. `Tri`, `JSCompDoubles`, `JSCompObjects`) -> `closure_rhino::jscomp_base`
    (re-exported as `closure_jscomp::base`).
  - `jscomp.colors.*` -> `closure_rhino::jscomp_colors` (re-exported as
    `closure_jscomp::colors`); `Color` is a cheap-clone `Arc` handle to an immutable value with
    Java's structural equality.
  - `jscomp.parsing.parser.util.SourcePosition` and `jscomp.parsing.parser.SourceFile` (needed by
    `NonJSDocComment`) -> `closure_rhino::jscomp_parsing_parser::{util::source_position, source_file}`
    (re-exported by `closure_parsing::parser`; the parsing crate must not keep a second copy).
  - `jscomp.serialization.NodeProperty` (proto enum, values from `TypedAst.proto`) ->
    `closure_rhino::jscomp_serialization::node_property`.
  - `jscomp.parsing.parser.FeatureSet` stays in parsing. Node stores it as an opaque object prop
    (section 4).
- A placeholder for a type that is not ported yet lives in its own module and is replaced in
  place, never duplicated.

## 2. Naming

- Types keep the Java class name verbatim: `JSDocInfo`, `IR`, `QualifiedName`, `JSError`.
- Methods and fields: Java camelCase -> snake_case; an acronym run is one word:
  `getFirstChild` -> `get_first_child`, `getJSDocInfo` -> `get_jsdoc_info`,
  `getJSType` -> `get_jstype`, `isGetProp` -> `is_get_prop`, `toStringTree` -> `to_string_tree`.
- Java enum constants keep their Java spelling: `Token::NAME`, `Prop::IS_PARENTHESIZED`,
  `SourceKind::STRONG`. Java `static final` constants keep SCREAMING_CASE. The crates allow
  `non_camel_case_types` and `clippy::upper_case_acronyms` at crate level for this.
- Overloads: the most-used overload keeps the plain name; each other overload gets a suffix
  naming what differs (`clone_tree()` / `clone_tree_with_type_exprs(bool)`,
  `new_string(s)` / `new_string_with_token(token, s)`). The `// port:` line names the Java
  signature when the name changed: `// port: Node#cloneTree(boolean)`.
- Every ported function, method and test carries `// port: <JavaClass>#<method>` on the line
  above it (tests: `// port: NodeTest#testFoo`). Test names are the Java names in snake_case.

## 3. The Node arena

```rust
pub struct NodeId(NonZeroU32);   // Copy, Eq, Hash, Ord. Java `Node n` -> `n: NodeId`
pub struct Ast { /* dense per-node arrays, never shrink */ }   // owns every node of one compilation
```

- Java object identity is `NodeId` equality: `n == m` means the same node. `@Nullable Node` is
  `Option<NodeId>`.
- **Instance methods** take the arena as their first argument, receiver first so chains read
  like Java: `n.getFirstChild().getNext()` -> `n.get_first_child(ast).unwrap().get_next(ast)`.
  Readers take `&Ast`, mutators take `&mut Ast`. Methods that return the receiver in Java
  (`@CanIgnoreReturnValue Node`) return `NodeId`.
- **Static methods of Node that create nodes** are methods on `Ast`:
  `new Node(Token.BLOCK)` -> `ast.new_node(Token::BLOCK)`, `new Node(t, a)` ->
  `ast.new_node_with_child(t, a)`, `new Node(t, a, b)` -> `ast.new_node_with_children2(t, a, b)`,
  `new Node(t, a, b, c)` -> `ast.new_node_with_children3(t, a, b, c)`,
  `Node.newString(Token.NAME, "x")` -> `ast.new_string_with_token(Token::NAME, "x")`,
  `Node.newNumber(1)` -> `ast.new_number(1.0)`. Static methods that need no arena are
  associated functions on `NodeId` (`NodeId::new_...` is never used for creation).
- `IR` is a unit struct with associated functions taking the arena: `IR.name("x")` ->
  `IR::name(ast, "x")`, `IR.call(target, args...)` -> `IR::call(ast, target, &[args])`
  (Java varargs become slices).
- A node has Java's fields: token, parent, first, next, previous (circular: the first child's
  `previous` is the last child, the last child's `next` is `None`, exactly as Java),
  `lineno_charno`, `length`, `jstype_or_color`, `original_name`, `prop_list_head`, and the
  subclass payload (`NumberNode.number: f64`, `BigIntNode.bigint: Arc<BigInt>`,
  `StringNode.str: JsString`, `TemplateLiteralSubstringNode.{cooked, raw}`). They are stored in
  parallel dense arrays indexed by `NodeId` (`NodeLinks`, `NodeData`, `NodeCold`, `NodeType`;
  see "Performance divergences"), not in one object. The Java subclass is a `NodeKind` enum
  inside the node data; `getDouble()` on a non-number node panics like Java's
  `ClassCastException`.
- BigInteger is immutable; its node payload uses `Arc<BigInt>` so `cloneNode()` preserves Java's
  BigInteger object identity (`NodeTest#testCloneValues`).
- Nodes are never freed. A detached node keeps its slot and may be re-attached, as in Java.
- Precondition failures (`checkState`, `checkArgument`, `checkNotNull`, casts) panic with the
  Java message text, via the `check_state!` / `check_argument!` / `check_not_null!` macros in
  `jscomp_base` (they format `%s` like Guava). Tests of Java `assertThrows` use
  `std::panic::catch_unwind` or `#[should_panic]`.
- Exceptions used for control flow become `Result` (docs/PORTING.md §8), never panics.

### Java idioms

| Java | Rust |
|---|---|
| `n.getFirstChild()` | `n.get_first_child(ast)` -> `Option<NodeId>` |
| `n.getOnlyChild()`, `getFirstFirstChild()`, `getSecondChild()` | same names, same return types as Java (`NodeId` when Java never returns null by contract, else `Option`) |
| `n.cloneTree()` / `n.cloneNode()` | `n.clone_tree(ast)` / `n.clone_node(ast)` -> new `NodeId` in the same arena |
| `n.replaceWith(m)` | `n.replace_with(ast, m)` |
| `n.detach()` | `n.detach(ast)` -> `NodeId` |
| `parent.addChildToBack(c)` | `parent.add_child_to_back(ast, c)` |
| `n.getBooleanProp(Node.QUOTED_PROP)` | `n.get_boolean_prop(ast, Prop::QUOTED)` (Java's `Node.QUOTED_PROP` aliases are `NodeId::QUOTED_PROP` associated consts) |
| `for (Node c : n.children())` (read only) | `for c in n.children(ast)` (an `Iterator` borrowing `&Ast`) |
| `for (Node c : n.children())` (body mutates the tree) | `let mut it = n.children_cursor(ast); while let Some(c) = it.next(ast) { ... }` (no borrow; fetches `c.next` when `c` is returned, exactly like Java's `SiblingNodeIterator`) |
| `for (Node c = n.getFirstChild(); c != null; c = next) { next = c.getNext(); ... }` | same loop with `let mut c = n.get_first_child(ast); while let Some(cur) = c { let next = cur.get_next(ast); ...; c = next; }` |
| `n.ancestors()` | `n.ancestors(ast)` (iterator) or `n.ancestors_cursor(ast)` (no borrow) |
| `n.toString()`, `n.toStringTree()` | `n.to_string(ast)`, `n.to_string_tree(ast)` -> `String` |

## 4. Properties, side props, JSDoc, types

- `Prop` is a `pub enum` with Java's constants in Java order (`as u8` is Java's `ordinal()`).
- The property list is Java's immutable, shareable linked list:
  `prop_list_head: Option<Arc<PropListItem>>`, `PropListItem { prop_type: u8, value, next }`.
  Sharing (as in `setStaticSourceFileFrom`) is `Arc` sharing; Java `==` on items is
  `Arc::ptr_eq`. Value is `PropValue::Int(i32)` or `PropValue::Object(ObjectProp)`.
- `ObjectProp` is a closed enum of what Java stores there: `NonJSDocComment(Arc<..>)`,
  `JSDocInfo(Arc<JSDocInfo>)`, `StaticSourceFile(Arc<dyn StaticSourceFile>)`,
  `InputId(Arc<InputId>)`, `Node(NodeId)`, `JSType(TypeId)`, and `Opaque(Arc<dyn OpaqueProp>)`
  for objects from crates above rhino (`FeatureSet`). `OpaqueProp: Any + Send + Sync + Display
  + Debug` with `as_any()` for downcasting; Display gives Java's `toString()`.
- `jstype_or_color: Option<JSTypeOrColor>` with `JSTypeOrColor::{JSType(TypeId), Color(Color)}`.
  `TypeId` remains a `u32` arena handle; `Color` is a cheap-clone `Arc` handle to an immutable
  value with Java's structural equality. The whole `jscomp.colors` package lives in
  `closure_rhino::jscomp_colors`, re-exported as `closure_jscomp::colors`. JSType-dependent
  printing and its tests live in closure-jstype.
- Everything inside `Ast` is `Send + Sync` (`Arc`, not `Rc`), so an `Ast` can move to a
  big-stack compiler thread.

## 5. Strings

- **`JsString`** (`js_string.rs`): an immutable WTF-16 string with its `hashCode` cached next
  to the reference to its code units, cheap `Clone`: an interned string is a reference to code
  units the pool keeps for the rest of the process (copied without reference counting), any
  other string is an `Arc<[u16]>`.
  Lone surrogates survive. It has Java `String`'s API on UTF-16 code units: `length()`,
  `char_at(i) -> u16`, `code_point_at`, `substring(b, e)`, `substring_from(b)`, `index_of`,
  `starts_with`, `ends_with`, `is_empty`, `hash_code() -> i32` (Java's `String.hashCode`),
  `compare_to`, `concat`, `From<&str>`, `From<String>`, `PartialEq<str>`/`PartialEq<&str>`,
  `to_string_lossy()`. Ordering is Java's `compareTo` (code-unit lexicographic).
- **Which Java `String` becomes what:** a `String` that is or may become JS source text or a JS
  value (node strings, identifiers, property names, string literal values, qualified names,
  original names, template strings, regexp text) is `JsString`. Every other `String` (file
  names, diagnostic messages, option values, JSON) is Rust `String`/`&str`. If Java indexes a
  string by `charAt`, the Rust value must be a `JsString` (or a local `Vec<u16>`).
- **`JsString` to Rust `String`:** text bound for an output (diagnostic arguments, type strings,
  a source name taken from a JSON string) uses `Display` (`to_string()`, which is
  `charset::utf8_encoded_text`): it writes `?` for an unpaired surrogate, as Java's UTF-8
  streams and `String#getBytes(UTF_8)` do. `to_string_lossy()` (U+FFFD) is only for text that
  never reaches an output, such as a comparison with an ASCII name.
- **Interning (`RhinoStringPool`)**: node strings are interned. `RhinoStringPool::add_or_get`
  returns a `JsString` that is pointer-identical for equal contents (Java's `==` on interned
  strings is `JsString::ptr_eq`). The pool is process-wide, split into independently locked
  shards (see "Performance divergences"), and keeps its strings for the rest of the process.
  `JsString` equality compares contents (pointer fast path), so code never depends on interning
  for correctness, only Java's identity checks do.
- `n.get_string(ast)` returns an owned `JsString` clone (no borrow of the arena is held);
  `n.set_string(ast, s)` takes `impl Into<JsString>` and interns. API that takes a Java
  `String` destined for a node takes `impl Into<JsString>`, so `"x"` literals work.

### Source names that hold unpaired surrogates

Source names are Rust `String`s throughout the port (`SourceFile`, the nodes' source files,
diagnostics, chunks, source maps), so they cannot hold an unpaired surrogate. Only one input can
give a name one: the `path` of a `--json_streams` input written with a JSON escape such as
`\ud800` (command-line arguments and files are decoded with replacement and never yield one). An
input source map's `sources` can hold one too, but Java fails on such a name with an
`InvalidPathException` (exit 254), which is not compared (docs/PORTING.md §4.4, "Dropped pairs").

closure-rs stores such a path as the text Java's UTF-8 streams write for it: each unpaired
surrogate becomes `?`, the encoder's replacement. Diagnostics (text and JSON), the manifest, the
bundle and the chunk dependency file then match Java byte for byte. Two differences from Java
remain:

- The output source map's `sources` entry: Java escapes the surrogate (`"p\ud800.js"`),
  closure-rs writes `"p?.js"`.
- Identity: two paths that differ only in their unpaired surrogates, or in a surrogate and a `?`,
  are two inputs in Java and one name in closure-rs, which stops with an internal error ("already
  exist in chunk").

## 6. Passes, the compiler, and borrowing (contract for jscomp)

- `closure_jscomp::Compiler` owns the `Ast` and implements `Deref<Target = Ast>` and
  `DerefMut`; `pub type AbstractCompiler = Compiler;` keeps Java's type name. Wherever Node API
  wants `&Ast`/`&mut Ast`, pass the compiler: `n.get_next(compiler)`, `n.detach(compiler)`.
  `NodeTraversal` derefs to `Ast` the same way.
- **A pass never stores the compiler.** Java's `private final AbstractCompiler compiler;` field
  is dropped; `CompilerPass::process(&mut self, compiler: &mut AbstractCompiler, externs:
  NodeId, root: NodeId)`, and every method that used `compiler` (or touches nodes) takes
  `compiler: &mut AbstractCompiler` (or `&AbstractCompiler`) as its first argument after
  `self`. All other Java fields stay fields with the same names.
- Callbacks: `NodeTraversal::traverse(compiler, root, &mut callback)`;
  `trait Callback { fn should_traverse(&mut self, t: &mut NodeTraversal, n: NodeId, parent:
  Option<NodeId>) -> bool; fn visit(&mut self, t: &mut NodeTraversal, n: NodeId, parent:
  Option<NodeId>); }`. Inside, `t.get_compiler()` gives `&mut AbstractCompiler`; call it at
  each use instead of binding it across other uses of `t`.
- No borrow conflicts arise because `NodeId` is `Copy` and no node reference outlives a call:
  `compiler.report(JSError::make(compiler, n, &DIAGNOSTIC, &[args]))` compiles (two-phase
  borrow; `make` only reads), and `compiler.report_change_to_enclosing_scope(n)` is a
  `&mut self` method that reads `self`'s own arena.

## 7. Values and collections

- `int` -> `i32` (use `wrapping_*` where Java relies on overflow), `long` -> `i64`,
  `double` -> `f64`, `char` -> `u16`, `byte` -> `i8`, `boolean` -> `bool`,
  `BigInteger` -> `num_bigint::BigInt`.
- `List`/`ArrayList`/`ImmutableList` -> `Vec`; `LinkedHashMap`/`LinkedHashSet` -> `IndexMap`/
  `IndexSet`; `ArrayDeque` -> `VecDeque`; `EnumSet` -> a bitset or `IndexSet`. `HashMap`/
  `HashSet` whose iteration order reaches output port Java's order (docs/PORTING.md §8); std
  `HashMap`/`HashSet` are banned in `crates/`.
- Java interfaces -> traits (`StaticSourceFile`, `StaticScope`, `StaticSlot`, `StaticRef`,
  `StaticSymbolTable`, `ErrorReporter`); default methods stay default methods. Abstract class
  hierarchies -> enums or traits, whichever keeps method bodies line-for-line.
- `toString()` used for output or messages -> `Display` (or `to_string(ast)` for nodes);
  `equals`/`hashCode` -> `PartialEq`/`Eq`/`Hash` with the same fields.
- JDK behaviour that output depends on (`Double.toString`, `String.hashCode`, `Character.isX`,
  `String.format` subsets) is ported once into `closure_rhino::java_lang` and reused; never
  approximate it with Rust's formatting.

## 8. Types (closure-jstype)

- `JSTypeRegistry` owns a never-shrinking `Vec<JSTypeData>` indexed by the existing Rhino
  `TypeId`. A slot contains the six `JSType` fields and `JSTypeKind`, one variant per concrete
  Java class. Kind payloads embed parent field structs; overrides live in their Java module,
  and `super` calls invoke that module's parent implementation directly.
- The receiver-first API uses public extension traits (`JSType`, `ObjectType`, `FunctionType`,
  etc.) because Rust cannot add inherent methods to Rhino's `TypeId`. Import the crate prelude.
  A Java chain becomes `t.to_maybe_function_type(reg).unwrap().get_prototype(reg, ast)`.
- Registry arguments follow the receiver. Readers take `&JSTypeRegistry`; methods that
  allocate or mutate, including transitive and lazy changes, take `&mut JSTypeRegistry`.
  Only pure caches and recursion flags use `Cell`. Node readers/creators then take `&Ast`/
  `&mut Ast`. Registry construction allocates its global name-table `ROOT` in the caller's Ast.
- Properties have stable `PropertyId` handles in a registry arena, preserving shared mutable
  slots. `StaticTypedSlot`/`StaticTypedRef` getters take the registry so `PropertyId`
  can implement them without a detached copy. Template maps use `Arc<TemplateTypeMap>`; pointer identity uses `Arc::ptr_eq`. Union alternate lists also use `Arc<Vec<TypeId>>`
  so reads retain Java immutable-list identity until rebuilding.
  `TypeId` equality is Java object identity; Java type equality and hashing explicitly call
  `EqualityChecker` and the recursive Java hash implementation. Type-keyed structural sets
  retain insertion order and compare Java hashes and equality; identity sets use handles.
- Builders do not borrow the registry; `build(reg, ast)` allocates. Every type is captured by the
  resolver at allocation and completed after constructor fields are set. Definition blocks use
  an explicit `Closer` token and `closer.close(reg, ast)` at the Java block's end.
- The registry owns its error reporter. Testing uses an `Arc<Mutex<TestErrorReporter>>`
  adapter so the fixture can inspect it; reporter handles lock only during individual
  callbacks, allowing recursive type resolution; fixture teardown validates warnings and errors.
  Scope objects are shared `Arc<dyn StaticTypedScope>` when retained by a type.

## Scopes and NodeTraversal (closure-jscomp)

Scopes and vars use `ScopeId` and `VarId` (`Copy + Eq + Hash + Ord`) handles, also exported as
`Scope` and `Var`. Handle equality preserves Java identity within a compiler. The compiler owns
`ScopeArena`, whose scope and var vectors never shrink. `AbstractScopeData<V>` and
`AbstractVarData<S>` contain the common Java fields, while the reusable `AbstractScope` and
`AbstractVar` traits contain each common Java method body exactly once. `TypedScope` and
`TypedVar` (`typed_scope.rs`, `typed_var.rs`) implement the same traits with their own associated
scope and var types and reuse these bodies. Scope.java's parent/depth fields and bodies stay in `scope.rs`; Var.java's
constructor validation and formatting stay in `var.rs`. Rust-only inherent forwarders make the
same methods available on handles without a trait import.

A `SyntacticScopeCreator` with the default redeclaration handler hands a scope out again in a
later pass while the code it was scanned from is unchanged (`syntactic_scope_cache.rs`; see
"Performance divergences"). Within one pass every request still makes a new scope, so handle
identity inside a pass is Java's object identity; across passes a handle may be the same. Code that adds or removes declarations
of an existing scope goes through `declare`, `undeclare`, `declare_internal` or
`clear_vars_internal`, which call `AbstractScope::note_mutation` so that such a scope is not
handed out again.

Metadata readers receive `&AbstractCompiler` and mutators receive `&mut AbstractCompiler`, after
the receiver. Java's `getOwnImplicitSlot` calls `computeIfAbsent`, so `get_own_slot`, `get_var`,
`get_slot`, `get_arguments_var`, and the checks that invoke those methods also receive a mutable
compiler: these lookups preserve Java's lazy implicit-var allocation. `has_own_slot` and
`has_slot` remain pure readers. Variable names use `JsString`. Declaration maps use `IndexMap`
for Java `LinkedHashMap` insertion order, and implicit-variable maps use `BTreeMap` with enum
ordinal order for Java `EnumMap`. Iterable methods return ordered vectors of handles.

Rhino's `StaticScope`, `StaticSlot`, and `StaticRef` traits return references to trait objects,
which arena handles cannot provide. The scope and var handles do not implement those traits; the
equivalent methods exist on the handles with the same names. `typed()` on a syntactic scope
fails with AbstractScope's message, as in Java; `ScopeId::untyped` returns the same handle.

`ScopeCreator::create_scope` takes a mutable compiler, the root node, and an optional parent
handle. Syntactic scope creators and scanners do not store the compiler, following DESIGN §6.
`RedeclarationHandler::on_redeclaration` receives it after the receiver, so a callback can operate
on the compiler-owned arenas. A creator can own or borrow a handler; memoized creators can own
or borrow a delegate. The scanner passes the compiler to NodeUtil's generic `AstContext` LHS
visitor and declares each node immediately in its consumer. This preserves mutations made by a
redeclaration handler before the Java traversal reads the next sibling.

`ScopedName` retains Java's equality over the captured name and scope-root identity, separate
from `VarId` handle equality. `AbstractScope::undeclare` uses that inherited `.equals` behavior.
`Simple` stores its immutable name/root fields, while variable getters read the captured name
from common variable data and the defining scope's root. `hash_code` preserves Java's wrapping
31 formula and `String.hashCode`; the opaque `Object.hashCode` contribution of a node uses its
Rust arena identity. The shared `AbstractVar::get_scope` returns its nullable stored field; `VarId::get_scope`
returns `ScopeId` through the syntactic constructor invariant. Associated scope handles are `'static`, allowing shared variable data to
lend compiler-owned input references without tying handles to a borrow lifetime.

`NodeTraversal` holds `&mut AbstractCompiler` and dereferences to its `Ast`;
`t.get_compiler()` returns that mutable compiler. Callback methods take `&mut NodeTraversal`,
`NodeId`, and `Option<NodeId>` parent exactly as §6 specifies. The stored callback is taken out
before traversal and threaded through recursion, then restored on success or unwind. Builders
can borrow a callback or own the Java Consumer adapter. Scope creators are borrowed or owned.
A scoped implementation returns `Some(self)` from `Callback::as_scoped_callback`, the Rust
counterpart of Java's `instanceof ScopedCallback`; every built-in scoped adapter does so.
The scope stack contains node handles until `get_abstract_scope` requests actual scopes;
that method creates ancestors and the requested scope in Java's order. Entry points retain
Java names with overload suffixes (`traverse_tree`, `traverse_with_scope`, and so on).

Java traversal error boundaries use `catch_unwind(AssertUnwindSafe(..))`. The error composer
preserves Java's node/parent context, position, source line, NULL and MISSING_SOURCE strings,
then invokes `Compiler::throw_internal_error`. Rust-only guards restore the borrowed callback
without changing Java's error wrapping boundaries. Java checkState/checkArgument/checkNotNull
use the rhino macros, including Java messages. Java exceptions caught as control flow remain
`Result`, including JDK numeric parsing helpers used by NodeUtil.

Node predicates consistently receive `&Ast` and `NodeId`; node consumers and `Visitor::visit`
receive a mutable arena and node. The LHS visitor's generic `AstContext` extends this convention
to a compiler context so declarations can update scopes during callbacks. Node-returning
iterables use ordered vectors or iterators, preserving Java's order. Linked maps/sets use
`IndexMap`/`IndexSet`; lookup-only hash maps need no observable bucket order. Nullable entries
remain `Option<NodeId>` where Java can include null. NodeUtil type/color queries take their
registries as trailing arguments because nodes contain handles (§4).

## Performance divergences

Performance work may leave the Java structure where stdout, stderr, output files, source maps and
exit codes stay byte-identical (CI, the unit-record ratchet, the complete D2 corpus and the
benchmark output comparison) and runs stay deterministic (docs/PORTING.md §4.7). Code that still
implements a Java method keeps its `// port:` marker. A divergence carries a short comment, marked
"Rust-only (performance divergence)" or similar, saying what Java does and why the Rust differs,
and is listed here, one line each, to ease upstream syncs (docs/PORTING.md §9). Crate files are
named by crate and file: `jstype/property_map.rs` is `crates/jstype/src/property_map.rs`.

- `jstype/property_map.rs`: `properties` / `known_symbols` are `Arc`, copy-on-write
  (`Arc::make_mut`), so the clones that release the registry borrow are O(1).
- `jscomp/data_flow_analysis.rs` `UniqueQueue`: `swap_remove` on the seen set; sorted queue
  insert by binary search (Java: `HashSet` + `PriorityQueue`).
- `parsing/config.rs`, `js_doc_info_parser.rs`: annotation / suppression / primitive-name maps
  are `Arc`-shared, not copied per JSDoc comment.
- `parsing/parser/scanner.rs` `peek_token_ref_at`: the parser peeks token types and lines by
  reference instead of cloning the token.
- `rhino/jscomp_colors/color.rs`: `Color` memoizes its hash code (immutable value).
- `rhino/node.rs` `lookup_property_ref`: the property list is walked by reference.
- `jscomp/serialization/malformed_typed_ast_exception.rs` `DebugParam`, `remove_unused_code.rs`
  unremovable log: messages are formatted only when used (Java formats eagerly).
- `jscomp/reference.rs`, `node_traversal.rs` `get_input_id_of_input`: references share the
  traversal's `InputId` `Arc` instead of copying it after an input lookup.
- `cli/src/main.rs`: the binary uses mimalloc as its global allocator (Java: the JVM's heap);
  faster compiles at the cost of a higher peak memory.
- `Cargo.toml` `[profile.release]`: fat LTO and one codegen unit.
- `scripts/pgo_build.sh`, `.github/workflows/release.yml`: the released binaries are built with a
  profile (PGO) from training compiles of the d3-12, lodash-es and three benchmark projects
  (`scripts/run_bench.py --write-args`), for less CPU time. Pushes and pull requests build without
  a profile; the output is the same either way.
- `jscomp/parallel_parse.rs`: inputs are parsed on up to 8 worker threads into arenas of their
  own (`Ast::new_for_preparse`) while the compiler runs; `CompilerInput#parse` moves a finished
  parse into the compiler's arena (`Ast::append_preparsed`) with the node ids, object sharing
  and error order that parsing in place gives, or parses itself (Java: on the compiler thread).
  No workers start while the machine is busy (on Linux, at least half as many other runnable
  threads as available CPUs, from `/proc/loadavg`): concurrent compiles then parse on their
  compiler threads instead of oversubscribing the CPUs.
- `rhino/rhino_string_pool.rs`: the intern pool is 64 independently locked shards, with a
  per-thread cache for the parser threads (Java: one weak interner). Interned strings are never
  freed, so copying one needs no reference count (`CLOSURE_RS_REFCOUNTED_NAMES` restores
  reference-counted entries, to compare the two).
- `rhino/js_string.rs`: a `JsString` carries its cached `hashCode` beside the reference to its
  code units (Java: a field of the String object), so hashing and comparing unequal strings
  read no code units (`Hash` writes the cached value); two interned strings are equal exactly
  when they are the same entry, and `Ord` short-cuts on identity.
- `rhino/fast_hash.rs`: every `IndexMap`/`IndexSet` uses a multiplicative word hasher instead
  of SipHash (insertion order, so iteration and output, are unaffected).
- `rhino/node.rs`: token and tree links of all nodes live in one dense array (`NodeLinks`) apart
  from the rest of the node; template literal strings are boxed; `get_string_ref`,
  `get_jsdoc_info_ref` read without copying; `removeProp`/`putProp` skip the list rebuild when
  the property is absent.
- `rhino/js_string.rs` `JsStrLike`: `matchesName`, `matchesQualifiedName`, `indexOf`,
  `startsWith`, `endsWith`, `Property.Key#matches` take string literals without allocating.
- `rhino/jscomp_parsing_parser/util/source_position.rs`: positions name their `SourceFile` by a
  copyable `SourceFileId` instead of holding a reference to it.
- `parsing/parser/keywords.rs`: keyword lookup matches bytes (Java: a map); the token type is
  computed without building the keyword string.
- `jscomp/scope.rs`, `var.rs`, `typed_scope.rs`: the fields of scopes and vars that never change
  are mirrored outside the shared arena lock (`ScopeMirror`, `typed_scope_mirror`); `getVar` and
  `declare` of syntactic scopes take the lock once.
- `jscomp/typed_scope_creator.rs`: the never-iterated `reservedNamesForScope` and
  `assignedVarNames` remove entries with `swap_remove`.
- `jstype/property_map.rs` `findClosest`, `object_type.rs` `getOwnSlot`/`getSlot`: the walk reads
  each property map in place instead of copying it.
- `jscomp/compiler_input.rs`: a `CompilerInput` is one `Arc` around its fields.
- `jscomp/rhino_error_reporter.rs` `JSErrorQueue`: the registry error queue's empty check is a
  flag load, not a lock.
- `jscomp/compiler.rs` `get_extern_properties_js`: the extern property names are converted to JS
  strings once per value, not on every RemoveUnusedCode run, and interned (copied without a
  reference count).
- `jscomp/node_traversal.rs` `get_input`, `syntactic_scope_creator.rs` `ScopeScanner`: the
  CompilerInput found for the current input id is kept (Java keeps the object) instead of being
  looked up by id again; `ImplicitVar::js_name` makes the implicit var names once.
- `rhino/java_lang/charset.rs` `decode`: one output buffer and a fast path for ASCII runs of
  UTF-8 (Java's decoder loop gives the same text).
- `rhino/js_string.rs`: `indexOf` scans for the needle's first unit, then compares the rest;
  `==` with an ASCII `str` compares unit by byte; `""` conversions share one empty string
  (Java's `""` literal is one interned object).
- `jscomp/compiler_options.rs`: the conformance path regex is compiled once per process, not
  per options object (CodePrinter.Builder makes default options per `toSource`).
- `jscomp/scope.rs` `NameArg`, `get_var_of_node`, `typed_scope.rs` `get_var`,
  `abstract_var.rs` `name_equals`: variable names are read by reference instead of copied, and
  `TypedScope#getVar` walks the scope chain under one lock for non-implicit names;
  `ImplicitVar::of` checks the name's length first.
- `jscomp/data_flow_analysis.rs` `flow`: the stored lattice element is compared with the new one
  before it is replaced instead of being copied first (Java keeps references); the
  `flowThrough` of MustBeReachingVariableDef, MaybeReachingVariableUse and
  LiveVariablesAnalysis use the owned input instead of copying it; `MustDef`'s map is shared
  copy-on-write.
- `jscomp/node_traversal.rs`: `getInput` returns the cached input without recomputing its id;
  `getChunk` is read once per script.
- `jscomp/source_file.rs`: the source kind is mirrored in an atomic, so `getKind`/`isExtern`
  take no lock.
- `jscomp/thread_safe_delegating_error_manager.rs`, `compiler.rs` `hasHaltingErrors`: a flag
  set when the manager found no halting errors and cleared by every change to it lets the
  per-node check of CombinedCompilerPass skip the locks.
- `rhino/node.rs` `Ast::prop_masks`: per node, one bit per property type present in its
  property list, in a dense array, so a lookup of an absent property reads neither the node nor
  the list.
- `rhino/node.rs` `NodeData`, `NodeCold`, `NodeType`: a node's fields are split over three
  dense arrays, the payload and property list (read by most passes), the source position and
  original name, and the type or color (Java: one object).
- Name lookups read the NAME node's string in place instead of copying it
  (`ScopeId::get_var_of_node`; PolyfillUsageFinder, OptimizeCalls, VarCheck,
  RemoveUnusedCode, DataFlowAnalysis#computeEscaped, InlineFunctions, GatherModuleMetadata,
  PeepholeFoldConstants); OptimizeCalls keeps the extern property names as JS strings instead
  of converting each property name to compare it.
- `jscomp/scope.rs` `declare`, `allocate`: a new var or scope takes the arena lock once.
- `jscomp/chunked_vec.rs`: the syntactic scope arena and its mirror grow in fixed-size chunks
  instead of one `Vec`, so growing never copies the scopes and vars already made.
- Source maps: `sourcemap/source_map_generator_v3.rs` `JavaAppendable` appends code units
  without making a JS string per write; `util.rs` `escapeString` copies unescaped runs and
  escapes directly; `jscomp/source_map.rs`, `compiler_source_excerpt_provider.rs` reuse the
  file name conversions of the previous mapping and the parsed input map
  (`SourceMapInput::get_cached_source_map`); `cli/java_io.rs` `EncodedWriter` writes UTF-8
  text to a UTF-8 stream without the UTF-16 round trip.
- `jscomp/reference_collector.rs` `skip_externs_when_unread` (InlineVariables,
  InlineObjectLiterals): a run leaves out the externs (Java traverses them on every run) while
  every NAME in them resolves to a var declared in them, which a traversal of the externs finds
  out and the compiler keeps until a change is recorded inside the externs
  (`ChangeTracker::get_externs_change_count`).
- `jscomp/basic_block.rs`: the basic blocks of references are `Rc`, not `Arc`.
- `rhino/js_string.rs` `JsStrLike::is_prefix_of`/`is_suffix_of`: `startsWith`/`endsWith` compare
  an ASCII literal with the code units byte by byte instead of converting it first.
- `jscomp/syntactic_scope_cache.rs` (hooks in `syntactic_scope_creator.rs`, `scope.rs`,
  `abstract_scope.rs`, `abstract_var.rs`): a scope made by a `SyntacticScopeCreator` with the
  default redeclaration handler is kept by root node and handed out again by a later pass while
  its code is unchanged and no pass declared or undeclared a name in it (Java scans anew for every
  request); a pass never receives one scope twice. This saves most scans, and much peak memory on
  large bundles. `CLOSURE_RS_SCOPE_CACHE=off` disables it, `=verify` checks every reuse against
  a new scan.
- `rhino/node.rs` `Ast::track_changes`: the arena records every node whose children, token or
  string change (independently of the compiler's change reports, which some passes omit), so
  that the scope cache sees every change.
- `jscomp/scope.rs` `ScopeMeta::may_have_vars`: a name lookup passes the scopes in which nothing
  was ever declared without reading the arena.
- `jscomp/syntactic_scope_creator.rs` `ScopeScanner::scan_script`: a new global scope takes the
  top-level declarations of each unchanged script (the externs, mostly) from the scope cache and
  makes them again through `declareVar`, instead of walking the script's statements.
- `rhino/node.rs` `Ast::jsdocs`: a dense per-node copy of the JSDOC_INFO item of the property
  list, kept by `Ast::set_props`, so that `getJSDocInfo` reads one array instead of the node and
  its list; `getStaticSourceFile`, `isFromExterns` read the SOURCE_FILE item by reference.
- `rhino/js_string.rs`: an ASCII `&str` becomes a JS string in one exact-size allocation, the
  empty string (`""`, `Default`) is the string pool's entry (copied without a reference count,
  so parser threads do not contend for it), and `hashCode` is summed four code units per step
  (the same value as Java's loop).
- `rhino/node.rs` `getQualifiedName`: a GETPROP name is assembled in a per-thread buffer and
  copied into its string once; `jstype/equality_checker.rs` `areEqualInternal` copies the template
  type maps only when the left one has keys (`js_type::template_type_map_ref`).
- `jscomp/parallel_parse.rs`: a parse worker keeps one unshared parser configuration for all
  the inputs it parses (instead of one per input).
- `jstype/property_map.rs` `PropertyTable`: the properties are a hash map (Java: `TreeMap`) for
  lookups by name, whose only walks are in key order (`values` sorts, `getOwnPropertyNames`
  returns a sorted set), as Java's.
- `jscomp/var_map.rs` (`AbstractScope#vars`): a scope keeps up to two declared names in itself,
  compared by cached hash, and moves to an insertion-ordered hash map beyond that (Java:
  `LinkedHashMap`), so a lookup through the scope chain reads no hash table for small scopes.
- `jscomp/code_printer.rs` `startSourceMapping`, `compiler_license_tracker.rs`,
  `node_util.rs` `isInSyntheticScript`: the source file name is read in place instead of being
  copied for every printed node.
- `jscomp/infer_js_doc_info.rs` `inferJSDocForProperty`: the qualified name is computed only when
  the JSDoc is attached to the property's type, its only use (Java computes it first).
- `parsing/parser/parser.rs` `peekId`, `getTreeStartLocation`, `parseClassElements`,
  `eatStaticIfNotElementName`, `eatObjectLiteralPropertyName`: the next token's type or start
  position is read in place instead of copying the token (`isClassElementStart` takes the type).
- `rhino/node.rs` `validateProperties`, `serializeProperties`, `getPropListDebugString`,
  `getSortedPropTypes`, `isEquivalentTo`: property lists are walked by reference, without
  reference counting.
- `jscomp/node_traversal.rs`: `AbstractModuleCallback` lends the current module to the callback
  for each node instead of copying it; the default `SyntacticScopeCreator` of a traversal is kept
  in the traversal, not boxed.
- `jscomp/abstract_scope.rs` `isBlockScope`, `isCfgRootScope`, `var.rs` `isImplicitGoogNamespace`:
  the scope root and whether a var is an implicit goog namespace (fixed when the var is made) are
  read from the lock-free mirrors.
- `jscomp/data_flow_analysis.rs` `analyze`, `control_flow_analysis.rs` `prioritizeFromEntryNode`:
  the successors (predecessors) of a node are read from the graph in place instead of being
  copied into a new list; `source_map.rs` `addMapping` reads the node's source file in place.
- `jscomp/compiler_executor.rs` `run_process_on_compiler_thread`, `cli/src/main.rs`: the CLI
  runs on one thread of the compiler's stack size, on which `runInCompilerThread` runs its
  callable in place with the same bookkeeping (Java's executor starts a thread per call, six
  per compile); the thread is parked, not joined, when the exit code is known. One allocator
  heap serves the whole compile instead of one per stage: less memory (6-25% less peak RSS)
  and no heap collection at each thread's exit.
- `jscomp/source_map.rs` `addMapping`, `source_file_mapping.rs` `get_source_mapping_cached`,
  `compiler_source_excerpt_provider.rs`: the output source map keeps the parsed input map of
  the file it is mapping and the original path it last resolved, so that mapping a printed
  node through the input map takes none of the compiler's locks (Java looks the map up by file
  name for every node).
