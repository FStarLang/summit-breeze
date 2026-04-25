use smtlib_parser::ast::*;
use smtlib_parser::span::Span;
use std::collections::HashMap;

/// A symbol definition in the document.
#[derive(Debug, Clone)]
pub struct SymbolDef {
    pub name: String,
    pub kind: SymbolKind,
    pub def_span: Span,
    /// Span of just the name token.
    pub name_span: Span,
    /// Push/pop stack depth at definition site.
    pub stack_depth: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Constant,
    Sort,
    Datatype,
    Constructor,
    Selector,
    Variable,
}

/// A reference (use) of a symbol.
#[derive(Debug, Clone)]
pub struct SymbolRef {
    pub name: String,
    pub span: Span,
}

/// A push/pop pair for navigation.
#[derive(Debug, Clone)]
pub struct PushPopPair {
    pub push_span: Span,
    pub pop_span: Option<Span>,
    pub depth: u32,
}

/// Per-document symbol index.
#[derive(Debug, Default)]
pub struct SymbolIndex {
    /// All definitions, keyed by name.
    pub definitions: HashMap<String, Vec<SymbolDef>>,
    /// All references/uses.
    pub references: Vec<SymbolRef>,
    /// Push/pop stack for navigation.
    pub push_pop_pairs: Vec<PushPopPair>,
    /// All top-level command spans for folding.
    pub command_spans: Vec<CommandInfo>,
}

/// Info about a top-level command (for outline/folding).
#[derive(Debug, Clone)]
pub struct CommandInfo {
    pub kind: CommandInfoKind,
    pub name: Option<String>,
    pub span: Span,
    pub stack_depth: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandInfoKind {
    SetLogic,
    DeclareSort,
    DefineSort,
    DeclareFun,
    DeclareConst,
    DefineFun,
    DefineFunRec,
    DeclareDatatype,
    DeclareDatatypes,
    Assert,
    CheckSat,
    Push,
    Pop,
    Other,
}

/// Build a symbol index from a parsed script.
pub fn build_index(script: &Script) -> SymbolIndex {
    let mut index = SymbolIndex::default();
    let mut stack_depth: u32 = 0;
    let mut push_stack: Vec<(Span, u32)> = Vec::new();

    for cmd in &script.commands {
        let span = cmd.span;

        match &cmd.node {
            Command::SetLogic(sym) => {
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::SetLogic,
                    name: Some(sym.node.name.clone()),
                    span,
                    stack_depth,
                });
            }

            Command::DeclareSort(name, _arity) => {
                add_def(
                    &mut index,
                    &name.node,
                    name.span,
                    span,
                    SymbolKind::Sort,
                    stack_depth,
                );
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::DeclareSort,
                    name: Some(name.node.name.clone()),
                    span,
                    stack_depth,
                });
            }

            Command::DefineSort(name, _params, _sort) => {
                add_def(
                    &mut index,
                    &name.node,
                    name.span,
                    span,
                    SymbolKind::Sort,
                    stack_depth,
                );
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::DefineSort,
                    name: Some(name.node.name.clone()),
                    span,
                    stack_depth,
                });
            }

            Command::DeclareFun(name, _params, _result) => {
                add_def(
                    &mut index,
                    &name.node,
                    name.span,
                    span,
                    SymbolKind::Function,
                    stack_depth,
                );
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::DeclareFun,
                    name: Some(name.node.name.clone()),
                    span,
                    stack_depth,
                });
            }

            Command::DeclareConst(name, _sort) => {
                add_def(
                    &mut index,
                    &name.node,
                    name.span,
                    span,
                    SymbolKind::Constant,
                    stack_depth,
                );
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::DeclareConst,
                    name: Some(name.node.name.clone()),
                    span,
                    stack_depth,
                });
            }

            Command::DefineFun(def) | Command::DefineFunRec(def) => {
                add_def(
                    &mut index,
                    &def.name.node,
                    def.name.span,
                    span,
                    SymbolKind::Function,
                    stack_depth,
                );
                // Index parameters as local variables
                for param in &def.params {
                    add_def(
                        &mut index,
                        &param.node.name.node,
                        param.node.name.span,
                        param.span,
                        SymbolKind::Variable,
                        stack_depth,
                    );
                }
                // Collect references in the body
                collect_term_refs(&mut index, &def.body);

                let kind = if matches!(&cmd.node, Command::DefineFunRec(_)) {
                    CommandInfoKind::DefineFunRec
                } else {
                    CommandInfoKind::DefineFun
                };
                index.command_spans.push(CommandInfo {
                    kind,
                    name: Some(def.name.node.name.clone()),
                    span,
                    stack_depth,
                });
            }

            Command::DefineFunsRec(defs) => {
                for def in defs {
                    add_def(
                        &mut index,
                        &def.name.node,
                        def.name.span,
                        span,
                        SymbolKind::Function,
                        stack_depth,
                    );
                    for param in &def.params {
                        add_def(
                            &mut index,
                            &param.node.name.node,
                            param.node.name.span,
                            param.span,
                            SymbolKind::Variable,
                            stack_depth,
                        );
                    }
                    collect_term_refs(&mut index, &def.body);
                }
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::DefineFun,
                    name: defs.first().map(|d| d.name.node.name.clone()),
                    span,
                    stack_depth,
                });
            }

            Command::DeclareDatatype(name, dec) => {
                add_def(
                    &mut index,
                    &name.node,
                    name.span,
                    span,
                    SymbolKind::Datatype,
                    stack_depth,
                );
                index_datatype_dec(&mut index, dec, stack_depth);
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::DeclareDatatype,
                    name: Some(name.node.name.clone()),
                    span,
                    stack_depth,
                });
            }

            Command::DeclareDatatypes(sort_decs, decs) => {
                for (name, _arity) in sort_decs {
                    add_def(
                        &mut index,
                        &name.node,
                        name.span,
                        span,
                        SymbolKind::Datatype,
                        stack_depth,
                    );
                }
                for dec in decs {
                    index_datatype_dec(&mut index, dec, stack_depth);
                }
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::DeclareDatatypes,
                    name: sort_decs.first().map(|(n, _)| n.node.name.clone()),
                    span,
                    stack_depth,
                });
            }

            Command::Assert(term) => {
                collect_term_refs(&mut index, term);
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::Assert,
                    name: None,
                    span,
                    stack_depth,
                });
            }

            Command::CheckSat | Command::CheckSatAssuming(_) => {
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::CheckSat,
                    name: None,
                    span,
                    stack_depth,
                });
            }

            Command::Push(n) => {
                for _ in 0..*n {
                    push_stack.push((span, stack_depth));
                    stack_depth += 1;
                }
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::Push,
                    name: None,
                    span,
                    stack_depth: stack_depth - 1,
                });
            }

            Command::Pop(n) => {
                for _ in 0..*n {
                    if let Some((push_span, depth)) = push_stack.pop() {
                        index.push_pop_pairs.push(PushPopPair {
                            push_span,
                            pop_span: Some(span),
                            depth,
                        });
                        stack_depth = stack_depth.saturating_sub(1);
                    }
                }
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::Pop,
                    name: None,
                    span,
                    stack_depth,
                });
            }

            Command::Unknown(_name, args) => {
                // Collect symbol references from s-expressions in unknown commands
                for sexpr in args {
                    collect_sexpr_refs(&mut index, sexpr);
                }
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::Other,
                    name: None,
                    span,
                    stack_depth,
                });
            }

            _ => {
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::Other,
                    name: None,
                    span,
                    stack_depth,
                });
            }
        }
    }

    // Any unmatched pushes
    for (push_span, depth) in push_stack {
        index.push_pop_pairs.push(PushPopPair {
            push_span,
            pop_span: None,
            depth,
        });
    }

    index
}

