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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   test/com/google/javascript/jscomp/MakeDeclaredNamesUniqueTest.java.

//! Port of the unit-corpus helper `MakeDeclaredNamesUniqueTest_Helpers.java`
//! (oracle/replay/helpers): the holder of MakeDeclaredNamesUniqueTest's fields and the
//! `Processor` pass copied from MakeDeclaredNamesUniqueTest#getProcessor's anonymous class.
use crate::{
    replay::replay_dsl::{Ctx, DslValue, Object},
    throwable::Throwable,
};
use closure_jscomp::{
    abstract_compiler::AbstractCompiler,
    closure_coding_convention::ClosureCodingConvention,
    coding_convention::CodingConvention,
    compiler_pass::CompilerPass,
    make_declared_names_unique::{InlineRenamer, MakeDeclaredNamesUnique},
    node_traversal::NodeTraversal,
};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::node::NodeId;
use std::{cell::RefCell, rc::Rc, sync::Arc};

const HOLDER: &str = "com.google.javascript.jscomp.MakeDeclaredNamesUniqueTest_Helpers";
const LOCAL_NAME_PREFIX: &str = "unique_";

// port: MakeDeclaredNamesUniqueTest_Helpers#MakeDeclaredNamesUniqueTest_Helpers
pub fn holder(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let mut fields = IndexMap::<_, _>::default();
    let mut field_types = IndexMap::<_, _>::default();
    for name in [
        "useDefaultRenamer",
        "invert",
        "removeConst",
        "assertOnChange",
    ] {
        fields.insert(name.to_string(), DslValue::Bool(false));
        field_types.insert(name.to_string(), "boolean".to_string());
    }
    Ok(DslValue::Object(Rc::new(RefCell::new(Object {
        class: HOLDER.into(),
        fields,
        field_types,
    }))))
}

/// MakeDeclaredNamesUniqueTest_Helpers.Processor: reads the outer holder's fields when it runs,
/// as the anonymous class reads the test's fields.
struct Processor {
    outer: Rc<RefCell<Object>>,
}

// port: MakeDeclaredNamesUniqueTest_Helpers.Processor#Processor
pub fn processor(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let [DslValue::Object(outer), DslValue::Compiler(_)] = args.as_slice() else {
        return Err(Throwable::HarnessError(
            "MakeDeclaredNamesUniqueTest_Helpers$Processor arguments".into(),
        ));
    };
    let pass: Box<dyn CompilerPass> = Box::new(Processor {
        outer: outer.clone(),
    });
    Ok(DslValue::Pass(Rc::new(RefCell::new(pass))))
}

impl Processor {
    // port: MakeDeclaredNamesUniqueTest_Helpers (boolean field read)
    fn field(&self, name: &str) -> bool {
        matches!(
            self.outer.borrow().fields.get(name),
            Some(DslValue::Bool(true))
        )
    }
}

impl CompilerPass for Processor {
    // port: MakeDeclaredNamesUniqueTest_Helpers.Processor#process
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        let mut renamer =
            MakeDeclaredNamesUnique::builder().with_assert_on_change(self.field("assertOnChange"));
        if !self.field("useDefaultRenamer") {
            // Compiler#getCodingConvention as a shared handle: the options' convention, else the
            // compiler's default ClosureCodingConvention.
            let convention: Arc<dyn CodingConvention> =
                match compiler.get_options().get_coding_convention().clone() {
                    Some(convention) => convention,
                    None => Arc::new(ClosureCodingConvention::new()),
                };
            renamer = renamer.with_renamer(InlineRenamer::new(
                convention,
                compiler.get_unique_name_id_supplier(),
                LOCAL_NAME_PREFIX,
                self.field("removeConst"),
                true,
                None,
            ));
        }
        let mut pass = renamer.build();
        NodeTraversal::traverse_roots(compiler, &mut pass, externs, root);
    }
}
