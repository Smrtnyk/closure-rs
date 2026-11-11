/*
 * Copyright 2026 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit ddfede7:
//   src/com/google/javascript/jscomp/ClosureExportTypeClassifier.java.

//! Syntactically classifies whether each goog.module namespace in a file is seen from TypeScript
//! as a set of "named" exports or as a single "default" export.
//!
//! This approximates the type-based logic originally implemented in Clutz. So most of the
//! (potentially surprising) logic is in place to mirror Clutz & allow migration to this syntactic
//! classifier.
//!
//! The core rules:
//!
//! - Object literals with at least one non-computed, non-spread key are NAMED.
//! - Properties assigned or declared (`/** @type {T} */ exports.x;`) on `exports`, or on a local
//!   alias of it, make an empty object NAMED.
//! - `exports = {a, b};` in a goog.module is NAMED even with a declared type, since module
//!   rewriting turns it into `exports.a = a; exports.b = b;` and drops the JSDoc.
//!
//! Users can make a namespace `named` with `@legacyNamedTsExport` on its `goog.module` call. The
//! annotation has no effect elsewhere.

#![allow(clippy::collapsible_match)] // Keep Java's control flow.
use crate::node_util::NodeUtil;
use closure_rhino::check_argument;
use closure_rhino::fast_hash::{IndexMap, IndexSet};
use closure_rhino::js_string::JsString;
use closure_rhino::js_type_expression::JSTypeExpression;
use closure_rhino::jsdoc_info::JSDocInfo;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::token::Token;
use std::sync::Arc;

// port: ClosureExportTypeClassifier#EXPORTS
const EXPORTS: &str = "exports";

/// How many local aliases to follow, e.g. `const a = {}; const b = a; exports = b;`.
///
/// Clutz follows aliases without a limit; this is an arbitrary cap to keep the work bounded.
/// Comparing against Clutz on a large codebase found no namespace that needs a longer chain.
// port: ClosureExportTypeClassifier#MAX_ALIAS_DEPTH
const MAX_ALIAS_DEPTH: i32 = 4;

/// The syntactic "shape" of a value, as relevant for determining its export type.
// port: ClosureExportTypeClassifier.Kind
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// Classes, functions, primitives, etc.
    NOT_NAMESPACE,
    /// An object with no known properties, like `{}`. May be treated as either NAMED or DEFAULT
    /// depending on whether properties are added to it later on in the file.
    EMPTY_OBJECT,
    /// An object with at least one property.
    NAMESPACE,
}

/// Facts gathered about a top-level name: `exports` or a local variable.
// port: ClosureExportTypeClassifier.NameInfo
#[derive(Default)]
struct NameInfo {
    /// The last value assigned to this name, if any.
    value: Option<NodeId>,

    /// The JSDoc on the declaration or assignment of `value`.
    jsdoc: Option<Arc<JSDocInfo>>,

    /// Whether this name is declared or assigned in a way that is never a namespace (class,
    /// function, or stub typedef).
    is_never_namespace: bool,

    /// Number of properties assigned or declared on this name (`name.x = ...;`).
    property_count: i32,

    /// Whether this is a goog.module's `exports`.
    is_goog_module_exports: bool,

    /// Whether `value` is assigned as `exports = {a, b};` in a goog.module. JSCompiler's module
    /// rewriting turns that into `exports.a = a; exports.b = b;` (see
    /// `NodeUtil#isNamedExportsLiteral`), dropping any JSDoc on the assignment, so it's always
    /// NAMED.
    is_named_exports_literal: bool,
}

// port: ClosureExportTypeClassifier.TsExportType
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TsExportType {
    /// The module exports a single default value.
    DEFAULT,
    /// The module exports multiple named values.
    NAMED,
}

// port: ClosureExportTypeClassifier
#[derive(Default)]
pub struct ClosureExportTypeClassifier {
    // Java's LinkedHashMap.
    top_level_names: IndexMap<JsString, NameInfo>,

    /// Namespaces whose goog.module call is annotated `@legacyNamedTsExport`.
    forced_named_export: IndexSet<JsString>,
}

impl ClosureExportTypeClassifier {
    /// Classifies a single `goog.module` namespace as named or default.
    ///
    /// `module_root` is the SCRIPT node for this module, `goog_namespace` the namespace of this
    /// module.
    // port: ClosureExportTypeClassifier#classify
    pub fn classify(ast: &Ast, module_root: NodeId, goog_namespace: &str) -> TsExportType {
        let mut classifier = ClosureExportTypeClassifier::default();
        classifier.add_goog_module_exports();
        Self::for_each_top_level_statement(ast, module_root, |statement| {
            classifier.visit_statement(ast, statement)
        });

        let info = &classifier.top_level_names[&JsString::from(EXPORTS)];
        if classifier
            .forced_named_export
            .contains(&JsString::from(goog_namespace))
            || classifier.kind_of_name(ast, info, 0) == Kind::NAMESPACE
        {
            return TsExportType::NAMED;
        }
        TsExportType::DEFAULT
    }

