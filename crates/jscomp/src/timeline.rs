/*
 * Copyright 2017 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/Timeline.java,
//   test/com/google/javascript/jscomp/TimelineTest.java.

use closure_rhino::fast_hash::IndexMap;
use closure_rhino::js_string::JsString;
use std::hash::{Hash, Hasher};

struct Event<T> {
    next_event: Option<usize>,
    previous_event: Option<usize>,
    value: EventValue<T>,
}

impl<T> Event<T> {
    // port: Timeline.Event#Event
    fn new(value: EventValue<T>) -> Self {
        Self {
            next_event: None,
            previous_event: None,
            value,
        }
    }
}

#[derive(Clone)]
struct Time {
    name: String,
}

impl Time {
    // port: Timeline.Time#Time
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
        }
    }
}

impl PartialEq for Time {
    // port: Timeline.Time#equals
    fn eq(&self, an_object: &Self) -> bool {
        self.name == an_object.name
    }
}

impl Eq for Time {}

impl Hash for Time {
    // port: Timeline.Time#hashCode
    fn hash<H: Hasher>(&self, state: &mut H) {
        JsString::from(self.name.as_str()).hash_code().hash(state);
    }
}

enum EventValue<T> {
    Time(Time),
    Value(T),
}

/// Java Event identities are indices into a never-shrinking list.
pub struct Timeline<T> {
    events_by_time: IndexMap<Time, usize>,
    events_by_value: IndexMap<T, usize>,
    head_event: usize,
    events: Vec<Event<T>>,
}

impl<T: Clone + Eq + Hash> Timeline<T> {
    pub fn new() -> Self {
        Self {
            events_by_time: IndexMap::<_, _>::default(),
            events_by_value: IndexMap::<_, _>::default(),
            head_event: 0,
            events: vec![Event::new(EventValue::Time(Time::new("-beginning-")))],
        }
    }

    // port: Timeline#add
    pub fn add(&mut self, value: T) {
        self.add_event(EventValue::Value(value));
    }

    // port: Timeline#remove
    pub fn remove(&mut self, value: &T) {
        let event = self.events_by_value.shift_remove(value);
        if let Some(event) = event {
            let next_event = self.events[event].next_event;
            let previous_event = self.events[event].previous_event;
            if let Some(next_event) = next_event {
                self.events[next_event].previous_event = previous_event;
            } else {
                self.head_event = previous_event.expect("");
            }
            self.events[previous_event.expect("")].next_event = next_event;
            self.events[event].next_event = None;
            self.events[event].previous_event = None;
        }
    }

    // port: Timeline#mark
    pub fn mark(&mut self, time_name: &str) {
        self.add_event(EventValue::Time(Time::new(time_name)));
    }

    // port: Timeline#getSince
    pub fn get_since(&self, time_name: &str) -> Option<Vec<T>> {
        let mut values = Vec::new();
        let first_event = self.events_by_time.get(&Time::new(time_name)).copied()?;
        let mut event = Some(first_event);
        while let Some(current) = event {
            if let EventValue::Value(value) = &self.events[current].value {
                values.push(value.clone());
            }
            event = self.events[current].next_event;
        }
        Some(values)
    }

    // port: Timeline#addEvent
    fn add_event(&mut self, value: EventValue<T>) {
        let event = match &value {
            EventValue::Time(time) => self.events_by_time.get(time).copied(),
            EventValue::Value(value) => self.events_by_value.get(value).copied(),
        };
        if Some(self.head_event) == event {
            return;
        }
        let event = if let Some(event) = event {
            let previous_event = self.events[event].previous_event.expect("");
            let next_event = self.events[event].next_event.expect("");
            self.events[previous_event].next_event = Some(next_event);
            self.events[next_event].previous_event = Some(previous_event);
            self.events[event].next_event = None;
            event
        } else {
            let event = self.events.len();
            match &value {
                EventValue::Time(time) => {
                    self.events_by_time.insert(time.clone(), event);
                }
                EventValue::Value(value) => {
                    self.events_by_value.insert(value.clone(), event);
                }
            }
            self.events.push(Event::new(value));
            event
        };
        self.events[event].previous_event = Some(self.head_event);
        self.events[self.head_event].next_event = Some(event);
        self.head_event = event;
    }
}

impl<T: Clone + Eq + Hash> Default for Timeline<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Timeline;
    use std::hash::{Hash, Hasher};

    #[derive(Clone, Copy, Debug)]
    struct Event {
        hashcode: i32,
    }
    impl Event {
        // port: TimelineTest.Event#Event
        const fn new(hashcode: i32) -> Self {
            Self { hashcode }
        }
    }
    impl Hash for Event {
        // port: TimelineTest.Event#hashCode
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.hashcode.hash(state);
        }
    }
    impl PartialEq for Event {
        // port: TimelineTest.Event#equals
        fn eq(&self, obj: &Self) -> bool {
            obj.hashcode == self.hashcode
        }
    }
    impl Eq for Event {}
    const ATE_GREEN_EGGS: Event = Event::new(1);
    const ATE_HAM: Event = Event::new(2);
    const ATE_HASHBROWNS: Event = Event::new(3);

    // port: TimelineTest#testRemove_simple
    #[test]
    fn test_remove_simple() {
        let mut timeline = Timeline::new();
        timeline.mark("Monday");
        timeline.add(ATE_GREEN_EGGS);
        timeline.remove(&ATE_GREEN_EGGS);
        assert_eq!(timeline.get_since("Monday"), Some(Vec::new()));
    }
    // port: TimelineTest#testRemove_maintainsOrder
    #[test]
    fn test_remove_maintains_order() {
        let mut timeline = Timeline::new();
        timeline.mark("Monday");
        timeline.add(ATE_GREEN_EGGS);
        timeline.add(ATE_HAM);
        timeline.add(ATE_HASHBROWNS);
        timeline.remove(&ATE_HAM);
        assert_eq!(
            timeline.get_since("Monday"),
            Some(vec![ATE_GREEN_EGGS, ATE_HASHBROWNS])
        );
    }
    // port: TimelineTest#testUnknownTimesReturnNull
    #[test]
    fn test_unknown_times_return_null() {
        let timeline = Timeline::<Event>::new();
        assert_eq!(timeline.get_since("Monday"), None);
    }
    // port: TimelineTest#testDoesntReturnValuesBeforeTime
    #[test]
    fn test_doesnt_return_values_before_time() {
        let mut timeline = Timeline::new();
        timeline.add(ATE_GREEN_EGGS);
        timeline.mark("Monday");
        assert_eq!(timeline.get_since("Monday"), Some(Vec::new()));
    }
    // port: TimelineTest#testReturnsValuesAfterTime
    #[test]
    fn test_returns_values_after_time() {
        let mut timeline = Timeline::new();
        timeline.mark("Monday");
        timeline.add(ATE_GREEN_EGGS);
        assert_eq!(timeline.get_since("Monday"), Some(vec![ATE_GREEN_EGGS]));
    }
    // port: TimelineTest#testMaintainsOrder1
    #[test]
    fn test_maintains_order1() {
        let mut timeline = Timeline::new();
        timeline.mark("Monday");
        timeline.add(ATE_GREEN_EGGS);
        timeline.add(ATE_HAM);
        assert_eq!(
            timeline.get_since("Monday"),
            Some(vec![ATE_GREEN_EGGS, ATE_HAM])
        );
    }
    // port: TimelineTest#testMaintainsOrder2
    #[test]
    fn test_maintains_order2() {
        let mut timeline = Timeline::new();
        timeline.mark("Monday");
        timeline.add(ATE_GREEN_EGGS);
        timeline.add(ATE_GREEN_EGGS);
        timeline.add(ATE_HAM);
        assert_eq!(
            timeline.get_since("Monday"),
            Some(vec![ATE_GREEN_EGGS, ATE_HAM])
        );
    }
    // port: TimelineTest#testMaintainsOrder3
    #[test]
    fn test_maintains_order3() {
        let mut timeline = Timeline::new();
        timeline.mark("Monday");
        timeline.add(ATE_GREEN_EGGS);
        timeline.add(ATE_HAM);
        timeline.add(ATE_GREEN_EGGS);
        assert_eq!(
            timeline.get_since("Monday"),
            Some(vec![ATE_HAM, ATE_GREEN_EGGS])
        );
    }
    // port: TimelineTest#testMaintainsOrder4
    #[test]
    fn test_maintains_order4() {
        let mut timeline = Timeline::new();
        timeline.mark("Monday");
        timeline.add(ATE_GREEN_EGGS);
        timeline.add(ATE_HAM);
        timeline.add(ATE_GREEN_EGGS);
        timeline.add(ATE_HAM);
        assert_eq!(
            timeline.get_since("Monday"),
            Some(vec![ATE_GREEN_EGGS, ATE_HAM])
        );
    }
    // port: TimelineTest#testUsesEqualityAndHashcode
    #[test]
    fn test_uses_equality_and_hashcode() {
        let mut timeline = Timeline::new();
        let ate_berries = Event::new(10);
        let ate_grapes = Event::new(10);
        timeline.mark("Monday");
        timeline.add(ate_berries);
        timeline.add(ate_grapes);
        assert_eq!(timeline.get_since("Monday"), Some(vec![ate_berries]));
        assert_eq!(timeline.get_since("Monday"), Some(vec![ate_grapes]));
    }
    // port: TimelineTest#testUpdatesExistingTimes
    #[test]
    fn test_updates_existing_times() {
        let mut timeline = Timeline::new();
        timeline.mark("Monday");
        timeline.add(ATE_GREEN_EGGS);
        timeline.mark("Monday");
        assert_eq!(timeline.get_since("Monday"), Some(Vec::new()));
    }
    // port: TimelineTest#testManagesDifferentTimes
    #[test]
    fn test_manages_different_times() {
        let mut timeline = Timeline::new();
        timeline.mark("Monday");
        timeline.add(ATE_GREEN_EGGS);
        timeline.mark("Thursday");
        timeline.add(ATE_HAM);
        assert_eq!(
            timeline.get_since("Monday"),
            Some(vec![ATE_GREEN_EGGS, ATE_HAM])
        );
        assert_eq!(timeline.get_since("Thursday"), Some(vec![ATE_HAM]));
    }
}
