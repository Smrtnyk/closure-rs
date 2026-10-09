/*
 * Copyright 2011 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/jscomp/parsing/parser/Parser.java.

#![allow(clippy::upper_case_acronyms)] // Retain Java enum constant names.
//! Recursive JavaScript parser. Callers parsing deep input should use a large-stack thread.
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, LazyLock},
};

use closure_rhino::java_lang::trim;

use super::{
    JsString,
    class_or_object_element_info::ClassOrObjectElementInfo,
    feature_set::{Feature, FeatureSet},
    identifier_token::IdentifierToken,
    keywords::Keywords,
    predefined_name::PredefinedName,
    scanner::{self, Scanner},
    source_file::SourceFile,
    template_literal_token::ErrorLevel,
    token::{Token, TokenData},
    token_type::TokenType,
    trees::*,
    trees::{
        comment,
        comprehension_tree::ComprehensionType,
        function_declaration_tree::{self, Kind},
    },
    util::{Reporter, SourcePosition, SourceRange},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub class: &'static str,
    pub message: Option<JsString>,
}
type P<T> = Result<T, ParseError>;
fn required<T>(value: Option<T>) -> P<T> {
    value.ok_or(ParseError {
        class: "java.lang.NullPointerException",
        message: None,
    })
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    ES3,
    ES5,
    ES6_OR_ES7,
    ES8_OR_GREATER,
}
#[derive(Clone, Copy, Debug)]
pub struct Config {
    at_least6: bool,
    at_least8: bool,
    is_strict_mode: bool,
    warn_trailing_commas: bool,
}
impl Default for Config {
    // port: Parser.Config#<init>
    fn default() -> Self {
        Self::new(Mode::ES8_OR_GREATER, /* isStrictMode= */ false)
    }
}
impl Config {
    // port: Parser.Config#<init>
    pub fn new(mode: Mode, is_strict_mode: bool) -> Self {
        Self {
            at_least6: !(mode == Mode::ES3 || mode == Mode::ES5),
            at_least8: mode == Mode::ES8_OR_GREATER,
            is_strict_mode,
            // Generally, we allow everything that is valid in any mode
            // we only warn about things that are not represented in the AST.
            warn_trailing_commas: mode == Mode::ES3,
        }
    }
}
// Indicates the type of function currently being parsed.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum FunctionFlavor {
    NORMAL,
    GENERATOR,
    ASYNCHRONOUS,
    ASYNCHRONOUS_GENERATOR,
}
struct FunctionFlavorData {
    is_generator: bool,
}
impl FunctionFlavorData {
    // port: Parser.FunctionFlavor#<init>
    const fn new(is_generator: bool) -> Self {
        Self { is_generator }
    }
}
impl FunctionFlavor {
    fn is_generator(self) -> bool {
        match self {
            Self::NORMAL => FunctionFlavorData::new(false),
            Self::GENERATOR => FunctionFlavorData::new(true),
            Self::ASYNCHRONOUS => FunctionFlavorData::new(false),
            Self::ASYNCHRONOUS_GENERATOR => FunctionFlavorData::new(true),
        }
        .is_generator
    }
}
// Differentiates between parsing for 'In' vs. 'NoIn' Variants of expression grammars.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Expression {
    NORMAL,
    NO_IN,
}
// Destructuring (aka pattern matching); see
// http://wiki.ecmascript.org/doku.php?id=harmony:destructuring

// Kinds of destructuring patterns
#[derive(Clone, Copy, PartialEq, Eq)]
enum PatternKind {
    // A var, let, const; catch head; or formal parameter list--only
    // identifiers are allowed as lvalues
    INITIALIZER,
    // An assignment or for-in initializer--any lvalue is allowed
    ANY,
}

static SOURCE_MAPPING_URL_PREFIX: LazyLock<JsString> =
    LazyLock::new(|| JsString::from("//# sourceMappingURL="));
static ASYNC: LazyLock<JsString> = LazyLock::new(|| JsString::from("async"));
static AWAIT: LazyLock<JsString> = LazyLock::new(|| JsString::from("await"));

#[derive(Default)]
struct CommentRecorder {
    comments: Vec<Comment>,
    last_comment_end_position: Option<SourcePosition>,
    source_map_url: Option<JsString>,
}
impl scanner::CommentRecorder for CommentRecorder {
    // port: Parser.CommentRecorder#recordComment
    fn record_comment(&mut self, type_: comment::Type, range: SourceRange, value: JsString) {
        // If we rewind the token stream, the scanner might pass comments that we've already seen.
        // Only record comments past the furthest comment end position we've seen.
        // NB: this assumes the CommentRecorder is used for at most one source file.
        if self
            .last_comment_end_position
            .as_ref()
            .is_none_or(|end| range.end.offset > end.offset)
        {
            let value = trim(&value);
            if value.starts_with(&*SOURCE_MAPPING_URL_PREFIX) {
                self.source_map_url =
                    Some(value.substring_from(SOURCE_MAPPING_URL_PREFIX.length()));
            }
            self.last_comment_end_position = Some(range.end.clone());
            self.comments.push(Comment::new(value, range, type_));
        }
    }
}
impl CommentRecorder {
    // port: Parser.CommentRecorder#getComments
    fn get_comments(&self) -> Vec<Comment> {
        self.comments.clone()
    }
}

