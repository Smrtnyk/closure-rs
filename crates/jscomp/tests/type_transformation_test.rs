/*
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2014 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/CompilerTypeTestCase.java,
//   test/com/google/javascript/jscomp/TypeTransformationTest.java.

//! Port of TypeTransformationTest (a CompilerTypeTestCase without JUnit records).
use closure_jscomp::{
    compiler::Compiler, source_file::SourceFile, type_transformation::TypeTransformation,
    typed_scope_creator::TypedScopeCreator,
};
use closure_jstype::{
    TypeId, js_type::JSType, js_type_native::JSTypeNative, js_type_native::JSTypeNative::*,
    object_type::ObjectType, record_type_builder::RecordTypeBuilder,
    testing::type_subject::TypeSubject,
};
use closure_parsing::{
    parser::feature_set::Feature, type_transformation_parser::TypeTransformationParser,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    ir::IR, js_string::JsString, static_source_file::StaticSourceFile,
    testing::test_error_reporter::TestErrorReporter,
};
use closure_testing::compiler_type_test_case::CompilerTypeTestCase;
use std::cell::RefCell;
use std::sync::Arc;

// port: TypeTransformationTest#EXTRA_TYPE_DEFS
const EXTRA_TYPE_DEFS: &str = "/** @type {number} */ Array.prototype.length;
/** @typedef {!Array<?>} */ var ArrayAlias;

/** @constructor */
function Bar() {}

/** @type {number} */
var n = 10;
";

/// The test fixture. Java's fields are mutated through `&self` from nested helper calls
/// (`union(record(..), ..)`), so the compiler lives in a RefCell.
struct TypeTransformationTest {
    compiler: RefCell<Compiler>,
    error_reporter: RefCell<TestErrorReporter>,
    type_vars: IndexMap<JsString, TypeId>,
    name_vars: IndexMap<JsString, JsString>,
    record_type_test: Option<TypeId>,
    nested_record_type_test: Option<TypeId>,
    asynch_record: Option<TypeId>,
}

impl TypeTransformationTest {
    // port: TypeTransformationTest#setUp
    fn set_up() -> Self {
        // super.setUp(): CompilerTypeTestCase#setUp / #initializeNewCompiler
        let mut compiler = Compiler::new();
        compiler.init_options(CompilerTypeTestCase::default_options().unwrap());
        compiler.mark_feature_not_allowed(Feature::MODULES);
        let mut t = Self {
            compiler: RefCell::new(compiler),
            error_reporter: RefCell::new(TestErrorReporter::new()),
            type_vars: IndexMap::<_, _>::default(),
            name_vars: IndexMap::<_, _>::default(),
            record_type_test: None,
            nested_record_type_test: None,
            asynch_record: None,
        };
        t.init_record_type_tests();
        let arrnum = t.type_(t.n(ARRAY_TYPE), &[t.n(NUMBER_TYPE)]);
        let type_vars = [
            ("S", t.n(STRING_TYPE)),
            ("N", t.n(NUMBER_TYPE)),
            ("B", t.n(BOOLEAN_TYPE)),
            ("BOT", t.n(NO_TYPE)),
            ("TOP", t.n(ALL_TYPE)),
            ("UNK", t.n(UNKNOWN_TYPE)),
            ("CHKUNK", t.n(CHECKED_UNKNOWN_TYPE)),
            ("SO", t.n(STRING_OBJECT_TYPE)),
            ("NO", t.n(NUMBER_OBJECT_TYPE)),
            ("BO", t.n(BOOLEAN_OBJECT_TYPE)),
            ("NULL", t.n(NULL_TYPE)),
            ("OBJ", t.n(OBJECT_TYPE)),
            ("UNDEF", t.n(VOID_TYPE)),
            ("ARR", t.n(ARRAY_TYPE)),
            ("ARRNUM", arrnum),
            ("REC", t.record_type_test.unwrap()),
            ("NESTEDREC", t.nested_record_type_test.unwrap()),
            ("ASYNCH", t.asynch_record.unwrap()),
        ];
        t.type_vars = type_vars
            .into_iter()
            .map(|(k, v)| (JsString::from(k), v))
            .collect();
        t.name_vars = [("s", "string"), ("n", "number"), ("b", "boolean")]
            .into_iter()
            .map(|(k, v)| (JsString::from(k), JsString::from(v)))
            .collect();
        t
    }

    // port: CompilerTypeTestCase#validateWarningsAndErrors
    fn validate_warnings_and_errors(&self) {
        self.error_reporter
            .borrow()
            .verify_has_encountered_all_warnings_and_errors();
    }

    /// Java's `getNative*Type()` getters of CompilerTypeTestCase.
    fn n(&self, native: JSTypeNative) -> TypeId {
        self.compiler
            .borrow_mut()
            .get_type_registry()
            .get_native_type(native)
    }

    // port: TypeTransformationTest#initRecordTypeTests
    fn init_record_type_tests(&mut self) {
        // {n:number, s:string, b:boolean}
        self.record_type_test = Some(self.record(&[
            ("n", self.n(NUMBER_TYPE)),
            ("s", self.n(STRING_TYPE)),
            ("b", self.n(BOOLEAN_TYPE)),
        ]));
        // {n:number, r:{s:string, b:boolean}}
        self.nested_record_type_test = Some(self.record(&[
            ("s", self.n(STRING_TYPE)),
            (
                "r",
                self.record(&[("s", self.n(STRING_TYPE)), ("b", self.n(BOOLEAN_TYPE))]),
            ),
        ]));
        // {service:Array<number>}
        self.asynch_record = Some(self.record(&[(
            "service",
            self.type_(self.n(ARRAY_TYPE), &[self.n(NUMBER_TYPE)]),
        )]));
    }

