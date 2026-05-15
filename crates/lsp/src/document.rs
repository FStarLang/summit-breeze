// Copyright 2026 Microsoft Research
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use dashmap::DashMap;
use smtlib_parser::ast::Script;
use smtlib_parser::parser::Diagnostic;
use tower_lsp::lsp_types::Url;

use crate::symbols::SymbolIndex;

/// Per-document state.
pub struct Document {
    pub text: String,
    #[allow(dead_code)]
    pub script: Script,
    pub diagnostics: Vec<Diagnostic>,
    pub index: SymbolIndex,
    /// Line start byte offsets (for position conversion).
    pub line_starts: Vec<u32>,
}

impl Document {
    pub fn new(text: String) -> Self {
        let line_starts = compute_line_starts(&text);
        let result = smtlib_parser::parse(&text);
        let index = crate::symbols::build_index(&result.script);
        Document {
            text,
            script: result.script,
            diagnostics: result.diagnostics,
            index,
            line_starts,
        }
    }

    /// Convert a byte offset to an LSP Position (0-based line and character).
    pub fn offset_to_position(&self, offset: u32) -> tower_lsp::lsp_types::Position {
        let line = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let col = offset.saturating_sub(self.line_starts[line]);
        tower_lsp::lsp_types::Position::new(line as u32, col)
    }

    /// Convert an LSP Position to a byte offset.
    pub fn position_to_offset(&self, pos: tower_lsp::lsp_types::Position) -> u32 {
        let line = pos.line as usize;
        if line < self.line_starts.len() {
            self.line_starts[line] + pos.character
        } else {
            self.text.len() as u32
        }
    }

    /// Convert a parser Span to an LSP Range.
    pub fn span_to_range(&self, span: smtlib_parser::span::Span) -> tower_lsp::lsp_types::Range {
        tower_lsp::lsp_types::Range::new(
            self.offset_to_position(span.start),
            self.offset_to_position(span.end),
        )
    }
}

/// Compute line start byte offsets.
fn compute_line_starts(text: &str) -> Vec<u32> {
    let mut starts = vec![0u32];
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            starts.push((i + 1) as u32);
        }
    }
    starts
}

/// Thread-safe document store.
pub struct DocumentStore {
    docs: DashMap<Url, Document>,
}

impl Default for DocumentStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            docs: DashMap::new(),
        }
    }

    pub fn open(&self, uri: Url, text: String) {
        self.docs.insert(uri, Document::new(text));
    }

    pub fn update(&self, uri: &Url, text: String) {
        self.docs.insert(uri.clone(), Document::new(text));
    }

    pub fn close(&self, uri: &Url) {
        self.docs.remove(uri);
    }

    pub fn get(&self, uri: &Url) -> Option<dashmap::mapref::one::Ref<'_, Url, Document>> {
        self.docs.get(uri)
    }
}
