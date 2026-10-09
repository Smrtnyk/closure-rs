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
 *   Roger Lawrence
 *   Mike McCabe
 *   Igor Bukanov
 *   Milen Nankov
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
//   src/com/google/javascript/rhino/Token.java.

use std::fmt;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u16)]
pub enum Token {
    RETURN,
    BITOR,
    BITXOR,
    BITAND,
    EQ,
    NE,
    LT,
    LE,
    GT,
    GE,
    LSH,
    RSH,
    URSH,
    ADD,
    SUB,
    MUL,
    DIV,
    MOD,
    EXPONENT,
    NOT,
    BITNOT,
    POS,
    NEG,
    NEW,
    DELPROP,
    TYPEOF,
    GETPROP,
    GETELEM,
    CALL,

    OPTCHAIN_GETPROP,
    OPTCHAIN_GETELEM,
    OPTCHAIN_CALL,

    NAME,
    NUMBER,
    BIGINT,
    STRINGLIT,
    NULL,
    THIS,
    FALSE,
    TRUE,
    SHEQ,
    SHNE,
    REGEXP,
    THROW,
    IN,
    INSTANCEOF,
    ARRAYLIT,
    OBJECTLIT,

    TRY,
    PARAM_LIST,
    COMMA,

    ASSIGN,
    ASSIGN_BITOR,
    ASSIGN_BITXOR,
    ASSIGN_BITAND,
    ASSIGN_LSH,
    ASSIGN_RSH,
    ASSIGN_URSH,
    ASSIGN_ADD,
    ASSIGN_SUB,
    ASSIGN_MUL,
    ASSIGN_DIV,
    ASSIGN_MOD,
    ASSIGN_EXPONENT,

    ASSIGN_OR,
    ASSIGN_AND,
    ASSIGN_COALESCE,

    HOOK,
    OR,
    AND,
    COALESCE,
    INC,
    DEC,
    FUNCTION,
    IF,
    SWITCH,
    CASE,
    DEFAULT_CASE,
    WHILE,
    DO,
    FOR,
    FOR_IN,
    BREAK,
    CONTINUE,
    VAR,
    WITH,
    CATCH,
    VOID,

    EMPTY,

    ROOT,
    BLOCK,
    SWITCH_BODY,
    LABEL,
    EXPR_RESULT,
    SCRIPT,

    GETTER_DEF,
    SETTER_DEF,

    CONST,
    DEBUGGER,

    LABEL_NAME,
    STRING_KEY,
    CAST,

    ARRAY_PATTERN,
    OBJECT_PATTERN,
    DESTRUCTURING_LHS,

    CLASS,
    CLASS_MEMBERS,
    MEMBER_FUNCTION_DEF,
    MEMBER_FIELD_DEF,
    COMPUTED_FIELD_DEF,
    SUPER,

    LET,

    FOR_OF,
    FOR_AWAIT_OF,

    YIELD,

    AWAIT,

    IMPORT,
    IMPORT_SPECS,
    IMPORT_SPEC,
    IMPORT_STAR,
    EXPORT,
    EXPORT_SPECS,
    EXPORT_SPEC,
    MODULE_BODY,
    DYNAMIC_IMPORT,

    ITER_REST,
    OBJECT_REST,
    ITER_SPREAD,
    OBJECT_SPREAD,

    COMPUTED_PROP,

    TAGGED_TEMPLATELIT,
    TEMPLATELIT,
    TEMPLATELIT_SUB,
    TEMPLATELIT_STRING,

    DEFAULT_VALUE,
    NEW_TARGET,
    IMPORT_META,

    STRING_TYPE,
    BOOLEAN_TYPE,
    NUMBER_TYPE,
    FUNCTION_TYPE,
    PARAMETERIZED_TYPE,
    UNION_TYPE,
    ANY_TYPE,
    NULLABLE_TYPE,
    VOID_TYPE,
    REST_PARAMETER_TYPE,
    NAMED_TYPE,
    OPTIONAL_PARAMETER,
    RECORD_TYPE,
    UNDEFINED_TYPE,
    ARRAY_TYPE,
    GENERIC_TYPE,
    GENERIC_TYPE_LIST,

