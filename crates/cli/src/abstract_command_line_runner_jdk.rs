/*
 * Copyright (c) 1994, 2022, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */
// Ported from OpenJDK 21 (the src.zip of Temurin-21.0.12.1+1): java.base/java/lang/Throwable.java.

//! `Throwable#printStackTrace` for [`RunnerException`], the stack-trace text Java prints for an
//! exception that escapes the command-line runner.

use super::{RunnerException, RunnerExceptionKind};
use std::io::Write;

impl RunnerException {
    // port: Throwable#printStackTrace
    pub fn print_stack_trace(&self, out: &mut dyn Write) -> std::io::Result<()> {
        self.print_enclosed_stack_trace(out, &[], false)
    }
    // port: Throwable#printEnclosedStackTrace
    fn print_enclosed_stack_trace(
        &self,
        out: &mut dyn Write,
        enclosing: &[String],
        is_cause: bool,
    ) -> std::io::Result<()> {
        if let RunnerExceptionKind::JavaException {
            class,
            frames,
            cause,
        } = &self.1
        {
            let common = frames
                .iter()
                .rev()
                .zip(enclosing.iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            write!(
                out,
                "{}{}",
                if is_cause { "Caused by: " } else { "" },
                class
            )?;
            if !self.0.is_empty() {
                write!(out, ": {}", self.0)?;
            }
            writeln!(out)?;
            for frame in &frames[..frames.len() - common] {
                writeln!(out, "\tat {frame}")?;
            }
            if common > 0 {
                writeln!(out, "\t... {common} more")?;
            }
            if let Some(cause) = cause {
                cause.print_enclosed_stack_trace(out, frames, true)?;
            }
        }
        Ok(())
    }
}
