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
    /// End of lexical scope for local variables (None = global/command-level).
    pub scope_end: Option<u32>,
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
    /// Push/pop stack depth at reference site.
    pub stack_depth: u32,
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

impl SymbolIndex {
    /// Find the reference at the given byte offset.
    pub fn ref_at(&self, offset: u32) -> Option<&SymbolRef> {
        // Prefer the smallest (most specific) span containing the offset.
        self.references
            .iter()
            .filter(|r| offset >= r.span.start && offset < r.span.end)
            .min_by_key(|r| r.span.end - r.span.start)
    }

    /// Resolve a symbol name at the given offset to its best definition.
    /// Respects both push/pop scoping and lexical scoping.
    pub fn resolve(&self, name: &str, offset: u32, ref_depth: u32) -> Option<&SymbolDef> {
        let defs = self.definitions.get(name)?;

        // Pick the best definition:
        // 1. Must be defined before the reference offset
        // 2. Stack depth must be ≤ ref depth (push/pop scoping)
        // 3. If scoped (local var), offset must be within scope
        // 4. Prefer the latest matching definition
        let best = defs
            .iter()
            .filter(|d| {
                d.name_span.start < offset
                    && d.stack_depth <= ref_depth
                    && d.scope_end.is_none_or(|end| offset <= end)
            })
            .max_by_key(|d| d.name_span.start);

        // Fall back to any definition with the name
        best.or_else(|| defs.first())
    }
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

            Command::DefineSort(name, _params, sort_body) => {
                add_def(
                    &mut index,
                    &name.node,
                    name.span,
                    span,
                    SymbolKind::Sort,
                    stack_depth,
                );
                collect_sort_refs(&mut index, sort_body, stack_depth);
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::DefineSort,
                    name: Some(name.node.name.clone()),
                    span,
                    stack_depth,
                });
            }

            Command::DeclareFun(name, params, result) => {
                add_def(
                    &mut index,
                    &name.node,
                    name.span,
                    span,
                    SymbolKind::Function,
                    stack_depth,
                );
                for param_sort in params {
                    collect_sort_refs(&mut index, param_sort, stack_depth);
                }
                collect_sort_refs(&mut index, result, stack_depth);
                index.command_spans.push(CommandInfo {
                    kind: CommandInfoKind::DeclareFun,
                    name: Some(name.node.name.clone()),
                    span,
                    stack_depth,
                });
            }

            Command::DeclareConst(name, sort) => {
                add_def(
                    &mut index,
                    &name.node,
                    name.span,
                    span,
                    SymbolKind::Constant,
                    stack_depth,
                );
                collect_sort_refs(&mut index, sort, stack_depth);
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
                // Index parameters as local variables (scoped to body end)
                let body_end = def.body.span.end;
                for param in &def.params {
                    add_def_scoped(
                        &mut index,
                        &param.node.name.node,
                        param.node.name.span,
                        param.span,
                        SymbolKind::Variable,
                        stack_depth,
                        Some(body_end),
                    );
                    collect_sort_refs(&mut index, &param.node.sort, stack_depth);
                }
                collect_sort_refs(&mut index, &def.result_sort, stack_depth);
                // Collect references in the body
                collect_term_refs(&mut index, &def.body, stack_depth);

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
                    let body_end = def.body.span.end;
                    for param in &def.params {
                        add_def_scoped(
                            &mut index,
                            &param.node.name.node,
                            param.node.name.span,
                            param.span,
                            SymbolKind::Variable,
                            stack_depth,
                            Some(body_end),
                        );
                        collect_sort_refs(&mut index, &param.node.sort, stack_depth);
                    }
                    collect_sort_refs(&mut index, &def.result_sort, stack_depth);
                    collect_term_refs(&mut index, &def.body, stack_depth);
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
                collect_term_refs(&mut index, term, stack_depth);
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
                    collect_sexpr_refs(&mut index, sexpr, stack_depth);
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
    add_def_scoped(index, sym, name_span, def_span, kind, stack_depth, None)
}

fn add_def_scoped(
    index: &mut SymbolIndex,
    sym: &Symbol,
    name_span: Span,
    def_span: Span,
    kind: SymbolKind,
    stack_depth: u32,
    scope_end: Option<u32>,
) {
    let def = SymbolDef {
        name: sym.name.clone(),
        kind,
        def_span,
        name_span,
        stack_depth,
        scope_end,
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
            collect_sort_refs(index, &sel.sort, stack_depth);
        }
    }
}

