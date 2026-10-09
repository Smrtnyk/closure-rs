/*
 * Copyright 2020 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/IsolatePolyfills.java.

//! Rewrites potential polyfill usages to use the hidden JSCompiler polyfills instead of the global.
//! Port of `IsolatePolyfills.java`.
//!
//! When $jscomp.ISOLATE_POLYFILLS is enabled, the $jscomp.polyfill library function does not add
//! polyfills to the global scope or as properties on the native type. Instead, classes like Map
//! are added to the $jscomp.polyfills object. Methods like `String.prototype.includes` are defined
//! under a unique Symbol on String.prototype.
//!
//! This pass rewrites polyfill usages so that they access the actual polyfills instead of trying
//! to access the native types. For example, `new Map()` becomes `new $jscomp.polyfills['Map']`.
//!
//! Limitations of this pass: a) it ignores destructuring and b) it does not support rewriting
//! writes to polyfilled methods.
use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_options::PropertyCollapseLevel,
    compiler_pass::CompilerPass,
    node_traversal::{AbstractShallowCallback, NodeTraversal},
    node_util::NodeUtil,
    polyfill_usage_finder::{Kind, PolyfillUsage, PolyfillUsageFinder, Polyfills},
    syntactic_scope_creator::SyntacticScopeCreator,
};
use closure_parsing::parser::feature_set::FeatureSet;
use closure_resources::resources::resource_loader::ResourceLoader;
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::{
    ir::IR,
    js_string::JsString,
    node::{NodeId, Prop},
};
use std::sync::{Arc, LazyLock};

// port: IsolatePolyfills#POLYFILL_TEMP
const POLYFILL_TEMP: &str = "$jscomp$polyfillTmp";

// port: IsolatePolyfills
pub struct IsolatePolyfills {
    polyfills: Arc<Polyfills>,
    jscomp_polyfills_object: NodeId,
    jscomp_lookup_method: NodeId,
    used_polyfill_method_lookup: bool,
    is_temp_var_initialized: bool,
}

// Code in the runtime libraries that may execute before any polyfills are injected and needs
// to opt out from polyfill isolation.
//   - util/global.js looks for `globalThis`
//   - util/shouldpolyfill.js checks whether Symbol is native
//   - es6/util/construct.js looks for the native Reflect.construct
// If desired we could programmatically detect these cases instead of having this allow list of
// polyfill accesses, but that might silently allow other usages of third-party polyfills
// into the codebase.
// TODO(b/156776817): crash on early references to polyfills that are not in our allow list.
// port: IsolatePolyfills#FILES_ALLOWED_UNQUALIFIED_POLYFILL_ACCESSES
static FILES_ALLOWED_UNQUALIFIED_POLYFILL_ACCESSES: LazyLock<IndexSet<String>> =
    LazyLock::new(|| {
        IndexSet::<_>::from_iter([
            format!("{}util/global.js", AbstractCompiler::RUNTIME_LIB_DIR),
            format!(
                "{}util/shouldpolyfill.js",
                AbstractCompiler::RUNTIME_LIB_DIR
            ),
            format!("{}es6/util/construct.js", AbstractCompiler::RUNTIME_LIB_DIR),
        ])
    });

impl IsolatePolyfills {
    // port: IsolatePolyfills#IsolatePolyfills(AbstractCompiler)
    pub fn new(compiler: &mut AbstractCompiler) -> Self {
        Self::new_with_polyfills(
            compiler,
            Polyfills::from_table(&ResourceLoader::load_text_resource(
                "com.google.javascript.jscomp.IsolatePolyfills",
                "js/polyfills.txt",
            )),
        )
    }

    // port: IsolatePolyfills#IsolatePolyfills(AbstractCompiler,Polyfills)
    pub fn new_with_polyfills(compiler: &mut AbstractCompiler, polyfills: Polyfills) -> Self {
        let jscomp_lookup_method = IR::name(compiler, "$jscomp$lookupPolyfilledValue");
        let has_property_collapsing_run =
            compiler.get_options().get_property_collapse_level() == PropertyCollapseLevel::ALL;
        let jscomp_polyfills_object = if has_property_collapsing_run {
            Self::create_collapsed_name(compiler)
        } else {
            Self::create_js_comp_polyfills_access(compiler)
        };
        Self {
            polyfills: Arc::new(polyfills),
            jscomp_polyfills_object,
            jscomp_lookup_method,
            used_polyfill_method_lookup: false,
            is_temp_var_initialized: false,
        }
    }

    /// Returns a name `$jscomp$polyfills`
    // port: IsolatePolyfills#createCollapsedName
    fn create_collapsed_name(compiler: &mut AbstractCompiler) -> NodeId {
        let collapsed_name = IR::name(compiler, "$jscomp$polyfills");
        collapsed_name.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
        collapsed_name
    }

    /// Returns a getprop `$jscomp.polyfills`
    // port: IsolatePolyfills#createJSCompPolyfillsAccess
    fn create_js_comp_polyfills_access(compiler: &mut AbstractCompiler) -> NodeId {
        let jscomp = IR::name(compiler, "$jscomp");
        jscomp.put_boolean_prop(compiler, Prop::IS_CONSTANT_NAME, true);
        IR::getprop(compiler, jscomp, "polyfills")
    }

    /// Searches the AST for all calls to $jscomp.polyfill and returns the polyfilled symbol names.
    ///
    /// At the moment we don't track anywhere the set of all polyfills that have been injected.
    /// That set may be modified by RewritePolyfills, Es6InjectRuntimeLibraries, and
    /// RemoveUnusedCode. If desired, we could delete this method by making any passes that
    /// add/remove polyfill calls responsible for tracking their presence.
    ///
    /// Note: this set cannot be injected into the constructor because it is not known until the
    /// polyfill injection pass actually runs.
    // port: IsolatePolyfills#findAllInjectedPolyfills
    fn find_all_injected_polyfills(compiler: &mut AbstractCompiler) -> IndexSet<JsString> {
        let mut actual_polyfills = IndexSet::<_>::default();

        let last_injected_node = compiler.get_node_for_code_insertion(None);

        NodeTraversal::traverse(
            compiler,
            last_injected_node,
            &mut AbstractShallowCallback::new(
                |t: &mut NodeTraversal<'_>, n: NodeId, _parent: Option<NodeId>| {
                    if Self::is_js_comp_polyfill_call(t.get_compiler(), n) {
                        // CALL
                        //  GETPROP/NAME $jscomp.polyfill
                        //  STRING NativeSymbol.prototype.method
                        //  [...]
                        let polyfilled_symbol = n.get_second_child(t).unwrap().get_string(t);
                        actual_polyfills.insert(polyfilled_symbol);
                    }
                },
            ),
        );

        actual_polyfills
    }

    // port: IsolatePolyfills#isJSCompPolyfillCall
    fn is_js_comp_polyfill_call(compiler: &AbstractCompiler, call: NodeId) -> bool {
        if !call.is_call(compiler) {
            return false;
        }
        let jscomp_polyfill_name =
            if compiler.get_options().get_property_collapse_level() == PropertyCollapseLevel::ALL {
                "$jscomp$polyfill"
            } else {
                "$jscomp.polyfill"
            };
        call.get_first_child(compiler)
            .unwrap()
            .matches_qualified_name(compiler, jscomp_polyfill_name)
    }

    /// Rewrites a potential access of a polyfilled class or method to first look for the
    /// non-global, polyfilled version.
    // port: IsolatePolyfills#rewritePolyfill
    fn rewrite_polyfill(
        &mut self,
        compiler: &mut AbstractCompiler,
        polyfill_usage: &PolyfillUsage,
    ) {
        let polyfill = polyfill_usage.polyfill();

        // If the output FeatureSet includes all of the features of the language version in which
        // the polyfilled symbol was introduced, we can assume that the intended platform has
        // the symbol defined natively, so the compiler won't include the polyfill in its output
        // and it won't need to be isolated.
        if compiler.get_options().get_output_feature_set().contains(
            FeatureSet::value_of(&polyfill.native_version).unwrap_or_else(|e| panic!("{e}")),
        ) {
            return;
        }

        let polyfill_access = polyfill_usage.node();
        let parent = polyfill_usage.node().get_parent(compiler).unwrap();

        if polyfill_access
            .get_source_file_name(compiler)
            .is_some_and(|name| FILES_ALLOWED_UNQUALIFIED_POLYFILL_ACCESSES.contains(&name))
        {
            return;
        }

        // For now we need to assume that these lvalues are unrelated to the polyfills, as we are
        // not using any type information. If we wanted, we could support assignments to
        // properties that are already present, e.g.
        //   Array.includes = intercept;
        // to
        //   tmp = Array;
        //   tmp[$maybePolyfillProp(tmp, 'includes)] = intercept;
        if NodeUtil::is_l_value(compiler, polyfill_access)
            || (parent.is_assign(compiler)
                && polyfill_access.is_first_child_of(compiler, Some(parent)))
        {
            return;
        }

        let name = polyfill_usage.name();
        let is_global_class =
            name.index_of_char(u16::from(b'.')) == -1 && polyfill.kind == Kind::STATIC;

        if is_global_class {
            // e.g. Symbol, Map, window.Map, or goog.global.Map
            // Optional chaining is forbidden on global classes, hence producing `getelem` for
            // property access would suffice.
            let object = self.jscomp_polyfills_object.clone_tree(compiler);
            let string = IR::string(compiler, name.clone());
            let replacement = IR::getelem(compiler, object, string) // $jscomp.polyfills['Map']
                .srcref_tree(compiler, polyfill_access);
            polyfill_access.replace_with(compiler, replacement);
        } else if (parent.is_call(compiler) || parent.is_opt_chain_call(compiler))
            && polyfill_access.is_first_child_of(compiler, Some(parent))
        {
            // e.g. `getStr().includes('x')` or `getStr()?.includes('x')`
            self.rewrite_polyfill_in_call(compiler, polyfill_access);
        } else {
            // e.g. [].includes.call(myIter, 0)
            let method_string = polyfill_access.get_string(compiler);
            let method_name = IR::string(compiler, method_string).srcref(compiler, polyfill_access);
            let receiver = polyfill_access.remove_first_child(compiler).unwrap();
            // The `$jscomp$lookupPolyfilledValue` can handle both normal prop access as well as
            // optional by checking whether the lhs (receiver) is null or undefined first.
            let is_opt_chain_node = NodeUtil::is_opt_chain_node(compiler, polyfill_access);
            let replacement = self
                .create_polyfill_method_lookup(compiler, receiver, method_name, is_opt_chain_node)
                .srcref_tree(compiler, polyfill_access);
            polyfill_access.replace_with(compiler, replacement);
        }

        compiler.report_change_to_enclosing_scope(parent);
    }

    /// Rewrites a call where the receiver is a potential polyfilled method
    ///
    /// Before: `receiver.method(arg)`
    ///
    /// After: `lookupPolyfilledValue(receiver, 'method').call(receiver, arg)`
    ///
    /// Or, if evaluating the receiver may have side effects, we store the receiver in a temporary
    /// variable to avoid evaluating it twice:
    ///
    /// After: `(tmpNode = receiver, lookupPolyfilledValue(tmpNode, 'method')).call(tmpNode, arg)`
    // port: IsolatePolyfills#rewritePolyfillInCall
    fn rewrite_polyfill_in_call(&mut self, compiler: &mut AbstractCompiler, callee: NodeId) {
        let callee_string = callee.get_string(compiler);
        let method_name = IR::string(compiler, callee_string).srcref(compiler, callee);
        let receiver = callee.remove_first_child(compiler).unwrap();
        let is_callee_opt_chain = NodeUtil::is_opt_chain_node(compiler, callee);

        let requires_temp = compiler
            .get_ast_analyzer()
            .may_effect_mutable_state(compiler, receiver);

        let polyfilled_method;
        let this_node;

        if requires_temp {
            // e.g. `sideEffects().includes(arg)`
            this_node = self.create_temp_name(compiler, callee);
            // (tmpNode = sideEffects(), lookupMethod(tmpNode, 'includes'))
            let tmp_target = this_node.clone_tree(compiler);
            let assign = IR::assign(compiler, tmp_target, receiver);
            let tmp_receiver = this_node.clone_tree(compiler);
            let lookup = self.create_polyfill_method_lookup(
                compiler,
                tmp_receiver,
                method_name,
                is_callee_opt_chain,
            );
            polyfilled_method = IR::comma(compiler, assign, lookup);
        } else {
            this_node = receiver;
            let receiver_clone = receiver.clone_tree(compiler);
            polyfilled_method = self.create_polyfill_method_lookup(
                compiler,
                receiver_clone,
                method_name,
                is_callee_opt_chain,
            );
        }

        // Fix the `this` type by using .call:
        //   lookupMethod(receiver, 'includes', isOptChainNode).call(receiver, arg)
        let receiver_dot_call = if NodeUtil::is_opt_chain_node(compiler, callee) {
            // If optional chaining exists at this point, we can have it in the output
            IR::start_opt_chain_getprop(compiler, polyfilled_method, "call")
                .srcref_tree(compiler, callee)
        } else {
            IR::getprop(compiler, polyfilled_method, "call").srcref_tree(compiler, callee)
        };
        callee.replace_with(compiler, receiver_dot_call);
        this_node.insert_after(compiler, receiver_dot_call);
    }

    // port: IsolatePolyfills#createTempName
    fn create_temp_name(&mut self, compiler: &mut AbstractCompiler, srcref: NodeId) -> NodeId {
        if !self.is_temp_var_initialized {
            self.is_temp_var_initialized = true;
            let name = IR::name(compiler, POLYFILL_TEMP);
            let decl = IR::var(compiler, name).srcref_tree(compiler, srcref);
            let insertion = compiler.get_node_for_code_insertion(None);
            insertion.add_child_to_front(compiler, decl);
            compiler.report_change_to_enclosing_scope(decl);
        }
        // The same temporary variable is always used for every polyfill invocation. This is
        // believed to be safe and makes the code easier to generate and smaller. There's a change
        // it will make it harder for V8 to optimize, though. If proves to be a problem we could
        // introduce unique tmp variables.
        IR::name(compiler, POLYFILL_TEMP).srcref(compiler, srcref)
    }

    /// Returns a call `$jscomp$lookupPolyfilledValue(receiver, 'methodName', isOptChainNode)`
    // port: IsolatePolyfills#createPolyfillMethodLookup
    fn create_polyfill_method_lookup(
        &mut self,
        compiler: &mut AbstractCompiler,
        receiver: NodeId,
        method_name: NodeId,
        is_opt_chain_node: bool,
    ) -> NodeId {
        self.used_polyfill_method_lookup = true;
        let call = if is_opt_chain_node {
            let lookup = self.jscomp_lookup_method.clone_tree(compiler);
            let true_node = IR::true_node(compiler);
            IR::call(compiler, lookup, &[receiver, method_name, true_node])
        } else {
            // 3rd param of `jscompLookupMethod` is optional
            let lookup = self.jscomp_lookup_method.clone_tree(compiler);
            IR::call(compiler, lookup, &[receiver, method_name])
        };
        call.put_boolean_prop(compiler, Prop::FREE_CALL, true);
        call
    }

    /// Deletes the dummy externs declaration of $jscomp$lookupPolyfilledValue and the function
    /// itself if unused.
    ///
    /// The RewritePolyfills pass injected a definition of $jscomp$lookupPolyfilledValue into the
    /// externs. This prevented dead code elimination, since the function is never unused until
    /// this pass runs. However, now we need to delete the externs definition so that variable
    /// renaming can actually rename $jscomp$lookupPolyfilledValue.
    // port: IsolatePolyfills#cleanUpJscompLookupPolyfilledValue
    fn clean_up_jscomp_lookup_polyfilled_value(&mut self, compiler: &mut AbstractCompiler) {
        let lookup_name = self.jscomp_lookup_method.get_string(compiler);
        let synthetic_externs_root = compiler.get_synthesized_externs_root();
        let synthetic_externs_scope = SyntacticScopeCreator::new().create_scope(
            compiler,
            synthetic_externs_root,
            /* parent= */ None,
        );
        let extern_var = synthetic_externs_scope
            .get_var(compiler, &lookup_name)
            .expect("Failed to find synthetic $jscomp$lookupPolyfilledValue extern");
        let extern_parent = extern_var.get_parent_node(compiler).unwrap();
        NodeUtil::delete_node(compiler, extern_parent);

        if self.used_polyfill_method_lookup {
            return;
        }

        let insertion = compiler.get_node_for_code_insertion(/* chunk= */ None);
        let synthetic_code_scope =
            SyntacticScopeCreator::new().create_scope(compiler, insertion, /* parent= */ None);
        let synthetic_var = synthetic_code_scope.get_var(compiler, &lookup_name);
        // Don't error if we can't find a definition for jscompLookupMethod. It's possible that we
        // are running in transpileOnly mode and are not injecting runtime libraries.
        if let Some(synthetic_var) = synthetic_var {
            let parent = synthetic_var.get_parent_node(compiler).unwrap();
            NodeUtil::delete_node(compiler, parent);
        }
    }
}

