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
//   src/com/google/javascript/rhino/jstype/JSTypeResolver.java.

use crate::{
    TypeId, js_type::JSType, js_type_class::JSTypeClass, js_type_native::JSTypeNative,
    js_type_registry::JSTypeRegistry, prototype_object_type::PrototypeObjectType,
};
use closure_rhino::{check_state, node::Ast};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    CLOSED,
    OPEN,
    CLOSING,
}

pub struct JSTypeResolver {
    capture_stack: VecDeque<TypeId>,
    resolution_queue: VecDeque<TypeId>,
    state: State,
}
pub struct Closer {
    has_run: bool,
}
impl Closer {
    // port: JSTypeResolver.Closer#close
    pub fn close(&mut self, reg: &mut JSTypeRegistry, ast: &Ast) {
        check_state!(!self.has_run);
        self.has_run = true;
        JSTypeResolver::resolve_all(reg, ast);
    }
}
impl JSTypeResolver {
    // port: JSTypeResolver#create
    pub fn create(reg: &mut JSTypeRegistry) -> &mut Self {
        &mut reg.resolver
    }
    // port: JSTypeResolver#JSTypeResolver
    pub(crate) fn new() -> Self {
        Self {
            capture_stack: VecDeque::new(),
            resolution_queue: VecDeque::new(),
            state: State::CLOSED,
        }
    }
    // port: JSTypeResolver#addUnresolved
    pub(crate) fn add_unresolved(&mut self, captured: TypeId) {
        self.capture_stack.push_back(captured);
    }
    // port: JSTypeResolver#resolveIfClosed
    pub fn resolve_if_closed(
        reg: &mut JSTypeRegistry,
        ast: &Ast,
        captured: TypeId,
        caller: JSTypeClass,
    ) {
        if captured.get_type_class(reg) != caller {
            return;
        }
        let expected = reg
            .resolver
            .capture_stack
            .pop_back()
            .expect("NoSuchElementException");
        if captured != expected {
            let captured_name = captured.to_string(reg, ast);
            let expected_name = expected.to_string(reg, ast);
            check_state!(
                false,
                "Captured %s; Expected %s",
                captured_name,
                expected_name
            );
        }
        match reg.resolver.state {
            State::CLOSED | State::CLOSING => Self::do_resolve(reg, ast, captured),
            State::OPEN => reg.resolver.resolution_queue.push_back(captured),
        }
    }
    // port: JSTypeResolver#openForDefinition
    pub fn open_for_definition(&mut self) -> Closer {
        check_state!(self.state == State::CLOSED);
        check_state!(self.capture_stack.is_empty());
        self.state = State::OPEN;
        Closer { has_run: false }
    }
    // port: JSTypeResolver#resolveAll
    fn resolve_all(reg: &mut JSTypeRegistry, ast: &Ast) {
        check_state!(reg.resolver.state == State::OPEN);
        check_state!(reg.resolver.capture_stack.is_empty());
        reg.resolver.state = State::CLOSING;
        while let Some(type_) = reg.resolver.resolution_queue.pop_front() {
            Self::do_resolve(reg, ast, type_);
        }
        reg.resolver.resolution_queue = VecDeque::new();
        reg.resolver.state = State::CLOSED;
        let global_this = reg.get_native_type(JSTypeNative::GLOBAL_THIS);
        let window_type = reg.get_global_type(ast, "Window");
        if global_this.is_unknown_type(reg, ast) {
            let window_obj_type = window_type.and_then(|t| t.to_maybe_object_type(reg));
            let proto = window_obj_type
                .unwrap_or_else(|| reg.get_native_object_type(JSTypeNative::OBJECT_TYPE));
            global_this.set_implicit_prototype(reg, Some(proto));
        }
    }
    // port: JSTypeResolver#doResolve
    fn do_resolve(reg: &mut JSTypeRegistry, ast: &Ast, type_: TypeId) {
        type_.resolve(reg, ast);
    }
    // port: JSTypeResolver#assertLegalToResolveTypes
    pub fn assert_legal_to_resolve_types(&self) {
        check_state!(
            self.state != State::OPEN,
            "Types cannot be resolved while the registry is open"
        );
    }
}
