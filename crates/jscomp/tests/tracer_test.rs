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

use closure_jscomp::tracer::{InternalClock, Tracer};
use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};
struct Clock(AtomicI64);
impl InternalClock for Clock {
    fn current_time_millis(&self) -> i64 {
        self.0.fetch_add(5, Ordering::SeqCst)
    }
}
#[test]
fn initialized_trace_report_matches_java() {
    Tracer::clear_current_thread_trace();
    let mut uninitialized = Tracer::new_comment(Some("uninitialized"));
    assert_eq!(uninitialized.stop(), 0);
    Tracer::clear_current_thread_trace();
    Tracer::set_clock_for_testing(Arc::new(Clock(AtomicI64::new(1000))));
    Tracer::set_pretty_print(true);
    Tracer::enable_type_maps();
    Tracer::init_current_thread_trace_with_threshold(0);
    let mut outer = Tracer::new(Some("A"), Some("outer"));
    let mut inner = Tracer::new(Some("A"), Some("inner"));
    assert_eq!(inner.stop(), 5);
    assert_eq!(outer.stop(), 15);
    let mut silent = Tracer::new(Some("A"), Some("silent"));
    assert_eq!(silent.stop_with_silence_threshold(10), 5);
    assert_eq!(
        Tracer::get_current_thread_trace_report(),
        include_str!("data/compiler_tracer_report.txt")
    );
    let stats = Tracer::get_stats_for_type("A");
    assert_eq!(stats.get_count(), 3);
    assert_eq!(stats.get_silent_count(), 1);
    assert_eq!(stats.get_total_time(), 25);
    let counts = Tracer::get_type_to_count_map().unwrap().get_map();
    assert_eq!(counts.lock().unwrap()["A"].load(Ordering::SeqCst), 3);
    Tracer::clear_current_thread_trace();
    Tracer::set_pretty_print(false);
}
