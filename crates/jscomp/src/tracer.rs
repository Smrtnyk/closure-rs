/*
 * Copyright 2002 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Tracer.java.

use closure_rhino::fast_hash::IndexMap;
use std::{
    cell::RefCell,
    fmt::{self, Write},
    sync::{
        Arc, LazyLock, Mutex, RwLock,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
    thread::{self, ThreadId},
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_TRACE_SIZE: usize = 1000;
static DEFAULT_PRETTY_PRINT: AtomicBool = AtomicBool::new(false);
static EXTRA_TRACING_STATISTICS: RwLock<Vec<Arc<dyn TracingStatistic>>> = RwLock::new(Vec::new());
static CLOCK: LazyLock<RwLock<Arc<dyn InternalClock>>> =
    LazyLock::new(|| RwLock::new(Arc::new(SystemClock)));
static TYPE_MAPS: Mutex<Option<TypeMaps>> = Mutex::new(None);
thread_local! {static TRACES:RefCell<Option<Arc<Mutex<ThreadTrace>>>>=const {RefCell::new(None)};}

pub trait InternalClock: Send + Sync {
    // port: Tracer.InternalClock#currentTimeMillis
    fn current_time_millis(&self) -> i64;
}
struct SystemClock;
impl InternalClock for SystemClock {
    // port: Tracer.<anonymous>#currentTimeMillis
    fn current_time_millis(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }
}
fn now() -> i64 {
    let clock = CLOCK.read().unwrap().clone();
    clock.current_time_millis()
}
fn statistics() -> Vec<Arc<dyn TracingStatistic>> {
    EXTRA_TRACING_STATISTICS.read().unwrap().clone()
}
fn warning(message: &str) {
    eprintln!("{message}");
}

struct TraceData {
    type_: Option<String>,
    comment: String,
    start_time_ms: i64,
    stop_time_ms: i64,
    extra_tracing_values: Option<Vec<i64>>,
}
impl fmt::Display for TraceData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(t) = &self.type_ {
            write!(f, "[{t}] {}", self.comment)
        } else {
            f.write_str(&self.comment)
        }
    }
}
pub struct Tracer {
    data: Arc<Mutex<TraceData>>,
    start_thread: ThreadId,
}
impl Tracer {
    // port: Tracer#Tracer(String, String)
    pub fn new(type_: Option<&str>, comment: Option<&str>) -> Self {
        let start_time_ms = now();
        let start_thread = thread::current().id();
        let extra = statistics();
        let extra_tracing_values = if extra.is_empty() {
            None
        } else {
            Some(extra.iter().map(|stat| stat.start(start_thread)).collect())
        };
        let tracer = Self {
            data: Arc::new(Mutex::new(TraceData {
                type_: type_.map(str::to_owned),
                comment: comment.unwrap_or_default().into(),
                start_time_ms,
                stop_time_ms: 0,
                extra_tracing_values,
            })),
            start_thread,
        };
        let handle = Self::get_thread_trace();
        let mut trace = handle.lock().unwrap();
        if !trace.is_initialized() {
            return tracer;
        }
        if trace.events.len() >= MAX_TRACE_SIZE {
            warning(&format!(
                "Giant thread trace. Too many Tracers created. Clearing to avoid memory leak.\n{trace}"
            ));
            trace.truncate_events();
        }
        if trace.outstanding_events.len() >= MAX_TRACE_SIZE {
            warning(&format!(
                "Too many outstanding Tracers. Tracer.stop() is missing or Tracer.stop() is not wrapped in a try/finally block. Clearing to avoid memory leak.\n{trace}"
            ));
            trace.truncate_outstanding_events();
        }
        trace.start_event(&tracer);
        tracer
    }
    // port: Tracer#Tracer(String)
    pub fn new_comment(comment: Option<&str>) -> Self {
        Self::new(None, comment)
    }
    // port: Tracer#longToPaddedString
    fn long_to_padded_string(value: i64, digits_column_width: i32) -> String {
        let digit_width = Self::num_digits(value);
        let mut result = String::new();
        Self::append_spaces(&mut result, digits_column_width - digit_width);
        write!(result, "{value}").unwrap();
        result
    }
    // port: Tracer#numDigits
    fn num_digits(mut n: i64) -> i32 {
        let mut i = 0;
        loop {
            i += 1;
            n /= 10;
            if n <= 0 {
                return i;
            }
        }
    }
    // port: Tracer#appendSpaces
    pub fn append_spaces(result: &mut String, mut num_spaces: i32) {
        if num_spaces > 16 {
            warning("Tracer.appendSpaces called with large numSpaces");
            num_spaces = 16;
        }
        while num_spaces >= 5 {
            result.push_str("     ");
            num_spaces -= 5;
        }
        match num_spaces {
            1 => result.push(' '),
            2 => result.push_str("  "),
            3 => result.push_str("   "),
            4 => result.push_str("    "),
            _ => {}
        }
    }
    // port: Tracer#addTracingStatistic
    pub fn add_tracing_statistic(statistic: Arc<dyn TracingStatistic>) -> i32 {
        if statistic.enable() {
            let mut extra = EXTRA_TRACING_STATISTICS.write().unwrap();
            extra.push(statistic.clone());
            extra
                .iter()
                .rposition(|entry| Arc::ptr_eq(entry, &statistic))
                .unwrap() as i32
        } else {
            -1
        }
    }
    // port: Tracer#clearTracingStatisticsTestingOnly
    pub fn clear_tracing_statistics_testing_only() {
        EXTRA_TRACING_STATISTICS.write().unwrap().clear();
    }
    // port: Tracer#stop(int)
    pub fn stop_with_silence_threshold(&mut self, silence_threshold: i32) -> i64 {
        assert_eq!(thread::current().id(), self.start_thread);
        let handle = Self::get_thread_trace();
        if !handle.lock().unwrap().is_initialized() {
            return 0;
        }
        let stop_time_ms = now();
        let extra = statistics();
        let mut data = self.data.lock().unwrap();
        data.stop_time_ms = stop_time_ms;
        if let Some(values) = &mut data.extra_tracing_values {
            for (i, value) in values.iter_mut().enumerate() {
                *value = extra[i].stop(self.start_thread).wrapping_sub(*value);
            }
        }
        let elapsed = data.stop_time_ms.wrapping_sub(data.start_time_ms);
        drop(data);
        if !handle.lock().unwrap().is_initialized() {
            return 0;
        }
        handle.lock().unwrap().end_event(self, silence_threshold);
        elapsed
    }
    // port: Tracer#stop()
    pub fn stop(&mut self) -> i64 {
        self.stop_with_silence_threshold(-1)
    }
    // port: Tracer#setDefaultSilenceThreshold
    pub fn set_default_silence_threshold(threshold: i32) {
        Self::get_thread_trace()
            .lock()
            .unwrap()
            .default_silence_threshold = threshold;
    }
    // port: Tracer#initCurrentThreadTrace()
    pub fn init_current_thread_trace() {
        let mut handle = Self::get_thread_trace();
        if !handle.lock().unwrap().is_empty() {
            warning(&format!("Non-empty timer log:\n{}", handle.lock().unwrap()));
            Self::clear_thread_trace();
            handle = Self::get_thread_trace();
        }
        handle.lock().unwrap().init();
    }
    // port: Tracer#initCurrentThreadTrace(int)
    pub fn init_current_thread_trace_with_threshold(default_silence_threshold: i32) {
        Self::init_current_thread_trace();
        Self::set_default_silence_threshold(default_silence_threshold);
    }
    // port: Tracer#getCurrentThreadTraceReport
    pub fn get_current_thread_trace_report() -> String {
        Self::get_thread_trace().lock().unwrap().to_string()
    }
    // port: Tracer#logCurrentThreadTrace
    pub fn log_current_thread_trace() {
        let handle = Self::get_thread_trace();
        let trace = handle.lock().unwrap();
        if !trace.is_initialized() {
            eprintln!(
                "Tracer log requested for this thread but was not initialized using Tracer.initCurrentThreadTrace()."
            );
            return;
        }
        if !trace.is_empty() {
            eprintln!("timers:\n{trace}");
        }
    }
    // port: Tracer#clearCurrentThreadTrace
    pub fn clear_current_thread_trace() {
        Self::clear_thread_trace();
    }
    // port: Tracer#logAndClearCurrentThreadTrace
    pub fn log_and_clear_current_thread_trace() {
        Self::log_current_thread_trace();
        Self::clear_thread_trace();
    }
    // port: Tracer#setPrettyPrint
    pub fn set_pretty_print(enabled: bool) {
        DEFAULT_PRETTY_PRINT.store(enabled, Ordering::Relaxed);
    }
    // port: Tracer#enableTypeMaps
    pub fn enable_type_maps() {
        let mut maps = TYPE_MAPS.lock().unwrap();
        if maps.is_none() {
            *maps = Some(TypeMaps::default());
        }
    }
    // port: Tracer#getTypeToCountMap
    pub fn get_type_to_count_map() -> Option<Arc<AtomicTracerStatMap>> {
        TYPE_MAPS
            .lock()
            .unwrap()
            .as_ref()
            .map(|maps| maps.count.clone())
    }
    // port: Tracer#getTypeToSilentMap
    pub fn get_type_to_silent_map() -> Option<Arc<AtomicTracerStatMap>> {
        TYPE_MAPS
            .lock()
            .unwrap()
            .as_ref()
            .map(|maps| maps.silent.clone())
    }
    // port: Tracer#getTypeToTimeMap
    pub fn get_type_to_time_map() -> Option<Arc<AtomicTracerStatMap>> {
        TYPE_MAPS
            .lock()
            .unwrap()
            .as_ref()
            .map(|maps| maps.time.clone())
    }
    // port: Tracer#getStatsForType
    pub fn get_stats_for_type(type_: &str) -> Stat {
        Self::get_thread_trace()
            .lock()
            .unwrap()
            .stats
            .get(type_)
            .cloned()
            .unwrap_or_default()
    }
    // port: Tracer#formatTime
    fn format_time(time: i64) -> String {
        let sec = (time / 1000) % 60;
        let ms = time % 1000;
        format!("{sec:02}.{ms:03}")
    }
    // port: Tracer#getThreadTrace
    pub fn get_thread_trace() -> Arc<Mutex<ThreadTrace>> {
        TRACES.with(|traces| {
            let mut traces = traces.borrow_mut();
            if traces.is_none() {
                *traces = Some(Arc::new(Mutex::new(ThreadTrace {
                    pretty_print: DEFAULT_PRETTY_PRINT.load(Ordering::Relaxed),
                    ..ThreadTrace::default()
                })));
            }
            traces.as_ref().unwrap().clone()
        })
    }
    // port: Tracer#clearThreadTrace
    pub fn clear_thread_trace() {
        TRACES.with(|traces| *traces.borrow_mut() = None);
    }
    pub fn set_clock_for_testing(clock: Arc<dyn InternalClock>) {
        *CLOCK.write().unwrap() = clock;
    }
}
impl fmt::Display for Tracer {
    // port: Tracer#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.data.lock().unwrap().fmt(f)
    }
}
#[derive(Clone, Default)]
pub struct Stat {
    count: i32,
    silent: i32,
    clock_time: i32,
    extra_info: Option<Vec<i32>>,
}
impl Stat {
    // port: Tracer.Stat#getCount
    pub fn get_count(&self) -> i32 {
        self.count
    }
    // port: Tracer.Stat#getSilentCount
    pub fn get_silent_count(&self) -> i32 {
        self.silent
    }
    // port: Tracer.Stat#getTotalTime
    pub fn get_total_time(&self) -> i32 {
        self.clock_time
    }
    // port: Tracer.Stat#getExtraInfo
    pub fn get_extra_info(&self, index: usize) -> i32 {
        let info = self.extra_info.as_ref().expect("null extraInfo");
        if index >= info.len() { 0 } else { info[index] }
    }
}
#[derive(Default)]
struct TypeMaps {
    count: Arc<AtomicTracerStatMap>,
    silent: Arc<AtomicTracerStatMap>,
    time: Arc<AtomicTracerStatMap>,
}
struct Event {
    is_start: bool,
    tracer: Arc<Mutex<TraceData>>,
}
impl Event {
    // port: Tracer.Event#Event
    fn new(start: bool, tracer: &Tracer) -> Self {
        Self {
            is_start: start,
            tracer: tracer.data.clone(),
        }
    }
    // port: Tracer.Event#eventTime
    fn event_time(&self) -> i64 {
        let tracer = self.tracer.lock().unwrap();
        if self.is_start {
            tracer.start_time_ms
        } else {
            tracer.stop_time_ms
        }
    }
    // port: Tracer.Event#toString
    fn to_string(&self, prev_event_time: i64, indent: &str, digits_col_width: i32) -> String {
        let mut result = String::with_capacity(120);
        if prev_event_time == -1 {
            Tracer::append_spaces(&mut result, digits_col_width);
        } else {
            result.push_str(&Tracer::long_to_padded_string(
                self.event_time().wrapping_sub(prev_event_time),
                digits_col_width,
            ));
        }
        result.push(' ');
        result.push_str(&Tracer::format_time(self.event_time()));
        if self.is_start {
            result.push_str(" Start ");
            Tracer::append_spaces(&mut result, digits_col_width);
            result.push_str("   ");
        } else {
            result.push_str(" Done ");
            let tracer = self.tracer.lock().unwrap();
            result.push_str(&Tracer::long_to_padded_string(
                tracer.stop_time_ms.wrapping_sub(tracer.start_time_ms),
                digits_col_width,
            ));
            result.push_str(" ms ");
            if let Some(values) = &tracer.extra_tracing_values {
                let extra = statistics();
                for (i, delta) in values.iter().enumerate() {
                    write!(result, "{delta:4}{};  ", extra[i].get_units()).unwrap();
                }
            }
        }
        result.push_str(indent);
        write!(result, "{}", self.tracer.lock().unwrap()).unwrap();
        result
    }
}
#[derive(Default)]
pub struct ThreadTrace {
    pub default_silence_threshold: i32,
    events: Vec<Event>,
    outstanding_events: IndexMap<usize, Arc<Mutex<TraceData>>>,
    stats: IndexMap<String, Stat>,
    is_outstanding_events_truncated: bool,
    is_events_truncated: bool,
    is_initialized: bool,
    pretty_print: bool,
}
impl ThreadTrace {
    // port: Tracer.ThreadTrace#init
    pub fn init(&mut self) {
        self.is_initialized = true;
    }
    // port: Tracer.ThreadTrace#isInitialized
    pub fn is_initialized(&self) -> bool {
        self.is_initialized
    }
    // port: Tracer.ThreadTrace#startEvent
    fn start_event(&mut self, tracer: &Tracer) {
        self.events.push(Event::new(true, tracer));
        assert!(
            self.outstanding_events
                .insert(Arc::as_ptr(&tracer.data) as usize, tracer.data.clone())
                .is_none()
        );
    }
    // port: Tracer.ThreadTrace#endEvent
    fn end_event(&mut self, tracer: &Tracer, mut silence_threshold: i32) {
        if self
            .outstanding_events
            .shift_remove(&(Arc::as_ptr(&tracer.data) as usize))
            .is_none()
        {
            if self.is_outstanding_events_truncated {
                warning(
                    "event not found, probably because the event stack overflowed and was truncated",
                );
            } else {
                panic!("IllegalStateException");
            }
        }
        let data = tracer.data.lock().unwrap();
        let elapsed = data.stop_time_ms.wrapping_sub(data.start_time_ms);
        if silence_threshold == -1 {
            silence_threshold = self.default_silence_threshold;
        }
        if elapsed < i64::from(silence_threshold) {
            let mut removed = false;
            for i in 0..self.events.len() {
                let event = &self.events[i];
                if Arc::ptr_eq(&event.tracer, &tracer.data) {
                    assert!(event.is_start);
                    self.events.remove(i);
                    removed = true;
                    break;
                }
            }
            assert!(removed || self.is_events_truncated);
        } else {
            self.events.push(Event::new(false, tracer));
        }
        if let Some(type_) = &data.type_ {
            let extra = statistics();
            let stat = self.stats.entry(type_.clone()).or_insert_with(|| Stat {
                extra_info: if extra.is_empty() {
                    None
                } else {
                    Some(vec![0; extra.len()])
                },
                ..Stat::default()
            });
            stat.count = stat.count.wrapping_add(1);
            if let Some(map) = Tracer::get_type_to_count_map() {
                map.increment_by(type_, 1);
            }
            stat.clock_time = stat.clock_time.wrapping_add(elapsed as i32);
            if let Some(map) = Tracer::get_type_to_time_map() {
                map.increment_by(type_, elapsed);
            }
            if let (Some(info), Some(values)) = (&mut stat.extra_info, &data.extra_tracing_values) {
                for i in 0..info.len().min(values.len()) {
                    info[i] = info[i].wrapping_add(values[i] as i32);
                    if let Some(map) = extra[i].get_tracing_stat() {
                        map.increment_by(type_, values[i]);
                    }
                }
            }
            if elapsed < i64::from(silence_threshold) {
                stat.silent = stat.silent.wrapping_add(1);
                if let Some(map) = Tracer::get_type_to_silent_map() {
                    map.increment_by(type_, 1);
                }
            }
        }
    }
    // port: Tracer.ThreadTrace#isEmpty
    pub fn is_empty(&self) -> bool {
        self.events.is_empty() && self.outstanding_events.is_empty()
    }
    // port: Tracer.ThreadTrace#truncateOutstandingEvents
    pub fn truncate_outstanding_events(&mut self) {
        self.is_outstanding_events_truncated = true;
        self.outstanding_events.clear();
    }
    // port: Tracer.ThreadTrace#truncateEvents
    pub fn truncate_events(&mut self) {
        self.is_events_truncated = true;
        self.events.clear();
    }
    // port: Tracer.ThreadTrace#getMaxDigits
    fn get_max_digits(&self) -> i32 {
        let mut etime = -1;
        let mut max_time = 0;
        for event in &self.events {
            if etime != -1 {
                max_time = max_time.max(event.event_time().wrapping_sub(etime));
            }
            if !event.is_start {
                let tracer = event.tracer.lock().unwrap();
                max_time = max_time.max(tracer.stop_time_ms.wrapping_sub(tracer.start_time_ms));
            }
            etime = event.event_time();
        }
        3.max(Tracer::num_digits(max_time))
    }
}
impl fmt::Display for ThreadTrace {
    // port: Tracer.ThreadTrace#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let num_digits = self.get_max_digits();
        let mut result = String::new();
        let mut etime = -1;
        let mut indent = Vec::new();
        for event in &self.events {
            if self.pretty_print && !event.is_start && !indent.is_empty() {
                indent.pop();
            }
            result.push(' ');
            let event_indent = if self.pretty_print {
                indent.concat()
            } else {
                String::new()
            };
            result.push_str(&event.to_string(
                etime,
                &event_indent,
                if self.pretty_print { num_digits } else { 4 },
            ));
            etime = event.event_time();
            result.push('\n');
            if self.pretty_print && event.is_start {
                indent.push("|  ");
            }
        }
        if !self.outstanding_events.is_empty() {
            let now = now();
            result.push_str(" Unstopped timers:\n");
            for tracer in self.outstanding_events.values() {
                let tracer = tracer.lock().unwrap();
                writeln!(
                    result,
                    "  {} ({} ms, started at {})",
                    tracer,
                    now.wrapping_sub(tracer.start_time_ms),
                    Tracer::format_time(tracer.start_time_ms)
                )
                .unwrap();
            }
        }
        for (name, stat) in &self.stats {
            if stat.count > 1 {
                write!(
                    result,
                    " TOTAL {name} {} ({} ms",
                    stat.count, stat.clock_time
                )
                .unwrap();
                if let Some(info) = &stat.extra_info {
                    let extra = statistics();
                    for (i, value) in info.iter().enumerate() {
                        write!(result, "; {value} {}", extra[i].get_units()).unwrap();
                    }
                }
                result.push_str(")\n");
            }
        }
        f.write_str(&result)
    }
}

pub trait TracingStatistic: Send + Sync {
    // port: Tracer.TracingStatistic#start
    fn start(&self, thread: ThreadId) -> i64;
    // port: Tracer.TracingStatistic#stop
    fn stop(&self, thread: ThreadId) -> i64;
    // port: Tracer.TracingStatistic#enable
    fn enable(&self) -> bool;
    // port: Tracer.TracingStatistic#getTracingStat
    fn get_tracing_stat(&self) -> Option<Arc<AtomicTracerStatMap>>;
    // port: Tracer.TracingStatistic#getUnits
    fn get_units(&self) -> &str;
}
#[derive(Default)]
pub struct AtomicTracerStatMap {
    map: Arc<Mutex<IndexMap<String, Arc<AtomicI64>>>>,
}
impl AtomicTracerStatMap {
    // port: Tracer.AtomicTracerStatMap#incrementBy
    pub fn increment_by(&self, key: &str, delta: i64) {
        let mut map = self.map.lock().unwrap();
        let old = map.get(key).cloned();
        let entry = if let Some(old) = old {
            old
        } else {
            map.insert(key.to_owned(), Arc::new(AtomicI64::new(delta)));
            return;
        };
        drop(map);
        let mut old_value = entry.load(Ordering::SeqCst);
        loop {
            match entry.compare_exchange(
                old_value,
                old_value.wrapping_add(delta),
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(value) => old_value = value,
            }
        }
    }
    // port: Tracer.AtomicTracerStatMap#getMap
    pub fn get_map(&self) -> Arc<Mutex<IndexMap<String, Arc<AtomicI64>>>> {
        self.map.clone()
    }
}