    // port: TypeTransformationTest#union
    fn union(&self, variants: &[TypeId]) -> TypeId {
        // createUnionType: CompilerTypeTestCase#createUnionType
        let mut compiler = self.compiler.borrow_mut();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        let type_ = reg.create_union_type(ast, variants);
        assert!(type_.is_union_type(reg));
        type_
    }

    // port: TypeTransformationTest#type
    fn type_(&self, base_type: TypeId, templatized_types: &[TypeId]) -> TypeId {
        // createTemplatizedType: CompilerTypeTestCase#createTemplatizedType
        let mut compiler = self.compiler.borrow_mut();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        reg.create_templatized_type(ast, base_type, templatized_types)
    }

    // port: TypeTransformationTest#record
    fn record(&self, props: &[(&str, TypeId)]) -> TypeId {
        // createRecordTypeBuilder: CompilerTypeTestCase#createRecordTypeBuilder
        let mut builder = RecordTypeBuilder::new();
        for (key, value) in props {
            builder.add_property(*key, *value, None);
        }
        let mut compiler = self.compiler.borrow_mut();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        builder.build(reg, ast)
    }

    /// `getNativeArrayType().defineDeclaredProperty(name, type, null)`.
    fn define_declared_property(&self, owner: TypeId, name: &str, type_: TypeId) {
        let mut compiler = self.compiler.borrow_mut();
        let (reg, ast) = compiler.get_type_registry_and_ast();
        owner.define_declared_property(reg, ast, name, type_, None);
    }

    // port: TypeTransformationTest#testTTL
    fn test_ttl(&self, expected_type: TypeId, ttl_exp: &str, expected_warnings: &[&str]) {
        let mut compiler = self.compiler.borrow_mut();
        let compiler = &mut *compiler;
        let mut closer = compiler
            .get_type_registry()
            .get_resolver()
            .open_for_definition();
        let source: Arc<dyn StaticSourceFile> =
            Arc::new(SourceFile::from_code("[testcode]", ttl_exp));
        let mut error_reporter = self.error_reporter.borrow_mut();
        let mut ttl_parser = TypeTransformationParser::new(
            JsString::from(ttl_exp),
            Some(source),
            &mut *error_reporter,
            0,
            0,
        );
        // Run the test if the parsing was successful
        if ttl_parser.parse_type_transformation(compiler) {
            let ast = ttl_parser.get_type_transformation_ast().unwrap();
            drop(ttl_parser);
            // Create the scope using the extra definitions
            let extra_type_defs = compiler.parse_test_code(EXTRA_TYPE_DEFS);
            let externs_root = IR::root(compiler, &[]);
            let main_root = IR::root(compiler, &[extra_type_defs]);
            let root = IR::root(compiler, &[externs_root, main_root]);
            let scope = TypedScopeCreator::new(compiler).create_scope(compiler, root, None);
            // Evaluate the type transformation
            let type_transformation =
                TypeTransformation::new(scope.as_static_typed_scope_arc(compiler));
            let result_type = type_transformation.eval_with_name_vars(
                compiler,
                ast,
                self.type_vars.clone(),
                self.name_vars.clone(),
            );
            // checkReportedWarningsHelper: CompilerTypeTestCase#checkReportedWarningsHelper
            let warnings = compiler
                .get_warnings()
                .iter()
                .map(|w| w.get_description().to_owned())
                .collect::<Vec<_>>();
            assert_eq!(warnings, expected_warnings, "Regarding warnings:");
            // assertTypeEquals: CompilerTypeTestCase#assertTypeEquals
            let (reg, ast) = compiler.get_type_registry_and_ast();
            TypeSubject::assert_type(result_type).is_equal_to(reg, ast, expected_type);
        }
        // try-with-resources: the closer closes when the block ends.
        let (reg, ast) = compiler.get_type_registry_and_ast();
        closer.close(reg, ast);
    }
}

