# Summit Breeze

Summit Breeze is an LSP server for SMT files.  It also comes with a VS Code extension.

## LSP server

- writen in rust
- needs to support all Z3 extensions
  - should gracefully accept new extensions
- needs to scale to files >100 megabytes
- files are completely separate, don't care about cross-referencing multiple files
- go-to-definition/find-references for constants/functions, sorts, local variables
- folding support (with push/pop)
- go-to-definition to switch between corresponding push<->pop
- outline support
- LSP stack should be off-the-shelf rust crates
- I want to own the whole SMT-LIB parsing stack
  - we need to be free to add custom extensions if we want
  - we might want to parse comments as well in the future (our tool adds some grouping and extra infos in the comments)

## VS Code extension

- sets up LSP server
- would be cool if we could bundle the LSP server as a wasm executable
  - bundling binaries for Linux/x86_64, Linux/aarch64, Windows/x64, Mac/arm64 is acceptable if performance doesn't work out
- syntax highlighting

## Post-MVP features

- autocompletion
- type checking
- linting
  - unused constants
  - unused sorts
  - unused variables in quantifiers/let-bindings
  - quantifier patterns that don't occur in the formula