    ANNOTATION,
    PIPE,
    STAR,
    EOC,
    QMARK,
    BANG,
    EQUALS,
    LB,
    LC,
    COLON,

    INTERFACE,
    INTERFACE_EXTENDS,
    INTERFACE_MEMBERS,
    ENUM,
    ENUM_MEMBERS,
    IMPLEMENTS,
    TYPE_ALIAS,
    DECLARE,
    MEMBER_VARIABLE_DEF,
    INDEX_SIGNATURE,
    CALL_SIGNATURE,
    NAMESPACE,
    NAMESPACE_ELEMENTS,

    PLACEHOLDER1,
    PLACEHOLDER2,
    PLACEHOLDER3,
}
impl Token {
    // port: Token#arity
    pub fn arity(token: Token) -> i32 {
        use Token::*;
        match token {
            ANNOTATION | ARRAYLIT | BANG | BLOCK | ROOT | BREAK | CALL | OPTCHAIN_CALL | COLON
            | CONST | CONTINUE | DEBUGGER | EOC | EQUALS | FOR | IF | LB | LC | NEW | OBJECTLIT
            | PARAM_LIST | PIPE | QMARK | REGEXP | RETURN | SCRIPT | STAR | STRING_KEY
            | SWITCH_BODY | TEMPLATELIT | TRY | VAR | YIELD | MEMBER_FIELD_DEF
            | COMPUTED_FIELD_DEF => -1,
            EMPTY | FALSE | IMPORT_STAR | LABEL_NAME | MEMBER_VARIABLE_DEF | NAME | NULL
            | NUMBER | BIGINT | STRINGLIT | TEMPLATELIT_STRING | THIS | TRUE => 0,
            AWAIT | BITNOT | CALL_SIGNATURE | CAST | DEC | DEFAULT_CASE | DELPROP | EXPR_RESULT
            | GETPROP | GETTER_DEF | INC | INDEX_SIGNATURE | ITER_REST | ITER_SPREAD
            | MEMBER_FUNCTION_DEF | NAMED_TYPE | NEG | NOT | OBJECT_REST | OBJECT_SPREAD
            | OPTCHAIN_GETPROP | POS | SETTER_DEF | TEMPLATELIT_SUB | THROW | TYPEOF
            | TYPE_ALIAS | VOID => 1,
            ADD | AND | ASSIGN | ASSIGN_ADD | ASSIGN_BITAND | ASSIGN_BITOR | ASSIGN_BITXOR
            | ASSIGN_DIV | ASSIGN_LSH | ASSIGN_MOD | ASSIGN_MUL | ASSIGN_EXPONENT | ASSIGN_RSH
            | ASSIGN_SUB | ASSIGN_URSH | ASSIGN_OR | ASSIGN_AND | ASSIGN_COALESCE | BITAND
            | BITOR | BITXOR | CASE | COALESCE | CATCH | COMMA | COMPUTED_PROP | DEFAULT_VALUE
            | DIV | DO | ENUM | EQ | EXPONENT | GE | GETELEM | OPTCHAIN_GETELEM | GT | IN
            | INSTANCEOF | LABEL | LE | LSH | LT | MOD | MUL | NAMESPACE | NE | OR | RSH
            | SWITCH | SHEQ | SHNE | SUB | TAGGED_TEMPLATELIT | URSH | WHILE | WITH => 2,
            CLASS | FOR_IN | FOR_OF | FOR_AWAIT_OF | FUNCTION | HOOK | IMPORT | INTERFACE => 3,
            _ => panic!("No arity defined for {token}"),
        }
    }
}
impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
