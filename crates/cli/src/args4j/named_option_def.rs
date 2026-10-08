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
//   org/kohsuke/args4j/NamedOptionDef.java.

use super::spi::option_handler::HandlerKind;
#[derive(Clone, Debug)]
pub struct NamedOptionDef {
    pub index: usize,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub usage: &'static str,
    pub meta_var: &'static str,
    pub hidden: bool,
    pub required: bool,
    pub help: bool,
    pub depends: &'static [&'static str],
    pub forbids: &'static [&'static str],
    pub handler: HandlerKind,
    pub multi_valued: bool,
}
impl std::fmt::Display for NamedOptionDef {
    // port: NamedOptionDef#toString
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)?;
        if !self.aliases.is_empty() {
            write!(f, " ({})", self.aliases.join(", "))?;
        }
        Ok(())
    }
}
