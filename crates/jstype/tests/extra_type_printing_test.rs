/*
 * Copyright 2026 The closure-rs Authors.
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

// Extra oracle comparisons, not ports of Java unit-test methods.
// Source: a Java probe run against the pinned reference (not in the repository); cached results:
// corpus-cache/jstype/type-printing-oracle.tsv (pinned Java reference jar).
use closure_jstype::{
    JSTypeNative, JSTypeRegistry, TypeId,
    enum_type::{EnumType, EnumTypeBuilder},
    function_param_builder::FunctionParamBuilder,
    function_type::{FunctionType, FunctionTypeBuilder, Parameter},
    js_type::{JSType, Nullability},
    named_type::{NamedTypeBuilder, ResolutionKind},
    record_type_builder::RecordTypeBuilder,
};
use closure_rhino::{error_reporter::NullErrorReporter, node::Ast, token::Token};

fn setup() -> (Ast, JSTypeRegistry) {
    let mut ast = Ast::new();
    let reg = JSTypeRegistry::new(&mut ast, Box::new(NullErrorReporter), Vec::new());
    (ast, reg)
}

#[allow(clippy::too_many_arguments)]
fn assert_printing(
    reg: &mut JSTypeRegistry,
    ast: &Ast,
    label: &str,
    type_: TypeId,
    string: &str,
    explicit: &str,
    implicit: &str,
    display: Option<&str>,
) {
    assert_eq!(type_.to_string(reg, ast), string, "{label} toString");
    assert_eq!(
        type_.to_annotation_string(reg, ast, Nullability::EXPLICIT),
        explicit,
        "{label} explicit annotation"
    );
    assert_eq!(
        type_.to_annotation_string(reg, ast, Nullability::IMPLICIT),
        implicit,
        "{label} implicit annotation"
    );
    assert_eq!(
        type_.get_display_name(reg).as_deref(),
        display,
        "{label} display name"
    );
}

#[test]
fn extra_oracle_printing_all_native_types() {
    let (ast, mut reg) = setup();
    assert_eq!(NATIVE_EXPECTATIONS.len(), JSTypeNative::VALUES.len());
    for &(native, string, explicit, implicit, display) in NATIVE_EXPECTATIONS {
        let type_ = reg.get_native_type(native);
        assert_printing(
            &mut reg,
            &ast,
            &format!("{native:?}"),
            type_,
            string,
            explicit,
            implicit,
            display,
        );
    }
}

#[test]
fn extra_oracle_printing_compound_types() {
    let (mut ast, mut reg) = setup();
    let number = reg.get_native_type(JSTypeNative::NUMBER_TYPE);
    let string = reg.get_native_type(JSTypeNative::STRING_TYPE);
    let boolean = reg.get_native_type(JSTypeNative::BOOLEAN_TYPE);
    let null = reg.get_native_type(JSTypeNative::NULL_TYPE);
    let number_string = reg.create_union_type(&ast, &[number, string]);
    let mut large = RecordTypeBuilder::new();
    for name in [
        "z",
        "a",
        "m",
        "0",
        "99",
        "space name",
        "q'quote",
        "double\"quote",
        "line\nbreak",
        "𐀀",
        "\u{e000}",
        "epsilon",
        "beta",
        "gamma",
        "delta",
    ] {
        large.add_property(name, number, None);
    }
    let many = large.build(&mut reg, &ast);
    let mut small = RecordTypeBuilder::new();
    let callback = reg.create_function_type(&ast, string, &[number]);
    small.add_property("callback", callback, None);
    let nullable = reg.create_nullable_type(&ast, string);
    small.add_property("nullable", nullable, None);
    let optional = reg.create_optional_type(&ast, boolean);
    small.add_property("optional", optional, None);
    let empty = RecordTypeBuilder::new().build(&mut reg, &ast);
    small.add_property("empty", empty, None);
    let record = small.build(&mut reg, &ast);
    let mut parameters = FunctionParamBuilder::new();
    parameters.add_required_params(&[boolean]);
    parameters.add_optional_params(&mut reg, &ast, &[string]);
    parameters.add_var_args(number_string);
    let return_type = reg.create_union_type(&ast, &[record, null]);
    let this_fn = FunctionTypeBuilder::new()
        .with_parameters(parameters.build())
        .with_return_type(return_type)
        .with_type_of_this(record)
        .build(&mut reg, &ast);
    let mut closer = reg.get_resolver().open_for_definition();
    let ctor = FunctionTypeBuilder::new()
        .for_constructor()
        .with_name("PrintCtor")
        .with_parameters(parameters.build())
        .with_return_type(string)
        .build(&mut reg, &ast);
    closer.close(&mut reg, &ast);
    let instance = ctor.get_instance_type(&reg).unwrap();
    let prototype = ctor.get_prototype(&mut reg, &ast);
    let name = ast.new_string_with_token(Token::NAME, "Declared");
    let parameter_list = ast.new_node(Token::PARAM_LIST);
    let body = ast.new_node(Token::BLOCK);
    let source = ast.new_node_with_children3(Token::FUNCTION, name, parameter_list, body);
    let mut closer = reg.get_resolver().open_for_definition();
    let declared = FunctionTypeBuilder::new()
        .for_constructor()
        .with_name("Declared")
        .with_source_node(Some(source))
        .with_parameters(parameters.build())
        .build(&mut reg, &ast);
    closer.close(&mut reg, &ast);
    let enumeration = EnumTypeBuilder::new()
        .set_name("Numbers")
        .set_element_type(number)
        .build(&mut reg, &ast);
    enumeration.define_element(&mut reg, &ast, "ONE", None);
    enumeration.define_element(&mut reg, &ast, "TWO", None);
    let element = enumeration.get_elements_type(&reg);
    let enum_union = reg.create_union_type(&ast, &[element, string, null]);
    let array = reg.get_native_object_type(JSTypeNative::ARRAY_TYPE);
    let array_record = reg.create_templatized_type(&ast, array, &[record]);
    let element_null = reg.create_union_type(&ast, &[element, null]);
    let array_enum = reg.create_templatized_type(&ast, array, &[element_null]);
    let readonly_map = reg.get_native_object_type(JSTypeNative::READONLY_MAP_TYPE);
    let map = reg.create_templatized_type(&ast, readonly_map, &[string, array_enum]);
    let nested_union = reg.create_union_type(&ast, &[this_fn, array_record, element, null]);
    let bounded = reg.create_template_type_with_bound(&ast, "Bounded", number_string);
    let types = [
        ("large_record", many),
        ("small_record", record),
        ("this_function", this_fn),
        ("constructor", ctor),
        ("constructor_instance", instance),
        ("constructor_prototype", prototype),
        ("declared_constructor", declared),
        ("enum", enumeration),
        ("enum_element", element),
        ("enum_union", enum_union),
        ("array_record", array_record),
        ("map_enum_array", map),
        ("nested_union", nested_union),
        ("bounded_template", bounded),
    ];
    for (label, type_) in types {
        let &(_, string, explicit, implicit, display) = EXTRA_EXPECTATIONS
            .iter()
            .find(|row| row.0 == label)
            .unwrap();
        assert_printing(
            &mut reg, &ast, label, type_, string, explicit, implicit, display,
        );
    }
    let mut closer = reg.get_resolver().open_for_definition();
    let named = NamedTypeBuilder::new(&reg, "Future")
        .set_resolution_kind(ResolutionKind::TYPE_NAME)
        .build(&mut reg, &ast);
    let &(_, string, explicit, implicit, display) = EXTRA_EXPECTATIONS
        .iter()
        .find(|row| row.0 == "named_before_resolution")
        .unwrap();
    assert_printing(
        &mut reg,
        &ast,
        "named_before_resolution",
        named,
        string,
        explicit,
        implicit,
        display,
    );
    closer.close(&mut reg, &ast);
    let &(_, string, explicit, implicit, display) = EXTRA_EXPECTATIONS
        .iter()
        .find(|row| row.0 == "named_after_resolution")
        .unwrap();
    assert_printing(
        &mut reg,
        &ast,
        "named_after_resolution",
        named,
        string,
        explicit,
        implicit,
        display,
    );
    let parameters = [
        Parameter::create(number, false, false),
        Parameter::create(number_string, true, false),
        Parameter::create(array_record, false, true),
    ];
    for (parameter, expected) in parameters.iter().zip(PARAMETER_EXPECTATIONS) {
        assert_eq!(parameter.to_string(&mut reg, &ast), *expected);
    }
}

const NATIVE_EXPECTATIONS: &[(JSTypeNative, &str, &str, &str, Option<&str>)] = &[
    (
        JSTypeNative::ARGUMENTS_TYPE,
        "Arguments",
        "!Arguments",
        "Arguments",
        Some("Arguments"),
    ),
    (
        JSTypeNative::ARGUMENTS_FUNCTION_TYPE,
        "function(new:Arguments): ?",
        "function(new:Arguments): ?",
        "function(new:Arguments): ?",
        Some("Arguments"),
    ),
    (
        JSTypeNative::ARRAY_TYPE,
        "Array",
        "!Array",
        "Array",
        Some("Array"),
    ),
    (
        JSTypeNative::ARRAY_FUNCTION_TYPE,
        "function(new:Array, ...*): Array",
        "function(new:Array, ...*): !Array",
        "function(new:Array, ...*): !Array",
        Some("Array"),
    ),
    (
        JSTypeNative::ASYNC_ITERABLE_FUNCTION_TYPE,
        "function(this:AsyncIterable): ?",
        "function(this:AsyncIterable): ?",
        "function(this:AsyncIterable): ?",
        Some("AsyncIterable"),
    ),
    (
        JSTypeNative::ASYNC_ITERABLE_TYPE,
        "AsyncIterable",
        "!AsyncIterable",
        "AsyncIterable",
        Some("AsyncIterable"),
    ),
    (
        JSTypeNative::ASYNC_ITERATOR_FUNCTION_TYPE,
        "function(this:AsyncIterator): ?",
        "function(this:AsyncIterator): ?",
        "function(this:AsyncIterator): ?",
        Some("AsyncIterator"),
    ),
    (
        JSTypeNative::ASYNC_ITERATOR_TYPE,
        "AsyncIterator",
        "!AsyncIterator",
        "AsyncIterator",
        Some("AsyncIterator"),
    ),
    (
        JSTypeNative::ASYNC_ITERATOR_ITERABLE_FUNCTION_TYPE,
        "function(this:AsyncIteratorIterable): ?",
        "function(this:AsyncIteratorIterable): ?",
        "function(this:AsyncIteratorIterable): ?",
        Some("AsyncIteratorIterable"),
    ),
    (
        JSTypeNative::ASYNC_ITERATOR_ITERABLE_TYPE,
        "AsyncIteratorIterable",
        "!AsyncIteratorIterable",
        "AsyncIteratorIterable",
        Some("AsyncIteratorIterable"),
    ),
    (
        JSTypeNative::ASYNC_GENERATOR_FUNCTION_TYPE,
        "function(this:AsyncGenerator): ?",
        "function(this:AsyncGenerator): ?",
        "function(this:AsyncGenerator): ?",
        Some("AsyncGenerator"),
    ),
    (
        JSTypeNative::ASYNC_GENERATOR_TYPE,
        "AsyncGenerator",
        "!AsyncGenerator",
        "AsyncGenerator",
        Some("AsyncGenerator"),
    ),
    (
        JSTypeNative::BIGINT_TYPE,
        "bigint",
        "bigint",
        "bigint",
        Some("bigint"),
    ),
    (
        JSTypeNative::BIGINT_OBJECT_TYPE,
        "BigInt",
        "!BigInt",
        "BigInt",
        Some("BigInt"),
    ),
    (
        JSTypeNative::BIGINT_OBJECT_FUNCTION_TYPE,
        "function(new:BigInt, (bigint|number|string)): bigint",
        "function(new:BigInt, (bigint|number|string)): bigint",
        "function(new:BigInt, (bigint|number|string)): bigint",
        Some("BigInt"),
    ),
    (
        JSTypeNative::BOOLEAN_TYPE,
        "boolean",
        "boolean",
        "boolean",
        Some("boolean"),
    ),
    (
        JSTypeNative::BOOLEAN_OBJECT_TYPE,
        "Boolean",
        "!Boolean",
        "Boolean",
        Some("Boolean"),
    ),
    (
        JSTypeNative::BOOLEAN_OBJECT_FUNCTION_TYPE,
        "function(new:Boolean, *=): boolean",
        "function(new:Boolean, *=): boolean",
        "function(new:Boolean, *=): boolean",
        Some("Boolean"),
    ),
    (
        JSTypeNative::CHECKED_UNKNOWN_TYPE,
        "??",
        "??",
        "??",
        Some("Unknown"),
    ),
    (
        JSTypeNative::DATE_TYPE,
        "Date",
        "!Date",
        "Date",
        Some("Date"),
    ),
    (
        JSTypeNative::DATE_FUNCTION_TYPE,
        "function(new:Date, ?=, ?=, ?=, ?=, ?=, ?=, ?=): string",
        "function(new:Date, ?=, ?=, ?=, ?=, ?=, ?=, ?=): string",
        "function(new:Date, ?=, ?=, ?=, ?=, ?=, ?=, ?=): string",
        Some("Date"),
    ),
    (
        JSTypeNative::FUNCTION_TYPE,
        "Function",
        "!Function",
        "!Function",
        Some("Function"),
    ),
    (
        JSTypeNative::FUNCTION_FUNCTION_TYPE,
        "function(new:Function, ...*): ?",
        "function(new:!Function, ...*): ?",
        "function(new:!Function, ...*): ?",
        Some("Function"),
    ),
    (
        JSTypeNative::FUNCTION_PROTOTYPE,
        "Function.prototype",
        "!Function.prototype",
        "Function.prototype",
        Some("Function.prototype"),
    ),
    (
        JSTypeNative::FUNCTION_INSTANCE_PROTOTYPE,
        "?.prototype",
        "!?.prototype",
        "?.prototype",
        Some("?.prototype"),
    ),
    (
        JSTypeNative::GBIGINT_TYPE,
        "gbigint",
        "!gbigint",
        "gbigint",
        Some("gbigint"),
    ),
    (
        JSTypeNative::GENERATOR_FUNCTION_TYPE,
        "function(this:Generator): ?",
        "function(this:Generator): ?",
        "function(this:Generator): ?",
        Some("Generator"),
    ),
    (
        JSTypeNative::GENERATOR_TYPE,
        "Generator",
        "!Generator",
        "Generator",
        Some("Generator"),
    ),
    (
        JSTypeNative::I_ITERABLE_RESULT_FUNCTION_TYPE,
        "function(this:IIterableResult): ?",
        "function(this:IIterableResult): ?",
        "function(this:IIterableResult): ?",
        Some("IIterableResult"),
    ),
    (
        JSTypeNative::I_ITERABLE_RESULT_TYPE,
        "IIterableResult",
        "!IIterableResult",
        "IIterableResult",
        Some("IIterableResult"),
    ),
    (
        JSTypeNative::ITERABLE_FUNCTION_TYPE,
        "function(this:Iterable): ?",
        "function(this:Iterable): ?",
        "function(this:Iterable): ?",
        Some("Iterable"),
    ),
    (
        JSTypeNative::ITERABLE_TYPE,
        "Iterable",
        "!Iterable",
        "Iterable",
        Some("Iterable"),
    ),
    (
        JSTypeNative::ITERATOR_LIKE_FUNCTION_TYPE,
        "function(this:IteratorLike): ?",
        "function(this:IteratorLike): ?",
        "function(this:IteratorLike): ?",
        Some("IteratorLike"),
    ),
    (
        JSTypeNative::ITERATOR_LIKE_TYPE,
        "IteratorLike",
        "!IteratorLike",
        "IteratorLike",
        Some("IteratorLike"),
    ),
    (
        JSTypeNative::ITERATOR_FUNCTION_TYPE,
        "function(new:Iterator): ?",
        "function(new:Iterator): ?",
        "function(new:Iterator): ?",
        Some("Iterator"),
    ),
    (
        JSTypeNative::ITERATOR_TYPE,
        "Iterator",
        "!Iterator",
        "Iterator",
        Some("Iterator"),
    ),
    (
        JSTypeNative::ITERATOR_ITERABLE_FUNCTION_TYPE,
        "function(this:IteratorIterable): ?",
        "function(this:IteratorIterable): ?",
        "function(this:IteratorIterable): ?",
        Some("IteratorIterable"),
    ),
    (
        JSTypeNative::ITERATOR_ITERABLE_TYPE,
        "IteratorIterable",
        "!IteratorIterable",
        "IteratorIterable",
        Some("IteratorIterable"),
    ),
    (
        JSTypeNative::I_ARRAY_LIKE_FUNCTION_TYPE,
        "function(this:IArrayLike): ?",
        "function(this:IArrayLike): ?",
        "function(this:IArrayLike): ?",
        Some("IArrayLike"),
    ),
    (
        JSTypeNative::I_ARRAY_LIKE_TYPE,
        "IArrayLike",
        "!IArrayLike",
        "IArrayLike",
        Some("IArrayLike"),
    ),
    (
        JSTypeNative::I_TEMPLATE_ARRAY_TYPE,
        "ITemplateArray",
        "!ITemplateArray",
        "ITemplateArray",
        Some("ITemplateArray"),
    ),
    (
        JSTypeNative::I_OBJECT_FUNCTION_TYPE,
        "function(this:IObject): ?",
        "function(this:IObject): ?",
        "function(this:IObject): ?",
        Some("IObject"),
    ),
    (
        JSTypeNative::I_OBJECT_TYPE,
        "IObject",
        "!IObject",
        "IObject",
        Some("IObject"),
    ),
    (
        JSTypeNative::I_THENABLE_FUNCTION_TYPE,
        "function(this:IThenable): ?",
        "function(this:IThenable): ?",
        "function(this:IThenable): ?",
        Some("IThenable"),
    ),
    (
        JSTypeNative::I_THENABLE_TYPE,
        "IThenable",
        "!IThenable",
        "IThenable",
        Some("IThenable"),
    ),
    (
        JSTypeNative::NULL_TYPE,
        "null",
        "null",
        "null",
        Some("null"),
    ),
    (
        JSTypeNative::NUMBER_TYPE,
        "number",
        "number",
        "number",
        Some("number"),
    ),
    (
        JSTypeNative::NUMBER_OBJECT_TYPE,
        "Number",
        "!Number",
        "Number",
        Some("Number"),
    ),
    (
        JSTypeNative::NUMBER_OBJECT_FUNCTION_TYPE,
        "function(new:Number, *=): number",
        "function(new:Number, *=): number",
        "function(new:Number, *=): number",
        Some("Number"),
    ),
    (
        JSTypeNative::PROMISE_TYPE,
        "Promise",
        "!Promise",
        "Promise",
        Some("Promise"),
    ),
    (
        JSTypeNative::PROMISE_FUNCTION_TYPE,
        "function(new:Promise, function(function((IThenable<TYPE>|TYPE|null|{then: ?})=): ?, function(*=): ?): ?): ?",
        "function(new:Promise, function(function((IThenable<TYPE>|TYPE|null|{then: ?})=): ?, function(*=): ?): ?): ?",
        "function(new:Promise, function(function((IThenable<TYPE>|TYPE|null|{then: ?})=): ?, function(*=): ?): ?): ?",
        Some("Promise"),
    ),
    (
        JSTypeNative::OBJECT_TYPE,
        "Object",
        "!Object",
        "Object",
        Some("Object"),
    ),
    (
        JSTypeNative::OBJECT_FUNCTION_TYPE,
        "function(new:Object, *=): Object",
        "function(new:Object, *=): !Object",
        "function(new:Object, *=): !Object",
        Some("Object"),
    ),
    (
        JSTypeNative::OBJECT_PROTOTYPE,
        "Object.prototype",
        "!Object.prototype",
        "Object.prototype",
        Some("Object.prototype"),
    ),
    (
        JSTypeNative::READONLY_ARRAY_TYPE,
        "ReadonlyArray",
        "!ReadonlyArray",
        "ReadonlyArray",
        Some("ReadonlyArray"),
    ),
    (
        JSTypeNative::READONLY_ARRAY_FUNCTION_TYPE,
        "function(this:ReadonlyArray): ?",
        "function(this:ReadonlyArray): ?",
        "function(this:ReadonlyArray): ?",
        Some("ReadonlyArray"),
    ),
    (
        JSTypeNative::READONLY_MAP_TYPE,
        "ReadonlyMap",
        "!ReadonlyMap",
        "ReadonlyMap",
        Some("ReadonlyMap"),
    ),
    (
        JSTypeNative::READONLY_MAP_FUNCTION_TYPE,
        "function(this:ReadonlyMap): ?",
        "function(this:ReadonlyMap): ?",
        "function(this:ReadonlyMap): ?",
        Some("ReadonlyMap"),
    ),
    (JSTypeNative::MAP_TYPE, "Map", "!Map", "Map", Some("Map")),
    (
        JSTypeNative::MAP_FUNCTION_TYPE,
        "function(new:Map, (Array<Array<(K|V)>>|Iterable<Array<(K|V)>,?,?>|null)=): ?",
        "function(new:Map, (Array<Array<(K|V)>>|Iterable<Array<(K|V)>,?,?>|null)=): ?",
        "function(new:Map, (Array<Array<(K|V)>>|Iterable<Array<(K|V)>,?,?>|null)=): ?",
        Some("Map"),
    ),
    (
        JSTypeNative::REGEXP_TYPE,
        "RegExp",
        "!RegExp",
        "RegExp",
        Some("RegExp"),
    ),
    (
        JSTypeNative::REGEXP_FUNCTION_TYPE,
        "function(new:RegExp, *=, *=): RegExp",
        "function(new:RegExp, *=, *=): !RegExp",
        "function(new:RegExp, *=, *=): !RegExp",
        Some("RegExp"),
    ),
    (
        JSTypeNative::STRING_OBJECT_TYPE,
        "String",
        "!String",
        "String",
        Some("String"),
    ),
    (
        JSTypeNative::STRING_OBJECT_FUNCTION_TYPE,
        "function(new:String, *=): string",
        "function(new:String, *=): string",
        "function(new:String, *=): string",
        Some("String"),
    ),
    (
        JSTypeNative::STRING_TYPE,
        "string",
        "string",
        "string",
        Some("string"),
    ),
    (
        JSTypeNative::SYMBOL_OBJECT_TYPE,
        "Symbol",
        "!Symbol",
        "Symbol",
        Some("Symbol"),
    ),
    (
        JSTypeNative::SYMBOL_OBJECT_FUNCTION_TYPE,
        "function(new:Symbol, *=): symbol",
        "function(new:Symbol, *=): symbol",
        "function(new:Symbol, *=): symbol",
        Some("Symbol"),
    ),
    (
        JSTypeNative::SYMBOL_TYPE,
        "symbol",
        "symbol",
        "symbol",
        Some("symbol"),
    ),
    (
        JSTypeNative::THENABLE_TYPE,
        "{then: ?}",
        "{then: ?}",
        "{then: ?}",
        None,
    ),
    (JSTypeNative::UNKNOWN_TYPE, "?", "?", "?", Some("Unknown")),
    (
        JSTypeNative::VOID_TYPE,
        "undefined",
        "undefined",
        "undefined",
        Some("undefined"),
    ),
    (JSTypeNative::ALL_TYPE, "*", "*", "*", Some("<Any Type>")),
    (JSTypeNative::NO_TYPE, "None", "!?", "?", None),
    (JSTypeNative::NO_OBJECT_TYPE, "NoObject", "!?", "?", None),
    (
        JSTypeNative::GLOBAL_THIS,
        "global this",
        "!global this",
        "global this",
        Some("global this"),
    ),
    (
        JSTypeNative::LEAST_FUNCTION_TYPE,
        "function(...*): None",
        "function(...*): !?",
        "function(...*): !?",
        None,
    ),
    (
        JSTypeNative::GREATEST_FUNCTION_TYPE,
        "function(...None): *",
        "function(...!?): *",
        "function(...!?): *",
        None,
    ),
    (
        JSTypeNative::NULL_VOID,
        "(null|undefined)",
        "(null|undefined)",
        "(null|undefined)",
        None,
    ),
    (
        JSTypeNative::NUMBER_STRING_BOOLEAN,
        "(boolean|number|string)",
        "(boolean|number|string)",
        "(boolean|number|string)",
        None,
    ),
    (
        JSTypeNative::VALUE_TYPES,
        "(boolean|number|string|symbol)",
        "(boolean|number|string|symbol)",
        "(boolean|number|string|symbol)",
        None,
    ),
    (
        JSTypeNative::NUMBER_SYMBOL,
        "(number|symbol)",
        "(number|symbol)",
        "(number|symbol)",
        None,
    ),
    (
        JSTypeNative::STRING_SYMBOL,
        "(string|symbol)",
        "(string|symbol)",
        "(string|symbol)",
        None,
    ),
    (
        JSTypeNative::NUMBER_STRING,
        "(number|string)",
        "(number|string)",
        "(number|string)",
        None,
    ),
    (
        JSTypeNative::NUMBER_STRING_SYMBOL,
        "(number|string|symbol)",
        "(number|string|symbol)",
        "(number|string|symbol)",
        None,
    ),
    (
        JSTypeNative::BIGINT_NUMBER,
        "(bigint|number)",
        "(bigint|number)",
        "(bigint|number)",
        None,
    ),
    (
        JSTypeNative::BIGINT_NUMBER_OBJECT,
        "(BigInt|Number)",
        "(BigInt|Number)",
        "(BigInt|Number)",
        None,
    ),
    (
        JSTypeNative::BIGINT_NUMBER_STRING,
        "(bigint|number|string)",
        "(bigint|number|string)",
        "(bigint|number|string)",
        None,
    ),
    (
        JSTypeNative::BIGINT_NUMBER_STRING_OBJECT,
        "(BigInt|Number|String)",
        "(BigInt|Number|String)",
        "(BigInt|Number|String)",
        None,
    ),
    (
        JSTypeNative::NUMBER_ADDITION_SUPERTYPE,
        "(Boolean|Number|boolean|null|number|undefined)",
        "(Boolean|Number|boolean|null|number|undefined)",
        "(Boolean|Number|boolean|null|number|undefined)",
        None,
    ),
];

const EXTRA_EXPECTATIONS: &[(&str, &str, &str, &str, Option<&str>)] = &[
    (
        "large_record",
        "{\n  0: number,\n  99: number,\n  a: number,\n  beta: number,\n  delta: number,\n  double\"quote: number,\n  epsilon: number,\n  gamma: number,\n  line\nbreak: number,\n  m: number,\n  ...\n}",
        "{0: number, 99: number, a: number, beta: number, delta: number, double\"quote: number, epsilon: number, gamma: number, line\nbreak: number, m: number, q'quote: number, space name: number, z: number, 𐀀: number, : number}",
        "{0: number, 99: number, a: number, beta: number, delta: number, double\"quote: number, epsilon: number, gamma: number, line\nbreak: number, m: number, q'quote: number, space name: number, z: number, 𐀀: number, : number}",
        None,
    ),
    (
        "small_record",
        "{\n  callback: function(number): string,\n  empty: Object,\n  nullable: (null|string),\n  optional: (boolean|undefined)\n}",
        "{callback: function(number): string, empty: !Object, nullable: (null|string), optional: (boolean|undefined)}",
        "{callback: function(number): string, empty: !Object, nullable: (null|string), optional: (boolean|undefined)}",
        None,
    ),
    (
        "this_function",
        "function(this:{\n  callback: function(number): string,\n  empty: Object,\n  nullable: (null|string),\n  optional: (boolean|undefined)\n}, boolean, string=, ...(number|string)): (null|{\n  callback: function(number): string,\n  empty: Object,\n  nullable: (null|string),\n  optional: (boolean|undefined)\n})",
        "function(this:{callback: function(number): string, empty: !Object, nullable: (null|string), optional: (boolean|undefined)}, boolean, string=, ...(number|string)): (null|{callback: function(number): string, empty: !Object, nullable: (null|string), optional: (boolean|undefined)})",
        "function(this:{callback: function(number): string, empty: !Object, nullable: (null|string), optional: (boolean|undefined)}, boolean, string=, ...(number|string)): (null|{callback: function(number): string, empty: !Object, nullable: (null|string), optional: (boolean|undefined)})",
        None,
    ),
    (
        "constructor",
        "function(new:PrintCtor, boolean, string=, ...(number|string)): string",
        "function(new:PrintCtor, boolean, string=, ...(number|string)): string",
        "function(new:PrintCtor, boolean, string=, ...(number|string)): string",
        Some("PrintCtor"),
    ),
    (
        "constructor_instance",
        "PrintCtor",
        "!PrintCtor",
        "PrintCtor",
        Some("PrintCtor"),
    ),
    (
        "constructor_prototype",
        "PrintCtor.prototype",
        "!PrintCtor.prototype",
        "PrintCtor.prototype",
        Some("PrintCtor.prototype"),
    ),
    (
        "declared_constructor",
        "(typeof Declared)",
        "(typeof Declared)",
        "(typeof Declared)",
        Some("Declared"),
    ),
    (
        "enum",
        "enum{Numbers}",
        "!!Object",
        "!Object",
        Some("Numbers"),
    ),
    (
        "enum_element",
        "Numbers<number>",
        "number",
        "number",
        Some("Numbers"),
    ),
    (
        "enum_union",
        "(Numbers<number>|null|string)",
        "(null|number|string)",
        "(null|number|string)",
        None,
    ),
    (
        "array_record",
        "Array<{\n  callback: function(number): string,\n  empty: Object,\n  nullable: (null|string),\n  optional: (boolean|undefined)\n}>",
        "!Array<{callback: function(number): string, empty: !Object, nullable: (null|string), optional: (boolean|undefined)}>",
        "Array<{callback: function(number): string, empty: !Object, nullable: (null|string), optional: (boolean|undefined)}>",
        Some("Array"),
    ),
    (
        "map_enum_array",
        "ReadonlyMap<string,Array<(Numbers<number>|null)>>",
        "!ReadonlyMap<string,Array<(null|number)>>",
        "ReadonlyMap<string,Array<(null|number)>>",
        Some("ReadonlyMap"),
    ),
    (
        "nested_union",
        "(Array<{\n  callback: function(number): string,\n  empty: Object,\n  nullable: (null|string),\n  optional: (boolean|undefined)\n}>|Numbers<number>|function(this:{\n  callback: function(number): string,\n  empty: Object,\n  nullable: (null|string),\n  optional: (boolean|undefined)\n}, boolean, string=, ...(number|string)): (null|{\n  callback: function(number): string,\n  empty: Object,\n  nullable: (null|string),\n  optional: (boolean|undefined)\n})|null)",
        "(Array<{callback: function(number): string, empty: !Object, nullable: (null|string), optional: (boolean|undefined)}>|function(this:{callback: function(number): string, empty: !Object, nullable: (null|string), optional: (boolean|undefined)}, boolean, string=, ...(number|string)): (null|{callback: function(number): string, empty: !Object, nullable: (null|string), optional: (boolean|undefined)})|null|number)",
        "(Array<{callback: function(number): string, empty: !Object, nullable: (null|string), optional: (boolean|undefined)}>|function(this:{callback: function(number): string, empty: !Object, nullable: (null|string), optional: (boolean|undefined)}, boolean, string=, ...(number|string)): (null|{callback: function(number): string, empty: !Object, nullable: (null|string), optional: (boolean|undefined)})|null|number)",
        None,
    ),
    (
        "bounded_template",
        "Bounded extends (number|string)",
        "Bounded extends (number|string)",
        "Bounded extends (number|string)",
        Some("Bounded"),
    ),
    (
        "named_before_resolution",
        "Future",
        "Future",
        "Future",
        Some("Future"),
    ),
    ("named_after_resolution", "?", "?", "?", Some("Future")),
];

const PARAMETER_EXPECTATIONS: &[&str] = &[
    "Parameter{JSType=number, optional=false, variadic=false}",
    "Parameter{JSType=(number|string), optional=true, variadic=false}",
    "Parameter{JSType=Array<{\n  callback: function(number): string,\n  empty: Object,\n  nullable: (null|string),\n  optional: (boolean|undefined)\n}>, optional=false, variadic=true}",
];
