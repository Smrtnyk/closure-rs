/*
 * Copyright 2011 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/parsing/parser/trees/ParseTree.java.

use super::*;
use std::rc::Rc;

pub type Tree = Rc<ParseTree>;

#[derive(Clone, Debug)]
pub enum ParseTreeData {
    ArgumentListTree(Box<ArgumentListTree>),
    ArrayLiteralExpressionTree(Box<ArrayLiteralExpressionTree>),
    ArrayPatternTree(Box<ArrayPatternTree>),
    AwaitExpressionTree(Box<AwaitExpressionTree>),
    BinaryOperatorTree(Box<BinaryOperatorTree>),
    BlockTree(Box<BlockTree>),
    BreakStatementTree(Box<BreakStatementTree>),
    CallExpressionTree(Box<CallExpressionTree>),
    CaseClauseTree(Box<CaseClauseTree>),
    CatchTree(Box<CatchTree>),
    ClassDeclarationTree(Box<ClassDeclarationTree>),
    CommaExpressionTree(Box<CommaExpressionTree>),
    ComprehensionForTree(Box<ComprehensionForTree>),
    ComprehensionIfTree(Box<ComprehensionIfTree>),
    ComprehensionTree(Box<ComprehensionTree>),
    ComputedPropertyDefinitionTree(Box<ComputedPropertyDefinitionTree>),
    ComputedPropertyFieldTree(Box<ComputedPropertyFieldTree>),
    ComputedPropertyGetterTree(Box<ComputedPropertyGetterTree>),
    ComputedPropertyMethodTree(Box<ComputedPropertyMethodTree>),
    ComputedPropertySetterTree(Box<ComputedPropertySetterTree>),
    ConditionalExpressionTree(Box<ConditionalExpressionTree>),
    ContinueStatementTree(Box<ContinueStatementTree>),
    DebuggerStatementTree(Box<DebuggerStatementTree>),
    DefaultClauseTree(Box<DefaultClauseTree>),
    DefaultParameterTree(Box<DefaultParameterTree>),
    DoWhileStatementTree(Box<DoWhileStatementTree>),
    DynamicImportTree(Box<DynamicImportTree>),
    EmptyStatementTree(Box<EmptyStatementTree>),
    ExportDeclarationTree(Box<ExportDeclarationTree>),
    ExportSpecifierTree(Box<ExportSpecifierTree>),
    ExpressionStatementTree(Box<ExpressionStatementTree>),
    FieldDeclarationTree(Box<FieldDeclarationTree>),
    FinallyTree(Box<FinallyTree>),
    ForAwaitOfStatementTree(Box<ForAwaitOfStatementTree>),
    ForInStatementTree(Box<ForInStatementTree>),
    ForOfStatementTree(Box<ForOfStatementTree>),
    ForStatementTree(Box<ForStatementTree>),
    FormalParameterListTree(Box<FormalParameterListTree>),
    GetAccessorTree(Box<GetAccessorTree>),
    IdentifierExpressionTree(Box<IdentifierExpressionTree>),
    IfStatementTree(Box<IfStatementTree>),
    ImportDeclarationTree(Box<ImportDeclarationTree>),
    ImportMetaExpressionTree(Box<ImportMetaExpressionTree>),
    ImportSpecifierTree(Box<ImportSpecifierTree>),
    IterRestTree(Box<IterRestTree>),
    IterSpreadTree(Box<IterSpreadTree>),
    LabelledStatementTree(Box<LabelledStatementTree>),
    LiteralExpressionTree(Box<LiteralExpressionTree>),
    MemberExpressionTree(Box<MemberExpressionTree>),
    MemberLookupExpressionTree(Box<MemberLookupExpressionTree>),
    MissingPrimaryExpressionTree(Box<MissingPrimaryExpressionTree>),
    NewExpressionTree(Box<NewExpressionTree>),
    NewTargetExpressionTree(Box<NewTargetExpressionTree>),
    NullTree(Box<NullTree>),
    ObjectLiteralExpressionTree(Box<ObjectLiteralExpressionTree>),
    ObjectPatternTree(Box<ObjectPatternTree>),
    ObjectRestTree(Box<ObjectRestTree>),
    ObjectSpreadTree(Box<ObjectSpreadTree>),
    OptChainCallExpressionTree(Box<OptChainCallExpressionTree>),
    OptionalMemberExpressionTree(Box<OptionalMemberExpressionTree>),
    OptionalMemberLookupExpressionTree(Box<OptionalMemberLookupExpressionTree>),
    ParenExpressionTree(Box<ParenExpressionTree>),
    ProgramTree(Box<ProgramTree>),
    PropertyNameAssignmentTree(Box<PropertyNameAssignmentTree>),
    ReturnStatementTree(Box<ReturnStatementTree>),
    SetAccessorTree(Box<SetAccessorTree>),
    SuperExpressionTree(Box<SuperExpressionTree>),
    SwitchStatementTree(Box<SwitchStatementTree>),
    TemplateLiteralExpressionTree(Box<TemplateLiteralExpressionTree>),
    TemplateLiteralPortionTree(Box<TemplateLiteralPortionTree>),
    TemplateSubstitutionTree(Box<TemplateSubstitutionTree>),
    ThisExpressionTree(Box<ThisExpressionTree>),
    ThrowStatementTree(Box<ThrowStatementTree>),
    TryStatementTree(Box<TryStatementTree>),
    UnaryExpressionTree(Box<UnaryExpressionTree>),
    UpdateExpressionTree(Box<UpdateExpressionTree>),
    VariableDeclarationListTree(Box<VariableDeclarationListTree>),
    VariableDeclarationTree(Box<VariableDeclarationTree>),
    VariableStatementTree(Box<VariableStatementTree>),
    WhileStatementTree(Box<WhileStatementTree>),
    WithStatementTree(Box<WithStatementTree>),
    YieldExpressionTree(Box<YieldExpressionTree>),
    FunctionDeclarationTree(Box<FunctionDeclarationTree>),
}

// An abstract syntax tree for JavaScript parse trees.
// Immutable.
// A plain old data structure. Should include data members and simple accessors only.
//
// Derived classes should have a 'Tree' suffix. Each concrete derived class should have a
// ParseTreeType whose name matches the derived class name.
//
// A parse tree derived from source should have a non-null location. A parse tree that is
// synthesized by the compiler may have a null location.
//
// When adding a new subclass of ParseTree you must also do the following:
//   - add a new entry to ParseTreeType
//   - add ParseTree.asXTree()
#[derive(Clone, Debug)]
pub struct ParseTree {
    pub type_: ParseTreeType,
    pub location: SourceRange,
    pub data: ParseTreeData,
}
impl ParseTree {
    // port: ParseTree#<init>
    #[allow(clippy::new_ret_no_self)]
    pub fn new(type_: ParseTreeType, location: SourceRange, data: ParseTreeData) -> Tree {
        Rc::new(Self {
            type_,
            location,
            data,
        })
    }
    pub fn as_argument_list(&self) -> &ArgumentListTree {
        match &self.data {
            ParseTreeData::ArgumentListTree(value) => value,
            _ => panic!("ClassCastException: ArgumentListTree"),
        }
    }
    // port: ParseTree#getStart
    pub fn get_start(&self) -> SourcePosition {
        self.location.start.clone()
    }
    // port: ParseTree#getEnd
    pub fn get_end(&self) -> SourcePosition {
        self.location.end.clone()
    }
    // port: ParseTree#asArrayLiteralExpression
    pub fn as_array_literal_expression(&self) -> &ArrayLiteralExpressionTree {
        match &self.data {
            ParseTreeData::ArrayLiteralExpressionTree(value) => value,
            _ => panic!("ClassCastException: ArrayLiteralExpressionTree"),
        }
    }
    // port: ParseTree#asArrayPattern
    pub fn as_array_pattern(&self) -> &ArrayPatternTree {
        match &self.data {
            ParseTreeData::ArrayPatternTree(value) => value,
            _ => panic!("ClassCastException: ArrayPatternTree"),
        }
    }
    // port: ParseTree#asBinaryOperator
    pub fn as_binary_operator(&self) -> &BinaryOperatorTree {
        match &self.data {
            ParseTreeData::BinaryOperatorTree(value) => value,
            _ => panic!("ClassCastException: BinaryOperatorTree"),
        }
    }
    // port: ParseTree#asBlock
    pub fn as_block(&self) -> &BlockTree {
        match &self.data {
            ParseTreeData::BlockTree(value) => value,
            _ => panic!("ClassCastException: BlockTree"),
        }
    }
    // port: ParseTree#asBreakStatement
    pub fn as_break_statement(&self) -> &BreakStatementTree {
        match &self.data {
            ParseTreeData::BreakStatementTree(value) => value,
            _ => panic!("ClassCastException: BreakStatementTree"),
        }
    }
    // port: ParseTree#asCallExpression
    pub fn as_call_expression(&self) -> &CallExpressionTree {
        match &self.data {
            ParseTreeData::CallExpressionTree(value) => value,
            _ => panic!("ClassCastException: CallExpressionTree"),
        }
    }
    // port: ParseTree#asOptChainCallExpression
    pub fn as_opt_chain_call_expression(&self) -> &OptChainCallExpressionTree {
        match &self.data {
            ParseTreeData::OptChainCallExpressionTree(value) => value,
            _ => panic!("ClassCastException: OptChainCallExpressionTree"),
        }
    }
    // port: ParseTree#asCaseClause
    pub fn as_case_clause(&self) -> &CaseClauseTree {
        match &self.data {
            ParseTreeData::CaseClauseTree(value) => value,
            _ => panic!("ClassCastException: CaseClauseTree"),
        }
    }
    // port: ParseTree#asCatch
    pub fn as_catch(&self) -> &CatchTree {
        match &self.data {
            ParseTreeData::CatchTree(value) => value,
            _ => panic!("ClassCastException: CatchTree"),
        }
    }
    // port: ParseTree#asClassDeclaration
    pub fn as_class_declaration(&self) -> &ClassDeclarationTree {
        match &self.data {
            ParseTreeData::ClassDeclarationTree(value) => value,
            _ => panic!("ClassCastException: ClassDeclarationTree"),
        }
    }
    // port: ParseTree#asCommaExpression
    pub fn as_comma_expression(&self) -> &CommaExpressionTree {
        match &self.data {
            ParseTreeData::CommaExpressionTree(value) => value,
            _ => panic!("ClassCastException: CommaExpressionTree"),
        }
    }
    // port: ParseTree#asComprehensionIf
    pub fn as_comprehension_if(&self) -> &ComprehensionIfTree {
        match &self.data {
            ParseTreeData::ComprehensionIfTree(value) => value,
            _ => panic!("ClassCastException: ComprehensionIfTree"),
        }
    }
    // port: ParseTree#asComprehensionFor
    pub fn as_comprehension_for(&self) -> &ComprehensionForTree {
        match &self.data {
            ParseTreeData::ComprehensionForTree(value) => value,
            _ => panic!("ClassCastException: ComprehensionForTree"),
        }
    }
    // port: ParseTree#asComprehension
    pub fn as_comprehension(&self) -> &ComprehensionTree {
        match &self.data {
            ParseTreeData::ComprehensionTree(value) => value,
            _ => panic!("ClassCastException: ComprehensionTree"),
        }
    }
    // port: ParseTree#asComputedPropertyDefinition
    pub fn as_computed_property_definition(&self) -> &ComputedPropertyDefinitionTree {
        match &self.data {
            ParseTreeData::ComputedPropertyDefinitionTree(value) => value,
            _ => panic!("ClassCastException: ComputedPropertyDefinitionTree"),
        }
    }
    // port: ParseTree#asComputedPropertyField
    pub fn as_computed_property_field(&self) -> &ComputedPropertyFieldTree {
        match &self.data {
            ParseTreeData::ComputedPropertyFieldTree(value) => value,
            _ => panic!("ClassCastException: ComputedPropertyFieldTree"),
        }
    }
    // port: ParseTree#asComputedPropertyGetter
    pub fn as_computed_property_getter(&self) -> &ComputedPropertyGetterTree {
        match &self.data {
            ParseTreeData::ComputedPropertyGetterTree(value) => value,
            _ => panic!("ClassCastException: ComputedPropertyGetterTree"),
        }
    }
    // port: ParseTree#asComputedPropertyMethod
    pub fn as_computed_property_method(&self) -> &ComputedPropertyMethodTree {
        match &self.data {
            ParseTreeData::ComputedPropertyMethodTree(value) => value,
            _ => panic!("ClassCastException: ComputedPropertyMethodTree"),
        }
    }
    // port: ParseTree#asComputedPropertySetter
    pub fn as_computed_property_setter(&self) -> &ComputedPropertySetterTree {
        match &self.data {
            ParseTreeData::ComputedPropertySetterTree(value) => value,
            _ => panic!("ClassCastException: ComputedPropertySetterTree"),
        }
    }
    // port: ParseTree#asConditionalExpression
    pub fn as_conditional_expression(&self) -> &ConditionalExpressionTree {
        match &self.data {
            ParseTreeData::ConditionalExpressionTree(value) => value,
            _ => panic!("ClassCastException: ConditionalExpressionTree"),
        }
    }
    // port: ParseTree#asContinueStatement
    pub fn as_continue_statement(&self) -> &ContinueStatementTree {
        match &self.data {
            ParseTreeData::ContinueStatementTree(value) => value,
            _ => panic!("ClassCastException: ContinueStatementTree"),
        }
    }
    // port: ParseTree#asDebuggerStatement
    pub fn as_debugger_statement(&self) -> &DebuggerStatementTree {
        match &self.data {
            ParseTreeData::DebuggerStatementTree(value) => value,
            _ => panic!("ClassCastException: DebuggerStatementTree"),
        }
    }
    // port: ParseTree#asDefaultClause
    pub fn as_default_clause(&self) -> &DefaultClauseTree {
        match &self.data {
            ParseTreeData::DefaultClauseTree(value) => value,
            _ => panic!("ClassCastException: DefaultClauseTree"),
        }
    }
    // port: ParseTree#asDefaultParameter
    pub fn as_default_parameter(&self) -> &DefaultParameterTree {
        match &self.data {
            ParseTreeData::DefaultParameterTree(value) => value,
            _ => panic!("ClassCastException: DefaultParameterTree"),
        }
    }
    // port: ParseTree#asDoWhileStatement
    pub fn as_do_while_statement(&self) -> &DoWhileStatementTree {
        match &self.data {
            ParseTreeData::DoWhileStatementTree(value) => value,
            _ => panic!("ClassCastException: DoWhileStatementTree"),
        }
    }
    // port: ParseTree#asEmptyStatement
    pub fn as_empty_statement(&self) -> &EmptyStatementTree {
        match &self.data {
            ParseTreeData::EmptyStatementTree(value) => value,
            _ => panic!("ClassCastException: EmptyStatementTree"),
        }
    }
    // port: ParseTree#asExportDeclaration
    pub fn as_export_declaration(&self) -> &ExportDeclarationTree {
        match &self.data {
            ParseTreeData::ExportDeclarationTree(value) => value,
            _ => panic!("ClassCastException: ExportDeclarationTree"),
        }
    }
    // port: ParseTree#asExportSpecifier
    pub fn as_export_specifier(&self) -> &ExportSpecifierTree {
        match &self.data {
            ParseTreeData::ExportSpecifierTree(value) => value,
            _ => panic!("ClassCastException: ExportSpecifierTree"),
        }
    }
    // port: ParseTree#asExpressionStatement
    pub fn as_expression_statement(&self) -> &ExpressionStatementTree {
        match &self.data {
            ParseTreeData::ExpressionStatementTree(value) => value,
            _ => panic!("ClassCastException: ExpressionStatementTree"),
        }
    }
    // port: ParseTree#asFinally
    pub fn as_finally(&self) -> &FinallyTree {
        match &self.data {
            ParseTreeData::FinallyTree(value) => value,
            _ => panic!("ClassCastException: FinallyTree"),
        }
    }
    // port: ParseTree#asForOfStatement
    pub fn as_for_of_statement(&self) -> &ForOfStatementTree {
        match &self.data {
            ParseTreeData::ForOfStatementTree(value) => value,
            _ => panic!("ClassCastException: ForOfStatementTree"),
        }
    }
    // port: ParseTree#asForInStatement
    pub fn as_for_in_statement(&self) -> &ForInStatementTree {
        match &self.data {
            ParseTreeData::ForInStatementTree(value) => value,
            _ => panic!("ClassCastException: ForInStatementTree"),
        }
    }
    // port: ParseTree#asFormalParameterList
    pub fn as_formal_parameter_list(&self) -> &FormalParameterListTree {
        match &self.data {
            ParseTreeData::FormalParameterListTree(value) => value,
            _ => panic!("ClassCastException: FormalParameterListTree"),
        }
    }
    // port: ParseTree#asForStatement
    pub fn as_for_statement(&self) -> &ForStatementTree {
        match &self.data {
            ParseTreeData::ForStatementTree(value) => value,
            _ => panic!("ClassCastException: ForStatementTree"),
        }
    }
    // port: ParseTree#asFunctionDeclaration
    pub fn as_function_declaration(&self) -> &FunctionDeclarationTree {
        match &self.data {
            ParseTreeData::FunctionDeclarationTree(value) => value,
            _ => panic!("ClassCastException: FunctionDeclarationTree"),
        }
    }
    // port: ParseTree#asGetAccessor
    pub fn as_get_accessor(&self) -> &GetAccessorTree {
        match &self.data {
            ParseTreeData::GetAccessorTree(value) => value,
            _ => panic!("ClassCastException: GetAccessorTree"),
        }
    }
    // port: ParseTree#asIdentifierExpression
    pub fn as_identifier_expression(&self) -> &IdentifierExpressionTree {
        match &self.data {
            ParseTreeData::IdentifierExpressionTree(value) => value,
            _ => panic!("ClassCastException: IdentifierExpressionTree"),
        }
    }
    // port: ParseTree#asIfStatement
    pub fn as_if_statement(&self) -> &IfStatementTree {
        match &self.data {
            ParseTreeData::IfStatementTree(value) => value,
            _ => panic!("ClassCastException: IfStatementTree"),
        }
    }
    // port: ParseTree#asImportDeclaration
    pub fn as_import_declaration(&self) -> &ImportDeclarationTree {
        match &self.data {
            ParseTreeData::ImportDeclarationTree(value) => value,
            _ => panic!("ClassCastException: ImportDeclarationTree"),
        }
    }
    // port: ParseTree#asImportSpecifier
    pub fn as_import_specifier(&self) -> &ImportSpecifierTree {
        match &self.data {
            ParseTreeData::ImportSpecifierTree(value) => value,
            _ => panic!("ClassCastException: ImportSpecifierTree"),
        }
    }
    // port: ParseTree#asDynamicImportExpression
    pub fn as_dynamic_import_expression(&self) -> &DynamicImportTree {
        match &self.data {
            ParseTreeData::DynamicImportTree(value) => value,
            _ => panic!("ClassCastException: DynamicImportTree"),
        }
    }
    // port: ParseTree#asImportMetaExpression
    pub fn as_import_meta_expression(&self) -> &ImportMetaExpressionTree {
        match &self.data {
            ParseTreeData::ImportMetaExpressionTree(value) => value,
            _ => panic!("ClassCastException: ImportMetaExpressionTree"),
        }
    }
    // port: ParseTree#asLabelledStatement
    pub fn as_labelled_statement(&self) -> &LabelledStatementTree {
        match &self.data {
            ParseTreeData::LabelledStatementTree(value) => value,
            _ => panic!("ClassCastException: LabelledStatementTree"),
        }
    }
    // port: ParseTree#asLiteralExpression
    pub fn as_literal_expression(&self) -> &LiteralExpressionTree {
        match &self.data {
            ParseTreeData::LiteralExpressionTree(value) => value,
            _ => panic!("ClassCastException: LiteralExpressionTree"),
        }
    }
    // port: ParseTree#asMemberExpression
    pub fn as_member_expression(&self) -> &MemberExpressionTree {
        match &self.data {
            ParseTreeData::MemberExpressionTree(value) => value,
            _ => panic!("ClassCastException: MemberExpressionTree"),
        }
    }
    // port: ParseTree#asOptionalMemberExpression
    pub fn as_optional_member_expression(&self) -> &OptionalMemberExpressionTree {
        match &self.data {
            ParseTreeData::OptionalMemberExpressionTree(value) => value,
            _ => panic!("ClassCastException: OptionalMemberExpressionTree"),
        }
    }
    // port: ParseTree#asMemberLookupExpression
    pub fn as_member_lookup_expression(&self) -> &MemberLookupExpressionTree {
        match &self.data {
            ParseTreeData::MemberLookupExpressionTree(value) => value,
            _ => panic!("ClassCastException: MemberLookupExpressionTree"),
        }
    }
    // port: ParseTree#asOptionalMemberLookupExpression
    pub fn as_optional_member_lookup_expression(&self) -> &OptionalMemberLookupExpressionTree {
        match &self.data {
            ParseTreeData::OptionalMemberLookupExpressionTree(value) => value,
            _ => panic!("ClassCastException: OptionalMemberLookupExpressionTree"),
        }
    }
    // port: ParseTree#asMissingPrimaryExpression
    pub fn as_missing_primary_expression(&self) -> &MissingPrimaryExpressionTree {
        match &self.data {
            ParseTreeData::MissingPrimaryExpressionTree(value) => value,
            _ => panic!("ClassCastException: MissingPrimaryExpressionTree"),
        }
    }
    // port: ParseTree#asNewExpression
    pub fn as_new_expression(&self) -> &NewExpressionTree {
        match &self.data {
            ParseTreeData::NewExpressionTree(value) => value,
            _ => panic!("ClassCastException: NewExpressionTree"),
        }
    }
    // port: ParseTree#asNull
    pub fn as_null(&self) -> &NullTree {
        match &self.data {
            ParseTreeData::NullTree(value) => value,
            _ => panic!("ClassCastException: NullTree"),
        }
    }
    // port: ParseTree#asObjectLiteralExpression
    pub fn as_object_literal_expression(&self) -> &ObjectLiteralExpressionTree {
        match &self.data {
            ParseTreeData::ObjectLiteralExpressionTree(value) => value,
            _ => panic!("ClassCastException: ObjectLiteralExpressionTree"),
        }
    }
    // port: ParseTree#asObjectPattern
    pub fn as_object_pattern(&self) -> &ObjectPatternTree {
        match &self.data {
            ParseTreeData::ObjectPatternTree(value) => value,
            _ => panic!("ClassCastException: ObjectPatternTree"),
        }
    }
    // port: ParseTree#asParenExpression
    pub fn as_paren_expression(&self) -> &ParenExpressionTree {
        match &self.data {
            ParseTreeData::ParenExpressionTree(value) => value,
            _ => panic!("ClassCastException: ParenExpressionTree"),
        }
    }
    // port: ParseTree#asProgram
    pub fn as_program(&self) -> &ProgramTree {
        match &self.data {
            ParseTreeData::ProgramTree(value) => value,
            _ => panic!("ClassCastException: ProgramTree"),
        }
    }
    // port: ParseTree#asPropertyNameAssignment
    pub fn as_property_name_assignment(&self) -> &PropertyNameAssignmentTree {
        match &self.data {
            ParseTreeData::PropertyNameAssignmentTree(value) => value,
            _ => panic!("ClassCastException: PropertyNameAssignmentTree"),
        }
    }
    // port: ParseTree#asIterRest
    pub fn as_iter_rest(&self) -> &IterRestTree {
        match &self.data {
            ParseTreeData::IterRestTree(value) => value,
            _ => panic!("ClassCastException: IterRestTree"),
        }
    }
    // port: ParseTree#asObjectRest
    pub fn as_object_rest(&self) -> &ObjectRestTree {
        match &self.data {
            ParseTreeData::ObjectRestTree(value) => value,
            _ => panic!("ClassCastException: ObjectRestTree"),
        }
    }
    // port: ParseTree#asReturnStatement
    pub fn as_return_statement(&self) -> &ReturnStatementTree {
        match &self.data {
            ParseTreeData::ReturnStatementTree(value) => value,
            _ => panic!("ClassCastException: ReturnStatementTree"),
        }
    }
    // port: ParseTree#asSetAccessor
    pub fn as_set_accessor(&self) -> &SetAccessorTree {
        match &self.data {
            ParseTreeData::SetAccessorTree(value) => value,
            _ => panic!("ClassCastException: SetAccessorTree"),
        }
    }
    // port: ParseTree#asIterSpread
    pub fn as_iter_spread(&self) -> &IterSpreadTree {
        match &self.data {
            ParseTreeData::IterSpreadTree(value) => value,
            _ => panic!("ClassCastException: IterSpreadTree"),
        }
    }
    // port: ParseTree#asObjectSpread
    pub fn as_object_spread(&self) -> &ObjectSpreadTree {
        match &self.data {
            ParseTreeData::ObjectSpreadTree(value) => value,
            _ => panic!("ClassCastException: ObjectSpreadTree"),
        }
    }
    // port: ParseTree#asSuperExpression
    pub fn as_super_expression(&self) -> &SuperExpressionTree {
        match &self.data {
            ParseTreeData::SuperExpressionTree(value) => value,
            _ => panic!("ClassCastException: SuperExpressionTree"),
        }
    }
    // port: ParseTree#asSwitchStatement
    pub fn as_switch_statement(&self) -> &SwitchStatementTree {
        match &self.data {
            ParseTreeData::SwitchStatementTree(value) => value,
            _ => panic!("ClassCastException: SwitchStatementTree"),
        }
    }
    // port: ParseTree#asTemplateLiteralExpression
    pub fn as_template_literal_expression(&self) -> &TemplateLiteralExpressionTree {
        match &self.data {
            ParseTreeData::TemplateLiteralExpressionTree(value) => value,
            _ => panic!("ClassCastException: TemplateLiteralExpressionTree"),
        }
    }
    // port: ParseTree#asTemplateLiteralPortion
    pub fn as_template_literal_portion(&self) -> &TemplateLiteralPortionTree {
        match &self.data {
            ParseTreeData::TemplateLiteralPortionTree(value) => value,
            _ => panic!("ClassCastException: TemplateLiteralPortionTree"),
        }
    }
    // port: ParseTree#asTemplateSubstitution
    pub fn as_template_substitution(&self) -> &TemplateSubstitutionTree {
        match &self.data {
            ParseTreeData::TemplateSubstitutionTree(value) => value,
            _ => panic!("ClassCastException: TemplateSubstitutionTree"),
        }
    }
    // port: ParseTree#asThisExpression
    pub fn as_this_expression(&self) -> &ThisExpressionTree {
        match &self.data {
            ParseTreeData::ThisExpressionTree(value) => value,
            _ => panic!("ClassCastException: ThisExpressionTree"),
        }
    }
    // port: ParseTree#asThrowStatement
    pub fn as_throw_statement(&self) -> &ThrowStatementTree {
        match &self.data {
            ParseTreeData::ThrowStatementTree(value) => value,
            _ => panic!("ClassCastException: ThrowStatementTree"),
        }
    }
    // port: ParseTree#asTryStatement
    pub fn as_try_statement(&self) -> &TryStatementTree {
        match &self.data {
            ParseTreeData::TryStatementTree(value) => value,
            _ => panic!("ClassCastException: TryStatementTree"),
        }
    }
    // port: ParseTree#asUnaryExpression
    pub fn as_unary_expression(&self) -> &UnaryExpressionTree {
        match &self.data {
            ParseTreeData::UnaryExpressionTree(value) => value,
            _ => panic!("ClassCastException: UnaryExpressionTree"),
        }
    }
    // port: ParseTree#asVariableDeclarationList
    pub fn as_variable_declaration_list(&self) -> &VariableDeclarationListTree {
        match &self.data {
            ParseTreeData::VariableDeclarationListTree(value) => value,
            _ => panic!("ClassCastException: VariableDeclarationListTree"),
        }
    }
    // port: ParseTree#asVariableDeclaration
    pub fn as_variable_declaration(&self) -> &VariableDeclarationTree {
        match &self.data {
            ParseTreeData::VariableDeclarationTree(value) => value,
            _ => panic!("ClassCastException: VariableDeclarationTree"),
        }
    }
    // port: ParseTree#asVariableStatement
    pub fn as_variable_statement(&self) -> &VariableStatementTree {
        match &self.data {
            ParseTreeData::VariableStatementTree(value) => value,
            _ => panic!("ClassCastException: VariableStatementTree"),
        }
    }
    // port: ParseTree#asWhileStatement
    pub fn as_while_statement(&self) -> &WhileStatementTree {
        match &self.data {
            ParseTreeData::WhileStatementTree(value) => value,
            _ => panic!("ClassCastException: WhileStatementTree"),
        }
    }
    // port: ParseTree#asWithStatement
    pub fn as_with_statement(&self) -> &WithStatementTree {
        match &self.data {
            ParseTreeData::WithStatementTree(value) => value,
            _ => panic!("ClassCastException: WithStatementTree"),
        }
    }
    // port: ParseTree#asYieldStatement
    pub fn as_yield_statement(&self) -> &YieldExpressionTree {
        match &self.data {
            ParseTreeData::YieldExpressionTree(value) => value,
            _ => panic!("ClassCastException: YieldExpressionTree"),
        }
    }
    // port: ParseTree#asAwaitExpression
    pub fn as_await_expression(&self) -> &AwaitExpressionTree {
        match &self.data {
            ParseTreeData::AwaitExpressionTree(value) => value,
            _ => panic!("ClassCastException: AwaitExpressionTree"),
        }
    }
    // port: ParseTree#asNewTargetExpression
    pub fn as_new_target_expression(&self) -> &NewTargetExpressionTree {
        match &self.data {
            ParseTreeData::NewTargetExpressionTree(value) => value,
            _ => panic!("ClassCastException: NewTargetExpressionTree"),
        }
    }
    // port: ParseTree#asUpdateExpression
    pub fn as_update_expression(&self) -> &UpdateExpressionTree {
        match &self.data {
            ParseTreeData::UpdateExpressionTree(value) => value,
            _ => panic!("ClassCastException: UpdateExpressionTree"),
        }
    }
    // port: ParseTree#asForAwaitOfStatement
    pub fn as_for_await_of_statement(&self) -> &ForAwaitOfStatementTree {
        match &self.data {
            ParseTreeData::ForAwaitOfStatementTree(value) => value,
            _ => panic!("ClassCastException: ForAwaitOfStatementTree"),
        }
    }
    // port: ParseTree#asFieldDeclaration
    pub fn as_field_declaration(&self) -> &FieldDeclarationTree {
        match &self.data {
            ParseTreeData::FieldDeclarationTree(value) => value,
            _ => panic!("ClassCastException: FieldDeclarationTree"),
        }
    }
    // port: ParseTree#isPattern
    pub fn is_pattern(&self) -> bool {
        let mut parse_tree = self;
        while parse_tree.type_ == ParseTreeType::PAREN_EXPRESSION {
            parse_tree = &parse_tree.as_paren_expression().expression;
        }
        matches!(
            parse_tree.type_,
            ParseTreeType::ARRAY_PATTERN | ParseTreeType::OBJECT_PATTERN
        )
    }
    // Is valid assignment target for non-vanilla assignment operators like `+=`, `-+`, `**=`, etc
    // port: ParseTree#isValidNonVanillaAssignmentTarget
    pub fn is_valid_non_vanilla_assignment_target(&self) -> bool {
        let mut parse_tree = self;
        while parse_tree.type_ == ParseTreeType::PAREN_EXPRESSION {
            parse_tree = &parse_tree.as_paren_expression().expression;
        }
        matches!(
            parse_tree.type_,
            ParseTreeType::IDENTIFIER_EXPRESSION
                | ParseTreeType::MEMBER_EXPRESSION
                | ParseTreeType::MEMBER_LOOKUP_EXPRESSION
                | ParseTreeType::DEFAULT_PARAMETER
        )
    }
    // Is valid assignment target for any assignment operator like `=`, `+=`, `-+`, `**=`, etc
    // port: ParseTree#isValidAssignmentTarget
    pub fn is_valid_assignment_target(&self) -> bool {
        let mut parse_tree = self;
        while parse_tree.type_ == ParseTreeType::PAREN_EXPRESSION {
            parse_tree = &parse_tree.as_paren_expression().expression;
        }
        matches!(
            parse_tree.type_,
            ParseTreeType::IDENTIFIER_EXPRESSION
                | ParseTreeType::MEMBER_EXPRESSION
                | ParseTreeType::MEMBER_LOOKUP_EXPRESSION
                | ParseTreeType::ARRAY_PATTERN
                | ParseTreeType::OBJECT_PATTERN
                | ParseTreeType::DEFAULT_PARAMETER
        )
    }
    // port: ParseTree#isRestParameter
    pub fn is_rest_parameter(&self) -> bool {
        self.type_ == ParseTreeType::ITER_REST
    }
}

impl std::fmt::Display for ParseTree {
    // port: ParseTree#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}@{}", self.type_, self.location)
    }
}
