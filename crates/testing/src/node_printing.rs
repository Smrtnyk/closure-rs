/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Norris Boyd
 *   Roger Lawrence
 *   Mike McCabe
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
/*
 *
 * ***** BEGIN LICENSE BLOCK *****
 * Version: MPL 1.1/GPL 2.0
 *
 * The contents of this file are subject to the Mozilla Public License Version
 * 1.1 (the "License"); you may not use this file except in compliance with
 * the License. You may obtain a copy of the License at
 * http://www.mozilla.org/MPL/
 *
 * Software distributed under the License is distributed on an "AS IS" basis,
 * WITHOUT WARRANTY OF ANY KIND, either express or implied. See the License
 * for the specific language governing rights and limitations under the
 * License.
 *
 * The Original Code is Rhino code, released
 * May 6, 1999.
 *
 * The Initial Developer of the Original Code is
 * Netscape Communications Corporation.
 * Portions created by the Initial Developer are Copyright (C) 1997-1999
 * the Initial Developer. All Rights Reserved.
 *
 * Contributor(s):
 *   Bob Jervis
 *   Google Inc.
 *
 * Alternatively, the contents of this file may be used under the terms of
 * the GNU General Public License Version 2 or later (the "GPL"), in which
 * case the provisions of the GPL are applicable instead of those above. If
 * you wish to allow use of your version of this file only under the terms of
 * the GPL and not to allow others to use your version of this file under the
 * MPL, indicate your decision by deleting the provisions above and replacing
 * them with the notice and other provisions required by the GPL. If you do
 * not delete the provisions above, a recipient may use your version of this
 * file under either the MPL or the GPL.
 *
 * ***** END LICENSE BLOCK ***** */
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/rhino/Node.java, src/com/google/javascript/rhino/jstype/JSType.java.

//! Rust-only: `Node#toString` / `Node#toStringTree` / `Node#appendJsonTree` of nodes that carry
//! JSTypes. In Java a JSType prints itself (`JSType#toString`); a Rust `TypeId` is a handle into
//! the compiler's `JSTypeRegistry`, which rhino cannot reach, so these helpers hand rhino's
//! registry-aware printers the compiler's registry. Nodes without JSTypes print exactly as with
//! the plain rhino methods; colors print as before.

use crate::jscomp_api::Compiler;
use closure_jstype::prelude::JSType;
use closure_rhino::js_string::JsString;
use closure_rhino::jstype::TypeId;
use closure_rhino::node::{Ast, NodeId};

/// Runs `print` with Java's `JSType#toString` over `compiler`'s registry (or rhino's
/// registry-free printer when the compiler has no registry). `ast` is the arena of the printed
/// node; `None` means the compiler's own arena. The types themselves are always read in the
/// compiler's arena, the one their registry was built on.
fn with_type_printer<T>(
    compiler: &mut Compiler,
    ast: Option<&Ast>,
    print: impl FnOnce(&Ast, Option<&mut dyn FnMut(TypeId) -> JsString>) -> T,
) -> T {
    let (registry, compiler_ast) = compiler.get_type_registry_field_and_ast();
    let node_ast = ast.unwrap_or(compiler_ast);
    match registry {
        Some(registry) => {
            // port: JSType#toString
            let mut type_printer =
                |t: TypeId| JsString::from(JSType::to_string(t, registry, compiler_ast));
            print(node_ast, Some(&mut type_printer))
        }
        None => print(node_ast, None),
    }
}

// port: Node#toString(boolean, boolean, boolean)
/// `n.toString(printSource, printAnnotations, printType)` with JSTypes printed through
/// `compiler`'s registry.
pub fn to_string_with_options(
    compiler: &mut Compiler,
    ast: Option<&Ast>,
    n: NodeId,
    print_source: bool,
    print_annotations: bool,
    print_type: bool,
) -> String {
    with_type_printer(compiler, ast, |ast, type_printer| {
        let text = match type_printer {
            Some(type_printer) => n.to_string_with_options_and_types_utf16(
                ast,
                print_source,
                print_annotations,
                print_type,
                type_printer,
            ),
            None => {
                n.to_string_with_options_utf16(ast, print_source, print_annotations, print_type)
            }
        };
        closure_rhino::java_lang::charset::utf8_encoded_text(text.as_units())
    })
}

// port: Node#toString()
/// `n.toString()` with JSTypes printed through `compiler`'s registry.
pub fn to_string(compiler: &mut Compiler, ast: Option<&Ast>, n: NodeId) -> String {
    to_string_with_options(compiler, ast, n, true, true, true)
}

// port: Node#toStringTree
/// `n.toStringTree()` with JSTypes printed through `compiler`'s registry.
pub fn to_string_tree(compiler: &mut Compiler, ast: Option<&Ast>, n: NodeId) -> String {
    with_type_printer(compiler, ast, |ast, type_printer| match type_printer {
        Some(type_printer) => n.to_string_tree_with_types(ast, type_printer),
        None => n.to_string_tree(ast),
    })
}

// port: Node#appendJsonTree
/// `n.appendJsonTree(..)` with JSTypes printed through `compiler`'s registry.
pub fn to_json_tree(compiler: &mut Compiler, ast: Option<&Ast>, n: NodeId) -> String {
    with_type_printer(compiler, ast, |ast, type_printer| {
        let text = match type_printer {
            Some(type_printer) => n.to_json_tree_with_types_utf16(ast, type_printer),
            None => n.to_json_tree_utf16(ast),
        };
        closure_rhino::java_lang::charset::utf8_encoded_text(text.as_units())
    })
}
