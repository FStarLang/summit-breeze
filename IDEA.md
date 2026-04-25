# Summit Breeze

An LSP server for [SMT-LIB](https://smtlib.cs.uiowa.edu/) files, with a VS Code extension for syntax highlighting and language support.

## Features

### LSP Server (Rust)
- **Full SMT-LIB v2.6 parser** with Z3 extension support
- **Go-to-definition** — jump to declarations of functions, constants, sorts, variables
- **Find references** — find all uses of a symbol in the document
- **Push/Pop navigation** — go-to-definition on `push` jumps to matching `pop` and vice versa
- **Document outline** — structured view of declarations, assertions, push/pop blocks
- **Folding** — fold top-level commands and push/pop blocks
- **Diagnostics** — real-time parse error reporting
- **Error recovery** — a single typo doesn't break the entire file
- **Scales to large files** — hand-written lexer and parser optimized for throughput

### VS Code Extension
- Syntax highlighting for `.smt2` and `.smt` files (TextMate grammar)
- Automatic LSP server startup via stdio transport
- Language registration for SMT-LIB

## Project Structure

```
summit-breeze/
├── crates/
│   ├── parser/          # smtlib-parser: lexer, AST, recursive descent parser
│   └── lsp/             # summit-breeze-lsp: LSP server binary
└── editors/
    └── vscode/          # VS Code extension
```

## Building

### Prerequisites
- Rust 1.75+ (2024 edition)
- Node.js 18+ (for the VS Code extension)

### LSP Server
```bash
cargo build --release -p summit-breeze-lsp
```

The binary is at `target/release/summit-breeze-lsp`.

### VS Code Extension
```bash
cd editors/vscode
npm install
npm run compile
```

## Running Tests

```bash
# All tests
cargo test

# Parser tests only
cargo test -p smtlib-parser

# LSP + integration tests
cargo test -p summit-breeze-lsp

# Performance benchmark
cargo run --release --example bench -p smtlib-parser
```

## Usage

### VS Code
1. Build the LSP server (`cargo build --release -p summit-breeze-lsp`)
2. Add `target/release/` to your PATH (or configure the extension)
3. Open a `.smt2` file in VS Code

### Other Editors
The LSP server communicates over stdio. Configure your editor's LSP client to launch `summit-breeze-lsp` with stdio transport.

## Supported SMT-LIB Commands

| Category | Commands |
|----------|----------|
| Logic | `set-logic`, `set-info`, `set-option` |
| Declarations | `declare-fun`, `declare-const`, `declare-sort`, `define-fun`, `define-sort` |
| Z3 Extensions | `declare-datatype(s)`, `define-fun-rec`, `define-funs-rec`, `lambda` |
| Assertions | `assert`, `check-sat`, `check-sat-assuming` |
| Stack | `push`, `pop`, `reset`, `reset-assertions` |
| Model | `get-model`, `get-value`, `get-proof`, `get-unsat-core` |
| Other | `echo`, `exit`, and unknown commands (gracefully parsed) |

## Design Decisions

- **Single-file model**: each document is independent — no cross-file references
- **Full reparse on edit**: fast enough for large files with the hand-written parser
- **Error recovery**: on parse error, skip to next balanced paren / next top-level command
- **Extensible grammar**: unknown commands are parsed as generic s-expressions

## License

MIT
