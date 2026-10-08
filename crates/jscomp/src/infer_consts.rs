/*
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
//   src/com/google/javascript/jscomp/InferConsts.java.

//! Port of `InferConsts.java`.

use crate::abstract_compiler::AbstractCompiler;
use crate::compiler_pass::CompilerPass;
use crate::reference_collection::ReferenceCollection;
use crate::reference_collector::ReferenceCollector;
use crate::syntactic_scope_creator::SyntacticScopeCreator;
use crate::var::VarId;
use closure_rhino::node::NodeId;
use closure_rhino::token::Token;

/// Attaches the CONST_VAR annotation to any variable that's
/// 1) Provably well-defined and assigned once in its lifetime.
/// 2) Annotated 'const'
/// 3) Declared with the 'const' keyword.
///
/// These 3 are considered semantically equivalent. Notice that a variable in a loop is never
/// considered const.
///
/// Note that criteria (1) is only used for normal code, not externs.
#[derive(Debug, Default)]
pub struct InferConsts;

impl InferConsts {
    // port: InferConsts#InferConsts
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self
    }

    // port: InferConsts#considerVar
    fn consider_var(
        &mut self,
        compiler: &mut AbstractCompiler,
        v: VarId,
        ref_collection: Option<&ReferenceCollection>,
    ) {
        if v.is_implicit_goog_namespace(compiler) {
            return; // no name node for provided variables.
        }

        let name_node = v.get_name_node(compiler);
        let doc_info = v.get_jsdoc_info(compiler);
        if doc_info.is_some_and(|doc_info| doc_info.is_constant()) {
            name_node
                .expect("InferConsts#considerVar nameNode")
                .set_declared_constant_var(compiler, true);
        } else if name_node
            .is_some_and(|name_node| name_node.get_parent(compiler).unwrap().is_const(compiler))
        {
            name_node.unwrap().set_declared_constant_var(compiler, true);
        }

        if Self::is_inferred_const(compiler, v, ref_collection) {
            name_node.unwrap().set_inferred_constant_var(compiler, true);
        }
    }

    // port: InferConsts#isInferredConst
    fn is_inferred_const(
        compiler: &mut AbstractCompiler,
        v: VarId,
        ref_collection: Option<&ReferenceCollection>,
    ) -> bool {
        let name_node = v.get_name_node(compiler);
        let (Some(name_node), Some(ref_collection)) = (name_node, ref_collection) else {
            return false;
        };
        if !ref_collection.is_assigned_once_in_lifetime(compiler) {
            return false;
        }

        if v.is_implicit_goog_namespace(compiler) {
            return false;
        }

        let declaration_type = v.declaration_type(compiler);
        match declaration_type {
            Some(Token::LET) => {
                // Check that non-destructuring let names are actually assigned at declaration.
                !name_node.get_parent(compiler).unwrap().is_let(compiler)
                    || name_node.has_children(compiler)
            }
            Some(
                Token::CONST
                | Token::CATCH
                | Token::CLASS
                | Token::PARAM_LIST // Parameters cannot be referenced before declaration.
                | Token::FUNCTION, // Function hoisting means no references before declaration.
            ) => true,
            Some(Token::IMPORT) => {
                // ES module exports are mutable.
                // TODO(lharker): make this check smarter if we start optimizing unrewritten
                // modules.
                false
            }
            Some(Token::VAR) => {
                // var hoisting requires this extra work to make sure the 'declaration' is also the
                // first reference.
                ref_collection.first_reference_is_assigning_declaration(compiler)
                    && ref_collection.is_well_defined(compiler)
            }
            // Java's switch on a null enum throws NullPointerException before any case.
            None => panic!("NullPointerException: InferConsts#isInferredConst declarationType"),
            Some(declaration_type) => {
                panic!("Unrecognized declaration type {declaration_type}")
            }
        }
    }
}

impl CompilerPass for InferConsts {
    // port: InferConsts#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, js: NodeId) {
        let mut scope_creator = SyntacticScopeCreator::new();
        let mut collector = ReferenceCollector::new(
            compiler,
            ReferenceCollector::DO_NOTHING_BEHAVIOR,
            &mut scope_creator,
        );
        collector.process(compiler, js);

        for v in collector.get_all_symbols() {
            self.consider_var(compiler, v, collector.get_references(v));
        }

        let global_externs_scope =
            SyntacticScopeCreator::new().create_scope(compiler, externs, None);
        for v in global_externs_scope.get_all_symbols(compiler) {
            self.consider_var(compiler, v, None);
        }
    }
}
