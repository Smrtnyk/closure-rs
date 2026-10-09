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

//! Locating and loading the corpus files.

use crate::derived::{ExpectedPipeline, OptionsDefaults};
use crate::descriptor::Descriptor;
use crate::json::{JsonValue, parse_json};
use crate::record::Record;
use flate2::read::GzDecoder;
use std::fmt;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

/// A load error: file, 1-based line (for JSONL files) and what failed.
#[derive(Clone, Debug)]
pub struct LoadError {
    pub file: PathBuf,
    pub line: Option<usize>,
    pub message: String,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(l) => write!(f, "{}:{}: {}", self.file.display(), l, self.message),
            None => write!(f, "{}: {}", self.file.display(), self.message),
        }
    }
}

impl std::error::Error for LoadError {}

/// `corpus/unit` of this checkout, or `$CLOSURE_RS_CORPUS_UNIT` when set.
pub fn corpus_unit_dir() -> PathBuf {
    match std::env::var_os("CLOSURE_RS_CORPUS_UNIT") {
        Some(d) => PathBuf::from(d),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/unit"),
    }
}

fn sorted_files(dir: &Path, suffix: &str) -> Result<Vec<PathBuf>, LoadError> {
    let rd = std::fs::read_dir(dir).map_err(|e| LoadError {
        file: dir.to_path_buf(),
        line: None,
        message: e.to_string(),
    })?;
    let mut out: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_str().is_some_and(|s| s.ends_with(suffix)))
        .collect();
    out.sort();
    Ok(out)
}

/// `corpus/unit/records/*.jsonl.gz`, sorted by name.
pub fn record_files() -> Result<Vec<PathBuf>, LoadError> {
    sorted_files(&corpus_unit_dir().join("records"), ".jsonl.gz")
}

/// `f(file)` for every file on `threads` threads, each taking the next file when it is done with
/// one; the results are in the order of `files`.
pub fn map_files_parallel<T: Send>(
    files: &[PathBuf],
    threads: usize,
    f: impl Fn(&Path) -> T + Sync,
) -> Vec<T> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut results: Vec<(usize, T)> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(file) = files.get(i) else { break };
                        out.push((i, f(file)));
                    }
                    out
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    results.sort_by_key(|(i, _)| *i);
    results.into_iter().map(|(_, r)| r).collect()
}

/// The file stem of a record or descriptor file (`AliasStringsTest`).
pub fn file_stem(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.split('.').next().unwrap_or("").to_string()
}

/// Reads a gzip file into a string.
pub fn read_gz(path: &Path) -> Result<String, LoadError> {
    let f = std::fs::File::open(path).map_err(|e| LoadError {
        file: path.to_path_buf(),
        line: None,
        message: e.to_string(),
    })?;
    let mut s = String::new();
    GzDecoder::new(f)
        .read_to_string(&mut s)
        .map_err(|e| LoadError {
            file: path.to_path_buf(),
            line: None,
            message: format!("gzip/UTF-8: {e}"),
        })?;
    Ok(s)
}

/// Parses every line of a JSONL text into JSON values.
pub fn parse_jsonl(path: &Path, text: &str) -> Result<Vec<JsonValue>, LoadError> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.is_empty())
        .map(|(i, l)| {
            parse_json(l).map_err(|e| LoadError {
                file: path.to_path_buf(),
                line: Some(i + 1),
                message: e.to_string(),
            })
        })
        .collect()
}

/// A record with the raw JSON it was read from (DSL.md `when` paths and `{"record": path}`
/// read the raw JSON).
#[derive(Clone, Debug)]
pub struct LoadedRecord {
    pub record: Record,
    pub raw: JsonValue,
    /// 0-based line index in the record file (the `index` of `ExpectedPipeline` entries).
    pub index: usize,
}

/// The non-empty lines of a gzip JSONL file, read one at a time so that only the current line
/// is in memory.
pub struct JsonlLines {
    path: PathBuf,
    reader: BufReader<GzDecoder<std::fs::File>>,
    /// 1-based number of the last line read.
    line: usize,
    /// 0-based index of the next non-empty line.
    index: usize,
    done: bool,
}

/// One non-empty line of a JSONL file.
pub struct JsonlLine {
    /// 1-based line number in the file.
    pub line: usize,
    /// 0-based index among the non-empty lines (the record index).
    pub index: usize,
    pub text: String,
}

impl JsonlLine {
    /// Parses the line into a JSON value.
    pub fn parse(&self, path: &Path) -> Result<JsonValue, LoadError> {
        parse_json(&self.text).map_err(|e| LoadError {
            file: path.to_path_buf(),
            line: Some(self.line),
            message: e.to_string(),
        })
    }
}

impl Iterator for JsonlLines {
    type Item = Result<JsonlLine, LoadError>;

    fn next(&mut self) -> Option<Self::Item> {
        while !self.done {
            let mut text = String::new();
            match self.reader.read_line(&mut text) {
                Ok(0) => self.done = true,
                Ok(_) => {
                    self.line += 1;
                    // As `str::lines`: strip "\n" or "\r\n".
                    if text.ends_with('\n') {
                        text.pop();
                        if text.ends_with('\r') {
                            text.pop();
                        }
                    }
                    if text.is_empty() {
                        continue;
                    }
                    let index = self.index;
                    self.index += 1;
                    return Some(Ok(JsonlLine {
                        line: self.line,
                        index,
                        text,
                    }));
                }
                Err(e) => {
                    self.done = true;
                    return Some(Err(LoadError {
                        file: self.path.clone(),
                        line: None,
                        message: format!("gzip/UTF-8: {e}"),
                    }));
                }
            }
        }
        None
    }
}

