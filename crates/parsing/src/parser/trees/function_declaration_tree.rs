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
//   src/com/google/javascript/jscomp/parsing/parser/trees/FunctionDeclarationTree.java.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    DECLARATION,
    EXPRESSION,
    MEMBER,
    ARROW,
}

#[derive(Clone, Debug)]
pub struct FunctionDeclarationTree {
    pub name: Option<Token>,
    pub formal_parameter_list: Tree,
    pub function_body: Tree,
    pub is_class_member: bool,
    pub is_static: bool,
    pub is_generator: bool,
    pub is_optional: bool,
    pub is_async: bool,
    pub kind: Kind,
}
impl FunctionDeclarationTree {
    // port: FunctionDeclarationTree#builder
    pub fn builder(kind: Kind) -> Builder {
        Builder::new(kind)
    }
    // port: FunctionDeclarationTree#<init>
    fn from_builder(builder: &Builder, location: SourceRange) -> Tree {
        ParseTree::new(
            ParseTreeType::FUNCTION_DECLARATION,
            location,
            ParseTreeData::FunctionDeclarationTree(Box::new(Self {
                name: builder.name.clone(),
                is_class_member: builder.is_class_member,
                is_static: builder.is_static,
                is_generator: builder.is_generator,
                is_optional: builder.is_optional,
                kind: builder.kind,
                formal_parameter_list: builder.formal_parameter_list.clone().expect("null"),
                function_body: builder.function_body.clone().expect("null"),
                is_async: builder.is_async,
            })),
        )
    }
}

/// Builds a {@link FunctionDeclarationTree}.
pub struct Builder {
    kind: Kind,
    name: Option<Token>,
    formal_parameter_list: Option<Tree>,
    function_body: Option<Tree>,
    is_class_member: bool,
    is_static: bool,
    is_generator: bool,
    is_optional: bool,
    is_async: bool,
}
impl Builder {
    // port: FunctionDeclarationTree.Builder#<init>
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            name: None,
            formal_parameter_list: None,
            function_body: None,
            is_class_member: false,
            is_static: false,
            is_generator: false,
            is_optional: false,
            is_async: false,
        }
    }
    /// Optional function name.
    ///
    /// <p>Default is {@code null}.
    // port: FunctionDeclarationTree.Builder#setName
    pub fn set_name(&mut self, name: impl Into<Option<Token>>) -> &mut Self {
        self.name = name.into();
        self
    }
    /// Required parameter list.
    // port: FunctionDeclarationTree.Builder#setFormalParameterList
    pub fn set_formal_parameter_list(&mut self, formal_parameter_list: Tree) -> &mut Self {
        self.formal_parameter_list = Some(formal_parameter_list);
        self
    }
    /// Required function body.
    // port: FunctionDeclarationTree.Builder#setFunctionBody
    pub fn set_function_body(&mut self, function_body: Tree) -> &mut Self {
        self.function_body = Some(function_body);
        self
    }
    /// Is the method an ES6 class member?
    ///
    /// <p>Default is {@code false}. Only relevant for class method member declarations.
    // port: FunctionDeclarationTree.Builder#setIsClassMember
    pub fn set_is_class_member(&mut self, is_class_member: bool) -> &mut Self {
        self.is_class_member = is_class_member;
        self
    }
    /// Is the method static?
    ///
    /// <p>Default is {@code false}. Only relevant for class method member declarations.
    // port: FunctionDeclarationTree.Builder#setStatic
    pub fn set_static(&mut self, is_static: bool) -> &mut Self {
        self.is_static = is_static;
        self
    }
    /// Is this a generator function?
    ///
    /// <p>Default is {@code false}.
    // port: FunctionDeclarationTree.Builder#setGenerator
    pub fn set_generator(&mut self, is_generator: bool) -> &mut Self {
        self.is_generator = is_generator;
        self
    }
    /// Is this the declaration of an optional function parameter? Default is {@code false}.
    ///
    /// <p>Only relevant for function declaration as a parameter to another function.
    // port: FunctionDeclarationTree.Builder#setOptional
    pub fn set_optional(&mut self, is_optional: bool) -> &mut Self {
        self.is_optional = is_optional;
        self
    }
    /// Is this an asynchronous function?
    ///
    /// <p>Default is {@code false}.
    // port: FunctionDeclarationTree.Builder#setAsync
    pub fn set_async(&mut self, is_async: bool) -> &mut Self {
        self.is_async = is_async;
        self
    }
    // Return a new {@link FunctionDeclarationTree}.
    //
    // <p>The location is provided at this point because it cannot be correctly calculated until the
    // whole function has been parsed.
    // port: FunctionDeclarationTree.Builder#build
    pub fn build(&mut self, location: SourceRange) -> Tree {
        FunctionDeclarationTree::from_builder(self, location)
    }
}
