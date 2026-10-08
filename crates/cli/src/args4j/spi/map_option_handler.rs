/*
 * Copyright (c) 2013 Kohsuke Kawaguchi and other contributors
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy of
 * this software and associated documentation files (the "Software"), to deal in
 * the Software without restriction, including without limitation the rights to
 * use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
 * of the Software, and to permit persons to whom the Software is furnished to do
 * so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */
// Ported from args4j 2.33 (https://github.com/kohsuke/args4j):
//   org/kohsuke/args4j/spi/MapOptionHandler.java.

use super::{option_handler::ParsedValue, parameters::Parameters};
use crate::args4j::cmd_line_exception::CmdLineException;
// port: MapOptionHandler#addToMap
pub fn parse_arguments(params: &Parameters<'_>) -> Result<(ParsedValue, usize), CmdLineException> {
    let argument = params.get_parameter(0)?;
    let (key, value) = argument.split_once('=').ok_or_else(|| {
        CmdLineException("An argument for setting a Map must contain a \"=\"".into())
    })?;
    if key.is_empty() {
        return Err(CmdLineException("A key must be set.".into()));
    }
    Ok((
        ParsedValue::Map(
            key.into(),
            if value.is_empty() {
                None
            } else {
                Some(value.into())
            },
        ),
        1,
    ))
}
