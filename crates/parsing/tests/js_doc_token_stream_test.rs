/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   test/com/google/javascript/jscomp/parsing/JsDocTokenStreamTest.java.

//! Port of `com.google.javascript.jscomp.parsing.JsDocTokenStreamTest`.

use closure_parsing::js_doc_token::JsDocToken::{self, *};
use closure_parsing::js_doc_token_stream::JsDocTokenStream;

// port: JsDocTokenStreamTest#testJsDocTokenization1
#[test]
fn test_js_doc_tokenization1() {
    let tokens: Vec<JsDocToken> = vec![
        STAR,
        ANNOTATION,
        LEFT_CURLY,
        STRING,
        RIGHT_CURLY,
        EOL,
        STAR,
        ANNOTATION,
    ];
    let strings: Vec<&str> = vec!["type", "string", "private"];
    test_js_doc_token_stream(" * @type {string}\n * @private", &tokens, &strings);
    test_js_doc_token_stream(" *    @type { string } \n * @private", &tokens, &strings);
    test_js_doc_token_stream(" * @type   {  string}\n * @private", &tokens, &strings);
    test_js_doc_token_stream(" * @type {string  }\n * @private", &tokens, &strings);
    test_js_doc_token_stream(" * @type {string}\n *   @private", &tokens, &strings);
    test_js_doc_token_stream(" * @type {string}   \n * @private", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization2
#[test]
fn test_js_doc_tokenization2() {
    let tokens: Vec<JsDocToken> = vec![
        ANNOTATION,
        LEFT_CURLY,
        STRING,
        LEFT_ANGLE,
        STRING,
        PIPE,
        STRING,
        RIGHT_ANGLE,
        RIGHT_CURLY,
    ];
    let strings: Vec<&str> = vec!["param", "Array", "string", "null"];
    test_js_doc_token_stream("@param {Array.<string|null>}", &tokens, &strings);
    test_js_doc_token_stream("@param {Array.<string|null>}", &tokens, &strings);
    test_js_doc_token_stream("@param {Array.<string |null>}", &tokens, &strings);
    test_js_doc_token_stream(" @param {Array.<string |  null>}", &tokens, &strings);
    test_js_doc_token_stream(" @param {Array.<string|null  >}", &tokens, &strings);
    test_js_doc_token_stream("@param {Array  .<string|null>}", &tokens, &strings);
    test_js_doc_token_stream("@param   {Array.<string|null>}", &tokens, &strings);
    test_js_doc_token_stream("@param {  Array.<string|null>}", &tokens, &strings);
    test_js_doc_token_stream("@param {Array.<string|   null>}  ", &tokens, &strings);
    test_js_doc_token_stream("@param {Array.<string|null>}", &tokens, &strings);
    test_js_doc_token_stream(
        "     @param { Array .< string |null > } ",
        &tokens,
        &strings,
    );
}

// port: JsDocTokenStreamTest#testJsDocTokenization4
#[test]
fn test_js_doc_tokenization4() {
    let tokens: Vec<JsDocToken> = vec![
        ANNOTATION,
        LEFT_CURLY,
        STRING,
        LEFT_ANGLE,
        LEFT_PAREN,
        STRING,
        COMMA,
        STRING,
        RIGHT_PAREN,
        RIGHT_ANGLE,
        RIGHT_CURLY,
        EOF,
    ];
    let strings: Vec<&str> = vec!["param", "Array", "string", "null"];
    test_js_doc_token_stream("@param {Array.<(string,null)>}", &tokens, &strings);
    test_js_doc_token_stream("@param {Array  .<(string,null)> } ", &tokens, &strings);
    test_js_doc_token_stream(" @param {Array.<  (  string,null)>}", &tokens, &strings);
    test_js_doc_token_stream("@param {Array.<(string  , null)>}", &tokens, &strings);
    test_js_doc_token_stream("@param {Array.<(string,   null)  > }  ", &tokens, &strings);
    test_js_doc_token_stream("@param {  Array  .<  (string,null)>}   ", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization5
#[test]
fn test_js_doc_tokenization5() {
    let tokens: Vec<JsDocToken> = vec![ANNOTATION, STRING, EOC, EOF];
    let strings: Vec<&str> = vec!["param", "foo.Bar"];
    test_js_doc_token_stream("@param foo.Bar*/", &tokens, &strings);
    test_js_doc_token_stream(" @param   foo.Bar*/", &tokens, &strings);
    test_js_doc_token_stream(" @param foo.Bar   */", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization6
#[test]
fn test_js_doc_tokenization6() {
    let tokens: Vec<JsDocToken> = vec![ANNOTATION, EOL, ANNOTATION, EOL, ANNOTATION, EOC];
    let strings: Vec<&str> = vec!["private", "static", "desc"];
    test_js_doc_token_stream("@private\n@static\n@desc*/", &tokens, &strings);
    test_js_doc_token_stream("@private\n @static\n@desc*/", &tokens, &strings);
    test_js_doc_token_stream("@private\n@static\n @desc*/", &tokens, &strings);
    test_js_doc_token_stream("@private\n@static\n@desc */", &tokens, &strings);
    test_js_doc_token_stream(" @private \n@static\n @desc*/", &tokens, &strings);
    test_js_doc_token_stream("@private\n@static    \n @desc  */", &tokens, &strings);
    test_js_doc_token_stream("@private\n@static\n@desc*/", &tokens, &strings);
    test_js_doc_token_stream("@private   \n@static   \n @desc*/", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization7
#[test]
fn test_js_doc_tokenization7() {
    let tokens: Vec<JsDocToken> = vec![
        ITER_REST, ITER_REST, ITER_REST, ITER_REST, ITER_REST, LEFT_ANGLE, EOC,
    ];
    let strings: Vec<&str> = vec![];

    test_js_doc_token_stream("................<*/", &tokens, &strings);
    test_js_doc_token_stream("............... .<*/", &tokens, &strings);
    test_js_doc_token_stream("................< */", &tokens, &strings);
    test_js_doc_token_stream("............... .< */", &tokens, &strings);
    test_js_doc_token_stream("............... .< */ ", &tokens, &strings);
    test_js_doc_token_stream(" ............... .< */ ", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization8
#[test]
fn test_js_doc_tokenization8() {
    let tokens: Vec<JsDocToken> = vec![
        STAR, ANNOTATION, STRING, STRING, STRING, STRING, STRING, STRING, STRING, EOL, EOC,
    ];
    let strings: Vec<&str> = vec![
        "param",
        "foo.Bar",
        "opt_name",
        "this",
        "parameter",
        "is",
        "a",
        "name",
    ];
    test_js_doc_token_stream(
        "* @param foo.Bar opt_name this parameter is a name\n*/\n",
        &tokens,
        &strings,
    );
    test_js_doc_token_stream(
        " *  @param foo.Bar opt_name this parameter is a name \n*/ \n",
        &tokens,
        &strings,
    );
}

// port: JsDocTokenStreamTest#testJsDocTokenization9
#[test]
fn test_js_doc_tokenization9() {
    let tokens: Vec<JsDocToken> = vec![
        STAR, ANNOTATION, STRING, STRING, STRING, STRING, STRING, ANNOTATION, STRING, EOL, EOC,
    ];
    let strings: Vec<&str> = vec![
        "param",
        "foo.Bar",
        "opt_name",
        "this",
        "parameter",
        "does",
        "media",
        "blah",
    ];
    test_js_doc_token_stream(
        "* @param foo.Bar opt_name this parameter does @media blah\n*/\n",
        &tokens,
        &strings,
    );
}

// port: JsDocTokenStreamTest#testJsDocTokenization10
#[test]
fn test_js_doc_tokenization10() {
    let tokens: Vec<JsDocToken> = vec![STRING, LEFT_ANGLE, STRING, RIGHT_ANGLE, EOC];
    let strings: Vec<&str> = vec!["Array", "String"];
    test_js_doc_token_stream("Array<String>*/", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization11
#[test]
fn test_js_doc_tokenization11() {
    let tokens: Vec<JsDocToken> =
        vec![ANNOTATION, LEFT_CURLY, STRING, QMARK, RIGHT_CURLY, EOC, EOF];
    let strings: Vec<&str> = vec!["param", "string"];
    test_js_doc_token_stream("@param {string?}*/", &tokens, &strings);
    test_js_doc_token_stream(" @param {string?}*/", &tokens, &strings);
    test_js_doc_token_stream("@param { string?}*/", &tokens, &strings);
    test_js_doc_token_stream("@param {string ?}*/", &tokens, &strings);
    test_js_doc_token_stream("@param  {string ?  } */", &tokens, &strings);
    test_js_doc_token_stream("@param { string  ?  }*/", &tokens, &strings);
    test_js_doc_token_stream("@param {string?  }*/", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization12
#[test]
fn test_js_doc_tokenization12() {
    let tokens: Vec<JsDocToken> = vec![STRING, ITER_REST, EOC];
    let strings: Vec<&str> = vec!["function"];

    test_js_doc_token_stream("function ...*/", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization13
#[test]
fn test_js_doc_tokenization13() {
    let tokens: Vec<JsDocToken> = vec![ITER_REST, LEFT_SQUARE, STRING, RIGHT_SQUARE, EOC];
    let strings: Vec<&str> = vec!["number"];

    test_js_doc_token_stream("...[number]*/", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization14
#[test]
fn test_js_doc_tokenization14() {
    // Since ES4 type parsing only requires to parse an ITER_REST when it is
    // followed by a comma (,) we are allowing this case to parse this way.
    // This is a simplification of the tokenizer, but the extra complexity is
    // never used.
    let tokens: Vec<JsDocToken> = vec![STRING, LEFT_SQUARE, STRING, EOC];
    let strings: Vec<&str> = vec!["foo", "bar..."];

    test_js_doc_token_stream("foo[ bar...*/", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization15
#[test]
fn test_js_doc_tokenization15() {
    let tokens: Vec<JsDocToken> = vec![STRING, LEFT_SQUARE, STRING, COMMA, ITER_REST, EOC];
    let strings: Vec<&str> = vec!["foo", "bar"];

    test_js_doc_token_stream("foo[ bar,...*/", &tokens, &strings);
    test_js_doc_token_stream("foo[ bar ,...*/", &tokens, &strings);
    test_js_doc_token_stream("foo[bar, ...*/", &tokens, &strings);
    test_js_doc_token_stream("foo[ bar  ,   ...  */", &tokens, &strings);
    test_js_doc_token_stream("foo [bar,... */", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization16
#[test]
fn test_js_doc_tokenization16() {
    let tokens: Vec<JsDocToken> = vec![
        STRING, COLON, COLON, COLON, ITER_REST, STRING, COLON, STRING, EOC,
    ];
    let strings: Vec<&str> = vec!["foo", "bar", "bar2"];

    test_js_doc_token_stream("foo:::...bar:bar2*/", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization17
#[test]
fn test_js_doc_tokenization17() {
    let tokens: Vec<JsDocToken> = vec![STRING, EOL, EOC];
    let strings: Vec<&str> = vec![".."];

    test_js_doc_token_stream("..\n*/", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization18
#[test]
fn test_js_doc_tokenization18() {
    let tokens: Vec<JsDocToken> = vec![STRING, EOL, EOC];
    let strings: Vec<&str> = vec!["."];

    test_js_doc_token_stream(".\n*/", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization19
#[test]
fn test_js_doc_tokenization19() {
    let tokens: Vec<JsDocToken> = vec![ANNOTATION, LEFT_CURLY, STAR, RIGHT_CURLY, EOC];
    let strings: Vec<&str> = vec!["type", "*"];

    test_js_doc_token_stream("@type {*}*/", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization20
#[test]
fn test_js_doc_tokenization20() {
    let tokens: Vec<JsDocToken> = vec![ANNOTATION, LEFT_CURLY, BANG, STRING, RIGHT_CURLY, EOC, EOF];
    let strings: Vec<&str> = vec!["param", "Object"];
    test_js_doc_token_stream("@param {!Object}*/", &tokens, &strings);
    test_js_doc_token_stream(" @param {!Object}*/", &tokens, &strings);
    test_js_doc_token_stream("@param {! Object}*/", &tokens, &strings);
    test_js_doc_token_stream("@param { !Object}*/", &tokens, &strings);
    test_js_doc_token_stream("@param  {!Object  } */", &tokens, &strings);
    test_js_doc_token_stream("@param {  ! Object  }*/", &tokens, &strings);
    test_js_doc_token_stream("@param {!Object  }*/", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJsDocTokenization21
#[test]
fn test_js_doc_tokenization21() {
    let tokens: Vec<JsDocToken> = vec![
        ANNOTATION,
        LEFT_CURLY,
        STRING,
        EQUALS,
        RIGHT_CURLY,
        EOC,
        EOF,
    ];
    let strings: Vec<&str> = vec!["param", "Object"];
    test_js_doc_token_stream("@param {Object=}*/", &tokens, &strings);
    test_js_doc_token_stream(" @param {Object=}*/", &tokens, &strings);
    test_js_doc_token_stream("@param { Object =}*/", &tokens, &strings);
    test_js_doc_token_stream("@param { Object=}*/", &tokens, &strings);
    test_js_doc_token_stream("@param  {Object=  } */", &tokens, &strings);
    test_js_doc_token_stream("@param { Object = }*/", &tokens, &strings);
    test_js_doc_token_stream("@param {Object=  }*/", &tokens, &strings);
}

// port: JsDocTokenStreamTest#testJSDocTokenStream
fn test_js_doc_token_stream(comment: &str, tokens: &[JsDocToken], strings: &[&str]) {
    let mut stream = JsDocTokenStream::new_with_lineno(comment, 0);
    let mut strings_index = 0;
    for &token in tokens {
        let read_token = stream.get_js_doc_token();

        // token equality
        if token != read_token {
            assert_eq!(read_token, token);
        }

        // string equality
        if token == ANNOTATION || token == STRING {
            assert_eq!(stream.get_string(), strings[strings_index]);
            strings_index += 1;
        }
    }
}
