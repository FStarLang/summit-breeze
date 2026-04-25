# Summit Breeze

<p align="center">
  <img src="logo.svg" width="128" height="128" alt="Summit Breeze logo">
</p>

An LSP server for [SMT-LIB](https://smtlib.cs.uiowa.edu/) files, with a VS Code extension for syntax highlighting and language support.

## Features

### LSP Server (Rust)
- **Full SMT-LIB v2.6 parser** with Z3 extension support
- **Go-to-definition** — jump to declarations of functions, constants, sorts, variables, and local bindings (forall/exists/let)
- **Find references** — scope-aware: only shows references to the specific definition under cursor
- **Hover** — shows the source definition of a symbol
- **Push/Pop navigation** — go-to-definition on `push` jumps to matching `pop` and vice versa
- **Document outline** — structured view of declarations, assertions, push/pop blocks
- **Folding** — fold top-level commands and push/pop blocks
- **Diagnostics** — real-time parse error reporting
- **Error recovery** — a single typo doesn't break the entire file
- **Scales to large files** — hand-written lexer and parser optimized for throughput

### VS Code Extension
- Syntax highlighting for `.smt2` and `.smt` files (TextMate grammar)
- Automatic LSP server startup (bundled binary, no separate install needed)
- Bracket matching, auto-closing pairs, and SMT-LIB-aware word selection
- Configurable server path for development/debugging

## Installation

### From a Release (recommended)

Download the `.vsix` for your platform from the [Releases](https://github.com/FStarLang/summit-breeze/releases) page, then:

```bash
code --install-extension summit-breeze-<platform>-<version>.vsix
```

The extension bundles the LSP server binary — no additional setup required.

### From Source

```bash
# Clone
git clone https://github.com/FStarLang/summit-breeze.git
cd summit-breeze

# Build and package the VSIX for your platform
./package.sh

# Install the resulting .vsix
code --install-extension editors/vscode/summit-breeze-*.vsix
```

#### Prerequisites
- Rust 1.75+ (2024 edition)
- Node.js 18+ (for the VS Code extension)

### Development Setup

For development, you can skip the VSIX and run the LSP binary directly:

```bash
cargo build --release --bin summit-breeze-lsp
```

Then either add `target/release/` to your PATH (the extension falls back to PATH lookup), or set the `summitBreeze.serverPath` setting in VS Code to the binary path.

## Configuration

| Setting | Description |
|---------|-------------|
| `summitBreeze.serverPath` | Path to a custom `summit-breeze-lsp` binary. Leave empty to use the bundled binary. |

## Project Structure

```
summit-breeze/
├── crates/
│   ├── parser/          # smtlib-parser: lexer, AST, recursive descent parser
│   └── lsp/             # summit-breeze-lsp: LSP server binary
├── editors/
│   └── vscode/          # VS Code extension
└── package.sh           # Build + package script
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

## Other Editors

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

Apache-2.0 — see [LICENSE](LICENSE) for details.
