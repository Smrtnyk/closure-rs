/*
 * Copyright 2019 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/ParsingUtil.java.

//! Port of `com.google.javascript.jscomp.parsing.ParsingUtil`.

use closure_rhino::check_not_null;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;

/// Utility functions for parsing that don't depend on the compiler.
pub struct ParsingUtil;

impl ParsingUtil {
    /// Calls {@code cb} with all NAMEs declared in a PARAM_LIST or destructuring pattern.
    ///
    /// Java's `checkNotNull(n)` on the recursive argument is done at each recursive call site,
    /// because the Rust parameter is a non-null `NodeId`.
    // port: ParsingUtil#getParamOrPatternNames
    pub fn get_param_or_pattern_names(ast: &Ast, n: NodeId, cb: &mut dyn FnMut(NodeId)) {
        match n.get_token(ast) {
            Token::EMPTY | Token::GETELEM | Token::GETPROP => {
                // let [,] = ...
                // [someArray[1]] = ...
                // [someObj.someProp] = ...
                // An empty node or assigned to an existing value means nothings here and we can't
                // recurse further.
            }
            Token::NAME => {
                // let [someName] = ...
                // A name node generally means we found an assignment happening.
                cb(n)
            }
            Token::ITER_REST | Token::OBJECT_REST | Token::STRING_KEY | Token::DEFAULT_VALUE => {
                // let [...name] = ...
                // let {...name} = ...
                // let {name} = ...
                // let {name = 10} = ...
                // For these node types there is a name node as the first child.
                Self::get_param_or_pattern_names(ast, check_not_null!(n.get_first_child(ast)), cb)
            }
            Token::COMPUTED_PROP => {
                // let {[someProp]: name} = ...
                // For this node types there is a name node as the second child.
                Self::get_param_or_pattern_names(ast, check_not_null!(n.get_second_child(ast)), cb)
            }
            Token::OBJECT_PATTERN | Token::ARRAY_PATTERN | Token::PARAM_LIST => {
                // let {<pattern>} = ...
                // let [<pattern>] = ...
                // function fn(<params>) ...
                // Each item in the destructuring pattern is scanned for assignments. Although
                // PARAM_LIST isn't technically a pattern, we know how to handle it anyway.
                let mut c = n.get_first_child(ast);
                while let Some(cur) = c {
                    Self::get_param_or_pattern_names(ast, cur, cb);
                    c = cur.get_next(ast);
                }
            }
            _ => panic!("Unexpected parameter structure"),
        }
    }
}

// Mutable arena adapter used by NodeUtil callbacks.
impl ParsingUtil {
    /// Calls {@code cb} with all NAMEs declared in a PARAM_LIST or destructuring pattern.
    ///
    /// Java's `checkNotNull(n)` on the recursive argument is done at each recursive call site,
    /// because the Rust parameter is a non-null `NodeId`.
    // port: ParsingUtil#getParamOrPatternNames
    pub fn get_param_or_pattern_names_with_mutable_context(
        ast: &mut Ast,
        n: NodeId,
        cb: &mut dyn FnMut(&mut Ast, NodeId),
    ) {
        match n.get_token(ast) {
            Token::EMPTY | Token::GETELEM | Token::GETPROP => {
                // let [,] = ...
                // [someArray[1]] = ...
                // [someObj.someProp] = ...
                // An empty node or assigned to an existing value means nothings here and we can't
                // recurse further.
            }
            Token::NAME => {
                // let [someName] = ...
                // A name node generally means we found an assignment happening.
                cb(ast, n)
            }
            Token::ITER_REST | Token::OBJECT_REST | Token::STRING_KEY | Token::DEFAULT_VALUE => {
                // let [...name] = ...
                // let {...name} = ...
                // let {name} = ...
                // let {name = 10} = ...
                // For these node types there is a name node as the first child.
                let first = check_not_null!(n.get_first_child(ast));
                Self::get_param_or_pattern_names_with_mutable_context(ast, first, cb)
            }
            Token::COMPUTED_PROP => {
                // let {[someProp]: name} = ...
                // For this node types there is a name node as the second child.
                let second = check_not_null!(n.get_second_child(ast));
                Self::get_param_or_pattern_names_with_mutable_context(ast, second, cb)
            }
            Token::OBJECT_PATTERN | Token::ARRAY_PATTERN | Token::PARAM_LIST => {
                // let {<pattern>} = ...
                // let [<pattern>] = ...
                // function fn(<params>) ...
                // Each item in the destructuring pattern is scanned for assignments. Although
                // PARAM_LIST isn't technically a pattern, we know how to handle it anyway.
                let mut c = n.get_first_child(ast);
                while let Some(cur) = c {
                    Self::get_param_or_pattern_names_with_mutable_context(ast, cur, cb);
                    c = cur.get_next(ast);
                }
            }
            _ => panic!("Unexpected parameter structure"),
        }
    }
}
