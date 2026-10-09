/*
 * Copyright 2008 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/CheckAccessControls.java.

//! Port of CheckAccessControls.java: checks that the programmer has obeyed all the access control
//! restrictions indicated by JSDoc annotations, like `@private` and `@deprecated`.
use crate::abstract_compiler::AbstractCompiler;
use crate::access_control_utils::AccessControlUtils;
use crate::collect_file_overview_visibility::{CollectFileOverviewVisibility, FileVisibilityMap};
use crate::compiler_pass::CompilerPass;
use crate::diagnostic_type::DiagnosticType;
use crate::js_error::JSError;
use crate::node_traversal::{Callback, NodeTraversal};
use crate::node_util::NodeUtil;
use crate::scope::Scope;
use crate::var::Var;
use closure_jstype::prelude::*;
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use closure_rhino::jsdoc_info::{JSDocInfo, Visibility};
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::static_source_file::StaticSourceFile;
use closure_rhino::token::Token;
use closure_rhino::{check_argument, check_state};
use std::sync::Arc;

// port: CheckAccessControls#DEPRECATED_NAME
pub static DEPRECATED_NAME: DiagnosticType =
    DiagnosticType::disabled("JSC_DEPRECATED_VAR", "Variable {0} has been deprecated.");

// port: CheckAccessControls#DEPRECATED_NAME_REASON
pub static DEPRECATED_NAME_REASON: DiagnosticType = DiagnosticType::disabled(
    "JSC_DEPRECATED_VAR_REASON",
    "Variable {0} has been deprecated: {1}",
);

// port: CheckAccessControls#DEPRECATED_PROP
pub static DEPRECATED_PROP: DiagnosticType = DiagnosticType::disabled(
    "JSC_DEPRECATED_PROP",
    "Property {0} of type {1} has been deprecated.",
);

// port: CheckAccessControls#DEPRECATED_PROP_REASON
pub static DEPRECATED_PROP_REASON: DiagnosticType = DiagnosticType::disabled(
    "JSC_DEPRECATED_PROP_REASON",
    "Property {0} of type {1} has been deprecated: {2}",
);

// port: CheckAccessControls#DEPRECATED_CLASS
pub static DEPRECATED_CLASS: DiagnosticType =
    DiagnosticType::disabled("JSC_DEPRECATED_CLASS", "Class {0} has been deprecated.");

// port: CheckAccessControls#DEPRECATED_CLASS_REASON
pub static DEPRECATED_CLASS_REASON: DiagnosticType = DiagnosticType::disabled(
    "JSC_DEPRECATED_CLASS_REASON",
    "Class {0} has been deprecated: {1}",
);

// port: CheckAccessControls#BAD_PACKAGE_PROPERTY_ACCESS
pub static BAD_PACKAGE_PROPERTY_ACCESS: DiagnosticType = DiagnosticType::error(
    "JSC_BAD_PACKAGE_PROPERTY_ACCESS",
    "Access to package-private property {0} of {1} not allowed here.",
);

// port: CheckAccessControls#BAD_PRIVATE_GLOBAL_ACCESS
pub static BAD_PRIVATE_GLOBAL_ACCESS: DiagnosticType = DiagnosticType::error(
    "JSC_BAD_PRIVATE_GLOBAL_ACCESS",
    "Access to private variable {0} not allowed outside file {1}.",
);

// port: CheckAccessControls#BAD_PRIVATE_PROPERTY_ACCESS
pub static BAD_PRIVATE_PROPERTY_ACCESS: DiagnosticType = DiagnosticType::warning(
    "JSC_BAD_PRIVATE_PROPERTY_ACCESS",
    "Access to private property {0} of {1} not allowed here.",
);

// port: CheckAccessControls#BAD_PROTECTED_PROPERTY_ACCESS
pub static BAD_PROTECTED_PROPERTY_ACCESS: DiagnosticType = DiagnosticType::warning(
    "JSC_BAD_PROTECTED_PROPERTY_ACCESS",
    "Access to protected property {0} of {1} not allowed here.",
);

// port: CheckAccessControls#PRIVATE_OVERRIDE
pub static PRIVATE_OVERRIDE: DiagnosticType = DiagnosticType::warning(
    "JSC_PRIVATE_OVERRIDE",
    "Overriding private property of {0}.",
);

// port: CheckAccessControls#EXTEND_FINAL_CLASS
pub static EXTEND_FINAL_CLASS: DiagnosticType = DiagnosticType::error(
    "JSC_EXTEND_FINAL_CLASS",
    "{0} is not allowed to extend final class {1}.",
);

// port: CheckAccessControls#VISIBILITY_MISMATCH
pub static VISIBILITY_MISMATCH: DiagnosticType = DiagnosticType::warning(
    "JSC_VISIBILITY_MISMATCH",
    "Overriding {0} property of {1} with {2} property.",
);

// port: CheckAccessControls#CONST_PROPERTY_REASSIGNED_VALUE
pub static CONST_PROPERTY_REASSIGNED_VALUE: DiagnosticType = DiagnosticType::warning(
    "JSC_CONSTANT_PROPERTY_REASSIGNED_VALUE",
    "constant property {0} assigned a value more than once\nInitialized at {1}",
);

// port: CheckAccessControls#FINAL_PROPERTY_OVERRIDDEN
pub static FINAL_PROPERTY_OVERRIDDEN: DiagnosticType = DiagnosticType::warning(
    "JSC_FINAL_PROPERTY_OVERRIDDEN",
    "@final method or property {0} overridden\nInitialized at {1}",
);

// port: CheckAccessControls#CONST_PROPERTY_DELETED
pub static CONST_PROPERTY_DELETED: DiagnosticType = DiagnosticType::warning(
    "JSC_CONSTANT_PROPERTY_DELETED",
    "constant property {0} cannot be deleted",
);

// port: CheckAccessControls#BAD_PROPERTY_OVERRIDE_IN_FILE_WITH_FILEOVERVIEW_VISIBILITY
pub static BAD_PROPERTY_OVERRIDE_IN_FILE_WITH_FILEOVERVIEW_VISIBILITY: DiagnosticType =
    DiagnosticType::error(
        "JSC_BAD_PROPERTY_OVERRIDE_IN_FILE_WITH_FILEOVERVIEW_VISIBILITY",
        "Overridden property {0} in file with fileoverview visibility {1} must explicitly redeclare superclass visibility",
    );

/// Distinguishes between different kinds of "constant" JSDoc to provide more useful error
/// messages
// port: CheckAccessControls.Constancy
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Constancy {
    FINAL,          // @final
    OTHER_CONSTANT, // e.g. @const, @define, or @desc but not @final
    MUTABLE,
}

// port: CheckAccessControls.ConstantDeclaration
#[derive(Clone, Copy, Debug)]
struct ConstantDeclaration {
    node: NodeId,
    annotation: Constancy,
}

impl ConstantDeclaration {
    // port: CheckAccessControls.ConstantDeclaration#ConstantDeclaration
    fn new(node: NodeId, annotation: Constancy) -> Self {
        Self { node, annotation }
    }
}

/// Java's `HashBasedTable<JSType, String, ConstantDeclaration>`: rows are found like a Java
/// `HashMap` finds a key (equal `JSType#hashCode`, then identity or `JSType#equals`). The table is
/// only read with `get` and written with `row(..).putIfAbsent(..)`; it is never iterated.
/// One row of the table: its JSType key and the column map.
type ConstPropertyRow = (TypeId, IndexMap<JsString, ConstantDeclaration>);

#[derive(Default)]
struct ConstPropertyTable {
    rows: IndexMap<i32, Vec<ConstPropertyRow>>,
}

impl ConstPropertyTable {
    fn find_row(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        hash: i32,
        row: TypeId,
    ) -> Option<usize> {
        let bucket = self.rows.get(&hash)?;
        bucket
            .iter()
            .position(|(key, _)| *key == row || row.equals(reg, ast, *key))
    }

    // port: HashBasedTable#get
    fn get(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        row: Option<TypeId>,
        column: &JsString,
    ) -> Option<ConstantDeclaration> {
        let row = row?;
        let hash = row.hash_code(reg);
        let index = self.find_row(reg, ast, hash, row)?;
        self.rows[&hash][index].1.get(column).copied()
    }

    // port: HashBasedTable#row + Map#putIfAbsent
    fn row_put_if_absent(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        row: TypeId,
        column: &JsString,
        value: ConstantDeclaration,
    ) {
        let hash = row.hash_code(reg);
        let index = self.find_row(reg, ast, hash, row);
        let bucket = self.rows.entry(hash).or_default();
        let index = match index {
            Some(index) => index,
            None => {
                bucket.push((row, IndexMap::<_, _>::default()));
                bucket.len() - 1
            }
        };
        bucket[index].1.entry(column.clone()).or_insert(value);
    }
}

/// A compiler pass that checks that the programmer has obeyed all the access control
/// restrictions indicated by JSDoc annotations, like `@private` and `@deprecated`.
///
/// Because access control restrictions are attached to type information, this pass must run
/// after TypeInference, and InferJSDocInfo.
// port: CheckAccessControls
pub struct CheckAccessControls {
    // State about the current traversal.
    deprecation_depth: i32,
    /// Java's `Deque<ObjectType>` (a null-permissive LinkedList): `push` adds at the front, so
    /// the front is the end of this Vec.
    current_class_stack: Vec<Option<TypeId>>,
    const_property_inits: ConstPropertyTable,
    default_visibility_for_files: FileVisibilityMap,
}

/// The registry and the arena it reads, split-borrowed from the compiler.
fn types(compiler: &mut AbstractCompiler) -> (&mut JSTypeRegistry, &Ast) {
    let (reg, ast) = compiler.get_type_registry_and_ast();
    (reg, &*ast)
}

fn js_str(s: &JsString) -> String {
    s.to_string()
}

impl CheckAccessControls {
    // port: CheckAccessControls#CheckAccessControls
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        // this.typeRegistry = compiler.getTypeRegistry();
        compiler.get_type_registry();
        Self {
            deprecation_depth: 0,
            current_class_stack: Vec::new(),
            const_property_inits: ConstPropertyTable::default(),
            default_visibility_for_files: FileVisibilityMap::default(),
        }
    }

    // port: CheckAccessControls#enterAccessControlScope
    fn enter_access_control_scope(&mut self, compiler: &mut AbstractCompiler, root: NodeId) {
        let (reg, ast) = types(compiler);
        let scope_type = Self::best_instance_type_for_method_or_ctor(reg, ast, root);

        if Self::is_marked_deprecated(ast, root) {
            self.deprecation_depth += 1;
        }
        self.current_class_stack.push(scope_type);
    }

    // port: CheckAccessControls#exitAccessControlScope
    fn exit_access_control_scope(&mut self, ast: &Ast, root: NodeId) {
        if Self::is_marked_deprecated(ast, root) {
            self.deprecation_depth -= 1;
        }
        self.current_class_stack
            .pop()
            .expect("NoSuchElementException");
    }

    /// Maps `node` to the *primary* root of an access-control scope if it is some root, or null
    /// if it is a non-root of the scope.
    // port: CheckAccessControls#primaryAccessControlScopeRootFor
    fn primary_access_control_scope_root_for(ast: &Ast, node: NodeId) -> Option<NodeId> {
        if Self::is_extends_target(ast, node) {
            node.get_parent(ast)
        } else if Self::is_function_or_class(ast, node) {
            Some(node)
        } else {
            None
        }
    }

    /// Returns the instance object type that best represents a method or constructor
    /// definition, or null if there is no representative type.
    // port: CheckAccessControls#bestInstanceTypeForMethodOrCtor
    fn best_instance_type_for_method_or_ctor(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        n: NodeId,
    ) -> Option<TypeId> {
        check_state!(Self::is_function_or_class(ast, n), &n.to_string(ast));
        let parent = n.get_parent(ast).expect("NullPointerException");

        // We need to handle declaration syntaxes separately in a way that we can't determine
        // based on the type of just one node.
        if NodeUtil::is_function_declaration(ast, n)
            // isClassDeclaration returns false for many instances that we need to handle here.
            || n.is_class(ast)
        {
            return Self::instance_type_for(reg, ast, n.get_jstype(ast));
        }

        // All the remaining cases can be isolated based on `parent`.
        match parent.get_token(ast) {
            Token::NAME => Self::instance_type_for(reg, ast, n.get_jstype(ast)),
            Token::ASSIGN => {
                let l_value = parent.get_first_child(ast).unwrap();
                if NodeUtil::is_normal_get(ast, l_value) {
                    // We have an assignment of the form `a.b = ...`.
                    let l_value_type = l_value.get_jstype(ast);
                    if l_value_type.is_some_and(|t| t.is_constructor(reg) || t.is_interface(reg)) {
                        // Case `a.B = ...`
                        Self::instance_type_for(reg, ast, l_value_type)
                    } else if NodeUtil::is_prototype_property(ast, l_value) {
                        // Case `a.B.prototype = ...`
                        let class_name = NodeUtil::get_prototype_class_name(ast, l_value)
                            .expect("NullPointerException");
                        Self::instance_type_for(reg, ast, class_name.get_jstype(ast))
                    } else {
                        // Case `a.b = ...`
                        let receiver = l_value.get_first_child(ast).unwrap();
                        Self::instance_type_for(reg, ast, receiver.get_jstype(ast))
                    }
                } else {
                    // We have an assignment of the form "a = ...", so pull the type off the "a".
                    Self::instance_type_for(reg, ast, l_value.get_jstype(ast))
                }
            }
            Token::STRING_KEY
            | Token::GETTER_DEF
            | Token::SETTER_DEF
            | Token::MEMBER_FUNCTION_DEF
            | Token::MEMBER_FIELD_DEF
            | Token::COMPUTED_PROP => {
                let grandparent = parent.get_parent(ast).expect("NullPointerException");
                let great_grandparent = grandparent.get_parent(ast).expect("NullPointerException");

                if grandparent.is_object_lit(ast) {
                    let grandparent_type =
                        grandparent.get_jstype(ast).expect("NullPointerException");
                    if grandparent_type.is_function_prototype_type(reg) {
                        // Case: `grandparent` is an object-literal prototype.
                        // Example: `Foo.prototype = { a: function() {} };` where `parent` is "a".
                        Self::instance_type_for(reg, ast, Some(grandparent_type))
                    } else {
                        None
                    }
                } else if great_grandparent.is_class(ast) {
                    // Case: `n` is a class member definition.
                    // Example: `class Foo { a() {} }` where `parent` is "a".
                    Self::instance_type_for(reg, ast, great_grandparent.get_jstype(ast))
                } else {
                    // This would indicate the AST is malformed.
                    panic!("AssertionError: {}", great_grandparent.to_string(ast));
                }
            }
            _ => None,
        }
    }

    /// Returns the type that best represents the instance type for `type`.
    // port: CheckAccessControls#instanceTypeFor
    fn instance_type_for(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: Option<TypeId>,
    ) -> Option<TypeId> {
        let type_ = type_?;
        if type_.is_union_type(reg) {
            None // A union has no meaningful instance type.
        } else if type_.is_instance_type(reg) || type_.is_unknown_type(reg, ast) {
            type_.to_maybe_object_type(reg)
        } else if type_.is_constructor(reg) || type_.is_interface(reg) {
            FunctionType::get_instance_type(type_.to_maybe_function_type(reg).unwrap(), reg)
        } else if type_.is_function_type(reg) {
            None // Functions that aren't ctors or interfaces have no instance type.
        } else if type_.is_function_prototype_type(reg) {
            let owner = type_
                .to_maybe_object_type(reg)
                .unwrap()
                .get_owner_function(reg);
            Self::instance_type_for(reg, ast, owner)
        } else {
            type_.to_maybe_object_type(reg)
        }
    }

    // port: CheckAccessControls#checkDeprecation
    fn check_deprecation(
        &mut self,
        node: NodeId,
        prop_ref: Option<&PropertyReference>,
        identifier_behaviour: IdentifierBehaviour,
        traversal: &mut NodeTraversal<'_>,
    ) {
        match identifier_behaviour {
            IdentifierBehaviour::ES5_CLASS_INVOCATION
            | IdentifierBehaviour::ES6_CLASS_INVOCATION
            | IdentifierBehaviour::ES6_CLASS_NAMESPACE => {
                // At these usages, treat the deprecation applied to type-declaration as referring
                // to the type, not the identifier (e.g. "the use of class `Foo` is deprecated").
                self.check_type_deprecation(traversal, node)
            }
            IdentifierBehaviour::NON_CONSTRUCTOR => {
                // For all identifiers that are not constructors, deprecation refers to the
                // identifier (e.g. "the use of variable `x` is deprecated").
                self.check_name_deprecation(traversal, node)
            }
            _ => {}
        }

        if let Some(prop_ref) = prop_ref
            && identifier_behaviour != IdentifierBehaviour::ES5_CLASS_NAMESPACE
        {
            self.check_property_deprecation(traversal, prop_ref);
        }
    }

    // port: CheckAccessControls#checkVisibility
    fn check_visibility(
        &mut self,
        compiler: &mut AbstractCompiler,
        node: NodeId,
        prop_ref: Option<&PropertyReference>,
        identifier_behaviour: IdentifierBehaviour,
        scope: Scope,
    ) {
        if identifier_behaviour == IdentifierBehaviour::ES6_CLASS_INVOCATION {
            self.check_es6_constructor_invocation_visibility(compiler, node);
        }

        if identifier_behaviour != IdentifierBehaviour::ES5_CLASS_NAMESPACE {
            self.check_name_visibility(compiler, scope, node);
        }

        if let Some(prop_ref) = prop_ref
            && identifier_behaviour != IdentifierBehaviour::ES5_CLASS_NAMESPACE
        {
            self.check_property_visibility(compiler, prop_ref);
        }
    }

    /// Reports deprecation issue with regard to a type usage.
    ///
    /// Precondition: `n` has a constructor JSType.
    // port: CheckAccessControls#checkTypeDeprecation
    fn check_type_deprecation(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        if !self.should_emit_deprecation_warning(t, n) {
            return;
        }

        let compiler = t.get_compiler();
        let (reg, ast) = types(compiler);
        let ctor_type = n
            .get_jstype(ast)
            .expect("NullPointerException")
            .to_maybe_function_type(reg)
            .expect("NullPointerException");
        let instance_type =
            FunctionType::get_instance_type(ctor_type, reg).expect("NullPointerException");

        let Some(deprecation_info) = Self::get_type_deprecation_info(reg, Some(instance_type))
        else {
            return;
        };

        let message: &'static DiagnosticType = if deprecation_info.is_empty() {
            &DEPRECATED_CLASS
        } else {
            &DEPRECATED_CLASS_REASON
        };
        let instance_type_string = instance_type.to_string(reg, ast);
        compiler.report(JSError::make(
            compiler,
            n,
            message,
            &[&instance_type_string, &deprecation_info],
        ));
    }

    /// Checks the given NAME node to ensure that access restrictions are obeyed.
    // port: CheckAccessControls#checkNameDeprecation
    fn check_name_deprecation(&mut self, t: &mut NodeTraversal<'_>, n: NodeId) {
        if !n.is_name(t) {
            return;
        }

        if !self.should_emit_deprecation_warning(t, n) {
            return;
        }

        let scope = t.get_scope();
        let compiler = t.get_compiler();
        let name = n.get_string(compiler);
        let var = scope.get_var(compiler, &name);
        let doc_info = var.and_then(|var| var.get_jsdoc_info(compiler));

        if let Some(doc_info) = doc_info
            && doc_info.is_deprecated()
        {
            if let Some(reason) = doc_info.get_deprecation_reason() {
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &DEPRECATED_NAME_REASON,
                    &[&js_str(&name), &js_str(&reason)],
                ));
            } else {
                compiler.report(JSError::make(
                    compiler,
                    n,
                    &DEPRECATED_NAME,
                    &[&js_str(&name)],
                ));
            }
        }
    }

    /// Checks the given GETPROP node to ensure that access restrictions are obeyed.
    // port: CheckAccessControls#checkPropertyDeprecation
    fn check_property_deprecation(
        &mut self,
        t: &mut NodeTraversal<'_>,
        prop_ref: &PropertyReference,
    ) {
        if !self.should_emit_deprecation_warning_for_property(t, prop_ref) {
            return;
        }

        let compiler = t.get_compiler();
        let (reg, ast) = types(compiler);
        // Don't bother checking constructors.
        if prop_ref
            .source_node
            .get_parent(ast)
            .expect("NullPointerException")
            .is_new(ast)
        {
            return;
        }

        let dereferenced = Self::dereference(reg, ast, Some(prop_ref.receiver_type));
        let object_type = Self::cast_to_object(reg, dereferenced);
        let property_name = &prop_ref.name;

        if let Some(object_type) = object_type {
            let deprecation_info =
                Self::get_property_deprecation_info(reg, ast, object_type, property_name);
            if let Some(deprecation_info) = deprecation_info {
                let readable_type_name = prop_ref.get_readable_type_name_or_default(reg, ast);
                if !deprecation_info.is_empty() {
                    compiler.report(JSError::make(
                        compiler,
                        prop_ref.source_node,
                        &DEPRECATED_PROP_REASON,
                        &[
                            &js_str(property_name),
                            &readable_type_name,
                            &deprecation_info,
                        ],
                    ));
                } else {
                    compiler.report(JSError::make(
                        compiler,
                        prop_ref.source_node,
                        &DEPRECATED_PROP,
                        &[&js_str(property_name), &readable_type_name],
                    ));
                }
            }
        }
    }

    /// Reports an error if the given name is not visible in the current context.
    // port: CheckAccessControls#checkNameVisibility
    fn check_name_visibility(
        &mut self,
        compiler: &mut AbstractCompiler,
        scope: Scope,
        name: NodeId,
    ) {
        if !name.is_name(compiler) {
            return;
        }

        let name_string = name.get_string(compiler);
        let Some(var) = scope.get_var(compiler, &name_string) else {
            return;
        };

        let v = AccessControlUtils::get_effective_name_visibility(
            compiler,
            name,
            var,
            &self.default_visibility_for_files,
        );

        match v {
            Visibility::PACKAGE if !Self::is_package_access_allowed(compiler, var, name) => {
                let file = var
                    .get_source_file(compiler)
                    .expect("NullPointerException")
                    .get_name()
                    .to_string();
                compiler.report(JSError::make(
                    compiler,
                    name,
                    &BAD_PACKAGE_PROPERTY_ACCESS,
                    &[&js_str(&name_string), &file],
                ));
            }
            Visibility::PRIVATE if !Self::is_private_access_allowed(compiler, var, name) => {
                let file = var
                    .get_source_file(compiler)
                    .expect("NullPointerException")
                    .get_name()
                    .to_string();
                compiler.report(JSError::make(
                    compiler,
                    name,
                    &BAD_PRIVATE_GLOBAL_ACCESS,
                    &[&js_str(&name_string), &file],
                ));
            }
            _ => {
                // Nothing to do for PUBLIC and PROTECTED
                // (which is irrelevant for names).
            }
        }
    }

    // port: CheckAccessControls#isPrivateAccessAllowed
    fn is_private_access_allowed(compiler: &AbstractCompiler, var: Var, name: NodeId) -> bool {
        let var_src = var.get_source_file(compiler);
        let ref_src = name.get_static_source_file(compiler);

        match (var_src, ref_src) {
            (Some(var_src), Some(ref_src)) => var_src.get_name() == ref_src.get_name(),
            _ => true,
        }
    }

    // port: CheckAccessControls#isPackageAccessAllowed
    fn is_package_access_allowed(compiler: &AbstractCompiler, var: Var, name: NodeId) -> bool {
        let var_src = var.get_source_file(compiler);
        let ref_src = name.get_static_source_file(compiler);
        if var_src.is_none() && ref_src.is_none() {
            // If the source file of either var or name is unavailable, conservatively assume they
            // belong to different packages.
            return false;
        }

        let coding_convention = compiler.get_coding_convention();
        // Every CodingConvention dereferences the source file (Java NPE on a null one).
        let src_package =
            coding_convention.get_package_name(&**var_src.as_ref().expect("NullPointerException"));
        let ref_package =
            coding_convention.get_package_name(&**ref_src.as_ref().expect("NullPointerException"));
        src_package.is_some() && ref_package.is_some() && src_package == ref_package
    }

    // port: CheckAccessControls#checkPropertyOverrideVisibilityIsSame
    fn check_property_override_visibility_is_same(
        &mut self,
        compiler: &mut AbstractCompiler,
        overriding: Visibility,
        overridden: Visibility,
        file_overview: Option<Visibility>,
        prop_ref: &PropertyReference,
    ) {
        if overriding == Visibility::INHERITED
            && overriding != overridden
            && let Some(file_overview) = file_overview
            && file_overview != Visibility::INHERITED
        {
            compiler.report(JSError::make(
                compiler,
                prop_ref.source_node,
                &BAD_PROPERTY_OVERRIDE_IN_FILE_WITH_FILEOVERVIEW_VISIBILITY,
                &[&js_str(&prop_ref.name), &visibility_name(file_overview)],
            ));
        }
    }

    // port: CheckAccessControls#getOverridingPropertyVisibility
    fn get_overriding_property_visibility(
        ast: &Ast,
        prop_ref: &PropertyReference,
    ) -> Option<Visibility> {
        let overriding_info = prop_ref.get_jsdoc_info(ast);
        match overriding_info {
            Some(info) if info.is_override() => Some(info.get_visibility()),
            _ => None,
        }
    }

    /// Checks if a constructor is trying to override a final class.
    // port: CheckAccessControls#checkFinalClassOverrides
    fn check_final_class_overrides(&mut self, compiler: &mut AbstractCompiler, ctor: NodeId) {
        let (reg, ast) = types(compiler);
        if !Self::is_function_or_class(ast, ctor)
            // checking class constructors is redundant because we already check the same thing
            // on the CLASS node
            || NodeUtil::is_es6_constructor_member_function_def(
                ast,
                ctor.get_parent(ast).expect("NullPointerException"),
            )
        {
            return;
        }

        let ctor_type = ctor
            .get_jstype(ast)
            .expect("NullPointerException")
            .to_maybe_function_type(reg);
        let Some(ctor_type) = ctor_type else {
            return;
        };
        if !ctor_type.is_constructor(reg) {
            return;
        }
        let final_parent_class = Self::get_super_class_instance_if_final(reg, ast, ctor_type);
        if let Some(final_parent_class) = final_parent_class {
            let ctor_name = ctor_type
                .get_display_name(reg)
                .unwrap_or_else(|| "null".to_string());
            let parent_name = final_parent_class
                .get_display_name(reg)
                .unwrap_or_else(|| "null".to_string());
            compiler.report(JSError::make(
                compiler,
                ctor,
                &EXTEND_FINAL_CLASS,
                &[&ctor_name, &parent_name],
            ));
        }
    }

    /// Determines whether the given constant property got reassigned
    // port: CheckAccessControls#checkConstantProperty
    fn check_constant_property(
        &mut self,
        compiler: &mut AbstractCompiler,
        prop_ref: Option<&PropertyReference>,
        identifier_behaviour: IdentifierBehaviour,
    ) {
        let Some(prop_ref) = prop_ref else {
            return;
        };
        if identifier_behaviour == IdentifierBehaviour::ES5_CLASS_NAMESPACE {
            return;
        }

        let (reg, ast) = types(compiler);
        let object_type = Self::dereference(reg, ast, Some(prop_ref.receiver_type));
        let property_name = &prop_ref.name;
        let source_node = prop_ref.source_node;

        let constness = Self::is_property_declared_constant(reg, ast, object_type, property_name);
        if constness == Constancy::MUTABLE {
            return;
        }
        // A declared-constant property was found on the chain, so the type is not null.
        let object_type = object_type.unwrap();

        if source_node.is_from_externs(ast) && prop_ref.declaration {
            // Treat stub declarations in externs as inits, but never warn on them.
            self.record_const_property_init(reg, ast, prop_ref, object_type, constness);
            return;
        }

        if !prop_ref.mutation {
            return;
        }

        if prop_ref.is_deletion(ast) {
            compiler.report(JSError::make(
                compiler,
                source_node,
                &CONST_PROPERTY_DELETED,
                &[&js_str(property_name)],
            ));
            return;
        }

        // Can't check for constant properties on generic function types.
        // TODO(johnlenz): I'm not 100% certain this is necessary, or if
        // the type is being inspected incorrectly.
        if object_type.is_function_type(reg)
            && !object_type
                .to_maybe_function_type(reg)
                .unwrap()
                .is_constructor(reg)
        {
            return;
        }

        if object_type.is_structural_type(reg) && !prop_ref.declaration {
            // We don't know the claess this structural type matches, so assume all assignments are
            // bad.
            compiler.report(JSError::make(
                compiler,
                source_node,
                &CONST_PROPERTY_REASSIGNED_VALUE,
                &[
                    &js_str(property_name),
                    "unknown location due to structural typing",
                ],
            ));
            return;
        }

        let init = self.get_const_property_init(reg, ast, prop_ref, object_type);
        if let Some(init) = init {
            let diagnostic: &'static DiagnosticType = if init.annotation == Constancy::FINAL {
                &FINAL_PROPERTY_OVERRIDDEN
            } else {
                &CONST_PROPERTY_REASSIGNED_VALUE
            };
            let location = init.node.get_location(ast);
            compiler.report(JSError::make(
                compiler,
                source_node,
                diagnostic,
                &[&js_str(property_name), &location],
            ));
        }

        let (reg, ast) = types(compiler);
        self.record_const_property_init(reg, ast, prop_ref, object_type, constness);
    }

    // port: CheckAccessControls#getConstPropertyInit
    fn get_const_property_init(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        ref_: &PropertyReference,
        type_: TypeId,
    ) -> Option<ConstantDeclaration> {
        let name = &ref_.name;
        let mut type_ = Some(type_);
        while let Some(t) = type_ {
            let init = self.const_property_inits.get(reg, ast, Some(t), name);
            if init.is_some() {
                return init;
            }
            let canonical = Self::get_canonical_instance(reg, t);
            let canonical_init = self.const_property_inits.get(reg, ast, canonical, name);
            if canonical_init.is_some() {
                return canonical_init;
            }

            type_ = t.get_implicit_prototype(reg, ast);
        }

        None
    }

    // port: CheckAccessControls#recordConstPropertyInit
    fn record_const_property_init(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        ref_: &PropertyReference,
        type_: TypeId,
        annotation: Constancy,
    ) {
        self.const_property_inits.row_put_if_absent(
            reg,
            ast,
            type_,
            &ref_.name,
            ConstantDeclaration::new(ref_.source_node, annotation),
        );

        // Add the prototype when we're looking at an instance object
        if type_.is_instance_type(reg) {
            let prototype = type_.get_implicit_prototype(reg, ast);
            if let Some(prototype) = prototype
                && prototype.has_property(reg, ast, &ref_.name)
            {
                self.const_property_inits.row_put_if_absent(
                    reg,
                    ast,
                    prototype,
                    &ref_.name,
                    ConstantDeclaration::new(ref_.source_node, annotation),
                );
            }
        }
    }

    /// Return an object with the same nominal type as obj, but without any possible extra
    /// properties that exist on obj.
    // port: CheckAccessControls#getCanonicalInstance
    pub fn get_canonical_instance(reg: &JSTypeRegistry, obj: TypeId) -> Option<TypeId> {
        match obj.get_constructor(reg) {
            None => Some(obj),
            Some(ctor) => FunctionType::get_instance_type(ctor, reg),
        }
    }

    /// Dereference a type, autoboxing it and filtering out null.
    // port: CheckAccessControls#dereference
    fn dereference(reg: &mut JSTypeRegistry, ast: &Ast, type_: Option<TypeId>) -> Option<TypeId> {
        type_.and_then(|t| t.dereference(reg, ast))
    }

    // port: CheckAccessControls#typeOrUnknown(JSType)
    fn type_or_unknown(reg: &JSTypeRegistry, type_: Option<TypeId>) -> TypeId {
        type_.unwrap_or_else(|| reg.get_native_type(JSTypeNative::UNKNOWN_TYPE))
    }

    // port: CheckAccessControls#boxedOrUnknown
    fn boxed_or_unknown(reg: &mut JSTypeRegistry, ast: &Ast, type_: Option<TypeId>) -> TypeId {
        let dereferenced = Self::dereference(reg, ast, type_);
        Self::type_or_unknown(reg, dereferenced)
    }

    /// Reports an error if the given property is not visible in the current context.
    ///
    /// This method covers both accesses to properties during execution and overrides of
    /// properties during declaration.
    // port: CheckAccessControls#checkPropertyVisibility
    fn check_property_visibility(
        &mut self,
        compiler: &mut AbstractCompiler,
        prop_ref: &PropertyReference,
    ) {
        if NodeUtil::is_es6_constructor_member_function_def(compiler, prop_ref.source_node) {
            // Class ctor *declarations* can never violate visibility restrictions. They are not
            // accesses and we don't consider them overrides.
            return;
        }

        let (reg, ast) = types(compiler);
        let raw_reference_type =
            Self::type_or_unknown(reg, Some(prop_ref.receiver_type)).autobox(reg, ast);
        let reference_type = Self::cast_to_object(reg, Some(raw_reference_type));

        let property_name = &prop_ref.name;

        let mut defining_source = AccessControlUtils::get_defining_source(
            reg,
            ast,
            prop_ref.source_node,
            reference_type,
            property_name,
        );

        // Is this a normal property access, or are we trying to override
        // an existing property?
        let is_override = prop_ref.is_documented_declaration(ast) || prop_ref.override_;

        let object_type = AccessControlUtils::get_object_type(
            reg,
            ast,
            reference_type,
            is_override,
            property_name,
        );

        let file_overview_visibility = self
            .default_visibility_for_files
            .get(defining_source.as_ref());

        let visibility = Self::get_effective_property_visibility(
            reg,
            ast,
            prop_ref,
            reference_type,
            &self.default_visibility_for_files,
        );

        if is_override {
            let overriding = Self::get_overriding_property_visibility(ast, prop_ref);
            if let Some(overriding) = overriding {
                self.check_property_override_visibility_is_same(
                    compiler,
                    overriding,
                    visibility,
                    file_overview_visibility,
                    prop_ref,
                );
            }
        }

        let (reg, ast) = types(compiler);
        let mut report_type = raw_reference_type;
        if let Some(object_type) = object_type {
            let node = object_type.get_own_property_def_site(reg, ast, property_name);
            let Some(node) = node else {
                // Assume the property is public.
                return;
            };
            report_type = object_type;
            defining_source = node.get_static_source_file(ast);
        } else if file_overview_visibility.is_none() {
            // We can only check visibility references if we know what file
            // it was defined in.
            // Otherwise just assume the property is public.
            return;
        }

        let reference_source = prop_ref.source_node.get_static_source_file(ast);

        if is_override {
            let same_input = reference_source.as_ref().is_some_and(|reference_source| {
                reference_source.get_name()
                    == defining_source
                        .as_ref()
                        .expect("NullPointerException")
                        .get_name()
            });
            self.check_property_override_visibility(
                compiler,
                prop_ref,
                visibility,
                file_overview_visibility,
                report_type,
                same_input,
            );
        } else {
            self.check_property_access_visibility(
                compiler,
                prop_ref,
                visibility,
                report_type,
                reference_source,
                defining_source,
            );
        }
    }

    /// Reports visibility violations on ES6 class constructor invocations.
    ///
    /// Precondition: `target` has an ES6 class JSType.
    // port: CheckAccessControls#checkEs6ConstructorInvocationVisibility
    fn check_es6_constructor_invocation_visibility(
        &mut self,
        compiler: &mut AbstractCompiler,
        target: NodeId,
    ) {
        let (reg, ast) = types(compiler);
        let ctor_type = target
            .get_jstype(ast)
            .expect("NullPointerException")
            .to_maybe_function_type(reg)
            .expect("NullPointerException");
        let prototype_type = ctor_type.get_prototype(reg, ast);

        // We use the class definition site because classes automatically get a implicit
        // constructor, so there may not be a definition node.
        let class_definition = FunctionType::get_source(ctor_type, reg);

        let constructor = JsString::from("constructor");
        let defining_source = match class_definition {
            None => None,
            Some(class_definition) => AccessControlUtils::get_defining_source(
                reg,
                ast,
                class_definition,
                Some(prototype_type),
                &constructor,
            ),
        };

        // Synthesize a `PropertyReference` for this constructor call as if we're accessing
        // `Foo.prototype.constructor`. This object allows us to reuse the
        // `checkPropertyAccessVisibility` method which actually reports violations.
        let faux_ctor_ref = PropertyReference::builder()
            .set_source_node(target)
            .set_name(constructor)
            .set_receiver_type(prototype_type)
            .set_mutation(false) // This shouldn't matter.
            .set_declaration(false) // This shouldn't matter.
            .set_override(false) // This shouldn't matter.
            .set_readable_type_name(ReadableTypeName::InstanceTypeOf(ctor_type))
            .build();

        let annotated_ctor_visibility =
            // This function defaults to `INHERITED` which isn't what we want here, but it does
            // handle combining inline and `@fileoverview` visibilities.
            Self::get_effective_visibility_for_non_overridden_property(
                reg,
                ast,
                &faux_ctor_ref,
                Some(prototype_type),
                self.default_visibility_for_files.get(defining_source.as_ref()),
            );
        let effective_ctor_visibility = if annotated_ctor_visibility == Visibility::INHERITED {
            Visibility::PUBLIC
        } else {
            annotated_ctor_visibility
        };

        let reference_source = target.get_static_source_file(ast);
        self.check_property_access_visibility(
            compiler,
            &faux_ctor_ref,
            effective_ctor_visibility,
            ctor_type,
            reference_source,
            defining_source,
        );
    }

    // port: CheckAccessControls#checkPropertyOverrideVisibility
    fn check_property_override_visibility(
        &mut self,
        compiler: &mut AbstractCompiler,
        prop_ref: &PropertyReference,
        visibility: Visibility,
        file_overview_visibility: Option<Visibility>,
        object_type: TypeId,
        same_input: bool,
    ) {
        let (reg, ast) = types(compiler);
        let overriding_visibility = if prop_ref.override_ {
            prop_ref
                .get_jsdoc_info(ast)
                .expect("NullPointerException")
                .get_visibility()
        } else {
            Visibility::INHERITED
        };

        // Check that:
        // (a) the property *can* be overridden,
        // (b) the visibility of the override is the same as (or broader than) the
        //     visibility of the original property,
        // (c) the visibility is explicitly redeclared if the override is in
        //     a file with default visibility in the @fileoverview block.
        if visibility == Visibility::PRIVATE && !same_input {
            let object_type_string = object_type.to_string(reg, ast);
            compiler.report(JSError::make(
                compiler,
                prop_ref.source_node,
                &PRIVATE_OVERRIDE,
                &[&object_type_string],
            ));
        } else if !Self::can_override_visibility(visibility, overriding_visibility)
            && file_overview_visibility.is_none()
        {
            let object_type_string = object_type.to_string(reg, ast);
            compiler.report(JSError::make(
                compiler,
                prop_ref.source_node,
                &VISIBILITY_MISMATCH,
                &[
                    &visibility_name(visibility),
                    &object_type_string,
                    &visibility_name(overriding_visibility),
                ],
            ));
        }
    }

    // port: CheckAccessControls#canOverrideVisibility
    fn can_override_visibility(
        superclass_visibility: Visibility,
        subclass_visibility: Visibility,
    ) -> bool {
        // This allows INHERITED to override anything, PUBLIC to override anything (except
        // INHERITED), and PROTECTED to override anything (except PUBLIC or INHERITED). PRIVATE was
        // already handled in a previous check, leaving PACKAGE as the lowest visibility.
        superclass_visibility.cmp(&subclass_visibility) != std::cmp::Ordering::Greater
    }

    // port: CheckAccessControls#checkPropertyAccessVisibility
    fn check_property_access_visibility(
        &mut self,
        compiler: &mut AbstractCompiler,
        prop_ref: &PropertyReference,
        visibility: Visibility,
        object_type: TypeId,
        reference_source: Option<Arc<dyn StaticSourceFile>>,
        defining_source: Option<Arc<dyn StaticSourceFile>>,
    ) {
        // private access is always allowed in the same file.
        if let (Some(reference_source), Some(defining_source)) =
            (&reference_source, &defining_source)
            && reference_source.get_name() == defining_source.get_name()
        {
            return;
        }

        let (reg, ast) = types(compiler);
        let owner_type = Self::instance_type_for(reg, ast, Some(object_type));

        match visibility {
            Visibility::PACKAGE => Self::check_package_property_visibility(
                compiler,
                prop_ref,
                reference_source,
                defining_source,
            ),
            Visibility::PRIVATE => {
                Self::check_private_property_visibility(compiler, prop_ref, owner_type)
            }
            Visibility::PROTECTED => {
                self.check_protected_property_visibility(compiler, prop_ref, owner_type)
            }
            _ => {}
        }
    }

    // port: CheckAccessControls#checkPackagePropertyVisibility
    fn check_package_property_visibility(
        compiler: &mut AbstractCompiler,
        prop_ref: &PropertyReference,
        reference_source: Option<Arc<dyn StaticSourceFile>>,
        defining_source: Option<Arc<dyn StaticSourceFile>>,
    ) {
        let coding_convention = compiler.get_coding_convention();
        // Every CodingConvention dereferences the source file (Java NPE on a null one).
        let ref_package = coding_convention
            .get_package_name(&**reference_source.as_ref().expect("NullPointerException"));
        let def_package = coding_convention
            .get_package_name(&**defining_source.as_ref().expect("NullPointerException"));
        if ref_package.is_none() || def_package.is_none() || ref_package != def_package {
            let (reg, ast) = types(compiler);
            let readable_type_name = prop_ref.get_readable_type_name_or_default(reg, ast);
            compiler.report(JSError::make(
                compiler,
                prop_ref.source_node,
                &BAD_PACKAGE_PROPERTY_ACCESS,
                &[&js_str(&prop_ref.name), &readable_type_name],
            ));
        }
    }

    // port: CheckAccessControls#checkPrivatePropertyVisibility
    fn check_private_property_visibility(
        compiler: &mut AbstractCompiler,
        prop_ref: &PropertyReference,
        owner_type: Option<TypeId>,
    ) {
        // private access is not allowed outside the file from a different
        // enclosing class.
        let (reg, ast) = types(compiler);
        let readable_type_name = match owner_type {
            Some(owner_type) if !owner_type.equals(reg, ast, prop_ref.receiver_type) => {
                owner_type.to_string(reg, ast)
            }
            _ => prop_ref.get_readable_type_name_or_default(reg, ast),
        };
        compiler.report(JSError::make(
            compiler,
            prop_ref.source_node,
            &BAD_PRIVATE_PROPERTY_ACCESS,
            &[&js_str(&prop_ref.name), &readable_type_name],
        ));
    }

    // port: CheckAccessControls#checkProtectedPropertyVisibility
    fn check_protected_property_visibility(
        &mut self,
        compiler: &mut AbstractCompiler,
        prop_ref: &PropertyReference,
        owner_type: Option<TypeId>,
    ) {
        // There are 3 types of legal accesses of a protected property:
        // 1) Accesses in the same file
        // 2) Overriding the property in a subclass
        // 3) Accessing the property from inside a subclass
        // The first two have already been checked for.
        let (reg, ast) = types(compiler);
        if let Some(owner_type) = owner_type {
            // Java iterates the deque from its front, the most recently pushed scope.
            for scope_type in self.current_class_stack.iter().rev() {
                let Some(scope_type) = *scope_type else {
                    continue;
                };
                if scope_type.is_subtype_of(reg, ast, owner_type) {
                    return;
                }
            }
        }

        let readable_type_name = prop_ref.get_readable_type_name_or_default(reg, ast);
        compiler.report(JSError::make(
            compiler,
            prop_ref.source_node,
            &BAD_PROTECTED_PROPERTY_ACCESS,
            &[&js_str(&prop_ref.name), &readable_type_name],
        ));
    }

    /// Determines whether a deprecation warning should be emitted.
    // port: CheckAccessControls#shouldEmitDeprecationWarning(NodeTraversal,Node)
    fn should_emit_deprecation_warning(&self, t: &mut NodeTraversal<'_>, n: NodeId) -> bool {
        // In the global scope, there are only two kinds of accesses that should
        // be flagged for warnings:
        // 1) Calls of deprecated functions and methods.
        // 2) Instantiations of deprecated classes.
        // For now, we just let everything else by.
        if t.in_global_scope() && !NodeUtil::is_invocation_target(t, n) && !n.is_new(t) {
            return false;
        }

        !self.can_access_deprecated_types(t)
    }

    /// Determines whether a deprecation warning should be emitted.
    // port: CheckAccessControls#shouldEmitDeprecationWarning(NodeTraversal,PropertyReference)
    fn should_emit_deprecation_warning_for_property(
        &self,
        t: &mut NodeTraversal<'_>,
        prop_ref: &PropertyReference,
    ) -> bool {
        // In the global scope, there are only two kinds of accesses that should
        // be flagged for warnings:
        // 1) Calls of deprecated functions and methods.
        // 2) Instantiations of deprecated classes.
        // For now, we just let everything else by.
        if t.in_global_scope() && !NodeUtil::is_invocation_target(t, prop_ref.source_node) {
            return false;
        }

        // We can always assign to a deprecated property, to keep it up to date.
        if prop_ref.mutation {
            return false;
        }

        // Don't warn if the node is just declaring the property, not reading it.
        let jsdoc = prop_ref.get_jsdoc_info(t);
        if prop_ref.declaration && jsdoc.is_some_and(|jsdoc| jsdoc.is_deprecated()) {
            return false;
        }

        !self.can_access_deprecated_types(t)
    }

    /// Returns whether it's currently OK to access deprecated names and properties.
    ///
    /// There are 3 exceptions when we're allowed to use a deprecated type or property:
    /// 1) When we're in a deprecated function.
    /// 2) When we're in a deprecated class.
    /// 3) When we're in a static method of a deprecated class.
    // port: CheckAccessControls#canAccessDeprecatedTypes
    fn can_access_deprecated_types(&self, t: &mut NodeTraversal<'_>) -> bool {
        let mut scope_root = t
            .get_closest_hoist_scope_root()
            .expect("NullPointerException");
        let (reg, ast) = types(t.get_compiler());
        if NodeUtil::is_function_block(ast, scope_root) {
            scope_root = scope_root.get_parent(ast).unwrap();
        }
        let scope_root_parent = scope_root.get_parent(ast);

        // Cases 2 and 3 are required to handle ES5-style class methods since they aren't nested
        // inside their class. This is tested in the CheckAccessControlsOldSyntaxTest class.
        // Case #1
        (self.deprecation_depth > 0)
            // Case #2
            || {
                let type_of_this = Self::get_type_of_this(reg, ast, scope_root);
                Self::get_type_deprecation_info(reg, type_of_this).is_some()
            }
            // Case #3
            || (scope_root_parent.is_some_and(|p| p.is_assign(ast)) && {
                let best = Self::best_instance_type_for_method_or_ctor(reg, ast, scope_root);
                Self::get_type_deprecation_info(reg, best).is_some()
            })
    }

    /// Returns whether this node roots a subtree under which references to deprecated constructs
    /// are allowed.
    // port: CheckAccessControls#isMarkedDeprecated
    fn is_marked_deprecated(ast: &Ast, n: NodeId) -> bool {
        Self::get_deprecation_reason(NodeUtil::get_best_jsdoc_info(ast, n).as_deref()).is_some()
    }

    /// Returns the deprecation reason for the type if it is marked as being deprecated. Returns
    /// empty string if the type is deprecated but no reason was given. Returns null if the type
    /// is not deprecated.
    // port: CheckAccessControls#getTypeDeprecationInfo
    fn get_type_deprecation_info(reg: &JSTypeRegistry, type_: Option<TypeId>) -> Option<String> {
        let type_ = type_?;

        Self::get_deprecation_reason(type_.get_jsdoc_info(reg).as_deref())
    }

    // port: CheckAccessControls#getDeprecationReason
    fn get_deprecation_reason(info: Option<&JSDocInfo>) -> Option<String> {
        if let Some(info) = info
            && info.is_deprecated()
        {
            if let Some(reason) = info.get_deprecation_reason() {
                return Some(js_str(&reason));
            }
            return Some(String::new());
        }
        None
    }

    /// Returns if a property is declared constant.
    // port: CheckAccessControls#isPropertyDeclaredConstant
    fn is_property_declared_constant(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        mut object_type: Option<TypeId>,
        prop: &JsString,
    ) -> Constancy {
        while let Some(ot) = object_type {
            let doc_info = ot.get_own_property_jsdoc_info(reg, ast, prop);
            if let Some(doc_info) = doc_info {
                if doc_info.is_final() {
                    return Constancy::FINAL;
                } else if doc_info.is_constant() {
                    return Constancy::OTHER_CONSTANT;
                }
            }
            object_type = ot.get_implicit_prototype(reg, ast);
        }
        Constancy::MUTABLE
    }

    /// Returns the deprecation reason for the property if it is marked as being deprecated.
    /// Returns empty string if the property is deprecated but no reason was given. Returns null
    /// if the property is not deprecated.
    // port: CheckAccessControls#getPropertyDeprecationInfo
    fn get_property_deprecation_info(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        type_: TypeId,
        prop: &JsString,
    ) -> Option<String> {
        let dep_reason = Self::get_deprecation_reason(
            type_.get_own_property_jsdoc_info(reg, ast, prop).as_deref(),
        );
        if dep_reason.is_some() {
            return dep_reason;
        }

        let implicit_proto = type_.get_implicit_prototype(reg, ast);
        if let Some(implicit_proto) = implicit_proto {
            return Self::get_property_deprecation_info(reg, ast, implicit_proto, prop);
        }
        None
    }

    /// If the superclass is final, this method returns an instance of the superclass.
    // port: CheckAccessControls#getSuperClassInstanceIfFinal
    fn get_super_class_instance_if_final(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        sub_ctor: TypeId,
    ) -> Option<TypeId> {
        let ctor = sub_ctor.get_super_class_constructor(reg, ast);
        let doc = ctor.and_then(|ctor| ctor.get_jsdoc_info(reg));
        if doc.is_some_and(|doc| doc.is_final()) {
            return FunctionType::get_instance_type(ctor.unwrap(), reg);
        }

        None
    }

    // port: CheckAccessControls#castToObject
    fn cast_to_object(reg: &JSTypeRegistry, type_: Option<TypeId>) -> Option<TypeId> {
        type_.and_then(|t| t.to_maybe_object_type(reg))
    }

    // port: CheckAccessControls#isFunctionOrClass
    fn is_function_or_class(ast: &Ast, n: NodeId) -> bool {
        n.is_function(ast) || n.is_class(ast)
    }

    // port: CheckAccessControls#isExtendsTarget
    fn is_extends_target(ast: &Ast, node: NodeId) -> bool {
        let parent = node.get_parent(ast).expect("NullPointerException");
        parent.is_class(ast) && node.is_second_child_of(ast, Some(parent))
    }

    // port: CheckAccessControls#getTypeOfThis
    fn get_type_of_this(reg: &mut JSTypeRegistry, ast: &Ast, scope_root: NodeId) -> Option<TypeId> {
        if scope_root.is_root(ast) || scope_root.is_script(ast) {
            return Self::cast_to_object(reg, scope_root.get_jstype(ast));
        } else if scope_root.is_module_body(ast) {
            return None;
        } else if NodeUtil::is_class_static_block(ast, scope_root) {
            let class_node =
                NodeUtil::get_enclosing_class(ast, scope_root).expect("NullPointerException");
            return class_node.get_jstype(ast);
        }

        check_argument!(scope_root.is_function(ast), &scope_root.to_string(ast));

        let node_type = scope_root.get_jstype(ast);
        match node_type {
            Some(node_type) if node_type.is_function_type(reg) => {
                Some(closure_jstype::function_type::get_type_of_this(
                    node_type.to_maybe_function_type(reg).unwrap(),
                    reg,
                ))
            }
            // Executed when the current scope has not been typechecked.
            _ => None,
        }
    }

    // port: CheckAccessControls#createPropertyReference
    #[allow(clippy::nonminimal_bool)] // Java's expression, kept verbatim
    fn create_property_reference(
        compiler: &mut AbstractCompiler,
        source_node: NodeId,
    ) -> Option<PropertyReference> {
        let (reg, ast) = types(compiler);
        let parent = source_node.get_parent(ast).expect("NullPointerException");
        let jsdoc = NodeUtil::get_best_jsdoc_info(ast, source_node);

        let mut builder = PropertyReference::builder();

        match source_node.get_token(ast) {
            Token::GETPROP => {
                let is_l_value = NodeUtil::is_l_value(ast, source_node);
                let receiver = source_node.get_first_child(ast).unwrap();

                builder = builder
                    .set_name(source_node.get_string(ast))
                    .set_receiver_type(Self::boxed_or_unknown(reg, ast, receiver.get_jstype(ast)))
                    // Props are always mutated as L-values, even when assigned `undefined`.
                    .set_mutation(is_l_value || parent.is_del_prop(ast))
                    .set_declaration(
                        parent.is_expr_result(ast)
                            || (jsdoc.as_ref().is_some_and(|jsdoc| jsdoc.is_constant())
                                && is_l_value),
                    )
                    // TODO(b/113704668): This definition is way too loose. It was used to prevent
                    // breakages during refactoring and should be tightened.
                    .set_override(jsdoc.is_some() && is_l_value)
                    .set_readable_type_name(ReadableTypeName::RegistryName(receiver));
            }
            Token::STRING_KEY
            | Token::GETTER_DEF
            | Token::SETTER_DEF
            | Token::MEMBER_FUNCTION_DEF
            | Token::MEMBER_FIELD_DEF => match parent.get_token(ast) {
                Token::OBJECTLIT => {
                    // TODO(b/80580110): Eventually object-literal members should be covered by
                    // `PropertyReference`s. However, doing so initially would have caused too many
                    // errors in existing code and delayed support for class syntax.
                    if !parent
                        .get_jstype(ast)
                        .expect("NullPointerException")
                        .is_literal_object(reg)
                    {
                        // Only add a mutation if the object type is actually a literal object
                        // (e.g. a global namespace).  OBJECTLIT tokens are often used to fulfill
                        // structural types, which is fine for writing constant properties the
                        // first time.
                        return None;
                    }
                    let receiver_type = Self::type_or_unknown(
                        reg,
                        closure_jstype::object_type::cast(reg, parent.get_jstype(ast)),
                    );
                    builder = builder
                        .set_name(source_node.get_string(ast))
                        .set_receiver_type(receiver_type)
                        .set_mutation(true)
                        .set_declaration(true)
                        .set_override(false)
                        .set_readable_type_name(ReadableTypeName::RegistryName(parent));
                }
                Token::OBJECT_PATTERN => {
                    let receiver_type = Self::type_or_unknown(
                        reg,
                        closure_jstype::object_type::cast(reg, parent.get_jstype(ast)),
                    );
                    builder = builder
                        .set_name(source_node.get_string(ast))
                        .set_receiver_type(receiver_type)
                        .set_mutation(false)
                        .set_declaration(false)
                        .set_override(false)
                        .set_readable_type_name(ReadableTypeName::RegistryName(parent));
                }
                Token::CLASS_MEMBERS => {
                    builder = builder
                        .set_name(source_node.get_string(ast))
                        .set_receiver_type(reg.get_native_object_type(JSTypeNative::UNKNOWN_TYPE))
                        .set_mutation(
                            !(source_node.is_member_field_def(ast)
                                && !source_node.has_children(ast)),
                        )
                        .set_declaration(true)
                        // TODO(b/113704668): This definition is way too loose. It was used to
                        // prevent breakages during refactoring and should be tightened.
                        .set_override(jsdoc.is_some())
                        .set_readable_type_name(ReadableTypeName::Empty); // The default is fine for class types.

                    let ctor_type = parent
                        .get_parent(ast)
                        .expect("NullPointerException")
                        .get_jstype(ast);
                    if let Some(ctor_type) = ctor_type
                        && ctor_type.is_function_type(reg)
                    {
                        let ctor_function_type = ctor_type.to_maybe_function_type(reg).unwrap();
                        let owning_type = if source_node.is_static_member(ast) {
                            ctor_function_type
                        } else if source_node.is_member_field_def(ast) {
                            FunctionType::get_instance_type(ctor_function_type, reg)
                                .expect("NullPointerException: Null receiverType")
                        } else {
                            ctor_function_type.get_prototype(reg, ast)
                        };
                        builder = builder.set_receiver_type(owning_type);
                    }
                }
                _ => panic!("AssertionError"),
            },
            _ => return None,
        }
        Some(builder.set_source_node(source_node).build())
    }

    /// Returns the effective visibility of the given property. This can differ from the
    /// property's declared visibility if the property is inherited from a superclass, or if the
    /// file's `@fileoverview` JsDoc specifies a default visibility.
    // port: CheckAccessControls#getEffectivePropertyVisibility
    fn get_effective_property_visibility(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        prop_ref: &PropertyReference,
        reference_type: Option<TypeId>,
        file_visibility_map: &FileVisibilityMap,
    ) -> Visibility {
        let property_name = &prop_ref.name;
        let is_override = prop_ref.override_;

        let defining_source = AccessControlUtils::get_defining_source(
            reg,
            ast,
            prop_ref.source_node,
            reference_type,
            property_name,
        );
        let file_overview_visibility = file_visibility_map.get(defining_source.as_ref());
        let object_type = AccessControlUtils::get_object_type(
            reg,
            ast,
            reference_type,
            is_override,
            property_name,
        );

        if is_override {
            let overridden = AccessControlUtils::get_overridden_property_visibility(
                reg,
                ast,
                object_type,
                property_name,
            );
            AccessControlUtils::get_effective_visibility_for_overridden_property(
                overridden,
                file_overview_visibility,
                property_name,
            )
        } else {
            Self::get_effective_visibility_for_non_overridden_property(
                reg,
                ast,
                prop_ref,
                object_type,
                file_overview_visibility,
            )
        }
    }

    /// Returns the effective visibility of the given non-overridden property. Non-overridden
    /// properties without an explicit visibility annotation receive the default visibility
    /// declared in the file's `@fileoverview` block, if one exists.
    // port: CheckAccessControls#getEffectiveVisibilityForNonOverriddenProperty
    fn get_effective_visibility_for_non_overridden_property(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        prop_ref: &PropertyReference,
        object_type: Option<TypeId>,
        file_overview_visibility: Option<Visibility>,
    ) -> Visibility {
        let property_name = &prop_ref.name;
        let mut raw = Visibility::INHERITED;
        if let Some(object_type) = object_type {
            let jsdoc = object_type.get_own_property_jsdoc_info(reg, ast, property_name);
            if let Some(jsdoc) = jsdoc {
                raw = jsdoc.get_visibility();
            }
        }
        let type_ = prop_ref.get_jstype(ast);
        let created_from_goog_provide = type_.is_some_and(|t| t.is_literal_object(reg));
        // Ignore @fileoverview visibility when computing the effective visibility
        // for properties created by goog.provide.
        //
        // ProcessClosurePrimitives rewrites goog.provide()s as object literal
        // declarations, but the exact form depends on the ordering of the
        // input files. If goog.provide('a.b.c') occurs in the inputs before
        // goog.provide('a'), it is rewritten like
        //
        // var a={};a.b={}a.b.c={};
        //
        // If the file containing goog.provide('a.b.c') also declares
        // a @fileoverview visibility, it must not apply to b, as this would make
        // every a.b.* namespace effectively package-private.
        match file_overview_visibility {
            Some(file_overview_visibility)
                if raw == Visibility::INHERITED && !created_from_goog_provide =>
            {
                file_overview_visibility
            }
            _ => raw,
        }
    }
}

/// Java's `Visibility#name()`.
fn visibility_name(v: Visibility) -> String {
    format!("{v:?}")
}

impl CompilerPass for CheckAccessControls {
    // port: CheckAccessControls#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let mut collect_pass = CollectFileOverviewVisibility::new(compiler);
        collect_pass.process(compiler, externs, root);
        self.default_visibility_for_files = collect_pass.get_file_overview_visibility_map();

        NodeTraversal::traverse(compiler, externs, self);
        NodeTraversal::traverse(compiler, root, self);
    }
}

impl Callback for CheckAccessControls {
    // port: CheckAccessControls#shouldTraverse
    fn should_traverse(
        &mut self,
        traversal: &mut NodeTraversal<'_>,
        node: NodeId,
        _parent: Option<NodeId>,
    ) -> bool {
        let access_control_root = Self::primary_access_control_scope_root_for(traversal, node);
        if let Some(access_control_root) = access_control_root {
            self.enter_access_control_scope(traversal.get_compiler(), access_control_root);
        }

        true
    }

    // port: CheckAccessControls#visit
    fn visit(&mut self, traversal: &mut NodeTraversal<'_>, node: NodeId, _parent: Option<NodeId>) {
        let identifier_behaviour = {
            let (reg, ast) = types(traversal.get_compiler());
            IdentifierBehaviour::select(reg, ast, node)
        };
        let prop_ref = Self::create_property_reference(traversal.get_compiler(), node);

        self.check_deprecation(node, prop_ref.as_ref(), identifier_behaviour, traversal);
        let scope = traversal.get_scope();
        self.check_visibility(
            traversal.get_compiler(),
            node,
            prop_ref.as_ref(),
            identifier_behaviour,
            scope,
        );
        self.check_constant_property(
            traversal.get_compiler(),
            prop_ref.as_ref(),
            identifier_behaviour,
        );

        self.check_final_class_overrides(traversal.get_compiler(), node);

        let access_control_root = Self::primary_access_control_scope_root_for(traversal, node);
        if let Some(access_control_root) = access_control_root {
            self.exit_access_control_scope(traversal, access_control_root);
        }
    }
}

/// The set of ways in which JSDoc and identifier usage can interact.
// port: CheckAccessControls.IdentifierBehaviour
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IdentifierBehaviour {
    NON_CONSTRUCTOR,
    ES5_CLASS_INVOCATION,
    ES5_CLASS_NAMESPACE,
    ES6_CLASS_INVOCATION,
    ES6_CLASS_NAMESPACE,
}

impl IdentifierBehaviour {
    // port: CheckAccessControls.IdentifierBehaviour#select
    fn select(reg: &mut JSTypeRegistry, ast: &Ast, target: NodeId) -> IdentifierBehaviour {
        let type_ = target.get_jstype(ast);
        let Some(type_) = type_.filter(|t| t.is_function_type(reg)) else {
            // If we aren't sure what we're dealing with be more strict.
            return IdentifierBehaviour::NON_CONSTRUCTOR;
        };

        let ctor_type = type_.to_maybe_function_type(reg).unwrap();
        if !ctor_type.is_constructor(reg) {
            return IdentifierBehaviour::NON_CONSTRUCTOR;
        }

        let is_invocation = NodeUtil::is_invocation_target(ast, target)
            || CheckAccessControls::is_extends_target(ast, target);
        let is_es6 =
            FunctionType::get_source(ctor_type, reg).is_some_and(|source| source.is_class(ast));

        if !is_es6 {
            if is_invocation {
                IdentifierBehaviour::ES5_CLASS_INVOCATION
            } else {
                IdentifierBehaviour::ES5_CLASS_NAMESPACE
            }
        } else if is_invocation {
            IdentifierBehaviour::ES6_CLASS_INVOCATION
        } else {
            IdentifierBehaviour::ES6_CLASS_NAMESPACE
        }
    }
}

/// The lazy `Supplier<String> readableTypeName` of a PropertyReference: the lambdas
/// CheckAccessControls passes, evaluated only when a message needs them.
#[derive(Clone, Copy, Debug)]
enum ReadableTypeName {
    /// `() -> typeRegistry.getReadableTypeName(node)`
    RegistryName(NodeId),
    /// `() -> ""`
    Empty,
    /// `() -> ctorType.getInstanceType().toString()`
    InstanceTypeOf(TypeId),
}

impl ReadableTypeName {
    // port: java.util.function.Supplier#get
    fn get(self, reg: &mut JSTypeRegistry, ast: &Ast) -> String {
        match self {
            ReadableTypeName::RegistryName(n) => reg.get_readable_type_name(ast, n),
            ReadableTypeName::Empty => String::new(),
            ReadableTypeName::InstanceTypeOf(ctor_type) => {
                FunctionType::get_instance_type(ctor_type, reg)
                    .expect("NullPointerException")
                    .to_string(reg, ast)
            }
        }
    }
}

/// A representation of an object property reference in JS code.
///
/// This is an abstraction to smooth over the various AST structures that can act on
/// *properties*. It is not useful for names (variables) or anonymous JS constructs.
// port: CheckAccessControls.PropertyReference
#[derive(Clone, Debug)]
struct PropertyReference {
    source_node: NodeId,
    name: JsString,
    receiver_type: TypeId,
    mutation: bool,
    declaration: bool,
    override_: bool,
    readable_type_name: ReadableTypeName,
}

// port: CheckAccessControls.PropertyReference.Builder
#[derive(Default)]
struct PropertyReferenceBuilder {
    source_node: Option<NodeId>,
    name: Option<JsString>,
    receiver_type: Option<TypeId>,
    mutation: Option<bool>,
    declaration: Option<bool>,
    override_: Option<bool>,
    readable_type_name: Option<ReadableTypeName>,
}

impl PropertyReferenceBuilder {
    // port: CheckAccessControls.PropertyReference.Builder#setSourceNode
    fn set_source_node(mut self, node: NodeId) -> Self {
        self.source_node = Some(node);
        self
    }
    // port: CheckAccessControls.PropertyReference.Builder#setName
    fn set_name(mut self, name: JsString) -> Self {
        self.name = Some(name);
        self
    }
    // port: CheckAccessControls.PropertyReference.Builder#setReceiverType
    fn set_receiver_type(mut self, receiver_type: TypeId) -> Self {
        self.receiver_type = Some(receiver_type);
        self
    }
    // port: CheckAccessControls.PropertyReference.Builder#setMutation
    fn set_mutation(mut self, is_mutation: bool) -> Self {
        self.mutation = Some(is_mutation);
        self
    }
    // port: CheckAccessControls.PropertyReference.Builder#setDeclaration
    fn set_declaration(mut self, is_declaration: bool) -> Self {
        self.declaration = Some(is_declaration);
        self
    }
    // port: CheckAccessControls.PropertyReference.Builder#setOverride
    fn set_override(mut self, is_override: bool) -> Self {
        self.override_ = Some(is_override);
        self
    }
    // port: CheckAccessControls.PropertyReference.Builder#setReadableTypeName
    fn set_readable_type_name(mut self, type_name: ReadableTypeName) -> Self {
        self.readable_type_name = Some(type_name);
        self
    }
    // port: CheckAccessControls.PropertyReference.Builder#build
    fn build(self) -> PropertyReference {
        PropertyReference {
            source_node: self
                .source_node
                .expect("Missing required properties: sourceNode"),
            name: self.name.expect("Missing required properties: name"),
            receiver_type: self
                .receiver_type
                .expect("Missing required properties: receiverType"),
            mutation: self
                .mutation
                .expect("Missing required properties: mutation"),
            declaration: self
                .declaration
                .expect("Missing required properties: declaration"),
            override_: self
                .override_
                .expect("Missing required properties: override"),
            readable_type_name: self
                .readable_type_name
                .expect("Missing required properties: readableTypeName"),
        }
    }
}

impl PropertyReference {
    // port: CheckAccessControls.PropertyReference#builder
    fn builder() -> PropertyReferenceBuilder {
        PropertyReferenceBuilder::default()
    }

    // Derived properties.

    // port: CheckAccessControls.PropertyReference#getJSType
    fn get_jstype(&self, ast: &Ast) -> Option<TypeId> {
        self.source_node.get_jstype(ast)
    }

    // port: CheckAccessControls.PropertyReference#getJSDocInfo
    fn get_jsdoc_info(&self, ast: &Ast) -> Option<Arc<JSDocInfo>> {
        NodeUtil::get_best_jsdoc_info(ast, self.source_node)
    }

    // port: CheckAccessControls.PropertyReference#isDocumentedDeclaration
    fn is_documented_declaration(&self, ast: &Ast) -> bool {
        self.declaration && self.get_jsdoc_info(ast).is_some()
    }

    // port: CheckAccessControls.PropertyReference#isDeletion
    fn is_deletion(&self, ast: &Ast) -> bool {
        self.source_node
            .get_parent(ast)
            .expect("NullPointerException")
            .is_del_prop(ast)
    }

    // port: CheckAccessControls.PropertyReference#getReadableTypeNameOrDefault
    fn get_readable_type_name_or_default(&self, reg: &mut JSTypeRegistry, ast: &Ast) -> String {
        let preferred = self.readable_type_name.get(reg, ast);
        if preferred.is_empty() {
            self.receiver_type.to_string(reg, ast)
        } else {
            preferred
        }
    }
}
