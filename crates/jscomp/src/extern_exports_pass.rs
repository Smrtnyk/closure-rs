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
//   src/com/google/javascript/jscomp/ExternExportsPass.java.

//! Port of `ExternExportsPass.java`.
//!
//! Creates an externs file containing all exported symbols and properties for later consumption.

use crate::AbstractCompiler;
use crate::code_printer;
use crate::compiler_pass::CompilerPass;
use crate::default_name_generator::DefaultNameGenerator;
use crate::name_generator::NameGenerator;
use crate::node_traversal::{
    AbstractPostOrderCallback, AbstractPostOrderCallbackInterface, NodeTraversal,
};
use crate::node_util::NodeUtil;
use closure_jstype::{TypeId, function_type::FunctionType, js_type::JSType};
use closure_rhino::ir::IR;
use closure_rhino::js_string::JsString;
use closure_rhino::jsdoc_info::JSDocInfo;
use closure_rhino::node::{Ast, NodeId};
use closure_rhino::{check_not_null, check_state};
use indexmap::{IndexMap, IndexSet};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

// port: ExternExportsPass#Q_NAME_JOINER
/// `Joiner.on('.').join(a, b)`.
fn q_name_join(a: &JsString, b: &JsString) -> JsString {
    a.concat(&JsString::from(".")).concat(b)
}

// port: ExternExportsPass#Q_NAME_SPLITTER
/// `Splitter.on('.').splitToList(s)`: every '.' separates, empty pieces are kept.
fn q_name_split_to_list(s: &JsString) -> Vec<JsString> {
    let units = s.as_units();
    let mut pieces = Vec::new();
    let mut start = 0;
    for (i, &c) in units.iter().enumerate() {
        if c == u16::from(b'.') {
            pieces.push(s.substring(start, i));
            start = i + 1;
        }
    }
    pieces.push(s.substring(start, units.len()));
    pieces
}

/// Rust-only: Java's `SymbolExport` / `PropertyExport` subclasses of the abstract `Export`.
enum ExportKind {
    /// A symbol export.
    Symbol,
    /// A property export.
    Property { export_path: JsString },
}

// port: ExternExportsPass.Export
struct Export {
    symbol_name: JsString,
    value: NodeId,
    kind: ExportKind,
}

pub struct ExternExportsPass {
    /// The exports found.
    exports: Vec<Export>,
    /// A map of all assigns to their parent nodes.
    definition_map: IndexMap<JsString, NodeId>,
    /// The AST root which holds the externs generated.
    externs_root: NodeId,
    /// A mapping of internal paths to exported paths.
    mapped_paths: IndexMap<JsString, JsString>,
    /// A list of exported paths.
    already_exported_paths: IndexSet<JsString>,
    /// A list of function names used to export symbols.
    export_symbol_function_names: IndexSet<JsString>,
    /// A list of function names used to export properties.
    export_property_function_names: IndexSet<JsString>,
}

