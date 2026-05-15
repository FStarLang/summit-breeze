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

// AST node types — to be implemented
use crate::span::Span;

/// A spanned AST node.
#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }
}

/// A complete SMT-LIB script.
#[derive(Debug, Clone)]
pub struct Script {
    pub commands: Vec<Spanned<Command>>,
}

// ---------------------------------------------------------------------------
// Identifiers and sorts
// ---------------------------------------------------------------------------

/// A symbol (unquoted or quoted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub quoted: bool,
}

/// An index in an indexed identifier: `(_ sym idx+)`
#[derive(Debug, Clone)]
pub enum Index {
    Numeral(u64),
    Symbol(Symbol),
}

/// An identifier, possibly indexed: `sym` or `(_ sym idx+)`
#[derive(Debug, Clone)]
pub enum Identifier {
    Simple(Symbol),
    Indexed(Symbol, Vec<Index>),
}

/// A qualified identifier: `id` or `(as id sort)`
#[derive(Debug, Clone)]
pub enum QualifiedIdentifier {
    Simple(Spanned<Identifier>),
    As(Spanned<Identifier>, Spanned<Sort>),
}

/// A sort: `id` or `(id sort+)`
#[derive(Debug, Clone)]
pub enum Sort {
    Simple(Identifier),
    Parameterized(Identifier, Vec<Spanned<Sort>>),
}

// ---------------------------------------------------------------------------
// Terms
// ---------------------------------------------------------------------------

/// A sorted variable: `(sym sort)`
#[derive(Debug, Clone)]
pub struct SortedVar {
    pub name: Spanned<Symbol>,
    pub sort: Spanned<Sort>,
}

/// A variable binding in a let: `(sym term)`
#[derive(Debug, Clone)]
pub struct VarBinding {
    pub name: Spanned<Symbol>,
    pub value: Spanned<Term>,
}

/// A match pattern.
#[derive(Debug, Clone)]
pub enum MatchPattern {
    /// A simple symbol pattern.
    Symbol(Symbol),
    /// A constructor pattern: `(ctor var*)`
    Application(Symbol, Vec<Symbol>),
}

/// A match case: `(pattern term)`
#[derive(Debug, Clone)]
pub struct MatchCase {
    pub pattern: Spanned<MatchPattern>,
    pub body: Spanned<Term>,
}

/// An attribute value in annotations.
#[derive(Debug, Clone)]
pub enum AttributeValue {
    None,
    Constant(String),
    Symbol(Symbol),
    SExpr(Vec<Spanned<SExpr>>),
}

/// An attribute: `:keyword` optionally followed by a value.
#[derive(Debug, Clone)]
pub struct Attribute {
    pub keyword: Spanned<String>,
    pub value: Option<Spanned<AttributeValue>>,
}

/// SMT-LIB terms.
#[derive(Debug, Clone)]
pub enum Term {
    /// A constant literal (numeral, decimal, hex, binary, string).
    Constant(Constant),
    /// A qualified identifier used as a term.
    QualifiedIdentifier(QualifiedIdentifier),
    /// Function application: `(f arg+)`
    Application(Spanned<QualifiedIdentifier>, Vec<Spanned<Term>>),
    /// Let binding: `(let ((x t)+) body)`
    Let(Vec<Spanned<VarBinding>>, Box<Spanned<Term>>),
    /// Universal quantifier: `(forall ((x S)+) body)`
    Forall(Vec<Spanned<SortedVar>>, Box<Spanned<Term>>),
    /// Existential quantifier: `(exists ((x S)+) body)`
    Exists(Vec<Spanned<SortedVar>>, Box<Spanned<Term>>),
    /// Pattern match: `(match t ((pat body)+))`
    Match(Box<Spanned<Term>>, Vec<Spanned<MatchCase>>),
    /// Annotated term: `(! t :attr+)`
    Annotated(Box<Spanned<Term>>, Vec<Attribute>),
    /// Z3 lambda: `(lambda ((x S)+) body)`
    Lambda(Vec<Spanned<SortedVar>>, Box<Spanned<Term>>),
}

/// Literal constants.
#[derive(Debug, Clone)]
pub enum Constant {
    Numeral(u64),
    Decimal(String),
    Hexadecimal(String),
    Binary(String),
    String(String),
}

// ---------------------------------------------------------------------------
// S-expressions (for attribute values, get-info responses, etc.)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum SExpr {
    Constant(Constant),
    Symbol(Symbol),
    Keyword(String),
    List(Vec<Spanned<SExpr>>),
}

// ---------------------------------------------------------------------------
// Datatype declarations (Z3 / SMT-LIB 2.6)
// ---------------------------------------------------------------------------

/// A constructor declaration: `(name (field sort)*)`
#[derive(Debug, Clone)]
pub struct ConstructorDec {
    pub name: Spanned<Symbol>,
    pub selectors: Vec<SelectorDec>,
}

/// A selector: `(name sort)`
#[derive(Debug, Clone)]
pub struct SelectorDec {
    pub name: Spanned<Symbol>,
    pub sort: Spanned<Sort>,
}

/// A datatype declaration body: list of constructors.
#[derive(Debug, Clone)]
pub struct DatatypeDec {
    /// Sort parameters (for parametric datatypes).
    pub params: Vec<Spanned<Symbol>>,
    pub constructors: Vec<Spanned<ConstructorDec>>,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// A function definition body (shared by define-fun and define-fun-rec).
#[derive(Debug, Clone)]
pub struct FunDef {
    pub name: Spanned<Symbol>,
    pub params: Vec<Spanned<SortedVar>>,
    pub result_sort: Spanned<Sort>,
    pub body: Spanned<Term>,
}

/// Top-level SMT-LIB commands.
#[derive(Debug, Clone)]
pub enum Command {
    SetLogic(Spanned<Symbol>),
    SetInfo(Attribute),
    SetOption(Attribute),
    GetInfo(Spanned<String>),
    GetOption(Spanned<String>),

    DeclareSort(Spanned<Symbol>, u64),
    DefineSort(Spanned<Symbol>, Vec<Spanned<Symbol>>, Spanned<Sort>),

    DeclareFun(Spanned<Symbol>, Vec<Spanned<Sort>>, Spanned<Sort>),
    DeclareConst(Spanned<Symbol>, Spanned<Sort>),
    DefineFun(FunDef),
    DefineFunRec(FunDef),
    DefineFunsRec(Vec<FunDef>),

    DeclareDatatype(Spanned<Symbol>, Spanned<DatatypeDec>),
    DeclareDatatypes(Vec<(Spanned<Symbol>, u64)>, Vec<Spanned<DatatypeDec>>),

    Assert(Spanned<Term>),
    CheckSat,
    CheckSatAssuming(Vec<Spanned<Term>>),

    Push(u64),
    Pop(u64),
    Reset,
    ResetAssertions,

    GetModel,
    GetValue(Vec<Spanned<Term>>),
    GetProof,
    GetUnsatCore,
    GetUnsatAssumptions,
    GetAssertions,
    GetAssignment,

    Echo(String),
    Exit,

    /// Any command we don't specifically handle — stored as raw s-expression.
    Unknown(String, Vec<Spanned<SExpr>>),
}