impl CompilerPass for IsolatePolyfills {
    // port: IsolatePolyfills#process
    fn process(&mut self, compiler: &mut AbstractCompiler, _externs: NodeId, root: NodeId) {
        // Calculate the set of polyfills that are actually present in the AST. It may be a subset
        // of the potential polyfills which PolyfillFindingCallback finds (it's fine if it's a
        // superset.)
        let injected_polyfills = Self::find_all_injected_polyfills(compiler);

        let mut polyfill_usages: Vec<PolyfillUsage> = Vec::new();
        PolyfillUsageFinder::new(Arc::clone(&self.polyfills)).traverse_including_guarded(
            compiler,
            root,
            &mut |_, usage| polyfill_usages.push(usage),
        );

        let mut visited_nodes: IndexSet<NodeId> = IndexSet::<_>::default();
        for usage in &polyfill_usages {
            if
            // Some nodes map to more than one polyfill usage. For example, `x.includes` maps to
            // both Array.prototype.includes and String.prototype.includes, but only needs to be
            // isolated once.
            visited_nodes.contains(&usage.node())
                // Skip visiting nodes whose 'polyfill.library' is empty. This is true for
                // language features like `Proxy` and `String.raw` that have no associated
                // polyfill, and hence are unnecessary to isolate.
                || usage.polyfill().library.is_empty()
                // The PolyfillFindingCallback may detect possible polyfill usages that are not
                // in fact injected. (possibly because RemoveUnusedCode deleted the polyfill.)
                || !injected_polyfills.contains(&usage.polyfill().native_symbol)
            {
                continue;
            }
            self.rewrite_polyfill(compiler, usage);
            visited_nodes.insert(usage.node());
        }

        self.clean_up_jscomp_lookup_polyfilled_value(compiler);
    }
}