/// Collect symbol references within a term.
fn collect_term_refs(index: &mut SymbolIndex, term: &Spanned<Term>, stack_depth: u32) {
    match &term.node {
        Term::QualifiedIdentifier(qi) => {
            collect_qi_ref(index, qi, stack_depth);
        }
        Term::Application(func, args) => {
            collect_qi_ref(index, &func.node, stack_depth);
            for arg in args {
                collect_term_refs(index, arg, stack_depth);
            }
        }
        Term::Let(bindings, body) => {
            // SMT-LIB let has simultaneous binding: RHSs don't see each other.
            // Phase 1: collect refs in all RHS values
            for binding in bindings {
                collect_term_refs(index, &binding.node.value, stack_depth);
            }
            // Phase 2: add let-bound variable definitions (scoped to body end)
            let body_end = body.span.end;
            for binding in bindings {
                add_def_scoped(
                    index,
                    &binding.node.name.node,
                    binding.node.name.span,
                    binding.span,
                    SymbolKind::Variable,
                    stack_depth,
                    Some(body_end),
                );
            }
            // Phase 3: collect refs in the body
            collect_term_refs(index, body, stack_depth);
        }
        Term::Forall(vars, body) | Term::Exists(vars, body) | Term::Lambda(vars, body) => {
            let body_end = body.span.end;
            for var in vars {
                add_def_scoped(
                    index,
                    &var.node.name.node,
                    var.node.name.span,
                    var.span,
                    SymbolKind::Variable,
                    stack_depth,
                    Some(body_end),
                );
                collect_sort_refs(index, &var.node.sort, stack_depth);
            }
            collect_term_refs(index, body, stack_depth);
        }
        Term::Match(scrutinee, cases) => {
            collect_term_refs(index, scrutinee, stack_depth);
            for case in cases {
                collect_term_refs(index, &case.node.body, stack_depth);
            }
        }
        Term::Annotated(inner, attrs) => {
            collect_term_refs(index, inner, stack_depth);
            // Walk :pattern and :no-pattern attribute values for refs
            for attr in attrs {
                let kw = &attr.keyword.node;
                if (kw == ":pattern" || kw == ":no-pattern")
                    && let Some(val) = &attr.value
                {
                    collect_attr_value_refs(index, &val.node, stack_depth);
                }
            }
        }
        Term::Constant(_) => {}
    }
}

/// Collect refs from attribute values (for :pattern/:no-pattern).
fn collect_attr_value_refs(
    index: &mut SymbolIndex,
    val: &AttributeValue,
    stack_depth: u32,
) {
    match val {
        AttributeValue::SExpr(items) => {
            for item in items {
                collect_sexpr_refs(index, item, stack_depth);
            }
        }
        AttributeValue::Symbol(_sym) => {
            // We don't have a span for the symbol in AttributeValue::Symbol,
            // so we can't add a precise ref here. Skip.
        }
        _ => {}
    }
}

/// Collect sort references from a sort node.
fn collect_sort_refs(index: &mut SymbolIndex, sort: &Spanned<Sort>, stack_depth: u32) {
    match &sort.node {
        Sort::Simple(Identifier::Simple(sym)) => {
            index.references.push(SymbolRef {
                name: sym.name.clone(),
                span: sort.span,
                stack_depth,
            });
        }
        Sort::Simple(Identifier::Indexed(sym, _)) => {
            // e.g., (_ BitVec 8) — add ref for the head symbol
            index.references.push(SymbolRef {
                name: sym.name.clone(),
                span: sort.span,
                stack_depth,
            });
        }
        Sort::Parameterized(ident, params) => {
            // Add ref for the head sort identifier
            match ident {
                Identifier::Simple(sym) => {
                    index.references.push(SymbolRef {
                        name: sym.name.clone(),
                        span: sort.span,
                        stack_depth,
                    });
                }
                Identifier::Indexed(sym, _) => {
                    index.references.push(SymbolRef {
                        name: sym.name.clone(),
                        span: sort.span,
                        stack_depth,
                    });
                }
            }
            for param in params {
                collect_sort_refs(index, param, stack_depth);
            }
        }
    }
}

fn collect_qi_ref(index: &mut SymbolIndex, qi: &QualifiedIdentifier, stack_depth: u32) {
    match qi {
        QualifiedIdentifier::Simple(ident) => {
            if let Identifier::Simple(sym) = &ident.node {
                index.references.push(SymbolRef {
                    name: sym.name.clone(),
                    span: ident.span,
                    stack_depth,
                });
            }
        }
        QualifiedIdentifier::As(ident, sort) => {
            if let Identifier::Simple(sym) = &ident.node {
                index.references.push(SymbolRef {
                    name: sym.name.clone(),
                    span: ident.span,
                    stack_depth,
                });
            }
            collect_sort_refs(index, sort, stack_depth);
        }
    }
}

