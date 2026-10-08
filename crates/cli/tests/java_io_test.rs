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

use closure_cli::java_io::{EncodedWriter, OutputWriter, PrintStream};
use closure_rhino::java_lang::charset::Charset;
use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;

#[derive(Default)]
struct Events {
    bytes: Vec<u8>,
    writes: usize,
    flushes: usize,
    closes: usize,
}
struct Output {
    events: Rc<RefCell<Events>>,
    fail_write: bool,
    fail_flush: bool,
}
impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut events = self.events.borrow_mut();
        events.writes += 1;
        if self.fail_write {
            return Err(io::Error::other("write failed"));
        }
        events.bytes.extend(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.events.borrow_mut().flushes += 1;
        if self.fail_flush {
            Err(io::Error::other("flush failed"))
        } else {
            Ok(())
        }
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        self.events.borrow_mut().closes += 1;
    }
}

#[test]
fn print_stream_suppresses_write_and_flush_io_exceptions() {
    for (fail_write, fail_flush) in [(true, false), (false, true), (true, true)] {
        let events = Rc::new(RefCell::new(Events::default()));
        let mut stream = PrintStream::new(Box::new(Output {
            events: events.clone(),
            fail_write,
            fail_flush,
        }));
        stream.write_all(b"output").unwrap();
        stream.flush().unwrap();
        assert!(stream.check_error());
        stream.close();
        stream.close();
        assert_eq!(events.borrow().closes, 1);
        stream.write_all(b"closed").unwrap();
        assert!(stream.check_error());
        assert_eq!(events.borrow().writes, 1);
    }
}

#[test]
fn encoded_writer_closes_shared_print_stream_on_normal_completion() {
    let events = Rc::new(RefCell::new(Events::default()));
    let mut stream = PrintStream::new(Box::new(Output {
        events: events.clone(),
        fail_write: false,
        fail_flush: false,
    }));
    let mut writer = EncodedWriter::for_print_stream(stream.clone(), Charset::UTF_8);
    writer.write_all(b"output").unwrap();
    writer.flush().unwrap();
    writer.close().unwrap();
    assert_eq!(events.borrow().bytes, b"output");
    assert_eq!(events.borrow().closes, 1);
    stream.write_all(b"after close").unwrap();
    assert!(stream.check_error());
    assert_eq!(events.borrow().bytes, b"output");
}

#[test]
fn dropping_unclosed_writer_during_exception_keeps_print_stream_open() {
    let events = Rc::new(RefCell::new(Events::default()));
    let mut stream = PrintStream::new(Box::new(Output {
        events: events.clone(),
        fail_write: false,
        fail_flush: false,
    }));
    {
        let mut writer = EncodedWriter::for_print_stream(stream.clone(), Charset::UTF_8);
        writer.write_all(b"buffered").unwrap();
    }
    assert_eq!(events.borrow().closes, 0);
    stream.write_all(b"exception handler").unwrap();
    assert!(!stream.check_error());
    assert_eq!(events.borrow().bytes, b"exception handler");
}
