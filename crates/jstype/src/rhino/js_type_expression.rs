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
//   src/com/google/javascript/rhino/JSTypeExpression.java.

use crate::{TypeId, js_type_registry::JSTypeRegistry, static_typed_scope::StaticTypedScope};
use closure_rhino::{js_type_expression::JSTypeExpression, node::Ast};
use std::sync::Arc;

pub trait JSTypeExpressionExt {
    fn evaluate(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &mut Ast,
        scope: Option<Arc<dyn StaticTypedScope>>,
    ) -> TypeId;
}
impl JSTypeExpressionExt for JSTypeExpression {
    // port: JSTypeExpression#evaluate
    fn evaluate(
        &self,
        reg: &mut JSTypeRegistry,
        ast: &mut Ast,
        scope: Option<Arc<dyn StaticTypedScope>>,
    ) -> TypeId {
        let type_ = reg.create_type_from_comment_node_with_location(
            ast,
            self.get_root(),
            self.get_source_name(),
            scope,
        );
        self.get_root().set_jstype(ast, Some(type_));
        type_
    }
}