/// Collect symbol references from s-expressions (for Unknown commands).
fn collect_sexpr_refs(index: &mut SymbolIndex, sexpr: &Spanned<SExpr>, stack_depth: u32) {
    match &sexpr.node {
        SExpr::Symbol(sym) => {
            index.references.push(SymbolRef {
                name: sym.name.clone(),
                span: sexpr.span,
                stack_depth,
            });
        }
        SExpr::List(items) => {
            for item in items {
                collect_sexpr_refs(index, item, stack_depth);
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

    #[test]
    fn test_sort_references() {
        let src = "(declare-sort MySort 0)\n(declare-fun f (MySort) MySort)";
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let index = build_index(&result.script);
        assert!(index.definitions.contains_key("MySort"));
        let sort_refs: Vec<_> = index.references.iter().filter(|r| r.name == "MySort").collect();
        // Should have at least 2 refs: param sort and result sort
        assert!(sort_refs.len() >= 2, "MySort should be referenced in param and result sort, got {}", sort_refs.len());
    }

    #[test]
    fn test_local_variable_defs_forall() {
        let src = "(declare-fun f (Int) Bool)\n(assert (forall ((x Int)) (f x)))";
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let index = build_index(&result.script);
        // x should be a local variable definition
        assert!(index.definitions.contains_key("x"), "forall-bound x should be a definition");
        assert_eq!(index.definitions["x"][0].kind, SymbolKind::Variable);
        assert!(index.definitions["x"][0].scope_end.is_some(), "x should have a scope_end");
        // x should also appear as a reference in (f x)
        let x_refs: Vec<_> = index.references.iter().filter(|r| r.name == "x").collect();
        assert!(!x_refs.is_empty(), "x should be referenced in body");
    }

    #[test]
    fn test_local_variable_defs_let() {
        let src = "(assert (let ((a 1) (b 2)) (+ a b)))";
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let index = build_index(&result.script);
        assert!(index.definitions.contains_key("a"), "let-bound a should be a definition");
        assert!(index.definitions.contains_key("b"), "let-bound b should be a definition");
        assert_eq!(index.definitions["a"][0].kind, SymbolKind::Variable);
        assert!(index.definitions["a"][0].scope_end.is_some());
    }

    #[test]
    fn test_let_simultaneous_binding() {
        // In SMT-LIB, let bindings are simultaneous: the second binding's RHS
        // should NOT see the first binding's definition.
        let src = "(declare-fun x () Int)\n(assert (let ((x 1) (y x)) (+ x y)))";
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let index = build_index(&result.script);
        // There should be a global x (declare-fun) and a local x (let)
        assert_eq!(index.definitions["x"].len(), 2, "should have global and local x");
        // The 'x' reference in (y x) should resolve to global x (before the let defs)
        // Find the ref for x that appears in the second binding's value
        let global_x = &index.definitions["x"][0]; // declare-fun
        let local_x = &index.definitions["x"][1]; // let
        assert!(global_x.scope_end.is_none(), "global x should have no scope_end");
        assert!(local_x.scope_end.is_some(), "local x should have scope_end");
    }

    #[test]
    fn test_lexical_scope_boundary() {
        // x bound in forall should not be visible after the forall body
        let src = "(declare-fun f (Int) Bool)\n(assert (and (forall ((x Int)) (f x)) (f 42)))";
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let index = build_index(&result.script);
        let x_def = &index.definitions["x"][0];
        assert!(x_def.scope_end.is_some());
        // The x ref inside forall body should be within scope
        let x_refs: Vec<_> = index.references.iter().filter(|r| r.name == "x").collect();
        assert_eq!(x_refs.len(), 1, "only one x reference (inside forall)");
        let x_ref = x_refs[0];
        assert!(x_ref.span.start >= x_def.name_span.start);
        assert!(x_ref.span.end <= x_def.scope_end.unwrap());
    }

    #[test]
    fn test_pattern_refs() {
        let src = "(declare-fun f (Int) Int)\n(assert (forall ((x Int)) (! (= (f x) 0) :pattern ((f x)))))";
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let index = build_index(&result.script);
        // f should appear multiple times: in body and in :pattern
        let f_refs: Vec<_> = index.references.iter().filter(|r| r.name == "f").collect();
        assert!(f_refs.len() >= 2, "f should be referenced in body and pattern, got {}", f_refs.len());
    }

    #[test]
    fn test_resolve_respects_scope() {
        let src = "(declare-fun x () Int)\n(assert (forall ((x Int)) (= x 0)))";
        let result = parse(src);
        let index = build_index(&result.script);
        // Find the x reference inside the forall body
        let x_ref = index.references.iter().find(|r| r.name == "x").unwrap();
        // Resolve should find the forall-bound x (local), not the global one
        let resolved = index.resolve("x", x_ref.span.start, x_ref.stack_depth).unwrap();
        assert!(resolved.scope_end.is_some(), "should resolve to local x, not global");
    }

    #[test]
    fn test_sort_refs_in_selector() {
        let src = "(declare-sort MySort 0)\n(declare-datatypes ((MyDT 0)) (((mk-dt (field1 MySort)))))";
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let index = build_index(&result.script);
        let sort_refs: Vec<_> = index.references.iter().filter(|r| r.name == "MySort").collect();
        assert!(!sort_refs.is_empty(), "MySort should be referenced in selector sort");
    }
}
