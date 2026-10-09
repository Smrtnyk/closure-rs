/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/ijs/JsdocUtil.java.

//! Port of `com.google.javascript.jscomp.ijs.JsdocUtil`: static utility methods for dealing with
//! inspecting and constructing JSDoc objects.

use crate::{
    abstract_compiler::AbstractCompiler,
    node_util::{NodeUtil, ValueType},
    var::VarId,
};
use closure_rhino::{
    ir::IR,
    js_type_expression::JSTypeExpression,
    jsdoc_info::{Builder, JSDocInfo, Visibility},
    node::{Ast, NodeId},
    simple_source_file::SimpleSourceFile,
    static_source_file::{SourceKind, StaticSourceFile},
    token::Token,
};
use std::sync::{Arc, LazyLock};

// port: JsdocUtil#SYNTHETIC_FILE_NAME
const SYNTHETIC_FILE_NAME: &str = "<synthetic>";
// port: JsdocUtil#SYNTHETIC_FILE
static SYNTHETIC_FILE: LazyLock<Arc<dyn StaticSourceFile>> = LazyLock::new(|| {
    Arc::new(SimpleSourceFile::new(
        SYNTHETIC_FILE_NAME,
        SourceKind::EXTERN,
    ))
});

/// Java `JsdocUtil` is a static utility class with a private constructor.
pub struct JsdocUtil;

impl JsdocUtil {
    /// Java's static `SYNTETIC_SRCINFO_NODE` (`new Node(Token.EMPTY)` with `SYNTHETIC_FILE`). A
    /// Java static node cannot live in a Rust arena shared by every compilation, so each use
    /// creates the same node (same token, default source position and length, the shared
    /// `SYNTHETIC_FILE`) in the arena of the tree it is copied into.
    // port: JsdocUtil#SYNTETIC_SRCINFO_NODE
    fn syntetic_srcinfo_node(ast: &mut Ast) -> NodeId {
        ast.new_node(Token::EMPTY)
            .set_static_source_file(ast, Some(SYNTHETIC_FILE.clone()))
    }

    // port: JsdocUtil#isPrivate
    pub fn is_private(jsdoc: Option<&JSDocInfo>) -> bool {
        jsdoc.is_some_and(|jsdoc| jsdoc.get_visibility() == Visibility::PRIVATE)
    }

    // port: JsdocUtil#getUnusableTypeJSDoc
    pub fn get_unusable_type_jsdoc(
        ast: &mut Ast,
        old_jsdoc: Option<&JSDocInfo>,
    ) -> Option<Arc<JSDocInfo>> {
        Self::get_const_jsdoc(ast, old_jsdoc, "UnusableType")
    }

    // port: JsdocUtil#getQmarkTypeJSDoc
    pub fn get_qmark_type_jsdoc(ast: &mut Ast) -> Option<Arc<JSDocInfo>> {
        let qmark = ast.new_node(Token::QMARK);
        Self::make_builder_with_type(ast, None, qmark).build()
    }

    // port: JsdocUtil#makeBuilderWithType
    fn make_builder_with_type(
        ast: &mut Ast,
        old_jsdoc: Option<&JSDocInfo>,
        type_ast: NodeId,
    ) -> Builder {
        let mut builder = Builder::maybe_copy_from(old_jsdoc);
        let srcinfo = Self::syntetic_srcinfo_node(ast);
        builder.record_type(Some(Arc::new(JSTypeExpression::new(
            type_ast.srcref_tree(ast, srcinfo),
            SYNTHETIC_FILE_NAME,
        ))));
        builder
    }

    // port: JsdocUtil#replaceWithBottomType
    fn replace_with_bottom_type(
        ast: &mut Ast,
        original: Arc<JSTypeExpression>,
    ) -> Arc<JSTypeExpression> {
        if Self::is_unknown_type(ast, &original) {
            // Emitting the UnusableType can cause errors in code if overriding a property
            // or method with a more specific type. There's no harm in preserving the `?`, since it's
            // already loose in the original code.
            return original;
        }
        let type_ast = ast.new_string("IjsNoneType");
        Self::new_type_expr(ast, type_ast)
    }

    // port: JsdocUtil#replaceWithUnusableType
    fn replace_with_unusable_type(
        ast: &mut Ast,
        original: Arc<JSTypeExpression>,
    ) -> Arc<JSTypeExpression> {
        if Self::is_unknown_type(ast, &original) {
            // Emitting the UnusableType can cause errors in code if overriding a property
            // or method with a more specific type. There's no harm in preserving the `?`, since it's
            // already loose in the original code.
            return original;
        }
        let type_ast = ast.new_string("UnusableType");
        Self::new_type_expr(ast, type_ast)
    }

