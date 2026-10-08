/*
 * Copyright 2021 The Closure Compiler Authors.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   test/com/google/javascript/jscomp/serialization/SerializeTypesToPointersTest.java.

//! Port of serialization/SerializeTypesToPointersTest.java.
use closure_jscomp::{
    Compiler,
    closure_reverse_abstract_interpreter::ClosureReverseAbstractInterpreter,
    compiler_options::CompilerOptions,
    serialization::{
        serialization_options::SerializationOptions,
        serialize_types_to_pointers::SerializeTypesToPointers, string_pool::StringPool,
        string_pool::StringPoolBuilder, type_pointers::TypePointers,
    },
    type_check::TypeCheck,
};
use closure_jstype::{TypeId, js_type::JSType, object_type::ObjectType};
use closure_rhino::{ir::IR, node::NodeId};
use std::cell::RefCell;
use std::rc::Rc;

struct SerializeTypesToPointersTest {
    compiler: Compiler,
    string_pool_builder: Rc<RefCell<StringPoolBuilder>>,
}

impl SerializeTypesToPointersTest {
    // port: SerializeTypesToPointersTest#setUp
    fn set_up() -> Self {
        let mut options = CompilerOptions::new();
        options.set_check_types(true);
        let mut compiler = Compiler::new();
        compiler.init_options(options);
        let string_pool_builder = Rc::new(RefCell::new(StringPool::builder()));
        Self {
            compiler,
            string_pool_builder,
        }
    }

    // port: SerializeTypesToPointersTest#getGlobalType
    fn get_global_type(&mut self, type_name: &str) -> TypeId {
        let (registry, ast) = self.compiler.get_type_registry_and_ast();
        registry.get_global_type(ast, type_name).unwrap()
    }

    /// `getGlobalType(typeName).toObjectType().getConstructor()`
    fn global_constructor(&mut self, type_name: &str) -> TypeId {
        let t = self.get_global_type(type_name);
        let registry = self.compiler.get_type_registry();
        let object_type = t.to_object_type(registry).expect("NullPointerException");
        object_type.get_constructor(registry).unwrap()
    }

    /// `getGlobalType(typeName).toObjectType().getImplicitPrototype()`
    fn global_implicit_prototype(&mut self, type_name: &str) -> TypeId {
        let t = self.get_global_type(type_name);
        let (registry, ast) = self.compiler.get_type_registry_and_ast();
        let object_type = t.to_object_type(registry).expect("NullPointerException");
        object_type.get_implicit_prototype(registry, ast).unwrap()
    }

    // port: SerializeTypesToPointersTest#parseAndTypecheckFiles
    fn parse_and_typecheck_files(&mut self, files: &[&str]) -> NodeId {
        let root = IR::root(&mut self.compiler, &[]);
        for (index, file) in files.iter().enumerate() {
            let script = self
                .compiler
                .parse_synthetic_code(&format!("test_{index}"), *file);
            root.add_child_to_back(&mut self.compiler, script);
        }
        assert!(
            self.compiler.get_errors().is_empty(),
            "{:?}",
            self.compiler.get_errors()
        );
        let externs = IR::root(&mut self.compiler, &[]);
        IR::root(&mut self.compiler, &[/* externs */ externs, /* js */ root]); // make this a valid AST
        let interpreter = ClosureReverseAbstractInterpreter::new(self.compiler.get_type_registry());
        TypeCheck::new(&mut self.compiler, interpreter).process_for_testing(
            &mut self.compiler,
            None,
            root,
        );
        root
    }

    // port: SerializeTypesToPointersTest#findInStringPool
    fn find_in_string_pool(&self, str: &str) -> i32 {
        self.string_pool_builder.borrow_mut().put(str)
    }

    // port: SerializeTypesToPointersTest#findAllInStringPool
    fn find_all_in_string_pool(&self, str: &[&str]) -> Vec<i32> {
        str.iter().map(|s| self.find_in_string_pool(s)).collect()
    }

    fn create_serializer(&mut self) -> SerializeTypesToPointers {
        SerializeTypesToPointers::create(
            &mut self.compiler,
            &self.string_pool_builder,
            SerializationOptions::builder()
                .set_include_debug_info(true)
                .build(),
        )
    }
}