/// Streams the non-empty lines of a gzip JSONL file.
pub fn jsonl_lines(path: &Path) -> Result<JsonlLines, LoadError> {
    let f = std::fs::File::open(path).map_err(|e| LoadError {
        file: path.to_path_buf(),
        line: None,
        message: e.to_string(),
    })?;
    Ok(JsonlLines {
        path: path.to_path_buf(),
        reader: BufReader::new(GzDecoder::new(f)),
        line: 0,
        index: 0,
        done: false,
    })
}

/// Streams the JSON values of the non-empty lines of a gzip JSONL file.
pub fn jsonl_values(
    path: &Path,
) -> Result<impl Iterator<Item = Result<JsonValue, LoadError>> + use<>, LoadError> {
    let lines = jsonl_lines(path)?;
    let path = path.to_path_buf();
    Ok(lines.map(move |l| l?.parse(&path)))
}

/// Reads one record line into a [`LoadedRecord`].
pub fn load_record(path: &Path, line: &JsonlLine) -> Result<LoadedRecord, LoadError> {
    let raw = line.parse(path)?;
    let record = Record::from_json(&raw).map_err(|e| LoadError {
        file: path.to_path_buf(),
        line: Some(line.line),
        message: e.to_string(),
    })?;
    Ok(LoadedRecord {
        record,
        raw,
        index: line.index,
    })
}

/// Streams the records of one `.jsonl.gz` file, one line at a time (a large class such as
/// TypedScopeCreatorTest is over 150 MB of JSON; materialised, it takes several times that).
pub fn records(
    path: &Path,
) -> Result<impl Iterator<Item = Result<LoadedRecord, LoadError>> + use<>, LoadError> {
    let lines = jsonl_lines(path)?;
    let path = path.to_path_buf();
    Ok(lines.map(move |l| load_record(&path, &l?)))
}

/// Loads every record of one `.jsonl.gz` file.
pub fn load_records(path: &Path) -> Result<Vec<LoadedRecord>, LoadError> {
    records(path)?.collect()
}

fn model_err(path: &Path, line: Option<usize>, e: impl fmt::Display) -> LoadError {
    LoadError {
        file: path.to_path_buf(),
        line,
        message: e.to_string(),
    }
}

/// Reads and parses one JSON file.
pub fn read_json_file(path: &Path) -> Result<JsonValue, LoadError> {
    let text = std::fs::read_to_string(path).map_err(|e| model_err(path, None, e))?;
    parse_json(&text).map_err(|e| model_err(path, None, e))
}

/// `corpus/unit/descriptors/*.json`, sorted by name.
pub fn descriptor_files() -> Result<Vec<PathBuf>, LoadError> {
    sorted_files(&corpus_unit_dir().join("descriptors"), ".json")
}

/// The descriptor file of a record file stem (`AliasStringsTest`).
pub fn descriptor_path(stem: &str) -> PathBuf {
    corpus_unit_dir()
        .join("descriptors")
        .join(format!("{stem}.json"))
}

/// A descriptor with the raw JSON it was read from.
#[derive(Clone, Debug)]
pub struct LoadedDescriptor {
    pub descriptor: Descriptor,
    pub raw: JsonValue,
}

/// Loads one descriptor file.
pub fn load_descriptor(path: &Path) -> Result<LoadedDescriptor, LoadError> {
    let raw = read_json_file(path)?;
    let descriptor = Descriptor::from_json(&raw, "$").map_err(|e| model_err(path, None, e))?;
    Ok(LoadedDescriptor { descriptor, raw })
}

/// Loads the descriptor of a record file stem, `None` when the class has no descriptor.
pub fn load_descriptor_for(stem: &str) -> Result<Option<LoadedDescriptor>, LoadError> {
    let p = descriptor_path(stem);
    if p.exists() {
        load_descriptor(&p).map(Some)
    } else {
        Ok(None)
    }
}

/// `corpus/unit/options_defaults.json`.
pub fn options_defaults_path() -> PathBuf {
    corpus_unit_dir().join("options_defaults.json")
}

/// Loads `corpus/unit/options_defaults.json` (and its raw JSON).
pub fn load_options_defaults() -> Result<(OptionsDefaults, JsonValue), LoadError> {
    let p = options_defaults_path();
    let raw = read_json_file(&p)?;
    let d = OptionsDefaults::from_json(&raw, "$").map_err(|e| model_err(&p, None, e))?;
    Ok((d, raw))
}

/// `corpus/unit/derived/expected_pipeline.jsonl.gz`.
pub fn expected_pipeline_path() -> PathBuf {
    corpus_unit_dir().join("derived/expected_pipeline.jsonl.gz")
}

/// Loads every line of `derived/expected_pipeline.jsonl.gz` (with its raw JSON).
pub fn load_expected_pipeline() -> Result<Vec<(ExpectedPipeline, JsonValue)>, LoadError> {
    let p = expected_pipeline_path();
    jsonl_lines(&p)?
        .map(|l| {
            let l = l?;
            let raw = l.parse(&p)?;
            let e = ExpectedPipeline::from_json(&raw, "$")
                .map_err(|e| model_err(&p, Some(l.line), e))?;
            Ok((e, raw))
        })
        .collect()
}
