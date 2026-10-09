/*
 * Copyright 2009 The Closure Compiler Authors.
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
//   src/com/google/javascript/jscomp/SourceMap.java.

#![allow(clippy::collapsible_if, clippy::unnecessary_unwrap)] // Retain Java nested conditionals and null checks.
use crate::{node_util::NodeUtil, source_file_mapping::SourceFileMapping};
use closure_rhino::fast_hash::IndexMap;
use closure_rhino::{
    js_string::JsString,
    node::{Ast, NodeId},
};
use closure_sourcemap::{
    file_position::FilePosition, proto::mapping::OriginalMapping,
    source_map_generator::SourceMapGenerator,
};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// An enumeration of available source map formats
pub enum Format {
    DEFAULT,
    V3,
}
impl Format {
    pub const VALUES: &'static [Self] = &[Self::DEFAULT, Self::V3];
    // port: SourceMap.Format#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
    // port: SourceMap.Format#getInstance
    // port: SourceMap.Format.DEFAULT#getInstance
    // port: SourceMap.Format.V3#getInstance
    pub fn get_instance(self) -> SourceMap {
        SourceMap::new(Box::new(
            closure_sourcemap::source_map_generator_v3::SourceMapGeneratorV3::new(),
        ))
    }
}
impl fmt::Display for Format {
    // port: SourceMap.Format#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Source maps can be very large different levels of detail can be specified.
pub enum DetailLevel {
    // ALL is best when the fullest details are needed for debugging or for
    // code-origin analysis.
    ALL,
    // SYMBOLS is intended to be used for stack trace deobfuscation when full
    // detail is not needed.
    SYMBOLS,
}
impl DetailLevel {
    pub const VALUES: &'static [Self] = &[Self::ALL, Self::SYMBOLS];
    // port: SourceMap.DetailLevel#valueOf
    pub fn value_of(name: &str) -> Option<Self> {
        Self::VALUES.iter().copied().find(|v| v.to_string() == name)
    }
    // port: SourceMap.DetailLevel.ALL#apply
    // port: SourceMap.DetailLevel.SYMBOLS#apply
    pub fn apply(self, ast: &Ast, node: NodeId) -> bool {
        match self {
            Self::ALL => true,
            Self::SYMBOLS => {
                node.is_call(ast)
                    || node.is_new(ast)
                    || node.is_function(ast)
                    || node.is_name(ast)
                    || NodeUtil::is_normal_or_opt_chain_get(ast, node)
                    || NodeUtil::may_be_object_lit_key(ast, node)
                    || node.is_tagged_template_lit(ast)
            }
        }
    }
}
impl fmt::Display for DetailLevel {
    // port: SourceMap.DetailLevel#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
/// Function that mape a "destination" location to use within the source map. Should return null if
/// the value is not mapped.
pub trait LocationMapping: Send + Sync + fmt::Display {
    /// @param location the location to transform
    /// @return the transformed location or null if not transformed
    // port: SourceMap.LocationMapping#map
    fn map(&self, location: &JsString) -> Option<JsString>;
    /// Rust-only: downcasting for Java's `Object#equals` on mappings (DESIGN.md, as `OpaqueProp`).
    fn as_any(&self) -> &dyn std::any::Any;
}
#[derive(Clone, Debug)]
/// Simple {@link LocationMapping} that strips a prefix from a location.
pub struct PrefixLocationMapping {
    pub prefix: JsString,
    pub replacement: JsString,
}
impl PrefixLocationMapping {
    // port: SourceMap.PrefixLocationMapping#PrefixLocationMapping
    pub fn new(prefix: impl Into<JsString>, replacement: impl Into<JsString>) -> Self {
        Self {
            prefix: prefix.into(),
            replacement: replacement.into(),
        }
    }
    // port: SourceMap.PrefixLocationMapping#hashCode
    pub fn hash_code(&self) -> i32 {
        31i32
            .wrapping_add(self.prefix.hash_code())
            .wrapping_mul(31)
            .wrapping_add(self.replacement.hash_code())
    }
}
impl LocationMapping for PrefixLocationMapping {
    // port: SourceMap.PrefixLocationMapping#map
    fn map(&self, location: &JsString) -> Option<JsString> {
        if location.starts_with(&self.prefix) {
            return Some(
                self.replacement
                    .concat(&location.substring_from(self.prefix.length())),
            );
        }
        None
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PartialEq for PrefixLocationMapping {
    // port: SourceMap.PrefixLocationMapping#equals
    fn eq(&self, other: &Self) -> bool {
        other.prefix == self.prefix && other.replacement == self.replacement
    }
}
impl Eq for PrefixLocationMapping {}
impl fmt::Display for PrefixLocationMapping {
    // port: SourceMap.PrefixLocationMapping#toString
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}|{})", self.prefix, self.replacement)
    }
}
#[derive(Clone, Debug)]
/// Maintains a mapping from a given node to the position in the source code at which its generated
/// form was placed. The positions are typically relative to the source file the node is located
/// in, but might be adjusted if that source is being concatenated to other sources.
pub struct Mapping {
    pub node: NodeId,
    pub start: FilePosition,
    pub end: Option<FilePosition>,
}
impl Mapping {
    // port: SourceMap.Mapping#toString
    pub fn to_string(&self, ast: &Ast) -> String {
        // This toString() representation is used for debugging purposes only.
        format!(
            "Mapping: start {}, end {}, node {}",
            self.start,
            self.end
                .map_or_else(|| "null".to_owned(), |end| end.to_string()),
            self.node.to_string(ast)
        )
    }
}
/// Collects information mapping the generated (compiled) source back to its original source for
/// debugging purposes.
///
/// @see CodeConsumer
/// @see CodeGenerator
/// @see CodePrinter
pub struct SourceMap {
    generator: Box<dyn SourceMapGenerator + Send>,
    prefix_mappings: Vec<Box<dyn LocationMapping>>,
    source_location_fixup_cache: IndexMap<JsString, JsString>,
    /// A mapping derived from input source maps. Maps back to input sources that inputs to this
    /// compilation job have been generated from, and used to create a source map that maps all the way
    /// back to original inputs. {@code null} if no such mapping is wanted.
    mapping: Option<Box<dyn SourceFileMapping + Send>>,
    /// Rust-only: the last source file name seen by `add_mapping_for_node`, and as a JS string.
    last_source_file_name: Option<(String, JsString)>,
}
impl SourceMap {
    // port: SourceMap#SourceMap
    pub fn new(generator: Box<dyn SourceMapGenerator + Send>) -> Self {
        Self {
            generator,
            prefix_mappings: Vec::new(),
            source_location_fixup_cache: IndexMap::<_, _>::default(),
            last_source_file_name: None,
            mapping: None,
        }
    }
    // port: SourceMap#addMapping(Mapping)
    pub fn add_mapping(&mut self, ast: &Ast, mapping: &Mapping) {
        self.add_mapping_for_node(ast, mapping.node, mapping.start, mapping.end);
    }
    // port: SourceMap#addMapping(Node,FilePosition,FilePosition)
    #[allow(clippy::unnecessary_unwrap)] // Retain Java null/default/mapped branches.
    pub fn add_mapping_for_node(
        &mut self,
        ast: &Ast,
        node: NodeId,
        output_start_position: FilePosition,
        output_end_position: Option<FilePosition>,
    ) {
        // If the node does not have an associated source file or
        // its line number is -1, then the node does not have sufficient
        // information for a mapping to be useful.
        let source_file = node.get_static_source_file(ast);
        if source_file.is_none() || node.get_lineno(ast) < 0 {
            return;
        }
        // Rust-only: the file name converted for the previous node is reused (D-025).
        let source_file = source_file.unwrap();
        if self
            .last_source_file_name
            .as_ref()
            .is_none_or(|(name, _)| name != source_file.get_name())
        {
            let name = source_file.get_name();
            self.last_source_file_name = Some((name.to_owned(), JsString::from(name)));
        }
        let mut source_file_name = self.last_source_file_name.as_ref().unwrap().1.clone();
        let mut line_no = node.get_lineno(ast);
        let mut char_no = node.get_charno(ast);
        let mut original_name = Self::get_original_name(ast, node);
        if let Some(mapping) = &self.mapping {
            let source_mapping = mapping.get_source_mapping(&source_file_name, line_no, char_no);
            if source_mapping.is_none() {
                // The source file does not have a input map. We consider this to be an
                // original source range and include it in the output map.
            } else if source_mapping.as_ref().unwrap() == OriginalMapping::get_default_instance() {
                // The source file does have an input map, but it does not map the
                // location. We consider this to be a synthetic code range and do not
                // include it in the output map.
                // TODO b/452676030 - Report the sourceless mapping.
                return;
            } else {
                // The source file mapped our code range to its original source location.
                let source_mapping = source_mapping.unwrap();
                source_file_name = source_mapping.get_original_file().to_owned();
                line_no = source_mapping.get_line_number();
                char_no = source_mapping.get_column_position();
                let identifier = source_mapping.get_identifier();
                if source_mapping.has_identifier() && !identifier.is_empty() {
                    original_name = Some(identifier.clone());
                }
            }
        }
        source_file_name = self.fixup_source_location(&source_file_name);
        // Rhino source lines are one based but for v3 source maps, we make
        // them zero based.
        let line_base_offset = 1;
        self.generator.add_mapping(
            Some(source_file_name),
            original_name,
            FilePosition::new(line_no - line_base_offset, char_no),
            output_start_position,
            output_end_position.expect("NullPointerException: source mapping end"),
        );
    }
    // port: SourceMap#addSourceFile
    pub fn add_source_file(&mut self, name: &JsString, code: Option<&JsString>) {
        let name = self.fixup_source_location(name);
        self.generator.add_sources_content(name, code.cloned());
    }
    // port: SourceMap#getOriginalName
    fn get_original_name(ast: &Ast, node: NodeId) -> Option<JsString> {
        if node.get_original_name(ast).is_some() {
            return node.get_original_name(ast);
        }
        // If this is the name identifier of a function declaration/expression and the name node itself
        // has no originalName, fall back to the enclosing FUNCTION node's originalName.
        // This covers functions that were originally anonymous (class methods compiled to prototype
        // assignments, `var f = function(){}` patterns): SourceInformationAnnotator skips empty-string
        // NAME nodes but always annotates the FUNCTION node. When the renaming pass later fills in the
        // NAME, the node's originalName remains null. The FUNCTION node's originalName is the correct
        // original name to use.
        if node.is_name(ast) {
            if let Some(parent) = node.get_parent(ast) {
                if parent.is_function(ast) && parent.get_first_child(ast) == Some(node) {
                    return parent.get_original_name(ast);
                }
            }
        }
        if node.is_member_function_def(ast) {
            return node.get_first_child(ast).unwrap().get_original_name(ast);
        }
        None
    }
    /// @param sourceFile The source file location to fixup.
    /// @return a remapped source file.
    // port: SourceMap#fixupSourceLocation
    fn fixup_source_location(&mut self, source_file: &JsString) -> JsString {
        if self.prefix_mappings.is_empty() {
            return source_file.to_owned();
        }
        let mut fixed = self.source_location_fixup_cache.get(source_file).cloned();
        if let Some(fixed) = fixed {
            return fixed;
        }
        // Replace the first prefix found with its replacement
        for mapping in &self.prefix_mappings {
            fixed = mapping.map(source_file);
            if fixed.is_some() {
                break;
            }
        }
        // If none of the mappings match then use the original file path.
        let fixed = fixed.unwrap_or_else(|| source_file.to_owned());
        self.source_location_fixup_cache
            .insert(source_file.to_owned(), fixed.clone());
        fixed
    }
    // port: SourceMap#appendTo
    pub fn append_to(
        &mut self,
        out: &mut dyn fmt::Write,
        name: impl Into<JsString>,
    ) -> fmt::Result {
        let name = self.fixup_source_location(&name.into());
        self.generator.append_to(out, Some(name)).map(|_| ())
    }
    // port: SourceMap#reset
    pub fn reset(&mut self) {
        self.generator.reset();
        self.source_location_fixup_cache.clear();
    }
    // port: SourceMap#setStartingPosition
    pub fn set_starting_position(&mut self, offset_line: i32, offset_index: i32) {
        self.generator
            .set_starting_position(offset_line, offset_index);
    }
    // port: SourceMap#setWrapperPrefix
    pub fn set_wrapper_prefix(&mut self, prefix: &JsString) {
        self.generator.set_wrapper_prefix(prefix.clone());
    }
    // port: SourceMap#validate
    pub fn validate(&mut self, validate: bool) {
        self.generator.validate(validate);
    }
    ///
    // port: SourceMap#setPrefixMappings
    #[allow(clippy::empty_docs)] // Retain Java's empty Javadoc.
    pub fn set_prefix_mappings(
        &mut self,
        source_map_location_mappings: Vec<Box<dyn LocationMapping>>,
    ) {
        self.prefix_mappings = source_map_location_mappings;
    }
    // port: SourceMap#setSourceFileMapping
    pub fn set_source_file_mapping(&mut self, mapping: Option<Box<dyn SourceFileMapping + Send>>) {
        self.mapping = mapping;
    }
}
