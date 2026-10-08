/*
 * Copyright 2004 The Closure Compiler Authors.
 * Copyright 2005 The Closure Compiler Authors.
 * Copyright 2006 The Closure Compiler Authors.
 * Copyright 2007 The Closure Compiler Authors.
 * Copyright 2008 The Closure Compiler Authors.
 * Copyright 2009 The Closure Compiler Authors.
 * Copyright 2015 The Closure Compiler Authors.
 * Copyright 2023 The Closure Compiler Authors.
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
/*
 * Copyright 2026 The closure-rs Authors.
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
//   src/com/google/javascript/jscomp/CollapseAnonymousFunctions.java,
//   src/com/google/javascript/jscomp/CollapseVariableDeclarations.java,
//   src/com/google/javascript/jscomp/ConvertToDottedProperties.java,
//   src/com/google/javascript/jscomp/Denormalize.java,
//   src/com/google/javascript/jscomp/ExtractPrototypeMemberDeclarations.java,
//   src/com/google/javascript/jscomp/GatherRawExports.java,
//   src/com/google/javascript/jscomp/Normalize.java,
//   src/com/google/javascript/jscomp/RemovePropertyRenamingCalls.java,
//   src/com/google/javascript/jscomp/RenameProperties.java,
//   src/com/google/javascript/jscomp/RenameVars.java,
//   src/com/google/javascript/jscomp/SubstituteEs6Syntax.java,
//   src/com/google/javascript/jscomp/VariableMap.java,
//   test/com/google/javascript/jscomp/RenamePropertiesTest.java,
//   test/com/google/javascript/jscomp/RenameVarsTest.java.
// Ported from closure-rs' own Java oracle tooling:
//   UnitRecorder.java (oracle/patches/0002-recording-hooks.patch),
//   oracle/replay/src/com/google/javascript/jscomp/ReplayDsl.java,
//   oracle/replay/src/com/google/javascript/jscomp/ReplayValues.java.

//! Replay adapters for the finalization and renaming passes: constructors
//! resolved through replay_signatures.tsv, plus the result getters UnitRecorder reads
//! (`getVariableMap`, `getPropertyMap`, `getExportedVariableNames`).
use crate::{
    replay::{
        options_values::OptionValue,
        registry::Entry,
        replay_dsl::{CompilerHandle, Ctx, DslValue, NativeObject},
    },
    throwable::Throwable,
};
use closure_jscomp::{
    collapse_anonymous_functions::CollapseAnonymousFunctions,
    collapse_variable_declarations::CollapseVariableDeclarations,
    convert_to_dotted_properties::ConvertToDottedProperties,
    extract_prototype_member_declarations::{ExtractPrototypeMemberDeclarations, Pattern},
    gather_raw_exports::GatherRawExports,
    name_generator::NameGenerator,
    remove_property_renaming_calls::RemovePropertyRenamingCalls,
    rename_properties::{PropertyRenameEligibilityFilter, RenameProperties},
    rename_vars::RenameVars,
    substitute_es6_syntax::SubstituteEs6Syntax,
    variable_map::VariableMap,
};
use closure_rhino::{js_string::JsString, node::NodeId};
use indexmap::{IndexMap, IndexSet};
use std::{cell::RefCell, rc::Rc, sync::Arc};

// port: ReplayDsl#invoke (resolved signatures backed by native implementations)
pub fn entry(signature: &str) -> Option<Entry> {
    Some(match signature {
        "com.google.javascript.jscomp.CollapseAnonymousFunctions#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            collapse_anonymous_functions
        }
        "com.google.javascript.jscomp.CollapseVariableDeclarations#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            collapse_variable_declarations
        }
        "com.google.javascript.jscomp.ConvertToDottedProperties#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            convert_to_dotted_properties
        }
        "com.google.javascript.jscomp.Denormalize#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.parsing.parser.FeatureSet)" => {
            denormalize
        }
        "com.google.javascript.jscomp.Normalize$NormalizeStatements#<init>(com.google.javascript.jscomp.AbstractCompiler,boolean,com.google.javascript.jscomp.MakeDeclaredNamesUnique)" => {
            normalize_statements
        }
        "com.google.javascript.jscomp.ExtractPrototypeMemberDeclarations#<init>(com.google.javascript.jscomp.AbstractCompiler,com.google.javascript.jscomp.ExtractPrototypeMemberDeclarations$Pattern)" => {
            extract_prototype_member_declarations
        }
        "com.google.javascript.jscomp.GatherRawExports#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            gather_raw_exports
        }
        "com.google.javascript.jscomp.RemovePropertyRenamingCalls#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            remove_property_renaming_calls
        }
        "com.google.javascript.jscomp.RenameVars#<init>(com.google.javascript.jscomp.AbstractCompiler,java.lang.String,boolean,boolean,boolean,com.google.javascript.jscomp.VariableMap,java.util.Set,java.util.Set,com.google.javascript.jscomp.NameGenerator)" => {
            rename_vars
        }
        "com.google.javascript.jscomp.RenameProperties#<init>(com.google.javascript.jscomp.AbstractCompiler,boolean,com.google.javascript.jscomp.VariableMap,java.util.Set,java.util.Set,com.google.javascript.jscomp.NameGenerator,java.util.function.Predicate)" => {
            rename_properties
        }
        "com.google.javascript.jscomp.SubstituteEs6Syntax#<init>(com.google.javascript.jscomp.AbstractCompiler)" => {
            substitute_es6_syntax
        }
        "com.google.javascript.jscomp.RenameVarsTestHelpers$NormalizePassWrapper#<init>(com.google.javascript.jscomp.Compiler,com.google.javascript.jscomp.CompilerPass)" => {
            normalize_pass_wrapper
        }
        "com.google.javascript.jscomp.RenameVarsTestHelpers$ClosurePassAndRenameVars#<init>(com.google.javascript.jscomp.RenameVarsTestHelpers,com.google.javascript.jscomp.Compiler)" => {
            closure_pass_and_rename_vars
        }
        "com.google.javascript.jscomp.RenamePropertiesTest_Helpers$DefaultFilter#<init>()" => {
            default_filter
        }
        "com.google.javascript.jscomp.RenamePropertiesTest_Helpers$GetPrefixFilter#<init>()" => {
            get_prefix_filter
        }
        "com.google.javascript.jscomp.RenamePropertiesTest_Helpers$FilterOnFilename#<init>()" => {
            filter_on_filename
        }
        "com.google.javascript.jscomp.RenamePropertiesTest_Helpers$RenameFunctionsFilterOnFilename#<init>()" => {
            filter_on_filename
        }
        "com.google.javascript.jscomp.RenamePropertiesTest_Helpers$RenameFunctionsFilterOnFilename2#<init>()" => {
            filter_on_filename
        }
        _ => return None,
    })
}

// port: ReplayValues.Undecodable#Undecodable
fn bad() -> Throwable {
    Throwable::HarnessError(
        "native method arguments do not match the resolved Java signature".into(),
    )
}
// port: ReplayDsl#invoke (receiver cast)
fn compiler(args: &[DslValue]) -> Result<CompilerHandle, Throwable> {
    if let Some(DslValue::Compiler(c)) = args.first() {
        Ok(c.clone())
    } else {
        Err(bad())
    }
}
// port: ReplayDsl#invoke (native CompilerPass result)
fn pass(pass: impl closure_jscomp::compiler_pass::CompilerPass + 'static) -> DslValue {
    DslValue::Pass(Rc::new(RefCell::new(Box::new(pass))))
}
// port: ReplayValues#decode (VariableMap raw fields)
fn decode_variable_map(value: &DslValue) -> Result<Option<Arc<VariableMap>>, Throwable> {
    match value.untyped() {
        DslValue::Null => Ok(None),
        DslValue::Native(o) => {
            let mut o = o.borrow_mut();
            let map = o
                .as_any_mut()
                .downcast_mut::<NativeVariableMap>()
                .ok_or_else(bad)?;
            Ok(Some(map.0.clone()))
        }
        DslValue::Object(o) if o.borrow().class == "com.google.javascript.jscomp.VariableMap" => {
            let map = o.borrow().fields.get("map").cloned().ok_or_else(bad)?;
            let map = IndexMap::<JsString, JsString>::decode_value(&map)?;
            Ok(Some(Arc::new(VariableMap::new(&map))))
        }
        _ => Err(bad()),
    }
}
// port: ReplayValues#decode (Set<Character>)
fn decode_chars(value: &DslValue) -> Result<IndexSet<u16>, Throwable> {
    IndexSet::<u16>::decode_value(value)
}
// port: ReplayValues#decode (NameGenerator): the pass resets the generator it receives; a clone
// keeps the decoded character priorities (DefaultNameGenerator#clone).
fn decode_name_generator(value: &DslValue) -> Result<Box<dyn NameGenerator>, Throwable> {
    let generator = Arc::<dyn NameGenerator + Send + Sync>::decode_value(value)?;
    Ok(NameGenerator::clone(
        &*generator,
        Arc::new(std::sync::RwLock::new(IndexSet::new())),
        JsString::from(""),
        &IndexSet::new(),
    ))
}

/// A `com.google.javascript.jscomp.VariableMap` result.
struct NativeVariableMap(Arc<VariableMap>);
impl NativeObject for NativeVariableMap {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.VariableMap"
    }
    // port: VariableMap#getOriginalNameToNewNameMap
    fn call(&mut self, method: &str, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        if method == "getOriginalNameToNewNameMap" {
            Ok(DslValue::Typed {
                class: "com.google.common.collect.ImmutableSortedMap".into(),
                value: Box::new(self.0.get_original_name_to_new_name_map().encode_value()?),
            })
        } else {
            Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            )))
        }
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
pub(crate) fn variable_map_value(map: VariableMap) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(NativeVariableMap(Arc::new(map)))))
}

// port: CollapseAnonymousFunctions#CollapseAnonymousFunctions
fn collapse_anonymous_functions(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let p = CollapseAnonymousFunctions::new(&c.borrow());
    Ok(pass(p))
}
/// Denormalize as a native object, so DSL calls of `process` resolve on its runtime class.
struct NativeDenormalize(closure_jscomp::denormalize::Denormalize);
impl NativeObject for NativeDenormalize {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.Denormalize"
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class_name()
            || class == "com.google.javascript.jscomp.CompilerPass"
            || class == "com.google.javascript.jscomp.NodeTraversal$Callback"
            || class == "com.google.javascript.jscomp.ReferenceCollector$Behavior"
    }
    // port: UnitRecorder#fields (Denormalize: the field the harness reads)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::from([(
            "outputFeatureSet".into(),
            self.0.get_output_feature_set().encode_value()?,
        )]))
    }
    // port: Denormalize#process
    fn process(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        crate::replay::native_unported::capture(|| {
            self.0.process(compiler, externs, root);
        })
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: Denormalize#Denormalize
fn denormalize(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let [_, output_feature_set] = args.as_slice() else {
        return Err(bad());
    };
    let output_feature_set =
        closure_parsing::parser::feature_set::FeatureSet::decode_value(output_feature_set)?;
    let p = closure_jscomp::denormalize::Denormalize::new(&c.borrow(), output_feature_set);
    Ok(DslValue::Native(Rc::new(RefCell::new(NativeDenormalize(
        p,
    )))))
}
// port: Denormalize#process (DSL call inside a CompilerPass lambda: the compiler is borrowed)
pub fn denormalize_process(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
    compiler: &mut crate::jscomp_api::Compiler,
) -> Result<DslValue, Throwable> {
    let [
        DslValue::Native(receiver),
        DslValue::Node(externs),
        DslValue::Node(root),
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    receiver.borrow_mut().process(compiler, *externs, *root)?;
    Ok(DslValue::Null)
}
/// Normalize.NormalizeStatements as a NodeTraversal callback (DenormalizeTest's
/// NormalizeAndDenormalizePass traverses with it).
struct NativeNormalizeStatements(closure_jscomp::normalize::NormalizeStatements, bool);
impl NativeObject for NativeNormalizeStatements {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.Normalize$NormalizeStatements"
    }
    // port: UnitRecorder#fields (NormalizeStatements: the constructor arguments)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::from([
            ("assertOnChange".into(), self.1.encode_value()?),
            ("makeDeclaredNamesUnique".into(), DslValue::Null),
        ]))
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class_name() || class == "com.google.javascript.jscomp.NodeTraversal$Callback"
    }
    fn as_traversal_callback(
        &mut self,
    ) -> Option<&mut dyn closure_jscomp::node_traversal::Callback> {
        Some(&mut self.0)
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: Normalize.NormalizeStatements#NormalizeStatements (only the null
// MakeDeclaredNamesUnique argument that DenormalizeTest passes is decodable here)
fn normalize_statements(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let [_, assert_on_change, DslValue::Null] = args.as_slice() else {
        return Err(bad());
    };
    let assert_on_change = bool::decode_value(assert_on_change)?;
    let p = closure_jscomp::normalize::NormalizeStatements::new(
        &mut c.borrow_mut(),
        assert_on_change,
        None,
    );
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeNormalizeStatements(p, assert_on_change),
    ))))
}
// port: CollapseVariableDeclarations#CollapseVariableDeclarations
fn collapse_variable_declarations(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let p = CollapseVariableDeclarations::new(&c.borrow());
    Ok(pass(p))
}
// port: ConvertToDottedProperties#ConvertToDottedProperties
fn convert_to_dotted_properties(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(pass(ConvertToDottedProperties::new()))
}
// port: ExtractPrototypeMemberDeclarations#ExtractPrototypeMemberDeclarations
fn extract_prototype_member_declarations(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    let pattern = match args.get(1).map(DslValue::untyped) {
        Some(DslValue::Enum { name, .. }) => match name.as_str() {
            "USE_GLOBAL_TEMP" => Pattern::USE_GLOBAL_TEMP,
            "USE_CHUNK_TEMP" => Pattern::USE_CHUNK_TEMP,
            "USE_IIFE" => Pattern::USE_IIFE,
            _ => return Err(bad()),
        },
        _ => return Err(bad()),
    };
    Ok(pass(ExtractPrototypeMemberDeclarations::new(pattern)))
}
// port: RemovePropertyRenamingCalls#RemovePropertyRenamingCalls
fn remove_property_renaming_calls(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(pass(RemovePropertyRenamingCalls::new()))
}
// port: SubstituteEs6Syntax#SubstituteEs6Syntax
fn substitute_es6_syntax(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(pass(SubstituteEs6Syntax::new()))
}

/// GatherRawExports with its `getExportedVariableNames` result getter.
struct NativeGatherRawExports(GatherRawExports);
impl NativeObject for NativeGatherRawExports {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.GatherRawExports"
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class_name()
            || class == "com.google.javascript.jscomp.CompilerPass"
            || class == "com.google.javascript.jscomp.NodeTraversal$Callback"
    }
    // port: GatherRawExports#getExportedVariableNames
    fn call(&mut self, method: &str, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        if method == "getExportedVariableNames" {
            Ok(DslValue::Typed {
                class: "java.util.LinkedHashSet".into(),
                value: Box::new(self.0.get_exported_variable_names().encode_value()?),
            })
        } else {
            Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            )))
        }
    }
    // port: UnitRecorder#fields (GatherRawExports: the field the harness walks)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::from([(
            "exportedVariables".into(),
            DslValue::Typed {
                class: "java.util.LinkedHashSet".into(),
                value: Box::new(self.0.get_exported_variable_names().encode_value()?),
            },
        )]))
    }
    // port: GatherRawExports#process
    fn process(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        crate::replay::native_unported::capture(|| {
            self.0.process(compiler, externs, root);
        })
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: GatherRawExports#GatherRawExports
fn gather_raw_exports(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeGatherRawExports(GatherRawExports::new()),
    ))))
}

/// RenameVars with its `getVariableMap` result getter.
struct NativeRenameVars(RenameVars);
impl NativeObject for NativeRenameVars {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.RenameVars"
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class_name() || class == "com.google.javascript.jscomp.CompilerPass"
    }
    // port: RenameVars#getVariableMap
    fn call(&mut self, method: &str, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        if method == "getVariableMap" {
            Ok(variable_map_value(self.0.get_variable_map()))
        } else {
            Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            )))
        }
    }
    // port: UnitRecorder#fields (RenameVars: the fields the harness reads)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let f = self.0.replay_fields();
        Ok(IndexMap::from([
            (
                "nameGenerator".into(),
                crate::replay::options_values::encode_name_generator(f.name_generator)?,
            ),
            ("prefix".into(), f.prefix.encode_value()?),
            ("assignmentCount".into(), f.assignment_count.encode_value()?),
            (
                "localRenamingOnly".into(),
                f.local_renaming_only.encode_value()?,
            ),
            (
                "preferStableNames".into(),
                f.prefer_stable_names.encode_value()?,
            ),
        ]))
    }
    // port: RenameVars#process
    fn process(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        crate::replay::native_unported::capture(|| {
            self.0.process(compiler, externs, root);
        })
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: RenameVars#RenameVars
fn rename_vars(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    let [
        _,
        prefix,
        local_renaming_only,
        generate_pseudo_names,
        prefer_stable_names,
        prev_used_rename_map,
        reserved_characters,
        reserved_names,
        name_generator,
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    let rename_vars = RenameVars::new(
        Option::<JsString>::decode_value(prefix)?,
        bool::decode_value(local_renaming_only)?,
        bool::decode_value(generate_pseudo_names)?,
        bool::decode_value(prefer_stable_names)?,
        decode_variable_map(prev_used_rename_map)?,
        decode_chars(reserved_characters)?,
        Option::<IndexSet<JsString>>::decode_value(reserved_names)?,
        decode_name_generator(name_generator)?,
    );
    Ok(DslValue::Native(Rc::new(RefCell::new(NativeRenameVars(
        rename_vars,
    )))))
}

/// RenameVarsTestHelpers.NormalizePassWrapper (corpus helper, copied verbatim from
/// RenameVarsTest.NormalizePassWrapper): normalizes for optimizations, then runs the wrapped pass.
struct NativeNormalizePassWrapper {
    wrapped_pass: DslValue,
}
impl NativeNormalizePassWrapper {
    // port: RenameVarsTest.NormalizePassWrapper#process (Normalize.createNormalizeForOptimizations)
    fn normalize(
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        use closure_jscomp::compiler_pass::CompilerPass;
        crate::replay::native_unported::capture(|| {
            let mut normalize =
                closure_jscomp::normalize::Normalize::create_normalize_for_optimizations(compiler);
            normalize.process(compiler, externs, root);
        })
    }
}
impl NativeObject for NativeNormalizePassWrapper {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.RenameVarsTestHelpers$NormalizePassWrapper"
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class_name() || class == "com.google.javascript.jscomp.CompilerPass"
    }
    // port: UnitRecorder#fields (NormalizePassWrapper: its wrappedPass; its compiler field is the
    // replay compiler the pass runs on)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::from([(
            "wrappedPass".into(),
            self.wrapped_pass.clone(),
        )]))
    }
    // port: RenameVarsTest.NormalizePassWrapper#process
    fn process(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        Self::normalize(compiler, externs, root)?;
        match &self.wrapped_pass {
            DslValue::Native(p) => p.borrow_mut().process(compiler, externs, root),
            DslValue::Pass(p) => {
                p.borrow_mut().process(compiler, externs, root);
                Ok(())
            }
            _ => Err(bad()),
        }
    }
    // port: RenameVarsTest.NormalizePassWrapper#process (the wrapped pass keeps the DSL context)
    fn process_with_ctx(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
        ctx: &mut Ctx,
    ) -> Result<(), Throwable> {
        Self::normalize(compiler, externs, root)?;
        crate::replay::replay_dsl::process_in_compiler(
            &self.wrapped_pass,
            compiler,
            externs,
            root,
            ctx,
        )
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: RenameVarsTest.NormalizePassWrapper#NormalizePassWrapper
fn normalize_pass_wrapper(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    compiler(&args)?;
    let [_, wrapped_pass] = args.as_slice() else {
        return Err(bad());
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeNormalizePassWrapper {
            wrapped_pass: wrapped_pass.clone(),
        },
    ))))
}

const RENAME_VARS_TEST_HELPERS: &str = "com.google.javascript.jscomp.RenameVarsTestHelpers";

// port: RenameVarsTestHelpers#RenameVarsTestHelpers (the outer holder of ClosurePassAndRenameVars:
// the test fields it reads, prefix and previouslyUsedMap, and the renameVars field it assigns)
pub fn rename_vars_test_helpers(
    _ctx: &mut Ctx,
    _args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let mut fields = IndexMap::new();
    let mut field_types = IndexMap::new();
    for (name, ty) in [
        ("prefix", "java.lang.String"),
        (
            "previouslyUsedMap",
            "com.google.javascript.jscomp.VariableMap",
        ),
        ("renameVars", "com.google.javascript.jscomp.RenameVars"),
    ] {
        fields.insert(name.to_string(), DslValue::Null);
        field_types.insert(name.to_string(), ty.to_string());
    }
    Ok(DslValue::Object(Rc::new(RefCell::new(
        crate::replay::replay_dsl::Object {
            class: RENAME_VARS_TEST_HELPERS.into(),
            fields,
            field_types,
        },
    ))))
}

/// RenameVarsTestHelpers.ClosurePassAndRenameVars (corpus helper, copied verbatim from
/// RenameVarsTest.ClosurePassAndRenameVars): gathers module metadata, runs the closure pass, then
/// renames with the closure pass's exported names reserved. It reads the outer holder's fields
/// when it runs and assigns the outer `renameVars`, as the inner class does with the test's.
struct NativeClosurePassAndRenameVars {
    outer: Rc<RefCell<crate::replay::replay_dsl::Object>>,
}
impl NativeObject for NativeClosurePassAndRenameVars {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.RenameVarsTestHelpers$ClosurePassAndRenameVars"
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class_name() || class == "com.google.javascript.jscomp.CompilerPass"
    }
    // port: UnitRecorder#fields (ClosurePassAndRenameVars: its compiler field is the replay
    // compiler the pass runs on; the synthetic outer-instance field is what collect walks to reach
    // the renameVars the pass assigns)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        Ok(IndexMap::from([(
            "this$0".into(),
            DslValue::Object(self.outer.clone()),
        )]))
    }
    // port: RenameVarsTest.ClosurePassAndRenameVars#process
    fn process(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        use closure_jscomp::compiler_pass::CompilerPass;
        let (prefix, previously_used_map) = {
            let outer = self.outer.borrow();
            let field = |name: &str| outer.fields.get(name).cloned().unwrap_or(DslValue::Null);
            (
                Option::<JsString>::decode_value(&field("prefix"))?,
                decode_variable_map(&field("previouslyUsedMap"))?,
            )
        };
        let rename_vars = crate::replay::native_unported::capture(|| {
            closure_jscomp::gather_module_metadata::GatherModuleMetadata::new(
                /* processCommonJsModules= */ false,
                closure_jscomp::deps::module_loader::ResolutionMode::BROWSER,
            )
            .process(compiler, externs, root);
            let mut closure_pass =
                closure_jscomp::process_closure_provides_and_requires::ProcessClosureProvidesAndRequires::new(
                    compiler, true,
                );
            closure_pass.process(compiler, externs, root);
            RenameVars::new(
                prefix,
                false,
                false,
                false,
                previously_used_map,
                IndexSet::new(),
                Some(closure_pass.get_exported_variable_names().clone()),
                Box::new(closure_jscomp::default_name_generator::DefaultNameGenerator::new()),
            )
        })?;
        let rename_vars = Rc::new(RefCell::new(NativeRenameVars(rename_vars)));
        self.outer
            .borrow_mut()
            .fields
            .insert("renameVars".into(), DslValue::Native(rename_vars.clone()));
        rename_vars.borrow_mut().process(compiler, externs, root)
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
// port: RenameVarsTest.ClosurePassAndRenameVars#ClosurePassAndRenameVars
fn closure_pass_and_rename_vars(
    _ctx: &mut Ctx,
    args: Vec<DslValue>,
) -> Result<DslValue, Throwable> {
    let [DslValue::Object(outer), DslValue::Compiler(_)] = args.as_slice() else {
        return Err(bad());
    };
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeClosurePassAndRenameVars {
            outer: outer.clone(),
        },
    ))))
}