    // port: ClosureExportTypeClassifier#addGoogModuleExports
    fn add_goog_module_exports(&mut self) {
        let info = NameInfo {
            is_goog_module_exports: true,
            ..NameInfo::default()
        };
        self.top_level_names.insert(JsString::from(EXPORTS), info);
    }

    // ---------------------------------------------------------------------------------------------
    // Gathering facts from top-level statements.
    // ---------------------------------------------------------------------------------------------

    /// Calls `callback` on each top-level statement of the module.
    // port: ClosureExportTypeClassifier#forEachTopLevelStatement
    fn for_each_top_level_statement(
        ast: &Ast,
        module_root: NodeId,
        mut callback: impl FnMut(NodeId),
    ) {
        check_argument!(
            module_root.is_script(ast)
                && module_root
                    .get_first_child(ast)
                    .expect("NullPointerException")
                    .is_module_body(ast),
            "%s",
            module_root.to_string(ast)
        );
        let body = module_root.get_first_child(ast).unwrap();
        let mut statement = body.get_first_child(ast);
        while let Some(s) = statement {
            callback(s);
            statement = s.get_next(ast);
        }
    }

    // port: ClosureExportTypeClassifier#visitStatement
    fn visit_statement(&mut self, ast: &Ast, statement: NodeId) {
        match statement.get_token(ast) {
            Token::VAR | Token::LET | Token::CONST => self.visit_name_declaration(ast, statement),
            Token::CLASS | Token::FUNCTION => {
                let name = statement.get_first_child(ast).unwrap();
                let info = self.get_or_create(name.get_string(ast));
                info.is_never_namespace = true;
            }
            Token::EXPR_RESULT => {
                self.maybe_record_legacy_named_ts_export(ast, statement);
                self.visit_expression(ast, statement.get_first_child(ast).unwrap());
            }
            _ => {}
        }
    }

    /// Records an explicit @legacyNamedTsExport annotation on a goog.module call statement.
    // port: ClosureExportTypeClassifier#maybeRecordLegacyNamedTsExport
    fn maybe_record_legacy_named_ts_export(&mut self, ast: &Ast, statement: NodeId) {
        if !NodeUtil::is_goog_module_call(ast, statement) {
            return;
        }
        let call = statement.get_first_child(ast).unwrap();
        let jsdoc = call.get_jsdoc_info(ast);
        if !jsdoc.is_some_and(|jsdoc| jsdoc.is_legacy_named_ts_export()) {
            return;
        }
        self.forced_named_export
            .insert(call.get_second_child(ast).unwrap().get_string(ast));
    }

    // port: ClosureExportTypeClassifier#visitNameDeclaration
    fn visit_name_declaration(&mut self, ast: &Ast, declaration: NodeId) {
        let mut lhs = declaration.get_first_child(ast);
        while let Some(l) = lhs {
            if l.is_name(ast) {
                self.visit_declared_name(ast, l);
            }
            lhs = l.get_next(ast);
        }
    }

    // port: ClosureExportTypeClassifier#visitDeclaredName
    fn visit_declared_name(&mut self, ast: &Ast, lhs: NodeId) {
        let initializer = lhs.get_first_child(ast);
        let name = lhs.get_string(ast);
        // Java keeps the NameInfo object across the visit of the initializer; it is looked up by
        // name again here (the map never drops an entry, so it is the same object).
        self.get_or_create(name.clone());
        if let Some(initializer) = initializer
            && initializer.is_assign(ast)
        {
            self.visit_expression(ast, initializer);
        }
        let jsdoc = NodeUtil::get_best_jsdoc_info(ast, lhs);
        let info = self.get_or_create(name);
        if initializer.is_some() {
            info.value = initializer;
            info.jsdoc = jsdoc;
        } else if info.value.is_none() && jsdoc.is_some() {
            // A stub like `/** @type {T} */ let x;`. A bare redeclaration like `var x;` keeps any
            // earlier value.
            info.jsdoc = jsdoc;
        }
    }

    // port: ClosureExportTypeClassifier#visitExpression
    fn visit_expression(&mut self, ast: &Ast, expr: NodeId) {
        match expr.get_token(ast) {
            Token::ASSIGN => self.visit_assign(ast, expr),
            Token::GETPROP => {
                // Stub declarations like `/** @typedef {number} */ exports.T;`
                if expr.get_jsdoc_info(ast).is_some()
                    && expr.get_first_child(ast).unwrap().is_name(ast)
                {
                    let variable_name = expr.get_first_child(ast).unwrap().get_string(ast);
                    let info = self.get_or_create(variable_name);
                    info.property_count = info.property_count.wrapping_add(1);
                }
            }
            _ => {}
        }
    }

