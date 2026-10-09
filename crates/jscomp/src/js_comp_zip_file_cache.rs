/*
 * Copyright 2018 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/JSCompZipFileCache.java.

use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{check_not_null, java_lang::io_exception::IOException};
use std::{
    fs,
    io::Read,
    sync::{LazyLock, Mutex},
    time::SystemTime,
};
pub struct JSCompZipFileCache;
static ZIP_CACHE_SIZE: LazyLock<usize> = LazyLock::new(|| {
    std::env::var("jscomp.zipfile.cachesize")
        .unwrap_or_else(|_| "1000".into())
        .parse()
        .unwrap()
});
static ZIP_FILE_CACHE: LazyLock<Mutex<IndexMap<String, CachedZipFile>>> =
    LazyLock::new(|| Mutex::new(IndexMap::<_, _>::default()));
impl JSCompZipFileCache {
    // port: JSCompZipFileCache#getEntryStream
    pub fn get_entry_stream(zip_name: &str, entry_name: &str) -> Result<Vec<u8>, IOException> {
        let mut cache = ZIP_FILE_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut entry = cache
            .shift_remove(zip_name)
            .unwrap_or_else(|| CachedZipFile::new(zip_name));
        let result = entry.get_entry_stream(entry_name);
        cache.insert(zip_name.into(), entry);
        while cache.len() > *ZIP_CACHE_SIZE {
            if let Some((_, mut removed)) = cache.shift_remove_index(0) {
                removed.maybe_close();
            }
        }
        result
    }
}
struct CachedZipFile {
    path: String,
    zip_file: Option<zip::ZipArchive<fs::File>>,
    last_modified: Option<SystemTime>,
    entry_indices: IndexMap<String, usize>,
}
impl CachedZipFile {
    // port: JSCompZipFileCache.CachedZipFile#CachedZipFile
    fn new(zip_name: &str) -> Self {
        Self {
            path: zip_name.into(),
            zip_file: None,
            last_modified: None,
            entry_indices: IndexMap::<_, _>::default(),
        }
    }
    // port: JSCompZipFileCache.CachedZipFile#getEntryStream
    fn get_entry_stream(&mut self, entry_name: &str) -> Result<Vec<u8>, IOException> {
        self.refresh_if_needed()?;
        let archive = self.zip_file.as_mut().unwrap();
        let index = check_not_null!(
            self.entry_indices
                .get(entry_name)
                .copied()
                .or_else(|| self.entry_indices.get(&format!("{entry_name}/")).copied()),
            "%s!/%s",
            self.path,
            entry_name
        );
        let mut entry = archive
            .by_index(index)
            .map_err(|e| IOException::new(e.to_string()))?;
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|e| IOException::new(e.to_string()))?;
        Ok(bytes)
    }
    // port: JSCompZipFileCache.CachedZipFile#refreshIfNeeded
    fn refresh_if_needed(&mut self) -> Result<(), IOException> {
        let new_last_modified = fs::metadata(&self.path)
            .and_then(|m| m.modified())
            .map_err(|e| IOException::from_io(e, &self.path))?;
        if self.last_modified == Some(new_last_modified) {
            return Ok(());
        }
        // The outer cache mutex is the synchronization boundary for the second
        // timestamp check, matching Java's double-checked cache refresh.
        if self.last_modified == Some(new_last_modified) {
            return Ok(());
        }
        self.maybe_close();
        let file = fs::File::open(&self.path).map_err(|e| IOException::from_io(e, &self.path))?;
        self.zip_file =
            Some(zip::ZipArchive::new(file).map_err(|e| IOException::new(e.to_string()))?);
        self.entry_indices.clear();
        let archive = self.zip_file.as_mut().unwrap();
        for index in 0..archive.len() {
            let entry = archive
                .by_index(index)
                .map_err(|e| IOException::new(e.to_string()))?;
            let name = std::str::from_utf8(entry.name_raw())
                .map_err(|_| IOException::new("invalid CEN header (bad entry name)"))?;
            self.entry_indices.insert(name.into(), index);
        }
        self.last_modified = Some(new_last_modified);
        Ok(())
    }
    // port: JSCompZipFileCache.CachedZipFile#maybeClose
    fn maybe_close(&mut self) {
        self.zip_file = None;
    }
}
