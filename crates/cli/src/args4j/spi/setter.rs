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
//   org/kohsuke/args4j/spi/MultiValueFieldSetter.java.

use super::option_handler::ParsedValue;
pub trait Setter {
    fn add_value(&mut self, index: usize, value: ParsedValue);
    fn print_default_value(&self, index: usize) -> Option<String>;
    fn add_argument(&mut self, value: String);
}

/// Reflection's String list field is represented by its declaration-order descriptor.
pub struct StringFieldSetter<'a> {
    pub field: &'a mut Vec<String>,
    pub option: &'static crate::args4j::named_option_def::NamedOptionDef,
}
impl StringFieldSetter<'_> {
    // port: MultiValueFieldSetter#isMultiValued
    pub fn is_multi_valued(&self) -> bool {
        self.option.multi_valued
    }
    // port: MultiValueFieldSetter#getType
    pub fn get_type(&self) -> std::any::TypeId {
        std::any::TypeId::of::<String>()
    }
    // port: MultiValueFieldSetter#addValue
    pub fn add_value(&mut self, value: String) {
        self.field.push(value);
    }
    // port: MultiValueFieldSetter#asFieldSetter
    pub fn as_field_setter(&self) -> Option<&Self> {
        Some(self)
    }
    // port: MultiValueFieldSetter#asAnnotatedElement
    pub fn as_annotated_element(&self) -> &crate::args4j::named_option_def::NamedOptionDef {
        self.option
    }
}
