/*
 * Copyright 2004 The Closure Compiler Authors.
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
// Ported from Closure Compiler (https://github.com/google/closure-compiler), commit bb8c8e7:
//   src/com/google/javascript/jscomp/AbstractCommandLineRunner.java,
//   src/com/google/javascript/jscomp/CommandLineRunner.java,
//   src/com/google/javascript/jscomp/Compiler.java,
//   src/com/google/javascript/jscomp/CompilerOptions.java,
//   src/com/google/javascript/jscomp/parsing/Config.java.

// tool-only: parse_dump depends only on closure-parsing, not on the jscomp CompilerOptions,
// Compiler and CommandLineRunner
use closure_parsing::{
    config::{Config, JsDocParsing, LanguageMode, RunMode, StrictMode},
    parser_runner::ParserRunner,
};
use closure_rhino::fx_hash::IndexSet;
use closure_rhino::js_string::JsString;
use serde_json::Value;

pub struct Options {
    pub language_in: String,
    pub config: Config,
}
// port: CommandLineRunner#processArgs
fn process_args(args: &[String]) -> Vec<String> {
    let mut processed_args = Vec::new();
    for arg in args {
        let assignment = arg.split_once('=').filter(|(flag, _)| {
            let name = flag.strip_prefix("--").or_else(|| flag.strip_prefix('-'));
            name.is_some_and(|name| {
                !name.is_empty()
                    && name
                        .bytes()
                        .all(|ch| ch.is_ascii_alphabetic() || ch == b'_')
            })
        });
        if let Some((flag, value)) = assignment {
            processed_args.push(flag.to_owned());
            let quoted = value.len() >= 2
                && matches!(value.as_bytes()[0], b'\'' | b'"')
                && matches!(value.as_bytes()[value.len() - 1], b'\'' | b'"')
                && !value.contains(['\n', '\r', '\u{0085}', '\u{2028}', '\u{2029}']);
            processed_args.push(if quoted {
                value[1..value.len() - 1].to_owned()
            } else {
                value.to_owned()
            });
        } else {
            processed_args.push(arg.clone());
        }
    }
    processed_args
}
// port: CommandLineRunner.BooleanOptionHandler#parseArguments
fn boolean_argument(value: Option<&String>) -> (bool, usize) {
    match value.map(|value| value.to_ascii_lowercase()).as_deref() {
        Some("true" | "on" | "yes" | "1") => (true, 1),
        Some("false" | "off" | "no" | "0") => (false, 1),
        _ => (true, 0),
    }
}
// port: CompilerOptions.LanguageMode#fromString
fn language_mode(value: &str) -> Option<String> {
    let value = value
        .trim_matches(|c: char| c <= '\u{0020}')
        .to_ascii_uppercase();
    let value = if let Some(suffix) = value.strip_prefix("ES") {
        format!("ECMASCRIPT{suffix}")
    } else {
        value
    };
    let value = match value.as_str() {
        "ECMASCRIPT6" | "ECMASCRIPT6_STRICT" => "ECMASCRIPT_2015".to_owned(),
        _ => value,
    };
    matches!(
        value.as_str(),
        "ECMASCRIPT3"
            | "ECMASCRIPT5"
            | "ECMASCRIPT5_STRICT"
            | "ECMASCRIPT_2015"
            | "ECMASCRIPT_2016"
            | "ECMASCRIPT_2017"
            | "ECMASCRIPT_2018"
            | "ECMASCRIPT_2019"
            | "ECMASCRIPT_2020"
            | "ECMASCRIPT_2021"
            | "ECMASCRIPT_2022"
            | "ECMASCRIPT_NEXT"
            | "STABLE"
            | "UNSTABLE"
            | "UNSUPPORTED"
            | "NO_TRANSPILE"
    )
    .then_some(value)
}
// port: CompilerOptions.LanguageMode#validCommandLineNames
fn valid_command_line_names() -> String {
    let mut names = Vec::new();
    for name in [
        "ECMASCRIPT3",
        "ECMASCRIPT5",
        "ECMASCRIPT5_STRICT",
        "ECMASCRIPT_2015",
        "ECMASCRIPT_2016",
        "ECMASCRIPT_2017",
        "ECMASCRIPT_2018",
        "ECMASCRIPT_2019",
        "ECMASCRIPT_2020",
        "ECMASCRIPT_2021",
        "ECMASCRIPT_2022",
        "ECMASCRIPT_NEXT",
        "STABLE",
        "NO_TRANSPILE",
        "UNSTABLE",
    ] {
        names.push(name.to_owned());
        if let Some(suffix) = name.strip_prefix("ECMASCRIPT") {
            names.push(format!("ES{suffix}"));
        }
    }
    names.extend([
        "ECMASCRIPT6".into(),
        "ES6".into(),
        "ECMASCRIPT6_STRICT".into(),
        "ES6_STRICT".into(),
    ]);
    format!("[{}]", names.join(", "))
}
// port: Compiler#getParserConfigLanguageMode
fn config_language_mode(language: &str) -> Result<LanguageMode, String> {
    Ok(match language {
        "ECMASCRIPT3" => LanguageMode::ECMASCRIPT3,
        "ECMASCRIPT5" | "ECMASCRIPT5_STRICT" => LanguageMode::ECMASCRIPT5,
        "ECMASCRIPT_2015" => LanguageMode::ECMASCRIPT_2015,
        "ECMASCRIPT_2016" => LanguageMode::ECMASCRIPT_2016,
        "ECMASCRIPT_2017" => LanguageMode::ECMASCRIPT_2017,
        "ECMASCRIPT_2018" => LanguageMode::ECMASCRIPT_2018,
        "ECMASCRIPT_2019" => LanguageMode::ECMASCRIPT_2019,
        "ECMASCRIPT_2020" => LanguageMode::ECMASCRIPT_2020,
        "ECMASCRIPT_2021" => LanguageMode::ECMASCRIPT_2021,
        "ECMASCRIPT_NEXT" => LanguageMode::ES_NEXT,
        "UNSTABLE" => LanguageMode::UNSTABLE,
        "UNSUPPORTED" => LanguageMode::UNSUPPORTED,
        _ => {
            return Err(format!(
                "java.lang.IllegalStateException: Unexpected language mode: {language}"
            ));
        }
    })
}
// port: Config.JsDocParsing#valueOf
fn jsdoc_mode(value: &str) -> Result<JsDocParsing, String> {
    Ok(match value {
        "TYPES_ONLY" => JsDocParsing::TYPES_ONLY,
        "INCLUDE_DESCRIPTIONS_NO_WHITESPACE" => JsDocParsing::INCLUDE_DESCRIPTIONS_NO_WHITESPACE,
        "INCLUDE_DESCRIPTIONS_WITH_WHITESPACE" => {
            JsDocParsing::INCLUDE_DESCRIPTIONS_WITH_WHITESPACE
        }
        "INCLUDE_ALL_COMMENTS" => JsDocParsing::INCLUDE_ALL_COMMENTS,
        "LICENSE_COMMENTS_ONLY" => JsDocParsing::LICENSE_COMMENTS_ONLY,
        _ => {
            return Err(format!(
                "java.lang.IllegalArgumentException: No enum constant com.google.javascript.jscomp.parsing.Config.JsDocParsing.{value}"
            ));
        }
    })
}

impl Options {
    // port: CommandLineRunner#createOptions
    // port: AbstractCommandLineRunner#setRunOptions
    // port: Compiler#getParserConfig
    // port: Compiler#createConfig
    pub fn from_request(req: &Value) -> Result<Self, String> {
        let mut args = Vec::new();
        if let Some(language) = req.get("language_in") {
            args.push(format!(
                "--language_in={}",
                language.as_str().ok_or("invalid language_in")?
            ));
        }
        if let Some(argv) = req.get("args") {
            for arg in argv.as_array().ok_or("invalid args")? {
                args.push(arg.as_str().ok_or("invalid argument")?.to_owned());
            }
        }
        let mut language = String::new(); // CommandLineRunner.Flags.languageIn
        let mut strict = true; // CommandLineRunner.Flags.strictModeInput
        let mut print_tree = false;
        let mut parse_inline_source_maps = true;
        let mut keep_going = false;
        let mut annotations = IndexSet::<_>::default();
        let mut i = 0;
        let processed_args = process_args(&args);
        while i < processed_args.len() {
            let flag = processed_args[i].as_str();
            match flag {
                "--language_in" | "--extra_annotation_name" => {
                    i += 1;
                    let value = processed_args
                        .get(i)
                        .filter(|value| !value.starts_with('-'))
                        .ok_or_else(|| {
                            format!(
                                "java.lang.IllegalArgumentException: bad flags: [{}]",
                                args.join(", ")
                            )
                        })?;
                    if flag == "--language_in" {
                        language = value.clone();
                    } else {
                        annotations.insert(JsString::from(value.as_str()));
                    }
                }
                "--strict_mode_input"
                | "--print_tree"
                | "--parse_inline_source_maps"
                | "--continue_after_errors" => {
                    let (value, consumed) = boolean_argument(processed_args.get(i + 1));
                    match flag {
                        "--strict_mode_input" => strict = value,
                        "--print_tree" => print_tree = value,
                        "--parse_inline_source_maps" => parse_inline_source_maps = value,
                        "--continue_after_errors" => keep_going = value,
                        _ => unreachable!(),
                    }
                    i += consumed;
                }
                // Flags.arguments accepts ordinary input filenames. ParseDump supplies its own
                // source file, so these arguments have no effect on this parse.
                _ if !flag.starts_with('-') => {}
                _ => {
                    return Err(format!(
                        "java.lang.IllegalArgumentException: bad flags: [{}]",
                        args.join(", ")
                    ));
                }
            }
            i += 1;
        }
        let mut language = if language.is_empty() {
            "ECMASCRIPT_NEXT".to_owned() // CompilerOptions.LanguageMode.STABLE_IN
        } else {
            language_mode(&language).ok_or_else(|| {
                format!(
                    "com.google.javascript.jscomp.AbstractCommandLineRunner$FlagUsageException: Unknown language `{language}' specified. Expected one of: {}",
                    valid_command_line_names()
                )
            })?
        };
        if language == "UNSUPPORTED" {
            return Err("com.google.javascript.jscomp.AbstractCommandLineRunner$FlagUsageException: Cannot specify the unsupported set of features for language_in.".into());
        }
        // port: CompilerOptions#setLanguageIn
        if language == "NO_TRANSPILE" {
            return Err("java.lang.IllegalStateException".into());
        }
        if language == "STABLE" {
            language = "ECMASCRIPT_NEXT".to_owned();
        }
        let mut language_mode = config_language_mode(&language)?;
        if req.get("kind").and_then(Value::as_str) == Some("extern")
            && language_mode == LanguageMode::ECMASCRIPT3
        {
            language_mode = LanguageMode::ECMASCRIPT5;
        }
        let mut jsdoc = if print_tree {
            JsDocParsing::INCLUDE_ALL_COMMENTS
        } else {
            JsDocParsing::TYPES_ONLY
        };
        if let Some(value) = req.get("jsdoc_parsing") {
            jsdoc = jsdoc_mode(value.as_str().ok_or("invalid jsdoc_parsing")?)?;
        }
        let config = ParserRunner::create_config_full(
            language_mode,
            jsdoc,
            if keep_going {
                RunMode::KEEP_GOING
            } else {
                RunMode::STOP_AFTER_ERROR
            },
            Some(&annotations),
            parse_inline_source_maps,
            if strict {
                StrictMode::STRICT
            } else {
                StrictMode::SLOPPY
            },
        );
        Ok(Self {
            language_in: language,
            config,
        })
    }
}