fn add_def(
    index: &mut SymbolIndex,
    sym: &Symbol,
    name_span: Span,
    def_span: Span,
    kind: SymbolKind,
    stack_depth: u32,
) {
    let def = SymbolDef {
        name: sym.name.clone(),
        kind,
        def_span,
        name_span,
        stack_depth,
    };
    index
        .definitions
        .entry(sym.name.clone())
        .or_default()
        .push(def);
}

fn index_datatype_dec(index: &mut SymbolIndex, dec: &Spanned<DatatypeDec>, stack_depth: u32) {
    for ctor in &dec.node.constructors {
        add_def(
            index,
            &ctor.node.name.node,
            ctor.node.name.span,
            ctor.span,
            SymbolKind::Constructor,
            stack_depth,
        );
        for sel in &ctor.node.selectors {
            add_def(
                index,
                &sel.name.node,
                sel.name.span,
                sel.name.span,
                SymbolKind::Selector,
                stack_depth,
            );
        }
    }
}

/// Collect symbol references within a term.
fn collect_term_refs(index: &mut SymbolIndex, term: &Spanned<Term>) {
    match &term.node {
        Term::QualifiedIdentifier(qi) => {
            collect_qi_ref(index, qi);
        }
        Term::Application(func, args) => {
            collect_qi_ref(index, &func.node);
            for arg in args {
                collect_term_refs(index, arg);
            }
        }
        Term::Let(bindings, body) => {
            for binding in bindings {
                collect_term_refs(index, &binding.node.value);
                index.references.push(SymbolRef {
                    name: binding.node.name.node.name.clone(),
                    span: binding.node.name.span,
                });
            }
            collect_term_refs(index, body);
        }
        Term::Forall(vars, body) | Term::Exists(vars, body) | Term::Lambda(vars, body) => {
            for var in vars {
                index.references.push(SymbolRef {
                    name: var.node.name.node.name.clone(),
                    span: var.node.name.span,
                });
            }
            collect_term_refs(index, body);
        }
        Term::Match(scrutinee, cases) => {
            collect_term_refs(index, scrutinee);
            for case in cases {
                collect_term_refs(index, &case.node.body);
            }
        }
        Term::Annotated(inner, _attrs) => {
            collect_term_refs(index, inner);
        }
        Term::Constant(_) => {}
    }
}

