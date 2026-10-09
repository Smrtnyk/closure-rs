/*
 * Copyright 2012 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/JvmMetrics.java.

//! A class to report jvm/jmx statistics.
//!
//! The Rust runtime has no JVM management beans. `management_factory` stands for
//! java.lang.management.ManagementFactory as seen from this process: no garbage collectors, no
//! memory pools, heap usage from Platform (which reports 0 like the J2CL Platform), and a
//! compilation bean without JIT time.
use crate::platform::Platform;
use std::io::{self, Write};

pub struct JvmMetrics;

const TABULAR_COLON_POS: usize = 40;
const ONE_KILO_BYTE: i64 = 1 << 10;
const ONE_MEGA_BYTE: i64 = 1 << 20;
const ONE_GIGA_BYTE: i64 = 1 << 30;

impl JvmMetrics {
    // port: JvmMetrics#maybeWriteJvmMetrics
    pub fn maybe_write_jvm_metrics(out: &mut dyn Write, options: Option<&str>) -> io::Result<()> {
        let Some(options) = options else {
            return Ok(());
        };

        let mut verbose_mode = false;
        let mut pretty_mode = false;
        let mut st = string_tokenizer(options, ':').into_iter().peekable();
        // options are grouped in order 'detail:format:types'
        if let Some(mode) = st.next()
            && equals_ignore_case(mode, "verbose")
        {
            verbose_mode = true;
        }

        if let Some(mode) = st.next()
            && equals_ignore_case(mode, "pretty")
        {
            pretty_mode = true;
        }

        if st.peek().is_some() {
            for types in st {
                for type_ in string_tokenizer(types, ',') {
                    Self::write_metrics(out, type_, verbose_mode, pretty_mode)?;
                }
            }
        } else {
            // the default
            Self::write_metrics(out, "all", verbose_mode, pretty_mode)?;
        }
        Ok(())
    }

    // port: JvmMetrics#writeMetrics
    fn write_metrics(
        out: &mut dyn Write,
        type_: &str,
        verbose: bool,
        pretty: bool,
    ) -> io::Result<()> {
        if type_ == "gc" || equals_ignore_case(type_, "all") {
            Self::write_garbage_collection_stats(out, verbose, pretty)?;
        }
        if type_ == "mem" || equals_ignore_case(type_, "all") {
            Self::write_memory_metrics(out, verbose, pretty)?;
        }
        if type_ == "jit" || equals_ignore_case(type_, "all") {
            Self::write_jit_metrics(out, verbose, pretty)?;
        }
        Ok(())
    }

    // port: JvmMetrics#writeJitMetrics
    fn write_jit_metrics(out: &mut dyn Write, verbose: bool, pretty: bool) -> io::Result<()> {
        let c_bean = management_factory::get_compilation_mx_bean();

        let name = if verbose { c_bean.get_name() } else { "total" };

        if pretty {
            writeln!(out, "\nJIT Stats")?;
            writeln!(
                out,
                "\t{name} jit time: {} ms",
                c_bean.get_total_compilation_time()
            )?;
        } else {
            writeln!(
                out,
                "{}",
                Self::normalize_tabular_colon_pos(&format!(
                    "{}-jit-time-ms : {}",
                    Self::normalize_name(name),
                    c_bean.get_total_compilation_time()
                ))
            )?;
        }
        Ok(())
    }

    // port: JvmMetrics#writeOverallMemoryUsage
    fn write_overall_memory_usage(
        out: &mut dyn Write,
        usage: &MemoryUsage,
        prefix: &str,
        pretty: bool,
    ) -> io::Result<()> {
        if pretty {
            writeln!(out, "\t{prefix}")?;
            writeln!(
                out,
                "\t\tavailable         : {}",
                Self::format_bytes(usage.get_max())
            )?;
            writeln!(
                out,
                "\t\tcurrent           : {}",
                Self::format_bytes(usage.get_used())
            )?;
        } else {
            let prefix = Self::normalize_name(prefix);
            writeln!(
                out,
                "{}",
                Self::normalize_tabular_colon_pos(&format!(
                    "{prefix}-available-bytes : {}",
                    usage.get_max()
                ))
            )?;
            writeln!(
                out,
                "{}",
                Self::normalize_tabular_colon_pos(&format!(
                    "{prefix}-current-bytes : {}",
                    usage.get_used()
                ))
            )?;
        }
        Ok(())
    }

    // port: JvmMetrics#writePoolMemoryUsage
    fn write_pool_memory_usage(
        out: &mut dyn Write,
        usage: &MemoryUsage,
        peak_usage: &MemoryUsage,
        prefix: Option<&str>,
        pretty: bool,
    ) -> io::Result<()> {
        if pretty {
            writeln!(
                out,
                "\t\tavailable         : {}",
                Self::format_bytes(usage.get_max())
            )?;
            writeln!(
                out,
                "\t\tpeak              : {}",
                Self::format_bytes(peak_usage.get_used())
            )?;
            writeln!(
                out,
                "\t\tcurrent           : {}",
                Self::format_bytes(usage.get_used())
            )?;
        } else {
            // String.format("%s", null) prints "null".
            let prefix = prefix.unwrap_or("null");
            writeln!(
                out,
                "{}",
                Self::normalize_tabular_colon_pos(&format!(
                    "{prefix}-available-bytes : {}",
                    usage.get_max()
                ))
            )?;
            writeln!(
                out,
                "{}",
                Self::normalize_tabular_colon_pos(&format!(
                    "{prefix}-peak-bytes : {}",
                    peak_usage.get_used()
                ))
            )?;
            writeln!(
                out,
                "{}",
                Self::normalize_tabular_colon_pos(&format!(
                    "{prefix}-current-bytes : {}",
                    usage.get_used()
                ))
            )?;
        }
        Ok(())
    }

    // port: JvmMetrics#writeMemoryMetrics
    fn write_memory_metrics(out: &mut dyn Write, verbose: bool, pretty: bool) -> io::Result<()> {
        if pretty {
            writeln!(out, "\nMemory usage")?;
        }

        // only show overall stats in verbose mode
        if verbose {
            let overall_mem_bean = management_factory::get_memory_mx_bean();
            let usage = overall_mem_bean.get_heap_memory_usage();
            Self::write_overall_memory_usage(out, &usage, "Heap", pretty)?;

            let usage = overall_mem_bean.get_non_heap_memory_usage();
            Self::write_overall_memory_usage(out, &usage, "Non-heap", pretty)?;
        }

        if verbose {
            let mp_beans = management_factory::get_memory_pool_mx_beans();
            for mp_bean in &mp_beans {
                let current_usage = mp_bean.get_usage();
                let peak_usage = mp_bean.get_peak_usage();
                if pretty {
                    writeln!(out, "\tPool {}", mp_bean.get_name())?;
                    Self::write_pool_memory_usage(out, &current_usage, &peak_usage, None, true)?;
                } else {
                    Self::write_pool_memory_usage(
                        out,
                        &current_usage,
                        &peak_usage,
                        Some(&format!(
                            "mem-pool-{}",
                            Self::normalize_name(mp_bean.get_name())
                        )),
                        false,
                    )?;
                }
            }
        } else {
            let mut available: i64 = 0;
            let mut current: i64 = 0;
            let mut peak: i64 = 0;
            let mp_beans = management_factory::get_memory_pool_mx_beans();
            for mp_bean in &mp_beans {
                let current_usage = mp_bean.get_usage();
                available = available.wrapping_add(current_usage.get_max());
                current = current.wrapping_add(current_usage.get_used());
                let peak_usage = mp_bean.get_peak_usage();
                peak = peak.wrapping_add(peak_usage.get_used());
            }
            let summary_usage = MemoryUsage::new(0, current, current, available);
            let summary_peak_usage = MemoryUsage::new(0, peak, peak, peak);
            if pretty {
                writeln!(out, "\tAggregate of {} memory pools", mp_beans.len())?;
                Self::write_pool_memory_usage(
                    out,
                    &summary_usage,
                    &summary_peak_usage,
                    None,
                    true,
                )?;
            } else {
                Self::write_pool_memory_usage(
                    out,
                    &summary_usage,
                    &summary_peak_usage,
                    Some("mem"),
                    false,
                )?;
            }
        }
        Ok(())
    }

    // port: JvmMetrics#writeGarbageCollectionStats
    fn write_garbage_collection_stats(
        out: &mut dyn Write,
        verbose: bool,
        pretty: bool,
    ) -> io::Result<()> {
        let gc_beans = management_factory::get_garbage_collector_mx_beans();

        if verbose {
            if pretty {
                writeln!(out, "\nGarbage collection stats")?;
                for gc_bean in &gc_beans {
                    writeln!(out, "\tCollector {}", gc_bean.get_name())?;
                    writeln!(
                        out,
                        "\t\tcollection count   : {}",
                        gc_bean.get_collection_count()
                    )?;
                    writeln!(
                        out,
                        "\t\tcollection time    : {} ms",
                        gc_bean.get_collection_time()
                    )?;
                }
            } else {
                for gc_bean in &gc_beans {
                    let name = Self::normalize_name(gc_bean.get_name());
                    writeln!(
                        out,
                        "{}",
                        Self::normalize_tabular_colon_pos(&format!(
                            "gc-{name}-collection-count : {}",
                            gc_bean.get_collection_count()
                        ))
                    )?;
                    writeln!(
                        out,
                        "{}",
                        Self::normalize_tabular_colon_pos(&format!(
                            "gc-{name}-collection-time-ms : {}",
                            gc_bean.get_collection_time()
                        ))
                    )?;
                }
            }
        } else {
            let mut collection_count: i64 = 0;
            let mut collection_time: i64 = 0;
            let collector_count = gc_beans.len();
            for gc_bean in &gc_beans {
                collection_count = collection_count.wrapping_add(gc_bean.get_collection_count());
                collection_time = collection_time.wrapping_add(gc_bean.get_collection_time());
            }
            if pretty {
                writeln!(out, "\nGarbage collection stats")?;
                writeln!(out, "\tAggregate of {collector_count} collectors")?;
                writeln!(out, "\t\tcollection count   : {collection_count}")?;
                writeln!(out, "\t\tcollection time    : {collection_time} ms")?;
            } else {
                let name = Self::normalize_name("aggregate");
                writeln!(
                    out,
                    "{}",
                    Self::normalize_tabular_colon_pos(&format!(
                        "gc-{name}-collection-count : {collection_count}"
                    ))
                )?;
                writeln!(
                    out,
                    "{}",
                    Self::normalize_tabular_colon_pos(&format!(
                        "gc-{name}-collection-time-ms : {collection_time}"
                    ))
                )?;
            }
        }
        Ok(())
    }

    // port: JvmMetrics#normalizeName
    fn normalize_name(name: &str) -> String {
        name.replace(' ', "_").to_lowercase()
    }

    // port: JvmMetrics#normalizeTabularColonPos
    fn normalize_tabular_colon_pos(string: &str) -> String {
        let mut sb: Vec<u16> = string.encode_utf16().collect();
        // StringBuilder#indexOf(":") is -1 without a colon, and insert(-1, ' ') then throws.
        let Some(index) = sb.iter().position(|&c| c == u16::from(b':')) else {
            panic!(
                "java.lang.StringIndexOutOfBoundsException: offset -1, length {}",
                sb.len()
            );
        };
        for index in index..TABULAR_COLON_POS {
            sb.insert(index, u16::from(b' '));
        }
        String::from_utf16_lossy(&sb)
    }

    // port: JvmMetrics#formatBytes
    fn format_bytes(num_bytes: i64) -> String {
        if num_bytes < ONE_KILO_BYTE {
            format!("{num_bytes} B")
        } else if num_bytes < ONE_MEGA_BYTE {
            format!("{} KB", num_bytes / ONE_KILO_BYTE)
        } else if num_bytes < ONE_GIGA_BYTE {
            format!("{} MB", num_bytes / ONE_MEGA_BYTE)
        } else {
            format!("{} GB", num_bytes / ONE_GIGA_BYTE)
        }
    }
}

// java.lang.String#equalsIgnoreCase (regionMatches with ignoreCase over UTF-16 units).
fn equals_ignore_case(a: &str, b: &str) -> bool {
    fn simple_map(c: u16, map: impl Fn(char) -> Vec<char>) -> u16 {
        let Some(ch) = char::from_u32(u32::from(c)) else {
            return c;
        };
        match map(ch).as_slice() {
            [mapped] if u32::from(*mapped) <= 0xFFFF => u32::from(*mapped) as u16,
            _ => c,
        }
    }
    fn simple_upper(c: u16) -> u16 {
        simple_map(c, |ch| ch.to_uppercase().collect())
    }
    fn simple_lower(c: u16) -> u16 {
        simple_map(c, |ch| ch.to_lowercase().collect())
    }
    let a: Vec<u16> = a.encode_utf16().collect();
    let b: Vec<u16> = b.encode_utf16().collect();
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(&b).all(|(&c1, &c2)| {
        if c1 == c2 {
            return true;
        }
        let u1 = simple_upper(c1);
        let u2 = simple_upper(c2);
        u1 == u2 || simple_lower(u1) == simple_lower(u2)
    })
}

// java.util.StringTokenizer(str, delim) without returnDelims: the non-empty runs between delims.
fn string_tokenizer(s: &str, delim: char) -> Vec<&str> {
    s.split(delim).filter(|token| !token.is_empty()).collect()
}

/// java.lang.management.MemoryUsage.
pub struct MemoryUsage {
    init: i64,
    used: i64,
    committed: i64,
    max: i64,
}
impl MemoryUsage {
    // java.lang.management.MemoryUsage(long, long, long, long)
    pub fn new(init: i64, used: i64, committed: i64, max: i64) -> Self {
        Self {
            init,
            used,
            committed,
            max,
        }
    }
    pub fn get_init(&self) -> i64 {
        self.init
    }
    pub fn get_used(&self) -> i64 {
        self.used
    }
    pub fn get_committed(&self) -> i64 {
        self.committed
    }
    pub fn get_max(&self) -> i64 {
        self.max
    }
}

/// The management beans of this (JVM-less) process.
mod management_factory {
    use super::{MemoryUsage, Platform};

    pub struct CompilationMXBean;
    impl CompilationMXBean {
        pub fn get_name(&self) -> &'static str {
            "none"
        }
        pub fn get_total_compilation_time(&self) -> i64 {
            0
        }
    }
    pub struct MemoryMXBean;
    impl MemoryMXBean {
        pub fn get_heap_memory_usage(&self) -> MemoryUsage {
            let total = Platform::total_memory();
            let used = total - Platform::free_memory();
            MemoryUsage::new(0, used, total, total)
        }
        pub fn get_non_heap_memory_usage(&self) -> MemoryUsage {
            MemoryUsage::new(0, 0, 0, -1)
        }
    }
    pub enum MemoryPoolMXBean {}
    impl MemoryPoolMXBean {
        pub fn get_name(&self) -> &'static str {
            match *self {}
        }
        pub fn get_usage(&self) -> MemoryUsage {
            match *self {}
        }
        pub fn get_peak_usage(&self) -> MemoryUsage {
            match *self {}
        }
    }
    pub enum GarbageCollectorMXBean {}
    impl GarbageCollectorMXBean {
        pub fn get_name(&self) -> &'static str {
            match *self {}
        }
        pub fn get_collection_count(&self) -> i64 {
            match *self {}
        }
        pub fn get_collection_time(&self) -> i64 {
            match *self {}
        }
    }
    pub fn get_compilation_mx_bean() -> CompilationMXBean {
        CompilationMXBean
    }
    pub fn get_memory_mx_bean() -> MemoryMXBean {
        MemoryMXBean
    }
    pub fn get_memory_pool_mx_beans() -> Vec<MemoryPoolMXBean> {
        Vec::new()
    }
    pub fn get_garbage_collector_mx_beans() -> Vec<GarbageCollectorMXBean> {
        Vec::new()
    }
}
