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
 *   Nick Santos
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
//   test/com/google/javascript/rhino/jstype/JSTypeResolverTest.java.

use closure_jstype::{
    TypeId,
    js_type::{JSType, UnitTestingJSType, UnitTestingJSTypeData},
    js_type_class::JSTypeClass,
    js_type_registry::JSTypeRegistry,
    js_type_resolver::JSTypeResolver,
};
use closure_rhino::{error_reporter::NullErrorReporter, node::Ast};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};

const STUB_TYPE_CLASS: JSTypeClass = JSTypeClass::FUNCTION;
type Callback = Arc<dyn Fn(TypeId, &mut JSTypeRegistry, &Ast) + Send + Sync>;
struct CustomTypeBuilder {
    ctor: Callback,
    resolve: Callback,
}
impl CustomTypeBuilder {
    fn new() -> Self {
        Self {
            ctor: Arc::new(|_, _, _| {}),
            resolve: Arc::new(|_, _, _| {}),
        }
    }
    // port: JSTypeResolverTest.CustomTypeBuilder#setCtor
    fn set_ctor(
        mut self,
        ctor: impl Fn(TypeId, &mut JSTypeRegistry, &Ast) + Send + Sync + 'static,
    ) -> Self {
        self.ctor = Arc::new(ctor);
        self
    }
    // port: JSTypeResolverTest.CustomTypeBuilder#setResolve
    fn set_resolve(
        mut self,
        resolve: impl Fn(TypeId, &mut JSTypeRegistry, &Ast) + Send + Sync + 'static,
    ) -> Self {
        self.resolve = Arc::new(resolve);
        self
    }
    // port: JSTypeResolverTest.CustomTypeBuilder#build
    #[allow(clippy::arc_with_non_send_sync)]
    fn build(self, reg: &mut JSTypeRegistry, ast: &Ast) -> TypeId {
        let resolve = self.resolve;
        // port: JSTypeResolverTest.CustomJSType#getTypeClass
        let data = UnitTestingJSTypeData {
            is_no_resolved_type: None,
            type_class: Some(STUB_TYPE_CLASS),
            resolve: Some(Arc::new(move |t, reg, ast| {
                // port: JSTypeResolverTest.CustomJSType#resolveInternal
                resolve(t, reg, ast);
                t
            })),
            hash: None,
        };
        // port: JSTypeResolverTest.CustomJSType#CustomJSType
        let t = UnitTestingJSType::with_data(reg, data);
        (self.ctor)(t, reg, ast);
        t
    }
}
fn setup() -> (Ast, JSTypeRegistry) {
    let mut ast = Ast::new();
    let reg = JSTypeRegistry::new(&mut ast, Box::new(NullErrorReporter), Vec::new());
    (ast, reg)
}
fn finish(t: TypeId, reg: &mut JSTypeRegistry, ast: &Ast) {
    JSTypeResolver::resolve_if_closed(reg, ast, t, STUB_TYPE_CLASS);
}

