/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/NodeTraversal.java,
//   src/com/google/javascript/jscomp/lint/CheckEs6ModuleFileStructure.java.

//! Checks the file structure of ES6 modules.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic_type::DiagnosticType,
    node_traversal::{Callback, NodeTraversal},
};
use closure_rhino::{
    node::{NodeId, Prop},
    qualified_name::QualifiedName,
    token::Token,
};
use std::{
    collections::BTreeSet,
    fmt,
    ops::Bound::{Excluded, Unbounded},
    sync::LazyLock,
};

// port: CheckEs6ModuleFileStructure#MUST_COME_BEFORE
pub static MUST_COME_BEFORE: DiagnosticType = DiagnosticType::warning(
    "JSC_MUST_COME_BEFORE_IN_ES6_MODULE",
    "In ES6 modules, {0} should come before {1}.",
);

/// Statements that must appear in a certain order within ES6 modules (for the sake of style).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum OrderedStatement {
    IMPORT,
    DECLARE_MODULE_ID,
    GOOG_REQUIRE,
    OTHER,
}

impl OrderedStatement {
    // port: CheckEs6ModuleFileStructure.OrderedStatement#description
    fn description(self) -> &'static str {
        match self {
            Self::IMPORT => "import statements",
            Self::DECLARE_MODULE_ID => "a call to goog.declareModuleId()",
            Self::GOOG_REQUIRE => "calls to goog.require()",
            Self::OTHER => "other statements",
        }
    }
}

impl fmt::Display for OrderedStatement {
    // port: CheckEs6ModuleFileStructure.OrderedStatement#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}

// port: CheckEs6ModuleFileStructure#GOOG_DECLAREMODULEID
static GOOG_DECLAREMODULEID: LazyLock<QualifiedName> =
    LazyLock::new(|| QualifiedName::of("goog.declareModuleId"));
// port: CheckEs6ModuleFileStructure#GOOG_REQUIRE
static GOOG_REQUIRE: LazyLock<QualifiedName> = LazyLock::new(|| QualifiedName::of("goog.require"));

pub struct CheckEs6ModuleFileStructure {
    /// Java `TreeSet<OrderedStatement>`: natural (ordinal) order.
    ordered_statements: BTreeSet<OrderedStatement>,
}

impl CheckEs6ModuleFileStructure {
    // port: CheckEs6ModuleFileStructure#CheckEs6ModuleFileStructure
    pub fn new(_compiler: &AbstractCompiler) -> Self {
        Self {
            ordered_statements: BTreeSet::new(),
        }
    }

    // port: CheckEs6ModuleFileStructure#checkOrder
    fn check_order(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, statement: OrderedStatement) {
        self.ordered_statements.insert(statement);
        let out_of_order: Vec<OrderedStatement> = self
            .ordered_statements
            .range((Excluded(statement), Unbounded))
            .copied()
            .collect();
        if !out_of_order.is_empty() {
            let joined = out_of_order
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            t.report(n, &MUST_COME_BEFORE, &[&statement.to_string(), &joined]);
        }
    }

    // port: CheckEs6ModuleFileStructure#visitExprResult
    fn visit_expr_result(
        &mut self,
        t: &mut NodeTraversal<'_>,
        expr_result: NodeId,
        parent: NodeId,
    ) -> bool {
        if parent.is_module_body(t) && expr_result.get_first_child(t).unwrap().is_call(t) {
            let call = expr_result.get_first_child(t).unwrap();
            if GOOG_DECLAREMODULEID.matches(t, call.get_first_child(t).unwrap()) {
                self.check_order(t, call, OrderedStatement::DECLARE_MODULE_ID);
                return false;
            } else if GOOG_REQUIRE.matches(t, call.get_first_child(t).unwrap()) {
                self.check_order(t, call, OrderedStatement::GOOG_REQUIRE);
                return false;
            }
        }

        self.check_order(t, expr_result, OrderedStatement::OTHER);
        true
    }

    // port: CheckEs6ModuleFileStructure#visitDeclaration
    fn visit_declaration(
        &mut self,
        t: &mut NodeTraversal<'_>,
        declaration: NodeId,
        parent: NodeId,
    ) -> bool {
        if parent.is_module_body(t)
            && declaration.has_one_child(t)
            && declaration.get_first_child(t).unwrap().has_one_child(t)
            && declaration.get_first_first_child(t).unwrap().is_call(t)
        {
            let call = declaration.get_first_first_child(t).unwrap();
            if GOOG_REQUIRE.matches(t, call.get_first_child(t).unwrap()) {
                self.check_order(t, call, OrderedStatement::GOOG_REQUIRE);
                return false;
            }
        }

        self.check_order(t, declaration, OrderedStatement::OTHER);
        true
    }

    // port: CheckEs6ModuleFileStructure#visitImport
    fn visit_import(&mut self, t: &mut NodeTraversal<'_>, import_node: NodeId) {
        self.check_order(t, import_node, OrderedStatement::IMPORT);
    }
}

impl CompilerPass for CheckEs6ModuleFileStructure {
    // port: CheckEs6ModuleFileStructure#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckEs6ModuleFileStructure {
    // port: CheckEs6ModuleFileStructure#shouldTraverse
    fn should_traverse(
        &mut self,
        t: &mut NodeTraversal<'_>,
        n: NodeId,
        parent: Option<NodeId>,
    ) -> bool {
        match n.get_token(t) {
            Token::ROOT => true,
            Token::SCRIPT => n.get_boolean_prop(t, Prop::ES6_MODULE),
            Token::MODULE_BODY => {
                self.ordered_statements.clear();
                true
            }
            Token::IMPORT => {
                self.visit_import(t, n);
                false
            }
            Token::EXPR_RESULT => self.visit_expr_result(t, n, parent.unwrap()),
            Token::VAR | Token::LET | Token::CONST => self.visit_declaration(t, n, parent.unwrap()),
            _ => {
                if let Some(parent) = parent
                    && parent.is_module_body(t)
                {
                    self.check_order(t, n, OrderedStatement::OTHER);
                }
                false
            }
        }
    }

    // port: NodeTraversal.AbstractPreOrderCallback#visit
    fn visit(&mut self, _t: &mut NodeTraversal<'_>, _n: NodeId, _parent: Option<NodeId>) {}
}
