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
//   src/com/google/javascript/jscomp/OptimizeReturns.java.

use crate::{
    abstract_compiler::AbstractCompiler,
    compiler_pass::CompilerPass,
    diagnostic::log_file::LogFile,
    node_util::{MatchShallowStatement, NodeUtil},
    optimize_calls::{CallGraphCompilerPass, OptimizeCalls, ReferenceMap},
};
use closure_rhino::{
    check_state,
    ir::IR,
    js_string::JsString,
    node::{Ast, NodeId},
    token::Token,
};

/// A compiler pass to optimize function return results. Currently this pass looks for results
/// that are completely unused and rewrites them as: "return x()" --> "x(); return"
///
/// Future work: expand this to look for use context to avoid unneeded type coercion:
///
/// "return x.toString()" --> "return x"
///
/// "return !!x" --> "return x"
#[derive(Default)]
pub struct OptimizeReturns {
    // Allocated & cleaned up by process()
    decisions_log: Option<Box<dyn LogFile>>,
}

impl OptimizeReturns {
    // port: OptimizeReturns#OptimizeReturns
    pub fn new() -> Self {
        Self {
            decisions_log: None,
        }
    }

    // Java: decisionsLog.log(format, args). LogFile#log takes the message lazily, which is what
    // Java's `if (decisionsLog.isLogging())` guards achieve.
    fn log(&mut self, message: impl FnOnce() -> String) {
        let mut message = Some(message);
        self.decisions_log
            .as_mut()
            .unwrap()
            .log(&mut || message.take().unwrap()());
    }

    /// This reference set is a candidate for return-value-removal if: - if the all call sites are
    /// known (not aliased, not exported) - if all call sites do not use the return value - if
    /// there is at least one known function definition - if there is at least one use NOTE:
    /// unknown definitions are allowed, as only known definitions will be removed.
    // port: OptimizeReturns#isCandidate
    fn is_candidate(
        &mut self,
        compiler: &AbstractCompiler,
        name: &JsString,
        refs: &[NodeId],
    ) -> bool {
        if !OptimizeCalls::may_be_optimizable_name(compiler, name) {
            self.log(|| format!("{name}\tnot an optimizable name"));
            return false;
        }

        let mut seen_candidate_definiton = false;
        let mut seen_use = false;
        for &n in refs {
            // Assume indirect definitions references use the result
            if ReferenceMap::is_call_target(compiler, n)
                || ReferenceMap::is_opt_chain_call_target(compiler, n)
            {
                let call_node = ReferenceMap::get_call_or_new_node_for_target(compiler, n);
                if NodeUtil::is_expression_result_used(compiler, call_node) {
                    // At least one call site uses the return value, this
                    // is not a candidate.
                    // avoid build location string when not logging
                    self.log(|| {
                        format!(
                            "{name}\treturn value used: {}",
                            call_node.get_location(compiler)
                        )
                    });
                    return false;
                }
                seen_use = true;
            } else if Self::is_candidate_definition(compiler, n) {
                // NOTE: While is is possible to optimize calls to functions for which we know
                // only some of the definition are candidates but to keep things simple, only
                // optimize if all of the definitions are known.
                seen_candidate_definiton = true;
            } else {
                // If this isn't an non-aliasing reference (typeof, instanceof, etc)
                // then there is nothing that can be done.
                if !OptimizeCalls::is_allowed_reference(compiler, n) {
                    // avoid build location string when not logging
                    self.log(|| {
                        format!("{name}\tdisallowed reference: {}", n.get_location(compiler))
                    });
                    return false;
                }
            }
        }

        if !seen_use {
            self.log(|| format!("{name}\tno usage seen"));
            return false;
        }
        if !seen_candidate_definiton {
            self.log(|| format!("{name}\tno definition seen"));
            return false;
        }
        true
    }

    // port: OptimizeReturns#isCandidateDefinition
    fn is_candidate_definition(compiler: &AbstractCompiler, n: NodeId) -> bool {
        let ast: &Ast = compiler;
        let parent = n.get_parent(ast).unwrap();
        if parent.is_function(ast) && NodeUtil::is_function_declaration(ast, parent) {
            return true;
        } else if ReferenceMap::is_simple_assignment_target(compiler, n) {
            if Self::is_candidate_function(ast, parent.get_last_child(ast).unwrap()) {
                return true;
            }
        } else if n.is_name(ast) {
            if n.has_children(ast)
                && Self::is_candidate_function(ast, n.get_first_child(ast).unwrap())
            {
                return true;
            }
        } else if Self::is_class_member_definition(ast, n) {
            return true;
        }

        false
    }

    // port: OptimizeReturns#isClassMemberDefinition
    fn is_class_member_definition(ast: &Ast, n: NodeId) -> bool {
        n.is_member_function_def(ast) && n.get_parent(ast).unwrap().is_class_members(ast)
    }