// port: SerializeTypesToPointersTest#outputsTypePointerForClass
#[test]
fn outputs_type_pointer_for_class() {
    let mut t = SerializeTypesToPointersTest::set_up();
    let src = t.parse_and_typecheck_files(&["class Foo {}"]);
    let foo_ctor_type = t.global_constructor("Foo");

    let mut serializer = t.create_serializer();
    serializer.gather_types_on_ast(&mut t.compiler, src);

    assert!(
        serializer
            .get_type_pointers_by_jstype()
            .get(&foo_ctor_type)
            .is_some()
    );

    let foo_ctor_pointer = *serializer
        .get_type_pointers_by_jstype()
        .get(&foo_ctor_type)
        .unwrap();
    assert!(
        serializer
            .get_type_pool()
            .unwrap()
            .get_type(TypePointers::trim_offset(foo_ctor_pointer))
            .get_object()
            .get_marked_constructor()
    );
}

// port: SerializeTypesToPointersTest#serializesPropertiesReferencedInSources
#[test]
fn serializes_properties_referenced_in_sources() {
    let mut t = SerializeTypesToPointersTest::set_up();
    let src = t.parse_and_typecheck_files(&["class Foo { serializeMe() {} } Foo.prototype;"]);
    let foo_prototype_type = t.global_implicit_prototype("Foo");

    let mut serializer = t.create_serializer();
    serializer.gather_types_on_ast(&mut t.compiler, src);

    assert!(
        serializer
            .get_type_pointers_by_jstype()
            .get(&foo_prototype_type)
            .is_some()
    );
    let foo_prototype_pointer = *serializer
        .get_type_pointers_by_jstype()
        .get(&foo_prototype_type)
        .unwrap();
    assert_eq!(
        serializer
            .get_type_pool()
            .unwrap()
            .get_type(TypePointers::trim_offset(foo_prototype_pointer))
            .get_object()
            .get_own_property_list(),
        t.find_all_in_string_pool(&["serializeMe"]).as_slice()
    );
}

// port: SerializeTypesToPointersTest#doesNotSerializePropertiesOnlyReferencedInTypeSummary
#[test]
fn does_not_serialize_properties_only_referenced_in_type_summary() {
    let mut t = SerializeTypesToPointersTest::set_up();
    let root = t.parse_and_typecheck_files(&[
        "/** @typeSummary */ class Foo { serializeMe() {} andMe() {} doNotSerializeMe() {} }",
        "new Foo().serializeMe(); const otherObj = {andMe: 0}; Foo.prototype;",
    ]);
    let foo_prototype_type = t.global_implicit_prototype("Foo");

    let mut serializer = t.create_serializer();
    serializer.gather_types_on_ast(&mut t.compiler, root);

    assert!(
        serializer
            .get_type_pointers_by_jstype()
            .get(&foo_prototype_type)
            .is_some()
    );
    let foo_prototype_pointer = *serializer
        .get_type_pointers_by_jstype()
        .get(&foo_prototype_type)
        .unwrap();
    assert_eq!(
        serializer
            .get_type_pool()
            .unwrap()
            .get_type(TypePointers::trim_offset(foo_prototype_pointer))
            .get_object()
            .get_own_property_list(),
        t.find_all_in_string_pool(&["andMe", "serializeMe"])
            .as_slice()
    );
}

// port: SerializeTypesToPointersTest#serializePropertiesInNonTypeSummaryExterns
#[test]
fn serialize_properties_in_non_type_summary_externs() {
    let mut t = SerializeTypesToPointersTest::set_up();
    let root = t.parse_and_typecheck_files(&[
        "/** @typeSummary @externs */ class Foo { serializeMe() {} doNotSerializeMe() {} }",
        "/** @externs */
class Bar { serializeMe() {} }
/** @type {string} */
Foo.prototype.andMe;
",
    ]);
    let foo_prototype_type = t.global_implicit_prototype("Foo");

    let mut serializer = t.create_serializer();
    serializer.gather_types_on_ast(&mut t.compiler, root);

    assert!(
        serializer
            .get_type_pointers_by_jstype()
            .get(&foo_prototype_type)
            .is_some()
    );
    let foo_prototype_pointer = *serializer
        .get_type_pointers_by_jstype()
        .get(&foo_prototype_type)
        .unwrap();
    assert_eq!(
        serializer
            .get_type_pool()
            .unwrap()
            .get_type(TypePointers::trim_offset(foo_prototype_pointer))
            .get_object()
            .get_own_property_list(),
        t.find_all_in_string_pool(&["andMe", "serializeMe"])
            .as_slice()
    );
}
