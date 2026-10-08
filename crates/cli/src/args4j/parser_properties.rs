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
//   org/kohsuke/args4j/ParserProperties.java.

#[derive(Clone, Debug)]
pub struct ParserProperties {
    pub usage_width: usize,
    pub sort_options: bool,
    pub option_value_delimiter: String,
    pub at_syntax: bool,
    pub show_defaults: bool,
}
impl Default for ParserProperties {
    // port: ParserProperties#defaults
    fn default() -> Self {
        Self {
            usage_width: 80,
            sort_options: true,
            option_value_delimiter: " ".into(),
            at_syntax: true,
            show_defaults: true,
        }
    }
}
impl ParserProperties {
    // port: ParserProperties#withUsageWidth
    pub fn with_usage_width(&mut self, width: usize) -> &mut Self {
        self.usage_width = width;
        self
    }
    // port: ParserProperties#withAtSyntax
    pub fn with_at_syntax(&mut self, value: bool) -> &mut Self {
        self.at_syntax = value;
        self
    }
    // port: ParserProperties#withShowDefaults
    pub fn with_show_defaults(&mut self, value: bool) -> &mut Self {
        self.show_defaults = value;
        self
    }
    // port: ParserProperties#withOptionValueDelimiter
    pub fn with_option_value_delimiter(&mut self, value: String) -> &mut Self {
        self.option_value_delimiter = value;
        self
    }
}
