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
  parallel arrays indexed by `NodeId` (`NodeLinks`, `NodeData`, `NodeCold` and type or color;
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
- `jscomp/data_flow_analysis.rs` keeps compiler-owned flow states in vectors indexed by
  graph node and edge ID. The graph-annotating analysis entry point supports callers that
  inspect annotations. The work queue tracks graph IDs in a membership array and inserts
  by priority using binary search.
- `parsing/config.rs`, `js_doc_info_parser.rs`: annotation / suppression / primitive-name maps
  are `Arc`-shared, not copied per JSDoc comment.
- `parsing/parser/scanner.rs` `peek_token_ref_at`: the parser peeks token types and lines by
  reference instead of cloning the token.
- `parsing/parser/scanner.rs`, `keywords.rs`, `line_number_scanner.rs`: the scanner consumes
  owned lookahead tokens, scans UTF-16 slices for comment ends, line breaks and identifier runs,
  and recognizes unescaped keywords before allocating their spelling.
- `parsing/parser/parser.rs` parses left-associative binary operators by precedence. The
  exponentiation, nullish-coalescing and `in` grammar rules keep their own restrictions.
  Identifiers carry the scanner's keyword classification into parsing and lowering.
- `parsing/parser/trees/parse_tree.rs` stores small payloads in the parse tree's `Rc`.
  Tokens box the larger template-literal payload. The tree constructors and typed accessors
  preserve node identity. Rust callers that construct the raw enum variants use their new
  payload types.
- `parsing/ir_factory.rs` selects the validator by node token and advances past discarded
  comments without assembling their text. Closure-unaware ranges retain their comments.
- `parsing/js_doc_token_stream.rs`, `ir_factory.rs`: the JSDoc scanner reads the original
  comment past its opener and retains source ranges for contiguous ASCII words and annotations.
  It materializes the text on request and uses the character reader for normalization and dot
  lookahead.
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
  profile (PGO) from training compiles of the d3-12, lodash-es, three and fabric-srcmap benchmark
  projects (`scripts/run_bench.py --write-args`; the last one covers a bundle compiled with an
  input source map), for less CPU time. Pushes and pull requests build without
  a profile; the output is the same either way.
- `cli/command_line_runner.rs` embeds the default BUNDLE module runtime. A test recompiles the
  runtime with the current compiler and compares every byte. Custom module resolution uses the
  live supplier. The single-use CLI also skips per-input transpiler map output when there are
  no compiler errors or input maps. Reusable runners retain those maps for callers.
- `jscomp/build.rs`, `parsing/extern_snapshot.rs` and `rhino/node/snapshot.rs`: Cargo parses the
  bundled browser externs on the build host and embeds their syntax trees and JSDoc type trees.
  A compiler using the original resource text and matching parser configuration loads the
  snapshot when it first needs that file. Bundled snapshots use fixed-width node rows and a
  packed string table when their indices fit. With a fixed relocation, the loader writes each
  arena column directly and shares one mask and JSDoc lookup per property list. The first
  file to introduce the implicit template bound uses the allocation-order decoder.
  Standalone snapshots keep their existing decoder.
  Bundled SourceFiles obtain UTF-16 length and line count from the build and materialize their
  text on demand. Loading preserves node allocation order, source
  positions, shared properties and type expressions. Files with diagnostics or unsupported
  parser data use the source parser. The build also checks uncommented dependency input,
  recording annotation flags for files without Closure primitives or module syntax. It
  marks top-level property, function, variable and assignment declarations in source order.
  Typed scope creation replays the statement shapes from the snapshot attached to the current
  script. It evaluates declarations, annotations, aliases and initializers against the live
  program in source order. Empty function stubs enter their ordinary scopes and contribute their
  parameter names and writes. Unrecognized shapes, changed scripts and appended statements use
  the ordinary visitor.
  For bare properties annotated with `number`, `string` or `boolean`, the build records the
  native type. The scope builder checks the live annotation and binds it to the current registry.
  Consecutive typed fields on `C.prototype` can share the lookup of that prototype. The
  builder checks the actual scope binding, evaluates each field's live annotation with its
  owner's template parameters, and drops the saved binding at the next ordinary declaration.
  The build records property names in traversal order and identifies statements that cannot
  affect the side-effect, module and extern name-reference checks. Each loaded script keeps
  its original last statement and the arena range of the parse, including detached type nodes.
  Mutations to relevant links, text or annotations invalidate that script’s record. The
  checks walk statements appended after the original prefix and resolve property roots in the
  live scope before skipping extern name diagnostics.
- `jscomp/parallel_parse.rs`: inputs are parsed on up to 8 worker threads into arenas of their
  own (`Ast::new_for_preparse`) while the compiler runs; `CompilerInput#parse` moves a finished
  parse into the compiler's arena (`Ast::append_preparsed`) with the node ids, object sharing
  and error order that parsing in place gives, or parses itself (Java: on the compiler thread).
  Workers read source text when they claim a job. In a pruned graph with file entry points
  and more jobs than workers, they follow static relative imports and re-exports. The compiler
  also queues dependencies found by its module resolver. It checks source text, configuration
  and source kind before it accepts a worker's result.
  No workers start while the machine is busy (on Linux, at least half as many other runnable
  threads as available CPUs, from `/proc/loadavg`): concurrent compiles then parse on their
  compiler threads instead of oversubscribing the CPUs.
- `rhino/rhino_string_pool.rs`: the intern pool is 64 independently locked shards, with a
  per-thread cache for the parser threads (Java: one weak interner). Both look up the same hash
  of the UTF-16 spelling. The shard uses different bits from the bucket and its lookup tag,
  and the tables borrow their keys from the canonical strings. Interned strings are never
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
- Function type construction reads the ordered JSDoc parameter map in place. Named
  parameters match by name and destructuring parameters by position. Out-of-order
  matches are tracked so unused annotations are reported in declaration order.
- Typed scopes take ownership of their reserved names when the scope builder creates
  them. The type registry reads an existing resolution result after checking the resolver
  phase. It obtains the error reporter when a type still needs resolution.
- A typed scope view records its parent at construction. For an ordinary type name it
  checks declarations and reserved names along the scope chain under one arena read lock.
  Implicit variables still use the usual slot lookup so they can be created on demand.
- `jscomp/typed_scope_creator.rs` retains the input ID for each script and builds a full
  property name when a declaration needs it. A function owner with a declared slot uses
  that slot before constructing the component list for a property-chain lookup.
- `jstype/js_type_registry.rs` resolves dotted type names one property at a time from the
  source string, and searches namespace prefixes only when the registry has Closure namespaces.
- The type registry records which property names have carried visibility, constancy or
  deprecation annotations. Access checks for a name with no such annotation walk the
  implicit prototypes, whose getters can initialize types, and omit the property-table
  probes. A property installed under another name is eligible for all three checks.
- `jstype/js_type.rs` stores large function, enum and proxy payloads behind their registry
  entries. The smaller entries retain the same type handles and allocation order.
  Prototype string-property definitions update the live copy-on-write property table.
- `jstype/property_map.rs` `findClosest`, `object_type.rs` `getOwnSlot`/`getSlot`: the walk reads
  each property map in place instead of copying it.
- `jscomp/compiler_input.rs`: a `CompilerInput` is one `Arc` around its fields.
- `jscomp/rhino_error_reporter.rs` `JSErrorQueue`: the registry error queue's empty check is a
  flag load, not a lock.
- `jscomp/compiler.rs` shares one set of interned extern property names between
  RemoveUnusedCode and OptimizeCalls. Replacing the extern properties starts a new set.
- `jscomp/guarded_callback.rs` stores conditional contexts in the traversal stack and
  links guards to earlier stack entries by index. The polyfill finder borrows property
  names and method candidates until it finds a match. In a normalized tree, a guarded-only
  search skips the name lookup when the expression is unsafe and no resource is guarded.
  RemoveUnusedCode stores a single
  pending continuation or removable inline and queues multiple entries in their original order.
- `jscomp/node_traversal.rs` `get_input`, `syntactic_scope_creator.rs` `ScopeScanner`: the
  CompilerInput found for the current input id is kept (Java keeps the object) instead of being
  looked up by id again; `ImplicitVar::js_name` makes the implicit var names once.
- `rhino/java_lang/charset.rs` `decode`: one output buffer and a fast path for ASCII runs of
  UTF-8 (Java's decoder loop gives the same text).
- `rhino/js_string.rs`: `indexOf` scans for the needle's first unit, then compares the rest;
  `==` with an ASCII `str` compares unit by byte; `""` conversions share one empty string
  (Java's `""` literal is one interned object).
- Type serialization visits borrowed own-property names in UTF-16 sort order. A function's
  synthetic `prototype` entry comes first, including when its property table has the same key.
- The JDK pattern matcher in `rhino/java_lang/regex.rs` keeps capture boundaries and loop
  positions in one register array. Each backtracking alternative saves its input position and
  an index into the log of changed registers. Retrying a branch restores those registers in
  reverse order. The backtracking storage lives for one matching or search operation.
- `jscomp/compiler_options.rs`: the conformance path regex is compiled once per process, not
  per options object (CodePrinter.Builder makes default options per `toSource`).
- `jscomp/scope.rs` `NameArg`, `get_var_of_node`, `typed_scope.rs` `get_var`,
  `abstract_var.rs` `name_equals`: variable names are read by reference instead of copied, and
  `TypedScope#getVar` resolves ordinary bindings and the live owner of a qualified name
  under one arena read. In type inference a flow-slot lookup also returns the declaration
  it used to choose the overlay. Implicit bindings retain their on-demand construction;
  `ImplicitVar::of` checks the name's length first.
- `jscomp/data_flow_analysis.rs` `flow`: the stored lattice element is compared with the new one
  before it is replaced instead of being copied first (Java keeps references); the
  `flowThrough` of MustBeReachingVariableDef, MaybeReachingVariableUse and
  LiveVariablesAnalysis use the owned input instead of copying it; `MustDef`'s map is shared
  copy-on-write.
- `jscomp/process_defines.rs` uses the arena's JSDoc flags to avoid indexing globals when no
  declaration could be a define. It builds an injected namespace at the original point and
  collects valid constant expressions only when it has define declarations to validate. The
  strings recorded by the arena also rule out the `CLOSURE_DEFINES` and misplaced-define
  walks when their markers are absent. A configured zone pattern still checks script names.
- `jscomp/transpilation_namespace.rs`, `compiler.rs`: the native CLI can omit the
  transpiler's global index when every remaining built-in ES2015+ rewrite will skip every
  script. A missing feature set, custom passes, ES5 output or a possible lookup keeps the
  original declaration snapshot. Embedders keep the eager namespace.
- `jscomp/type_inference_pass.rs` records CFG entries as the typed scopes are built and runs
  them in the same traversal order after type resolution, saving a second AST walk. Each entry
  retains its original node and script for diagnostics. With the built-in reverse interpreter,
  straight-line functions run the same entry and statement transfers in control-flow order.
  For an empty extern function declared in a script, variable or assignment, plain named
  parameters take their types directly from the live signature. Entry inference updates each
  parameter node and inferred slot in order. Default and destructured parameters, ES rest
  syntax and nonempty bodies run their usual transfers.
  In the global graph, leading scripts of unconditional declarations and expressions each
  occupy one vertex. Inference visits their statements in order and keeps the separate edge
  that bypasses externs. Debug histograms and custom interpreters use the full graph.
- `jscomp/graph/linked_directed_graph.rs` keeps the first incoming and outgoing edge in each
  node. Additional edges and callers requesting a mutable vector use ordered vectors.
  Control-flow construction resolves both endpoints once when checking for duplicate edges.
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
- `rhino/node.rs` keeps payloads and property lists separate from source positions.
  Original names use a 32-bit index into an arena table keyed by string identity. The type
  and color array grows only through the last assigned slot, so an untyped parse allocates
  no slots. Worker arenas remap original-name indices when joined.
- Name lookups read the NAME node's string in place instead of copying it
  (`ScopeId::get_var_of_node`; PolyfillUsageFinder, OptimizeCalls, VarCheck,
  RemoveUnusedCode, DataFlowAnalysis#computeEscaped, InlineFunctions, GatherModuleMetadata,
  PeepholeFoldConstants); OptimizeCalls keeps the extern property names as JS strings instead
  of converting each property name to compare it.
- `jscomp/scope.rs` `declare`, `allocate`: a new var or scope takes the arena lock once.
- `jscomp/chunked_vec.rs`: the syntactic scope arena and its mirror grow in fixed-size chunks
  instead of one `Vec`, so growing never copies the scopes and vars already made.
- `jscomp/code_consumer.rs` and `code_generator.rs` borrow punctuation across calls
  and pass single-line tokens directly to the printer. The final printer transfers its
  code and mapping buffers to the output. License tracking uses its exclusive state
  while emitting nodes. ASCII letters and digits use the Java character ranges directly.
- `sourcemap/source_map_consumer_v3.rs` decodes each incoming map into compact or
  wide records, appending fields and carrying the decoded deltas to the next entry.
  The JSON reader scans runs of ordinary UTF-16 units together. Resolving a file
  and column transfers the mapping's owned strings into the result.
- Source maps: `sourcemap/source_map_generator_v3.rs` `JavaAppendable` appends code units
  without making a JS string per write; `util.rs` `escapeString` copies unescaped runs and
  escapes directly; `jscomp/source_map.rs`, `compiler_source_excerpt_provider.rs` reuse the
  file name conversions of the previous mapping and the parsed input map
  (`SourceMapInput::get_cached_source_map`); `cli/java_io.rs` `EncodedWriter` writes UTF-8
  text to a UTF-8 stream without the UTF-16 round trip.
- `sourcemap/source_map_generator_v3.rs` stores file and symbol indices in each mapping and
  assigns their output IDs on first emission. The CLI writes escaped source content and buffered
  VLQ segments directly to its writer. The API that returns a Java string retains its UTF-16
  code units, including lone surrogates.
- `jscomp/reference_collector.rs` `skip_externs_when_unread` (InlineVariables,
  InlineObjectLiterals): a run leaves out the externs (Java traverses them on every run) while
  every NAME in them resolves to a var declared in them, which a traversal of the externs finds
  out and the compiler keeps until a change is recorded inside the externs
  (`ChangeTracker::get_externs_change_count`).
- `jscomp/inline_object_literals.rs` collects references for spellings that can receive an
  object literal, including names generated by splitting another object. It also resolves hoisted
  functions and implicit variables, and fully resolves externs while verifying their summary.
- The reference collector reuses ordered name sites and basic-block boundaries in function
  bodies and plain blocks for inlining, const inference, variable reference checks and
  denormalization. It resolves each name in the current scope. Hoisted functions requiring a
  jump use the ordinary visitor. An arena change, a replaced node payload or an externally
  drained mutation log invalidates saved analyses.
  `CLOSURE_RS_REFERENCE_SITES=off` disables reuse of the ordered sites.
- `jscomp/disambiguate` records property uses in traversal order. When diagnostics do not need
  the full graph, properties used only in externs omit their rename sites and union-find work.
  Extern-only memberships propagate in sparse 64-property words. An edge still schedules
  its destination when any membership is new. Source properties retain their insertion
  and union order, which determines their output names.
- `jscomp/function_argument_injector.rs` collects modified parameters, nested closures and
  uses of the enclosing receiver in one walk for `InlineFunctions`. Modified names retain
  first-use order, and only arrows inherit the receiver of an enclosing function.
- `jscomp/coalesce_variable_names.rs` stores each function's variable interference graph
  in rows of 128 bits. The upstream limit admits fewer than 100 variables. Row membership
  preserves self-edge weights, declaration-order ties and the first visited representative.
  Functions with a single binding or only parameters can skip liveness analysis when the
  coding convention's export queries are pure.
- `jscomp/flow_sensitive_inline_variables.rs` checks for a possible definition and a read
  of the same name outside a loop before constructing a control-flow graph. Hash collisions
  send the function through the full checks. On normalized trees with pure export queries,
  the scan can cover wide bodies. Its recursion depth is bounded. Deep expressions, classes,
  logical assignments to names and nested functions with complex parameters keep the eager
  schedule. The first candidate that passes its local checks triggers backwards analysis
  before any replacement.
  Both analyses share the function's variable map and escape set by transferring ownership.
  The backward analysis stores a single reaching CFG node directly. A second distinct
  node promotes it to a persistent set. At a control-flow join, both the variable map
  and the node sets share branches present on only one path. The inliner can therefore
  test for a single use without enumerating the set. The forward analysis keeps the
  existing definition map when incoming definitions already agree or are already unknown.
- `jscomp/remove_unused_code.rs` scans a script for guarded polyfill references the first
  time the optimizer needs to decide whether a reference in that script keeps a polyfill.
  Each script starts with an empty guard context, and the pass finishes its worklist
  before it changes the AST. This applies to a single run over normalized scripts with
  pure property-test queries. Reusable passes, custom passes and other tree shapes use
  the whole-root traversal. A fresh pass with a pure export predicate reuses a real
  variable's classification within that pass. A sparse index maps its compiler-wide VarId
  to the pass's analysis. Removals still follow first-visit order. Extern and export
  decisions that produce a log entry are evaluated on every use.
- `jscomp/inline_variables.rs` retains an inlining result when another variable may alias it.
  The expert shares the reference list collected at scope exit. Further additions give the
  collector its own copy. A plain parameter or catch binding has no value expression to
  substitute. Its aliases still check assignments and uses of `arguments`.
- Token-based peephole rules declare their candidates alongside the rewrite. The pass calls
  rules in upstream order and checks the current token again after each rewrite. Custom
  rules can run on every token.
- `jscomp/basic_block.rs`: the basic blocks of references are `Rc`, not `Arc`.
- `jscomp/gather_getter_and_setter_properties.rs` gathers accessors by walking the paths to
  current getter, setter and descriptor-call sites in Java traversal order. The arena indexes
  selected tokens on demand, adds new slots on the next scan and records retagged nodes in
  `setToken`. Each refresh checks live strings, parents and function and class shapes, so
  moved, renamed and detached sites retain the behavior of a fresh traversal.
- `jscomp/timeline.rs` keeps change order in the links between events. Deleting a
  function swaps out its index entry in constant expected time.
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
- `jscomp/scope.rs` records one bit per Java name hash modulo 64 on each scope.
  An ordinary lookup starts at the first enclosing scope whose bits admit the spelling.
  Node and string lookups share a 256-slot table keyed by that scope and the complete
  spelling. Both successful and absent bindings can be reused. Declarations and reparenting
  invalidate entries that could have observed the change. Unrestricted mutable access to a
  scope also sets all its bits. Implicit names consult the current AST and create their
  slots on demand.
- `jscomp/phase_optimizer.rs` uses exact cached subtree sizes for the loop stopping
  heuristic. Each small count fits in two spare bytes in the tree links, preserving the
  20-byte link record. A large subtree has a marker and is counted from its children.
  Changing a child list invalidates the count for that parent and its counted ancestors.
- `jscomp/inline_object_literals.rs` finds candidate names from the arena's object literal
  and function token index. It checks current tokens and ancestry before accepting a name.
  The arena keeps the two most recent token selections so getter/setter gathering and
  object literal inlining can each reuse their index.
- `jscomp/global_namespace.rs` resolves the first component of a qualified name before
  constructing the full spelling. It copies the traversal's chunk handle only for references
  that belong to the namespace. Method-call checks and dead-code removal also read property
  spellings directly from nodes and copy them when adding a retained name to a collection.
- `jscomp/reference_collector.rs` passes the resolved traversal scope into each reference.
  When checking whether an assignment occurs only once in a variable's lifetime,
  `reference_collection.rs` resolves its declaration once across the enclosing basic blocks.
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
- `rhino/jsdoc_info.rs` visits type expressions directly in packed property order.
  The extern property collector reads the type roots from the stored expressions.
  Both public collection getters keep their original order and duplicates.
- `jscomp/check_access_controls.rs` reuses the defining property owner for a
  visibility check when both walks start at the same prototype. `type_validator.rs`
  answers the null-or-undefined subtype check directly for known concrete scalars and
  objects. Functions with an unknown native prototype use the general relation.
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
- `jscomp/reference_collector.rs`: `ReferenceCollector` is generic over its behavior (Java: a
  `Behavior` field), so the per-node `beforeTraverseNode` / `afterVisitNode` hooks that a behavior
  does not override cost no call; `addReference` names the added reference by its index in the
  reference map, so `afterVisitNode` receives it without a second lookup by var.
- `jscomp/serialization/color_pool.rs` reconciles colors using ordered references into the
  input shards. The definitions stay in their original type pools until reconciliation and
  diagnostics finish. A color with one definition stores that reference inline.
  During an in-memory compilation the optimizer receives the already assigned color IDs
  and interned property names. Shards read from TypedAST files reconstruct their IDs from
  the serialized definitions.
- `jscomp/serialization/serialize_types_to_pointers.rs` gathers property spellings and
  assigns type pointers in one postorder traversal. Type summaries are skipped and
  closure-unaware shadows contribute only types. Property filtering begins after the walk.
  Each referenced property records its string-pool offset on its first use in a type.
- `jscomp/invalidating_types.rs` indexes mismatch records by the Java type hash. The builder
  checks type equality within each hash bucket and records distinct pairs of types and source
  locations in insertion order. The finished multimap rebuilds the index from the current types.
  Color serialization hashes the standard native types when it constructs their lookup table.
- Type inference resolves a declared slot directly when a flow scope has no overlays. String
  property tables allocate their shared storage at the first insertion. Template substitution
  shares the original binding map while visiting its values in order and attaches the resolved
  values when the visit finishes. Type equality allocates its cycle table at the existing
  recursion threshold.
- `jscomp/serialization/convert_types_to_colors.rs` transfers the completed type shard to the
  color builder and resolves colors once per observed registry type. Nodes share the resulting
  lookup by TypeId. JSDoc conversion uses the same serialization rules and allocates its
  temporary string index when an annotation actually contains a serialized string.
- `jscomp/serialization/runtime_libraries.rs` indexes the bundled single-shard runtime at build
  time. Each compiler registers its strings, optimization colors and synthetic extern metadata
  during initialization, then decodes source files and scripts on first use. A file keeps its
  identity within the compiler and each script request produces fresh nodes. Packages with path
  aliases, source-map annotations or other shard layouts use the general TypedAST loader.
- `jscomp/polyfill_usage_finder.rs` `Polyfills::from_jar_table`: the jar's polyfill table is parsed
  once per process and shared (Java: the RewritePolyfills, IsolatePolyfills and RemoveUnusedCode
  constructors each load and parse `js/polyfills.txt`).
- `jscomp/node_traversal.rs` `WalkBranch`, `traverse_branch_static`: `traverseBranch` is compiled
  for each callback type and calls `shouldTraverse` and `visit` directly (Java: through the
  Callback interface for every node); the entry points and the handlers of functions, classes,
  scripts, modules and destructuring keep a `dyn Callback` and enter that walk with one virtual
  call.
- `jscomp/pass_gates.rs` (hook in `phase_optimizer.rs` `NamedPass::process`), `rhino/node.rs`
  `PresenceSummary`: a pass whose every effect needs a name, token or JSDoc flag that the arena
  never held (Closure Library idioms, `MSG_` names, CAST nodes, `@export`, ...) is neither created
  nor run (Java runs every pass); the arena keeps a monotone record of the tokens, watched names
  and JSDoc flags it ever held, each gate carries its proof, and VarCheck's `goog.forwardDeclare`
  pattern lives in an arena of its own so that it does not enter every record.
  `CLOSURE_RS_PASS_GATES=off` runs every pass, `=verify` checks every run a gate would skip
  against a fingerprint of the compilation, `=log` names the skipped runs on stderr. An upstream
  sync that changes a gated pass, or what its trigger means, must re-check that pass's gate.
- `jscomp/node_traversal.rs` `TRAVERSAL_TOKEN_CLASS`: `traverseBranch` classifies a node's token
  with one table lookup; tokens without a handler skip the dispatch switch, and `createsBlockScope`
  is asked only for the tokens that can create a block scope (Java: a switch and a call per node).
- `rhino/java_util/bit_set.rs`: `BitSet` stores up to two 64-bit words inline (`Inline([u64; 2])`)
  before spilling to a heap `Vec<u64>`, avoiding heap allocation for dataflow lattices under 128 bits.
- `rhino/node.rs`, `rhino/node/snapshot.rs`: `Ast::append_preparsed` remaps only slots recorded in
  `preparse_node_prop_slots`; `Ast::nodes_with_jsdoc_bit` scans dense `Ast::jsdocs` when a JSDoc
  flag has been observed; `Ast::changed_flag_nodes` tracks per-node mutations to `JSDOC_INFO`,
  `SIDE_EFFECT_FLAGS`, `IS_CONSTANT_NAME`, and `CONSTANT_VAR_FLAGS` for per-subtree cache
  invalidation.
- `jscomp/syntactic_scope_cache.rs`, `syntactic_scope_creator.rs`: `ScopeContext::of` reuses a
  parent scope's verified `(script, change_root)` tag within the same AST change round, and
  `SyntacticScopeCache` maintains per-subtree `flag_stamps` (`subtree_stamp_with_flags`) alongside
  structural `stamps`.
- `jscomp/pass_gates.rs`, `j2cl_checks_pass.rs`: `PassGate` also gates `rewriteDynamicImport`
  (`Token::DYNAMIC_IMPORT`) and `renameLabels` (`Token::LABEL`), and `J2clChecksPass` skips
  traversing scripts when no J2CL watched name or file marker is present.
- `jscomp/rename_properties.rs`, `rename_vars.rs`: property/variable occurrence accounting uses
  O(N log N) sorting and interned name/slot indices instead of quadratic rescans; `rename_vars`
  walks AST scopes directly with reusable per-scope buffers.
- `jscomp/inline_functions.rs`, `inline_simple_methods.rs`, `inline_object_literals.rs`:
  function-inlining and simple-method passes pre-index candidate names and prune unreachable
  subtrees; `inline_object_literals` pre-filters candidate object-literal sites by enclosing
  top-level function.
- `jscomp/normalize.rs`, `node_util.rs`: normalization avoids redundant child-list allocations and
  reuses direct AST helpers when preparing normalized function and statement shapes.
- `jscomp/remove_unused_code.rs`, `compiler.rs`: `RemoveUnusedCodeCache` (`CLOSURE_RS_RUC_CACHE=off`)
  caches unchanged pure-leaf function analyses with a 128-bit `polyfill_mask` and skips removable
  property tracking when `tracks_removable_properties()` is false.
- `jscomp/parallel_parse.rs`: `parallel_thread_count` scales intra-pass worker threads smoothly by
  `work_items / min_per_thread` (up to `max_threads`, disabled by `CLOSURE_RS_PARALLEL=off` or the
  busy-machine guard).
- `jscomp/code_printer.rs`, `compiler.rs`, `compiler_license_tracker.rs`, `source_map.rs`,
  `sourcemap/util.rs`: large multi-script bundles (`>= 8` non-empty scripts and `>= 500_000` nodes)
  render per-script code and mappings in parallel on up to 4 threads before merging in exact script
  order (`emit_scripts_to_builder`); `sourcemap/util.rs` buffers escaped output in 16 KB
  chunks with ASCII fast-path scanning, and `source_map.rs` caches resolved file/symbol mappings.
- `jscomp/control_flow_analysis.rs`, `control_flow_graph.rs`, `data_flow_analysis.rs`,
  `live_variables_analysis.rs`, `must_be_reaching_variable_def.rs`,
  `maybe_reaching_variable_use.rs`, `dead_assignments_elimination.rs`,
  `coalesce_variable_names.rs`, `flow_sensitive_inline_variables.rs`: read-only CFG construction
  (`compute_cfg_readonly`) and borrowed indexed dataflow analyses (`run_indexed_readonly`,
  `PreparedLiveness`, `PreparedMustDef`, `BorrowedLiveness`, `BorrowedMustDef`,
  `BorrowedReachingUses`) screen large batches of functions in 2048-item parallel chunks on worker
  threads (returning only functions that actually rewrite, or `CompactColoringOutput` for
  coalescing, and dropping graphs/lattices on workers); `FlowInlineCleanCache`
  (`CLOSURE_RS_FSIV_CACHE=off`) skips unchanged leaf functions known to produce zero flow-sensitive
  inlines under the current purity environment and `subtree_stamp_with_flags`.
- `jscomp/process_defines.rs`, `global_namespace.rs`: when no external namespace supplier is given,
  `ProcessDefines` inspects live attached `@define` nodes directly, skips `GlobalNamespace` when no
  live `@define` exists, builds a root-filtered namespace (`GlobalNamespace::new_filtered_roots`)
  pruning off-path containers when defines exist, and skips invalid-location rescans once all live
  `@define` nodes are accounted for.
- `jscomp/abstract_peephole_optimization.rs`, `peephole_optimizations_pass.rs`,
  `peephole_substitute_alternate_syntax.rs`, `peephole_fold_constants.rs`,
  `minimize_exit_points.rs`, `statement_fusion.rs`, `exploit_assigns.rs`: all built-in peephole
  rules declare tight `candidate_tokens()` (split by `late` in `PeepholeSubstituteAlternateSyntax`)
  and `FastTokenFilter` predicates (`NameIsUndefined`, `GetPropOnWindow`, `GetPropOnLiteral`), and
  `PeepholeOptimizationsPass` runs a direct post-order `PeepWalker` with lazy feature flushing
  instead of `NodeTraversal`.
- `jscomp/graph/lowest_common_ancestor_finder.rs`, `graph/fixed_point_graph_traversal.rs`,
  `disambiguate/color_graph_node_factory.rs`, `disambiguate/color_find_property_references.rs`,
  `disambiguate/cluster_propagator.rs`, `disambiguate/disambiguate_properties.rs`: LCA coloring
  and fixed-point worksets use dense `Vec` tables indexed by graph node ID; `ColorGraphNodeFactory`
  caches raw `ColorId -> ColorGraphNodeId`; `ColorFindPropertyReferences` walks the AST directly;
  `ClusterPropagator` stores extern memberships in sorted `Vec<(u32, u64)>` and prunes propagation
  to `Ancestors(Reachable(source_prop_nodes))`; `DisambiguateProperties` exits early when no
  non-extern non-invalidated property exists.
- `jscomp/reference_collector.rs`, `reference_collector/function_body.rs`, `inline_variables.rs`,
  `compiler.rs`: `FunctionBodySites` uses 8-byte tagged `Site` entries and records nested empty
  scopes; `replay_body_sites` materializes nested `BasicBlock`s and empty `ScopeId`s lazily;
  `InlineVarsCleanCache` (`CLOSURE_RS_INLINE_VARS_CACHE=off`) skips unchanged simple-parameter leaf
  functions with no local inlines and no outer-name alias dependencies by replaying only pruned
  free-variable `Site`s (`prune_to_slots`), validated via `subtree_stamp_with_flags`;
  `Compiler::set_life_cycle_stage` drops optimization-loop caches when leaving a normalized stage.
- `jscomp/serialization/convert_types_to_colors.rs`, `serialization/serialize_types_to_pointers.rs`,
  `serialization/js_type_reconserializer.rs`, `pure_function_identifier.rs`: type-to-color
  conversion and type-pointer serialization use direct post-order AST walks and dense `Vec` caches
  indexed by `TypeId`; `PureFunctionIdentifier` indexes property summaries by raw property name to
  avoid per-lookup `"." + name` string allocations.
- `jscomp/infer_consts.rs`, `remove_unused_code.rs`, `compiler.rs`, `reference_collector.rs`,
  `reference.rs`, `basic_block.rs`, `syntactic_scope_cache.rs`: `InferConsts` skips unchanged
  already-inferred scripts (`InferConstsCache`) and scans references in a single direct pass;
  `RemoveUnusedCodeCache` extends to `ADVANCED` property-tracking runs via `PropertyAccess` /
  `ReadOnlyPropertyAccess` replay and extern-property-set version guards (`CLOSURE_RS_RUC_CACHE=off`);
  `ReferenceCollector` and `BasicBlock` reuse compact `Reference` / `BasicBlock` layouts and
  zero-copy scope/site lookups.
- `jscomp/inline_object_literals.rs`, `optimize_calls.rs`, `inline_simple_methods.rs`,
  `inline_functions.rs`, `normalize.rs`, `make_declared_names_unique.rs`: `InlineObjectLiterals`
  prunes candidate subtrees via pre-indexed object-literal functions; `OptimizeCalls` and
  `InlineSimpleMethods` use direct recursive AST walks instead of `NodeTraversal` and avoid
  redundant property-name copies; `InlineFunctions` skips unchanged call/declaration rescans and
  reuses per-function analysis scratch; `Normalize` and `MakeDeclaredNamesUnique` avoid per-node
  allocations and redundant name lookups when scopes have no conflicts.
- `jscomp/deps/js_file_line_parser/lines.rs`, `deps/js_file_regex_parser.rs`, `compiler_input.rs`,
  `parallel_parse.rs`, `deps/module_loader.rs`, `deps/module_resolver.rs`,
  `deps/node_module_resolver.rs`, `find_module_dependencies.rs`, `modules/es_module_processor.rs`,
  `es6_rewrite_modules.rs`: `Comments::strip` uses single-pass ASCII/UTF-16 scanning; `CompilerInput`
  prebuilds `DependencyInfo` in parallel on parse workers (`prebuild_dependency_info_parallel`) while
  `ImportQueue` seeds relative ES imports in BFS order; module resolvers avoid redundant path
  normalizations and clones; `FindModuleDependencies`, `EsModuleProcessor`, and `Es6RewriteModules`
  walk module statements directly and cache per-module export/import lookups.
- `jscomp/typed_scope.rs`, `typed_scope_creator.rs`, `flow_scope.rs`, `linked_flow_scope.rs`,
  `type_inference.rs`, `type_inference_pass.rs`, `infer_js_doc_info.rs`,
  `jstype/js_type_registry.rs`, `jstype/property_map.rs`, `jstype/object_type.rs`,
  `jstype/subtype_checker.rs`: `TypedScope` tracks qualified-name root prefixes and `this.`
  presence (`may_have_qualified_name`, `may_have_this_qualified_name`), `JSTypeRegistry` tracks
  qualified type roots and `property_registration_epoch` and indexes property-type maps in place
  without cloning `Vec`s in `register_property_on_type` and `can_property_be_defined`, `PropertyMap`
  walks secondary parents in place via `find_closest_of_type` and exposes `is_own_empty`,
  `ObjectType::get_own_property_keys` builds a `Vec` directly without an intermediate `IndexSet`,
  and `SubtypeChecker` matches covariant native names directly on `JsString`; `TypedScopeCreator`
  fast-paths empty non-synthetic `BLOCK`s and plain function stubs without `NodeTraversal`;
  `TypeInference` caches container-scope globals, checks `is_constructor` before mixin walks, and
  uses zero-allocation `get_property_type` fast paths via `FlowScope::has_qualified_overlay`;
  `InferJSDocInfo` walks the AST directly without `NodeTraversal`.
- `jscomp/type_check.rs`, `type_validator.rs`, `rhino/jsdoc_info.rs`, `rhino/node.rs`,
  `rhino/java_lang/string.rs`, `rhino/jscomp_colors/color_id.rs`, `rhino/jscomp_colors/color.rs`,
  `jscomp/serialization/js_type_color_id_hasher.rs`,
  `serialization/js_type_color_id_hasher_rhino.rs`, `serialization/js_type_reconserializer.rs`,
  `serialization/serialize_types_to_pointers.rs`, `serialization/convert_types_to_colors.rs`,
  `serialization/color_pool.rs`: `TypeValidator` stores `subtyping_mode` in an `AtomicU8`,
  memoizes well-formed templatized types (`expect_well_formed_templatized_type_cached`), fast-paths
  valid property assignments (`can_assign_to_property_fast`), and skips ancestor walks for
  interface properties absent locally; `TypeCheck` fast-paths plain extern function stubs, memoizes
  per-`TypeId` predicates (`type_check_flags`), caches `can_property_be_defined(UNKNOWN_TYPE, ...)`
  (invalidated by `property_registration_epoch`), caches constructor nominal hierarchies, and checks
  `can_assign_to_property_fast` before walking prototype chains for `@implicitCast`; color
  serialization uses a Bloom filter and reusable UTF-8 byte buffer (`write_bytes_utf8`), a 16-slot
  stack fast path in `ColorId::union_from_iter`, `OptimizationTagsKey` JSDoc `Arc` sharing in
  `RemoveTypes`/`RemoveTypesAndApplyColors`, and single-borrow `AstProperties` iteration with
  reusable `ReconcileScratch` sets.
- `rhino/node.rs`, `rhino/node/snapshot.rs`, `rhino/node/snapshot_watch.rs`, `jscomp/compiler.rs`,
  `pass_gates.rs`, `replace_id_generators.rs`, `process_closure_primitives.rs`,
  `cross_chunk_method_motion.rs`, `cross_chunk_code_motion.rs`, `gather_raw_exports.rs`,
  `inline_and_collapse_properties.rs`, `serialization/convert_types_to_colors.rs`,
  `source_map_input.rs`, `cli/java_io.rs`, `cli/abstract_command_line_runner.rs`: pure-boolean AST
  properties and 2-bit `CONSTANT_VAR_FLAGS` are packed into dense per-node `Ast::prop_masks` with
  lazy `change_times` and `side_effect_flags` vectors; `PresenceSummary` records synthetic JSDoc
  bits (`ID_GENERATOR`, `CLOSURE_PRIMITIVE_ID`) and avoids `WatchedName::Goog` poisoning from
  `RewriteJsonToModule`; `PassGate` gates `closureCodeRemoval` and single-chunk/unannotated passes
  exit early without traversing untouched subtrees; `Compiler::clear_js_type_registry` reclaims
  typed-scope arenas early; input source maps pre-decode on a background worker during parsing;
  CLI source-map output streams through a 64 KiB `StreamingFmtWriter`.
- `jscomp/build.rs`, `build/bundled_script_summary.rs`, `source_file/bundled_script.rs`,
  `source_file/declarations.rs`, `typed_scope_creator.rs`, `type_inference_pass.rs`,
  `type_inference.rs`, `default_pass_config.rs`, `reference_collector.rs`, `node_util.rs`,
  `pure_function_identifier.rs`, `check_access_controls.rs`, `make_declared_names_unique.rs`,
  `normalize.rs`: `SnapshotUse::SyntaxAndNonParamNames` separates extern parameter renames from
  structural/name invalidation (`PARAM_NAME`), keeping build-time `BundledScript` metadata
  (`PLAIN_STUB_NO_SCOPE`, `declared_var_names`, `extern_referenced_names`, `normalize_clean`,
  `name_hash_mask`) valid across normalization and later passes; `TypeInferencePass` omits empty
  plain extern stub `TypedScope`s and types their parameters directly; `ReferenceCollector`
  verifies `externs_refer_only_to_externs` on first run via bundled summaries; `NodeUtil`,
  `PureFunctionIdentifier`, `CheckAccessControls`, `Normalize`, and `MakeDeclaredNamesUnique`
  fast-path bundled externs and empty stubs without redundant AST traversals or child scopes.
- `jscomp/parallel_parse.rs`, `compiler.rs`, `dead_property_assignment_elimination.rs`,
  `var_check.rs`, `ast_validator.rs`, `reference_collector/function_body.rs`,
  `flow_sensitive_inline_variables.rs`, `dead_assignments_elimination.rs`,
  `coalesce_variable_names.rs`, `live_variables_analysis.rs`, `must_be_reaching_variable_def.rs`,
  `inline_functions.rs`, `inline_simple_methods.rs`, `optimize_calls.rs`,
  `inline_object_literals.rs`, `inline_variables.rs`, `syntactic_scope_cache.rs`,
  `remove_unused_code.rs`, `scope.rs`, `code_generator.rs`, `sourcemap/base64_vlq.rs`,
  `sourcemap/source_map_generator_v3.rs`, `sourcemap/util.rs`: shared scoped work-stealing helpers (`run_scoped_indexed`,
  `run_scoped_for_each`) parallelize code printing (`emit_scripts_to_builder`, `to_source_array`
  from 40K nodes up to 8 threads), `DeadPropertyAssignmentElimination` (borrowing cached extern
  properties and pre-screening `< 2` `GETPROP` assignments), `VarCheck` validity mode (skipping
  verified bundled extern prefixes and chunking top-level statements), `AstValidator` (read-only
  `ValidatorTarget` across scripts and statement slices when `type_validation_mode == NONE`),
  `FunctionReferenceCache::precompute_parallel`, `InlineFunctions` Walks 1 and 2, and
  single-bundle `OptimizeCalls`/`InlineSimpleMethods` (with bundled-extern skips in
  `InlineSimpleMethods`); dataflow passes use lower parallel thresholds (64) and `O(1)` escaped
  key checks; `RemoveUnusedCode` raises `LeafRecorder` nested-stamp and outer-var limits to 512;
  `ReferenceCollector` skips unchanged or non-candidate top-level functions when no pending
  hoisted global function is referenced (`InlineObjectLiteralsCleanCache` and
  `InlineVarsCleanCache::clean_locals_only_global_fns`, both keyed by
  `SyntacticScopeCache::deep_function_stamp`: the stamps of the function and of every function
  nested in it, each a change scope of its own); `CodeGenerator` and `sourcemap` use
  zero-clone Latin identifiers, ASCII lookup tables (`SAFE_STR_ASCII`, `SAFE_JSON_ASCII`), and
  64 KiB byte-buffered VLQ encoding (`encode_to_bytes`, `MappingNames::emitted_ids`).
- `jscomp/cross_chunk_reference_collector.rs`, `reference_collector.rs`,
  `reference_collector/function_body.rs`, `syntactic_scope_cache.rs`, `guarded_callback.rs`,
  `polyfill_usage_finder.rs`, `remove_unused_code.rs`, `inline_functions.rs`, `optimize_calls.rs`,
  `inline_simple_methods.rs`: `CrossChunkReferenceCollector` takes the references of a function
  body from its cached `FunctionBodySites` (`Behavior::after_replayed_reference` hands each one to
  the behavior that reads `added_reference`); `FunctionReferenceCache::precompute_parallel` looks
  only at the functions `SyntacticScopeCache::take_changed_functions` lists instead of walking the
  tree, and not at all without a second thread; `RemoveUnusedCode` collects guarded polyfill
  usages with `PolyfillUsageFinder::traverse_only_guarded_cached`, which does not traverse again
  a function that is unchanged (its change stamp and those of the functions below it) and that
  `GuardedCallback` reaches with nothing guarded and no conditional linked, so that the usages in
  it depend on its subtree alone (`CLOSURE_RS_RUC_CACHE=off`; `=verify` compares every cached
  collection with a full traversal); the parallel scans of `InlineFunctions`, `OptimizeCalls` and
  `InlineSimpleMethods` split every large non-module script into runs of top-level statements, not
  only a lone one, so that one big file among several chunks is scanned on several threads.
- `jscomp/reference_collector/function_body.rs`, `reference_collector.rs`, `node_traversal.rs`,
  `inline_variables.rs`, `node_iterators.rs`, `compiler.rs`: `FunctionBodySites` also covers a
  body in which nested scopes declare `let`/`const` bindings (`SiteKind::EnterScope`,
  `declares_in_nested_scopes`). In a normalized tree, where every declared name is unique,
  `ReferenceCollector::replay_scoped_body_sites` replays such a body: the traversal enters and
  leaves every nested scope (`NodeTraversal::push_scope_root_quietly`), a name without a binding
  in the body's own scope is resolved at its first site, and the exit of a scope that declares
  bindings is reported to the behavior where the traversal would report it. The other users of
  the sites keep to bodies with one scope. `InlineVariables` records such a function as a clean
  leaf too, unless a nested scope inlined a variable or holds one that depends on a variable
  outside the function (`nested_dirty_fns`). `LocalVarMotion` keeps one `AstAnalyzer` instead of
  asking the compiler for one at every node. `Compiler::add_source_map_source_files` resolves
  the source names of an input source map once (`resolved_source_map_sources`), not again for
  the source map of every chunk.
- `jscomp/build/bundled_script_summary.rs`, `source_file/declarations.rs` (`clean_other`),
  `normalize.rs`: `extern_referenced_names` treats a GETELEM in a global expression
  (`C.prototype[Symbol.iterator] = ...`) as part of it, and the plain parameters and the
  expression name of any function in one as bound in the function's own scope (a body with a
  name still invalidates the summary), so every bundled script has the summary and the first
  InlineVariables run can skip the externs. `normalize_clean` also admits stray `;` statements and
  computed members of qualified names, alone or assigned an empty stub, which both Normalize fast
  paths handle in statement order.
- `jscomp/var_check.rs`, `variable_reference_check.rs`, `reference_collector.rs`,
  `analyze_prototype_properties.rs`, `strict_mode_check.rs`: these passes leave out the original
  statements of unchanged bundled extern scripts and visit only appended ones (Java traverses
  them). VarCheck: in a `normalize_clean` prefix whose `extern_referenced_names` the global scope
  declares, every name resolves and nothing is reported. VariableReferenceCheck
  (`ReferenceCollector::skip_bundled_extern_prefixes`): when every bundled script precedes all
  other non-empty scripts, declares every global name the bundled scripts use, and no other
  script declares such a name (a conservative scan outside functions), a var a bundled script
  declares is checked with its other references, starting as declared; a test shows the bundled
  references report nothing on their own with every diagnostic of the pass enabled.
  AnalyzePrototypeProperties marks the `properties` summary of a `normalize_clean` prefix (only
  GETPROPs there; a repeated mark adds an edge that changes nothing). StrictModeCheck has nothing
  to report in a `normalize_clean` prefix (a test checks its shapes).
  `CLOSURE_RS_BUNDLED_EXTERN_SKIPS=off` traverses those prefixes in all four passes.
- `jscomp/scope.rs`, `var.rs`, `compiler.rs` (`scope_arena`, `scope_arena_loan`),
  `implicit_nullability_check.rs`, `typed_scope_creator.rs`: the compiler owns the syntactic
  `ScopeArena` in a plain field and reads and writes it without a lock (it was an
  `Arc<RwLock<..>>` shared with the closure-jstype views). The views (`ScopeView`, `VarView`), which
  the registry calls without the compiler, read the arena only inside `scope::with_static_scope`,
  which lends it to them through the `scope_arena_loan` slot for one registry call and takes it
  back afterwards (also on unwind); the earlier bullets' lock-free `ScopeMirror` keeps only the
  scope fields, the vars are read from the arena.
- `jscomp/js_chunk.rs`: a chunk's final name and the depth and index the chunk graph assigns are
  kept outside the mutex of its inputs and dependencies (atomics for depth and index), so
  `getName`, `isWeak`, `getIndex` and `getDepth` take no lock; `name_ref` reads the name without
  copying it. `var_check.rs` `checkName` compares the current input with the var's input in place
  (`NodeTraversal::get_input_and_compiler`, `VarId::input_ref`) and takes the current chunk from
  the traversal's cache instead of copying both inputs for every name.
- `jscomp/typed_scope.rs`, `typed_var.rs`, `compiler.rs` (`typed_scope_arena`,
  `initialize_type_registry`, `clear_js_type_registry`), `jstype/js_type_registry.rs`
  (`scope_storage`), `jstype/static_typed_scope.rs`: while the compiler has a type registry, the
  `TypedScopeArena` lives in the registry's scope storage (an opaque `Box<dyn Any>` the registry
  only holds), before and after that in a compiler field; the compiler reads and writes it without
  a lock (it was an `Arc<RwLock<..>>` shared with the closure-jstype views). The canonical
  `TypedScopeView` / `TypedVarView` no longer hold a handle to the arena: they read it through the
  registry their lookups receive (`StaticTypedScope::get_slot` and `get_own_slot` take the
  registry, like the slot methods already did), and a scope view keeps its root and its parent's
  view, made with it.
- `rhino/input_id.rs` `InputKey`, `jscomp/reference.rs`, `node_traversal.rs`
  (`get_input_key_of_input`, replacing `get_input_id_of_input`): a `Reference` keeps a copyable
  key of its input id (numbered per distinct id name in a process-wide table, computed once per
  `InputId`) instead of a shared `Arc<InputId>`; two keys are equal exactly when the ids are
  equal, which is all `ReferenceCollection#areInSameFile` asks. The reference stays 24 bytes.
- `jscomp/analyze_prototype_properties.rs` `ChunkIndex`, `cross_chunk_method_motion.rs`: the
  symbol graph's edges and each `NameInfo`'s deepest common chunk name a chunk by its index in the
  chunk graph (`getAllChunks()[index]`), so the fixed-point propagation copies chunks without
  reference counts; `NameInfo::get_deepest_common_chunk_ref` takes the chunk graph and lends the
  chunk. `markReference` computes `getDeepestCommonDependencyInclusive` on the indices.
- `rhino/node.rs` `get_color_ref`: the node's color is lent instead of copied where callers only
  inspect it (`AstAnalyzer#isTypedAsString`, `NodeUtil#mayBeString`, `PeepholeFoldConstants`,
  `PeepholeReplaceKnownMethods`, `ClosureCodeRemoval`, and the color presence checks).
- `jscomp/compiler.rs`, `source_map.rs`, `source_file_mapping.rs`,
  `compiler_source_excerpt_provider.rs`, `sourcemap/source_map_generator.rs`,
  `sourcemap/source_map_generator_v3.rs`, `cli/abstract_command_line_runner.rs`: chunked output
  (`outputChunkBinaryAndSourceMaps`, files, not JSON streams) is computed before its loop by
  `Compiler::prepare_chunk_outputs` when the chunks' scripts have at least 10K nodes
  (`PARALLEL_CHUNK_OUTPUT_MIN_NODES`) and `parallel_thread_count` allows two threads. The scripts
  of all chunks are printed in one batch on worker threads (a `RecordingLicenseTracker` each, as
  `emit_scripts_to_builder` does for the scripts of one chunk) while the compiler thread runs
  `resetAndIntitializeSourceMap` once (after decoding undecoded input source maps, which reports
  nothing); each chunk's code is then assembled in chunk order with the
  `ChunkGraphAwareLicenseTracker`. When no chunk has more than 3/4 of the mappings
  (`PARALLEL_CHUNK_MAPS_MAX_LARGEST_SHARE`; otherwise the loop builds the maps, streaming each to
  its file, as without preparation), each chunk's source map is built on a worker from a copy
  (`SourceMap::try_clone`, `SourceMapGenerator::try_clone`, `SourceFileMapping::try_clone`) of
  that reset map, the state the loop gives the map for every chunk (`reset` leaves the sources
  content, which every chunk adds again unchanged): its mappings are added in order, its wrapper
  prefix set, and `appendTo` writes it into 64 KiB blocks (`BlockText`); the loop writes the
  files in chunk order as before. What waits is kept small: a printed script's mappings are
  packed into varints of the differences from the previous mapping (`PrintedMappings`), a chunk's
  code is held a byte per code unit when every unit fits (`HeldCode`), and a map reserves room
  for its mappings once (`SourceMapGenerator::reserve_mappings`). A panic is resumed where the
  loop would raise it. The maps stay in the loop when an input map does not decode (its lookup
  reports the error) or too few chunks are left. `SourceMappingCache` keeps every file asked
  about and every original file resolved for it (nodes of other files interleave, and a bundle's
  map names many original files in turn), and converts the file name itself instead of going
  through the provider's shared `last_mapping_name`.
- `jscomp/remove_unused_code.rs` `record_function_name_non_local`: a function declaration directly
  in a script marks its name's (outer) var as having a non-local value when it is traversed
  (`isExpressionResultUsed` is true under a SCRIPT). That no longer ends the function's
  `LeafFunctionSummary` recording: the summary notes it (`name_non_local`, with the name as its
  first outer var) and a hit sets the flag right after replaying that var, so the top-level
  functions of a bundle are summarized too. A hit also requires the same
  `isExpressionResultUsed` answer for a named function (`name_value_used`), which depends on
  ancestors outside the function's change scope (`CLOSURE_RS_RUC_CACHE=off` disables all summaries).
- `jscomp/reference_collector.rs` `CollectorCallback::should_traverse`: the input of the current
  script is looked up at every function, not only where a traversal starts at one, so that the
  function shortcuts above (which need a known input) apply from the first function of a script
  on, also in a script none of whose names a behavior with a narrow `needs_name`
  (`InlineObjectLiterals`) has resolved yet. The lookup reads a map and changes nothing else.
- `jscomp/inline_object_literals.rs` `process`, `reference_collector.rs`
  `order_free_global_functions`: the pass first makes a dry run that splits nothing. It splits
  only local variables, each decided by the references inside the function declaring it, so
  while nothing is split the order in which functions are looked at does not matter: the dry run
  jumps to no hoisted global function (it meets them at their declarations), resolves only the
  spellings assigned an object and the names of hoisted functions not declared in a script, and
  skips every global function that is cached clean whatever it references. When it finds nothing
  to split, the pass is done (the pass would make its first split in a function the dry run looks
  at the same way, or knows clean); otherwise the pass runs as before. The dry run changes no
  scope, so the full traversal may receive the kept scopes again
  (`Compiler::start_scope_handout_round`). `CLOSURE_RS_IOL_DRY_RUN=off` disables the dry run,
  `=verify` runs the pass after every dry run and checks that it splits nothing when the dry run
  found nothing.
- `jscomp/inline_variables.rs` `MotionMemo`, `node_iterators.rs`: a failed `canMoveModerately`
  walk (`LocalVarMotion`) from an initialization directly in a script is kept in
  `InlineVarsCleanCache` by initialization and reference NAME nodes, with the
  `subtree_stamp_with_flags` of the statements it depends on (the walk does not enter functions,
  and the stamps cover the side-effect flags of calls). One that stopped at a blocker fails again
  while the initialization's and the blocker's statements are unchanged, the blocker's statement
  still follows and the reference is not in a statement between them (a change between them can
  only stop the walk earlier); any other one while the same statements follow each other
  unchanged. Such walks are long (hundreds of nodes for a global var) and repeated by every run.
  `CLOSURE_RS_IV_MOTION_MEMO=off` disables it, `=verify` walks on every hit and checks that the
  walk fails.
- `jscomp/node_util.rs` `getBooleanValue`, `getNumberValue`, `peephole_replace_known_methods.rs`
  `tryFoldKnownMethods`, `tryFoldKnownStringMethods`: names are compared with the special values
  (`undefined`, `NaN`, `Infinity`; `Array`, `Math`, `Number`) in place instead of being converted
  to a Rust string (or a qualified name built) first, and a method name is converted only for a
  call that can be folded. A lone surrogate matches none of these values either way.
- `jscomp/reference_collector.rs` `replay_body_sites_with_jumps`, `replay_scoped_body_sites`: a
  function body whose cached sites name a hoisted function the collector has not collected yet
  is replayed too (it was collected by the ordinary visitor): at the site where the visitor would
  jump to the function (`maybe_jump_to_hoisted_function`, after adding the reference), the replay
  jumps, with its own state put aside. Such a body declares no function, the jumped-to function
  lies outside it, and for a behavior whose scope exits change only their own scopes
  (`Behavior::scope_exits_change_only_their_scopes`: InlineVariables, InlineObjectLiterals,
  InferConsts, VariableReferenceCheck, Denormalize) what the jump changes lies inside that
  function, so the body's sites stay valid; map entries are made at the first site of each
  spelling, so the jump's entries keep their place. In a bundle most module functions are referenced before their declarations.
  `CLOSURE_RS_REPLAY_JUMPS=off` restores the ordinary visitor for those bodies.
  `try_skip_clean_leaf_function` (`InlineVarsCleanCache`) skips such a clean leaf function too,
  replaying its free sites with the same jumps.
- `jscomp/remove_unused_code.rs` `trailing_parameter_is_empty_pattern`: a function with default
  value, rest or destructuring parameters can have a `LeafFunctionSummary` too (it needed plain
  or unused-marked parameters). A hit skips `removeUnreferencedFunctionArgs`, which changes
  nothing for a function whose parameter vars are all stable (the summary's condition) unless its
  last parameter is an empty destructuring pattern, which it deletes whatever the vars.
- `jscomp/syntactic_scope_cache.rs` `record_changes`, `rhino/node.rs` `Ast::no_changes_since`:
  `record_changes` (run by every scope request and every change stamp read) returns at once when
  the arena recorded no edit since the cache's cursor and no other reader drained the log, which
  is what it would find after draining nothing.
- `jscomp/source_file.rs`, `source_file/line_count.rs`: a source file's lines are counted when the
  count is first asked for, or before its code is cleared (Java keeps the count of cleared code),
  not whenever code is set, so the counts nobody reads (an input source map's, most inputs') are
  never taken; the count is every line terminator less the CRLF pairs (looked for only in a text
  with a CR), in passes the compiler can vectorize, equal to Java's loop.
- `jscomp/source_map_input.rs` `adopt_decoded_from`, `compiler.rs` `add_input_source_map`: an input
  source map that replaces an earlier one of the same input (the file an input's
  `//# sourceMappingURL` names, after `--source_map_input` gave it) takes the earlier map's decoded
  consumer when that decode succeeded and the two texts are equal (Java reads and decodes the map
  twice; decoding depends on the text alone and the consumer is immutable). A decode in progress
  on the background worker is waited for, one not started yet is done first on the compiler
  thread (as the worker does it, reporting nothing) when the compiler decodes both maps anyway; a
  failed one is not shared, so each map still reports its own error.
  `CLOSURE_RS_SHARED_INPUT_MAPS=off` decodes every map.
- `jscomp/type_inference.rs` `declare_module_imports`, `typed_scope.rs` `VarWatch`, `typed_var.rs`,
  `module_import_resolver.rs`: TypeInference declares an ES module's imports at the first node of
  the module body it flows through and again only when something the declaration reads or writes
  has changed since (Java declares them again at every node, each as a new var with the current
  type of the var the import names). The vars read (own vars of their module scopes) and declared
  are watched: the arena's epoch advances when a watched var's type changes, a watched var is
  replaced or removed, declarations are cleared, or a typedef the declaration propagates changes;
  an unchanged epoch means the declaration would make equal vars, differing only in identity and
  index, which nothing reads. Modules with an import resolving to no scope, or to a var outside
  its module scope, declare at every node. `CLOSURE_RS_MODULE_IMPORT_CACHE=off` declares at every
  node, `=verify` declares too and checks each skipped declaration would have made equal vars.
- `jscomp/compiler.rs` `has_halting_errors`, `report_queued_type_registry_errors_to_manager`: the
  check that runs twice per node of every CombinedCompilerPass is inlined down to three flag loads
  (queued registry errors, continue-after-errors, no halting errors); reporting queued errors and
  asking the error manager stay out of line. Taking an empty queue did nothing before either.
- `jscomp/es6_rewrite_modules.rs` `RenameGlobalVars`: the global name of a module-level variable
  is the variable's name, `$$` and the module's global name, computed once per module (Java derives
  the module's name from its path at every reference and splits the result into the terms of a
  qualified name only to join them again); a debug assertion compares it with Java's computation.
- `rhino/java_lang.rs` `string_compare_to` (`String#compareTo`): the UTF-16 units are compared as
  they are encoded, after the common prefix of the UTF-8 encodings, instead of encoding both
  strings into new vectors (the sorting error manager compares diagnostics' source names and
  messages many times each).
- `jscomp/compiler_warnings_guard.rs`, `compose_warnings_guard.rs` (`version`): the composition of
  the J2CL, closure-unaware-code, `@suppress` and options guards that decides each diagnostic's
  level is ordered once and kept while the options guard is unchanged (the contextual guards need
  the compiler, so it was composed again for every diagnostic, their levels computed first); the
  guards are asked in that order until one answers, as Java asks them.
  `CLOSURE_RS_WARNINGS_GUARD_ORDER=off` composes for every diagnostic.
- `rhino/js_string.rs` `java_hash`: the cached `String#hashCode` of a new string is summed sixteen
  units per step (`h·31^16` plus a dot product with constant powers, which vectorizes; the same
  sum mod 2^32), so hashing large texts (input source maps, their `sourcesContent`) costs less.
- `jscomp/source_file/line_count.rs` `line_offsets`, `source_file.rs` `find_line_offsets`: the line
  offsets skip blocks of sixteen units without a line terminator (a test that vectorizes), and a
  pending line count of the same text is taken from them (the two agree, as Java checks).
- `sourcemap/source_map_consumer_v3.rs`, `jscomp/source_map.rs`, `compiler_source_excerpt_provider.rs`:
  the file names and symbol names of a parsed input source map, the resolved original file names
  (`getSourceMapping`'s relative path), the generated file names of `SourceMap#addMapping` and the
  prefix-fixed names of `fixupSourceLocation` are pool strings (`RhinoStringPool::add_or_get`)
  instead of the parsed or converted strings. Each output mapping copies and drops several of them
  (the input-map lookup, the resolved mapping, the generator's file and name keys); a pool string
  needs no reference count for that, so a mapping costs no locked instruction and the workers of
  the parallel mapping resolve share no count. The strings are equal and hash the same.
- `jscomp/compiler_warnings_guard.rs`, `compose_warnings_guard.rs` (`with_guards`): the level of a
  report is what the `ComposeWarningsGuard` of the contextual guards and the options guard would
  return, computed from borrowed guards: the list is built by `addGuard`'s steps (replace the same
  guard, insert before the first guard of the same or a higher priority; the options guard's
  guards last first, its demotion passed on) instead of building the composed guard, which copied
  every guard handle of the options guard per report. `ComposeWarningsGuard#level` evaluates its
  guards under its lock instead of copying the list (the list holds no composed guard).
- `rhino/node.rs` (`NodeType`, `Ast::colors`, `NodeId::set_color_ref`), `jscomp_colors/color.rs`
  (`Color::address`), `jscomp/serialization/convert_types_to_colors.rs`,
  `disambiguate/color_find_property_references.rs`: a node's color is an index into a table of
  the arena (each color object once, keyed by its address and kept alive by the table) instead of
  its own reference-counted `Color` handle. Giving a node a color the caller lends
  (`set_color_ref`, which `RemoveTypesAndApplyColors` uses for every typed node), copying a node's
  type (`copyTypeFrom`, `cloneNode`) and reading it in place (`get_color_ref`, now used by
  `ColorFindPropertyReferences`) cost no reference count. `get_color` still returns a copy, and a
  color reads back as the same object. `append_preparsed` moves a worker arena's colors into the
  table. `rhino::node::JSTypeOrColor` is no longer public (nothing outside `node.rs` used it).
- `rhino/qualified_name.rs` (`QualifiedName::of_units`), `jscomp/module_renaming.rs`:
  `QualifiedName.of` interns each term from its code units (`add_or_get_units`) instead of making
  the substring first, and takes an interned string without a dot as its only term;
  `getGlobalNameOfEsModuleLocalVariable` of an ES or CommonJS module builds
  `name + "$$" + path.toModuleName()` (what the module's global name joins back to) in one buffer.
  Each intermediate string cost a reference-count decrement when dropped.
- `jscomp/code_change_handler.rs`, `change_tracker.rs`, `recent_change.rs`, `forbidden_change.rs`,
  `phase_optimizer.rs`, `performance_tracker.rs`, `validity_check.rs`: `CodeChangeHandler::report_change`
  takes `&self` and the handlers are shared as `Arc<dyn CodeChangeHandler>`, not behind a mutex;
  `RecentChange` and `PhaseOptimizer`'s `ScopedChangeHandler` keep their state in relaxed atomics,
  and `ChangeTracker::increment_change_stamp` is a load and a store (only the tracker writes the
  stamp), so that a reported change costs no locked instruction.
- `rhino/rhino_string_pool.rs`: a thread without a scoped cache (`with_thread_cache`; the compiler
  thread, scoped workers) looks up a direct-mapped table of the `RECENT_SLOTS` (16K) pool entries it
  interned last before taking the lock of a pool shard (two locked instructions per lookup); pool
  entries are never removed, so the table returns what the pool would.
- `parsing/parser/scanner.rs`, `rhino/rhino_string_pool.rs`: the scanner interns the spelling of an
  identifier without escapes straight from the source (`RhinoStringPool::add_or_get_units`, which
  goes through the thread's cache and allocates nothing on a hit) instead of copying it into a
  reference-counted string that `Node.newString` interns later, so the copies the token, the parse
  trees and `IRFactory` make of it cost no reference counts. The parser compares identifier values
  by content only.
- `rhino/hamt_pmap.rs`, `jscomp/linked_flow_scope.rs`, `flow_scope.rs`, `type_inference.rs`,
  `semantic_reverse_abstract_interpreter.rs`, the other reverse interpreters, `data_flow_analysis.rs`,
  `graph/annotation.rs`: TypeInference's lattice (`Rc<dyn FlowScope>`), its overlay scopes and
  slots, the refinement sets of `RefinementTrackingFlowScope` (`Rc<RefCell<..>>`) and the nodes of
  `HamtPMap` (also MaybeReachingVariableUse's lattice) are shared with `Rc` instead of `Arc` (and
  `Mutex`): they are built, read and dropped by one thread, and their reference counts were the
  largest source of locked instructions. `Annotation` no longer requires `Send + Sync`;
  `DataFlowAnalysisState` and these lattices are not `Send`.
- `jstype/union_type.rs`, `js_type.rs`: `UnionType::get_alternates` and `get_union_members` return an
  `Alternates` snapshot (a slice through `Deref`) that copies a union of at most eight members
  instead of cloning the union's `Arc<Vec<TypeId>>`, so that taking and dropping it costs no
  reference count; larger unions share the list. Tests of the list's identity use
  `get_alternates_shared`.
- `rhino/rhino_string_pool.rs`: `LazyInternedStringList` keeps the interned string of each entry in
  a `OnceLock` set by its first `get`, instead of a mutex per entry and a `WriteOnlyBitset` bit, so
  that a `get` of an interned entry is a plain load.
- `jscomp/must_be_reaching_variable_def.rs`, `flow_sensitive_inline_variables.rs`: `MustDef`'s map and
  its `Definition`s are shared with `Rc` instead of `Arc`; `PreparedMustDef`, which the workers of
  `FlowSensitiveInlineVariables` share, keeps the entry definitions as plain values and each run
  builds its entry lattice from them (definitions compare by node, so a fresh copy is equal).
- `jscomp/linked_flow_scope.rs`: lookups of a slot name and of the root of a simple name pass the
  name by reference (`get_var_of_root`), and joins borrow the slot name, instead of copying a
  reference-counted string per lookup.
- `jscomp/typed_scope_creator/typed_externs.rs`, `typed_externs.bin`, `jstype/js_type_registry/snapshot.rs`,
  `rhino/node/typed_snapshot.rs`, `jscomp/build.rs` (fingerprint), `function_type_builder.rs`
  (`ValidatorDescriptor`), `jstype/js_type.rs` (`TypeValidatorFn`): the global scope builder does
  not type the bundled externs when prepared data applies; it decodes the state that typing leaves
  (every registry field it writes, the typed scope arena, the creator's and builder's state, node
  types and `TYPEDEF_TYPE` properties, and the detached parameter lists `inferParameterTypes`
  makes) in its place, relocating nodes per script range and appended block. The data holds, for
  one canonical ADVANCED compile, the encoded state before and after that work and digests of what
  it reads. It applies only when the externs root holds `[synthetic:externs]` and then the
  unchanged parser snapshots of the bundled scripts in canonical order with the expected appended
  statements and node properties (`InferConsts` flags), the encoded state before is the recorded
  one, the coding convention is the default `ClosureCodingConvention`, no synthetic block markers
  are set, no name the bundled externs look up is marked non-nullable or forward declared by the
  program, and every Closure namespace is a legacy one; what `[synthetic:externs]` declares is
  left out of both states and put back by position. The encodings destructure every field, so a
  new field does not compile until it is encoded; the key order of the lookup-only indexes by
  property name is not part of the state. `CLOSURE_RS_TYPED_EXTERNS=off` runs the ordinary path,
  `=verify` runs it and compares the state after it with the data. The data is checked in;
  `CLOSURE_RS_REGENERATE_TYPED_EXTERNS=1 cargo test -p closure-jscomp typed_externs_data_is_current`
  rewrites it after a change to the type system, to typed scope creation or to the bundled externs
  (that test and `typed_externs_verify_on_bundled_externs` fail while it is stale; data made from
  other extern snapshots is ignored).
- `jscomp/typed_scope.rs` `ReservedNames`: a typed scope's reserved names (Java: a
  `LinkedHashSet` that only shrinks after construction) mark a declared name removed instead of
  unlinking it, so declaring each of a module scope's thousands of names no longer moves the rest
  of the set; the live names keep Java's order.
- `jscomp/typed_scope_creator.rs` `FirstOrderFunctionAnalyzer::visit`: the closest container
  scope is looked up only for the returns and assignments that use it (Java: for every node
  outside the global scope); every scope on the traversal's stack already exists, so the lookup
  has no effect.
- `jscomp/control_flow_analysis.rs` `ast_position`: the AST positions are logged during the
  traversal and replayed once, in order, into a table indexed by CFG node, which the priority
  queue's comparator reads (Java: a `LinkedIdentityHashMap` filled for every traversed node and
  read twice per comparison). Only CFG nodes are looked up, after every position is recorded.
- `jscomp/type_inference.rs` `redeclare_simple_var_declared`, `flow_scope.rs` /
  `linked_flow_scope.rs` `infer_simple_slot_type`: a simple name's declaration is looked up once
  per assignment and handed to the flow scope (Java: `updateScopeForAssignment`,
  `redeclareSimpleVar` and `getOverlayScopeForName` each look the same name up in the same
  scope).
- `jscomp/control_flow_analysis.rs` `connect_to_possible_exception_handler`: outside a `try`
  (the innermost exception handler is the function, or there is none) the pure
  `mayThrowException` walk is skipped, since connecting would stop at once.
- `jscomp/control_flow_graph.rs` `new_ast`, `graph/linked_directed_graph.rs` `reserve`: an AST
  control-flow graph starts with room for 16 nodes and 16 edges.
- `rhino/java_lang/message_format.rs` `format_plain`: a message pattern without quotes whose
  format elements are all `{n}` is formatted in place (Java parses the pattern into a
  `MessageFormat` each time `Platform.formatMessage` runs); other patterns are parsed as before,
  and debug builds compare both results.
- `jscomp/type_check.rs` `find_object_with_non_stringifiable_key`: the already-checked types
  keep their `hashCode` and membership compares it before `equals`, as Java's `LinkedHashSet`
  does (the port called `equals` on every element).
- `jscomp/type_inference.rs` `traverse_name`: the outer-scope lookup for an inferred name is
  skipped when the slot found is an overlay slot or a var declared in the container scope or a
  scope nested in it, which the outer var can never equal (unless the name is an implicit one).
- `jscomp/typed_scope_creator.rs` `IdentifyEnumsAndTypedefsAsNonNullable`,
  `jstype/js_type_registry.rs` `may_be_non_nullable_name`: `isNonNullableName` of an
  initializer's qualified name is not asked when no scope root has that name (and no lookup log
  is kept); its lookup scope is found without effect unless the name's root is an implicit var.
- `jscomp/serialization/convert_types_to_colors.rs` `RemoveTypesAndApplyColors`,
  `rhino/node.rs` `color_slot_of` / `set_color_slot`: the node color slot of a type's color is
  found once per type and stored directly on later nodes of that type, instead of being found
  by the color's address for every node. `CLOSURE_RS_COLOR_SLOT_MEMO=off|verify`.
- `jscomp/check_unreachable_code.rs`, `control_flow_graph.rs` `all_valued_nodes_reachable`: when
  every node of a function's graph is reachable after `initScope`, `shouldTraverse` reports
  nothing and skips looking each visited node up among the graph's nodes.
- `jscomp/typed_scope_creator.rs` `IdentifyEnumsAndTypedefsAsNonNullable::replay`: the source
  statements this callback looks at (declarations and expression statements) are recorded with
  their scope by `FirstOrderFunctionAnalyzer`, which walks the same tree with the same scopes just
  before, and visited from that list in the same order; only the externs are walked again.
- `jscomp/check_unreachable_code.rs`, `node_traversal.rs` `CfgObject::FullyReachable`: for a
  root whose graph has every node reachable (`needs_no_graph`, below; it accepts every function
  `ControlFlowAnalysis::linear_function` accepts, whose statements run once, in order),
  `CheckUnreachableCode` neither builds nor walks the graph (`AbstractCfgCallback` still builds it
  if asked for).
- `jscomp/rhino_error_reporter.rs` `map_error`: each thread keeps the diagnostic type it matched
  for each message (bounded); the result depends on the message only.
  `CLOSURE_RS_RHINO_ERROR_MAP_MEMO=off|verify`.
- `jscomp/type_inference.rs` `last_module_root`: `flowThrough` finds the module of a scope
  root once per run of nodes with that root. `CLOSURE_RS_INFERENCE_MODULE_MEMO=off|verify`.
- `jscomp/typed_scope_creator/typed_externs.rs` `after_visit`: the stage test that every node the
  scope builder visits passes is inlined; the work of a watched segment is a separate function.
- `jscomp/check_missing_return.rs`: for a function whose body runs its statements once, in
  order (`ControlFlowAnalysis::linear_function`), the implicit return's only predecessor is the
  last statement (or the body), so `fastAllPathsReturnCheck` of that node decides without the
  graph. Debug builds build the graph and compare; `check_unreachable_code.rs` does the same for
  its fully reachable shortcut.
- `jscomp/function_vars.rs`, `dead_assignments_elimination.rs`, `flow_sensitive_inline_variables.rs`,
  `coalesce_variable_names.rs`, `live_variables_analysis.rs`, `must_be_reaching_variable_def.rs`,
  `parallel_parse.rs`, `compiler.rs`: before they create the scopes of a function, the parallel
  paths of `DeadAssignmentsElimination`, `FlowSensitiveInlineVariables` and
  `CoalesceVariableNames` screen every function on worker threads with the variables
  `FunctionVars::collect` finds from the `Ast` alone: the scopes `get_all_vars_declared_in_function`
  enters, in its order, with what `ScopeScanner` declares in each (same redeclaration tests), and
  the variables a name in a nested function resolves to (`computeEscaped`'s closure capture);
  functions with a CLASS, a synthetic block or a name of an implicit variable below them are left
  to the scopes. The analyses run on stand-in `VarId`s (one per variable of the function, never
  looked up in the compiler; `PreparedLiveness::from_function_vars`,
  `PreparedMustDef::from_function_vars`), exactly as each pass's worker job runs, and a function
  for which the job finds nothing (no dead assignment, no inlining, no merge, or too many
  variables) gets no scopes and no further work; the others go through the existing path (a
  function the coalescing job merges in gets its scopes only to materialize the job's output when
  their variables have the same names in the same order). `CoalesceVariableNames`' traversal then
  does not descend below nodes that are neither in a function with a coloring nor on the path to
  one (`on_path_to_coloring`): nothing there is renamed, and the nodes it visits it visits in the
  same order, so the changes are reported in the same order. DAE and
  FSIV also collect their candidate functions on worker threads, over runs of the statements of
  the scripts concatenated in order (`statement_units`, `collect_in_unit_order`; trees of at least
  50,000 nodes), and DAE reads the phase optimizer's change cutoff once
  (`scope_change_cutoff`) instead of locking it per function. A worker panic only makes the
  function (or the walk) go the serial way (`catch_unwind_quietly`, silent panic hook).
  `CLOSURE_RS_PARALLEL_ANALYSIS=off` turns these sections off; `=verify` screens but gives every
  function the existing path too and checks that its variables, counts and escaped set equal the
  scopes' and that the job finds nothing where the screen found nothing.
- `jscomp/check_unreachable_code.rs`, `node_traversal.rs`: at its first scope `CheckUnreachableCode`
  computes, on worker threads, the graph and the reachability marks of every CFG root below
  (except the roots `settled_without_graph` settles, `needs_no_graph` below, whose graphs it
  knows to report nothing without building them) and keeps the roots whose graph has no unreachable node it would report (a line
  number, no EMPTY or BREAK); `AbstractCfgCallback` enters those as skipped (`set_skip_roots`,
  `is_current_cfg_skipped`), and the check neither computes their graph nor looks nodes up in it
  (at least 64 roots per thread; `CLOSURE_RS_PARALLEL_ANALYSIS=off` turns it off, `=verify`
  computes the skipped graphs and checks that they report nothing).
- `jscomp/parallel_parse.rs`: `available_cpus` asks `std::thread::available_parallelism` once per
  process (on Linux each call reads the affinity mask and the cgroup quota files) for
  `parallel_thread_count`, `can_spawn_background_worker` and the parse workers.
- `jscomp/syntactic_scope_cache.rs` (`stamp_declaration_change`), `scope.rs` (`set_parent`,
  `set_depth_below_parent`): in a function's change scope, a kept syntactic scope goes stale only
  through a change that a scan of its root can see (the statement lists of blocks and control
  structures, declarations with their binding pattern targets, parameter lists, the names of
  functions and classes, a declared name's string), recorded per scope root on the path from the
  changed node; a stamped function stamps its body, whose scan reads the parameters. A kept scope
  may move to a parent at another depth (a module body rewritten into its script leaves every scope
  below it one level higher) and takes the depth below the parent on every reuse.
  `CLOSURE_RS_SCOPE_DECLARATION_STAMPS=off` restores whole-change-scope stamps and same-depth
  moves; `CLOSURE_RS_SCOPE_CACHE=verify` compares each reused scope, depth included, with a new scan.
- `jscomp/syntactic_scope_creator.rs`, `syntactic_scope_cache.rs`: a scope creator with a
  redeclaration handler of its own (`VarCheck`, `Normalize`'s duplicate declaration handler) takes
  part in the syntactic scope cache: it reuses a kept scope whose scan called no redeclaration
  handler (a new scan of the same code would not call its handler either) and keeps its own scans
  that called none, without replaying script declarations into a global scope. A creator that
  treats provides as redeclarations stays out. `CLOSURE_RS_SCOPE_REDECLARATION_FREE_REUSE=off`
  keeps all such creators out; `CLOSURE_RS_SCOPE_CACHE=verify` also checks the recorded handler
  calls against a new scan.
- `jscomp/denormalize.rs` (`Candidates`), `reference_collector.rs` (the `&mut` behavior forwards
  the function skip hooks): the reference collection that inlines `var` keywords
  resolves only the spellings of `var` declarations without a value in functions (found by the
  pass's first traversal) and of function declarations (so hoisted functions are visited in Java's
  order), skips leaf and top-level functions that hold no such declaration, and passes over the
  variables of a function body whose names it did not resolve: only a declaration without a value
  lets a scope exit change the body. `CLOSURE_RS_DENORMALIZE_CANDIDATES=off` resolves every name;
  `=verify` checks the candidates against a walk of the whole tree.
- `jscomp/check_unreachable_code.rs` (`needs_no_graph`): a CFG root needs no graph when its
  statements show every graph node reachable: statement lists in which only the last statement
  may fail to complete normally (`return`, `throw`, unlabeled jumps; an `if` completes when a
  branch does, a loop always does), no condition with a known boolean value, no `for` without a
  condition or with an increment after a body that cannot complete, and no `do`, `switch`, `try`,
  label, `with` or synthetic block. Such a graph reports nothing (`CfgObject::FullyReachable`).
  `CLOSURE_RS_REACHABLE_SHORTCUT=off` builds every graph; `=verify`, and every debug build, also
  builds the graphs of such roots and checks that none would report (an unreachable node without
  a line, an empty statement or a break, such as the empty increment of a `for` whose body cannot
  complete, is not reported), and `=verify` checks that no straight-line function is left to a
  graph.
- `jscomp/check_unreachable_code.rs` (`all_reachable`), `node_traversal.rs`
  (`AbstractCfgCallback::skip_control_flow_graph`): `CheckUnreachableCode` neither builds nor
  walks the control flow graph of a CFG root whose statements show every graph node reachable:
  statement lists in which only the last statement may fail to complete normally (`return`,
  `throw`, unlabeled jumps; an `if` completes when a branch does, a loop always does), no condition
  with a known boolean value, no `for` without a condition or with an increment after a body that
  cannot complete, and no `do`, `switch`, `try`, label, `with` or synthetic block. Such a graph
  reports nothing. `CLOSURE_RS_REACHABLE_SHORTCUT=off` builds every graph; `=verify` also builds
  the skipped ones and checks that none would report.
- `jscomp/concretize_static_inheritance_for_inlining.rs` (`has_inherits_call`): the pass first
  searches the tree for a `$jscomp.inherits` call and returns without its traversal when there is
  none: the traversal only records classes and static members for `process_inherits`, which acts
  on those calls alone. `CLOSURE_RS_STATIC_INHERITANCE_PRECHECK=off` always traverses.
- `jscomp/closure_code_removal.rs`: the abstract-method and assertion finders, which only read the
  tree and report nothing, run in a plain post-order walk (what they collect comes out in the
  order of the combined `NodeTraversal`, which visits a node after its children in child order).
  `CLOSURE_RS_CLOSURE_CODE_REMOVAL_WALK=off` uses the traversal; `=verify` runs both and compares.
- `jscomp/j2cl_checks_pass.rs`: the equality operators are listed by a plain post-order walk and
  checked after it, in the order the `NodeTraversal` would visit them (the check changes no
  tree). `CLOSURE_RS_J2CL_CHECKS_WALK=off` checks them in the traversal.
- `sourcemap/util.rs` (`write_escaped_string`): runs of characters that need no JSON escape are
  tested and narrowed eight UTF-16 units at a time (`is_safe_json_ascii`, equal to
  `SAFE_JSON_ASCII`) before the per-unit loop.
- `jscomp/optimize_calls.rs` (`UnitReferenceCache`, `collect_with_unit_cache`), `compiler.rs`:
  an `OptimizeCalls` run that ignores the externs takes the references of each unchanged
  top-level unit (a statement of a script, or a whole script) from the previous run and walks only
  the others, then adds them all in unit order (global names filtered then, against the current
  global scope). A unit counts as unchanged while its script argument, the change stamps of the
  unit and of every function in it, and the `is_quoted_string_key` results the walk read are as
  recorded, and the extern-property set is the same object. The cache is cleared with the other
  stamp-keyed caches when the change log is not continuous. `CLOSURE_RS_OPTIMIZE_CALLS_CACHE=off`
  walks every unit; `=verify` walks every unit and compares the cached references.
- `jscomp/remove_unused_code.rs` (`LeafRecorder::outer`, `abort_leaves_inside`,
  `record_outer_var_in_leaves`): a function traversed without a summary no longer ends the
  recordings of the functions around it. Every active recorder sees each event: a var is outer
  to the recorders it lies outside of, a removable added to a var ends only those recorders, a
  hit's summary is absorbed by each, and what marking a var not removable causes is recorded by
  the recorders that hold the var. So a function whose nested function keeps missing (it adds a
  removable to a var of the function around it) gets a summary of its whole subtree. A function
  whose last parameter is an empty pattern ends the recordings around it (a hit skips
  `removeUnreferencedFunctionArgs` for every function in the summary). `CLOSURE_RS_RUC_NESTED=off`
  ends them at every function without a summary.
- `jscomp/remove_unused_code.rs` (`drop_summaries_reading_changed_accessors`),
  `accessor_summary.rs` (`changed_names`): a new accessor summary (DisambiguateProperties renames
  getters and setters) drops only the RemoveUnusedCode summaries of functions whose subtree holds
  a GETPROP, OPTCHAIN_GETPROP or STRING_KEY with a name whose kind changed, the only names a
  function's analysis reads the accessor summary for (`considerForAccessorSideEffects`,
  `AstAnalyzer#getPropertyKind`), instead of every summary. `CLOSURE_RS_RUC_ACCESSOR_KEY=off`
  drops them all.
- `rhino/js_string.rs` (`JsStrLike::equals_units`), `rhino/node.rs` `matches_name`: a NAME is
  compared with a `str` name without converting the name to UTF-16 first (a str of n bytes has at
  most n code units, an ASCII str exactly n). `matchesName` is asked for every variable
  InlineVariables and RemoveUnusedCode look at (`isPropertyRenameFunction`).
- `jscomp/reference_collector/function_body.rs` (`FunctionBodySites::collect_delegating`,
  `SiteKind::Delegate`, `FunctionReferenceCache::delegating_sites`), `reference_collector.rs`
  (`collect_body_from_sites`, `replay_scoped_body_sites`), `node_traversal.rs`
  (`traverse_replayed_child`): a block that holds functions, classes, catch clauses, default
  values or destructuring targets is replayed from sites too (they ended the collection of
  sites). Each such node is a delegate: the replay, with the block's nested scopes entered
  (`push_scope_root_quietly`) and the basic blocks around the node on the block stack, hands
  it to the traversal at its place, so its subtree is traversed (or replayed) as before; map
  entries are made at the first site of each spelling, so the entries a delegate makes keep
  their place, and a function or class declared in a nested scope makes that scope one that
  declares bindings. These sites are kept apart from the others (whose users rely on a body
  without functions), under the block's stamp, which covers the place of each delegate.
  `CLOSURE_RS_DELEGATING_SITES=off` collects such blocks with the ordinary visitor, `=verify`
  collects the sites again at every use and checks that they are the same.
- `jscomp/remove_unused_code.rs` (`verify_same_summary`, `LeafRecorder::verify_failure`):
  `CLOSURE_RS_RUC_CACHE=verify` also traverses every function a RemoveUnusedCode summary would be
  replayed for and checks that its recording now makes the same summary (outer vars with their
  marks, pinned and required properties, polyfills, nested functions) and passes the checks of a
  kept summary.
- `jscomp/syntactic_scope_cache.rs` (`reuse_global_after_removals`, `global_declarations`),
  `syntactic_scope_creator.rs` (`ScopeScanner::script_declarations_now`, `script_lists`),
  `scope.rs` (`remove_vars_as_rescanned`), `var_map.rs` (`retain`): a kept global scope is
  updated and handed out again when every script that changed since its scan declares a
  subsequence of the name nodes it declared then (Java scans the global scope for every request;
  loop passes mostly change function bodies or remove declarations, while each scan declares
  every extern and program global). The vars of the declarations that are gone are removed and
  the vars after them numbered as a scan would number them, as long as no declaration left has
  the name of one of them (it would make the var) and every declared name node left still has
  its var's name and is in the tree. The declarations made for each script are kept with the
  scope; those of a changed script are found by the scan that records them, without declaring;
  a script with a Closure namespace is scanned anew. Whether a scan would call the redeclaration
  handler (a later declaration of a name finding it declared) is counted again over the
  declarations left when one that is gone called it, so that a creator with a handler of its own
  gets the updated scope only when no handler would be called. `CLOSURE_RS_GLOBAL_SCOPE_UPDATE=off`
  scans anew whenever a script changed; `CLOSURE_RS_SCOPE_CACHE=verify` checks every handed-out
  scope against a new scan (vars, depth and redeclarations).
  a script with a Closure namespace is scanned anew. `CLOSURE_RS_GLOBAL_SCOPE_UPDATE=off` scans
  anew whenever a script changed; `CLOSURE_RS_SCOPE_CACHE=verify` checks every handed-out scope
  against a new scan.
- `jscomp/inline_functions.rs` (`FunctionFactsCache`, `FunctionFacts`), `compiler.rs`: what
  `updateFunctionStateForInlining` computes from a candidate function's subtree and name (the
  size against the limit, direct replacement, non-inlinable parameters, the minimum requirements,
  the uses of the parameters and of `this`, local names) is kept between runs of InlineFunctions,
  each fact computed when the pass first asks for it, while the function's
  `SyntacticScopeCache::deep_function_stamp` (with in-place JSDoc and side-effect flags) is
  current and its name and the size limit are the same (Java computes them on every run).
  `CLOSURE_RS_INLINE_FUNCTION_FACTS=off` computes them on every run, `=verify` computes every kept
  fact again and compares.
- `jscomp/inline_functions.rs` (`NoInlineSite`, `FindCandidatesReferences::maybe_add_reference_using_mode`):
  a call site that InlineFunctions could not inline a candidate into (the size limit or
  `canInlineReferenceToFunction`) is kept by call node, candidate and mode with the
  `DeepFunctionStamp` of the function around the call (or of its statement of the script) and of
  the candidate, and answers no again while both are current, without making the call's scopes
  (Java checks every call site on every run). Under the same switch as `FunctionFactsCache`:
  `=verify` checks every kept answer.
- `jscomp/remove_unused_code.rs` (`is_guarded_polyfill_use`, `deferred_guard_functions`),
  `polyfill_usage_finder.rs` (`traverse_only_guarded_cached` with `deferred`),
  `guarded_callback.rs` (`GuardedCallbackSubclass::is_function_root`): the guarded polyfill
  traversal of a script, made when a polyfill reference in it is first considered, leaves out
  each isolated function (reached with nothing guarded and no conditional linked, outside every
  function it records) that has no kept usages, and traverses such a function on its own, as a
  root in the context of an isolated function, only when a reference in it is considered (Java
  traverses every script). The guarded usages in an isolated function depend on its subtree
  alone and its traversal leaves no guard behind, so the answer for every node is that of the
  whole traversal; the first run of the pass, with nothing kept, traverses every function of a
  script with a polyfill reference otherwise. `CLOSURE_RS_RUC_GUARD_DEFER=off` traverses them all;
  `CLOSURE_RS_RUC_CACHE=verify` compares every answer with a whole traversal of the script.
- `rhino/js_string.rs` (`JsStrLike::is_prefix_of` for `str`): a name whose first code unit is not
  the ASCII first byte of the prefix is rejected before the prefix is looked at as a whole
  (`isExported` asks every var whether it starts with `$super`).
- `jscomp/remove_unused_code.rs` (`get_var_info`): the mark that a new var info is not removable
  because globals or locals are not removed (SIMPLE) is not seen by the recordings of summaries;
  the var info has no removables, and a summary's hit makes it the same way, while recording the
  mark ended the recording of every function whose first reference to an outer var made its var
  info (`CLOSURE_RS_RUC_CACHE=verify` found such a recording ending where the kept summary did not).
