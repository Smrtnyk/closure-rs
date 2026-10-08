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

//! Double to text conversions in the pinned Java sources (paths relative to `src/com/google/javascript`).
//!
//! `D` below means [`d_to_a::number_to_string`]; `J` means [`crate::java_lang::double_to_string`] (Java `Double.toString`).
//! Preserve the surrounding Java replacements, casts, and printing decisions.
//! D callers receive `Result<Vec<u16>, DToAError>`: preserve the Java UTF-16 code units
//! (including lone surrogates) and propagate Java exceptions as Results.
//! J always returns an ASCII `String`.
//!
//! | File:line | Java expression | Rust conversion |
//! | --- | --- | --- |
//! | jscomp/NodeUtil.java:232 | `DToA.numberToString(value.doubleValue())` | D |
//! | jscomp/ClosureOptimizePrimitives.java:187,201 | `numberToString(curParam/keyNode.getDouble())` | D |
//! | jscomp/parsing/IRFactory.java:1455 | `DToA.numberToString(normalizeNumber(literal))` | D |
//! | jscomp/CodeConsumer.java:337 | `String.valueOf(x)` | J, then `.replace(".0E", "E")` and leading `0.` removal |
//! | jscomp/CodeGenerator.java:409,668,1820 | `cc.addNumber(node.getDouble()/d, node/n)` | delegate to CodeConsumer's decisions |
//! | jscomp/PeepholeFoldConstants.java:915-916 | `String.valueOf(result/lval/rval)` | J (including length checks) |
//! | jscomp/PeepholeReplaceKnownMethods.java:551 | `String.valueOf(newVal)` | J, then `normalizeNumericString` |
//! | jscomp/lint/CheckEnums.java:149 | `Double.toString(valueNode.getDouble())` | J |
//! | rhino/Node.java:1503,1578 | `sb.append(getDouble())`, `String.valueOf(getDouble())` | J |
//! | jscomp/ant/AntErrorManager.java:55 | `", " + getTypedPercent() + " typed"` | J |
//! | jscomp/CodeConsumer.java:351 | `mantissa + "E" + exp` | `i64::to_string`, `i32::to_string` |
//! | jscomp/CodeConsumer.java:356 | `Long.toString(value)` | `value.to_string()` on i64 |
//! | jscomp/CodeConsumer.java:363 | `Long.toHexString(value)` | `format!("{value:x}")` on i64 (two's complement) |
//! | jscomp/CodeConsumer.java:372-373 | `bi.toString(16)`, `bi + "n"` | `bi.to_str_radix(16/10)` |
//! | jscomp/CodeGenerator.java:413 | `cc.addBigInt(node.getBigInt())` | delegate to CodeConsumer |
//! | jscomp/NodeUtil.java:236 | `n.getBigInt().toString()` | `to_str_radix(10)` |
//! | jscomp/serialization/TypedAstSerializer.java:245 | `n.getBigInt().toString()` | `to_str_radix(10)` |
//! | jscomp/ProcessCommonJSModules.java:220 | `String.valueOf((int) pathArgument.getDouble())` | `(v as i32).to_string()` (Java saturating cast) |
//! | jscomp/JsMessageVisitor.java:498 | `Ascii.toUpperCase(Long.toString(nonnegativeHash, 36))` | `BigInt::from(v).to_str_radix(36).to_ascii_uppercase()` |
//! | jscomp/GoogleJsMessageIdGenerator.java:72-73 | `String.valueOf(MessageId.generateId(...))` | `i64::to_string` |
//! | jscomp/colors/ColorId.java:160 | `Long.toHexString(this.rightAligned)` | `format!("{v:x}")` on i64 |
//! | rhino/JSDocInfo.java:1304 | `Long.toHexString(propertyBits)` | `format!("{v:x}")` on i64 |
//!
//! Additional output paths use Java Formatter's **precision rounding**, not its shortest-string
//! conversion: jscomp/{PrintStreamErrorManager.java:71-73,
//! PrintStreamErrorReportGenerator.java:63-65, LoggerErrorManager.java:62-64,
//! JsonErrorReportGenerator.java:140-142}, `printf/String.format("... %.1f%% typed", ..., getTypedPercent())`.
//! Their ports need the Java Formatter/FormattedFPDecimal rounding path (outside this `toString`
//! port); neither J nor Rust f64 formatting substitutes for it. No BigDecimal conversion occurs in
//! jscomp/rhino. Generic boxed-Double output (`String.valueOf(Object)` at rhino/Node.java:493,
//! rhino/Msg.java:179; `%s`/precondition messages, including jscomp/CodeConsumer.java:334 and
//! rhino/IR.java:763) also delegates to J when the object is a Double.

pub mod d_to_a;
