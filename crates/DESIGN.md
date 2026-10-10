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
  D-025), not in one object. The Java subclass is a `NodeKind` enum inside the node data;
  `getDouble()` on a non-number node panics like Java's `ClassCastException`.
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
- **Interning (`RhinoStringPool`)**: node strings are interned. `RhinoStringPool::add_or_get`
  returns a `JsString` that is pointer-identical for equal contents (Java's `==` on interned
  strings is `JsString::ptr_eq`). The pool is process-wide, split into independently locked
  shards (D-025), and keeps its strings for the rest of the process. `JsString`
  equality compares contents (pointer fast path), so code never depends on interning for
  correctness, only Java's identity checks do.
- `n.get_string(ast)` returns an owned `JsString` clone (no borrow of the arena is held);
  `n.set_string(ast, s)` takes `impl Into<JsString>` and interns. API that takes a Java
  `String` destined for a node takes `impl Into<JsString>`, so `"x"` literals work.

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
later pass while the code it was scanned from is unchanged (`syntactic_scope_cache.rs`, D-025).
Within one pass every request still makes a new scope, so handle identity inside a pass is Java's
object identity; across passes a handle may be the same. Code that adds or removes declarations
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