    // port: JsdocUtil#newTypeExpr
    fn new_type_expr(ast: &mut Ast, type_ast: NodeId) -> Arc<JSTypeExpression> {
        let srcinfo = Self::syntetic_srcinfo_node(ast);
        Arc::new(JSTypeExpression::new(
            type_ast.srcref_tree(ast, srcinfo),
            SYNTHETIC_FILE_NAME,
        ))
    }

    // port: JsdocUtil#getConstJSDoc(JSDocInfo,String)
    fn get_const_jsdoc(
        ast: &mut Ast,
        old_jsdoc: Option<&JSDocInfo>,
        contents: &str,
    ) -> Option<Arc<JSDocInfo>> {
        let type_ast = ast.new_string(contents);
        Self::get_const_jsdoc_node(ast, old_jsdoc, type_ast)
    }

    // port: JsdocUtil#getConstJSDoc(JSDocInfo,Node)
    fn get_const_jsdoc_node(
        ast: &mut Ast,
        old_jsdoc: Option<&JSDocInfo>,
        type_ast: NodeId,
    ) -> Option<Arc<JSDocInfo>> {
        let mut builder = Self::make_builder_with_type(ast, old_jsdoc, type_ast);
        builder.record_constancy();
        builder.build()
    }

    // port: JsdocUtil#markConstant
    pub fn mark_constant(old_jsdoc: Option<&JSDocInfo>) -> Option<Arc<JSDocInfo>> {
        let mut builder = Builder::maybe_copy_from(old_jsdoc);
        builder.record_constancy();
        builder.build()
    }

    // port: JsdocUtil#mergeJsdocs
    pub fn merge_jsdocs(
        classic_jsdoc: Option<Arc<JSDocInfo>>,
        inline_jsdoc: Option<&JSDocInfo>,
    ) -> Option<Arc<JSDocInfo>> {
        let Some(inline_jsdoc) = inline_jsdoc.filter(|inline_jsdoc| inline_jsdoc.has_type()) else {
            return classic_jsdoc;
        };
        let mut builder = Builder::maybe_copy_from(classic_jsdoc.as_deref());
        builder.record_type(inline_jsdoc.get_type());
        builder.build()
    }

    // port: JsdocUtil#replaceAnnotatedTypesWithUnusableType
    pub fn replace_annotated_types_with_unusable_type(
        ast: &mut Ast,
        jsdoc: &JSDocInfo,
    ) -> Option<Arc<JSDocInfo>> {
        let mut builder = JSDocInfo::builder();
        if jsdoc.has_type() {
            let t = Self::replace_with_unusable_type(ast, jsdoc.get_type().unwrap());
            builder.record_type(Some(t));
        }
        if jsdoc.has_return_type() {
            let t = Self::replace_with_unusable_type(ast, jsdoc.get_return_type().unwrap());
            builder.record_return_type(Some(t));
        }
        for i in 0..jsdoc.get_parameter_count() {
            let name = jsdoc.get_parameter_name_at(i).unwrap();
            let Some(original) = jsdoc.get_parameter_type(name.clone()) else {
                continue;
            };
            let mut new_type = Self::replace_with_bottom_type(ast, original.clone());
            if original.is_optional_arg(ast) {
                new_type = JSTypeExpression::make_optional_arg(ast, new_type);
            } else if original.is_var_args(ast) {
                new_type = JSTypeExpression::make_var_args(ast, new_type);
            }
            builder.record_parameter(name, Some(new_type));
        }
        if jsdoc.has_this_type() {
            let t = Self::replace_with_unusable_type(ast, jsdoc.get_this_type().unwrap());
            builder.record_this_type(Some(t));
        }
        // NOTE: intentionally *not* replacing @enum, @extends, @implements & @typedef. We have
        // decided that anything *declaring* a new type name should be preserved. For example -
        // given:
        //   class MyClass {}
        //   /** @private @typedef {!Foo|!Bar} */
        //   MyClass.NestedType; */
        //   /** @return {!MyClass.NestedType} */
        //   MyClass.SomeFn = function() { // ...
        // we choose to preserve the definition of `MyClass.NestedType` in order to make
        // `MyClass.SomeFn` well-typed.

        // Preserve existing @const or @private/@package/@protected annotations.
        if jsdoc.is_constant() {
            builder.record_constancy();
        }
        // Java `jsdoc.getVisibility() != null`: getVisibility defaults to INHERITED, never null.
        builder.record_visibility(jsdoc.get_visibility());
        builder.build()
    }