// port: JSTypeResolverTest#capturesAllTypes_beforeOpening_isVerified
#[test]
fn captures_all_types_before_opening_is_verified() {
    let (ast, mut reg) = setup();
    CustomTypeBuilder::new()
        .set_ctor(|_, _, _| {})
        .build(&mut reg, &ast);
    assert!(
        catch_unwind(AssertUnwindSafe(|| reg
            .get_resolver()
            .open_for_definition()))
        .is_err()
    );
}
// port: JSTypeResolverTest#capturesAllTypes_beforeClosing_isVerified
#[test]
fn captures_all_types_before_closing_is_verified() {
    let (ast, mut reg) = setup();
    let mut closer = reg.get_resolver().open_for_definition();
    CustomTypeBuilder::new()
        .set_ctor(|_, _, _| {})
        .build(&mut reg, &ast);
    assert!(catch_unwind(AssertUnwindSafe(|| closer.close(&mut reg, &ast))).is_err());
}
// port: JSTypeResolverTest#capturesAllTypes_inPostorder_isVerified
#[test]
fn captures_all_types_in_postorder_is_verified() {
    let (ast, mut reg) = setup();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            CustomTypeBuilder::new()
                .set_ctor(|t1, reg, ast| {
                    CustomTypeBuilder::new()
                        .set_ctor(|_, _, _| {})
                        .build(reg, ast);
                    finish(t1, reg, ast);
                })
                .build(&mut reg, &ast);
        }))
        .is_err()
    );
}
// port: JSTypeResolverTest#capturesAllTypes_allowsNestedConstructorCalls
#[test]
fn captures_all_types_allows_nested_constructor_calls() {
    let (ast, mut reg) = setup();
    CustomTypeBuilder::new()
        .set_ctor(|t1, reg, ast| {
            CustomTypeBuilder::new().set_ctor(finish).build(reg, ast);
            finish(t1, reg, ast);
        })
        .build(&mut reg, &ast);
}
// port: JSTypeResolverTest#capturesAllTypes_onlyOnce_isVerified
#[test]
fn captures_all_types_only_once_is_verified() {
    let (ast, mut reg) = setup();
    let t = CustomTypeBuilder::new()
        .set_ctor(finish)
        .build(&mut reg, &ast);
    assert!(catch_unwind(AssertUnwindSafe(|| finish(t, &mut reg, &ast))).is_err());
}
// port: JSTypeResolverTest#capturesAllTypes_ignoresNonLowestSubclassConstructor
#[test]
fn captures_all_types_ignores_non_lowest_subclass_constructor() {
    let (ast, mut reg) = setup();
    let superclass = JSTypeClass::ALL;
    assert_ne!(superclass, STUB_TYPE_CLASS);
    CustomTypeBuilder::new()
        .set_ctor(move |t, reg, ast| {
            JSTypeResolver::resolve_if_closed(reg, ast, t, superclass);
            finish(t, reg, ast);
        })
        .build(&mut reg, &ast);
}
// port: JSTypeResolverTest#resolvesAllTypes_eagerly_whileClosed
#[test]
fn resolves_all_types_eagerly_while_closed() {
    let (ast, mut reg) = setup();
    let example = CustomTypeBuilder::new()
        .set_ctor(finish)
        .build(&mut reg, &ast);
    assert!(example.is_resolved(&reg));
}
// port: JSTypeResolverTest#resolvesAllTypes_eagerly_whileClosing
#[test]
fn resolves_all_types_eagerly_while_closing() {
    let (ast, mut reg) = setup();
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut closer = reg.get_resolver().open_for_definition();
    let ctor_events = events.clone();
    let resolve_events = events.clone();
    CustomTypeBuilder::new()
        .set_ctor(move |t1, reg, ast| {
            ctor_events.lock().unwrap().push("t1_ctor_start");
            finish(t1, reg, ast);
            ctor_events.lock().unwrap().push("t1_ctor_end");
        })
        .set_resolve(move |_, reg, ast| {
            resolve_events.lock().unwrap().push("t1_resolve_start");
            let ctor_events = resolve_events.clone();
            let child_events = resolve_events.clone();
            CustomTypeBuilder::new()
                .set_ctor(move |t2, reg, ast| {
                    ctor_events.lock().unwrap().push("t2_ctor_start");
                    finish(t2, reg, ast);
                    ctor_events.lock().unwrap().push("t2_ctor_end");
                })
                .set_resolve(move |_, _, _| {
                    child_events.lock().unwrap().push("t2_resolve");
                })
                .build(reg, ast);
            resolve_events.lock().unwrap().push("t1_resolve_end");
        })
        .build(&mut reg, &ast);
    closer.close(&mut reg, &ast);
    assert_eq!(
        *events.lock().unwrap(),
        vec![
            "t1_ctor_start",
            "t1_ctor_end",
            "t1_resolve_start",
            "t2_ctor_start",
            "t2_resolve",
            "t2_ctor_end",
            "t1_resolve_end"
        ]
    );
}
// port: JSTypeResolverTest#resolvesAllTypes_lazily_whileOpen
#[test]
fn resolves_all_types_lazily_while_open() {
    let (ast, mut reg) = setup();
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut closer = reg.get_resolver().open_for_definition();
    let ctor_events = events.clone();
    let resolve_events = events.clone();
    CustomTypeBuilder::new()
        .set_ctor(move |t1, reg, ast| {
            ctor_events.lock().unwrap().push("ctor_start");
            finish(t1, reg, ast);
            ctor_events.lock().unwrap().push("ctor_end");
        })
        .set_resolve(move |_, _, _| {
            resolve_events.lock().unwrap().push("resolve");
        })
        .build(&mut reg, &ast);
    events.lock().unwrap().push("close");
    closer.close(&mut reg, &ast);
    assert_eq!(
        *events.lock().unwrap(),
        vec!["ctor_start", "ctor_end", "close", "resolve"]
    );
}
// port: JSTypeResolverTest#cannotBeOpened_whileOpen
#[test]
fn cannot_be_opened_while_open() {
    let (_ast, mut reg) = setup();
    reg.get_resolver().open_for_definition();
    assert!(
        catch_unwind(AssertUnwindSafe(|| reg
            .get_resolver()
            .open_for_definition()))
        .is_err()
    );
}
// port: JSTypeResolverTest#cannotBeOpened_whileClosing
#[test]
fn cannot_be_opened_while_closing() {
    let (ast, mut reg) = setup();
    let mut closer = reg.get_resolver().open_for_definition();
    CustomTypeBuilder::new()
        .set_ctor(finish)
        .set_resolve(|_, reg, _| {
            reg.get_resolver().open_for_definition();
        })
        .build(&mut reg, &ast);
    assert!(catch_unwind(AssertUnwindSafe(|| closer.close(&mut reg, &ast))).is_err());
}
// port: JSTypeResolverTest#closer_cannotBeReused
#[test]
fn closer_cannot_be_reused() {
    let (ast, mut reg) = setup();
    let mut closer = reg.get_resolver().open_for_definition();
    closer.close(&mut reg, &ast);
    assert!(catch_unwind(AssertUnwindSafe(|| closer.close(&mut reg, &ast))).is_err());
}
// port: JSTypeResolverTest#types_cannotBeResolved_whileOpen
#[test]
fn types_cannot_be_resolved_while_open() {
    let (ast, mut reg) = setup();
    let mut closer = reg.get_resolver().open_for_definition();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let t = CustomTypeBuilder::new()
                .set_ctor(|t, reg, ast| {
                    let class = t.get_type_class(reg);
                    JSTypeResolver::resolve_if_closed(reg, ast, t, class);
                })
                .build(&mut reg, &ast);
            t.resolve(&mut reg, &ast);
        }))
        .is_err()
    );
    closer.close(&mut reg, &ast);
}
