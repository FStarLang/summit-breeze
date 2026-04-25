# Summit Breeze — Implementation Plan

## Problem Statement

Build an LSP server for SMT-LIB v2.6 files (with Z3 extensions) in Rust, plus a VS Code extension for syntax highlighting and LSP integration. The server must scale to files >100MB, support go-to-definition/references, folding, outline, and push/pop navigation.

## Approach

- **Monorepo**: Cargo workspace for Rust crates + VS Code extension as a Node/npm package
- **Parser**: Hand-written recursive descent parser (full ownership, extensible, comment-aware)
- **LSP**: `tower-lsp` for the async LSP transport layer
- **VS Code**: TypeScript extension that launches the native LSP binary, with TextMate grammar for syntax highlighting
- **WASM**: Deferred to post-MVP

## Project Structure

```
summit-breeze/
├── Cargo.toml                   # workspace root
├── crates/
│   ├── parser/                  # smtlib-parser: lexer + parser + AST
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── lexer.rs         # tokenizer (streaming, zero-copy where possible)
│   │       ├── ast.rs           # AST node types with spans
│   │       ├── parser.rs        # recursive descent parser
│   │       └── span.rs          # byte-offset span types
│   └── lsp/                     # summit-breeze-lsp: the LSP server binary
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs          # tower-lsp setup, stdio transport
│           ├── backend.rs       # LanguageServer trait impl
│           ├── document.rs      # per-document state (text, parsed AST, symbol index)
│           ├── symbols.rs       # symbol table: constants, functions, sorts, locals
│           ├── folding.rs       # folding ranges (s-expr + push/pop)
│           ├── navigation.rs    # go-to-def, references, push↔pop
│           └── outline.rs       # document symbols for outline
└── editors/
    └── vscode/
        ├── package.json
        ├── tsconfig.json
        ├── src/
        │   └── extension.ts     # activate: spawn LSP binary, create LanguageClient
        └── syntaxes/
            └── smtlib.tmLanguage.json
```

## Phases & Todos

### Phase 1: Project Scaffolding
- **scaffold-workspace**: Set up Cargo workspace, crate skeletons, VS Code extension skeleton, `.gitignore`, basic CI
- **scaffold-vscode**: Initialize VS Code extension with `package.json`, `tsconfig.json`, basic activation

### Phase 2: Lexer
- **lexer-core**: Implement a streaming lexer that tokenizes SMT-LIB v2.6 input
  - Token types: `(`, `)`, numeral, decimal, hex, binary, string literal, symbol (simple + quoted), keyword, reserved words, comments
  - Each token carries a `Span` (byte offsets)
  - Must handle >100MB files without loading entire file into a single string (use rope or chunked approach if needed; for MVP a single `String` with byte-offset indexing is acceptable since 100MB fits in memory)
  - Z3 extensions: `!` attributes, indexed identifiers `(_ name idx+)`, as-array, etc.
- **lexer-tests**: Comprehensive tests for all token types, edge cases (nested quotes, escaped chars, large numerals)

### Phase 3: Parser & AST
- **ast-types**: Define AST node types with spans for all SMT-LIB v2.6 commands
  - Commands: `set-logic`, `set-info`, `set-option`, `declare-sort`, `define-sort`, `declare-fun`, `define-fun`, `declare-const`, `assert`, `check-sat`, `push`, `pop`, `get-model`, `get-value`, `get-proof`, `get-unsat-core`, `exit`, etc.
  - Z3 extensions: `declare-datatypes`, `declare-datatype`, `define-fun-rec`, `define-funs-rec`, `forall`/`exists` with patterns, etc.
  - Terms: qualified identifiers, applications, let-bindings, quantifiers, match, annotated terms
  - Sorts: simple, parameterized, indexed
- **parser-core**: Recursive descent parser producing AST with error recovery
  - On error: skip to next balanced paren / next top-level command
  - Collect diagnostics (with spans) for the LSP to report
  - Must be incremental-friendly: for MVP, full reparse on each edit is fine (fast enough for 100MB with a good lexer)
- **parser-tests**: Round-trip and snapshot tests on real SMT-LIB files

### Phase 4: LSP Server Core
- **lsp-transport**: Set up `tower-lsp` with stdio transport, initialize/shutdown lifecycle
- **document-store**: `DashMap<Url, Document>` holding source text + parsed AST + symbol index, updated on `didOpen`/`didChange`/`didClose`
- **diagnostics**: Publish parser diagnostics on each document change

### Phase 5: LSP Features
- **symbols-index**: Build a per-document symbol table from the AST
  - Declarations: `declare-fun`, `declare-const`, `declare-sort`, `define-fun`, `define-sort`, `declare-datatypes`
  - Local scopes: `let`-bindings, quantifier-bound variables
  - Track push/pop stack depth for each declaration
- **goto-def**: Resolve symbol under cursor → declaration site (including local scopes)
- **find-refs**: For a declaration, find all uses in the document
- **push-pop-nav**: Go-to-definition on a `push` jumps to matching `pop` and vice versa
- **folding**: Folding ranges for every top-level s-expression, and push/pop blocks
- **outline**: Document symbols for outline view — top-level commands, grouped by push/pop depth

### Phase 6: VS Code Extension
- **vscode-extension**: TypeScript extension that spawns the LSP binary and connects via stdio using `vscode-languageclient`
- **syntax-highlighting**: TextMate grammar for SMT-LIB v2.6 — keywords, literals, comments, sorts, parentheses
- **extension-packaging**: `vsce package`, bundling the appropriate native binary

### Phase 7: Testing & Polish
- **integration-tests**: End-to-end tests: send LSP requests, verify responses (using `tower-lsp`'s testing utilities or a custom test harness)
- **perf-benchmark**: Benchmark parsing a 100MB+ SMT file, ensure <5s cold-open
- **docs**: README with usage instructions, supported features, build instructions

## Key Design Decisions

1. **Single-file model**: No cross-file references. Each document is independent. This simplifies the architecture significantly.
2. **Full reparse on edit (MVP)**: For files up to 100MB, a fast lexer + parser in Rust should reparse in <1s. Incremental parsing is a future optimization.
3. **Symbol scoping**: Push/pop creates implicit scope boundaries. Declarations after a `push` are conceptually scoped to that level, but SMT-LIB doesn't formally scope names this way — we track stack depth as metadata but don't restrict references by scope.
4. **Error recovery**: The parser skips to the next balanced top-level form on error, so a single typo doesn't break the entire file's analysis.
5. **Z3 extensions**: Treated as first-class citizens in the grammar, not bolted on. Unknown commands are parsed as generic s-expressions to gracefully handle future extensions.

## Post-MVP (not in this plan)

- WASM bundling for VS Code web
- Autocompletion
- Type checking
- Linting (unused constants/sorts/variables, quantifier pattern checks)
- Incremental parsing for large files
- Comment-aware parsing (tool-specific metadata in comments)
