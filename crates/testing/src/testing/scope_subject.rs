/*
 * Copyright 2015 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/testing/ScopeSubject.java.

//! Port of testing/ScopeSubject.java: a Truth Subject for the AbstractScope class. Usage:
//!
//! ```text
//!   assert_scope(compiler, scope).declares("somevar");
//!   assert_scope(compiler, scope).declares("otherVar").directly();
//!   assert_scope(compiler, scope).declares("yetAnotherVar").on_closest_container_scope();
//! ```
//!
//! Rust-only: the scope is an arena handle, so the subject borrows the compiler that owns it.
use closure_jscomp::{
    abstract_compiler::AbstractCompiler, abstract_scope::AbstractScope, abstract_var::AbstractVar,
    typed_scope::TypedScope,
};
use closure_jstype::testing::type_subject::TypeSubject;
use closure_rhino::{check_not_null, check_state, js_string::JsString, node::NodeId};

/// Truth's `failWithoutActual(facts...)`: each fact is a `key: value` line (or a bare key).
fn fail_without_actual(facts: &[(&str, Option<String>)]) -> ! {
    let message = facts
        .iter()
        .map(|(key, value)| match value {
            Some(value) => format!("{key}: {value}"),
            None => (*key).to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    panic!("{message}")
}

// port: ScopeSubject
pub struct ScopeSubject<'a, S: AbstractScope> {
    compiler: &'a mut AbstractCompiler,
    actual: S,
}

// port: ScopeSubject#assertScope
pub fn assert_scope<S: AbstractScope>(
    compiler: &mut AbstractCompiler,
    scope: S,
) -> ScopeSubject<'_, S> {
    ScopeSubject::assert_scope(compiler, scope)
}

impl<'a, S: AbstractScope> ScopeSubject<'a, S> {
    // port: ScopeSubject#assertScope
    pub fn assert_scope(compiler: &'a mut AbstractCompiler, scope: S) -> Self {
        Self::new(compiler, scope)
    }

    // port: ScopeSubject#ScopeSubject
    fn new(compiler: &'a mut AbstractCompiler, scope: S) -> Self {
        Self {
            compiler,
            actual: scope,
        }
    }

    // port: ScopeSubject#doesNotDeclare
    pub fn does_not_declare(&mut self, name: &str) {
        let var = self.get_var(name);
        if let Some(var) = var {
            fail_without_actual(&[
                ("expected not to declare", Some(name.to_owned())),
                (
                    "but declared it with value",
                    Some(var.to_string(self.compiler)),
                ),
                ("scope was", Some(self.actual.to_string(self.compiler))),
            ]);
        }
    }

    // port: ScopeSubject#declares
    pub fn declares(&mut self, name: &str) -> DeclarationSubject<'_, S> {
        let var = self.get_var(name);
        let Some(var) = var else {
            let declared = self.actual.get_all_accessible_variables(self.compiler);
            let mut names: Vec<String> = declared
                .iter()
                .map(|var| var.get_name(self.compiler).to_string())
                .collect();
            if names.len() > 10 {
                let others = format!("and {} others", names.len() - 9);
                names.truncate(9);
                names.push(others);
            }
            fail_without_actual(&[
                ("expected to declare", Some(name.to_owned())),
                ("but did not", None),
                ("did declare", Some(names.join(", "))),
                ("scope was", Some(self.actual.to_string(self.compiler))),
            ]);
        };
        let topmost = self
            .actual
            .get_topmost_scope_of_eventual_declaration(self.compiler, &JsString::from(name));
        assert!(
            topmost == var.get_scope(self.compiler),
            "value of: getTopmostScopeOfEventualDeclaration({name})\nexpected to be equal to var.getScope()"
        );
        DeclarationSubject {
            compiler: &mut *self.compiler,
            actual: self.actual,
            var,
        }
    }

    // port: ScopeSubject#getVar
    fn get_var(&mut self, name: &str) -> Option<S::Var> {
        let name = JsString::from(name);
        if self.actual.has_slot(self.compiler, &name) {
            Some(check_not_null!(self.actual.get_var(self.compiler, &name)))
        } else {
            None
        }
    }
}

/// A subject for an `AbstractVar` declared by this particular scope.
// port: ScopeSubject.DeclarationSubject
pub struct DeclarationSubject<'a, S: AbstractScope> {
    compiler: &'a mut AbstractCompiler,
    /// The enclosing ScopeSubject's `actual`.
    actual: S,
    var: S::Var,
}

impl<S: AbstractScope> DeclarationSubject<'_, S> {
    /// Expects the variable to be defined on the given `scope`. The `preposition` is either "on"
    /// or "", depending on whether it is needed for grammatical correctness. The `expected` object
    /// is displayed in brackets, in parallel to the actual scope that the variable is defined on.
    // port: ScopeSubject.DeclarationSubject#expectScope
    fn expect_scope(&self, preposition: &str, expected: String, scope: Option<S>) {
        let var_scope = self.var.get_scope(self.compiler);
        if var_scope != scope {
            fail_without_actual(&[
                (
                    "for var",
                    Some(self.var.get_name(self.compiler).to_string()),
                ),
                (
                    &format!(
                        "expected to be declared{}{}",
                        if !preposition.is_empty() { " " } else { "" },
                        preposition
                    ),
                    Some(expected),
                ),
                (
                    "but is declared on",
                    Some(
                        var_scope.map_or_else(
                            || "null".to_owned(),
                            |scope| scope.to_string(self.compiler),
                        ),
                    ),
                ),
            ]);
        }
    }

    /// Expects the declared variable to be declared on the subject scope.
    // port: ScopeSubject.DeclarationSubject#directly
    pub fn directly(&mut self) -> &mut Self {
        self.expect_scope("", "directly".to_owned(), Some(self.actual));
        self
    }

    /// Expects the declared variable to be declared on the given scope.
    // port: ScopeSubject.DeclarationSubject#on
    pub fn on(&mut self, scope: S) -> &mut Self {
        check_state!(
            scope != self.actual,
            "It doesn't make sense to pass the scope already being asserted about. Use .directly()"
        );
        self.expect_scope("on", scope.to_string(self.compiler), Some(scope));
        self
    }

    /// Expects the declared variable to be declared on the closest container scope.
    // port: ScopeSubject.DeclarationSubject#onClosestContainerScope
    pub fn on_closest_container_scope(&mut self) -> &mut Self {
        let scope = self.actual.get_closest_container_scope(self.compiler);
        self.expect_scope("on", "the closest container scope".to_owned(), Some(scope));
        self
    }

    /// Expects the declared variable to be declared on the closest hoist scope.
    // port: ScopeSubject.DeclarationSubject#onClosestHoistScope
    pub fn on_closest_hoist_scope(&mut self) -> &mut Self {
        let scope = self.actual.get_closest_hoist_scope(self.compiler);
        self.expect_scope("on", "the closest hoist scope".to_owned(), scope);
        self
    }

    /// Expects the declared variable to be declared on the global scope.
    // port: ScopeSubject.DeclarationSubject#globally
    pub fn globally(&mut self) -> &mut Self {
        let scope = self.actual.get_global_scope(self.compiler);
        self.expect_scope("", "globally".to_owned(), Some(scope));
        self
    }

    /// Expects the declared variable to be declared on any scope other than the subject.
    // port: ScopeSubject.DeclarationSubject#onSomeParent
    pub fn on_some_parent(&mut self) -> &mut Self {
        if self.var.get_scope(self.compiler) == Some(self.actual) {
            fail_without_actual(&[
                (
                    "for var",
                    Some(self.var.get_name(self.compiler).to_string()),
                ),
                ("expected a declaration on a parent scope", None),
                ("but found it declared directly", None),
                ("scope was", Some(self.actual.to_string(self.compiler))),
            ]);
        }
        self
    }

    /// Expects the declared variable to be declared on some scope with the given label.
    // port: ScopeSubject.DeclarationSubject#onScopeLabeled
    pub fn on_scope_labeled(&mut self, expected_label: &str) -> &mut Self {
        let actual_label = get_label(self.compiler, self.var.get_scope_root(self.compiler));
        match actual_label {
            None => fail_without_actual(&[
                (
                    "expected to declare",
                    Some(self.var.get_name(self.compiler).to_string()),
                ),
                ("on a scope labeled", Some(expected_label.to_owned())),
                ("but declared it on an unlabeled scope", None),
                (
                    "scope under test was",
                    Some(self.actual.to_string(self.compiler)),
                ),
            ]),
            Some(actual_label) if actual_label != expected_label => fail_without_actual(&[
                (
                    "expected to declare",
                    Some(self.var.get_name(self.compiler).to_string()),
                ),
                ("on a scope labeled", Some(expected_label.to_owned())),
                ("but declared it on an scope labeled", Some(actual_label)),
                (
                    "scope under test was",
                    Some(self.actual.to_string(self.compiler)),
                ),
            ]),
            Some(_) => {}
        }
        self
    }
}

impl DeclarationSubject<'_, TypedScope> {
    /// Java casts `var.getSymbol()` to TypedVar; only a TypedScope's vars are TypedVars.
    // port: ScopeSubject.DeclarationSubject#withTypeThat
    pub fn with_type_that(&self) -> TypeSubject {
        let typed_var = self.var.get_symbol(self.compiler);
        TypeSubject::assert_type(typed_var.get_type(self.compiler))
    }
}

/// Returns the name of the label applied to n, or null if none exists.
// port: ScopeSubject#getLabel
fn get_label(compiler: &AbstractCompiler, n: NodeId) -> Option<String> {
    // If the node is labeled it will be the second child of a LABEL and the first child
    // will be a LABEL_NAME.
    let parent = n.get_parent(compiler);
    if let Some(parent) = parent.filter(|parent| parent.is_label(compiler)) {
        let label_name_node = parent.get_first_child(compiler).unwrap();
        check_state!(
            label_name_node.is_label_name(compiler),
            "%s",
            label_name_node.to_string(compiler)
        );
        check_state!(
            label_name_node.get_next(compiler) == Some(n),
            "%s",
            n.to_string(compiler)
        );
        Some(label_name_node.get_string(compiler).to_string())
    } else {
        None
    }
}
