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
//   src/com/google/javascript/rhino/jstype/TypeStringBuilder.java.

use crate::{JSTypeRegistry, TypeId, function_type::Parameter, js_type::JSType};
use closure_rhino::node::Ast;

pub enum TypeStringItem<'a> {
    Text(&'a str),
    Type(TypeId),
    Parameter(&'a Parameter),
}

pub struct TypeStringBuilder {
    builder: String,
    is_for_annotations: bool,
    indentation: String,
}
impl TypeStringBuilder {
    // port: TypeStringBuilder#TypeStringBuilder
    pub fn new(is_for_annotations: bool) -> Self {
        Self {
            builder: String::new(),
            is_for_annotations,
            indentation: String::new(),
        }
    }
    // port: TypeStringBuilder#isForAnnotations
    pub fn is_for_annotations(&self) -> bool {
        self.is_for_annotations
    }
    // port: TypeStringBuilder#append
    pub fn append(&mut self, x: &str) -> &mut Self {
        self.builder.push_str(x);
        self
    }
    // port: TypeStringBuilder#append
    pub fn append_type(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, x: TypeId) -> &mut Self {
        x.append_to(reg, ast, self);
        self
    }
    // port: TypeStringBuilder#appendAll
    pub fn append_all(
        &mut self,
        elements: impl IntoIterator<Item = String>,
        separator: &str,
    ) -> &mut Self {
        let mut separate = false;
        for e in elements {
            if separate {
                self.append(separator);
            } else {
                separate = true;
            }
            self.append(&e);
        }
        self
    }
    // port: TypeStringBuilder#appendAll
    pub fn append_all_types(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        elements: &[TypeId],
        separator: &str,
    ) -> &mut Self {
        let mut separate = false;
        for &e in elements {
            if separate {
                self.append(separator);
            } else {
                separate = true;
            }
            self.append_type(reg, ast, e);
        }
        self
    }
    // port: TypeStringBuilder#appendAll
    pub fn append_all_objects<'a>(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        elements: impl IntoIterator<Item = TypeStringItem<'a>>,
        separator: &str,
    ) -> &mut Self {
        let mut separate = false;
        for e in elements {
            if separate {
                self.append(separator);
            } else {
                separate = true;
            }
            match e {
                TypeStringItem::Type(type_) => {
                    self.append_type(reg, ast, type_);
                }
                TypeStringItem::Text(text) => {
                    self.append(text);
                }
                TypeStringItem::Parameter(_) => {
                    panic!(
                        "class com.google.javascript.rhino.jstype.AutoValue_FunctionType_Parameter cannot be cast to class java.lang.String (com.google.javascript.rhino.jstype.AutoValue_FunctionType_Parameter is in unnamed module of loader 'app'; java.lang.String is in module java.base of loader 'bootstrap')"
                    );
                }
            }
        }
        self
    }
    // port: TypeStringBuilder#appendNonNull
    pub fn append_non_null(&mut self, reg: &mut JSTypeRegistry, ast: &Ast, t: TypeId) -> &mut Self {
        if self.is_for_annotations
            && t.is_object(reg, ast)
            && !t.is_unknown_type(reg, ast)
            && !t.is_template_type(reg)
            && !t.is_record_type(reg)
            && !t.is_function_type(reg)
            && !t.is_union_type(reg)
            && !t.is_literal_object(reg)
        {
            self.append("!");
        }
        self.append_type(reg, ast, t)
    }
    // port: TypeStringBuilder#breakLineAndIndent
    pub fn break_line_and_indent(&mut self) -> &mut Self {
        self.builder.push('\n');
        self.builder.push_str(&self.indentation);
        self
    }
    // port: TypeStringBuilder#indent
    pub fn indent(&mut self, cb: impl FnOnce(&mut Self)) -> &mut Self {
        let last_indent = self.indentation.clone();
        self.indentation.push_str("  ");
        cb(self);
        self.indentation = last_indent;
        self
    }
    // port: TypeStringBuilder#build
    pub fn build(&self) -> String {
        self.builder.clone()
    }
    // port: TypeStringBuilder#cloneWithConfig
    pub fn clone_with_config(&self) -> Self {
        Self {
            builder: String::new(),
            is_for_annotations: self.is_for_annotations,
            indentation: self.indentation.clone(),
        }
    }
}
