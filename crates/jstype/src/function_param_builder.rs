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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/rhino/jstype/FunctionParamBuilder.java.

use crate::{TypeId, function_type::Parameter, js_type_registry::JSTypeRegistry};
use closure_rhino::node::Ast;

#[derive(Clone, Debug, Default)]
pub struct FunctionParamBuilder {
    parameters: Vec<Parameter>,
}
impl FunctionParamBuilder {
    // port: FunctionParamBuilder#FunctionParamBuilder
    pub fn new() -> Self {
        Self {
            parameters: Vec::new(),
        }
    }
    // port: FunctionParamBuilder#FunctionParamBuilder
    pub fn with_capacity(initial_parameter_capacity: usize) -> Self {
        Self {
            parameters: Vec::with_capacity(initial_parameter_capacity),
        }
    }
    // port: FunctionParamBuilder#addRequiredParams
    pub fn add_required_params(&mut self, types: &[TypeId]) -> bool {
        if self.has_optional_or_var_args() {
            return false;
        }
        for &type_ in types {
            self.new_parameter(type_, false, false);
        }
        true
    }
    // port: FunctionParamBuilder#addOptionalParams
    pub fn add_optional_params(
        &mut self,
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        types: &[TypeId],
    ) -> bool {
        if self.has_var_args() {
            return false;
        }
        for &type_ in types {
            self.new_parameter(reg.create_optional_type(ast, type_), true, false);
        }
        true
    }
    // port: FunctionParamBuilder#addVarArgs
    pub fn add_var_args(&mut self, type_: TypeId) -> bool {
        if self.has_var_args() {
            return false;
        }
        self.new_parameter(type_, false, true);
        true
    }
    // port: FunctionParamBuilder#newParameterFrom
    pub fn new_parameter_from(&mut self, parameter: Parameter) {
        self.parameters.push(parameter);
    }
    // port: FunctionParamBuilder#newOptionalParameterFrom
    pub fn new_optional_parameter_from(&mut self, p: Parameter) {
        let is_optional = p.is_optional() || !p.is_variadic();
        if is_optional != p.is_optional() {
            self.new_parameter(p.get_jstype(), is_optional, p.is_variadic());
        } else {
            self.new_parameter_from(p);
        }
    }
    // port: FunctionParamBuilder#newParameter
    fn new_parameter(&mut self, type_: TypeId, is_optional: bool, is_variadic: bool) {
        self.parameters
            .push(Parameter::create(type_, is_optional, is_variadic));
    }
    // port: FunctionParamBuilder#hasOptionalOrVarArgs
    fn has_optional_or_var_args(&self) -> bool {
        self.parameters
            .last()
            .is_some_and(|p| p.is_optional() || p.is_variadic())
    }
    // port: FunctionParamBuilder#hasVarArgs
    pub fn has_var_args(&self) -> bool {
        self.parameters.last().is_some_and(Parameter::is_variadic)
    }
    // port: FunctionParamBuilder#build
    pub fn build(&self) -> Vec<Parameter> {
        self.parameters.clone()
    }
}
