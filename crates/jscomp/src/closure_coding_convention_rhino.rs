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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/rhino/Node.java.

//! `Node#matchesQualifiedName(Node)` for a node and a pattern that live in two different node
//! arenas.

use closure_rhino::node::{Ast, NodeId};

// port: Node#matchesQualifiedName(Node) (two immutable arenas)
pub(crate) fn matches_qualified_name_pattern(
    ast: &Ast,
    node: NodeId,
    pattern_ast: &Ast,
    pattern: NodeId,
) -> bool {
    if node.get_token(ast) != pattern.get_token(pattern_ast) {
        return false;
    }
    match node.get_token(ast) {
        closure_rhino::token::Token::NAME => {
            node.get_string(ast) == pattern.get_string(pattern_ast)
        }
        closure_rhino::token::Token::THIS | closure_rhino::token::Token::SUPER => true,
        closure_rhino::token::Token::GETPROP => {
            node.get_string(ast) == pattern.get_string(pattern_ast)
                && matches_qualified_name_pattern(
                    ast,
                    node.get_first_child(ast).unwrap(),
                    pattern_ast,
                    pattern.get_first_child(pattern_ast).unwrap(),
                )
        }
        _ => false,
    }
}
