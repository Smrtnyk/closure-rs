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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit 48f4107:
//   src/com/google/javascript/rhino/Msg.java.

//! Error message constants, in Java declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    BAD_FILEOVERVIEW_VISIBIIITY_ANNOTATION,
    BAD_JSDOC_TAG,
    DUP_VARIABLE_NAME,
    END_ANNOTATION_EXPECTED,
    INVALID_VARIABLE_NAME,
    JSDOC_ALTERNATEMESSAGEID_EXTRA,
    JSDOC_REQUIRE_INLINING,
    JSDOC_ENCOURAGE_INLINING,
    JSDOC_AUTHORMISSING,
    JSDOC_CLOSUREPRIMITIVE_EXTRA,
    JSDOC_CLOSUREPRIMITIVE_INVALID,
    JSDOC_CLOSUREPRIMITIVE_MISSING,
    JSDOC_CLOSURE_UNAWARE_CODE_EXTRA,
    JSDOC_CLOSURE_UNAWARE_CONFIG_INVALID_VALUE,
    JSDOC_CLOSURE_UNAWARE_CODE_INVALID,
    JSDOC_COLLAPSIBLEORBREAKMYCODE,
    JSDOC_CONST,
    JSDOC_CUSTOMELEMENT_EXTRA,
    JSDOC_DEFINE,
    JSDOC_DEPRECATED,
    JSDOC_DESC_EXTRA,
    JSDOC_EXPORT,
    JSDOC_EXTENDS_DUPLICATE,
    JSDOC_EXTERNS,
    JSDOC_EXTRAVERSION,
    JSDOC_EXTRA_VISIBILITY,
    JSDOC_FILEOVERVIEW_EXTRA,
    JSDOC_FINAL,
    JSDOC_FUNCTION_NEWNOTOBJECT,
    JSDOC_FUNCTION_VARARGS,
    JSDOC_IDGEN_BAD,
    JSDOC_IDGEN_DUPLICATE,
    JSDOC_IDGEN_UNKNOWN,
    JSDOC_IMPLEMENTS_DUPLICATE,
    JSDOC_IMPLEMENTS_EXTRAQUALIFIER,
    JSDOC_IMPLICITCAST,
    JSDOC_IMPORT,
    JSDOC_INCOMPAT_TYPE,
    JSDOC_INCOMPAT_INLINING,
    JSDOC_INTERFACE_CONSTRUCTOR,
    JSDOC_LENDS_INCOMPATIBLE,
    JSDOC_LENDS_MISSING,
    JSDOC_LOCALEFILE,
    JSDOC_LOCALEOBJECT,
    JSDOC_LOCALESELECT,
    JSDOC_LOCALEVALUE,
    JSDOC_PROVIDE_GOOG,
    JSDOC_PROVIDE_ALREADY_PROVIDED,
    JSDOC_MEANING_EXTRA,
    JSDOC_MISSING_BRACES,
    JSDOC_MISSING_COLON,
    JSDOC_MISSING_GT,
    JSDOC_MISSING_LC,
    JSDOC_MISSING_LP,
    JSDOC_MISSING_RB,
    JSDOC_MISSING_RC,
    JSDOC_MISSING_RP,
    JSDOC_MISSING_TYPE_DECLARATION,
    JSDOC_MIXINCLASS_EXTRA,
    JSDOC_MIXINFUNCTION_EXTRA,
    JSDOC_MODIFIES,
    JSDOC_MODIFIES_DUPLICATE,
    JSDOC_MODIFIES_UNKNOWN,
    JSDOC_MODS,
    JSDOC_MODS_EXTRA,
    JSDOC_NAME_SYNTAX,
    JSDOC_NGINJECT_EXTRA,
    JSDOC_NOCOLLAPSE,
    JSDOC_NOCOMPILE,
    JSDOC_NOCOVERAGE,
    JSDOC_NODTS,
    JSDOC_NOINLINE,
    JSDOC_NOSIDEEFFECTS,
    JSDOC_NOSIDEEFFECTS_WITH_MODIFIES,
    JSDOC_NOSIDEEFFECTS_WITH_THROWS,
    JSDOC_OVERRIDE,
    JSDOC_POLYMERBEHAVIOR_EXTRA,
    JSDOC_POLYMER_EXTRA,
    JSDOC_PUREORBREAKMYCODE,
    JSDOC_RECORD,
    JSDOC_SASS_GENERATED_CSS_TS,
    JSDOC_SEEMISSING,
    JSDOC_SUPPRESS,
    JSDOC_SUPPRESS_UNKNOWN,
    JSDOC_TEMPLATE_BOUNDEDGENERICS_USED,
    JSDOC_TEMPLATE_BOUNDSWITHTTL,
    JSDOC_TEMPLATE_MULTIPLEDECLARATION,
    JSDOC_TEMPLATE_NAME_MISSING,
    JSDOC_TEMPLATE_NAME_REDECLARATION,
    JSDOC_TEMPLATE_TYPETRANSFORMATION_EXPRESSIONMISSING,
    JSDOC_TEMPLATE_TYPETRANSFORMATION_MISSINGDELIMIIER,
    JSDOC_THROWS,
    JSDOC_TYPE,
    JSDOC_TYPESUMMARY,
    JSDOC_TYPETRANSFORMATION_EXTRA_PARAM,
    JSDOC_TYPETRANSFORMATION_INVALID,
    JSDOC_TYPETRANSFORMATION_INVALID_EXPRESSION,
    JSDOC_TYPETRANSFORMATION_INVALID_INSIDE,
    JSDOC_TYPETRANSFORMATION_MISSING_PARAM,
    JSDOC_TYPE_RECORD_DUPLICATE,
    JSDOC_TYPE_SYNTAX,
    JSDOC_USEDVIADOTCONSTRUCTOR,
    JSDOC_UNNECESSARY_BRACES,
    JSDOC_VERSIONMISSING,
    JSDOC_WIZACTION,
    MISSING_VARIABLE_NAME,
    NO_TYPE_NAME,
    UNEXPECTED_EOF,
    JSDOC_WIZCALLBACK,
}
impl Msg {
    // port: Msg#Msg
    pub const fn text(self) -> &'static str {
        match self {
            Self::BAD_FILEOVERVIEW_VISIBIIITY_ANNOTATION => {
                "{0} visibility not allowed in @fileoverview block"
            }
            Self::BAD_JSDOC_TAG => {
                "illegal use of unknown JSDoc tag \"{0}\"; ignoring it. Place another character before the @ to stop JSCompiler from parsing it as an annotation."
            }
            Self::DUP_VARIABLE_NAME => "duplicate variable name \"{0}\"",
            Self::END_ANNOTATION_EXPECTED => "expected end of line or comment.",
            Self::INVALID_VARIABLE_NAME => "invalid param name \"{0}\"",
            Self::JSDOC_ALTERNATEMESSAGEID_EXTRA => "extra @alternateMessageId tag",
            Self::JSDOC_REQUIRE_INLINING => "extra @requireInlining tag",
            Self::JSDOC_ENCOURAGE_INLINING => "extra @encourageInlining tag",
            Self::JSDOC_AUTHORMISSING => "@author tag missing author",
            Self::JSDOC_CLOSUREPRIMITIVE_EXTRA => "conflicting @closurePrimitive tag",
            Self::JSDOC_CLOSUREPRIMITIVE_INVALID => "invalid id in @closurePrimitive tag.",
            Self::JSDOC_CLOSUREPRIMITIVE_MISSING => "missing id in @closurePrimitive tag.",
            Self::JSDOC_CLOSURE_UNAWARE_CODE_EXTRA => "extra @closureUnaware tag",
            Self::JSDOC_CLOSURE_UNAWARE_CONFIG_INVALID_VALUE => {
                "invalid value for @closureUnaware: {0}"
            }
            Self::JSDOC_CLOSURE_UNAWARE_CODE_INVALID => {
                "@closureUnaware annotation is not allowed in this compilation"
            }
            Self::JSDOC_COLLAPSIBLEORBREAKMYCODE => "extra @collapsibleOrBreakMyCode tag",
            Self::JSDOC_CONST => "conflicting @const tag",
            Self::JSDOC_CUSTOMELEMENT_EXTRA => "extra @customElement tag",
            Self::JSDOC_DEFINE => "conflicting @define tag",
            Self::JSDOC_DEPRECATED => "extra @deprecated tag",
            Self::JSDOC_DESC_EXTRA => "extra @desc tag",
            Self::JSDOC_EXPORT => "extra @export tag",
            Self::JSDOC_EXTENDS_DUPLICATE => "duplicate @extends tag",
            Self::JSDOC_EXTERNS => "extra @externs tag",
            Self::JSDOC_EXTRAVERSION => "conflicting @version tag",
            Self::JSDOC_EXTRA_VISIBILITY => "extra visibility tag",
            Self::JSDOC_FILEOVERVIEW_EXTRA => "extra @fileoverview tag",
            Self::JSDOC_FINAL => "extra @final tag.",
            Self::JSDOC_FUNCTION_NEWNOTOBJECT => "constructed type must be an object type",
            Self::JSDOC_FUNCTION_VARARGS => "variable length argument must be last.",
            Self::JSDOC_IDGEN_BAD => "malformed @idGenerator tag",
            Self::JSDOC_IDGEN_DUPLICATE => "extra @idGenerator tag",
            Self::JSDOC_IDGEN_UNKNOWN => "unknown @idGenerator parameter: {0}",
            Self::JSDOC_IMPLEMENTS_DUPLICATE => "duplicate @implements tag.",
            Self::JSDOC_IMPLEMENTS_EXTRAQUALIFIER => {
                "@implements/@extends requires a bare interface/record name without ! or ?."
            }
            Self::JSDOC_IMPLICITCAST => "extra @implicitCast tag.",
            Self::JSDOC_IMPORT => "Import in typedef is not supported.",
            Self::JSDOC_INCOMPAT_TYPE => "type annotation incompatible with other annotations.",
            Self::JSDOC_INCOMPAT_INLINING => {
                "@requireInlining, @encourageInlining, and @noinline are mutually exclusive."
            }
            Self::JSDOC_INTERFACE_CONSTRUCTOR => "cannot be both an interface and a constructor.",
            Self::JSDOC_LENDS_INCOMPATIBLE => "@lends tag incompatible with other annotations.",
            Self::JSDOC_LENDS_MISSING => "missing object name in @lends tag.",
            Self::JSDOC_LOCALEFILE => "extra @localeFile tag",
            Self::JSDOC_LOCALEOBJECT => "extra @localeObject tag",
            Self::JSDOC_LOCALESELECT => "extra @localeSelect tag",
            Self::JSDOC_LOCALEVALUE => "extra @localeValue tag",
            Self::JSDOC_PROVIDE_GOOG => "extra @provideGoog tag",
            Self::JSDOC_PROVIDE_ALREADY_PROVIDED => "extra @provideAlreadyProvided tag",
            Self::JSDOC_MEANING_EXTRA => "extra @meaning tag",
            Self::JSDOC_MISSING_BRACES => "Type annotations should have curly braces.",
            Self::JSDOC_MISSING_COLON => "expecting colon after this",
            Self::JSDOC_MISSING_GT => "missing closing >",
            Self::JSDOC_MISSING_LC => "missing opening {",
            Self::JSDOC_MISSING_LP => "missing opening (",
            Self::JSDOC_MISSING_RB => "missing closing ]",
            Self::JSDOC_MISSING_RC => "expected closing }",
            Self::JSDOC_MISSING_RP => "missing closing )",
            Self::JSDOC_MISSING_TYPE_DECLARATION => "Missing type declaration.",
            Self::JSDOC_MIXINCLASS_EXTRA => "extra @mixinClass tag",
            Self::JSDOC_MIXINFUNCTION_EXTRA => "extra @mixinFunction tag",
            Self::JSDOC_MODIFIES => "malformed @modifies tag",
            Self::JSDOC_MODIFIES_DUPLICATE => "conflicting @modifies tag",
            Self::JSDOC_MODIFIES_UNKNOWN => "unknown @modifies parameter: {0}",
            Self::JSDOC_MODS => "malformed @mods tag",
            Self::JSDOC_MODS_EXTRA => "extra @mods tag",
            Self::JSDOC_NAME_SYNTAX => "name not recognized due to syntax error.",
            Self::JSDOC_NGINJECT_EXTRA => "extra @ngInject tag",
            Self::JSDOC_NOCOLLAPSE => "extra @nocollapse tag",
            Self::JSDOC_NOCOMPILE => "extra @nocompile tag",
            Self::JSDOC_NOCOVERAGE => "extra @nocoverage tag",
            Self::JSDOC_NODTS => "extra @nodts tag",
            Self::JSDOC_NOINLINE => "extra @noinline tag",
            Self::JSDOC_NOSIDEEFFECTS => "conflicting @nosideeffects tag",
            Self::JSDOC_NOSIDEEFFECTS_WITH_MODIFIES => {
                "@nosideeffects functions cannot have @modifies (modifying arguments/this is a side effect)"
            }
            Self::JSDOC_NOSIDEEFFECTS_WITH_THROWS => {
                "@nosideeffects functions cannot have @throws (throwing is a side effect)"
            }
            Self::JSDOC_OVERRIDE => "extra @override/@inheritDoc tag.",
            Self::JSDOC_POLYMERBEHAVIOR_EXTRA => "extra @polymerBehavior tag",
            Self::JSDOC_POLYMER_EXTRA => "extra @polymer tag",
            Self::JSDOC_PUREORBREAKMYCODE => "extra @pureOrBreakMyCode tag",
            Self::JSDOC_RECORD => "conflicting @record tag.",
            Self::JSDOC_SASS_GENERATED_CSS_TS => "extra @sassGeneratedCssTs tag",
            Self::JSDOC_SEEMISSING => "@see tag missing description",
            Self::JSDOC_SUPPRESS => "malformed @suppress tag",
            Self::JSDOC_SUPPRESS_UNKNOWN => "unknown @suppress parameter: {0}",
            Self::JSDOC_TEMPLATE_BOUNDEDGENERICS_USED => {
                "Bounded generic semantics are currently still in development"
            }
            Self::JSDOC_TEMPLATE_BOUNDSWITHTTL => "Template types cannot combine bounds and TTL.",
            Self::JSDOC_TEMPLATE_MULTIPLEDECLARATION => {
                "Multiple template names cannot be declared with bounds or TTL."
            }
            Self::JSDOC_TEMPLATE_NAME_MISSING => "@template tag missing type name.",
            Self::JSDOC_TEMPLATE_NAME_REDECLARATION => {
                "Type name(s) for @template annotation declared twice."
            }
            Self::JSDOC_TEMPLATE_TYPETRANSFORMATION_EXPRESSIONMISSING => {
                "Missing type transformation expression."
            }
            Self::JSDOC_TEMPLATE_TYPETRANSFORMATION_MISSINGDELIMIIER => {
                "Expected end delimiter for a type transformation."
            }
            Self::JSDOC_THROWS => "conflicting @throws tag",
            Self::JSDOC_TYPE => "conflicting @type tag",
            Self::JSDOC_TYPESUMMARY => "extra @typeSummary tag",
            Self::JSDOC_TYPETRANSFORMATION_EXTRA_PARAM => "Found extra parameter in {0}",
            Self::JSDOC_TYPETRANSFORMATION_INVALID => "Invalid {0}",
            Self::JSDOC_TYPETRANSFORMATION_INVALID_EXPRESSION => "Invalid {0} expression",
            Self::JSDOC_TYPETRANSFORMATION_INVALID_INSIDE => "Invalid expression inside {0}",
            Self::JSDOC_TYPETRANSFORMATION_MISSING_PARAM => "Missing parameter in {0}",
            Self::JSDOC_TYPE_RECORD_DUPLICATE => "Duplicate record field {0}.",
            Self::JSDOC_TYPE_SYNTAX => "type not recognized due to syntax error.",
            Self::JSDOC_USEDVIADOTCONSTRUCTOR => "extra @usedViaDotConstructor tag",
            Self::JSDOC_UNNECESSARY_BRACES => "braces are not required here",
            Self::JSDOC_VERSIONMISSING => "@version tag missing version information",
            Self::JSDOC_WIZACTION => "extra @wizaction tag",
            Self::MISSING_VARIABLE_NAME => "expecting a variable name in a @param tag.",
            Self::NO_TYPE_NAME => "expecting a type name.",
            Self::UNEXPECTED_EOF => "Unexpected end of file",
            Self::JSDOC_WIZCALLBACK => "extra @wizcallback tag",
        }
    }
    // port: Msg#format()
    pub fn format(self) -> &'static str {
        self.text()
    }
    /// Arguments are the strings produced by Java String.valueOf (including "null").
    // port: Msg#format(Object...)
    pub fn format_with(self, args: &[String]) -> String {
        let mut s = self.text().to_string();
        for (i, arg) in args.iter().enumerate() {
            let to_replace = format!("{{{i}}}");
            s = s.replace(&to_replace, arg);
        }
        s
    }
    // port: Msg#format(Object...)
    pub fn format_with_js_strings(
        self,
        args: &[crate::js_string::JsString],
    ) -> crate::js_string::JsString {
        let mut s = crate::js_string::JsString::from(self.text());
        for (i, arg) in args.iter().enumerate() {
            let to_replace = format!("{{{i}}}").into();
            s = s.replace(&to_replace, arg);
        }
        s
    }
}