/// RenameProperties with its `getPropertyMap` result getter.
struct NativeRenameProperties(RenameProperties);
impl NativeObject for NativeRenameProperties {
    fn class_name(&self) -> &str {
        "com.google.javascript.jscomp.RenameProperties"
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class_name() || class == "com.google.javascript.jscomp.CompilerPass"
    }
    // port: RenameProperties#getPropertyMap
    fn call(&mut self, method: &str, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
        if method == "getPropertyMap" {
            Ok(variable_map_value(self.0.get_property_map()))
        } else {
            Err(Throwable::Unported(format!(
                "{}#{method}",
                self.class_name()
            )))
        }
    }
    // port: UnitRecorder#fields (RenameProperties: the fields the harness walks)
    fn fields(&self) -> Result<IndexMap<String, DslValue>, Throwable> {
        let f = self.0.replay_fields();
        Ok(IndexMap::from([
            (
                "generatePseudoNames".into(),
                f.generate_pseudo_names.encode_value()?,
            ),
            (
                "prevUsedPropertyMap".into(),
                f.prev_used_property_map.cloned().encode_value()?,
            ),
            (
                "reservedFirstCharacters".into(),
                f.reserved_first_characters.encode_value()?,
            ),
            (
                "reservedNonFirstCharacters".into(),
                f.reserved_non_first_characters.encode_value()?,
            ),
            ("externedNames".into(), f.externed_names.encode_value()?),
            ("quotedNames".into(), f.quoted_names.encode_value()?),
            (
                "nameGenerator".into(),
                crate::replay::options_values::encode_name_generator(f.name_generator)?,
            ),
        ]))
    }
    // port: RenameProperties#process
    fn process(
        &mut self,
        compiler: &mut crate::jscomp_api::Compiler,
        externs: NodeId,
        root: NodeId,
    ) -> Result<(), Throwable> {
        crate::replay::native_unported::capture(|| {
            self.0.process(compiler, externs, root);
        })
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
/// A `java.util.function.Predicate<Node>` helper of RenamePropertiesTest_Helpers.
struct NativeNodePredicate {
    class: &'static str,
    filter: Option<PropertyRenameEligibilityFilter>,
}
impl NativeObject for NativeNodePredicate {
    fn class_name(&self) -> &str {
        self.class
    }
    fn is_instance_of(&self, class: &str) -> bool {
        class == self.class || class == "java.util.function.Predicate"
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
fn predicate(class: &'static str, filter: PropertyRenameEligibilityFilter) -> DslValue {
    DslValue::Native(Rc::new(RefCell::new(NativeNodePredicate {
        class,
        filter: Some(filter),
    })))
}
// port: RenamePropertiesTest_Helpers.DefaultFilter#test
fn default_filter(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(predicate(
        "com.google.javascript.jscomp.RenamePropertiesTest_Helpers$DefaultFilter",
        Box::new(|_, _| true),
    ))
}
// port: RenamePropertiesTest_Helpers.GetPrefixFilter#test
fn get_prefix_filter(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(predicate(
        "com.google.javascript.jscomp.RenamePropertiesTest_Helpers$GetPrefixFilter",
        Box::new(|ast, node| {
            let name = node.get_string(ast);
            name.starts_with(&JsString::from("get"))
        }),
    ))
}
// port: RenamePropertiesTest_Helpers.FilterOnFilename#test (also RenameFunctionsFilterOnFilename
// and RenameFunctionsFilterOnFilename2, which have the same body)
fn filter_on_filename(_ctx: &mut Ctx, _args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    Ok(predicate(
        "com.google.javascript.jscomp.RenamePropertiesTest_Helpers$FilterOnFilename",
        Box::new(|ast, node| {
            let name = node.get_source_file_name(ast).unwrap_or_default();
            name == "foo.js"
        }),
    ))
}
// port: RenameProperties#RenameProperties
fn rename_properties(_ctx: &mut Ctx, args: Vec<DslValue>) -> Result<DslValue, Throwable> {
    let c = compiler(&args)?;
    let [
        _,
        generate_pseudo_names,
        prev_used_property_map,
        reserved_first_characters,
        reserved_non_first_characters,
        name_generator,
        property_rename_eligibility_filter,
    ] = args.as_slice()
    else {
        return Err(bad());
    };
    let filter = match property_rename_eligibility_filter.untyped() {
        DslValue::Native(o) => {
            let mut o = o.borrow_mut();
            let class = o.class_name().to_string();
            o.as_any_mut()
                .downcast_mut::<NativeNodePredicate>()
                .and_then(|p| p.filter.take())
                .ok_or(Throwable::Unported(class))?
        }
        _ => return Err(bad()),
    };
    let rename_properties = RenameProperties::new_with_filter(
        &c.borrow(),
        bool::decode_value(generate_pseudo_names)?,
        decode_variable_map(prev_used_property_map)?,
        decode_chars(reserved_first_characters)?,
        decode_chars(reserved_non_first_characters)?,
        decode_name_generator(name_generator)?,
        filter,
    );
    Ok(DslValue::Native(Rc::new(RefCell::new(
        NativeRenameProperties(rename_properties),
    ))))
}