    // port: OptimizeReturns#isCandidateFunction
    fn is_candidate_function(ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::FUNCTION => {
                // Named function expression can be recursive, this creates an alias of the
                // name, meaning it might be used in an unexpected way.
                !NodeUtil::is_named_function_expression(ast, n)
            }
            Token::COMMA | Token::CAST => {
                Self::is_candidate_function(ast, n.get_last_child(ast).unwrap())
            }
            Token::HOOK => {
                Self::is_candidate_function(ast, n.get_second_child(ast).unwrap())
                    && Self::is_candidate_function(ast, n.get_last_child(ast).unwrap())
            }
            Token::OR | Token::AND | Token::COALESCE => {
                Self::is_candidate_function(ast, n.get_first_child(ast).unwrap())
                    && Self::is_candidate_function(ast, n.get_last_child(ast).unwrap())
            }
            _ => false,
        }
    }

    /// For the supplied function node, rewrite all the return expressions so that: return foo();
    /// becomes: foo(); return; Useless return will be removed later by the peephole optimization
    /// passes.
    // port: OptimizeReturns#rewriteReturns
    fn rewrite_returns(compiler: &mut AbstractCompiler, fn_node: NodeId) {
        check_state!(fn_node.is_function(compiler));
        let body = fn_node.get_last_child(compiler).unwrap();
        visit_post_order(
            compiler,
            body,
            &mut |compiler: &mut AbstractCompiler, n: NodeId| {
                if n.is_return(compiler) && n.has_one_child(compiler) {
                    let result = n.get_first_child(compiler).unwrap();
                    let keep_value = !Self::is_removable_value(compiler, result);
                    result.detach(compiler);
                    if keep_value {
                        let expr = IR::expr_result(compiler, result);
                        expr.srcref(compiler, result).insert_before(compiler, n);
                    } else {
                        NodeUtil::mark_functions_deleted(compiler, result);
                    }
                    compiler.report_change_to_enclosing_scope(body);
                }
            },
            &|ast: &Ast, n: NodeId| MatchShallowStatement.apply(ast, n),
        );
    }

    // Just remove objects that don't reference properties (object literals) or names (functions)
    // So we don't need to update the graph.
    // port: OptimizeReturns#isRemovableValue
    fn is_removable_value(ast: &Ast, n: NodeId) -> bool {
        match n.get_token(ast) {
            Token::TEMPLATELIT | Token::ARRAYLIT => {
                let mut child = n.get_first_child(ast);
                while let Some(c) = child {
                    if (!c.is_empty(ast)) && !Self::is_removable_value(ast, c) {
                        return false;
                    }
                    child = c.get_next(ast);
                }
                true
            }
            Token::REGEXP
            | Token::STRINGLIT
            | Token::NUMBER
            | Token::NULL
            | Token::TRUE
            | Token::FALSE
            | Token::TEMPLATELIT_STRING => true,
            Token::TEMPLATELIT_SUB | Token::CAST | Token::NOT | Token::VOID | Token::NEG => {
                Self::is_removable_value(ast, n.get_first_child(ast).unwrap())
            }
            _ => false,
        }
    }
}

/// NodeUtil#visitPostOrder(Node,Visitor,Predicate) with a visitor that receives the compiler:
/// the anonymous NodeUtil.Visitor of rewriteReturns uses the outer pass's `compiler`, which
/// NodeUtil::visit_post_order_with_predicate (a `&mut Ast` visitor) cannot pass. Same traversal:
/// children first (the next sibling is read before a child is visited), then the node.
fn visit_post_order(
    compiler: &mut AbstractCompiler,
    node: NodeId,
    visitor: &mut dyn FnMut(&mut AbstractCompiler, NodeId),
    traverse_children_pred: &dyn Fn(&Ast, NodeId) -> bool,
) {
    if traverse_children_pred(compiler, node) {
        let mut c = node.get_first_child(compiler);
        while let Some(current) = c {
            let next = current.get_next(compiler);
            visit_post_order(compiler, current, visitor, traverse_children_pred);
            c = next;
        }
    }
    visitor(compiler, node);
}

impl CompilerPass for OptimizeReturns {
    // port: OptimizeReturns#process(Node,Node)
    fn process(&mut self, compiler: &mut AbstractCompiler, externs: NodeId, root: NodeId) {
        // Java passes `this`; the builder owns its passes here, and an OptimizeReturns holds no
        // state between runs (decisionsLog is allocated and cleared by each process call), so a
        // new instance is the same pass.
        OptimizeCalls::builder()
            .set_compiler(compiler)
            .set_consider_externs(false)
            .add_pass(Box::new(OptimizeReturns::new()))
            .build()
            .process(compiler, externs, root);
    }
}

impl CallGraphCompilerPass for OptimizeReturns {
    // port: OptimizeReturns#process(Node,Node,ReferenceMap)
    fn process(
        &mut self,
        compiler: &mut AbstractCompiler,
        _externs: NodeId,
        _root: NodeId,
        definitions: &mut ReferenceMap,
    ) {
        // try-with-resources: the log is closed when dropped; finally: decisionsLog = null.
        let log_file =
            compiler.create_or_reopen_indexed_log("OptimizeReturns", "decisions.log", &[]);
        self.decisions_log = Some(log_file); // avoid passing the log file through a bunch of methods
        // Find all function nodes whose callers ignore the return values.
        let mut to_optimize: Vec<&Vec<NodeId>> = Vec::new();

        // Find all the candidates before modifying the AST.
        for (key, refs) in definitions.get_name_references() {
            if self.is_candidate(compiler, key, refs) {
                self.log(|| format!("name {key}\tremoving return value"));
                to_optimize.push(refs);
            }
        }

        for (key, refs) in definitions.get_prop_references() {
            if self.is_candidate(compiler, key, refs) {
                self.log(|| format!("property {key}\tremoving return value"));
                to_optimize.push(refs);
            }
        }

        // Now modify the AST
        for refs in to_optimize {
            for fns in ReferenceMap::get_function_nodes(compiler, refs).values() {
                for &fn_node in fns {
                    Self::rewrite_returns(compiler, fn_node);
                }
            }
        }
        self.decisions_log = None;
    }
}
