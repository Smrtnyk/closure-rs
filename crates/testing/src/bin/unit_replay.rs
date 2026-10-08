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
// Ported from closure-rs' own Java oracle tooling:
//   oracle/replay/src/com/google/javascript/jscomp/ReplayMain.java.

//! Native unit corpus replay CLI.
#![forbid(unsafe_code)]
use closure_testing::{
    replay::replay_main::{Runner, default_corpus, write_report},
    throwable::Throwable,
};
use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};
// port: ReplayMain#main
fn run() -> Result<bool, Throwable> {
    let mut corpus = default_corpus();
    let mut classes = None;
    let mut report_path = None;
    let mut records_path = None;
    let mut all = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--all" => all = true,
            "--classes" => {
                classes = Some(
                    next_arg(&mut args, &arg)?
                        .split(',')
                        .map(str::trim)
                        .filter(|c| !c.is_empty())
                        .map(String::from)
                        .collect(),
                )
            }
            "--report" => report_path = Some(PathBuf::from(next_arg(&mut args, &arg)?)),
            "--records-out" => records_path = Some(PathBuf::from(next_arg(&mut args, &arg)?)),
            "--corpus" => corpus = PathBuf::from(next_arg(&mut args, &arg)?),
            "--help" | "-h" => {
                println!(
                    "unit_replay [--all | --classes A,B,...] [--report FILE.json] [--records-out FILE.jsonl] [--corpus DIR]"
                );
                return Ok(false);
            }
            _ => return Err(Throwable::HarnessError(format!("unknown arg {arg}"))),
        }
    }
    if all && classes.is_some() {
        return Err(Throwable::HarnessError(
            "--all and --classes are mutually exclusive".into(),
        ));
    }
    let mut records = records_path
        .map(|p| std::fs::File::create(p).map(BufWriter::new))
        .transpose()
        .map_err(|e| Throwable::HarnessError(e.to_string()))?;
    let report = Runner {
        corpus,
        classes,
        sample: None,
    }
    .run(
        |record| {
            if let Some(out) = &mut records {
                writeln!(out, "{}", record.to_json().to_json_string())
                    .map_err(|e| Throwable::HarnessError(e.to_string()))?;
            }
            Ok(())
        },
        |class, c| {
            println!(
                "REPLAY {class} records={} pass={} fail={} unported={}",
                c.records, c.pass, c.fail, c.unported
            )
        },
    )?;
    if let Some(out) = &mut records {
        out.flush()
            .map_err(|e| Throwable::HarnessError(e.to_string()))?;
    }
    if let Some(path) = report_path {
        write_report(&path, &report)?;
    }
    let c = report.totals;
    println!(
        "REPLAY_TOTAL records={} pass={} fail={} unported={}",
        c.records, c.pass, c.fail, c.unported
    );
    Ok(c.fail > 0)
}
// port: ReplayMain#main (argument value)
fn next_arg(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, Throwable> {
    args.next()
        .ok_or_else(|| Throwable::HarnessError(format!("{flag} needs a value")))
}
// port: ReplayMain#main
fn main() {
    match run() {
        Ok(failed) => std::process::exit(i32::from(failed)),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
