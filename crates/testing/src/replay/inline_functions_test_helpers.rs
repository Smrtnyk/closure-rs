/*
 * Copyright 2005 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/InlineFunctions.java,
//   test/com/google/javascript/jscomp/InlineFunctionsTest.java.

//! Port of the unit-corpus helper `InlineFunctionsTest_Helpers.java` (oracle/replay/helpers): the
//! holder of InlineFunctionsTest's `uniqueIdSupplier` field and its `UniqueIdSupplier`, plus the
//! `new InlineFunctions(compiler, uniqueIdSupplier, ...)` the InlineFunctionsTest descriptor calls.
//!
//! Java hands the same `UniqueIdSupplier` object to every repetition's InlineFunctions, which
//! draws ids from it with `get()` (`"v" + nextId++`). The jscomp supplier is an
//! `Arc<dyn Fn() -> String + Send + Sync>`, while the DSL object is an `Rc<RefCell<Object>>`, so
//! the supplier counts in an `AtomicI32` seeded from the object's `nextId`, and the pass writes
//! the counter back into the object when `process` returns (or unwinds), the point where Java's
//! object holds the same value.
use crate::{
    replay::replay_dsl::{Ctx, DslValue, Object},
    throwable::Throwable,
};
use closure_jscomp::{
    abstract_compiler::AbstractCompiler, compiler_options::Reach, compiler_pass::CompilerPass,
    inline_functions::InlineFunctions,
};
use closure_rhino::fx_hash::IndexMap;
use closure_rhino::node::NodeId;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
};

const HOLDER: &str = "com.google.javascript.jscomp.InlineFunctionsTest_Helpers";
const UNIQUE_ID_SUPPLIER: &str =
    "com.google.javascript.jscomp.InlineFunctionsTest_Helpers$UniqueIdSupplier";

// port: InlineFunctionsTest_Helpers.UniqueIdSupplier#UniqueIdSupplier
fn unique_id_supplier() -> DslValue {
    let mut fields = IndexMap::<_, _>::default();
    let mut field_types = IndexMap::<_, _>::default();
    // private int nextId = 0;
    fields.insert("nextId".to_string(), DslValue::Int(0));
    field_types.insert("nextId".to_string(), "int".to_string());
    DslValue::Object(Rc::new(RefCell::new(Object {
        class: UNIQUE_ID_SUPPLIER.into(),
        fields,
        field_types,
    })))
}

// port: InlineFunctionsTest_Helpers#InlineFunctionsTest_Helpers
pub fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let mut fields = IndexMap::<_, _>::default();
    let mut field_types = IndexMap::<_, _>::default();
    // private UniqueIdSupplier uniqueIdSupplier = new UniqueIdSupplier();
    fields.insert("uniqueIdSupplier".to_string(), unique_id_supplier());
    field_types.insert(
        "uniqueIdSupplier".to_string(),
        UNIQUE_ID_SUPPLIER.to_string(),
    );
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: HOLDER.into(),
        fields,
        field_types,
    }))))
}

/// The `nextId` field of a `UniqueIdSupplier` object.
fn next_id(supplier: &Rc<RefCell<Object>>) -> Result<i32, Throwable> {
    match supplier.borrow().fields.get("nextId") {
        Some(DslValue::Int(next_id)) => Ok(*next_id),
        _ => Err(Throwable::HarnessError(
            "InlineFunctionsTest_Helpers$UniqueIdSupplier has no int nextId".into(),
        )),
    }
}

/// `InlineFunctions` drawing its safe-name ids from a DSL `UniqueIdSupplier` object.
struct InlineFunctionsWithSupplier {
    pass: InlineFunctions,
    supplier: Rc<RefCell<Object>>,
    next_id: Arc<AtomicI32>,
}

/// Writes the supplier's counter back into the DSL object, also when `process` unwinds.
struct WriteBack<'a> {
    supplier: &'a Rc<RefCell<Object>>,
    next_id: &'a AtomicI32,
}

impl Drop for WriteBack<'_> {
    // port: InlineFunctionsTest_Helpers.UniqueIdSupplier (nextId field state)
    fn drop(&mut self) {
        self.supplier.borrow_mut().fields.insert(
            "nextId".to_string(),
            DslValue::Int(self.next_id.load(Ordering::SeqCst)),
        );
    }
}

impl CompilerPass for InlineFunctionsWithSupplier {
    // port: InlineFunctions#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let _write_back = WriteBack {
            supplier: &self.supplier,
            next_id: &self.next_id,
        };
        self.pass.process(compiler, externs, root);
    }
}

// port: InlineFunctions#InlineFunctions (with InlineFunctionsTest_Helpers.UniqueIdSupplier)
pub fn inline_functions(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [
        DslValue::Compiler(compiler),
        DslValue::Object(supplier),
        DslValue::Enum { name: reach, .. },
        DslValue::Bool(assume_strict_this),
        DslValue::Bool(assume_minimum_capture),
        DslValue::Int(max_size_after_inlining),
    ] = args.as_slice()
    else {
        return Err(Throwable::HarnessError(
            "InlineFunctions#<init>(AbstractCompiler,Supplier,Reach,boolean,boolean,int) arguments"
                .into(),
        ));
    };
    if supplier.borrow().class != UNIQUE_ID_SUPPLIER {
        return Err(Throwable::Unported(supplier.borrow().class.clone()));
    }
    let reach = Reach::value_of(reach)
        .ok_or_else(|| Throwable::HarnessError(format!("unknown CompilerOptions.Reach {reach}")))?;
    let next_id = Arc::new(AtomicI32::new(next_id(supplier)?));
    let counter = next_id.clone();
    // port: InlineFunctionsTest_Helpers.UniqueIdSupplier#get
    let get = move || {
        // prefix with "v" to distinguish from Compiler's unique id supplier.
        format!("v{}", counter.fetch_add(1, Ordering::SeqCst))
    };
    let pass = InlineFunctions::new(
        &compiler.borrow(),
        Arc::new(get),
        reach,
        *assume_strict_this,
        *assume_minimum_capture,
        *max_size_after_inlining,
    );
    let pass: Box<dyn CompilerPass> = Box::new(InlineFunctionsWithSupplier {
        pass,
        supplier: supplier.clone(),
        next_id,
    });
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}