impl ExternExportsPass {
    /// Creates an instance.
    // port: ExternExportsPass#ExternExportsPass
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        let externs_root = IR::script(compiler);
        let mut pass = Self {
            exports: Vec::new(),
            definition_map: IndexMap::new(),
            externs_root,
            already_exported_paths: IndexSet::new(),
            mapped_paths: IndexMap::new(),
            export_symbol_function_names: IndexSet::new(),
            export_property_function_names: IndexSet::new(),
        };
        pass.init_export_methods(compiler);
        pass
    }

    // port: ExternExportsPass#initExportMethods
    fn init_export_methods(&mut self, compiler: &AbstractCompiler) {
        let convention = compiler.get_coding_convention();
        // ImmutableSet.of throws on a null element.
        self.export_symbol_function_names = IndexSet::from([
            convention
                .get_export_symbol_function()
                .expect("NullPointerException: getExportSymbolFunction"), // goog.exportSymbol(name, value)
            JsString::from("google_exportSymbol"), // used within Google
        ]);

        self.export_property_function_names = IndexSet::from([
            convention
                .get_export_property_function()
                .expect("NullPointerException: getExportPropertyFunction"), // goog.exportProperty(owner, name, value)
            JsString::from("google_exportProperty"), // used within Google
        ]);
    }

    // port: ExternExportsPass#process
    pub fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        NodeTraversal::traverse(
            compiler,
            root,
            &mut AbstractPostOrderCallback::new(ExternExportsVisitor { pass: self }),
        );

        // Sort by path length to ensure that the longer
        // paths (which may depend on the shorter ones)
        // come later.
        // A TreeSet ordered by getExportedPath: an export whose path equals an earlier one's is
        // dropped.
        let mut sorted: BTreeMap<JsString, usize> = BTreeMap::new();

        for (i, export) in self.exports.iter().enumerate() {
            sorted.entry(self.get_exported_path(export)).or_insert(i);
        }

        let exports = std::mem::take(&mut self.exports);
        for &i in sorted.values() {
            self.generate_externs(compiler, &exports[i]);
        }
        self.exports = exports;

        self.set_generated_externs_on_compiler(compiler);
    }

    // port: ExternExportsPass#setGeneratedExternsOnCompiler
    fn set_generated_externs_on_compiler(&self, compiler: &mut AbstractCompiler) {
        let (registry, ast) = compiler.get_type_registry_and_ast();
        let builder = code_printer::Builder::new(self.externs_root)
            .set_pretty_print(true)
            .set_output_types(true)
            .set_type_registry(registry);
        let built = builder.build(ast);

        compiler.set_extern_exports(
            String::from(concat!(
                "/**\n",
                " * @fileoverview Generated externs.\n",
                " * @externs\n",
                " */\n",
            )) + &built.to_string_lossy(),
        );
    }

    /// Computes a list of the path prefixes constructed from the components of the path.
    ///
    /// <pre>
    /// E.g., if the path is:
    ///      "a.b.c"
    /// then then path prefixes will be
    ///    ["a","a.b","a.b.c"]:
    /// </pre>
    // port: ExternExportsPass#computePathPrefixes
    fn compute_path_prefixes(path: &JsString) -> Vec<JsString> {
        let pieces = q_name_split_to_list(path);
        let mut path_prefixes = Vec::new();

        let mut partial = pieces[0].clone(); // There will always be at least 1.
        path_prefixes.push(partial.clone());
        for piece in pieces.iter().skip(1) {
            partial = q_name_join(&partial, piece);
            path_prefixes.push(partial.clone());
        }

        path_prefixes
    }

    // port: ExternExportsPass.SymbolExport#SymbolExport
    fn new_symbol_export(&mut self, ast: &Ast, symbol_name: JsString, value: NodeId) -> Export {
        let export = Export::new(symbol_name, value, ExportKind::Symbol);

        let qualified_name = value.get_qualified_name(ast);

        if let Some(qualified_name) = qualified_name {
            self.mapped_paths
                .insert(qualified_name, export.symbol_name.clone());
        }
        export
    }

    // port: ExternExportsPass.PropertyExport#PropertyExport
    fn new_property_export(export_path: JsString, symbol_name: JsString, value: NodeId) -> Export {
        Export::new(symbol_name, value, ExportKind::Property { export_path })
    }

    /// Generates the externs representation of this export and appends it to the externsRoot
    /// AST.
    // port: ExternExportsPass.Export#generateExterns
    fn generate_externs(&mut self, compiler: &mut AbstractCompiler, export: &Export) {
        let path = self.get_exported_path(export);
        let value = self.get_value(compiler, export);
        self.append_extern(compiler, &path, value);
    }

    /// Returns the path exported by this export.
    // port: ExternExportsPass.Export#getExportedPath
    fn get_exported_path(&self, export: &Export) -> JsString {
        match &export.kind {
            // port: ExternExportsPass.SymbolExport#getExportedPath
            ExportKind::Symbol => export.symbol_name.clone(),
            // port: ExternExportsPass.PropertyExport#getExportedPath
            ExportKind::Property { export_path } => {
                // Find the longest path that has been mapped (if any).
                for current_path in Self::compute_path_prefixes(export_path).iter().rev() {
                    check_state!(current_path.length() > 0);

                    // If this path is mapped, return the mapped path plus any remaining pieces.
                    let Some(mapped_path) = self.mapped_paths.get(current_path) else {
                        continue;
                    };

                    // Append the remaining path segments, including a leading separator.
                    let mapped_path =
                        mapped_path.concat(&export_path.substring_from(current_path.length()));
                    return q_name_join(&mapped_path, &export.symbol_name);
                }

                q_name_join(export_path, &export.symbol_name)
            }
        }
    }

    /// Appends the exported function and all paths necessary for the path to be declared. For
    /// example, for a property "a.b.c", the initializers for paths "a", "a.b" will be appended
    /// (if they have not already) and a.b.c will be initialized with the exported version of the
    /// function:
    /// <pre>
    /// var a = {};
    /// a.b = {};
    /// a.b.c = function(x,y) { }
    /// </pre>
    // port: ExternExportsPass.Export#appendExtern
    fn append_extern(
        &mut self,
        compiler: &mut AbstractCompiler,
        path: &JsString,
        value_to_export: Option<NodeId>,
    ) {
        let path_prefixes = Self::compute_path_prefixes(path);

        for (i, path_prefix) in path_prefixes.iter().enumerate() {
            // The complete path (the last path prefix) must be emitted and
            // it gets initialized to the externed version of the value.
            let is_complete_path_prefix = i == path_prefixes.len() - 1;

            let skip_path_prefix = path_prefix.ends_with(&JsString::from(".prototype"))
                || (self.already_exported_paths.contains(path_prefix) && !is_complete_path_prefix);
            if skip_path_prefix {
                continue;
            }

            let mut exported_value_defines_new_type = false;

            if let Some(value_to_export) = value_to_export {
                let jsdoc = NodeUtil::get_best_jsdoc_info(compiler, value_to_export);
                if value_to_export.is_class(compiler)
                    || jsdoc.is_some_and(|jsdoc| jsdoc.contains_type_definition())
                {
                    exported_value_defines_new_type = true;
                }
            }

            // Namespaces get initialized to {}, functions to externed versions of their value, and
            // if we can't figure out where the value came from we initialize it to {}.
            //
            // Since externs are always exported in sorted order, we know that if we export a.b =
            // function() {} and later a.b.c = function then a.b will always be in
            // alreadyExportedPaths when we emit a.b.c and thus we will never overwrite the
            // function exported for a.b with a namespace.
            let initializer;
            let mut jsdoc = None;
            if let Some(value_to_export) = value_to_export
                && is_complete_path_prefix
            {
                if value_to_export.is_function(compiler) {
                    initializer = Self::create_extern_function(compiler, value_to_export);
                } else if value_to_export.is_class(compiler) {
                    initializer =
                        Self::create_extern_function_for_es6_class(compiler, value_to_export);
                } else {
                    check_state!(value_to_export.is_object_lit(compiler));
                    initializer = Self::create_extern_object_lit(compiler, value_to_export);
                }
            } else if !is_complete_path_prefix && exported_value_defines_new_type {
                jsdoc = Self::build_namespace_js_doc();
                let objectlit = IR::objectlit(compiler, &[]);
                initializer = Self::create_extern_object_lit(compiler, objectlit);
                // Don't add the empty jsdoc here
                initializer.set_jsdoc_info(compiler, None);
            } else {
                initializer = IR::empty(compiler);
            }

            self.append_path_definition(compiler, path_prefix, initializer, jsdoc);
        }
    }

    // port: ExternExportsPass.Export#appendPathDefinition
    fn append_path_definition(
        &mut self,
        compiler: &mut AbstractCompiler,
        path: &JsString,
        initializer: NodeId,
        jsdoc: Option<Arc<JSDocInfo>>,
    ) {
        let path_definition;

        if path.index_of_char(u16::from(b'.')) >= 0 {
            let qualified_path = NodeUtil::new_qname(compiler, path.clone());
            if initializer.is_empty(compiler) {
                path_definition = NodeUtil::new_expr(compiler, qualified_path);
            } else {
                let assign = IR::assign(compiler, qualified_path, initializer);
                path_definition = NodeUtil::new_expr(compiler, assign);
            }
        } else if initializer.is_empty(compiler) {
            let name = IR::name(compiler, path.clone());
            path_definition = IR::var(compiler, name);
        } else {
            path_definition = NodeUtil::new_var_node(compiler, path.clone(), Some(initializer));
        }

        if let Some(jsdoc) = jsdoc {
            if path_definition.is_expr_result(compiler) {
                path_definition
                    .get_first_child(compiler)
                    .unwrap()
                    .set_jsdoc_info(compiler, Some(jsdoc));
            } else {
                check_state!(path_definition.is_var(compiler));
                path_definition.set_jsdoc_info(compiler, Some(jsdoc));
            }
        }

        self.externs_root
            .add_child_to_back(compiler, path_definition);

        self.already_exported_paths.insert(path.clone());
    }

    /// Given a function to export, create the empty function that will be put in the externs
    /// file. This extern function should have the same type as the original function and the
    /// same parameter name but no function body.
    ///
    /// We create a warning here if the the function to export is missing parameter or return
    /// types.
    // port: ExternExportsPass.Export#createExternFunction
    fn create_extern_function(ast: &mut Ast, exported_function: NodeId) -> NodeId {
        let param_list =
            Self::create_externs_param_list_from_original_function(ast, exported_function);
        let name = IR::name(ast, "");
        let block = IR::block(ast);
        let extern_function = IR::function(ast, name, param_list, block);

        let t = exported_function.get_jstype(ast);
        extern_function.set_jstype(ast, t);

        extern_function
    }

    /// Creates a PARAM_LIST to store in the AST we'll use to generate externs for a function with
    /// the given type.
    ///
    /// If the NODE defining the original function is available, it would be better to use
    /// createExternsParamListFromOriginalFunction(), because that one will keep the parameter
    /// names the same instead of generating arbitrary parameter names.
    ///
    /// `exported_function`: FUNCTION Node of the original function
    // port: ExternExportsPass.Export#createExternsParamListFromOriginalFunction
    fn create_externs_param_list_from_original_function(
        ast: &mut Ast,
        exported_function: NodeId,
    ) -> NodeId {
        let original_param_list = NodeUtil::get_function_parameters(ast, exported_function);
        // First get all of the original positional parameter list names we can.
        // Place empty stings in the positions where we'll need to generate names.
        let mut original_param_names = Vec::new();
        let mut original_param = original_param_list.get_first_child(ast);
        while let Some(param) = original_param {
            // We'll get an empty string for a destructuring pattern.
            // Also if originalParamList came from a FunctionType instead of an actual FUNCTION
            // node, then all of the NAME nodes in it will have empty strings, so we'll end up
            // generating names for all of them.
            original_param_names.push(Self::get_original_name_for_param(ast, param));
            original_param = param.get_next(ast);
        }
        Self::create_externs_param_list_from_original_param_list(ast, &original_param_names)
    }

    /// Creates a PARAM_LIST to store in the AST we'll use to generate externs for a function with
    /// the given type.
    ///
    /// If the NODE defining the original function is available, it would be better to use
    /// createExternsParamListFromOriginalFunction(), because that one will keep the parameter
    /// names the same instead of generating arbitrary parameter names.
    ///
    /// `function_type`: JSType read from the FUNCTION (or possibly CLASS) node
    // port: ExternExportsPass.Export#createExternsParamListFromFunctionType
    fn create_externs_param_list_from_function_type(
        compiler: &mut AbstractCompiler,
        function_type: Option<TypeId>,
    ) -> NodeId {
        // Place empty stings in the positions where we'll need to generate names.
        let function_type = check_not_null!(function_type, "NullPointerException");
        let (registry, ast) = compiler.get_type_registry_and_ast();
        let parameter_count = function_type
            .assert_function_type(registry, ast)
            .get_parameters(registry)
            .len();
        let empty_param_names = vec![JsString::from(""); parameter_count];

        Self::create_externs_param_list_from_original_param_list(compiler, &empty_param_names)
    }

    /// Creates a PARAM_LIST to store in the AST we'll use to generate externs for a function.
    ///
    /// `original_param_names`: names for the parameters, possibly synthetic.
    // port: ExternExportsPass.Export#createExternsParamListFromOriginalParamList
    fn create_externs_param_list_from_original_param_list(
        ast: &mut Ast,
        original_param_names: &[JsString],
    ) -> NodeId {
        let param_list = IR::param_list(ast, &[]);
        let mut name_generator = DefaultNameGenerator::with_reserved_characters(
            Arc::new(RwLock::new(original_param_names.iter().cloned().collect())),
            JsString::from(""),
            /* reservedCharacters= */ &IndexSet::new(),
        );
        for original_param_name in original_param_names {
            let extern_param_name = if original_param_name.is_empty() {
                name_generator.generate_next_name()
            } else {
                original_param_name.clone()
            };
            let name = IR::name(ast, extern_param_name);
            param_list.add_child_to_back(ast, name);
        }
        param_list
    }

    /// `param_node`: expected to be a node in a PARAM_LIST
    ///
    /// Returns the original name of the parameter, if possible, otherwise an empty string.
    // port: ExternExportsPass.Export#getOriginalNameForParam
    fn get_original_name_for_param(ast: &Ast, param_node: NodeId) -> JsString {
        let name_or_pattern_node = if param_node.is_rest(ast) {
            // get name or pattern from `...nameOrPattern`
            param_node.get_only_child(ast)
        } else if param_node.is_default_value(ast) {
            // get name or pattern from `nameOrPattern = defaultValue`
            param_node.get_first_child(ast).unwrap()
        } else {
            param_node
        };
        if name_or_pattern_node.is_name(ast) {
            let original_name = name_or_pattern_node.get_original_name(ast);
            original_name.unwrap_or_else(|| name_or_pattern_node.get_string(ast))
        } else {
            check_state!(
                name_or_pattern_node.is_destructuring_pattern(ast),
                "%s",
                name_or_pattern_node.to_string(ast)
            );
            JsString::from("")
        }
    }

    /// Given a class to export, create the empty function that will be put in the externs file.
    ///
    /// This extern function should have the same type as the original function and the same
    /// parameter name but no function body.
    ///
    /// TODO(b/123352214): It would be nice if we could put ES6 classes in the generated externs,
    /// but we'd have to fix some things first.
    // port: ExternExportsPass.Export#createExternFunctionForEs6Class
    fn create_extern_function_for_es6_class(
        compiler: &mut AbstractCompiler,
        exported_class: NodeId,
    ) -> NodeId {
        let constructor_method_definition =
            NodeUtil::get_es6_class_constructor_member_function_def(compiler, exported_class);
        match constructor_method_definition {
            None => {
                // no constructor for the class, so just create an empty function with parameters
                // to match the parameters indicated in the JSType, which should have inherited
                // parameters from the superclass, if any.
                let class_js_type = exported_class.get_jstype(compiler);
                let param_list =
                    Self::create_externs_param_list_from_function_type(compiler, class_js_type);
                let name = IR::name(compiler, "");
                let block = IR::block(compiler);
                let extern_function = IR::function(compiler, name, param_list, block);
                extern_function.set_jstype(compiler, class_js_type);
                extern_function
            }
            Some(constructor_method_definition) => {
                // The JSType on the constructor function definition is the same as the JSType on
                // the whole class, so we can just pretend that the function is an ES5 constructor
                // function.
                let only_child = constructor_method_definition.get_only_child(compiler);
                Self::create_extern_function(compiler, only_child)
            }
        }
    }

    // port: ExternExportsPass.Export#buildEmptyJSDoc
    fn build_empty_js_doc() -> Option<Arc<JSDocInfo>> {
        // TODO(johnlenz): share the JSDocInfo here rather than building
        // a new one each time.
        JSDocInfo::builder().build_with_always(true)
    }

    // port: ExternExportsPass.Export#buildNamespaceJSDoc
    fn build_namespace_js_doc() -> Option<Arc<JSDocInfo>> {
        let mut builder = JSDocInfo::builder();
        builder.record_constancy();
        builder.record_suppressions(&IndexSet::from([
            JsString::from("const"),
            JsString::from("duplicate"),
        ]));
        builder.build()
    }

    /// Given an object literal to export, create an object lit with all its string properties. We
    /// don't care what the values of those properties are because they are not checked.
    // port: ExternExportsPass.Export#createExternObjectLit
    fn create_extern_object_lit(ast: &mut Ast, exported_object_lit: NodeId) -> NodeId {
        let lit = IR::objectlit(ast, &[]);
        let t = exported_object_lit.get_jstype(ast);
        lit.set_jstype(ast, t);

        // This is an indirect way of telling the typed code generator
        // "print the type of this"
        lit.set_jsdoc_info(ast, Self::build_empty_js_doc());

        let mut index = 1;
        let mut child = exported_object_lit.get_first_child(ast);
        while let Some(c) = child {
            // TODO(dimvar): handle getters or setters?
            if c.is_string_key(ast) {
                let key = IR::string_key(ast, c.get_string(ast));
                let number = IR::number(ast, f64::from(index));
                index += 1;
                let propdef = IR::propdef(ast, key, number);
                lit.add_child_to_back(ast, propdef);
            }
            child = c.get_next(ast);
        }
        lit
    }

    /// If the given value is a qualified name which refers a function or object literal, the node
    /// is returned. Otherwise, `None` is returned.
    // port: ExternExportsPass.Export#getValue
    fn get_value(&self, ast: &Ast, export: &Export) -> Option<NodeId> {
        let qualified_name = export.value.get_qualified_name(ast);

        let Some(qualified_name) = qualified_name else {
            // We expect to see
            // goog.exportSymbol('exportedName', some.path);
            // goog.exportProperty(some.path, 'exportedName', some.path.prop);
            //
            // In either case `value` will be the last argument, which we expect to be a qualified
            // name If it isn't we won't include any type information in the output externs.
            // It would be very strange to use a literal value as the final argument, since it
            // wouldn't then be accessible by any non-exported name.
            return None;
        };

        let definition = self.definition_map.get(&qualified_name).copied();
        let Some(definition) = definition else {
            // Couldn't find any assignment to the qualified name
            return None;
        };

        if definition.is_function(ast) || definition.is_class(ast) || definition.is_object_lit(ast)
        {
            // We can generate good type information for all of these cases.
            return Some(definition);
        }

        // value was something unusual, so we won't return any node from which to get type
        // information.
        None
    }

    // port: ExternExportsPass#lookForQnameDefinition
    fn look_for_qname_definition(&mut self, ast: &Ast, n: NodeId) {
        // TODO(b/123725559): There are lots of cases where this could fail to find the right
        //     definition or be fooled by there being multiple definitions.
        if n.is_class(ast) {
            if NodeUtil::is_class_declaration(ast, n) {
                // class Foo {...}
                self.definition_map
                    .insert(n.get_first_child(ast).unwrap().get_string(ast), n);
            }
        } else if n.is_function(ast) {
            if NodeUtil::is_function_declaration(ast, n) {
                // function foo() {...}
                self.definition_map
                    .insert(n.get_first_child(ast).unwrap().get_string(ast), n);
            }
        } else if n.is_assign(ast) {
            // TODO(b/123718645): Add support for destructuring assignments
            let lhs = n.get_first_child(ast).unwrap();
            if lhs.is_qualified_name(ast)
                && (!lhs.has_children(ast) || !lhs.get_first_child(ast).unwrap().is_this(ast))
            {
                // qualified.name = value;
                //   but not
                // this.prop = value;
                self.definition_map.insert(
                    lhs.get_qualified_name(ast).unwrap(),
                    n.get_last_child(ast).unwrap(),
                );
            }
        } else if n.is_name(ast) {
            // TODO(b/123718645): Add support for destructuring declarations
            let parent = check_not_null!(n.get_parent(ast), "%s", n.to_string(ast));
            if NodeUtil::is_name_declaration(ast, Some(parent)) {
                let value = n.get_first_child(ast);
                if let Some(value) = value {
                    // const foo = value;
                    self.definition_map.insert(n.get_string(ast), value);
                }
            }
        } else if n.is_member_function_def(ast) {
            // Try to find a fully qualified name for the method
            let lvalue_name = NodeUtil::get_best_l_value_name(ast, Some(n));
            if let Some(lvalue_name) = lvalue_name {
                // Store the function as the value
                self.definition_map
                    .insert(lvalue_name, n.get_only_child(ast));
            }
        }
        // TODO(b/123725422): Getters and setters?
    }

    // port: ExternExportsPass#lookForSymbolExportCall
    fn look_for_symbol_export_call(&mut self, ast: &Ast, n: NodeId) {
        if !Self::is_call_to_one_of(ast, n, &self.export_symbol_function_names) {
            return; // not a call to goog.exportSymbol()
        }
        // TODO(b/123725716): We should report errors for malformed calls instead of just ignoring
        // them.
        // Ensure that we only check valid calls with the 2 arguments
        // (plus the GETPROP node itself).
        if !n.has_x_children(ast, 3) {
            return;
        }

        let this_node = n.get_first_child(ast).unwrap();
        let name_arg = this_node.get_next(ast).unwrap();
        let value_arg = name_arg.get_next(ast).unwrap();

        // Confirm the arguments are the expected types. If they are not,
        // then we have an export that we cannot statically identify.
        if !name_arg.is_string_lit(ast) {
            return;
        }

        // Add the export to the list.
        let export = self.new_symbol_export(ast, name_arg.get_string(ast), value_arg);
        self.exports.push(export);
    }

    // port: ExternExportsPass#lookForPropertyExportCall
    fn look_for_property_export_call(&mut self, ast: &Ast, n: NodeId) {
        if !Self::is_call_to_one_of(ast, n, &self.export_property_function_names) {
            return; // not a call to goog.exportProperty()
        }
        // TODO(b/123725716): We should report errors for malformed calls instead of just ignoring
        // them.
        // Ensure that we only check valid calls with the 3 arguments
        // (plus the GETPROP node itself).
        if !n.has_x_children(ast, 4) {
            return;
        }

        let this_node = n.get_first_child(ast).unwrap();
        let object_arg = this_node.get_next(ast).unwrap();
        let name_arg = object_arg.get_next(ast).unwrap();
        let value_arg = name_arg.get_next(ast).unwrap();

        // Confirm the arguments are the expected types. If they are not,
        // then we have an export that we cannot statically identify.
        if !object_arg.is_qualified_name(ast) {
            return;
        }

        if !name_arg.is_string_lit(ast) {
            return;
        }

        // Add the export to the list.
        self.exports.push(Self::new_property_export(
            object_arg.get_qualified_name(ast).unwrap(),
            name_arg.get_string(ast),
            value_arg,
        ));
    }

    // port: ExternExportsPass#isCallToOneOf
    fn is_call_to_one_of(ast: &Ast, n: NodeId, function_qnames: &IndexSet<JsString>) -> bool {
        if !n.is_call(ast) {
            false
        } else {
            let callee = n.get_first_child(ast).unwrap();
            callee.is_qualified_name(ast)
                && function_qnames.contains(&callee.get_qualified_name(ast).unwrap())
        }
    }

    // port: ExternExportsPass#lookForAtExportOnThisDotProperty
    fn look_for_at_export_on_this_dot_property(
        &mut self,
        t: &mut NodeTraversal<'_>,
        this_dot_prop_name: NodeId,
    ) {
        if !this_dot_prop_name.is_get_prop(t)
            || !this_dot_prop_name.get_first_child(t).unwrap().is_this(t)
        {
            return; // not this.propName
        }
        let jsdoc = NodeUtil::get_best_jsdoc_info(t, this_dot_prop_name);
        if !jsdoc.is_some_and(|jsdoc| jsdoc.is_export()) {
            return; // no @export on this.propName
        }

        let constructor_node = t.get_enclosing_function();
        let is_constructor = {
            let (registry, ast) = t.get_compiler().get_type_registry_and_ast();
            NodeUtil::is_constructor(ast, constructor_node, registry)
        };
        if !is_constructor {
            return; // @export on this.propName only works within a constructor
        }
        let constructor_node = constructor_node.unwrap();

        let class_node = if NodeUtil::is_es6_constructor(t, constructor_node) {
            NodeUtil::get_enclosing_class(t, constructor_node)
        } else {
            Some(constructor_node)
        };
        // Java string concatenation renders a null className as "null".
        let class_name = class_node
            .and_then(|class_node| NodeUtil::get_name(t, class_node))
            .unwrap_or_else(|| JsString::from("null"));
        let property_name = this_dot_prop_name.get_string(t);
        let prototype_name = class_name.concat(&JsString::from(".prototype"));
        let property_name_node = NodeUtil::new_qname(
            t.get_compiler(),
            JsString::from("this.").concat(&property_name),
        );

        // Add the export to the list.
        self.exports.push(Self::new_property_export(
            prototype_name,
            property_name,
            property_name_node,
        ));
    }
}

impl Export {
    // port: ExternExportsPass.Export#Export
    fn new(symbol_name: JsString, value: NodeId, kind: ExportKind) -> Self {
        Self {
            symbol_name,
            value,
            kind,
        }
    }
}

impl CompilerPass for ExternExportsPass {
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        Self::process(self, compiler, externs, root);
    }
}

/// Rust-only: Java's ExternExportsPass is its own AbstractPostOrderCallback.
struct ExternExportsVisitor<'a> {
    pass: &'a mut ExternExportsPass,
}

impl AbstractPostOrderCallbackInterface for ExternExportsVisitor<'_> {
    // port: ExternExportsPass#visit
    fn visit(&mut self, t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>) {
        self.pass.look_for_qname_definition(t, n);
        self.pass.look_for_at_export_on_this_dot_property(t, n);
        self.pass.look_for_symbol_export_call(t, n);
        self.pass.look_for_property_export_call(t, n);
    }
}