fn collect_qi_ref(index: &mut SymbolIndex, qi: &QualifiedIdentifier) {
    match qi {
        QualifiedIdentifier::Simple(ident) => {
            if let Identifier::Simple(sym) = &ident.node {
                index.references.push(SymbolRef {
                    name: sym.name.clone(),
                    span: ident.span,
                });
            }
        }
        QualifiedIdentifier::As(ident, _sort) => {
            if let Identifier::Simple(sym) = &ident.node {
                index.references.push(SymbolRef {
                    name: sym.name.clone(),
                    span: ident.span,
                });
            }
        }
    }
}

/// Collect symbol references from s-expressions (for Unknown commands).
fn collect_sexpr_refs(index: &mut SymbolIndex, sexpr: &Spanned<SExpr>) {
    match &sexpr.node {
        SExpr::Symbol(sym) => {
            index.references.push(SymbolRef {
                name: sym.name.clone(),
                span: sexpr.span,
            });
        }
        SExpr::List(items) => {
            for item in items {
                collect_sexpr_refs(index, item);
            }
        }
        SExpr::Constant(_) | SExpr::Keyword(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smtlib_parser::parse;

    #[test]
    fn test_index_declarations() {
        let result = parse(
            "(declare-fun f (Int) Bool)\n(declare-const x Int)\n(declare-sort MySort 0)",
        );
        let index = build_index(&result.script);
        assert!(index.definitions.contains_key("f"));
        assert!(index.definitions.contains_key("x"));
        assert!(index.definitions.contains_key("MySort"));
        assert_eq!(index.definitions["f"][0].kind, SymbolKind::Function);
        assert_eq!(index.definitions["x"][0].kind, SymbolKind::Constant);
        assert_eq!(index.definitions["MySort"][0].kind, SymbolKind::Sort);
    }

    #[test]
    fn test_index_push_pop() {
        let result = parse("(push 1)(assert true)(pop 1)");
        let index = build_index(&result.script);
        assert_eq!(index.push_pop_pairs.len(), 1);
        assert!(index.push_pop_pairs[0].pop_span.is_some());
    }

    #[test]
    fn test_index_references() {
        let result = parse("(declare-fun f () Int)\n(assert (= f 0))");
        let index = build_index(&result.script);
        let f_refs: Vec<_> = index.references.iter().filter(|r| r.name == "f").collect();
        assert!(!f_refs.is_empty());
    }

    #[test]
    fn test_index_define_fun_params() {
        let result = parse("(define-fun add ((x Int) (y Int)) Int (+ x y))");
        let index = build_index(&result.script);
        assert!(index.definitions.contains_key("add"));
        assert!(index.definitions.contains_key("x"));
        assert!(index.definitions.contains_key("y"));
        assert_eq!(index.definitions["x"][0].kind, SymbolKind::Variable);
    }

    #[test]
    fn test_index_datatypes() {
        let src = "(declare-datatypes ((Color 0)) (((Red) (Green) (Blue))))";
        let result = parse(src);
        let index = build_index(&result.script);
        assert!(index.definitions.contains_key("Color"));
        assert!(index.definitions.contains_key("Red"));
        assert!(index.definitions.contains_key("Green"));
        assert!(index.definitions.contains_key("Blue"));
        assert_eq!(index.definitions["Color"][0].kind, SymbolKind::Datatype);
        assert_eq!(index.definitions["Red"][0].kind, SymbolKind::Constructor);
    }

    #[test]
    fn test_assert_not_references() {
        let src = "(declare-fun foo (Int) Int)\n(assert-not (= (foo 10) 20))";
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let index = build_index(&result.script);
        assert!(index.definitions.contains_key("foo"));
        let foo_refs: Vec<_> = index.references.iter().filter(|r| r.name == "foo").collect();
        assert!(!foo_refs.is_empty(), "foo should have references inside assert-not");
    }
}