// port: TypeTransformationTest#testTransformationWithValidBasicTypePredicate
#[test]
fn test_transformation_with_valid_basic_type_predicate() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(NUMBER_TYPE), "'number'", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithBasicTypePredicateWithInvalidTypename
#[test]
fn test_transformation_with_basic_type_predicate_with_invalid_typename() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "'foo'",
        &["Reference to an unknown type name foo"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithSingleTypeVar
#[test]
fn test_transformation_with_single_type_var() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(STRING_TYPE), "S", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithMultipleTypeVars
#[test]
fn test_transformation_with_multiple_type_vars() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(STRING_TYPE), "S", &[]);
    t.test_ttl(t.n(NUMBER_TYPE), "N", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithValidUnionTypeOnlyVars
#[test]
fn test_transformation_with_valid_union_type_only_vars() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.union(&[t.n(NUMBER_TYPE), t.n(STRING_TYPE)]),
        "union(N, S)",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithValidUnionTypeOnlyTypePredicates
#[test]
fn test_transformation_with_valid_union_type_only_type_predicates() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.union(&[t.n(NUMBER_TYPE), t.n(STRING_TYPE)]),
        "union('number', 'string')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithValidUnionTypeMixed
#[test]
fn test_transformation_with_valid_union_type_mixed() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.union(&[t.n(NUMBER_TYPE), t.n(STRING_TYPE)]),
        "union(S, 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithUnknownParameter
#[test]
fn test_transformation_with_unknown_parameter() {
    let t = TypeTransformationTest::set_up();
    // Returns ? because foo is not defined
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "union(foo, 'number')",
        &["Reference to an unknown type variable foo"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithUnknownParameter2
#[test]
fn test_transformation_with_unknown_parameter2() {
    let t = TypeTransformationTest::set_up();
    // Returns ? because foo is not defined
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "union(N, 'foo')",
        &["Reference to an unknown type name foo"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedUnionInFirstParameter
#[test]
fn test_transformation_with_nested_union_in_first_parameter() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.union(&[t.n(NUMBER_TYPE), t.n(NULL_TYPE), t.n(STRING_TYPE)]),
        "union(union(N, 'null'), S)",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedUnionInSecondParameter
#[test]
fn test_transformation_with_nested_union_in_second_parameter() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.union(&[t.n(NUMBER_TYPE), t.n(NULL_TYPE), t.n(STRING_TYPE)]),
        "union(N, union('null', S))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithRepeatedTypePredicate
#[test]
fn test_transformation_with_repeated_type_predicate() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(NUMBER_TYPE), "union('number', 'number')", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithUndefinedTypeVar
#[test]
fn test_transformation_with_undefined_type_var() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "foo",
        &["Reference to an unknown type variable foo"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueEqtypeConditional
#[test]
fn test_transformation_with_true_eqtype_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(STRING_TYPE), "cond(eq(N, N), 'string', 'number')", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithFalseEqtypeConditional
#[test]
fn test_transformation_with_false_eqtype_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(NUMBER_TYPE), "cond(eq(N, S), 'string', 'number')", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueSubtypeConditional
#[test]
fn test_transformation_with_true_subtype_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond( sub('Number', 'Object'), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithFalseSubtypeConditional
#[test]
fn test_transformation_with_false_subtype_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "cond( sub('Number', 'String'), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueStreqConditional
#[test]
fn test_transformation_with_true_streq_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond(streq(n, 'number'), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueStreqConditional2
#[test]
fn test_transformation_with_true_streq_conditional2() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond(streq(n, n), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueStreqConditional3
#[test]
fn test_transformation_with_true_streq_conditional3() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond(streq('number', 'number'), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithFalseStreqConditional
#[test]
fn test_transformation_with_false_streq_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "cond(streq('number', 'foo'), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithFalseStreqConditional2
#[test]
fn test_transformation_with_false_streq_conditional2() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "cond(streq(n, 'foo'), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithFalseStreqConditional3
#[test]
fn test_transformation_with_false_streq_conditional3() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "cond(streq(n, s), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithInvalidEqConditional
#[test]
fn test_transformation_with_invalid_eq_conditional() {
    let t = TypeTransformationTest::set_up();
    // It returns number since a failing boolean expression returns false
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "cond(eq(foo, S), 'string', 'number')",
        &["Reference to an unknown type variable foo"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithInvalidStreqConditional
#[test]
fn test_transformation_with_invalid_streq_conditional() {
    let t = TypeTransformationTest::set_up();
    // It returns number since a failing boolean expression returns false
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "cond(streq(foo, s), 'string', 'number')",
        &["Reference to an unknown string variable foo"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedExpressionInBooleanFirstParam
#[test]
fn test_transformation_with_nested_expression_in_boolean_first_param() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond( eq( cond(eq(N, N), 'string', 'number'), 'string'),'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedExpressionInBooleanSecondParam
#[test]
fn test_transformation_with_nested_expression_in_boolean_second_param() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond( eq( 'string', cond(eq(N, N), 'string', 'number')),'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedExpressionInIfBranch
#[test]
fn test_transformation_with_nested_expression_in_if_branch() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_OBJECT_TYPE),
        "cond(eq(N, N), cond(eq(N, S), 'string', 'String'), 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedExpressionInElseBranch
#[test]
fn test_transformation_with_nested_expression_in_else_branch() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_OBJECT_TYPE),
        "cond(eq(N, S), 'number', cond(eq(N, S), 'string', 'String'))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithMapunionMappingEverythingToString
#[test]
fn test_transformation_with_mapunion_mapping_everything_to_string() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(STRING_TYPE), "mapunion(union(S, N), (x) => S)", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithMapunionIdentity
#[test]
fn test_transformation_with_mapunion_identity() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.union(&[t.n(NUMBER_TYPE), t.n(STRING_TYPE)]),
        "mapunion(union(N, S), (x) => x)",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithMapunionWithUnionEvaluatedToANonUnion
#[test]
fn test_transformation_with_mapunion_with_union_evaluated_to_anon_union() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "mapunion(union(N, 'number'), (x) => x)",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithMapunionFilterWithOnlyString
#[test]
fn test_transformation_with_mapunion_filter_with_only_string() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "mapunion(union(S, B, N), (x) => cond(eq(x, S), x, BOT))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithMapunionOnSingletonStringToNumber
#[test]
fn test_transformation_with_mapunion_on_singleton_string_to_number() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "mapunion(S, (x) => cond(eq(x, S), N, BOT))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedUnionInMapunionFilterString
#[test]
fn test_transformation_with_nested_union_in_mapunion_filter_string() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.union(&[t.n(NUMBER_TYPE), t.n(BOOLEAN_TYPE)]),
        "mapunion(union(union(S, B), union(N, S)), (x) => cond(eq(x, S), BOT, x))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedMapunionInMapFunctionBody
#[test]
fn test_transformation_with_nested_mapunion_in_map_function_body() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "mapunion(union(S, B),\n(x) => mapunion(union(S, N),\n(y) => cond(eq(x, y), x, BOT)))\n",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithObjectUseCase
#[test]
fn test_transformation_with_object_use_case() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(OBJECT_TYPE), "mapunion(\nunion(S, N, B, NULL, UNDEF, ARR),\n(x) =>\ncond(eq(x, S), SO,\ncond(eq(x, N), NO,\ncond(eq(x, B), BO,\ncond(eq(x, NULL), OBJ,\ncond(eq(x, UNDEF), OBJ,\nx ))))))\n", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformatioWithNoneType
#[test]
fn test_transformatio_with_none_type() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(UNKNOWN_TYPE), "none()", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformatioWithNoneTypeInConditional
#[test]
fn test_transformatio_with_none_type_in_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(UNKNOWN_TYPE), "cond(eq(BOT, none()), none(), N)", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformatioWithNoneTypeInMapunionFilterString
#[test]
fn test_transformatio_with_none_type_in_mapunion_filter_string() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "mapunion(union(S, B, N), (x) => cond(eq(x, S), x, none()))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformatioWithAllType
#[test]
fn test_transformatio_with_all_type() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(ALL_TYPE), "all()", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformatioWithAllTypeInConditional
#[test]
fn test_transformatio_with_all_type_in_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(ALL_TYPE), "cond(eq(TOP, all()), all(), N)", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformatioWithAllTypeMixUnion
#[test]
fn test_transformatio_with_all_type_mix_union() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(ALL_TYPE),
        "mapunion(union(S, B, N), (x) => cond(eq(x, S), x, all()))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformatioWithUnknownType
#[test]
fn test_transformatio_with_unknown_type() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(UNKNOWN_TYPE), "unknown()", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformatioWithUnknownTypeInConditional
#[test]
fn test_transformatio_with_unknown_type_in_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(NUMBER_TYPE), "cond(eq(UNK, unknown()), N, S)", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformatioWithUnknownTypeInMapunionStringToUnknown
#[test]
fn test_transformatio_with_unknown_type_in_mapunion_string_to_unknown() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "mapunion(union(S, B, N), (x) => cond(eq(x, S), x, unknown()))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTemplatizedType
#[test]
fn test_transformation_with_templatized_type() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.type_(t.n(ARRAY_TYPE), &[t.n(NUMBER_TYPE)]),
        "type('Array', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTemplatizedType2
#[test]
fn test_transformation_with_templatized_type2() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.type_(t.n(ARRAY_TYPE), &[t.n(NUMBER_TYPE)]),
        "type(ARR, 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTemplatizedType3
#[test]
fn test_transformation_with_templatized_type3() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.type_(t.n(ARRAY_TYPE), &[t.n(NUMBER_TYPE)]),
        "type(ARR, N)",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTemplatizedTypeInvalidBaseType
#[test]
fn test_transformation_with_templatized_type_invalid_base_type() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "type('string', 'number')",
        &["The type string cannot be templatized"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTemplatizedTypeInvalidBaseType2
#[test]
fn test_transformation_with_templatized_type_invalid_base_type2() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "type(S, 'number')",
        &["The type string cannot be templatized"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTemplatizedTypeInvalidBaseType3
#[test]
fn test_transformation_with_templatized_type_invalid_base_type3() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "type('ArrayAlias', number)",
        &["The type Array<?> cannot be templatized"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithRawTypeOf
#[test]
fn test_transformation_with_raw_type_of() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(ARRAY_TYPE), "rawTypeOf(type('Array', 'number'))", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithRawTypeOf2
#[test]
fn test_transformation_with_raw_type_of2() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(ARRAY_TYPE), "rawTypeOf(ARRNUM)", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedRawTypeOf
#[test]
fn test_transformation_with_nested_raw_type_of() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(ARRAY_TYPE),
        "rawTypeOf(type('Array', rawTypeOf(ARRNUM)))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithInvalidRawTypeOf
#[test]
fn test_transformation_with_invalid_raw_type_of() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "rawTypeOf(N)",
        &["Expected templatized type in rawTypeOf found number"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTemplateTypeOf
#[test]
fn test_transformation_with_template_type_of() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "templateTypeOf(type('Array', 'number'), 0)",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTemplateTypeOf2
#[test]
fn test_transformation_with_template_type_of2() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(NUMBER_TYPE), "templateTypeOf(ARRNUM, 0)", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedTemplateTypeOf
#[test]
fn test_transformation_with_nested_template_type_of() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "templateTypeOf(templateTypeOf(type('Array', type('Array', 'number')), 0), 0)",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithInvalidTypeTemplateTypeOf
#[test]
fn test_transformation_with_invalid_type_template_type_of() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "templateTypeOf(N, 0)",
        &["Expected templatized type in templateTypeOf found number"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithInvalidIndexTemplateTypeOf
#[test]
fn test_transformation_with_invalid_index_template_type_of() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "templateTypeOf(ARRNUM, 2)",
        &["Index out of bounds in templateTypeOf: expected a number less than 1, found 2"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithRecordType
#[test]
fn test_transformation_with_record_type() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.record(&[("x", t.n(NUMBER_TYPE))]),
        "record({x:'number'})",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithRecordType2
#[test]
fn test_transformation_with_record_type2() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.record(&[("0", t.n(NUMBER_TYPE))]),
        "record({0:'number'})",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithRecordTypeMultipleProperties
#[test]
fn test_transformation_with_record_type_multiple_properties() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.record(&[("x", t.n(NUMBER_TYPE)), ("y", t.n(STRING_TYPE))]),
        "record({x:'number', y:S})",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedRecordType
#[test]
fn test_transformation_with_nested_record_type() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.record(&[
            ("x", t.record(&[("z", t.n(BOOLEAN_TYPE))])),
            ("y", t.n(STRING_TYPE)),
        ]),
        "record({x:record({z:B}), y:S})",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedRecordType2
#[test]
fn test_transformation_with_nested_record_type2() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.record(&[("x", t.n(NUMBER_TYPE))]),
        "record(record({x:N}))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithEmptyRecordType
#[test]
fn test_transformation_with_empty_record_type() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.record(&[]), "record({})", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithMergeRecord
#[test]
fn test_transformation_with_merge_record() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.record(&[
            ("x", t.n(NUMBER_TYPE)),
            ("y", t.n(STRING_TYPE)),
            ("z", t.n(BOOLEAN_TYPE)),
        ]),
        "record({x:N}, {y:S}, {z:B})",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithMergeDuplicatedRecords
#[test]
fn test_transformation_with_merge_duplicated_records() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.record(&[("x", t.n(NUMBER_TYPE))]),
        "record({x:N}, {x:N}, {x:N})",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithMergeRecordTypeWithEmpty
#[test]
fn test_transformation_with_merge_record_type_with_empty() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.record(&[("x", t.n(NUMBER_TYPE))]),
        "record({x:N}, {})",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithInvalidRecordType
#[test]
fn test_transformation_with_invalid_record_type() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "record(N)",
        &["Expected a record type, found number"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithInvalidMergeRecordType
#[test]
fn test_transformation_with_invalid_merge_record_type() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "record({x:N}, N)",
        &["Expected a record type, found number"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTTLTypeTransformationInFirstParamMapunion
#[test]
fn test_transformation_with_ttltype_transformation_in_first_param_mapunion() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.union(&[t.n(NUMBER_TYPE), t.n(STRING_TYPE)]),
        "mapunion(templateTypeOf(type(ARR, union(N, S)), 0), (x) => x)",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithInvalidNestedMapunion
#[test]
fn test_transformation_with_invalid_nested_mapunion() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "mapunion(union(S, B),\n(x) => mapunion(union(S, N),\n(x) => cond(eq(x, x), x, BOT)))\n",
        &["The variable x is already defined"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTTLRecordWithReference
#[test]
fn test_transformation_with_ttlrecord_with_reference() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.record(&[
            ("number", t.n(NUMBER_TYPE)),
            ("string", t.n(STRING_TYPE)),
            ("boolean", t.n(BOOLEAN_TYPE)),
        ]),
        "record({[n]:N, [s]:S, [b]:B})",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTTLRecordWithInvalidReference
#[test]
fn test_transformation_with_ttlrecord_with_invalid_reference() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "record({[Foo]:N})",
        &[
            "Expected a record type, found ?",
            "Reference to an unknown name variable Foo",
        ],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithMaprecordMappingEverythingToString
#[test]
fn test_transformation_with_maprecord_mapping_everything_to_string() {
    let t = TypeTransformationTest::set_up();
    // {n:number, s:string, b:boolean}
    // is transformed to
    // {n:string, s:string, b:string}
    t.test_ttl(
        t.record(&[
            ("n", t.n(STRING_TYPE)),
            ("s", t.n(STRING_TYPE)),
            ("b", t.n(STRING_TYPE)),
        ]),
        "maprecord(REC, (k, v) => record({[k]:S}))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithMaprecordIdentity
#[test]
fn test_transformation_with_maprecord_identity() {
    let t = TypeTransformationTest::set_up();
    // {n:number, s:string, b:boolean} remains the same
    t.test_ttl(
        t.record_type_test.unwrap(),
        "maprecord(REC, (k, v) => record({[k]:v}))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithMaprecordDeleteEverything
#[test]
fn test_transformation_with_maprecord_delete_everything() {
    let t = TypeTransformationTest::set_up();
    // {n:number, s:string, b:boolean}
    // is transformed to object type
    t.test_ttl(t.n(OBJECT_TYPE), "maprecord(REC, (k, v) => BOT)", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithInvalidMaprecord
#[test]
fn test_transformation_with_invalid_maprecord() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(UNKNOWN_TYPE), "maprecord(REC, (k, v) => 'number')", &["The body of a maprecord function must evaluate to a record type or a no type, found number"]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithMaprecordFilterWithOnlyString
#[test]
fn test_transformation_with_maprecord_filter_with_only_string() {
    let t = TypeTransformationTest::set_up();
    // {n:number, s:string, b:boolean}
    // is transformed to
    // {s:string}
    t.test_ttl(
        t.record(&[("s", t.n(STRING_TYPE))]),
        "maprecord(REC, (k, v) => cond(eq(v, S), record({[k]:v}), BOT))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithInvalidMaprecordFirstParam
#[test]
fn test_transformation_with_invalid_maprecord_first_param() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "maprecord(N, (k, v) => BOT)",
        &["The first parameter of a maprecord must be a record type, found number"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithObjectInMaprecord
#[test]
fn test_transformation_with_object_in_maprecord() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(OBJECT_TYPE),
        "maprecord(OBJ, (k, v) => record({[k]:v}))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithUnionInMaprecord
#[test]
fn test_transformation_with_union_in_maprecord() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "maprecord(union(record({n:N}), S), (k, v) => record({[k]:v}))",
        &["The first parameter of a maprecord must be a record type, found (string|{n: number})"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithUnionOfRecordsInMaprecord
#[test]
fn test_transformation_with_union_of_records_in_maprecord() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(UNKNOWN_TYPE), "maprecord(union(record({n:N}), record({s:S})),  (k, v) => record({[k]:v}))", &["The first parameter of a maprecord must be a record type, found ({n: number}|{s: string})"]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedRecordInMaprecordFilterOneLevelString
#[test]
fn test_transformation_with_nested_record_in_maprecord_filter_one_level_string() {
    let t = TypeTransformationTest::set_up();
    // {s:string, r:{s:string, b:boolean}}
    // is transformed to
    // {r:{s:string, b:boolean}}
    t.test_ttl(
        t.record(&[(
            "r",
            t.record(&[("s", t.n(STRING_TYPE)), ("b", t.n(BOOLEAN_TYPE))]),
        )]),
        "maprecord(NESTEDREC, (k, v) => cond(eq(v, S), BOT, record({[k]:v})))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedRecordInMaprecordFilterTwoLevelsString
#[test]
fn test_transformation_with_nested_record_in_maprecord_filter_two_levels_string() {
    let t = TypeTransformationTest::set_up();
    // {s:string, r:{s:string, b:boolean}}
    // is transformed to
    // {r:{b:boolean}}
    t.test_ttl(t.record(&[("r", t.record(&[("b", t.n(BOOLEAN_TYPE))]))]), "maprecord(NESTEDREC,\n    (k1, v1) =>\n        cond(sub(v1, 'Object'),\n            maprecord(v1, (k2, v2) =>\n                cond(eq(v2, S), BOT, record({[k1]:record({[k2]:v2})}))),\n            cond(eq(v1, S), BOT, record({[k1]:v1}))))\n", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedIdentityOneLevel
#[test]
fn test_transformation_with_nested_identity_one_level() {
    let t = TypeTransformationTest::set_up();
    // {r:{n:number, s:string}}
    t.test_ttl(t.record(&[("r", t.record(&[("b", t.n(BOOLEAN_TYPE)), ("s", t.n(STRING_TYPE))]))]), "maprecord(record({r:record({b:B, s:S})}),\n    (k1, v1) =>\n        maprecord(v1,\n            (k2, v2) =>\n                record({[k1]:record({[k2]:v2})})))\n", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedIdentityOneLevel2
#[test]
fn test_transformation_with_nested_identity_one_level2() {
    let t = TypeTransformationTest::set_up();
    // {r:{n:number, s:string}}
    t.test_ttl(t.record(&[("r", t.record(&[("b", t.n(BOOLEAN_TYPE)), ("s", t.n(STRING_TYPE))]))]), "maprecord(record({r:record({b:B, s:S})}),\n    (k1, v1) =>\n        record({[k1]:\n            maprecord(v1,\n                (k2, v2) => record({[k2]:v2}))}))\n", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedIdentityTwoLevels
#[test]
fn test_transformation_with_nested_identity_two_levels() {
    let t = TypeTransformationTest::set_up();
    // {r:{r2:{n:number, s:string}}}
    t.test_ttl(t.record(&[("r", t.record(&[("r2", t.record(&[("n", t.n(NUMBER_TYPE)), ("s", t.n(STRING_TYPE))]))]))]), "maprecord(record({r:record({r2:record({n:N, s:S})})}),\n    (k1, v1) =>\n        maprecord(v1,\n            (k2, v2) =>\n                maprecord(v2,\n                    (k3, v3) =>\n                        record({[k1]:\n                            record({[k2]:\n                                record({[k3]:v3})})}))))\n", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedIdentityTwoLevels2
#[test]
fn test_transformation_with_nested_identity_two_levels2() {
    let t = TypeTransformationTest::set_up();
    // {r:{r2:{n:number, s:string}}}
    t.test_ttl(t.record(&[("r", t.record(&[("r2", t.record(&[("n", t.n(NUMBER_TYPE)), ("s", t.n(STRING_TYPE))]))]))]), "maprecord(record({r:record({r2:record({n:N, s:S})})}),\n    (k1, v1) =>\n        record({[k1]:\n            maprecord(v1,\n                (k2, v2) =>\n                    record({[k2]:\n                        maprecord(v2,\n                            (k3, v3) =>\n                                record({[k3]:v3}))}))}))\n", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedIdentityThreeLevels
#[test]
fn test_transformation_with_nested_identity_three_levels() {
    let t = TypeTransformationTest::set_up();
    // {r:{r2:{r3:{n:number, s:string}}}}
    t.test_ttl(t.record(&[("r", t.record(&[("r2", t.record(&[("r3", t.record(&[("n", t.n(NUMBER_TYPE)), ("s", t.n(STRING_TYPE))]))]))]))]), "maprecord(record({r:record({r2:record({r3:record({n:N, s:S})})})}),\n    (k1, v1) =>\n        maprecord(v1,\n            (k2, v2) =>\n                maprecord(v2,\n                    (k3, v3) =>\n                        maprecord(v3,\n                            (k4, v4) =>\n                                record({[k1]:\n                                    record({[k2]:\n                                        record({[k3]:\n                                            record({[k4]:v4})})})})))))\n", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedIdentityThreeLevels2
#[test]
fn test_transformation_with_nested_identity_three_levels2() {
    let t = TypeTransformationTest::set_up();
    // {r:{r2:{r3:{n:number, s:string}}}}
    t.test_ttl(t.record(&[("r", t.record(&[("r2", t.record(&[("r3", t.record(&[("n", t.n(NUMBER_TYPE)), ("s", t.n(STRING_TYPE))]))]))]))]), "maprecord(record({r:record({r2:record({r3:record({n:N, s:S})})})}),\n    (k1, v1) =>\n        record({[k1]:\n            maprecord(v1,\n                (k2, v2) =>\n                    record({[k2]:\n                        maprecord(v2,\n                            (k3, v3) =>\n                                record({[k3]:\n                                    maprecord(v3,\n                                        (k4, v4) =>\n                                            record({[k4]:v4}))}))}))}))\n", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedRecordDeleteLevelTwoAndThree
#[test]
fn test_transformation_with_nested_record_delete_level_two_and_three() {
    let t = TypeTransformationTest::set_up();
    // {r:{r2:{r3:{n:number, s:string}}}}
    // is transformed into
    // {r:{n:number, s:string}}
    t.test_ttl(t.record(&[("r", t.record(&[("n", t.n(NUMBER_TYPE)), ("s", t.n(STRING_TYPE))]))]), "maprecord(record({r:record({r2:record({r3:record({n:N, s:S})})})}),\n    (k1, v1) =>\n        maprecord(v1,\n            (k2, v2) =>\n                maprecord(v2,\n                    (k3, v3) =>\n                        maprecord(v3,\n                            (k4, v4) =>\n                                record({[k1]:\n                                    record({[k4]:v4})})))))\n", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedRecordDeleteLevelTwoAndThree2
#[test]
fn test_transformation_with_nested_record_delete_level_two_and_three2() {
    let t = TypeTransformationTest::set_up();
    // {r:{r2:{r3:{n:number, s:string}}}}
    // is transformed into
    // {r:{n:number, s:string}}
    t.test_ttl(t.record(&[("r", t.record(&[("n", t.n(NUMBER_TYPE)), ("s", t.n(STRING_TYPE))]))]), "maprecord(record({r:record({r2:record({r3:record({n:N, s:S})})})}),\n    (k1, v1) =>\n        record({[k1]:\n            maprecord(v1,\n                (k2, v2) =>\n                    maprecord(v2,\n                        (k3, v3) =>\n                            maprecord(v3,\n                                (k4, v4) =>\n                                    record({[k4]:v4}))))}))\n", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedRecordCollapsePropertiesToRecord
#[test]
fn test_transformation_with_nested_record_collapse_properties_to_record() {
    let t = TypeTransformationTest::set_up();
    // {a:Array, b:{n:number}}
    // is transformed to
    // {foo:{n:number}}
    t.test_ttl(
        t.record(&[("foo", t.record(&[("n", t.n(NUMBER_TYPE))]))]),
        "maprecord(record({a:ARR, b:record({n:N})}), (k, v) => record({foo:v}))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedRecordCollapsePropertiesToType
#[test]
fn test_transformation_with_nested_record_collapse_properties_to_type() {
    let t = TypeTransformationTest::set_up();
    // {a:{n:number}, b:Array}
    // is transformed to
    // {foo:Array}
    t.test_ttl(
        t.record(&[("foo", t.n(ARRAY_TYPE))]),
        "maprecord(record({a:record({n:N}), b:ARR}), (k, v) => record({foo:v}))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedRecordCollapsePropertiesJoinRecords
#[test]
fn test_transformation_with_nested_record_collapse_properties_join_records() {
    let t = TypeTransformationTest::set_up();
    // {a:{n:number}, b:{s:Array}}
    // is transformed to
    // {foo:{n:number, s:Array}}
    t.test_ttl(
        t.record(&[(
            "foo",
            t.record(&[("n", t.n(NUMBER_TYPE)), ("s", t.n(ARRAY_TYPE))]),
        )]),
        "maprecord(record({a:record({n:N}), b:record({s:ARR})}), (k, v) => record({foo:v}))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithNestedRecordCollapsePropertiesJoinRecords2
#[test]
fn test_transformation_with_nested_record_collapse_properties_join_records2() {
    let t = TypeTransformationTest::set_up();
    // {a:{n:number, {x:number}}, b:{s:Array, {y:number}}}
    // is transformed to
    // {foo:{n:number, s:Array, r:{x:number, y:number}}}
    t.test_ttl(t.record(&[("foo", t.record(&[("n", t.n(NUMBER_TYPE)), ("s", t.n(ARRAY_TYPE)), ("r", t.record(&[("x", t.n(NUMBER_TYPE)), ("y", t.n(NUMBER_TYPE))]))]))]), "maprecord(\n    record({a:record({n:N, r:record({x:N})}), b:record({s:ARR, r:record({y:N})})}),\n    (k, v) => record({foo:v}))\n", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithAsynchUseCase
#[test]
fn test_transformation_with_asynch_use_case() {
    let t = TypeTransformationTest::set_up();
    // TODO(lpino): Use the type Promise instead of Array
    // {service:Array<number>}
    // is transformed to
    // {service:number}
    t.test_ttl(t.record(&[("service", t.n(NUMBER_TYPE))]), "cond(\n    sub(ASYNCH, 'Object'),\n    maprecord(\n        ASYNCH,\n        (k, v) =>\n            cond(\n                eq(rawTypeOf(v), 'Array'),\n                record({[k]:templateTypeOf(v, 0)}),\n                record({[k]:'undefined'}))),\n    ASYNCH)\n", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithInvalidNestedMaprecord
#[test]
fn test_transformation_with_invalid_nested_maprecord() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "maprecord(NESTEDREC, (k, v) => maprecord(v, (k, v) => BOT))",
        &["The variable k is already defined"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithMaprecordAndStringEquivalence
#[test]
fn test_transformation_with_maprecord_and_string_equivalence() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.record(&[("bool", t.n(NUMBER_TYPE)), ("str", t.n(STRING_TYPE))]), "maprecord(record({bool:B, str:S}),\n(k, v) => record({[k]:cond(streq(k, 'bool'), N, v)}))\n", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTypeOfVar
#[test]
fn test_transformation_with_type_of_var() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(NUMBER_TYPE), "typeOfVar('n')", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithUnknownTypeOfVar
#[test]
fn test_transformation_with_unknown_type_of_var() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "typeOfVar('foo')",
        &["Variable foo is undefined in the scope"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueIsConstructorConditional
#[test]
fn test_transformation_with_true_is_constructor_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond(isCtor(typeOfVar('Bar')), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithFalseIsConstructorConditional
#[test]
fn test_transformation_with_false_is_constructor_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(NUMBER_TYPE), "cond(isCtor(N), 'string', 'number')", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueIsTemplatizedConditional
#[test]
fn test_transformation_with_true_is_templatized_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond(isTemplatized(type(ARR, N)), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithFalseIsTemplatizedConditional
#[test]
fn test_transformation_with_false_is_templatized_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "cond(isTemplatized(ARR), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueIsRecordConditional
#[test]
fn test_transformation_with_true_is_record_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond(isRecord(REC), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithFalseIsRecordConditional
#[test]
fn test_transformation_with_false_is_record_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "cond(isRecord(N), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueIsDefinedConditional
#[test]
fn test_transformation_with_true_is_defined_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond(isDefined(N), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithFalseIsDefinedConditional
#[test]
fn test_transformation_with_false_is_defined_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "cond(isDefined(Foo), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueIsUnknownConditional
#[test]
fn test_transformation_with_true_is_unknown_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond(isUnknown(UNK), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueIsUnknownConditional2
#[test]
fn test_transformation_with_true_is_unknown_conditional2() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond(isUnknown(CHKUNK), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithFalseIsUnknownConditional
#[test]
fn test_transformation_with_false_is_unknown_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "cond(isUnknown(N), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueAndConditional
#[test]
fn test_transformation_with_true_and_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond(isDefined(N) && isDefined(N), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithFalseAndConditional
#[test]
fn test_transformation_with_false_and_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "cond(isDefined(N) && isDefined(Foo), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithFalseAndConditional2
#[test]
fn test_transformation_with_false_and_conditional2() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "cond(isDefined(Foo) && isDefined(N), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithFalseAndConditional3
#[test]
fn test_transformation_with_false_and_conditional3() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "cond(isDefined(Foo) && isDefined(Foo), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueOrConditional
#[test]
fn test_transformation_with_true_or_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond(isDefined(N) || isDefined(N), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueOrConditional2
#[test]
fn test_transformation_with_true_or_conditional2() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond(isDefined(Foo) || isDefined(N), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueOrConditional3
#[test]
fn test_transformation_with_true_or_conditional3() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond(isDefined(N) || isDefined(Foo), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithFalseOrConditional
#[test]
fn test_transformation_with_false_or_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "cond(isDefined(Foo) || isDefined(Foo), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTrueNotConditional3
#[test]
fn test_transformation_with_true_not_conditional3() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(STRING_TYPE),
        "cond(!isDefined(Foo), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithFalseNotConditional
#[test]
fn test_transformation_with_false_not_conditional() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "cond(!isDefined(N), 'string', 'number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithInstanceOf
#[test]
fn test_transformation_with_instance_of() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_OBJECT_TYPE),
        "instanceOf(typeOfVar('Number'))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithInvalidInstanceOf
#[test]
fn test_transformation_with_invalid_instance_of() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "instanceOf(N)",
        &["Expected a constructor type, found number"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithInvalidInstanceOf2
#[test]
fn test_transformation_with_invalid_instance_of2() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "instanceOf(foo)",
        &[
            "Expected a constructor type, found Unknown",
            "Reference to an unknown type variable foo",
        ],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationWithTypeExpr
#[test]
fn test_transformation_with_type_expr() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(NUMBER_TYPE), "typeExpr('number')", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testParserWithTTLNativeTypeExprUnion
#[test]
fn test_parser_with_ttlnative_type_expr_union() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.union(&[t.n(NUMBER_TYPE), t.n(BOOLEAN_TYPE)]),
        "typeExpr('number|boolean')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testParserWithTTLNativeTypeExprRecord
#[test]
fn test_parser_with_ttlnative_type_expr_record() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.record(&[("foo", t.n(NUMBER_TYPE)), ("bar", t.n(BOOLEAN_TYPE))]),
        "typeExpr('{foo:number, bar:boolean}')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testParserWithTTLNativeTypeExprNullable
#[test]
fn test_parser_with_ttlnative_type_expr_nullable() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.union(&[t.n(NUMBER_TYPE), t.n(NULL_TYPE)]),
        "typeExpr('?number')",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testParserWithTTLNativeTypeExprNonNullable
#[test]
fn test_parser_with_ttlnative_type_expr_non_nullable() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(NUMBER_TYPE), "typeExpr('!number')", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationPrintType
#[test]
fn test_transformation_print_type() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(NUMBER_TYPE), "printType('Test message: ', N)", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationPrintType2
#[test]
fn test_transformation_print_type2() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.record_type_test.unwrap(),
        "printType('Test message: ', REC)",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationPropType
#[test]
fn test_transformation_prop_type() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(NUMBER_TYPE),
        "propType('a', record({a:N, b:record({x:B})}))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationPropType2
#[test]
fn test_transformation_prop_type2() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.record(&[("x", t.n(BOOLEAN_TYPE))]),
        "propType('b', record({a:N, b:record({x:B})}))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationPropTypeNotFound
#[test]
fn test_transformation_prop_type_not_found() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "propType('c', record({a:N, b:record({x:B})}))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationPropTypeInvalid
#[test]
fn test_transformation_prop_type_invalid() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "propType('c', N)",
        &["Expected object type, found number"],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationInstanceObjectToRecord
#[test]
fn test_transformation_instance_object_to_record() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(t.n(OBJECT_TYPE), "record(type(OBJ, N))", &[]);
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationInstanceObjectToRecord2
#[test]
fn test_transformation_instance_object_to_record2() {
    let t = TypeTransformationTest::set_up();
    // TODO(bradfordcsmith): Define Array.prototype.length using externs instead.
    t.define_declared_property(t.n(ARRAY_TYPE), "length", t.n(NUMBER_TYPE));
    t.test_ttl(
        t.record(&[("length", t.n(NUMBER_TYPE))]),
        "record(type(ARR, N))",
        &[],
    );
    t.validate_warnings_and_errors();
}

// port: TypeTransformationTest#testTransformationInstanceObjectToRecordInvalid
#[test]
fn test_transformation_instance_object_to_record_invalid() {
    let t = TypeTransformationTest::set_up();
    t.test_ttl(
        t.n(UNKNOWN_TYPE),
        "record(union(OBJ, NULL))",
        &["Expected a record type, found (Object|null)"],
    );
    t.validate_warnings_and_errors();
}
