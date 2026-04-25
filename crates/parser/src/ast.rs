// AST node types — to be implemented
use crate::span::Span;

/// A spanned AST node.
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

/// A complete SMT-LIB script.
#[derive(Debug, Clone)]
pub struct Script {
    pub commands: Vec<Spanned<Command>>,
}

/// Top-level SMT-LIB commands.
#[derive(Debug, Clone)]
pub enum Command {
    /// Placeholder — will be expanded in the parser phase.
    Unknown(String),
}
