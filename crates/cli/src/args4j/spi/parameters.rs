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
//   org/kohsuke/args4j/CmdLineParser.java.

use crate::args4j::{cmd_line_exception::CmdLineException, messages};
pub struct Parameters<'a> {
    pub args: &'a [String],
    pub pos: usize,
    pub option_name: String,
}
impl Parameters<'_> {
    // port: CmdLineParser.CmdLineImpl#getParameter
    pub fn get_parameter(&self, index: isize) -> Result<&str, CmdLineException> {
        self.pos
            .checked_add_signed(index)
            .and_then(|i| self.args.get(i))
            .map(String::as_str)
            .ok_or_else(|| CmdLineException(messages::missing_operand(&self.option_name)))
    }
    // port: CmdLineParser.CmdLineImpl#size
    pub fn size(&self) -> usize {
        self.args.len() - self.pos
    }
}