    // port: ClosureExportTypeClassifier#visitAssign
    fn visit_assign(&mut self, ast: &Ast, expr: NodeId) {
        let lhs = expr.get_first_child(ast).unwrap();
        let rhs = expr.get_second_child(ast).unwrap();
        let jsdoc = expr.get_jsdoc_info(ast);
        if lhs.is_name(ast) {
            // `exports = ...;` or `localName = ...;`
            let assigned = self.get_or_create(lhs.get_string(ast));
            assigned.value = Some(rhs);
            assigned.jsdoc = jsdoc;
            assigned.is_never_namespace = false;
            assigned.is_named_exports_literal =
                assigned.is_goog_module_exports && NodeUtil::is_named_exports_literal(ast, rhs);
            return;
        }
        if lhs.is_get_prop(ast) && lhs.get_first_child(ast).unwrap().is_name(ast) {
            // `exports.x = ...;` or `localName.x = ...;`
            let info = self.get_or_create(lhs.get_first_child(ast).unwrap().get_string(ast));
            info.property_count = info.property_count.wrapping_add(1);
        }
    }

    // port: ClosureExportTypeClassifier#getOrCreate
    fn get_or_create(&mut self, name: JsString) -> &mut NameInfo {
        self.top_level_names.entry(name).or_default()
    }

    // ---------------------------------------------------------------------------------------------
    // Classifying values.
    // ---------------------------------------------------------------------------------------------

    // port: ClosureExportTypeClassifier#kindOfName
    fn kind_of_name(&self, ast: &Ast, info: &NameInfo, depth: i32) -> Kind {
        if info.is_never_namespace {
            return Kind::NOT_NAMESPACE;
        }
        if info.is_named_exports_literal {
            return Kind::NAMESPACE;
        }
        let kind = if info.value.is_some() || info.jsdoc.is_some() {
            self.kind_of_value(ast, info.value, info.jsdoc.as_deref(), depth)
        } else if info.is_goog_module_exports {
            Kind::EMPTY_OBJECT
        } else {
            Kind::NOT_NAMESPACE
        };
        if kind == Kind::EMPTY_OBJECT && info.property_count > 0 {
            return Kind::NAMESPACE;
        }
        kind
    }

    // port: ClosureExportTypeClassifier#kindOfValue
    fn kind_of_value(
        &self,
        ast: &Ast,
        value: Option<NodeId>,
        jsdoc: Option<&JSDocInfo>,
        depth: i32,
    ) -> Kind {
        if let Some(jsdoc) = jsdoc {
            if jsdoc.has_enum_parameter_type()
                || jsdoc.has_typedef_type()
                || jsdoc.is_constructor_or_interface()
            {
                return Kind::NOT_NAMESPACE;
            }
            if jsdoc.has_type() {
                return Self::kind_of_declared_type(ast, &jsdoc.get_type().unwrap());
            }
        }
        let Some(value) = value else {
            return Kind::NOT_NAMESPACE;
        };
        match value.get_token(ast) {
            Token::OBJECTLIT => {
                // Computed properties and spreads do not create named own properties.
                let mut member = value.get_first_child(ast);
                while let Some(m) = member {
                    if !m.is_computed_prop(ast) && !m.is_spread(ast) {
                        return Kind::NAMESPACE;
                    }
                    member = m.get_next(ast);
                }
                Kind::EMPTY_OBJECT
            }
            Token::NAME => {
                if depth >= MAX_ALIAS_DEPTH {
                    return Kind::NOT_NAMESPACE;
                }
                match self.top_level_names.get(&value.get_string(ast)) {
                    None => Kind::NOT_NAMESPACE,
                    Some(aliased) => self.kind_of_name(ast, aliased, depth + 1),
                }
            }
            Token::CAST => self.kind_of_value(
                ast,
                value.get_first_child(ast),
                value.get_jsdoc_info(ast).as_deref(),
                depth,
            ),
            // Classes, functions, calls, `new`, literals, expressions, etc.
            _ => Kind::NOT_NAMESPACE,
        }
    }

    /// Only record type literals like `{a: number}` are namespaces. Any named type (e.g.
    /// `!Object<string, number>` or `Foo`) is an instance type or a typedef, which the syntactic
    /// analysis cannot resolve, so is not a namespace.
    // port: ClosureExportTypeClassifier#kindOfDeclaredType
    fn kind_of_declared_type(ast: &Ast, type_: &JSTypeExpression) -> Kind {
        let root = type_.get_root();
        if root.get_token(ast) == Token::LC {
            Kind::NAMESPACE
        } else {
            Kind::NOT_NAMESPACE
        }
    }
}