// Parses a javascript file.
//
// <p>The various parseX() methods never return null - even when parse errors are encountered.
// Typically parseX() will return a XTree ParseTree. Each ParseTree that is created includes its
// source location. The typical pattern for a parseX() method is:
//
// <pre>
// XTree parseX() {
//   SourcePosition start = getTreeStartLocation();
//   parse X grammar element and its children
//   return new XTree(getTreeLocation(start), children);
// }
// </pre>
//
// <p>parseX() methods must consume at least 1 token - even in error cases. This prevents infinite
// loops in the parser.
//
// <p>Many parseX() methods are matched by a 'boolean peekX()' method which will return true if the
// beginning of an X appears at the current location. There are also peek() methods which examine
// the next token. peek() methods must not consume any tokens.
//
// <p>The eat() method consumes a token and reports an error if the consumed token is not of the
// expected type. The eatOpt() methods consume the next token iff the next token is of the expected
// type and return the consumed token or null if no token was consumed.
//
// <p>When parse errors are encountered, an error should be reported and the parse should return a
// best guess at the current parse tree.
//
// <p>When parsing lists, the preferred pattern is:
//
// <pre>
//   eat(LIST_START);
//   ImmutableList.Builder&lt;ParseTree&gt; elements = ImmutableList.builder();
//   while (peekListElement()) {
//     elements.add(parseListElement());
//   }
//   eat(LIST_END);
// </pre>
pub struct Parser {
    scanner: Scanner,
    error_reporter: Reporter,
    config: Config,
    comment_recorder: Rc<RefCell<CommentRecorder>>,
    function_context_stack: Vec<FunctionFlavor>,
    features: FeatureSet,
    last_source_position: SourcePosition,
}
impl Parser {
    // port: Parser#<init>
    pub fn new(config: Config, error_reporter: Reporter, source: Arc<SourceFile>) -> Self {
        let comment_recorder = Rc::new(RefCell::new(CommentRecorder::default()));
        let mut scanner = Scanner::new(error_reporter.clone(), comment_recorder.clone(), source, 0);
        let last_source_position = scanner.get_position();
        Self {
            config,
            error_reporter,
            scanner,
            comment_recorder,
            function_context_stack: vec![FunctionFlavor::NORMAL],
            features: FeatureSet::BARE_MINIMUM,
            last_source_position,
        }
    }
    // port: Parser#getComments
    pub fn get_comments(&self) -> Vec<Comment> {
        self.comment_recorder.borrow().get_comments()
    }
    // port: Parser#getFeatures
    pub fn get_features(&self) -> FeatureSet {
        self.features
    }
    // Returns the url provided by the sourceMappingURL if any was found.
    // port: Parser#getSourceMapURL
    pub fn get_source_map_url(&self) -> Option<JsString> {
        self.comment_recorder.borrow().source_map_url.clone()
    }
    // 14 Program
    // port: Parser#parseProgram
    pub fn parse_program(&mut self) -> P<Option<Tree>> {
        let result: P<Option<Tree>> = (|| {
            // Set the start location at the beginning of the file rather than the beginning of the first
            // token.  This ensures that it accounts for leading comments.
            let start = self.last_source_position.clone();
            let source_elements = self.parse_global_source_elements()?;
            self.eat(TokenType::END_OF_FILE);
            Ok(Some(ProgramTree::new(
                self.get_tree_location(start),
                source_elements,
                self.get_comments(),
            )))
        })();
        match result {
            // We are checking the error message instead of catching StackOverflowError since
            // StackOverflowError is not emulated on the Web.
            // Error outcomes use Result here; native callers must provide a large-stack thread.
            Err(error)
                if error.class.ends_with("Error")
                    && (error.class.contains("java.lang.StackOverflowError")
                        || error.message.as_ref().is_some_and(|message| {
                            let stack_overflow = JsString::from("java.lang.StackOverflowError");
                            message.index_of(&stack_overflow) >= 0
                        })) =>
            {
                self.report_error("Too deep recursion while parsing", &[]);
                Ok(None)
            }
            other => other,
        }
    }
    // port: Parser#parseGlobalSourceElements
    fn parse_global_source_elements(&mut self) -> P<Vec<Tree>> {
        let mut result = Vec::new();
        while !self.peek(TokenType::END_OF_FILE) {
            result.push(self.parse_script_element()?);
        }
        Ok(result)
    }
    // ImportDeclaration
    // ExportDeclaration
    // SourceElement
    // port: Parser#parseScriptElement
    fn parse_script_element(&mut self) -> P<Tree> {
        if self.peek_import_declaration() {
            return self.parse_import_declaration();
        }
        if self.peek_export_declaration() {
            return self.parse_export_declaration();
        }
        self.parse_source_element()
    }
    // port: Parser#peekImportDeclaration
    fn peek_import_declaration(&mut self) -> bool {
        self.peek(TokenType::IMPORT)
            && (self.peek_id_or_keyword_at(1)
                || self.peek_at(1, TokenType::STRING)
                || self.peek_at(1, TokenType::OPEN_CURLY)
                || self.peek_at(1, TokenType::STAR))
    }
    // port: Parser#parseImportDeclaration
    fn parse_import_declaration(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::IMPORT);
        // import ModuleSpecifier ;
        // import ModuleSpecifier ;
        if self.peek(TokenType::STRING) {
            let module_specifier = required(self.eat(TokenType::STRING))?;
            self.eat_possibly_implicit_semi_colon();
            return Ok(ImportDeclarationTree::new(
                self.get_tree_location(start),
                None,
                None,
                None,
                module_specifier,
            ));
        }
        // import ImportedDefaultBinding from ModuleSpecifier
        // import NameSpaceImport from ModuleSpecifier
        // import NamedImports from ModuleSpecifier ;
        // import ImportedDefaultBinding , NameSpaceImport from ModuleSpecifier ;
        // import ImportedDefaultBinding , NamedImports from ModuleSpecifier ;
        let mut default_binding_identifier = None;
        let mut name_space_import_identifier = None;
        let mut identifier_set = None;
        let mut parse_explicit_names = true;
        if self.peek_id() {
            default_binding_identifier = self.eat_id();
            if self.peek(TokenType::COMMA) {
                self.eat(TokenType::COMMA);
            } else {
                parse_explicit_names = false;
            }
        } else if Keywords::is_keyword_type(self.peek_type()) {
            let keyword = self.next_token();
            self.report_error_token(
                Some(&keyword),
                "cannot use keyword '%s' here.",
                &[keyword.to_string_utf16()],
            );
        }
        if parse_explicit_names {
            if self.peek(TokenType::STAR) {
                self.eat(TokenType::STAR);
                self.eat_predefined_string(PredefinedName::AS);
                name_space_import_identifier = self.eat_id();
            } else {
                identifier_set = Some(self.parse_import_specifier_set()?);
            }
        }
        self.eat_predefined_string(PredefinedName::FROM);
        let module_specifier = self.eat(TokenType::STRING);
        self.eat_possibly_implicit_semi_colon();
        Ok(ImportDeclarationTree::new(
            self.get_tree_location(start),
            default_binding_identifier,
            identifier_set,
            name_space_import_identifier,
            module_specifier,
        ))
    }
    //  ImportSpecifierSet ::= '{' (ImportSpecifier (',' ImportSpecifier)* (,)? )?  '}'
    // port: Parser#parseImportSpecifierSet
    fn parse_import_specifier_set(&mut self) -> P<Vec<Tree>> {
        let mut elements = Vec::new();
        self.eat(TokenType::OPEN_CURLY);
        while self.peek_id_or_keyword() {
            elements.push(self.parse_import_specifier()?);
            if !self.peek(TokenType::CLOSE_CURLY) {
                self.eat(TokenType::COMMA);
            }
        }
        self.eat(TokenType::CLOSE_CURLY);
        Ok(elements)
    }
    // ImportSpecifier ::= Identifier ('as' Identifier)?
    // port: Parser#parseImportSpecifier
    fn parse_import_specifier(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let imported_name = required(self.eat_id_or_keyword_as_id())?;
        let mut destination_name = None;
        if self.peek_predefined_string(PredefinedName::AS) {
            self.eat_predefined_string(PredefinedName::AS);
            destination_name = self.eat_id();
        } else if imported_name.is_keyword() {
            self.report_expected_error(None, (**PredefinedName::AS).clone());
        }
        Ok(ImportSpecifierTree::new(
            self.get_tree_location(start),
            imported_name,
            destination_name,
        ))
    }
    // export  VariableStatement
    // export  FunctionDeclaration
    // export  ConstStatement
    // export  ClassDeclaration
    // export  default expression
    // etc
    // port: Parser#peekExportDeclaration
    fn peek_export_declaration(&mut self) -> bool {
        self.peek(TokenType::EXPORT)
    }
    // ExportDeclaration :
    //   export * FromClause ;
    //   export ExportClause [NoReference] FromClause ;
    //   export ExportClause ;
    //   export VariableStatement
    //   export Declaration[Default]
    //   export default AssignmentExpression ;
    // ExportClause [NoReference] :
    //   { }
    //   { ExportsList [?NoReference] }
    //   { ExportsList [?NoReference] , }
    // ExportsList [NoReference] :
    //   ExportSpecifier [?NoReference]
    //   ExportsList [?NoReference] , ExportSpecifier [?NoReference]
    // ExportSpecifier [NoReference] :
    //   [~NoReference] IdentifierReference
    //   [~NoReference] IdentifierReference as IdentifierName
    //   [+NoReference] IdentifierName
    //   [+NoReference] IdentifierName as IdentifierName
    // port: Parser#parseExportDeclaration
    fn parse_export_declaration(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut is_default = false;
        let mut is_export_all = false;
        let mut is_export_specifier = false;
        let mut needs_semi_colon = true;
        self.eat(TokenType::EXPORT);
        let mut export = None;
        let mut export_specifier_list = None;
        match self.peek_type() {
            TokenType::STAR => {
                is_export_all = true;
                self.next_token();
            }
            TokenType::IDENTIFIER => {
                export = Some(self.parse_async_function_declaration()?);
            }
            TokenType::FUNCTION => {
                export = Some(self.parse_function_declaration()?);
                needs_semi_colon = false;
            }
            TokenType::CLASS => {
                export = Some(self.parse_class_declaration()?);
                needs_semi_colon = false;
            }
            TokenType::DEFAULT => {
                is_default = true;
                self.next_token();
                export = Some(self.parse_expression()?);
                needs_semi_colon = false;
            }
            TokenType::OPEN_CURLY => {
                is_export_specifier = true;
                export_specifier_list = Some(self.parse_export_specifier_set()?);
            }
            _ => {
                // unreachable, parse as a var decl to get a parse error.
                export = self.parse_variable_declaration_list()?;
            }
        }
        let mut module_specifier = None;
        if is_export_all
            || (is_export_specifier && self.peek_predefined_string(PredefinedName::FROM))
        {
            self.eat_predefined_string(PredefinedName::FROM);
            module_specifier = self.eat(TokenType::STRING);
        } else if is_export_specifier {
            for tree in export_specifier_list.as_ref().unwrap() {
                let imported_name = &tree.as_export_specifier().imported_name;
                if imported_name.is_keyword() {
                    self.report_error_token(
                        Some(imported_name),
                        "cannot use keyword '%s' here.",
                        &[imported_name.to_string_utf16()],
                    );
                }
            }
        }
        if needs_semi_colon || self.peek_implicit_semi_colon() {
            self.eat_possibly_implicit_semi_colon();
        }
        Ok(ExportDeclarationTree::new(
            self.get_tree_location(start),
            is_default,
            is_export_all,
            export,
            export_specifier_list,
            module_specifier,
        ))
    }
    //  ExportSpecifierSet ::= '{' (ExportSpecifier (',' ExportSpecifier)* (,)? )?  '}'
    // port: Parser#parseExportSpecifierSet
    fn parse_export_specifier_set(&mut self) -> P<Vec<Tree>> {
        let mut elements = Vec::new();
        self.eat(TokenType::OPEN_CURLY);
        while self.peek_id_or_keyword() {
            elements.push(self.parse_export_specifier()?);
            if !self.peek(TokenType::CLOSE_CURLY) {
                self.eat(TokenType::COMMA);
            }
        }
        self.eat(TokenType::CLOSE_CURLY);
        Ok(elements)
    }
    // ExportSpecifier ::= Identifier ('as' Identifier)?
    // port: Parser#parseExportSpecifier
    fn parse_export_specifier(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let imported_name = required(self.eat_id_or_keyword_as_id())?;
        let mut destination_name = None;
        if self.peek_predefined_string(PredefinedName::AS) {
            self.eat_predefined_string(PredefinedName::AS);
            destination_name = self.eat_id_or_keyword_as_id();
        }
        Ok(ExportSpecifierTree::new(
            self.get_tree_location(start),
            imported_name,
            destination_name,
        ))
    }
    // port: Parser#peekClassDeclaration
    fn peek_class_declaration(&mut self) -> bool {
        self.peek(TokenType::CLASS)
    }
    // port: Parser#parseClassDeclaration
    fn parse_class_declaration(&mut self) -> P<Tree> {
        self.parse_class(/* isExpression= */ false)
    }
    // port: Parser#parseClassExpression
    fn parse_class_expression(&mut self) -> P<Tree> {
        self.parse_class(/* isExpression= */ true)
    }
    // port: Parser#parseClass
    fn parse_class(&mut self, is_expression: bool) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::CLASS);
        let mut name = None;
        if !is_expression || self.peek_id() {
            name = self.eat_id();
        }
        let mut super_class = None;
        if self.peek(TokenType::EXTENDS) {
            self.eat(TokenType::EXTENDS);
            super_class = Some(self.parse_left_hand_side_expression()?);
        }
        self.eat(TokenType::OPEN_CURLY);
        let elements = self.parse_class_elements()?;
        self.eat(TokenType::CLOSE_CURLY);
        Ok(ClassDeclarationTree::new(
            self.get_tree_location(start),
            name,
            super_class,
            elements,
        ))
    }
    // port: Parser#parseClassElements
    fn parse_class_elements(&mut self) -> P<Vec<Tree>> {
        let mut result = Vec::new();
        loop {
            let token = self.peek_token();
            if token.type_ == TokenType::SEMI_COLON {
                self.eat(TokenType::SEMI_COLON);
                continue;
            } else if !self.is_class_element_start(&token) {
                return Ok(result);
            }
            result.push(self.parse_class_element()?);
        }
    }
    // port: Parser#isClassElementStart
    fn is_class_element_start(&self, token: &Token) -> bool {
        match token.type_ {
            TokenType::IDENTIFIER
            | TokenType::NUMBER
            | TokenType::BIGINT
            | TokenType::STAR
            | TokenType::STATIC
            | TokenType::STRING
            | TokenType::OPEN_SQUARE => true,
            _ => Keywords::is_keyword_type(token.type_),
        }
    }
    // port: Parser#createObjectLiteralElementInfo
    fn create_object_literal_element_info(&mut self) -> ClassOrObjectElementInfo {
        ClassOrObjectElementInfo::create_object_literal_element_info(self.get_tree_start_location())
    }
    // port: Parser#parseClassElement
    fn parse_class_element(&mut self) -> P<Tree> {
        if self.peek(TokenType::SEMI_COLON) {
            return self.parse_empty_statement();
        }
        if self.peek_class_static_initializer_block() {
            return self.parse_class_static_initializer_block();
        }
        let start = self.get_tree_start_location();
        let is_static = self.eat_static_if_not_element_name();
        let element_info = ClassOrObjectElementInfo::create_class_member_info(start, is_static);
        if self.peek_get_accessor() {
            self.parse_get_accessor(element_info)
        } else if self.peek_set_accessor() {
            self.parse_set_accessor(element_info)
        } else if self.peek_async_method() {
            self.parse_async_method(element_info)
        } else {
            self.parse_class_member_declaration(element_info)
        }
    }
    // port: Parser#eatStaticIfNotElementName
    fn eat_static_if_not_element_name(&mut self) -> bool {
        // only eat `static` if it being used as a keyword and not
        // a member name.
        if self.peek(TokenType::STATIC) {
            let token = self.peek_token_at(1);
            if self.is_class_element_start(&token) {
                self.eat(TokenType::STATIC);
                return true;
            }
        }
        false
    }
    // port: Parser#peekAsyncMethod
    fn peek_async_method(&mut self) -> bool {
        self.peek_predefined_string(&ASYNC)
            && !self.peek_implicit_semi_colon_at(1)
            && (self.peek_property_name_or_computed_prop(1)
                || (self.peek_at(1, TokenType::STAR)
                    && self.peek_property_name_or_computed_prop(2)))
    }
    // port: Parser#peekClassStaticInitializerBlock
    fn peek_class_static_initializer_block(&mut self) -> bool {
        self.peek(TokenType::STATIC) && self.peek_at(1, TokenType::OPEN_CURLY)
    }
    // port: Parser#parseClassElementName
    fn parse_class_element_name(&mut self, element_info: &mut ClassOrObjectElementInfo) -> P<()> {
        if self.peek_property_name(0) {
            if self.peek_id_or_keyword() {
                element_info.set_name(required(self.eat_id_or_keyword_as_id())?);
                if element_info.get_name().is_keyword() {
                    self.record_feature_used(Feature::KEYWORDS_AS_PROPERTIES);
                }
            } else {
                // { 'str'() {} }
                // { 123() {} }
                // Treat these as if they were computed properties.
                // TODO(b/123769080): Stop making this assumption!
                element_info.set_name_expr(self.parse_literal_expression()?);
            }
        } else {
            element_info.set_name_expr(self.parse_computed_property_name()?);
        }
        Ok(())
    }
    // port: Parser#parseFieldDefinition
    fn parse_field_definition(&mut self, element_info: ClassOrObjectElementInfo) -> P<Tree> {
        let mut initializer = None;
        if self.peek(TokenType::EQUAL) {
            initializer = Some(self.parse_initializer(Expression::NORMAL)?);
        }
        self.eat_possibly_implicit_semi_colon();
        if element_info.has_name() {
            Ok(FieldDeclarationTree::new(
                self.get_tree_location(element_info.start.clone()),
                element_info.get_name(),
                element_info.is_static,
                initializer,
            ))
        } else {
            assert!(element_info.has_name_expr());
            Ok(ComputedPropertyFieldTree::new(
                self.get_tree_location(element_info.start.clone()),
                element_info.get_name_expr(),
                element_info.is_static,
                initializer,
            ))
        }
    }
    // port: Parser#parseMethodDefinition
    fn parse_method_definition(
        &mut self,
        element_info: ClassOrObjectElementInfo,
        is_generator: bool,
    ) -> P<Tree> {
        let kind = if element_info.has_name_expr() {
            Kind::EXPRESSION
        } else {
            Kind::MEMBER
        };
        let mut builder = FunctionDeclarationTree::builder(kind);
        builder
            .set_is_class_member(element_info.is_class_member)
            .set_static(element_info.is_static);
        if element_info.has_name() {
            builder.set_name(element_info.get_name());
        }
        self.parse_function_tail(
            &mut builder,
            if is_generator {
                FunctionFlavor::GENERATOR
            } else {
                FunctionFlavor::NORMAL
            },
        )?;
        let function = builder.build(self.get_tree_location(element_info.start.clone()));
        if kind == Kind::MEMBER {
            Ok(function)
        } else {
            assert!(element_info.has_name_expr());
            Ok(ComputedPropertyMethodTree::new(
                self.get_tree_location(element_info.start.clone()),
                element_info.get_name_expr(),
                function,
            ))
        }
    }
    // port: Parser#parseObjectLiteralMethodDeclaration
    fn parse_object_literal_method_declaration(&mut self) -> P<Tree> {
        let info = self.create_object_literal_element_info();
        self.parse_method_declaration(info)
    }
    // port: Parser#parseMethodDeclaration
    fn parse_method_declaration(&mut self, mut element_info: ClassOrObjectElementInfo) -> P<Tree> {
        let is_generator = self.eat_opt(TokenType::STAR).is_some();
        self.parse_class_element_name(&mut element_info)?;
        self.parse_method_definition(element_info, is_generator)
    }
    // port: Parser#parseClassMemberDeclaration
    fn parse_class_member_declaration(
        &mut self,
        mut element_info: ClassOrObjectElementInfo,
    ) -> P<Tree> {
        let is_generator = self.eat_opt(TokenType::STAR).is_some();
        self.parse_class_element_name(&mut element_info)?;
        if self.peek_type_at(0) == TokenType::OPEN_PAREN {
            self.parse_method_definition(element_info, is_generator)
        } else {
            self.parse_field_definition(element_info)
        }
    }
    // port: Parser#parseObjectLiteralAsyncMethod
    fn parse_object_literal_async_method(&mut self) -> P<Tree> {
        let info = self.create_object_literal_element_info();
        self.parse_async_method(info)
    }
    // port: Parser#parseAsyncMethod
    fn parse_async_method(&mut self, element_info: ClassOrObjectElementInfo) -> P<Tree> {
        self.eat_predefined_string(&ASYNC);
        let generator = self.peek(TokenType::STAR);
        if generator {
            self.eat(TokenType::STAR);
        }
        if self.peek_property_name(0) {
            if self.peek_id_or_keyword() {
                let name = required(self.eat_id_or_keyword_as_id())?;
                let mut builder = FunctionDeclarationTree::builder(Kind::MEMBER);
                builder
                    .set_async(true)
                    .set_generator(generator)
                    .set_is_class_member(element_info.is_class_member)
                    .set_static(element_info.is_static)
                    .set_name(name.clone());
                self.parse_function_tail(
                    &mut builder,
                    if generator {
                        FunctionFlavor::ASYNCHRONOUS_GENERATOR
                    } else {
                        FunctionFlavor::ASYNCHRONOUS
                    },
                )?;
                return Ok(builder.build(self.get_tree_location(name.get_start())));
            } else {
                // { 'str'() {} }
                // { 123() {} }
                // Treat these as if they were computed properties.
                let name_expr = self.parse_literal_expression()?;
                let mut builder = FunctionDeclarationTree::builder(Kind::EXPRESSION);
                builder
                    .set_async(true)
                    .set_generator(generator)
                    .set_is_class_member(element_info.is_class_member)
                    .set_static(element_info.is_static);
                self.parse_function_tail(
                    &mut builder,
                    if generator {
                        FunctionFlavor::ASYNCHRONOUS_GENERATOR
                    } else {
                        FunctionFlavor::ASYNCHRONOUS
                    },
                )?;
                let function = builder.build(self.get_tree_location(name_expr.get_start()));
                return Ok(ComputedPropertyMethodTree::new(
                    self.get_tree_location(name_expr.get_start()),
                    name_expr,
                    function,
                ));
            }
        }
        // expect '[' to start computed property name
        let name_expr = self.parse_computed_property_name()?;
        let mut builder = FunctionDeclarationTree::builder(Kind::EXPRESSION);
        builder
            .set_async(true)
            .set_generator(generator)
            .set_is_class_member(element_info.is_class_member)
            .set_static(element_info.is_static);
        self.parse_function_tail(
            &mut builder,
            if generator {
                FunctionFlavor::ASYNCHRONOUS_GENERATOR
            } else {
                FunctionFlavor::ASYNCHRONOUS
            },
        )?;
        let function = builder.build(self.get_tree_location(name_expr.get_start()));
        Ok(ComputedPropertyMethodTree::new(
            self.get_tree_location(name_expr.get_start()),
            name_expr,
            function,
        ))
    }
    // port: Parser#parseClassStaticInitializerBlock
    fn parse_class_static_initializer_block(&mut self) -> P<Tree> {
        self.eat(TokenType::STATIC);
        self.parse_block()
    }
    // port: Parser#parseFunctionTail
    fn parse_function_tail(
        &mut self,
        builder: &mut function_declaration_tree::Builder,
        function_flavor: FunctionFlavor,
    ) -> P<()> {
        self.function_context_stack.push(function_flavor);
        builder
            .set_generator(function_flavor.is_generator())
            .set_formal_parameter_list(self.parse_formal_parameter_list()?)
            .set_function_body(self.parse_function_body()?);
        self.function_context_stack.pop();
        Ok(())
    }
    // port: Parser#parseSourceElement
    fn parse_source_element(&mut self) -> P<Tree> {
        if self.peek_async_function_start() {
            return self.parse_async_function_declaration();
        }
        if self.peek_function() {
            return self.parse_function_declaration();
        }
        if self.peek_class_declaration() {
            return self.parse_class_declaration();
        }
        // Harmony let block scoped bindings. let can only appear in
        // a block, not as a standalone statement: if() let x ... illegal
        if self.peek(TokenType::LET) {
            return self.parse_variable_statement();
        }
        // const and var are handled inside parseStatement
        self.parse_statement_standard()
    }
    // port: Parser#peekSourceElement
    fn peek_source_element(&mut self) -> bool {
        self.peek_function() || self.peek_statement_standard() || self.peek_declaration()
    }
    // port: Parser#peekAsyncFunctionStart
    fn peek_async_function_start(&mut self) -> bool {
        self.peek_predefined_string(&ASYNC)
            && !self.peek_implicit_semi_colon_at(1)
            && self.peek_function_at(1)
    }
    // port: Parser#eatAsyncFunctionStart
    fn eat_async_function_start(&mut self) {
        self.eat_predefined_string(&ASYNC);
        self.eat(TokenType::FUNCTION);
    }
    // port: Parser#peekFunction
    fn peek_function(&mut self) -> bool {
        self.peek_function_at(0)
    }
    // port: Parser#peekDeclaration
    fn peek_declaration(&mut self) -> bool {
        self.peek(TokenType::LET) || self.peek_class_declaration()
    }
    // port: Parser#peekFunction
    fn peek_function_at(&mut self, index: usize) -> bool {
        self.peek_at(index, TokenType::FUNCTION)
    }
    // 13 Function Definition
    // port: Parser#parseFunctionDeclaration
    fn parse_function_declaration(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(Keywords::FUNCTION.type_());
        let is_generator = self.eat_opt(TokenType::STAR).is_some();
        let mut builder = FunctionDeclarationTree::builder(Kind::DECLARATION);
        builder.set_name(self.eat_id());
        self.parse_function_tail(
            &mut builder,
            if is_generator {
                FunctionFlavor::GENERATOR
            } else {
                FunctionFlavor::NORMAL
            },
        )?;
        Ok(builder.build(self.get_tree_location(start)))
    }
    // port: Parser#parseFunctionExpression
    fn parse_function_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(Keywords::FUNCTION.type_());
        let is_generator = self.eat_opt(TokenType::STAR).is_some();
        let mut builder = FunctionDeclarationTree::builder(Kind::EXPRESSION);
        builder.set_name(self.eat_id_opt());
        self.parse_function_tail(
            &mut builder,
            if is_generator {
                FunctionFlavor::GENERATOR
            } else {
                FunctionFlavor::NORMAL
            },
        )?;
        Ok(builder.build(self.get_tree_location(start)))
    }
    // port: Parser#parseAsyncFunctionDeclaration
    fn parse_async_function_declaration(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat_async_function_start();
        let generator = self.peek(TokenType::STAR);
        if generator {
            self.eat(TokenType::STAR);
        }
        let mut builder = FunctionDeclarationTree::builder(Kind::DECLARATION);
        builder.set_name(self.eat_id()).set_async(true);
        self.parse_function_tail(
            &mut builder,
            if generator {
                FunctionFlavor::ASYNCHRONOUS_GENERATOR
            } else {
                FunctionFlavor::ASYNCHRONOUS
            },
        )?;
        Ok(builder.build(self.get_tree_location(start)))
    }
    // port: Parser#parseAsyncFunctionExpression
    fn parse_async_function_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat_async_function_start();
        let generator = self.peek(TokenType::STAR);
        if generator {
            self.eat(TokenType::STAR);
        }
        let mut builder = FunctionDeclarationTree::builder(Kind::EXPRESSION);
        builder.set_name(self.eat_id_opt()).set_async(true);
        self.parse_function_tail(
            &mut builder,
            if generator {
                FunctionFlavor::ASYNCHRONOUS_GENERATOR
            } else {
                FunctionFlavor::ASYNCHRONOUS
            },
        )?;
        Ok(builder.build(self.get_tree_location(start)))
    }
    // port: Parser#peekParameter
    fn peek_parameter(&mut self) -> bool {
        if self.peek_id() || self.peek(TokenType::ELLIPSIS) {
            return true;
        }
        self.peek(TokenType::OPEN_SQUARE) || self.peek(TokenType::OPEN_CURLY)
    }
    // port: Parser#parseParameter
    fn parse_parameter(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut parameter = if self.peek(TokenType::ELLIPSIS) {
            self.parse_iter_rest(PatternKind::INITIALIZER)?
        } else if self.peek_id() {
            self.parse_identifier_expression()?
        } else if self.peek_pattern_start() {
            self.parse_pattern(PatternKind::INITIALIZER)?
        } else {
            return Err(ParseError {
                class: "java.lang.IllegalStateException",
                message: Some(JsString::from(
                    "parseParameterCalled() without confirming a parameter exists.",
                )),
            });
        };
        if !parameter.is_rest_parameter() && self.peek(TokenType::EQUAL) {
            self.eat(TokenType::EQUAL);
            let default_value = self.parse_assignment_expression()?;
            parameter =
                DefaultParameterTree::new(self.get_tree_location(start), parameter, default_value);
        }
        Ok(parameter)
    }
    // port: Parser#parseFormalParameterList
    fn parse_formal_parameter_list(&mut self) -> P<Tree> {
        let list_start = self.get_tree_start_location();
        self.eat(TokenType::OPEN_PAREN);
        let mut result = Vec::new();
        let mut trailing_comma = false;
        let mut comma_positions = Vec::new();
        while self.peek_parameter() {
            result.push(self.parse_parameter()?);
            if !self.peek(TokenType::CLOSE_PAREN) {
                let comma = self.eat(TokenType::COMMA);
                if let Some(comma) = &comma {
                    comma_positions.push(comma.get_start());
                } else {
                    // semi-arbitrary comma position in case the code is syntactially invalid & missing one
                    comma_positions.push(self.get_tree_end_location());
                }
                if self.peek(TokenType::CLOSE_PAREN) {
                    if !self.config.at_least8 {
                        self.report_error_token(
                            comma.as_ref(),
                            "Invalid trailing comma in formal parameter list",
                            &[],
                        );
                    }
                    trailing_comma = true;
                }
            }
        }
        self.eat(TokenType::CLOSE_PAREN);
        Ok(FormalParameterListTree::new(
            self.get_tree_location(list_start),
            result,
            trailing_comma,
            comma_positions,
        ))
    }
    // port: Parser#parseSetterParameterList
    fn parse_setter_parameter_list(&mut self) -> P<Tree> {
        let parameter_list = self.parse_formal_parameter_list()?;
        let parameters = &parameter_list.as_formal_parameter_list().parameters;
        if parameters.len() != 1 {
            self.report_error_tree(
                Some(&parameter_list),
                "Setter must have exactly 1 parameter, found %d",
                &[JsString::from(parameters.len().to_string())],
            );
        }
        if !parameters.is_empty() {
            let parameter = &parameters[0];
            if parameter.is_rest_parameter() {
                self.report_error_tree(
                    Some(parameter),
                    "Setter must not have a rest parameter",
                    &[],
                );
            }
        }
        Ok(parameter_list)
    }
    // port: Parser#parseFunctionBody
    fn parse_function_body(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::OPEN_CURLY);
        let result = self.parse_source_element_list()?;
        self.eat(TokenType::CLOSE_CURLY);
        Ok(BlockTree::new(self.get_tree_location(start), result))
    }
    // port: Parser#parseSourceElementList
    fn parse_source_element_list(&mut self) -> P<Vec<Tree>> {
        let mut result = Vec::new();
        while self.peek_source_element() {
            result.push(self.parse_source_element()?);
        }
        Ok(result)
    }
    // port: Parser#parseIterSpread
    fn parse_iter_spread(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::ELLIPSIS);
        let operand = self.parse_assignment_expression()?;
        Ok(IterSpreadTree::new(self.get_tree_location(start), operand))
    }
    // 12 Statements
    // In V8, all source elements may appear where statements occur in the grammar.
    // port: Parser#parseStatement
    fn parse_statement(&mut self) -> P<Tree> {
        self.parse_source_element()
    }
    // This function reflects the ECMA standard. Most places use peekStatement instead.
    // port: Parser#parseStatementStandard
    fn parse_statement_standard(&mut self) -> P<Tree> {
        match self.peek_type() {
            TokenType::OPEN_CURLY => self.parse_block(),
            TokenType::CONST | TokenType::VAR => self.parse_variable_statement(),
            TokenType::SEMI_COLON => self.parse_empty_statement(),
            TokenType::IF => self.parse_if_statement(),
            TokenType::DO => self.parse_do_while_statement(),
            TokenType::WHILE => self.parse_while_statement(),
            TokenType::FOR => self.parse_for_statement(),
            TokenType::CONTINUE => self.parse_continue_statement(),
            TokenType::BREAK => self.parse_break_statement(),
            TokenType::RETURN => self.parse_return_statement(),
            TokenType::WITH => self.parse_with_statement(),
            TokenType::SWITCH => self.parse_switch_statement(),
            TokenType::THROW => self.parse_throw_statement(),
            TokenType::TRY => self.parse_try_statement(),
            TokenType::DEBUGGER => self.parse_debugger_statement(),
            _ => {
                if self.peek_labelled_statement() {
                    self.parse_labelled_statement()
                } else {
                    self.parse_expression_statement()
                }
            }
        }
    }
    // In V8 all source elements may appear where statements appear in the grammar.
    // port: Parser#peekStatement
    fn peek_statement(&mut self) -> bool {
        self.peek_source_element()
    }
    // This function reflects the ECMA standard. Most places use peekStatement instead.
    // port: Parser#peekStatementStandard
    fn peek_statement_standard(&mut self) -> bool {
        matches!(
            self.peek_type(),
            TokenType::OPEN_CURLY
                | TokenType::VAR
                | TokenType::CONST
                | TokenType::SEMI_COLON
                | TokenType::IF
                | TokenType::DO
                | TokenType::WHILE
                | TokenType::FOR
                | TokenType::CONTINUE
                | TokenType::BREAK
                | TokenType::RETURN
                | TokenType::WITH
                | TokenType::SWITCH
                | TokenType::THROW
                | TokenType::TRY
                | TokenType::DEBUGGER
                | TokenType::YIELD
                | TokenType::IDENTIFIER
                | TokenType::TYPE
                | TokenType::DECLARE
                | TokenType::MODULE
                | TokenType::NAMESPACE
                | TokenType::THIS
                | TokenType::CLASS
                | TokenType::SUPER
                | TokenType::NUMBER
                | TokenType::BIGINT
                | TokenType::STRING
                | TokenType::NO_SUBSTITUTION_TEMPLATE
                | TokenType::TEMPLATE_HEAD
                | TokenType::NULL
                | TokenType::TRUE
                | TokenType::SLASH // regular expression literal
                | TokenType::SLASH_EQUAL // regular expression literal
                | TokenType::FALSE
                | TokenType::OPEN_SQUARE
                | TokenType::OPEN_PAREN
                | TokenType::NEW
                | TokenType::DELETE
                | TokenType::VOID
                | TokenType::TYPEOF
                | TokenType::PLUS_PLUS
                | TokenType::MINUS_MINUS
                | TokenType::PLUS
                | TokenType::MINUS
                | TokenType::TILDE
                | TokenType::BANG
                | TokenType::IMPORT
        )
    }
    // 12.1 Block
    // port: Parser#parseBlock
    fn parse_block(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::OPEN_CURLY);
        // Spec says Statement list. However functions are also embedded in the wild.
        let result = self.parse_source_element_list()?;
        self.eat(TokenType::CLOSE_CURLY);
        Ok(BlockTree::new(self.get_tree_location(start), result))
    }
    // port: Parser#parseStatementList
    fn parse_statement_list(&mut self) -> P<Vec<Tree>> {
        let mut result = Vec::new();
        while self.peek_statement() {
            result.push(self.parse_statement()?);
        }
        Ok(result)
    }
    // 12.2 Variable Statement
    // port: Parser#parseVariableStatement
    fn parse_variable_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let declarations = required(self.parse_variable_declaration_list()?)?;
        self.eat_possibly_implicit_semi_colon();
        Ok(VariableStatementTree::new(
            self.get_tree_location(start),
            declarations,
        ))
    }
    // port: Parser#parseVariableDeclarationList
    fn parse_variable_declaration_list(&mut self) -> P<Option<Tree>> {
        self.parse_variable_declaration_list_in(Expression::NORMAL)
    }
    // port: Parser#parseVariableDeclarationListNoIn
    fn parse_variable_declaration_list_no_in(&mut self) -> P<Option<Tree>> {
        self.parse_variable_declaration_list_in(Expression::NO_IN)
    }
    // port: Parser#parseVariableDeclarationList
    fn parse_variable_declaration_list_in(&mut self, expression_in: Expression) -> P<Option<Tree>> {
        let start = self.get_tree_start_location();
        let token = self.peek_type();
        match token {
            TokenType::CONST | TokenType::LET | TokenType::VAR => {
                self.eat(token);
            }
            _ => {
                let token = self.peek_token();
                self.report_error_token(Some(&token), "expected declaration", &[]);
                return Ok(None);
            }
        }
        let mut declarations = vec![self.parse_variable_declaration(token, expression_in)?];
        while self.peek(TokenType::COMMA) {
            self.eat(TokenType::COMMA);
            declarations.push(self.parse_variable_declaration(token, expression_in)?);
        }
        Ok(Some(VariableDeclarationListTree::new(
            self.get_tree_location(start),
            token,
            declarations,
        )))
    }
    // port: Parser#parseVariableDeclaration
    fn parse_variable_declaration(
        &mut self,
        binding: TokenType,
        expression_in: Expression,
    ) -> P<Tree> {
        let start = self.get_tree_start_location();
        let lvalue = if self.peek_pattern_start() {
            self.parse_pattern(PatternKind::INITIALIZER)?
        } else {
            self.parse_identifier_expression()?
        };
        let mut initializer = None;
        if self.peek(TokenType::EQUAL) {
            initializer = Some(self.parse_initializer(expression_in)?);
        } else if expression_in != Expression::NO_IN {
            // NOTE(blickly): this is a bit of a hack, declarations outside of for statements allow "in",
            // and by chance, also must have initializers for const/destructuring. Vanilla for loops
            // also require intializers, but are handled separately in checkVanillaForInitializers
            self.maybe_report_no_initializer(binding, &lvalue);
        }
        Ok(VariableDeclarationTree::new(
            self.get_tree_location(start),
            lvalue,
            initializer,
        ))
    }
    // port: Parser#parseInitializer
    fn parse_initializer(&mut self, expression_in: Expression) -> P<Tree> {
        self.eat(TokenType::EQUAL);
        self.parse_assignment(expression_in)
    }
    // 12.3 Empty Statement
    // port: Parser#parseEmptyStatement
    fn parse_empty_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::SEMI_COLON);
        Ok(EmptyStatementTree::new(self.get_tree_location(start)))
    }
    // 12.4 Expression Statement
    // port: Parser#parseExpressionStatement
    fn parse_expression_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let expression = self.parse_expression()?;
        self.eat_possibly_implicit_semi_colon();
        Ok(ExpressionStatementTree::new(
            self.get_tree_location(start),
            expression,
        ))
    }
    // 12.5 If Statement
    // port: Parser#parseIfStatement
    fn parse_if_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::IF);
        self.eat(TokenType::OPEN_PAREN);
        let condition = self.parse_expression()?;
        self.eat(TokenType::CLOSE_PAREN);
        let if_clause = self.parse_statement()?;
        let mut else_clause = None;
        if self.peek(TokenType::ELSE) {
            self.eat(TokenType::ELSE);
            else_clause = Some(self.parse_statement()?);
        }
        Ok(IfStatementTree::new(
            self.get_tree_location(start),
            condition,
            if_clause,
            else_clause,
        ))
    }
    // 12.6 Iteration Statements
    // 12.6.1 The do-while Statement
    // port: Parser#parseDoWhileStatement
    fn parse_do_while_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::DO);
        let body = self.parse_statement()?;
        self.eat(TokenType::WHILE);
        self.eat(TokenType::OPEN_PAREN);
        let condition = self.parse_expression()?;
        self.eat(TokenType::CLOSE_PAREN);
        // The semicolon after the "do-while" is optional.
        if self.peek(TokenType::SEMI_COLON) {
            self.eat(TokenType::SEMI_COLON);
        }
        Ok(DoWhileStatementTree::new(
            self.get_tree_location(start),
            body,
            condition,
        ))
    }
    // 12.6.2 The while Statement
    // port: Parser#parseWhileStatement
    fn parse_while_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::WHILE);
        self.eat(TokenType::OPEN_PAREN);
        let condition = self.parse_expression()?;
        self.eat(TokenType::CLOSE_PAREN);
        let body = self.parse_statement()?;
        Ok(WhileStatementTree::new(
            self.get_tree_location(start),
            condition,
            body,
        ))
    }
    // 12.6.3 The for Statement
    // 12.6.4 The for-in Statement
    // The for-of Statement
    // The for-await-of Statement
    // port: Parser#parseForStatement
    fn parse_for_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::FOR);
        let awaited = self.peek_predefined_string(&AWAIT);
        if awaited {
            self.eat_predefined_string(&AWAIT);
        }
        self.eat(TokenType::OPEN_PAREN);
        if self.peek_variable_declaration_list() {
            let variables = required(self.parse_variable_declaration_list_no_in()?)?;
            let declarations = &variables.as_variable_declaration_list().declarations;
            if self.peek(TokenType::IN) {
                if awaited {
                    self.report_error(
                        "for-await-of is the only allowed asynchronous iteration",
                        &[],
                    );
                }
                // for-in: only one declaration allowed
                if declarations.len() > 1 {
                    self.report_error(
                        "for-in statement may not have more than one variable declaration",
                        &[],
                    );
                }
                let declaration = &declarations[0];
                if declaration.as_variable_declaration().initializer.is_some() {
                    // An initializer is allowed here in ES5 and below, but not in ES6.
                    // Warn about it, to encourage people to eliminate it from their code.
                    // http://esdiscuss.org/topic/initializer-expression-on-for-in-syntax-subject
                    if self.config.at_least6 {
                        self.report_error("for-in statement may not have initializer", &[]);
                    } else {
                        self.error_reporter.report_warning(
                            declaration.location.start.clone(),
                            "for-in statement should not have initializer",
                            &[],
                        );
                    }
                }
                return self.parse_for_in_statement(start, variables);
            } else if self.peek_predefined_string(PredefinedName::OF) {
                // for-of: only one declaration allowed
                if declarations.len() > 1 {
                    self.report_error(
                        if awaited {
                            "for-await-of statement may not have more than one variable declaration"
                        } else {
                            "for-of statement may not have more than one variable declaration"
                        },
                        &[],
                    );
                }
                // for-of: initializer is illegal
                let declaration = &declarations[0];
                if declaration.as_variable_declaration().initializer.is_some() {
                    self.report_error(
                        if awaited {
                            "for-await-of statement may not have initializer"
                        } else {
                            "for-of statement may not have initializer"
                        },
                        &[],
                    );
                }
                return if awaited {
                    self.parse_for_await_of_statement(start, variables)
                } else {
                    self.parse_for_of_statement(start, variables)
                };
            } else {
                // "Vanilla" for statement: const/destructuring must have initializer
                self.check_vanilla_for_initializers(&variables);
                return self.parse_for_statement_tail(start, Some(variables));
            }
        }
        if self.peek(TokenType::SEMI_COLON) {
            return self.parse_for_statement_tail(start, None);
        }
        let mut initializer = self.parse_expression_no_in()?;
        if self.peek(TokenType::IN)
            || self.peek(TokenType::EQUAL)
            || self.peek_predefined_string(PredefinedName::OF)
        {
            initializer = self.transform_left_hand_side_expression(initializer)?;
            if !initializer.is_valid_assignment_target() {
                self.report_error("invalid assignment target", &[]);
            }
        }
        if (self.peek(TokenType::IN) || self.peek_predefined_string(PredefinedName::OF))
            && initializer.type_ != ParseTreeType::BINARY_OPERATOR
            && initializer.type_ != ParseTreeType::COMMA_EXPRESSION
        {
            if self.peek(TokenType::IN) {
                return self.parse_for_in_statement(start, initializer);
            }
            // for {await}? ( _ of _ )
            return if awaited {
                self.parse_for_await_of_statement(start, initializer)
            } else {
                self.parse_for_of_statement(start, initializer)
            };
        }
        self.parse_for_statement_tail(start, Some(initializer))
    }
    // The for-of Statement
    // for  (  { let | var }?  identifier  of  expression  )  statement
    // port: Parser#parseForOfStatement
    fn parse_for_of_statement(&mut self, start: SourcePosition, initializer: Tree) -> P<Tree> {
        self.eat_predefined_string(PredefinedName::OF);
        let collection = self.parse_expression()?;
        self.eat(TokenType::CLOSE_PAREN);
        let body = self.parse_statement()?;
        Ok(ForOfStatementTree::new(
            self.get_tree_location(start),
            initializer,
            collection,
            body,
        ))
    }
    // port: Parser#parseForAwaitOfStatement
    fn parse_for_await_of_statement(
        &mut self,
        start: SourcePosition,
        initializer: Tree,
    ) -> P<Tree> {
        self.eat_predefined_string(PredefinedName::OF);
        let collection = self.parse_expression()?;
        self.eat(TokenType::CLOSE_PAREN);
        let body = self.parse_statement()?;
        Ok(ForAwaitOfStatementTree::new(
            self.get_tree_location(start),
            initializer,
            collection,
            body,
        ))
    }
    // Checks variable declarations in for statements.
    // port: Parser#checkVanillaForInitializers
    fn check_vanilla_for_initializers(&mut self, variables: &Tree) {
        let variables = variables.as_variable_declaration_list();
        for declaration in &variables.declarations {
            let declaration = declaration.as_variable_declaration();
            if declaration.initializer.is_none() {
                self.maybe_report_no_initializer(variables.declaration_type, &declaration.lvalue);
            }
        }
    }
    // Reports if declaration requires an initializer, assuming initializer is absent.
    // port: Parser#maybeReportNoInitializer
    fn maybe_report_no_initializer(&mut self, token: TokenType, lvalue: &Tree) {
        if token == TokenType::CONST {
            self.report_error("const variables must have an initializer", &[]);
        } else if lvalue.is_pattern() {
            self.report_error("destructuring must have an initializer", &[]);
        }
    }
    // port: Parser#peekVariableDeclarationList
    fn peek_variable_declaration_list(&mut self) -> bool {
        matches!(
            self.peek_type(),
            TokenType::VAR | TokenType::CONST | TokenType::LET
        )
    }
    // 12.6.3 The for Statement
    // port: Parser#parseForStatement
    fn parse_for_statement_tail(
        &mut self,
        start: SourcePosition,
        initializer: Option<Tree>,
    ) -> P<Tree> {
        let initializer = initializer.unwrap_or_else(|| {
            let end = self.get_tree_end_location();
            let start = self.get_tree_start_location();
            NullTree::new(SourceRange::new(end, start))
        });
        self.eat(TokenType::SEMI_COLON);
        let condition = if !self.peek(TokenType::SEMI_COLON) {
            self.parse_expression()?
        } else {
            let end = self.get_tree_end_location();
            let start = self.get_tree_start_location();
            NullTree::new(SourceRange::new(end, start))
        };
        self.eat(TokenType::SEMI_COLON);
        let increment = if !self.peek(TokenType::CLOSE_PAREN) {
            self.parse_expression()?
        } else {
            let end = self.get_tree_end_location();
            let start = self.get_tree_start_location();
            NullTree::new(SourceRange::new(end, start))
        };
        self.eat(TokenType::CLOSE_PAREN);
        let body = self.parse_statement()?;
        Ok(ForStatementTree::new(
            self.get_tree_location(start),
            initializer,
            condition,
            increment,
            body,
        ))
    }
    // 12.6.4 The for-in Statement
    // port: Parser#parseForInStatement
    fn parse_for_in_statement(&mut self, start: SourcePosition, initializer: Tree) -> P<Tree> {
        self.eat(TokenType::IN);
        let collection = self.parse_expression()?;
        self.eat(TokenType::CLOSE_PAREN);
        let body = self.parse_statement()?;
        Ok(ForInStatementTree::new(
            self.get_tree_location(start),
            initializer,
            collection,
            body,
        ))
    }
    // 12.7 The continue Statement
    // port: Parser#parseContinueStatement
    fn parse_continue_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::CONTINUE);
        let mut name = None;
        if !self.peek_implicit_semi_colon() {
            name = self.eat_id_opt();
        }
        self.eat_possibly_implicit_semi_colon();
        Ok(ContinueStatementTree::new(
            self.get_tree_location(start),
            name,
        ))
    }
    // 12.8 The break Statement
    // port: Parser#parseBreakStatement
    fn parse_break_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::BREAK);
        let mut name = None;
        if !self.peek_implicit_semi_colon() {
            name = self.eat_id_opt();
        }
        self.eat_possibly_implicit_semi_colon();
        Ok(BreakStatementTree::new(self.get_tree_location(start), name))
    }
    // 12.9 The return Statement
    // port: Parser#parseReturnStatement
    fn parse_return_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::RETURN);
        let mut expression = None;
        if !self.peek_implicit_semi_colon() {
            expression = Some(self.parse_expression()?);
        }
        self.eat_possibly_implicit_semi_colon();
        Ok(ReturnStatementTree::new(
            self.get_tree_location(start),
            expression,
        ))
    }
    // 12.10 The with Statement
    // port: Parser#parseWithStatement
    fn parse_with_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::WITH);
        self.eat(TokenType::OPEN_PAREN);
        let expression = self.parse_expression()?;
        self.eat(TokenType::CLOSE_PAREN);
        let body = self.parse_statement()?;
        Ok(WithStatementTree::new(
            self.get_tree_location(start),
            expression,
            body,
        ))
    }
    // 12.11 The switch Statement
    // port: Parser#parseSwitchStatement
    fn parse_switch_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::SWITCH);
        self.eat(TokenType::OPEN_PAREN);
        let expression = self.parse_expression()?;
        self.eat(TokenType::CLOSE_PAREN);
        self.eat(TokenType::OPEN_CURLY);
        let case_clauses = self.parse_case_clauses()?;
        self.eat(TokenType::CLOSE_CURLY);
        Ok(SwitchStatementTree::new(
            self.get_tree_location(start),
            expression,
            case_clauses,
        ))
    }
    // port: Parser#parseCaseClauses
    fn parse_case_clauses(&mut self) -> P<Vec<Tree>> {
        let mut found_default_clause = false;
        let mut result = Vec::new();
        loop {
            let start = self.get_tree_start_location();
            match self.peek_type() {
                TokenType::CASE => {
                    self.eat(TokenType::CASE);
                    let expression = self.parse_expression()?;
                    self.eat(TokenType::COLON);
                    let statements = self.parse_case_statements_opt()?;
                    result.push(CaseClauseTree::new(
                        self.get_tree_location(start),
                        expression,
                        statements,
                    ));
                }
                TokenType::DEFAULT => {
                    if found_default_clause {
                        self.report_error(
                            "Switch statements may have at most one default clause",
                            &[],
                        );
                    } else {
                        found_default_clause = true;
                    }
                    self.eat(TokenType::DEFAULT);
                    self.eat(TokenType::COLON);
                    let location = self.get_tree_location(start);
                    let statements = self.parse_case_statements_opt()?;
                    result.push(DefaultClauseTree::new(location, statements));
                }
                _ => return Ok(result),
            }
        }
    }
    // port: Parser#parseCaseStatementsOpt
    fn parse_case_statements_opt(&mut self) -> P<Vec<Tree>> {
        self.parse_statement_list()
    }
    // 12.12 Labelled Statement
    // port: Parser#parseLabelledStatement
    fn parse_labelled_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let name = self.eat_id();
        self.eat(TokenType::COLON);
        let location = self.get_tree_location(start);
        let statement = self.parse_statement()?;
        Ok(LabelledStatementTree::new(location, name, statement))
    }
    // port: Parser#peekLabelledStatement
    fn peek_labelled_statement(&mut self) -> bool {
        self.peek_id() && self.peek_at(1, TokenType::COLON)
    }
    // 12.13 Throw Statement
    // port: Parser#parseThrowStatement
    fn parse_throw_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::THROW);
        let mut value = None;
        if self.peek_implicit_semi_colon() {
            self.report_error("semicolon/newline not allowed after 'throw'", &[]);
        } else {
            value = Some(self.parse_expression()?);
        }
        self.eat_possibly_implicit_semi_colon();
        Ok(ThrowStatementTree::new(
            self.get_tree_location(start),
            value,
        ))
    }
    // 12.14 Try Statement
    // port: Parser#parseTryStatement
    fn parse_try_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::TRY);
        let body = self.parse_block()?;
        let mut catch_block = None;
        if self.peek(TokenType::CATCH) {
            catch_block = Some(self.parse_catch()?);
        }
        let mut finally_block = None;
        if self.peek(TokenType::FINALLY) {
            finally_block = Some(self.parse_finally_block()?);
        }
        if catch_block.is_none() && finally_block.is_none() {
            self.report_error("'catch' or 'finally' expected.", &[]);
        }
        Ok(TryStatementTree::new(
            self.get_tree_location(start),
            body,
            catch_block,
            finally_block,
        ))
    }
    // port: Parser#parseCatch
    fn parse_catch(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::CATCH);
        let end = self.get_tree_end_location();
        let next = self.get_tree_start_location();
        let mut exception = EmptyStatementTree::new(SourceRange::new(end, next));
        if self.scanner.peek_token_ref_at(0).type_ == TokenType::OPEN_PAREN {
            self.eat(TokenType::OPEN_PAREN);
            exception = if self.peek_pattern_start() {
                self.parse_pattern(PatternKind::INITIALIZER)?
            } else {
                self.parse_identifier_expression()?
            };
            self.eat(TokenType::CLOSE_PAREN);
        } else {
            self.record_feature_used(Feature::OPTIONAL_CATCH_BINDING);
        }
        let catch_body = self.parse_block()?;
        Ok(CatchTree::new(
            self.get_tree_location(start),
            exception,
            catch_body,
        ))
    }
    // port: Parser#parseFinallyBlock
    fn parse_finally_block(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::FINALLY);
        let finally_block = self.parse_block()?;
        Ok(FinallyTree::new(
            self.get_tree_location(start),
            finally_block,
        ))
    }
    // 12.15 The Debugger Statement
    // port: Parser#parseDebuggerStatement
    fn parse_debugger_statement(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::DEBUGGER);
        self.eat_possibly_implicit_semi_colon();
        Ok(DebuggerStatementTree::new(self.get_tree_location(start)))
    }
    // 11.1 Primary Expressions
    // port: Parser#parsePrimaryExpression
    fn parse_primary_expression(&mut self) -> P<Tree> {
        match self.peek_type() {
            TokenType::CLASS => self.parse_class_expression(),
            TokenType::SUPER => self.parse_super_expression(),
            TokenType::THIS => self.parse_this_expression(),
            TokenType::IMPORT => self.parse_dynamic_import_expression(),
            TokenType::IDENTIFIER
            | TokenType::TYPE
            | TokenType::DECLARE
            | TokenType::MODULE
            | TokenType::NAMESPACE => self.parse_identifier_expression(),
            TokenType::NUMBER
            | TokenType::STRING
            | TokenType::BIGINT
            | TokenType::TRUE
            | TokenType::FALSE
            | TokenType::NULL => self.parse_literal_expression(),
            TokenType::NO_SUBSTITUTION_TEMPLATE | TokenType::TEMPLATE_HEAD => {
                self.parse_template_literal(None)
            }
            TokenType::OPEN_SQUARE => self.parse_array_initializer(),
            TokenType::OPEN_CURLY => self.parse_object_literal(),
            TokenType::OPEN_PAREN => {
                self.parse_cover_parenthesized_expression_and_arrow_parameter_list()
            }
            TokenType::SLASH | TokenType::SLASH_EQUAL => self.parse_regular_expression_literal(),
            _ => self.parse_missing_primary_expression(),
        }
    }
    // port: Parser#parseSuperExpression
    fn parse_super_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::SUPER);
        if self.peek(TokenType::QUESTION_DOT) {
            // super?.() not allowed
            self.report_error("Optional chaining is forbidden in super?.", &[]);
        }
        Ok(SuperExpressionTree::new(self.get_tree_location(start)))
    }
    // port: Parser#parseThisExpression
    fn parse_this_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::THIS);
        Ok(ThisExpressionTree::new(self.get_tree_location(start)))
    }
    // https://tc39.github.io/proposal-dynamic-import
    // port: Parser#parseDynamicImportExpression
    fn parse_dynamic_import_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::IMPORT);
        if self.peek(TokenType::QUESTION_DOT) {
            // import?.() not allowed
            self.report_error("Optional chaining is forbidden in import?.", &[]);
        }
        self.eat(TokenType::OPEN_PAREN);
        let argument = self.parse_assignment_expression()?;
        self.eat(TokenType::CLOSE_PAREN);
        self.record_feature_used(Feature::DYNAMIC_IMPORT);
        Ok(DynamicImportTree::new(
            self.get_tree_location(start),
            argument,
        ))
    }
    // port: Parser#parseIdentifierExpression
    fn parse_identifier_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let identifier = self.eat_id();
        Ok(IdentifierExpressionTree::new(
            self.get_tree_location(start),
            identifier,
        ))
    }
    // port: Parser#parseLiteralExpression
    fn parse_literal_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let literal = self.next_literal_token();
        if literal.type_ == TokenType::STRING
            && matches!(&literal.data, TokenData::StringLiteral(s) if s.has_unescaped_unicode_line_or_paragraph_separator())
        {
            self.record_feature_used(Feature::UNESCAPED_UNICODE_LINE_OR_PARAGRAPH_SEP);
        }
        if literal.type_ == TokenType::NUMBER
            && literal.to_string_utf16().index_of_char(u16::from(b'_')) >= 0
        {
            self.record_feature_used(Feature::NUMERIC_SEPARATOR);
        }
        if literal.type_ == TokenType::BIGINT {
            self.record_feature_used(Feature::BIGINT);
        }
        Ok(LiteralExpressionTree::new(
            self.get_tree_location(start),
            literal,
        ))
    }
    // Constructs a template literal expression tree. "operand" is used to handle the case like
    // "foo`bar`", which is a CallExpression or MemberExpression that calls the function foo() with
    // the template literal as the argument (with extra handling). In this case, operand would be
    // "foo", which is the callsite.
    //
    // <p>We store this operand in the TemplateLiteralExpressionTree and generate a TAGGED_TEMPLATELIT
    // node if it's not null later when transpiling.
    //
    // @param operand A non-null value would represent the callsite
    // @return The template literal expression
    // port: Parser#parseTemplateLiteral
    fn parse_template_literal(&mut self, operand: Option<Tree>) -> P<Tree> {
        let start = match &operand {
            None => self.get_tree_start_location(),
            Some(operand) => operand.location.start.clone(),
        };
        let token = self.next_token();
        if !matches!(&token.data, TokenData::TemplateLiteral(_)) {
            self.report_error_token(
                Some(&token),
                "Unexpected template literal token %s.",
                &[token.type_.to_string_utf16()],
            );
        }
        let is_tagged_template = operand.is_some();
        let mut template_token = token;
        if !matches!(&template_token.data, TokenData::TemplateLiteral(_)) {
            return Err(ParseError {
                class: "java.lang.ClassCastException",
                message: None,
            });
        }
        if !is_tagged_template {
            self.report_template_error_if_present(&template_token);
        }
        let mut elements = vec![TemplateLiteralPortionTree::new(
            template_token.location.clone(),
            template_token.clone(),
        )];
        if template_token.type_ == TokenType::NO_SUBSTITUTION_TEMPLATE {
            return Ok(TemplateLiteralExpressionTree::new(
                self.get_tree_location(start),
                operand,
                elements,
            ));
        }
        // `abc${
        let mut expression = self.parse_expression()?;
        elements.push(TemplateSubstitutionTree::new(
            expression.location.clone(),
            expression,
        ));
        while !self.error_reporter.had_error() {
            template_token = self.next_template_literal_token();
            if template_token.type_ == TokenType::ERROR
                || template_token.type_ == TokenType::END_OF_FILE
            {
                break;
            }
            if !is_tagged_template {
                self.report_template_error_if_present(&template_token);
            }
            elements.push(TemplateLiteralPortionTree::new(
                template_token.location.clone(),
                template_token.clone(),
            ));
            if template_token.type_ == TokenType::TEMPLATE_TAIL {
                break;
            }
            expression = self.parse_expression()?;
            elements.push(TemplateSubstitutionTree::new(
                expression.location.clone(),
                expression,
            ));
        }
        Ok(TemplateLiteralExpressionTree::new(
            self.get_tree_location(start),
            operand,
            elements,
        ))
    }
    // port: Parser#nextLiteralToken
    fn next_literal_token(&mut self) -> Token {
        self.next_token()
    }
    // port: Parser#parseRegularExpressionLiteral
    fn parse_regular_expression_literal(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let literal = self.next_regular_expression_literal_token();
        self.record_feature_used(Feature::REGEXP_SYNTAX);
        Ok(LiteralExpressionTree::new(
            self.get_tree_location(start),
            literal,
        ))
    }
    // port: Parser#parseArrayInitializer
    fn parse_array_initializer(&mut self) -> P<Tree> {
        if self.peek_type_at(1) == TokenType::FOR {
            self.parse_array_comprehension()
        } else {
            self.parse_array_literal()
        }
    }
    // port: Parser#parseGeneratorComprehension
    fn parse_generator_comprehension(&mut self) -> P<Tree> {
        self.parse_comprehension(
            ComprehensionType::GENERATOR,
            TokenType::OPEN_PAREN,
            TokenType::CLOSE_PAREN,
        )
    }
    // port: Parser#parseArrayComprehension
    fn parse_array_comprehension(&mut self) -> P<Tree> {
        self.parse_comprehension(
            ComprehensionType::ARRAY,
            TokenType::OPEN_SQUARE,
            TokenType::CLOSE_SQUARE,
        )
    }
    // port: Parser#parseComprehension
    fn parse_comprehension(
        &mut self,
        type_: ComprehensionType,
        start_token: TokenType,
        end_token: TokenType,
    ) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(start_token);
        let mut children = Vec::new();
        while self.peek(TokenType::FOR) || self.peek(TokenType::IF) {
            if self.peek(TokenType::FOR) {
                children.push(self.parse_comprehension_for()?);
            } else {
                children.push(self.parse_comprehension_if()?);
            }
        }
        let tail_expression = self.parse_assignment_expression()?;
        self.eat(end_token);
        Ok(ComprehensionTree::new(
            self.get_tree_location(start),
            type_,
            children,
            tail_expression,
        ))
    }
    // port: Parser#parseComprehensionFor
    fn parse_comprehension_for(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::FOR);
        self.eat(TokenType::OPEN_PAREN);
        let initializer = if self.peek_id() {
            self.parse_identifier_expression()?
        } else {
            self.parse_pattern(PatternKind::ANY)?
        };
        self.eat_predefined_string(PredefinedName::OF);
        let collection = self.parse_assignment_expression()?;
        self.eat(TokenType::CLOSE_PAREN);
        Ok(ComprehensionForTree::new(
            self.get_tree_location(start),
            initializer,
            collection,
        ))
    }
    // port: Parser#parseComprehensionIf
    fn parse_comprehension_if(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::IF);
        self.eat(TokenType::OPEN_PAREN);
        let initializer = self.parse_assignment_expression()?;
        self.eat(TokenType::CLOSE_PAREN);
        Ok(ComprehensionIfTree::new(
            self.get_tree_location(start),
            initializer,
        ))
    }
    // 11.1.4 Array Literal Expression
    // port: Parser#parseArrayLiteral
    fn parse_array_literal(&mut self) -> P<Tree> {
        // ArrayLiteral :
        //   [ Elisionopt ]
        //   [ ElementList ]
        //   [ ElementList , Elisionopt ]
        //
        // ElementList :
        //   Elisionopt AssignmentOrSpreadExpression
        //   ElementList , Elisionopt AssignmentOrSpreadExpression
        //
        // Elision :
        //   ,
        //   Elision ,
        let start = self.get_tree_start_location();
        let mut elements = Vec::new();
        self.eat(TokenType::OPEN_SQUARE);
        let mut trailing_comma_token = None;
        while self.peek(TokenType::COMMA)
            || self.peek(TokenType::ELLIPSIS)
            || self.peek_assignment_expression()
        {
            trailing_comma_token = None;
            if self.peek(TokenType::COMMA) {
                let comma_start = self.get_tree_start_location();
                trailing_comma_token = self.eat(TokenType::COMMA);
                // Consider the empty element to start & end immediately before the comma token.
                elements.push(NullTree::new(SourceRange::new(
                    comma_start.clone(),
                    comma_start,
                )));
            } else {
                if self.peek(TokenType::ELLIPSIS) {
                    self.record_feature_used(Feature::SPREAD_EXPRESSIONS);
                    elements.push(self.parse_iter_spread()?);
                } else {
                    elements.push(self.parse_assignment_expression()?);
                }
                if !self.peek(TokenType::CLOSE_SQUARE) {
                    trailing_comma_token = self.eat(TokenType::COMMA);
                }
            }
        }
        self.eat(TokenType::CLOSE_SQUARE);
        self.maybe_report_trailing_comma(trailing_comma_token.as_ref());
        Ok(ArrayLiteralExpressionTree::new(
            self.get_tree_location(start),
            elements,
            trailing_comma_token.is_some(),
        ))
    }
    // 11.1.4 Object Literal Expression
    // port: Parser#parseObjectLiteral
    fn parse_object_literal(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut result = Vec::new();
        self.eat(TokenType::OPEN_CURLY);
        let mut comma_token = None;
        while self.peek(TokenType::ELLIPSIS)
            || self.peek_property_name_or_computed_prop(0)
            || self.peek(TokenType::STAR)
        {
            result.push(self.parse_object_literal_property_assignment()?);
            comma_token = self.eat_opt(TokenType::COMMA);
            if comma_token.is_none() {
                break;
            }
        }
        self.eat(TokenType::CLOSE_CURLY);
        self.maybe_report_trailing_comma(comma_token.as_ref());
        Ok(ObjectLiteralExpressionTree::new(
            self.get_tree_location(start),
            result,
            comma_token.is_some(),
        ))
    }
    // port: Parser#maybeReportTrailingComma
    pub fn maybe_report_trailing_comma(&mut self, comma_token: Option<&Token>) {
        if let Some(comma_token) = comma_token {
            self.record_feature_used(Feature::TRAILING_COMMA);
            if self.config.warn_trailing_commas {
                // In ES3 mode warn about trailing commas which aren't accepted by
                // older browsers (such as IE8).
                self.error_reporter.report_warning(
                    comma_token.location.start.clone(),
                    "Trailing comma is not legal in an ECMA-262 object initializer",
                    &[],
                );
            }
        }
    }
    // port: Parser#peekPropertyNameOrComputedProp
    fn peek_property_name_or_computed_prop(&mut self, token_index: usize) -> bool {
        self.peek_property_name(token_index)
            || self.peek_type_at(token_index) == TokenType::OPEN_SQUARE
    }
    // port: Parser#peekPropertyName
    fn peek_property_name(&mut self, token_index: usize) -> bool {
        let type_ = self.peek_type_at(token_index);
        match type_ {
            TokenType::IDENTIFIER | TokenType::STRING | TokenType::NUMBER | TokenType::BIGINT => {
                true
            }
            _ => Keywords::is_keyword_type(type_),
        }
    }
    // port: Parser#parseObjectLiteralPropertyAssignment
    fn parse_object_literal_property_assignment(&mut self) -> P<Tree> {
        let type_ = self.peek_type();
        if type_ == TokenType::STAR {
            return self.parse_object_literal_property_assignment_generator();
        } else if type_ == TokenType::ELLIPSIS {
            self.record_feature_used(Feature::OBJECT_LITERALS_WITH_SPREAD);
            let start = self.get_tree_start_location();
            self.eat(TokenType::ELLIPSIS);
            let operand = self.parse_assignment_expression()?;
            return Ok(ObjectSpreadTree::new(
                self.get_tree_location(start),
                operand,
            ));
        } else if matches!(
            type_,
            TokenType::STRING | TokenType::NUMBER | TokenType::BIGINT | TokenType::IDENTIFIER
        ) || Keywords::is_keyword_type(type_)
        {
            if self.peek_get_accessor() {
                return self.parse_object_literal_get_accessor();
            } else if self.peek_set_accessor() {
                return self.parse_object_literal_set_accessor();
            } else if self.peek_async_method() {
                return self.parse_object_literal_async_method();
            } else if self.peek_type_at(1) == TokenType::OPEN_PAREN {
                return self.parse_object_literal_method_declaration();
            } else {
                return self.parse_object_literal_property_name_assignment();
            }
        } else if type_ == TokenType::OPEN_SQUARE {
            let start = self.get_tree_start_location();
            let name = self.parse_computed_property_name()?;
            if self.peek(TokenType::COLON) {
                self.eat(TokenType::COLON);
                let value = self.parse_assignment_expression()?;
                return Ok(ComputedPropertyDefinitionTree::new(
                    self.get_tree_location(start),
                    name,
                    value,
                ));
            } else {
                let mut builder = FunctionDeclarationTree::builder(Kind::EXPRESSION);
                self.parse_function_tail(&mut builder, FunctionFlavor::NORMAL)?;
                let value = builder.build(self.get_tree_location(start.clone()));
                return Ok(ComputedPropertyMethodTree::new(
                    self.get_tree_location(start),
                    name,
                    value,
                ));
            }
        }
        Err(ParseError {
            class: "java.lang.RuntimeException",
            message: Some(JsString::from("unreachable")),
        })
    }
    // port: Parser#parseObjectLiteralPropertyAssignmentGenerator
    fn parse_object_literal_property_assignment_generator(&mut self) -> P<Tree> {
        let type_ = self.peek_type_at(1);
        if matches!(
            type_,
            TokenType::STRING | TokenType::NUMBER | TokenType::IDENTIFIER
        ) || Keywords::is_keyword_type(type_)
        {
            // parseMethodDeclaration will consume the '*'.
            return self.parse_object_literal_method_declaration();
        }
        let start = self.get_tree_start_location();
        self.eat(TokenType::STAR);
        let name = self.parse_computed_property_name()?;
        let mut builder = FunctionDeclarationTree::builder(Kind::EXPRESSION);
        self.parse_function_tail(&mut builder, FunctionFlavor::GENERATOR)?;
        let value = builder.build(self.get_tree_location(start.clone()));
        Ok(ComputedPropertyMethodTree::new(
            self.get_tree_location(start),
            name,
            value,
        ))
    }
    // port: Parser#parseComputedPropertyName
    fn parse_computed_property_name(&mut self) -> P<Tree> {
        self.eat(TokenType::OPEN_SQUARE);
        let assign = self.parse_assignment_expression()?;
        self.eat(TokenType::CLOSE_SQUARE);
        Ok(assign)
    }
    // port: Parser#peekGetAccessor
    fn peek_get_accessor(&mut self) -> bool {
        self.peek_predefined_string(PredefinedName::GET)
            && self.peek_property_name_or_computed_prop(1)
    }
    // port: Parser#peekPredefinedString
    fn peek_predefined_string(&mut self, string: &JsString) -> bool {
        self.peek_predefined_string_at(0, string)
    }
    // port: Parser#eatPredefinedString
    fn eat_predefined_string(&mut self, string: &JsString) -> Option<Token> {
        let token = self.eat_id();
        if token
            .as_ref()
            .is_none_or(|token| !token.value_equals(string))
        {
            self.report_expected_error(token.as_ref(), string.clone());
            return None;
        }
        token
    }
    // port: Parser#peekPredefinedString
    fn peek_predefined_string_at(&mut self, index: usize, string: &JsString) -> bool {
        self.peek_at(index, TokenType::IDENTIFIER)
            && self.scanner.peek_token_ref_at(index).value_equals(string)
    }
    // port: Parser#parseObjectLiteralGetAccessor
    fn parse_object_literal_get_accessor(&mut self) -> P<Tree> {
        let info = self.create_object_literal_element_info();
        self.parse_get_accessor(info)
    }
    // port: Parser#parseGetAccessor
    fn parse_get_accessor(&mut self, element_info: ClassOrObjectElementInfo) -> P<Tree> {
        self.eat_predefined_string(PredefinedName::GET);
        if self.peek_property_name(0) {
            let property_name = required(self.eat_object_literal_property_name())?;
            self.eat(TokenType::OPEN_PAREN);
            self.eat(TokenType::CLOSE_PAREN);
            let body = self.parse_function_body()?;
            self.record_feature_used(Feature::GETTER);
            Ok(GetAccessorTree::new(
                self.get_tree_location(element_info.start),
                property_name,
                element_info.is_class_member,
                element_info.is_static,
                body,
            ))
        } else {
            let property = self.parse_computed_property_name()?;
            self.eat(TokenType::OPEN_PAREN);
            self.eat(TokenType::CLOSE_PAREN);
            let body = self.parse_function_body()?;
            self.record_feature_used(Feature::GETTER);
            Ok(ComputedPropertyGetterTree::new(
                self.get_tree_location(element_info.start),
                property,
                element_info.is_static,
                body,
            ))
        }
    }
    // port: Parser#peekSetAccessor
    fn peek_set_accessor(&mut self) -> bool {
        self.peek_predefined_string(PredefinedName::SET)
            && self.peek_property_name_or_computed_prop(1)
    }
    // port: Parser#parseObjectLiteralSetAccessor
    fn parse_object_literal_set_accessor(&mut self) -> P<Tree> {
        let info = self.create_object_literal_element_info();
        self.parse_set_accessor(info)
    }
    // port: Parser#parseSetAccessor
    fn parse_set_accessor(&mut self, element_info: ClassOrObjectElementInfo) -> P<Tree> {
        self.eat_predefined_string(PredefinedName::SET);
        if self.peek_property_name(0) {
            let property_name = required(self.eat_object_literal_property_name())?;
            let parameter = self.parse_setter_parameter_list()?;
            let body = self.parse_function_body()?;
            self.record_feature_used(Feature::SETTER);
            Ok(SetAccessorTree::new(
                self.get_tree_location(element_info.start),
                property_name,
                element_info.is_class_member,
                element_info.is_static,
                parameter,
                body,
            ))
        } else {
            let property = self.parse_computed_property_name()?;
            let parameter = self.parse_setter_parameter_list()?;
            let body = self.parse_function_body()?;
            self.record_feature_used(Feature::SETTER);
            Ok(ComputedPropertySetterTree::new(
                self.get_tree_location(element_info.start),
                property,
                element_info.is_static,
                parameter,
                body,
            ))
        }
    }
    // port: Parser#parseObjectLiteralPropertyNameAssignment
    fn parse_object_literal_property_name_assignment(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let name = required(self.eat_object_literal_property_name())?;
        let colon = self.eat_opt(TokenType::COLON);
        if colon.is_none() {
            if name.type_ != TokenType::IDENTIFIER {
                let token = self.peek_token();
                self.report_expected_error(Some(&token), TokenType::COLON.to_string_utf16());
            } else if name.is_keyword() {
                self.report_error_token(
                    Some(&name),
                    "Cannot use keyword in short object literal",
                    &[],
                );
            } else if self.peek(TokenType::EQUAL) {
                let id_tree =
                    IdentifierExpressionTree::new(self.get_tree_location(start.clone()), name);
                self.eat(TokenType::EQUAL);
                let default_value = self.parse_assignment_expression()?;
                return Ok(DefaultParameterTree::new(
                    self.get_tree_location(start),
                    id_tree,
                    default_value,
                ));
            }
        }
        let value = if colon.is_none() {
            None
        } else {
            Some(self.parse_assignment_expression()?)
        };
        Ok(PropertyNameAssignmentTree::new(
            self.get_tree_location(start),
            name,
            value,
        ))
    }
    // 12.2 Primary Expression
    //   CoverParenthesizedExpressionAndArrowParameterList ::=
    //     ( Expression )
    //     ( Expression, )
    //     ( )
    //     ( ... BindingIdentifier )
    //     ( Expression , ... BindingIdentifier )
    // port: Parser#parseCoverParenthesizedExpressionAndArrowParameterList
    fn parse_cover_parenthesized_expression_and_arrow_parameter_list(&mut self) -> P<Tree> {
        if self.peek_type_at(1) == TokenType::FOR {
            return self.parse_generator_comprehension();
        }
        let start = self.get_tree_start_location();
        self.eat(TokenType::OPEN_PAREN);
        // Case ( )
        if self.peek(TokenType::CLOSE_PAREN) {
            self.eat(TokenType::CLOSE_PAREN);
            if self.peek(TokenType::ARROW) {
                return Ok(FormalParameterListTree::new(
                    self.get_tree_location(start),
                    vec![],
                    // hasTrailingComma=
                    false,
                    vec![],
                ));
            }
            self.report_error("invalid parenthesized expression", &[]);
            return Ok(MissingPrimaryExpressionTree::new(
                self.get_tree_location(start),
            ));
        }
        // Case ( ... BindingIdentifier )
        if self.peek(TokenType::ELLIPSIS) {
            let params = vec![self.parse_parameter()?];
            self.eat(TokenType::CLOSE_PAREN);
            if self.peek(TokenType::ARROW) {
                return Ok(FormalParameterListTree::new(
                    self.get_tree_location(start),
                    params,
                    // hasTrailingComma=
                    false,
                    vec![],
                ));
            }
            self.report_error("invalid parenthesized expression", &[]);
            return Ok(MissingPrimaryExpressionTree::new(
                self.get_tree_location(start),
            ));
        }
        // For either of the three remaining cases:
        //     ( Expression )
        //     ( Expression, )
        //     ( Expression, ...BindingIdentifier )
        // we can parse as an expression.
        let mut result = self.parse_expression()?;
        // If it follows with a comma, we must be in either of two cases
        //     ( Expression, )
        //     ( Expression, ...BindingIdentifier )
        // case.
        if self.peek(TokenType::COMMA) {
            if self.peek_at(1, TokenType::CLOSE_PAREN) {
                // Create the formal parameter list here so we can record
                // the trailing comma
                self.reset_scanner(start);
                // If we fail to parse as an ArrowFunction parameter list then
                // parseFormalParameterList will take care of reporting errors.
                return self.parse_formal_parameter_list();
            }
            self.eat(TokenType::COMMA);
            // Since we already parsed as an expression, we will guaranteed reparse this expression
            // as an arrow function parameter list, but just leave it as a comma expression for now.
            let location = self.get_tree_location(start.clone());
            let parameter = self.parse_parameter()?;
            result = CommaExpressionTree::new(location, vec![result, parameter]);
        }
        self.eat(TokenType::CLOSE_PAREN);
        Ok(ParenExpressionTree::new(
            self.get_tree_location(start),
            result,
        ))
    }
    // port: Parser#parseMissingPrimaryExpression
    fn parse_missing_primary_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.next_token();
        self.report_error("primary expression expected", &[]);
        Ok(MissingPrimaryExpressionTree::new(
            self.get_tree_location(start),
        ))
    }
    // 11.14 Expressions
    // port: Parser#parseExpressionNoIn
    fn parse_expression_no_in(&mut self) -> P<Tree> {
        self.parse(Expression::NO_IN)
    }
    // port: Parser#parseExpression
    fn parse_expression(&mut self) -> P<Tree> {
        self.parse(Expression::NORMAL)
    }
    // port: Parser#peekExpression
    fn peek_expression(&mut self) -> bool {
        match self.peek_type() {
            TokenType::BANG
            | TokenType::CLASS
            | TokenType::DELETE
            | TokenType::FALSE
            | TokenType::FUNCTION
            | TokenType::IDENTIFIER
            | TokenType::TYPE
            | TokenType::DECLARE
            | TokenType::MODULE
            | TokenType::NAMESPACE
            | TokenType::MINUS
            | TokenType::MINUS_MINUS
            | TokenType::NEW
            | TokenType::NULL
            | TokenType::NUMBER
            | TokenType::BIGINT
            | TokenType::OPEN_CURLY
            | TokenType::OPEN_PAREN
            | TokenType::OPEN_SQUARE
            | TokenType::PLUS
            | TokenType::PLUS_PLUS
            | TokenType::SLASH // regular expression literal
            | TokenType::SLASH_EQUAL // regular expression literal
            | TokenType::STRING
            | TokenType::NO_SUBSTITUTION_TEMPLATE
            | TokenType::TEMPLATE_HEAD
            | TokenType::SUPER
            | TokenType::THIS
            | TokenType::TILDE
            | TokenType::TRUE
            | TokenType::TYPEOF
            | TokenType::VOID
            | TokenType::YIELD => true,
            TokenType::IMPORT => self.peek_import_call() || self.peek_import_dot(),
            _ => false,
        }
    }
    // port: Parser#parse
    fn parse(&mut self, expression_in: Expression) -> P<Tree> {
        let start = self.get_tree_start_location();
        let result = self.parse_assignment(expression_in)?;
        if self.peek(TokenType::COMMA)
            && !self.peek_at(1, TokenType::ELLIPSIS)
            && !self.peek_at(1, TokenType::CLOSE_PAREN)
        {
            let mut exprs = vec![result];
            while self.peek(TokenType::COMMA)
                && !self.peek_at(1, TokenType::ELLIPSIS)
                && !self.peek_at(1, TokenType::CLOSE_PAREN)
            {
                self.eat(TokenType::COMMA);
                exprs.push(self.parse_assignment(expression_in)?);
            }
            return Ok(CommaExpressionTree::new(
                self.get_tree_location(start),
                exprs,
            ));
        }
        Ok(result)
    }
    // 12.14 Assignment operators
    // port: Parser#parseAssignmentExpression
    fn parse_assignment_expression(&mut self) -> P<Tree> {
        self.parse_assignment(Expression::NORMAL)
    }
    // port: Parser#peekAssignmentExpression
    fn peek_assignment_expression(&mut self) -> bool {
        self.peek_expression()
    }
    // port: Parser#parseAssignment
    fn parse_assignment(&mut self, expression_in: Expression) -> P<Tree> {
        if self.peek(TokenType::YIELD) && self.in_generator_context() {
            return self.parse_yield(expression_in);
        }
        let start = self.get_tree_start_location();
        let mut left = self.parse_conditional(expression_in)?;
        if self.is_start_of_async_arrow_function(&left)? {
            // re-evaluate as an async arrow function.
            self.reset_scanner_tree(&left);
            return self.parse_async_arrow_function(expression_in);
        }
        if self.peek(TokenType::ARROW) {
            return self.complete_assignment_expression_parse_at_arrow(left, expression_in);
        }
        if self.peek_assignment_operator() {
            // not the vanilla assignment operator `=`, but a special equals operator (`+=`, `-=`,
            // `**=`, etc)
            if !self.peek(TokenType::EQUAL) && !left.is_valid_non_vanilla_assignment_target() {
                self.report_error("invalid assignment target", &[]);
                let start = self.get_tree_start_location();
                return Ok(MissingPrimaryExpressionTree::new(
                    self.get_tree_location(start),
                ));
            }
            left = self.transform_left_hand_side_expression(left)?;
            if !left.is_valid_assignment_target() {
                self.report_error("invalid assignment target", &[]);
                let start = self.get_tree_start_location();
                return Ok(MissingPrimaryExpressionTree::new(
                    self.get_tree_location(start),
                ));
            }
            let operator = self.next_token();
            let right = self.parse_assignment(expression_in)?;
            return Ok(BinaryOperatorTree::new(
                self.get_tree_location(start),
                left,
                operator,
                right,
            ));
        }
        Ok(left)
    }
    // port: Parser#isStartOfAsyncArrowFunction
    fn is_start_of_async_arrow_function(&mut self, partial_expression: &Tree) -> P<bool> {
        if partial_expression.type_ == ParseTreeType::IDENTIFIER_EXPRESSION {
            let identifier_token = required(
                partial_expression
                    .as_identifier_expression()
                    .identifier_token
                    .as_ref(),
            )?;
            // partialExpression is `async`
            // followed by `[no newline] bindingIdentifier [no newline] =>`
            Ok(identifier_token.value_equals(&ASYNC)
                && !self.peek_implicit_semi_colon_at(0)
                && self.peek_id()
                && !self.peek_implicit_semi_colon_at(1)
                && self.peek_at(1, TokenType::ARROW))
        } else if partial_expression.type_ == ParseTreeType::CALL_EXPRESSION {
            let call_expression = partial_expression.as_call_expression();
            let callee = &call_expression.operand;
            let arguments = &call_expression.arguments;
            // partialExpression is `async [no newline] (parameters)`
            // followed by `[no newline] =>`
            Ok(callee.type_ == ParseTreeType::IDENTIFIER_EXPRESSION
                && required(callee.as_identifier_expression().identifier_token.as_ref())?
                    .value_equals(&ASYNC)
                && callee.location.end.line == arguments.location.start.line
                && !self.peek_implicit_semi_colon()
                && self.peek(TokenType::ARROW))
        } else {
            Ok(false)
        }
    }
    // port: Parser#completeAssignmentExpressionParseAtArrow
    fn complete_assignment_expression_parse_at_arrow(
        &mut self,
        left_of_arrow: Tree,
        expression_in: Expression,
    ) -> P<Tree> {
        if left_of_arrow.type_ == ParseTreeType::CALL_EXPRESSION {
            //   ... someAssignmentExpression // implicit semicolon
            //   (args) =>
            self.complete_assignment_expression_parse_at_arrow_call(left_of_arrow)
        } else {
            self.complete_arrow_function_parse_at_arrow(left_of_arrow, expression_in)
        }
    }
    // port: Parser#completeArrowFunctionParseAtArrow
    fn complete_arrow_function_parse_at_arrow(
        &mut self,
        left_of_arrow: Tree,
        expression_in: Expression,
    ) -> P<Tree> {
        let arrow_formal_parameters = self.transform_to_arrow_formal_parameters(left_of_arrow)?;
        if self.peek_implicit_semi_colon() {
            self.report_error("No newline allowed before '=>'", &[]);
        }
        self.eat(TokenType::ARROW);
        let arrow_function_body =
            self.parse_arrow_function_body(expression_in, FunctionFlavor::NORMAL)?;
        let mut builder = FunctionDeclarationTree::builder(Kind::ARROW);
        builder
            .set_formal_parameter_list(arrow_formal_parameters.clone())
            .set_function_body(arrow_function_body);
        Ok(builder.build(self.get_tree_location(arrow_formal_parameters.location.start.clone())))
    }
    // port: Parser#transformToArrowFormalParameters
    fn transform_to_arrow_formal_parameters(&mut self, left_of_arrow: Tree) -> P<Tree> {
        match left_of_arrow.type_ {
            ParseTreeType::FORMAL_PARAMETER_LIST => Ok(left_of_arrow),
            // e.g. x => x + 1
            ParseTreeType::IDENTIFIER_EXPRESSION => Ok(FormalParameterListTree::new(
                left_of_arrow.location.clone(),
                vec![left_of_arrow],
                // hasTrailingComma=
                false,
                vec![],
            )),
            ParseTreeType::ARGUMENT_LIST | ParseTreeType::PAREN_EXPRESSION => {
                // e.g. (x) => x + 1
                self.reset_scanner_tree(&left_of_arrow);
                // If we fail to parse as an ArrowFunction parameter list then
                // parseFormalParameterList will take care of reporting errors.
                self.parse_formal_parameter_list()
            }
            _ => {
                self.report_error_tree(
                    Some(&left_of_arrow),
                    "invalid arrow function parameters",
                    &[],
                );
                Ok(Self::new_empty_formal_parameter_list(
                    left_of_arrow.location.clone(),
                ))
            }
        }
    }
    // port: Parser#completeAssignmentExpressionParseAtArrow
    fn complete_assignment_expression_parse_at_arrow_call(
        &mut self,
        call_expression: Tree,
    ) -> P<Tree> {
        let call = call_expression.as_call_expression();
        let operand = &call.operand;
        let arguments = &call.arguments;
        if operand.location.end.line < arguments.location.start.line {
            // break at the implicit semicolon
            // Example:
            // foo.bar // operand and implicit semicolon
            // () => { doSomething; };
            self.reset_scanner_after(operand);
            Ok(operand.clone())
        } else {
            self.report_error("'=>' unexpected", &[]);
            Ok(call_expression)
        }
    }
    // port: Parser#parseAsyncArrowFunction
    fn parse_async_arrow_function(&mut self, expression_in: Expression) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat_predefined_string(&ASYNC);
        if self.peek_implicit_semi_colon() {
            self.report_error(
                "No newline allowed between `async` and arrow function parameter list",
                &[],
            );
        }
        let arrow_parameter_list = if self.peek(TokenType::OPEN_PAREN) {
            // async (...) =>
            self.parse_formal_parameter_list()?
        } else {
            // async arg =>
            let single_parameter = self.parse_identifier_expression()?;
            FormalParameterListTree::new(
                single_parameter.location.clone(),
                vec![single_parameter],
                // hasTrailingComma=
                false,
                vec![],
            )
        };
        if self.peek_implicit_semi_colon() {
            self.report_error("No newline allowed before '=>'", &[]);
        }
        self.eat(TokenType::ARROW);
        let arrow_function_body =
            self.parse_arrow_function_body(expression_in, FunctionFlavor::ASYNCHRONOUS)?;
        let mut builder = FunctionDeclarationTree::builder(Kind::ARROW);
        builder
            .set_async(true)
            .set_formal_parameter_list(arrow_parameter_list)
            .set_function_body(arrow_function_body);
        Ok(builder.build(self.get_tree_location(start)))
    }
    // port: Parser#parseArrowFunctionBody
    fn parse_arrow_function_body(
        &mut self,
        expression_in: Expression,
        function_flavor: FunctionFlavor,
    ) -> P<Tree> {
        self.function_context_stack.push(function_flavor);
        let arrow_function_body = if self.peek(TokenType::OPEN_CURLY) {
            self.parse_function_body()?
        } else {
            self.parse_assignment(expression_in)?
        };
        self.function_context_stack.pop();
        Ok(arrow_function_body)
    }
    // port: Parser#newEmptyFormalParameterList
    fn new_empty_formal_parameter_list(location: SourceRange) -> Tree {
        FormalParameterListTree::new(location, vec![], /* hasTrailingComma= */ false, vec![])
    }
    // Transforms a LeftHandSideExpression into a LeftHandSidePattern if possible. This returns the
    // transformed tree if it parses as a LeftHandSidePattern, otherwise it returns the original tree.
    // port: Parser#transformLeftHandSideExpression
    fn transform_left_hand_side_expression(&mut self, tree: Tree) -> P<Tree> {
        match tree.type_ {
            ParseTreeType::ARRAY_LITERAL_EXPRESSION | ParseTreeType::OBJECT_LITERAL_EXPRESSION => {
                self.reset_scanner_tree(&tree);
                // If we fail to parse as an LeftHandSidePattern then
                // parseLeftHandSidePattern will take care reporting errors.
                self.parse_left_hand_side_pattern()
            }
            _ => Ok(tree),
        }
    }
    // port: Parser#parseLeftHandSidePattern
    fn parse_left_hand_side_pattern(&mut self) -> P<Tree> {
        self.parse_pattern(PatternKind::ANY)
    }
    // port: Parser#resetScanner
    fn reset_scanner(&mut self, start: SourcePosition) {
        // TODO(bradfordcsmith): lastSourcePosition should really point to the end of the last token
        //     before the tree to correctly detect implicit semicolons, but it doesn't matter for the
        //     current use case.
        self.last_source_position = start;
        self.scanner.set_position(self.last_source_position.clone());
    }
    // port: Parser#resetScanner
    fn reset_scanner_tree(&mut self, tree: &Tree) {
        self.scanner.set_position(tree.location.start.clone());
    }
    // port: Parser#resetScannerAfter
    fn reset_scanner_after(&mut self, parse_tree: &Tree) {
        self.last_source_position = parse_tree.location.end.clone();
        // NOTE: The "end" position for a parseTree actually points to the first character after the
        //     last token in the tree, so this is not an off-by-one error.
        self.scanner.set_position(self.last_source_position.clone());
    }
    // port: Parser#peekAssignmentOperator
    fn peek_assignment_operator(&mut self) -> bool {
        matches!(
            self.peek_type(),
            TokenType::EQUAL
                | TokenType::STAR_EQUAL
                | TokenType::STAR_STAR_EQUAL
                | TokenType::SLASH_EQUAL
                | TokenType::PERCENT_EQUAL
                | TokenType::PLUS_EQUAL
                | TokenType::MINUS_EQUAL
                | TokenType::LEFT_SHIFT_EQUAL
                | TokenType::RIGHT_SHIFT_EQUAL
                | TokenType::UNSIGNED_RIGHT_SHIFT_EQUAL
                | TokenType::AMPERSAND_EQUAL
                | TokenType::CARET_EQUAL
                | TokenType::BAR_EQUAL
                | TokenType::OR_EQUAL
                | TokenType::AND_EQUAL
                | TokenType::QUESTION_QUESTION_EQUAL
        )
    }
    // port: Parser#inGeneratorContext
    fn in_generator_context(&self) -> bool {
        // disallow yield outside of generators
        self.function_context_stack.last().unwrap().is_generator()
    }
    // yield [no line terminator] (*)? AssignExpression
    // https://people.mozilla.org/~jorendorff/es6-draft.html#sec-generator-function-definitions-runtime-semantics-evaluation
    // port: Parser#parseYield
    fn parse_yield(&mut self, expression_in: Expression) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::YIELD);
        let mut is_yield_all = false;
        let mut expression = None;
        if !self.peek_implicit_semi_colon() {
            is_yield_all = self.eat_opt(TokenType::STAR).is_some();
            if self.peek_assignment_expression() {
                expression = Some(self.parse_assignment(expression_in)?);
            } else if is_yield_all {
                self.report_error("yield* requires an expression", &[]);
            }
        }
        Ok(YieldExpressionTree::new(
            self.get_tree_location(start),
            is_yield_all,
            expression,
        ))
    }
    // 11.12 Conditional Expression
    // port: Parser#parseConditional
    fn parse_conditional(&mut self, expression_in: Expression) -> P<Tree> {
        let start = self.get_tree_start_location();
        let condition = self.parse_short_circuit(expression_in)?;
        if self.peek(TokenType::QUESTION) {
            self.eat(TokenType::QUESTION);
            let left = self.parse_assignment(expression_in)?;
            self.eat(TokenType::COLON);
            let right = self.parse_assignment(expression_in)?;
            return Ok(ConditionalExpressionTree::new(
                self.get_tree_location(start),
                condition,
                left,
                right,
            ));
        }
        Ok(condition)
    }
    // port: Parser#parseShortCircuit
    fn parse_short_circuit(&mut self, expression_in: Expression) -> P<Tree> {
        let start = self.get_tree_start_location();
        let left = self.parse_logical_or(expression_in)?;
        if self.peek(TokenType::QUESTION_QUESTION) {
            if left.type_ == ParseTreeType::BINARY_OPERATOR {
                let binary_tree = left.as_binary_operator();
                if binary_tree.operator.type_ == TokenType::AND
                    || binary_tree.operator.type_ == TokenType::OR
                {
                    self.report_error(
                        "Logical OR and logical AND require parentheses when used with '??'",
                        &[],
                    );
                }
            }
            self.parse_nullish_coalesce(expression_in, left, start)
        } else {
            Ok(left)
        }
    }
    // port: Parser#parseNullishCoalesce
    fn parse_nullish_coalesce(
        &mut self,
        expression_in: Expression,
        mut left: Tree,
        start: SourcePosition,
    ) -> P<Tree> {
        while self.peek(TokenType::QUESTION_QUESTION) {
            let operator = required(self.eat(TokenType::QUESTION_QUESTION))?;
            let right = self.parse_bitwise_or(expression_in)?;
            left = BinaryOperatorTree::new(
                self.get_tree_location(start.clone()),
                left,
                operator,
                right,
            );
        }
        if self.peek(TokenType::AND) || self.peek(TokenType::OR) {
            self.report_error(
                "Logical OR and logical AND require parentheses when used with '??'",
                &[],
            );
        }
        Ok(left)
    }
    // 11.11 Logical OR
    // port: Parser#parseLogicalOR
    fn parse_logical_or(&mut self, expression_in: Expression) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut left = self.parse_logical_and(expression_in)?;
        while self.peek(TokenType::OR) {
            let operator = required(self.eat(TokenType::OR))?;
            let right = self.parse_logical_and(expression_in)?;
            left = BinaryOperatorTree::new(
                self.get_tree_location(start.clone()),
                left,
                operator,
                right,
            );
        }
        Ok(left)
    }
    // 11.11 Logical AND
    // port: Parser#parseLogicalAND
    fn parse_logical_and(&mut self, expression_in: Expression) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut left = self.parse_bitwise_or(expression_in)?;
        while self.peek(TokenType::AND) {
            let operator = required(self.eat(TokenType::AND))?;
            let right = self.parse_bitwise_or(expression_in)?;
            left = BinaryOperatorTree::new(
                self.get_tree_location(start.clone()),
                left,
                operator,
                right,
            );
        }
        Ok(left)
    }
    // 11.10 Bitwise OR
    // port: Parser#parseBitwiseOR
    fn parse_bitwise_or(&mut self, expression_in: Expression) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut left = self.parse_bitwise_xor(expression_in)?;
        while self.peek(TokenType::BAR) {
            let operator = required(self.eat(TokenType::BAR))?;
            let right = self.parse_bitwise_xor(expression_in)?;
            left = BinaryOperatorTree::new(
                self.get_tree_location(start.clone()),
                left,
                operator,
                right,
            );
        }
        Ok(left)
    }
    // 11.10 Bitwise XOR
    // port: Parser#parseBitwiseXOR
    fn parse_bitwise_xor(&mut self, expression_in: Expression) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut left = self.parse_bitwise_and(expression_in)?;
        while self.peek(TokenType::CARET) {
            let operator = required(self.eat(TokenType::CARET))?;
            let right = self.parse_bitwise_and(expression_in)?;
            left = BinaryOperatorTree::new(
                self.get_tree_location(start.clone()),
                left,
                operator,
                right,
            );
        }
        Ok(left)
    }
    // 11.10 Bitwise AND
    // port: Parser#parseBitwiseAND
    fn parse_bitwise_and(&mut self, expression_in: Expression) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut left = self.parse_equality(expression_in)?;
        while self.peek(TokenType::AMPERSAND) {
            let operator = required(self.eat(TokenType::AMPERSAND))?;
            let right = self.parse_equality(expression_in)?;
            left = BinaryOperatorTree::new(
                self.get_tree_location(start.clone()),
                left,
                operator,
                right,
            );
        }
        Ok(left)
    }
    // 11.9 Equality Expression
    // port: Parser#parseEquality
    fn parse_equality(&mut self, expression_in: Expression) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut left = self.parse_relational(expression_in)?;
        while self.peek_equality_operator() {
            let operator = self.next_token();
            let right = self.parse_relational(expression_in)?;
            left = BinaryOperatorTree::new(
                self.get_tree_location(start.clone()),
                left,
                operator,
                right,
            );
        }
        Ok(left)
    }
    // port: Parser#peekEqualityOperator
    fn peek_equality_operator(&mut self) -> bool {
        matches!(
            self.peek_type(),
            TokenType::EQUAL_EQUAL
                | TokenType::NOT_EQUAL
                | TokenType::EQUAL_EQUAL_EQUAL
                | TokenType::NOT_EQUAL_EQUAL
        )
    }
    // 11.8 Relational
    // port: Parser#parseRelational
    fn parse_relational(&mut self, expression_in: Expression) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut left = self.parse_shift_expression()?;
        while self.peek_relational_operator(expression_in) {
            let operator = self.next_token();
            let right = self.parse_shift_expression()?;
            left = BinaryOperatorTree::new(
                self.get_tree_location(start.clone()),
                left,
                operator,
                right,
            );
        }
        Ok(left)
    }
    // port: Parser#peekRelationalOperator
    fn peek_relational_operator(&mut self, expression_in: Expression) -> bool {
        match self.peek_type() {
            TokenType::OPEN_ANGLE
            | TokenType::CLOSE_ANGLE
            | TokenType::GREATER_EQUAL
            | TokenType::LESS_EQUAL
            | TokenType::INSTANCEOF => true,
            TokenType::IN => expression_in == Expression::NORMAL,
            _ => false,
        }
    }
    // 11.7 Shift Expression
    // port: Parser#parseShiftExpression
    fn parse_shift_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut left = self.parse_additive_expression()?;
        while self.peek_shift_operator() {
            let operator = self.next_token();
            let right = self.parse_additive_expression()?;
            left = BinaryOperatorTree::new(
                self.get_tree_location(start.clone()),
                left,
                operator,
                right,
            );
        }
        Ok(left)
    }
    // port: Parser#peekShiftOperator
    fn peek_shift_operator(&mut self) -> bool {
        matches!(
            self.peek_type(),
            TokenType::LEFT_SHIFT | TokenType::RIGHT_SHIFT | TokenType::UNSIGNED_RIGHT_SHIFT
        )
    }
    // 11.6 Additive Expression
    // port: Parser#parseAdditiveExpression
    fn parse_additive_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut left = self.parse_multiplicative_expression()?;
        while self.peek_additive_operator() {
            let operator = self.next_token();
            let right = self.parse_multiplicative_expression()?;
            left = BinaryOperatorTree::new(
                self.get_tree_location(start.clone()),
                left,
                operator,
                right,
            );
        }
        Ok(left)
    }
    // port: Parser#peekAdditiveOperator
    fn peek_additive_operator(&mut self) -> bool {
        matches!(self.peek_type(), TokenType::PLUS | TokenType::MINUS)
    }
    // 11.5 Multiplicative Expression
    // port: Parser#parseMultiplicativeExpression
    fn parse_multiplicative_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut left = self.parse_exponentiation_expression()?;
        while self.peek_multiplicative_operator() {
            let operator = self.next_token();
            let right = self.parse_exponentiation_expression()?;
            left = BinaryOperatorTree::new(
                self.get_tree_location(start.clone()),
                left,
                operator,
                right,
            );
        }
        Ok(left)
    }
    // port: Parser#peekMultiplicativeOperator
    fn peek_multiplicative_operator(&mut self) -> bool {
        matches!(
            self.peek_type(),
            TokenType::STAR | TokenType::SLASH | TokenType::PERCENT
        )
    }
    // port: Parser#parseExponentiationExpression
    fn parse_exponentiation_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let left = self.parse_unary_expression()?;
        if self.peek(TokenType::STAR_STAR) {
            // ExponentiationExpression does not allow a UnaryExpression before '**'.
            // Parentheses are required to disambiguate:
            //   (-x)**y is valid
            //   -(x**y) is valid
            //   -x**y is a syntax error
            if left.type_ == ParseTreeType::UNARY_EXPRESSION {
                self.report_error(
                    "Unary operator '%s' requires parentheses before '**'",
                    &[left.as_unary_expression().operator.to_string_utf16()],
                );
            } else if left.type_ == ParseTreeType::AWAIT_EXPRESSION {
                self.report_error(
                    "Unary operator 'await' requires parentheses before '**'",
                    &[],
                );
            }
            let operator = self.next_token();
            let right = self.parse_exponentiation_expression()?;
            Ok(BinaryOperatorTree::new(
                self.get_tree_location(start),
                left,
                operator,
                right,
            ))
        } else {
            Ok(left)
        }
    }
    // 11.4 Unary Operator
    // port: Parser#parseUnaryExpression
    fn parse_unary_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        if self.peek_unary_operator() {
            let operator = self.next_token();
            let operand = self.parse_unary_expression()?;
            Ok(UnaryExpressionTree::new(
                self.get_tree_location(start),
                operator,
                operand,
            ))
        } else if self.peek_await_expression() {
            self.parse_await_expression()
        } else {
            self.parse_update_expression()
        }
    }
    // port: Parser#peekUnaryOperator
    fn peek_unary_operator(&mut self) -> bool {
        matches!(
            self.peek_type(),
            TokenType::DELETE
                | TokenType::VOID
                | TokenType::TYPEOF
                | TokenType::PLUS
                | TokenType::MINUS
                | TokenType::TILDE
                | TokenType::BANG
        )
    }
    // port: Parser#peekAwaitExpression
    fn peek_await_expression(&mut self) -> bool {
        self.peek_predefined_string(&AWAIT)
    }
    // port: Parser#parseAwaitExpression
    fn parse_await_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat_predefined_string(&AWAIT);
        let expression = self.parse_unary_expression()?;
        Ok(AwaitExpressionTree::new(
            self.get_tree_location(start),
            expression,
        ))
    }
    // port: Parser#parseUpdateExpression
    fn parse_update_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        if self.peek_update_operator() {
            let operator = self.next_token();
            let operand = self.parse_unary_expression()?;
            Ok(UpdateExpressionTree::prefix(
                self.get_tree_location(start),
                operator,
                operand,
            ))
        } else {
            let lhs = self.parse_left_hand_side_expression()?;
            if self.peek_update_operator() && !self.peek_implicit_semi_colon() {
                // newline not allowed before an update operator.
                let operator = self.next_token();
                Ok(UpdateExpressionTree::postfix(
                    self.get_tree_location(start),
                    operator,
                    lhs,
                ))
            } else {
                Ok(lhs)
            }
        }
    }
    // port: Parser#peekUpdateOperator
    fn peek_update_operator(&mut self) -> bool {
        matches!(
            self.peek_type(),
            TokenType::PLUS_PLUS | TokenType::MINUS_MINUS
        )
    }
    // port: Parser#peekImportCall
    fn peek_import_call(&mut self) -> bool {
        self.peek(TokenType::IMPORT) && self.peek_at(1, TokenType::OPEN_PAREN)
    }
    // port: Parser#peekImportDot
    fn peek_import_dot(&mut self) -> bool {
        self.peek(TokenType::IMPORT) && self.peek_at(1, TokenType::PERIOD)
    }
    // Parse LeftHandSideExpression.
    // port: Parser#parseLeftHandSideExpression
    fn parse_left_hand_side_expression(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        // We have these possible productions.
        // LeftHandSideExpression -> NewExpression
        //                        -> CallExpression
        //                        -> MemberExpression
        //                        -> OptionalExpression
        //
        // NewExpression -> new NewExpression
        //               -> MemberExpression
        //
        // CallExpression -> MemberExpression Arguments
        //                -> CallExpression ... see below
        //
        // OptionalExpression -> MemberExpression OptionalChain
        //                    -> CallExpression OptionalChain
        //                    -> OptionalExpression OptionalChain
        //
        // We try parsing a NewExpression, here, because that will include parsing MemberExpression.
        // If what we really have is a CallExpression or OptionalExpression, then the MemberExpression
        // we get back from parseNewExpression will be the first part of it, and we'll build the
        // rest later.
        let mut operand = self.parse_new_expression()?;
        // this test is equivalent to is member expression
        if operand.type_ != ParseTreeType::NEW_EXPRESSION
            || operand.as_new_expression().arguments.is_some()
        {
            // We have a MemberExpression, but it may actually be just the first part of a CallExpression
            // Attempt to gather the rest of the CallExpression, if so.
            while self.peek_call_suffix() {
                match self.peek_type() {
                    TokenType::OPEN_PAREN => {
                        let arguments = self.parse_arguments()?;
                        operand = CallExpressionTree::new(
                            self.get_tree_location(start.clone()),
                            operand,
                            arguments,
                        );
                    }
                    TokenType::OPEN_SQUARE => {
                        self.eat(TokenType::OPEN_SQUARE);
                        let member = self.parse_expression()?;
                        self.eat(TokenType::CLOSE_SQUARE);
                        operand = MemberLookupExpressionTree::new(
                            self.get_tree_location(start.clone()),
                            operand,
                            member,
                        );
                    }
                    TokenType::PERIOD => {
                        self.eat(TokenType::PERIOD);
                        let id = self.eat_id_or_keyword_as_id();
                        operand = MemberExpressionTree::new(
                            self.get_tree_location(start.clone()),
                            operand,
                            id,
                        );
                    }
                    TokenType::NO_SUBSTITUTION_TEMPLATE | TokenType::TEMPLATE_HEAD => {
                        operand = self.parse_template_literal(Some(operand))?;
                    }
                    _ => {
                        let type_ = self.peek_type();
                        panic!("unexpected case: {}", type_);
                    }
                }
            }
            operand = self.maybe_parse_optional_expression(operand)?;
        }
        Ok(operand)
    }
    // port: Parser#peekCallSuffix
    fn peek_call_suffix(&mut self) -> bool {
        self.peek(TokenType::OPEN_PAREN)
            || self.peek(TokenType::OPEN_SQUARE)
            || self.peek(TokenType::PERIOD)
            || self.peek(TokenType::NO_SUBSTITUTION_TEMPLATE)
            || self.peek(TokenType::TEMPLATE_HEAD)
    }
    // Tries to parse the expression as an optional expression.
    //
    // <p>`operand?.identifier` or `operand?.[expression]` or `operand?.(arg1, arg2)`
    //
    // <p>returns parse tree after trying to parse it as an optional expression
    // port: Parser#maybeParseOptionalExpression
    fn maybe_parse_optional_expression(&mut self, mut operand: Tree) -> P<Tree> {
        // The optional chain's source info should cover the lhs operand also
        let start = operand.location.start.clone();
        while self.peek(TokenType::QUESTION_DOT) {
            self.eat(TokenType::QUESTION_DOT);
            match self.peek_type() {
                TokenType::OPEN_PAREN => {
                    let arguments = self.parse_arguments()?;
                    let has_trailing_comma = arguments.as_argument_list().has_trailing_comma;
                    operand = OptChainCallExpressionTree::new(
                        self.get_tree_location(start.clone()),
                        operand,
                        arguments,
                        // isStartOfOptionalChain=
                        true,
                        has_trailing_comma,
                    );
                }
                TokenType::OPEN_SQUARE => {
                    self.eat(TokenType::OPEN_SQUARE);
                    let member = self.parse_expression()?;
                    self.eat(TokenType::CLOSE_SQUARE);
                    operand = OptionalMemberLookupExpressionTree::new(
                        self.get_tree_location(start.clone()),
                        operand,
                        member,
                        // isStartOfOptionalChain=
                        true,
                    );
                }
                TokenType::NO_SUBSTITUTION_TEMPLATE | TokenType::TEMPLATE_HEAD => {
                    self.report_error(
                        "template literal cannot be used within optional chaining",
                        &[],
                    );
                }
                _ => {
                    if self.peek_id_or_keyword() {
                        let id = self.eat_id_or_keyword_as_id();
                        operand = OptionalMemberExpressionTree::new(
                            self.get_tree_location(start.clone()),
                            operand,
                            id,
                            // isStartOfOptionalChain=
                            true,
                        );
                    } else {
                        let type_ = self.peek_type();
                        self.report_error(
                            "syntax error: %s not allowed in optional chain",
                            &[type_.to_string_utf16()],
                        );
                    }
                }
            }
            operand = self.parse_remaining_optional_chain_segment(operand)?;
        }
        Ok(operand)
    }
    // Parses the remaining components of an optional chain till the current chain's end, or a new
    // chain's start.
    //
    // <p>`optionalExpression.identifier`, `optionalExpression[expression]`, `optionalExpression(arg1,
    // arg2)`, or `optionalExpression?.optionalExpression`
    //
    // <p>returns parse tree after trying to parse it as an optional chain
    // port: Parser#parseRemainingOptionalChainSegment
    fn parse_remaining_optional_chain_segment(&mut self, mut optional_expression: Tree) -> P<Tree> {
        // The optional chain's source info should cover the lhs operand also
        let start = optional_expression.location.start.clone();
        while self.peek_optional_chain_suffix() {
            if self.peek_type() == TokenType::NO_SUBSTITUTION_TEMPLATE
                || self.peek_type() == TokenType::TEMPLATE_HEAD
            {
                self.report_error(
                    "template literal cannot be used within optional chaining",
                    &[],
                );
                break;
            }
            match self.peek_type() {
                TokenType::PERIOD => {
                    self.eat(TokenType::PERIOD);
                    let id = self.eat_id_or_keyword_as_id();
                    optional_expression = OptionalMemberExpressionTree::new(
                        self.get_tree_location(start.clone()),
                        optional_expression,
                        id,
                        // isStartOfOptionalChain=
                        false,
                    );
                }
                TokenType::OPEN_PAREN => {
                    let arguments = self.parse_arguments()?;
                    let has_trailing_comma = arguments.as_argument_list().has_trailing_comma;
                    optional_expression = OptChainCallExpressionTree::new(
                        self.get_tree_location(start.clone()),
                        optional_expression,
                        arguments,
                        // isStartOfOptionalChain=
                        false,
                        has_trailing_comma,
                    );
                }
                TokenType::OPEN_SQUARE => {
                    self.eat(TokenType::OPEN_SQUARE);
                    let member = self.parse_expression()?;
                    self.eat(TokenType::CLOSE_SQUARE);
                    optional_expression = OptionalMemberLookupExpressionTree::new(
                        self.get_tree_location(start.clone()),
                        optional_expression,
                        member,
                        // isStartOfOptionalChain=
                        false,
                    );
                }
                _ => {
                    let type_ = self.peek_type();
                    panic!("unexpected case: {}", type_);
                }
            }
        }
        Ok(optional_expression)
    }
    // Tokens that indicate continuation of an optional chain.
    // port: Parser#peekOptionalChainSuffix
    fn peek_optional_chain_suffix(&mut self) -> bool {
        self.peek(TokenType::OPEN_PAREN) // a?.b( ...
            || self.peek(TokenType::OPEN_SQUARE) // a?.b[ ...
            || self.peek(TokenType::PERIOD) // a?.b. ...
            // TEMPLATE_HEAD and NO_SUBSTITUTION_TEMPLATE are actually not allowed within optional
            // chaining and leads to an early error as dictated by the spec.
            // https://tc39.es/proposal-optional-chaining/#sec-left-hand-side-expressions-static-semantics-early-errors
            || self.peek(TokenType::NO_SUBSTITUTION_TEMPLATE) // a?.b`text`
            || self.peek(TokenType::TEMPLATE_HEAD) // a?.b`text ${substitution} text`
    }
    // 11.2 Member Expression without the new production
    // port: Parser#parseMemberExpressionNoNew
    fn parse_member_expression_no_new(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut operand = if self.peek_import_dot() {
            self.parse_import_dot_meta()?
        } else if self.peek_async_function_start() {
            self.parse_async_function_expression()?
        } else if self.peek_function() {
            self.parse_function_expression()?
        } else {
            self.parse_primary_expression()?
        };
        while self.peek_member_expression_suffix() {
            match self.peek_type() {
                TokenType::OPEN_SQUARE => {
                    self.eat(TokenType::OPEN_SQUARE);
                    let member = self.parse_expression()?;
                    self.eat(TokenType::CLOSE_SQUARE);
                    operand = MemberLookupExpressionTree::new(
                        self.get_tree_location(start.clone()),
                        operand,
                        member,
                    );
                }
                TokenType::PERIOD => {
                    self.eat(TokenType::PERIOD);
                    let id = self.eat_id_or_keyword_as_id();
                    operand = MemberExpressionTree::new(
                        self.get_tree_location(start.clone()),
                        operand,
                        id,
                    );
                }
                TokenType::NO_SUBSTITUTION_TEMPLATE | TokenType::TEMPLATE_HEAD => {
                    operand = self.parse_template_literal(Some(operand))?;
                }
                _ => {
                    return Err(ParseError {
                        class: "java.lang.RuntimeException",
                        message: Some(JsString::from("unreachable")),
                    });
                }
            }
        }
        Ok(operand)
    }
    // port: Parser#peekMemberExpressionSuffix
    fn peek_member_expression_suffix(&mut self) -> bool {
        self.peek(TokenType::OPEN_SQUARE)
            || self.peek(TokenType::PERIOD)
            || self.peek(TokenType::NO_SUBSTITUTION_TEMPLATE)
            || self.peek(TokenType::TEMPLATE_HEAD)
    }
    // port: Parser#parseNewExpression
    fn parse_new_expression(&mut self) -> P<Tree> {
        if !self.peek(TokenType::NEW) {
            return self.parse_member_expression_no_new();
        } else if self.peek_at(1, TokenType::PERIOD) {
            return self.parse_new_dot_something();
        }
        let start = self.get_tree_start_location();
        self.eat(TokenType::NEW);
        if self.peek(TokenType::QUESTION_DOT) {
            // new?.target not allowed
            self.report_error(
                "Optional chaining is forbidden in `new?.target` contexts.",
                &[],
            );
        }
        let operand = self.parse_new_expression()?;
        if self.peek(TokenType::QUESTION_DOT) {
            // new a?.() not allowed
            self.report_error(
                "Optional chaining is forbidden in construction contexts.",
                &[],
            );
        }
        let mut arguments = None;
        if self.peek(TokenType::OPEN_PAREN) {
            arguments = Some(self.parse_arguments()?);
        }
        let has_trailing_comma = arguments
            .as_ref()
            .is_some_and(|arguments| arguments.as_argument_list().has_trailing_comma);
        Ok(NewExpressionTree::new(
            self.get_tree_location(start),
            operand,
            arguments,
            has_trailing_comma,
        ))
    }
    // port: Parser#parseNewDotSomething
    fn parse_new_dot_something(&mut self) -> P<Tree> {
        // currently only "target" is valid after "new."
        let start = self.get_tree_start_location();
        self.eat(TokenType::NEW);
        self.eat(TokenType::PERIOD);
        self.eat_predefined_string(&JsString::from("target"));
        Ok(NewTargetExpressionTree::new(self.get_tree_location(start)))
    }
    // port: Parser#parseImportDotMeta
    fn parse_import_dot_meta(&mut self) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::IMPORT);
        self.eat(TokenType::PERIOD);
        self.eat_predefined_string(&JsString::from("meta"));
        Ok(ImportMetaExpressionTree::new(self.get_tree_location(start)))
    }
    // port: Parser#parseArguments
    fn parse_arguments(&mut self) -> P<Tree> {
        // ArgumentList :
        //   AssignmentOrSpreadExpression
        //   ArgumentList , AssignmentOrSpreadExpression
        //
        // AssignmentOrSpreadExpression :
        //   ... AssignmentExpression
        //   AssignmentExpression
        let start = self.get_tree_start_location();
        let mut arguments = Vec::new();
        let mut trailing_comma = false;
        let mut comma_positions = Vec::new();
        self.eat(TokenType::OPEN_PAREN);
        while self.peek_assignment_or_spread() {
            arguments.push(self.parse_assignment_or_spread()?);
            if !self.peek(TokenType::CLOSE_PAREN) {
                let comma = self.eat(TokenType::COMMA);
                if let Some(comma) = &comma {
                    comma_positions.push(comma.get_start());
                }
                if self.peek(TokenType::CLOSE_PAREN) {
                    if !self.config.at_least8 {
                        self.report_error_token(
                            comma.as_ref(),
                            "Invalid trailing comma in arguments list",
                            &[],
                        );
                    }
                    trailing_comma = true;
                }
            }
        }
        self.eat(TokenType::CLOSE_PAREN);
        Ok(ArgumentListTree::new(
            self.get_tree_location(start),
            arguments,
            trailing_comma,
            comma_positions,
        ))
    }
    // Whether we have a spread expression or an assignment next.
    //
    // <p>This does not peek the operand for the spread expression. This means that {@link
    // #parseAssignmentOrSpread} might still fail when this returns true.
    // port: Parser#peekAssignmentOrSpread
    fn peek_assignment_or_spread(&mut self) -> bool {
        self.peek(TokenType::ELLIPSIS) || self.peek_assignment_expression()
    }
    // port: Parser#parseAssignmentOrSpread
    fn parse_assignment_or_spread(&mut self) -> P<Tree> {
        if self.peek(TokenType::ELLIPSIS) {
            self.parse_iter_spread()
        } else {
            self.parse_assignment_expression()
        }
    }
    // port: Parser#peekPatternStart
    fn peek_pattern_start(&mut self) -> bool {
        self.peek(TokenType::OPEN_SQUARE) || self.peek(TokenType::OPEN_CURLY)
    }
    // port: Parser#parsePattern
    fn parse_pattern(&mut self, kind: PatternKind) -> P<Tree> {
        match self.peek_type() {
            TokenType::OPEN_SQUARE => self.parse_array_pattern(kind),
            _ => self.parse_object_pattern(kind),
        }
    }
    // port: Parser#peekArrayPatternElement
    fn peek_array_pattern_element(&mut self) -> bool {
        self.peek_expression()
    }
    // port: Parser#parseIterRest
    fn parse_iter_rest(&mut self, pattern_kind: PatternKind) -> P<Tree> {
        let start = self.get_tree_start_location();
        self.eat(TokenType::ELLIPSIS);
        let pattern_assignment_target = self.parse_rest_assignment_target(pattern_kind)?;
        Ok(IterRestTree::new(
            self.get_tree_location(start),
            pattern_assignment_target,
        ))
    }
    // port: Parser#parseRestAssignmentTarget
    fn parse_rest_assignment_target(&mut self, pattern_kind: PatternKind) -> P<Tree> {
        let pattern_assignment_target =
            self.parse_pattern_assignment_target_no_default(pattern_kind)?;
        if self.peek(TokenType::EQUAL) {
            self.report_error("A default value cannot be specified after '...'", &[]);
        }
        Ok(pattern_assignment_target)
    }
    // Pattern ::= ... | "[" Element? ("," Element?)* "]"
    // port: Parser#parseArrayPattern
    fn parse_array_pattern(&mut self, kind: PatternKind) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut elements = Vec::new();
        self.eat(TokenType::OPEN_SQUARE);
        while self.peek(TokenType::COMMA) || self.peek_array_pattern_element() {
            if self.peek(TokenType::COMMA) {
                let null_start = self.get_tree_start_location();
                self.eat(TokenType::COMMA);
                elements.push(NullTree::new(self.get_tree_location(null_start)));
            } else {
                elements.push(self.parse_pattern_assignment_target(kind)?);
                if self.peek(TokenType::COMMA) {
                    // Consume the comma separator
                    self.eat(TokenType::COMMA);
                } else {
                    // Otherwise we must be done
                    break;
                }
            }
        }
        if self.peek(TokenType::ELLIPSIS) {
            self.record_feature_used(Feature::ARRAY_PATTERN_REST);
            elements.push(self.parse_iter_rest(kind)?);
        }
        if self.eat(TokenType::CLOSE_SQUARE).is_none() {
            // If we get no closing bracket then return invalid tree to avoid compiler tripping
            // downstream. It's needed only for IDE mode where compiler continues processing even if
            // source has syntactic errors.
            let start = self.get_tree_start_location();
            return Ok(MissingPrimaryExpressionTree::new(
                self.get_tree_location(start),
            ));
        }
        Ok(ArrayPatternTree::new(
            self.get_tree_location(start),
            elements,
        ))
    }
    // Pattern ::= "{" (Field ("," Field)* ","?)? "}" | ...
    // port: Parser#parseObjectPattern
    fn parse_object_pattern(&mut self, kind: PatternKind) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut fields = Vec::new();
        self.eat(TokenType::OPEN_CURLY);
        while self.peek_object_pattern_field() {
            fields.push(self.parse_object_pattern_field(kind)?);
            if self.peek(TokenType::COMMA) {
                // Consume the comma separator
                self.eat(TokenType::COMMA);
            } else {
                // Otherwise we must be done
                break;
            }
        }
        if self.peek(TokenType::ELLIPSIS) {
            self.record_feature_used(Feature::OBJECT_PATTERN_REST);
            let rest_start = self.get_tree_start_location();
            self.eat(TokenType::ELLIPSIS);
            let pattern_assignment_target = self.parse_rest_assignment_target(kind)?;
            fields.push(ObjectRestTree::new(
                self.get_tree_location(rest_start),
                pattern_assignment_target,
            ));
        }
        self.eat(TokenType::CLOSE_CURLY);
        Ok(ObjectPatternTree::new(
            self.get_tree_location(start),
            fields,
        ))
    }
    // port: Parser#peekObjectPatternField
    fn peek_object_pattern_field(&mut self) -> bool {
        self.peek_property_name_or_computed_prop(0)
    }
    // port: Parser#parseObjectPatternField
    #[allow(clippy::needless_late_init)] // Retain Java declaration and branch order.
    fn parse_object_pattern_field(&mut self, kind: PatternKind) -> P<Tree> {
        let start = self.get_tree_start_location();
        if self.peek_type() == TokenType::OPEN_SQUARE {
            let key = self.parse_computed_property_name()?;
            self.eat(TokenType::COLON);
            let value = self.parse_pattern_assignment_target(kind)?;
            return Ok(ComputedPropertyDefinitionTree::new(
                self.get_tree_location(start),
                key,
                value,
            ));
        }
        let name;
        if self.peek_id_or_keyword() {
            let id_token = required(self.eat_id_or_keyword_as_id())?;
            if !self.peek(TokenType::COLON) {
                if id_token.is_keyword() {
                    self.report_error(
                        "cannot use keyword '%s' here.",
                        &[id_token.to_string_utf16()],
                    );
                }
                if self.peek(TokenType::EQUAL) {
                    let id_tree = IdentifierExpressionTree::new(
                        self.get_tree_location(start.clone()),
                        id_token,
                    );
                    self.eat(TokenType::EQUAL);
                    let default_value = self.parse_assignment_expression()?;
                    return Ok(DefaultParameterTree::new(
                        self.get_tree_location(start),
                        id_tree,
                        default_value,
                    ));
                }
                return Ok(PropertyNameAssignmentTree::new(
                    self.get_tree_location(start),
                    id_token,
                    None,
                ));
            }
            name = id_token;
        } else {
            name = self
                .parse_literal_expression()?
                .as_literal_expression()
                .literal_token
                .clone();
        }
        self.eat(TokenType::COLON);
        let value = self.parse_pattern_assignment_target(kind)?;
        Ok(PropertyNameAssignmentTree::new(
            self.get_tree_location(start),
            name,
            value,
        ))
    }
    // A PatternAssignmentTarget is the location where the assigned value gets stored, including an
    // optional default value.
    //
    // <dl>
    //   <dt>Spec AssignmentElement === PatternAssignmentTarget(PatternKind.ANY)
    //   <dd>Valid in an assignment that is not a formal parameter list or variable declaration.
    //       Sub-patterns and arbitrary left hand side expressions are allowed.
    //   <dt>Spec BindingElement === PatternAssignmentElement(PatternKind.INITIALIZER)
    //   <dd>Valid in a formal parameter list or variable declaration statement. Only sub-patterns and
    //       identifiers are allowed.
    // </dl>
    //
    // Examples:
    //
    // <pre>
    //   <code>
    //     [a, {foo: b = 'default'}] = someArray;          // valid
    //     [x.a, {foo: x.b = 'default'}] = someArray;      // valid
    //
    //     let [a, {foo: b = 'default'}] = someArray;      // valid
    //     let [x.a, {foo: x.b = 'default'}] = someArray;  // invalid
    //
    //     function f([a, {foo: b = 'default'}]) {...}     // valid
    //     function f([x.a, {foo: x.b = 'default'}]) {...} // invalid
    //   </code>
    // </pre>
    // port: Parser#parsePatternAssignmentTarget
    fn parse_pattern_assignment_target(&mut self, pattern_kind: PatternKind) -> P<Tree> {
        let start = self.get_tree_start_location();
        let mut assignment_target =
            self.parse_pattern_assignment_target_no_default(pattern_kind)?;
        if self.peek(TokenType::EQUAL) {
            self.eat(TokenType::EQUAL);
            let default_value = self.parse_assignment_expression()?;
            assignment_target = DefaultParameterTree::new(
                self.get_tree_location(start),
                assignment_target,
                default_value,
            );
        }
        Ok(assignment_target)
    }
    // port: Parser#parsePatternAssignmentTargetNoDefault
    fn parse_pattern_assignment_target_no_default(&mut self, kind: PatternKind) -> P<Tree> {
        if self.peek_pattern_start() {
            self.parse_pattern(kind)
        } else {
            let assignment_target = self.parse_left_hand_side_expression()?;
            if !assignment_target.is_valid_assignment_target() {
                self.report_error("invalid assignment target", &[]);
            }
            if kind == PatternKind::INITIALIZER
                && assignment_target.type_ != ParseTreeType::IDENTIFIER_EXPRESSION
            {
                // We're in the context of a formal parameter list or a variable declaration statement
                self.report_error(
                    "Only an identifier or destructuring pattern is allowed here.",
                    &[],
                );
            }
            Ok(assignment_target)
        }
    }
    // Consume a (possibly implicit) semi-colon. Reports an error if a semi-colon is not present.
    // port: Parser#eatPossiblyImplicitSemiColon
    fn eat_possibly_implicit_semi_colon(&mut self) {
        if self.peek(TokenType::SEMI_COLON) {
            self.eat(TokenType::SEMI_COLON);
            return;
        }
        if self.peek_implicit_semi_colon() {
            return;
        }
        self.report_error("Semi-colon expected", &[]);
    }
    // Returns true if an implicit or explicit semi colon is at the current location.
    // port: Parser#peekImplicitSemiColon
    fn peek_implicit_semi_colon(&mut self) -> bool {
        self.peek_implicit_semi_colon_at(0)
    }
    // port: Parser#peekImplicitSemiColon
    fn peek_implicit_semi_colon_at(&mut self, index: usize) -> bool {
        let line_advanced = if index == 0 {
            self.get_next_line() > self.get_last_line()
        } else {
            self.scanner.peek_token_ref_at(index).location.start.line
                > self.scanner.peek_token_ref_at(index - 1).location.end.line
        };
        line_advanced
            || self.peek_at(index, TokenType::SEMI_COLON)
            || self.peek_at(index, TokenType::CLOSE_CURLY)
            || self.peek_at(index, TokenType::END_OF_FILE)
    }
    // Returns the line number of the most recently consumed token.
    // port: Parser#getLastLine
    fn get_last_line(&self) -> i32 {
        self.last_source_position.line
    }
    // Returns the line number of the next token.
    // port: Parser#getNextLine
    fn get_next_line(&mut self) -> i32 {
        self.scanner.peek_token_ref_at(0).location.start.line
    }
    // Consumes the next token if it is of the expected type. Otherwise returns null. Never reports
    // errors.
    //
    // @return The consumed token, or null if the next token is not of the expected type.
    // port: Parser#eatOpt
    fn eat_opt(&mut self, expected_token_type: TokenType) -> Option<Token> {
        if self.peek(expected_token_type) {
            self.eat(expected_token_type)
        } else {
            None
        }
    }
    // port: Parser#inStrictContext
    fn in_strict_context(&self) -> bool {
        // TODO(johnlenz): track entering strict scripts/modules/functions.
        self.config.is_strict_mode
    }
    // port: Parser#peekId
    fn peek_id(&mut self) -> bool {
        self.peek_id_at(0)
    }
    // @return whether the next token is an identifier.
    // port: Parser#peekId
    fn peek_id_at(&mut self, index: usize) -> bool {
        let token = self.peek_token_at(index);
        let mut type_to_check = token.type_;
        if type_to_check == TokenType::IDENTIFIER {
            let value = token.get_maybe_private_value();
            let k = Keywords::get_by_name(value);
            let Some(k) = k else {
                return true;
            };
            // Swap the type to the matched keyword's type
            type_to_check = k.type_();
        }
        // There is one special case to handle here: outside of strict-mode code, strict-mode keywords
        // can be used as identifiers
        !self.in_strict_context() && Keywords::is_strict_keyword(type_to_check)
    }
    // port: Parser#peekIdOrKeyword
    fn peek_id_or_keyword(&mut self) -> bool {
        self.peek_id_or_keyword_at(0)
    }
    // port: Parser#peekIdOrKeyword
    fn peek_id_or_keyword_at(&mut self, index: usize) -> bool {
        let type_ = self.peek_type_at(index);
        type_ == TokenType::IDENTIFIER || Keywords::is_keyword_type(type_)
    }
    // Shorthand for eatOpt(TokenType.IDENTIFIER)
    // port: Parser#eatIdOpt
    fn eat_id_opt(&mut self) -> Option<Token> {
        if self.peek_id() {
            self.eat_id_or_keyword_as_id()
        } else {
            None
        }
    }
    // Consumes an identifier token that is not a reserved word.
    //
    // @see "http://www.ecma-international.org/ecma-262/5.1/#sec-7.6"
    // port: Parser#eatId
    fn eat_id(&mut self) -> Option<Token> {
        if self.peek_id() {
            self.eat_id_or_keyword_as_id()
        } else {
            let token = self.peek_token();
            self.report_expected_error(Some(&token), TokenType::IDENTIFIER.to_string_utf16());
            if self.peek_id_or_keyword() {
                self.eat_id_or_keyword_as_id()
            } else {
                None
            }
        }
    }
    // port: Parser#eatObjectLiteralPropertyName
    fn eat_object_literal_property_name(&mut self) -> Option<Token> {
        let token = self.peek_token();
        match token.type_ {
            TokenType::STRING | TokenType::NUMBER | TokenType::BIGINT => Some(self.next_token()),
            _ => self.eat_id_or_keyword_as_id(),
        }
    }
    // Consumes an identifier token that may be a reserved word, i.e. an IdentifierName, not
    // necessarily an Identifier.
    //
    // @see "http://www.ecma-international.org/ecma-262/5.1/#sec-7.6"
    // port: Parser#eatIdOrKeywordAsId
    fn eat_id_or_keyword_as_id(&mut self) -> Option<Token> {
        let token = self.next_token();
        if token.type_ == TokenType::IDENTIFIER {
            Some(token)
        } else if Keywords::is_keyword_type(token.type_) {
            let value = Keywords::get_by_type(token.type_)
                .unwrap()
                .to_string_utf16();
            Some(IdentifierToken::new(token.location, value))
        } else {
            self.report_expected_error(Some(&token), TokenType::IDENTIFIER.to_string_utf16());
            None
        }
    }
    // Consumes the next token. If the consumed token is not of the expected type then report an error
    // and return null. Otherwise return the consumed token.
    //
    // @return The consumed token, or null if the next token is not of the expected type.
    // port: Parser#eat
    fn eat(&mut self, expected_token_type: TokenType) -> Option<Token> {
        let token = self.next_token();
        if token.type_ != expected_token_type {
            self.report_expected_error(Some(&token), expected_token_type.to_string_utf16());
            None
        } else {
            Some(token)
        }
    }
    // Report a 'X' expected error message.
    //
    // @param token The location to report the message at.
    // @param expected The thing that was expected.
    // port: Parser#reportExpectedError
    fn report_expected_error(&mut self, token: Option<&Token>, expected: JsString) {
        self.report_error_token(token, "'%s' expected", &[expected]);
    }
    // Returns a SourcePosition for the start of a parse tree that starts at the current location.
    // port: Parser#getTreeStartLocation
    fn get_tree_start_location(&mut self) -> SourcePosition {
        self.peek_token().location.start
    }
    // Returns a SourcePosition for the end of a parse tree that ends at the current location.
    // port: Parser#getTreeEndLocation
    fn get_tree_end_location(&self) -> SourcePosition {
        self.last_source_position.clone()
    }
    // Returns a SourceRange for a parse tree that starts at {start} and ends at the current location.
    // port: Parser#getTreeLocation
    fn get_tree_location(&self, start: SourcePosition) -> SourceRange {
        SourceRange::new(start, self.get_tree_end_location())
    }
    // Consumes the next token and returns it. Will return a never ending stream of
    // TokenType.END_OF_FILE at the end of the file so callers don't have to check for EOF explicitly.
    //
    // <p>Tokenizing is contextual. nextToken() will never return a regular expression literal.
    // port: Parser#nextToken
    fn next_token(&mut self) -> Token {
        let token = self.scanner.next_token();
        self.last_source_position = token.location.end.clone();
        token
    }
    // Consumes a regular expression literal token and returns it.
    // port: Parser#nextRegularExpressionLiteralToken
    fn next_regular_expression_literal_token(&mut self) -> Token {
        let token = self.scanner.next_regular_expression_literal_token();
        self.last_source_position = token.location.end.clone();
        token
    }
    // Consumes a template literal token and returns it.
    // port: Parser#nextTemplateLiteralToken
    fn next_template_literal_token(&mut self) -> Token {
        let token = self.scanner.next_template_literal_token();
        self.last_source_position = token.location.end.clone();
        token
    }
    // Returns true if the next token is of the expected type. Does not consume the token.
    // port: Parser#peek
    fn peek(&mut self, expected_type: TokenType) -> bool {
        self.peek_at(0, expected_type)
    }
    // Returns true if the index-th next token is of the expected type. Does not consume any tokens.
    // port: Parser#peek
    fn peek_at(&mut self, index: usize, expected_type: TokenType) -> bool {
        self.peek_type_at(index) == expected_type
    }
    // Returns the TokenType of the next token. Does not consume any tokens.
    // port: Parser#peekType
    fn peek_type(&mut self) -> TokenType {
        self.peek_type_at(0)
    }
    // Returns the TokenType of the index-th next token. Does not consume any tokens.
    // port: Parser#peekType
    fn peek_type_at(&mut self, index: usize) -> TokenType {
        self.scanner.peek_token_ref_at(index).type_
    }
    // Returns the next token. Does not consume any tokens.
    // port: Parser#peekToken
    fn peek_token(&mut self) -> Token {
        self.peek_token_at(0)
    }
    // Returns the index-th next token. Does not consume any tokens.
    // port: Parser#peekToken
    fn peek_token_at(&mut self, index: usize) -> Token {
        self.scanner.peek_token_at(index)
    }
    // Reports an error message at a given token.
    //
    // @param token The location to report the message at.
    // @param message The message to report in String.format style.
    // @param arguments The arguments to fill in the message format.
    // port: Parser#reportError
    fn report_error_token(&mut self, token: Option<&Token>, message: &str, arguments: &[JsString]) {
        match token {
            None => self.report_error(message, arguments),
            Some(token) => self
                .error_reporter
                .report_error(token.get_start(), message, arguments),
        }
    }
    // Reports an error message at a given parse tree's location.
    //
    // @param parseTree The location to report the message at.
    // @param message The message to report in String.format style.
    // @param arguments The arguments to fill in the message format.
    // port: Parser#reportError
    fn report_error_tree(
        &mut self,
        parse_tree: Option<&Tree>,
        message: &str,
        arguments: &[JsString],
    ) {
        match parse_tree {
            None => self.report_error(message, arguments),
            Some(parse_tree) => self.error_reporter.report_error(
                parse_tree.location.start.clone(),
                message,
                arguments,
            ),
        }
    }
    // Reports an error at the current location.
    //
    // @param message The message to report in String.format style.
    // @param arguments The arguments to fill in the message format.
    // port: Parser#reportError
    fn report_error(&mut self, message: &str, arguments: &[JsString]) {
        let position = self.scanner.get_position();
        self.error_reporter
            .report_error(position, message, arguments);
    }
    // Reports an error at the specified location.
    //
    // @param position The position of the error.
    // @param message The message to report in String.format style.
    // @param arguments The arguments to fill in the message format.
    // port: Parser#reportError
    fn report_error_at(&self, position: SourcePosition, message: &str, arguments: &[JsString]) {
        self.error_reporter
            .report_error(position, message, arguments);
    }
    // port: Parser#reportTemplateErrorIfPresent
    fn report_template_error_if_present(&mut self, template_token: &Token) {
        let template_token = template_token.as_template_literal();
        let Some(message) = &template_token.error_message else {
            return;
        };
        match template_token.error_level {
            Some(ErrorLevel::WARNING) => self.error_reporter.report_warning(
                template_token.error_position.clone().unwrap(),
                "%s",
                std::slice::from_ref(message),
            ),
            Some(ErrorLevel::ERROR) => self.report_error_at(
                template_token.error_position.clone().unwrap(),
                "%s",
                std::slice::from_ref(message),
            ),
            _ => panic!("AssertionError"),
        }
    }
    // port: Parser#recordFeatureUsed
    fn record_feature_used(&mut self, feature: Feature) -> &mut Self {
        self.features = self.features.with(feature);
        self
    }
}