    // port: JsdocUtil#hasAnnotatedType
    pub fn has_annotated_type(jsdoc: Option<&JSDocInfo>) -> bool {
        let Some(jsdoc) = jsdoc else {
            return false;
        };
        jsdoc.has_type()
            || jsdoc.has_return_type()
            || jsdoc.get_parameter_count() > 0
            || jsdoc.is_constructor_or_interface()
            || jsdoc.has_typedef_type()
            || jsdoc.has_this_type()
            || jsdoc.has_enum_parameter_type()
    }

    // port: JsdocUtil#getJSDocForRhs
    pub fn get_jsdoc_for_rhs(
        ast: &mut Ast,
        rhs: NodeId,
        old_jsdoc: Option<&JSDocInfo>,
    ) -> Option<Arc<JSDocInfo>> {
        match NodeUtil::get_known_value_type(ast, rhs) {
            ValueType::BOOLEAN => {
                return Self::get_const_jsdoc(ast, old_jsdoc, "boolean");
            }
            ValueType::NUMBER => {
                return Self::get_const_jsdoc(ast, old_jsdoc, "number");
            }
            ValueType::BIGINT => {
                return Self::get_const_jsdoc(ast, old_jsdoc, "bigint");
            }
            ValueType::STRING => {
                return Self::get_const_jsdoc(ast, old_jsdoc, "string");
            }
            ValueType::NULL => {
                return Self::get_const_jsdoc(ast, old_jsdoc, "null");
            }
            ValueType::VOID => {
                return Self::get_const_jsdoc(ast, old_jsdoc, "void");
            }
            ValueType::OBJECT => {
                if rhs.is_reg_exp(ast) {
                    let reg_exp = IR::string(ast, "RegExp");
                    let bang = ast.new_node_with_child(Token::BANG, reg_exp);
                    return Self::get_const_jsdoc_node(ast, old_jsdoc, bang);
                }
            }
            ValueType::UNDETERMINED => {
                if old_jsdoc.is_some_and(|old_jsdoc| old_jsdoc.get_description().is_some()) {
                    return Self::get_const_jsdoc(ast, old_jsdoc, "string");
                }
            }
        }
        if rhs.is_cast(ast) {
            let root = rhs
                .get_jsdoc_info(ast)
                .unwrap()
                .get_type()
                .unwrap()
                .get_root();
            return Self::get_const_jsdoc_node(ast, old_jsdoc, root);
        }
        None
    }

    // port: JsdocUtil#getJSDocForName
    pub fn get_jsdoc_for_name(
        compiler: &mut AbstractCompiler,
        decl: Option<VarId>,
        old_jsdoc: Option<&JSDocInfo>,
    ) -> Option<Arc<JSDocInfo>> {
        let decl = decl?;
        let name_node = decl.get_name_node(compiler).unwrap();
        let expr = NodeUtil::get_declared_type_expression(compiler, name_node)?;
        let mut type_ast = expr.get_root();
        match type_ast.get_token(compiler) {
            Token::EQUALS => {
                let mut type_root = type_ast
                    .get_first_child(compiler)
                    .unwrap()
                    .clone_tree(compiler);
                if !decl.is_default_param(compiler) {
                    let undefined = IR::string(compiler, "undefined");
                    type_root = compiler.new_node_with_children2(Token::PIPE, type_root, undefined);
                }
                type_ast = type_root;
            }
            Token::ITER_REST => {
                let new_type = compiler.new_node(Token::BANG);
                let array = IR::string(compiler, "Array");
                new_type.add_child_to_back(compiler, array);
                let element = type_ast
                    .get_first_child(compiler)
                    .unwrap()
                    .clone_tree(compiler);
                let block = compiler.new_node_with_child(Token::BLOCK, element);
                array.add_child_to_back(compiler, block);
                type_ast = new_type;
            }
            _ => {}
        }
        Self::get_const_jsdoc_node(compiler, old_jsdoc, type_ast)
    }

    // port: JsdocUtil#isUnknownType
    fn is_unknown_type(ast: &Ast, r#type: &JSTypeExpression) -> bool {
        let type_ast = r#type.get_root();
        type_ast.get_token(ast) == Token::QMARK && !type_ast.has_children(ast)
    }
}
