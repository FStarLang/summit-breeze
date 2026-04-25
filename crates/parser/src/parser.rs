use crate::ast::*;
use crate::lexer::{Lexer, Token, TokenKind};
use crate::span::Span;

/// A parse diagnostic.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub span: Span,
    pub message: String,
}

/// Parse result: AST + diagnostics.
#[derive(Debug)]
pub struct ParseResult {
    pub script: Script,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse an SMT-LIB source string into a script.
pub fn parse(src: &str) -> ParseResult {
    let mut parser = Parser::new(src);
    let script = parser.parse_script();
    ParseResult {
        script,
        diagnostics: parser.diagnostics,
    }
}

struct Parser<'a> {
    src: &'a str,
    lexer: Lexer<'a>,
    /// Tokens buffered for lookahead (comments stripped).
    peeked: Option<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            lexer: Lexer::new(src),
            peeked: None,
            diagnostics: Vec::new(),
        }
    }

    fn error(&mut self, span: Span, msg: impl Into<String>) {
        self.diagnostics.push(Diagnostic {
            span,
            message: msg.into(),
        });
    }

    fn text(&self, token: &Token) -> &'a str {
        &self.src[token.span.start as usize..token.span.end as usize]
    }

    /// Peek at the next non-comment token (returns a copy).
    fn peek(&mut self) -> Option<Token> {
        if self.peeked.is_none() {
            self.peeked = self.next_meaningful_token();
        }
        self.peeked
    }

    /// Consume and return the next non-comment token.
    fn next(&mut self) -> Option<Token> {
        if let Some(tok) = self.peeked.take() {
            Some(tok)
        } else {
            self.next_meaningful_token()
        }
    }

    fn next_meaningful_token(&mut self) -> Option<Token> {
        loop {
            let tok = self.lexer.next_token()?;
            if tok.kind != TokenKind::Comment {
                return Some(tok);
            }
        }
    }

    /// Expect a specific token kind; return it or emit an error.
    fn expect(&mut self, kind: TokenKind) -> Option<Token> {
        match self.next() {
            Some(tok) if tok.kind == kind => Some(tok),
            Some(tok) => {
                self.error(tok.span, format!("expected {:?}, found {:?}", kind, tok.kind));
                None
            }
            None => {
                let end = self.src.len() as u32;
                self.error(Span::new(end, end), format!("expected {:?}, found EOF", kind));
                None
            }
        }
    }

    /// Expect an LParen.
    fn expect_lparen(&mut self) -> Option<Token> {
        self.expect(TokenKind::LParen)
    }

    /// Expect an RParen.
    fn expect_rparen(&mut self) -> Option<Token> {
        self.expect(TokenKind::RParen)
    }

    /// Skip tokens until we find a balanced RParen at the current depth (depth starts at 1).
    fn skip_to_balanced_rparen(&mut self) {
        let mut depth = 1u32;
        while let Some(tok) = self.next() {
            match tok.kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        return;
                    }
                }
                _ => {}
            }
        }
    }

    // -----------------------------------------------------------------------
    // Script
    // -----------------------------------------------------------------------

    fn parse_script(&mut self) -> Script {
        let mut commands = Vec::new();
        while self.peek().is_some() {
            if let Some(cmd) = self.parse_command() {
                commands.push(cmd);
            }
        }
        Script { commands }
    }

    // -----------------------------------------------------------------------
    // Commands
    // -----------------------------------------------------------------------

    fn parse_command(&mut self) -> Option<Spanned<Command>> {
        let lparen = self.expect_lparen()?;
        let start = lparen.span.start;

        let name_tok = match self.next() {
            Some(tok) if tok.kind == TokenKind::Symbol => tok,
            Some(tok) => {
                self.error(tok.span, "expected command name");
                self.skip_to_balanced_rparen();
                return None;
            }
            None => return None,
        };

        let name = self.text(&name_tok);
        let cmd = match name {
            "set-logic" => self.parse_set_logic(),
            "set-info" => self.parse_set_info(),
            "set-option" => self.parse_set_option(),
            "get-info" => self.parse_get_info(),
            "get-option" => self.parse_get_option(),
            "declare-sort" => self.parse_declare_sort(),
            "define-sort" => self.parse_define_sort(),
            "declare-fun" => self.parse_declare_fun(),
            "declare-const" => self.parse_declare_const(),
            "define-fun" => self.parse_define_fun(),
            "define-fun-rec" => self.parse_define_fun_rec(),
            "define-funs-rec" => self.parse_define_funs_rec(),
            "declare-datatype" => self.parse_declare_datatype(),
            "declare-datatypes" => self.parse_declare_datatypes(),
            "assert" | "assert-not" => self.parse_assert(),
            "check-sat" => Some(Command::CheckSat),
            "check-sat-assuming" => self.parse_check_sat_assuming(),
            "push" => self.parse_push(),
            "pop" => self.parse_pop(),
            "reset" => Some(Command::Reset),
            "reset-assertions" => Some(Command::ResetAssertions),
            "get-model" => Some(Command::GetModel),
            "get-value" => self.parse_get_value(),
            "get-proof" => Some(Command::GetProof),
            "get-unsat-core" => Some(Command::GetUnsatCore),
            "get-unsat-assumptions" => Some(Command::GetUnsatAssumptions),
            "get-assertions" => Some(Command::GetAssertions),
            "get-assignment" => Some(Command::GetAssignment),
            "echo" => self.parse_echo(),
            "exit" => Some(Command::Exit),
            _ => self.parse_unknown_command(name),
        };

        let cmd = match cmd {
            Some(c) => c,
            None => {
                self.skip_to_balanced_rparen();
                return None;
            }
        };

        let rparen = self.expect_rparen();
        let end = rparen.map(|t| t.span.end).unwrap_or(self.src.len() as u32);

        Some(Spanned::new(cmd, Span::new(start, end)))
    }

    // --- Individual command parsers ---

    fn parse_set_logic(&mut self) -> Option<Command> {
        let sym = self.parse_spanned_symbol()?;
        Some(Command::SetLogic(sym))
    }

    fn parse_set_info(&mut self) -> Option<Command> {
        let attr = self.parse_attribute()?;
        Some(Command::SetInfo(attr))
    }

    fn parse_set_option(&mut self) -> Option<Command> {
        let attr = self.parse_attribute()?;
        Some(Command::SetOption(attr))
    }

    fn parse_get_info(&mut self) -> Option<Command> {
        let kw = self.parse_keyword()?;
        Some(Command::GetInfo(kw))
    }

    fn parse_get_option(&mut self) -> Option<Command> {
        let kw = self.parse_keyword()?;
        Some(Command::GetOption(kw))
    }

    fn parse_declare_sort(&mut self) -> Option<Command> {
        let name = self.parse_spanned_symbol()?;
        // Arity is optional — defaults to 0 (Z3 extension)
        let arity = if self.peek_is(TokenKind::Numeral) {
            self.parse_numeral_value()?
        } else {
            0
        };
        Some(Command::DeclareSort(name, arity))
    }

    fn parse_define_sort(&mut self) -> Option<Command> {
        let name = self.parse_spanned_symbol()?;
        self.expect_lparen()?;
        let mut params = Vec::new();
        while !self.peek_is(TokenKind::RParen) {
            params.push(self.parse_spanned_symbol()?);
        }
        self.expect_rparen()?;
        let sort = self.parse_sort()?;
        Some(Command::DefineSort(name, params, sort))
    }

    fn parse_declare_fun(&mut self) -> Option<Command> {
        let name = self.parse_spanned_symbol()?;
        self.expect_lparen()?;
        let mut param_sorts = Vec::new();
        while !self.peek_is(TokenKind::RParen) {
            param_sorts.push(self.parse_sort()?);
        }
        self.expect_rparen()?;
        let result_sort = self.parse_sort()?;
        Some(Command::DeclareFun(name, param_sorts, result_sort))
    }

    fn parse_declare_const(&mut self) -> Option<Command> {
        let name = self.parse_spanned_symbol()?;
        let sort = self.parse_sort()?;
        Some(Command::DeclareConst(name, sort))
    }

    fn parse_define_fun(&mut self) -> Option<Command> {
        let def = self.parse_fun_def()?;
        Some(Command::DefineFun(def))
    }

    fn parse_define_fun_rec(&mut self) -> Option<Command> {
        let def = self.parse_fun_def()?;
        Some(Command::DefineFunRec(def))
    }

    fn parse_define_funs_rec(&mut self) -> Option<Command> {
        // ( define-funs-rec ( (sig)+ ) ( term+ ) )
        self.expect_lparen()?;
        let mut sigs = Vec::new();
        while !self.peek_is(TokenKind::RParen) {
            self.expect_lparen()?;
            let name = self.parse_spanned_symbol()?;
            self.expect_lparen()?;
            let mut params = Vec::new();
            while !self.peek_is(TokenKind::RParen) {
                params.push(self.parse_sorted_var()?);
            }
            self.expect_rparen()?;
            let result_sort = self.parse_sort()?;
            self.expect_rparen()?;
            sigs.push((name, params, result_sort));
        }
        self.expect_rparen()?;

        self.expect_lparen()?;
        let mut defs = Vec::new();
        for (name, params, result_sort) in sigs {
            let body = self.parse_term()?;
            defs.push(FunDef {
                name,
                params,
                result_sort,
                body,
            });
        }
        self.expect_rparen()?;
        Some(Command::DefineFunsRec(defs))
    }

    fn parse_declare_datatype(&mut self) -> Option<Command> {
        let name = self.parse_spanned_symbol()?;
        let dec = self.parse_datatype_dec()?;
        Some(Command::DeclareDatatype(name, dec))
    }

    fn parse_declare_datatypes(&mut self) -> Option<Command> {
        // Supports both syntaxes:
        // SMT-LIB 2.6: (declare-datatypes ((name arity)+) (dec+))
        // Z3 legacy:   (declare-datatypes () ((TypeName (Ctor ...) ...)+))
        self.expect_lparen()?;

        if self.peek_is(TokenKind::RParen) {
            // Z3 legacy syntax: (declare-datatypes () ((TypeName ctors...) ...))
            self.expect_rparen()?;
            self.expect_lparen()?;
            let mut sort_decs = Vec::new();
            let mut decs = Vec::new();
            while !self.peek_is(TokenKind::RParen) {
                // Each entry: (TypeName (Ctor (sel Sort)?)+ )
                let lp = self.expect_lparen()?;
                let name = self.parse_spanned_symbol()?;
                let mut ctors = Vec::new();
                while !self.peek_is(TokenKind::RParen) {
                    ctors.push(self.parse_constructor_dec()?);
                }
                let rp = self.expect_rparen()?;
                sort_decs.push((name.clone(), 0u64));
                decs.push(Spanned::new(
                    DatatypeDec {
                        params: Vec::new(),
                        constructors: ctors,
                    },
                    Span::new(lp.span.start, rp.span.end),
                ));
            }
            self.expect_rparen()?;
            Some(Command::DeclareDatatypes(sort_decs, decs))
        } else {
            // SMT-LIB 2.6 syntax: (declare-datatypes ((name arity)+) (dec+))
            let mut sort_decs = Vec::new();
            while !self.peek_is(TokenKind::RParen) {
                self.expect_lparen()?;
                let name = self.parse_spanned_symbol()?;
                let arity = self.parse_numeral_value()?;
                self.expect_rparen()?;
                sort_decs.push((name, arity));
            }
            self.expect_rparen()?;

            self.expect_lparen()?;
            let mut decs = Vec::new();
            while !self.peek_is(TokenKind::RParen) {
                decs.push(self.parse_datatype_dec()?);
            }
            self.expect_rparen()?;

            Some(Command::DeclareDatatypes(sort_decs, decs))
        }
    }

    fn parse_assert(&mut self) -> Option<Command> {
        let term = self.parse_term()?;
        Some(Command::Assert(term))
    }

    fn parse_check_sat_assuming(&mut self) -> Option<Command> {
        self.expect_lparen()?;
        let mut terms = Vec::new();
        while !self.peek_is(TokenKind::RParen) {
            terms.push(self.parse_term()?);
        }
        self.expect_rparen()?;
        Some(Command::CheckSatAssuming(terms))
    }

    fn parse_push(&mut self) -> Option<Command> {
        let n = if self.peek_is(TokenKind::Numeral) {
            self.parse_numeral_value()?
        } else {
            1
        };
        Some(Command::Push(n))
    }

    fn parse_pop(&mut self) -> Option<Command> {
        let n = if self.peek_is(TokenKind::Numeral) {
            self.parse_numeral_value()?
        } else {
            1
        };
        Some(Command::Pop(n))
    }

    fn parse_get_value(&mut self) -> Option<Command> {
        self.expect_lparen()?;
        let mut terms = Vec::new();
        while !self.peek_is(TokenKind::RParen) {
            terms.push(self.parse_term()?);
        }
        self.expect_rparen()?;
        Some(Command::GetValue(terms))
    }

    fn parse_echo(&mut self) -> Option<Command> {
        let tok = self.expect(TokenKind::StringLiteral)?;
        let text = self.string_value(&tok);
        Some(Command::Echo(text))
    }

    fn parse_unknown_command(&mut self, name: &str) -> Option<Command> {
        let mut args = Vec::new();
        while !self.peek_is(TokenKind::RParen) {
            if self.peek().is_none() {
                break;
            }
            args.push(self.parse_sexpr()?);
        }
        Some(Command::Unknown(name.to_string(), args))
    }

    // -----------------------------------------------------------------------
    // Fun def helper
    // -----------------------------------------------------------------------

    fn parse_fun_def(&mut self) -> Option<FunDef> {
        let name = self.parse_spanned_symbol()?;
        self.expect_lparen()?;
        let mut params = Vec::new();
        while !self.peek_is(TokenKind::RParen) {
            params.push(self.parse_sorted_var()?);
        }
        self.expect_rparen()?;
        let result_sort = self.parse_sort()?;
        let body = self.parse_term()?;
        Some(FunDef {
            name,
            params,
            result_sort,
            body,
        })
    }

    // -----------------------------------------------------------------------
    // Datatypes
    // -----------------------------------------------------------------------

    fn parse_datatype_dec(&mut self) -> Option<Spanned<DatatypeDec>> {
        let lparen = self.expect_lparen()?;
        let start = lparen.span.start;

        // Check for parametric: (par (params) (constructors))
        let (params, constructors) = if self.peek_is_symbol("par") {
            self.next(); // consume 'par'
            self.expect_lparen()?;
            let mut params = Vec::new();
            while !self.peek_is(TokenKind::RParen) {
                params.push(self.parse_spanned_symbol()?);
            }
            self.expect_rparen()?;
            self.expect_lparen()?;
            let mut ctors = Vec::new();
            while !self.peek_is(TokenKind::RParen) {
                ctors.push(self.parse_constructor_dec()?);
            }
            self.expect_rparen()?;
            (params, ctors)
        } else {
            let mut ctors = Vec::new();
            while !self.peek_is(TokenKind::RParen) {
                ctors.push(self.parse_constructor_dec()?);
            }
            (Vec::new(), ctors)
        };

        let rparen = self.expect_rparen()?;
        let end = rparen.span.end;

        Some(Spanned::new(
            DatatypeDec {
                params,
                constructors,
            },
            Span::new(start, end),
        ))
    }

    fn parse_constructor_dec(&mut self) -> Option<Spanned<ConstructorDec>> {
        let lparen = self.expect_lparen()?;
        let start = lparen.span.start;
        let name = self.parse_spanned_symbol()?;
        let mut selectors = Vec::new();
        while !self.peek_is(TokenKind::RParen) {
            self.expect_lparen()?;
            let sel_name = self.parse_spanned_symbol()?;
            let sel_sort = self.parse_sort()?;
            self.expect_rparen()?;
            selectors.push(SelectorDec {
                name: sel_name,
                sort: sel_sort,
            });
        }
        let rparen = self.expect_rparen()?;
        Some(Spanned::new(
            ConstructorDec { name, selectors },
            Span::new(start, rparen.span.end),
        ))
    }

    // -----------------------------------------------------------------------
    // Sorts
    // -----------------------------------------------------------------------

    fn parse_sort(&mut self) -> Option<Spanned<Sort>> {
        let tok = self.peek()?;
        if tok.kind == TokenKind::LParen {
            let lparen = self.next().unwrap();
            let start = lparen.span.start;

            // Check for indexed sort: (_ id idx+)
            if self.peek_is_symbol("_") {
                self.next(); // consume _
                let sym = self.parse_symbol()?;
                let mut indices = Vec::new();
                while !self.peek_is(TokenKind::RParen) {
                    indices.push(self.parse_index()?);
                }
                let rparen = self.expect_rparen()?;
                let ident = Identifier::Indexed(sym, indices);
                return Some(Spanned::new(
                    Sort::Simple(ident),
                    Span::new(start, rparen.span.end),
                ));
            }

            // Parameterized sort: (id sort+)
            let ident = self.parse_identifier()?;
            let mut params = Vec::new();
            while !self.peek_is(TokenKind::RParen) {
                params.push(self.parse_sort()?);
            }
            let rparen = self.expect_rparen()?;
            Some(Spanned::new(
                Sort::Parameterized(ident, params),
                Span::new(start, rparen.span.end),
            ))
        } else {
            let ident = self.parse_spanned_identifier()?;
            Some(Spanned::new(Sort::Simple(ident.node), ident.span))
        }
    }

    // -----------------------------------------------------------------------
    // Terms
    // -----------------------------------------------------------------------

    fn parse_term(&mut self) -> Option<Spanned<Term>> {
        let tok = self.peek()?;
        let _start = tok.span.start;

        match tok.kind {
            TokenKind::Numeral
            | TokenKind::Decimal
            | TokenKind::Hexadecimal
            | TokenKind::Binary
            | TokenKind::StringLiteral => {
                let tok = self.next().unwrap();
                let c = self.parse_constant(&tok);
                Some(Spanned::new(Term::Constant(c), tok.span))
            }
            TokenKind::Symbol | TokenKind::QuotedSymbol => {
                let ident = self.parse_spanned_qualified_identifier()?;
                Some(Spanned::new(
                    Term::QualifiedIdentifier(ident.node),
                    ident.span,
                ))
            }
            TokenKind::LParen => {
                let lparen = self.next().unwrap();
                self.parse_compound_term(lparen.span.start)
            }
            _ => {
                let tok = self.peek().unwrap();
                self.error(tok.span, "expected term");
                None
            }
        }
    }

    fn parse_compound_term(&mut self, start: u32) -> Option<Spanned<Term>> {
        let tok = self.peek()?;

        // Special forms
        if tok.kind == TokenKind::Symbol || tok.kind == TokenKind::QuotedSymbol {
            let name = &self.src[tok.span.start as usize..tok.span.end as usize];
            match name {
                "let" => return self.parse_let(start),
                "forall" => return self.parse_quantifier(start, true),
                "exists" => return self.parse_quantifier(start, false),
                "lambda" => return self.parse_lambda(start),
                "match" => return self.parse_match(start),
                "!" => return self.parse_annotated(start),
                "_" => {
                    // Indexed identifier used as a term: (_ sym idx+)
                    let ident = self.parse_spanned_qualified_identifier_inner(start)?;
                    return Some(Spanned::new(
                        Term::QualifiedIdentifier(ident.node),
                        ident.span,
                    ));
                }
                "as" => {
                    // (as ident sort) — qualified identifier
                    let ident = self.parse_spanned_qualified_identifier_inner(start)?;
                    return Some(Spanned::new(
                        Term::QualifiedIdentifier(ident.node),
                        ident.span,
                    ));
                }
                _ => {}
            }
        }

        // Function application: (f args...)
        let func = self.parse_spanned_qualified_identifier()?;
        let mut args = Vec::new();
        while !self.peek_is(TokenKind::RParen) {
            if self.peek().is_none() {
                break;
            }
            args.push(self.parse_term()?);
        }
        let rparen = self.expect_rparen()?;
        Some(Spanned::new(
            Term::Application(func, args),
            Span::new(start, rparen.span.end),
        ))
    }

    fn parse_let(&mut self, start: u32) -> Option<Spanned<Term>> {
        self.next(); // consume 'let'
        self.expect_lparen()?;
        let mut bindings = Vec::new();
        while !self.peek_is(TokenKind::RParen) {
            let lp = self.expect_lparen()?;
            let name = self.parse_spanned_symbol()?;
            let value = self.parse_term()?;
            let rp = self.expect_rparen()?;
            bindings.push(Spanned::new(
                VarBinding { name, value },
                Span::new(lp.span.start, rp.span.end),
            ));
        }
        self.expect_rparen()?;
        let body = self.parse_term()?;
        let rparen = self.expect_rparen()?;
        Some(Spanned::new(
            Term::Let(bindings, Box::new(body)),
            Span::new(start, rparen.span.end),
        ))
    }

    fn parse_quantifier(&mut self, start: u32, is_forall: bool) -> Option<Spanned<Term>> {
        self.next(); // consume 'forall'/'exists'
        self.expect_lparen()?;
        let mut vars = Vec::new();
        while !self.peek_is(TokenKind::RParen) {
            vars.push(self.parse_sorted_var()?);
        }
        self.expect_rparen()?;
        let body = self.parse_term()?;
        let rparen = self.expect_rparen()?;
        let term = if is_forall {
            Term::Forall(vars, Box::new(body))
        } else {
            Term::Exists(vars, Box::new(body))
        };
        Some(Spanned::new(term, Span::new(start, rparen.span.end)))
    }

    fn parse_lambda(&mut self, start: u32) -> Option<Spanned<Term>> {
        self.next(); // consume 'lambda'
        self.expect_lparen()?;
        let mut vars = Vec::new();
        while !self.peek_is(TokenKind::RParen) {
            vars.push(self.parse_sorted_var()?);
        }
        self.expect_rparen()?;
        let body = self.parse_term()?;
        let rparen = self.expect_rparen()?;
        Some(Spanned::new(
            Term::Lambda(vars, Box::new(body)),
            Span::new(start, rparen.span.end),
        ))
    }

    fn parse_match(&mut self, start: u32) -> Option<Spanned<Term>> {
        self.next(); // consume 'match'
        let scrutinee = self.parse_term()?;
        self.expect_lparen()?;
        let mut cases = Vec::new();
        while !self.peek_is(TokenKind::RParen) {
            let lp = self.expect_lparen()?;
            let pattern = self.parse_match_pattern()?;
            let body = self.parse_term()?;
            let rp = self.expect_rparen()?;
            cases.push(Spanned::new(
                MatchCase { pattern, body },
                Span::new(lp.span.start, rp.span.end),
            ));
        }
        self.expect_rparen()?;
        let rparen = self.expect_rparen()?;
        Some(Spanned::new(
            Term::Match(Box::new(scrutinee), cases),
            Span::new(start, rparen.span.end),
        ))
    }

    fn parse_match_pattern(&mut self) -> Option<Spanned<MatchPattern>> {
        let tok = self.peek()?;
        if tok.kind == TokenKind::LParen {
            let lparen = self.next().unwrap();
            let ctor = self.parse_symbol()?;
            let mut vars = Vec::new();
            while !self.peek_is(TokenKind::RParen) {
                vars.push(self.parse_symbol()?);
            }
            let rparen = self.expect_rparen()?;
            Some(Spanned::new(
                MatchPattern::Application(ctor, vars),
                Span::new(lparen.span.start, rparen.span.end),
            ))
        } else {
            let sym = self.parse_spanned_symbol()?;
            let span = sym.span;
            Some(Spanned::new(MatchPattern::Symbol(sym.node), span))
        }
    }

    fn parse_annotated(&mut self, start: u32) -> Option<Spanned<Term>> {
        self.next(); // consume '!'
        let term = self.parse_term()?;
        let mut attrs = Vec::new();
        while !self.peek_is(TokenKind::RParen) {
            if self.peek().is_none() {
                break;
            }
            attrs.push(self.parse_attribute()?);
        }
        let rparen = self.expect_rparen()?;
        Some(Spanned::new(
            Term::Annotated(Box::new(term), attrs),
            Span::new(start, rparen.span.end),
        ))
    }

    // -----------------------------------------------------------------------
    // Identifiers
    // -----------------------------------------------------------------------

    fn parse_symbol(&mut self) -> Option<Symbol> {
        let tok = self.next()?;
        match tok.kind {
            TokenKind::Symbol => Some(Symbol {
                name: self.text(&tok).to_string(),
                quoted: false,
            }),
            TokenKind::QuotedSymbol => {
                let raw = self.text(&tok);
                // Strip the | delimiters
                let inner = &raw[1..raw.len() - 1];
                Some(Symbol {
                    name: inner.to_string(),
                    quoted: true,
                })
            }
            _ => {
                self.error(tok.span, "expected symbol");
                None
            }
        }
    }

    fn parse_spanned_symbol(&mut self) -> Option<Spanned<Symbol>> {
        let tok = self.peek()?;
        let span = tok.span;
        let sym = self.parse_symbol()?;
        Some(Spanned::new(sym, span))
    }

    fn parse_identifier(&mut self) -> Option<Identifier> {
        let tok = self.peek()?;
        if tok.kind == TokenKind::LParen {
            // Could be indexed: (_ sym idx+)
            self.next(); // consume (
            if self.peek_is_symbol("_") {
                self.next(); // consume _
                let sym = self.parse_symbol()?;
                let mut indices = Vec::new();
                while !self.peek_is(TokenKind::RParen) {
                    indices.push(self.parse_index()?);
                }
                self.expect_rparen()?;
                Some(Identifier::Indexed(sym, indices))
            } else {
                // Not an identifier — this is an error in the identifier position
                let err_start = self.peek().map(|t| t.span.start).unwrap_or(0);
                self.error(
                    Span::new(err_start, err_start),
                    "expected '_' for indexed identifier",
                );
                self.skip_to_balanced_rparen();
                None
            }
        } else {
            let sym = self.parse_symbol()?;
            Some(Identifier::Simple(sym))
        }
    }

    fn parse_spanned_identifier(&mut self) -> Option<Spanned<Identifier>> {
        let tok = self.peek()?;
        let start = tok.span.start;

        if tok.kind == TokenKind::LParen {
            let ident = self.parse_identifier()?;
            // For indexed identifiers, the closing ) was consumed by parse_identifier.
            // Use position just after the last consumed token.
            let end = self
                .peeked
                .as_ref()
                .map(|t| t.span.start)
                .unwrap_or(self.src.len() as u32);
            Some(Spanned::new(ident, Span::new(start, end)))
        } else {
            // Simple identifier — span is just the symbol token
            let end = tok.span.end;
            let ident = self.parse_identifier()?;
            Some(Spanned::new(ident, Span::new(start, end)))
        }
    }

    /// Parse a qualified identifier in non-compound context.
    fn parse_spanned_qualified_identifier(&mut self) -> Option<Spanned<QualifiedIdentifier>> {
        let tok = self.peek()?;
        let _start = tok.span.start;

        if tok.kind == TokenKind::LParen {
            // Might be (as id sort) or (_ sym idx+)
            let lparen = self.next().unwrap();
            self.parse_spanned_qualified_identifier_inner(lparen.span.start)
        } else {
            let ident = self.parse_spanned_identifier()?;
            let span = ident.span;
            Some(Spanned::new(QualifiedIdentifier::Simple(ident), span))
        }
    }

    /// Parse inside a `(` that was already consumed. Handles `as` and `_`.
    fn parse_spanned_qualified_identifier_inner(
        &mut self,
        start: u32,
    ) -> Option<Spanned<QualifiedIdentifier>> {
        let tok = self.peek()?;
        let name = &self.src[tok.span.start as usize..tok.span.end as usize];

        if name == "as" {
            self.next(); // consume 'as'
            let ident = self.parse_spanned_identifier()?;
            let sort = self.parse_sort()?;
            let rparen = self.expect_rparen()?;
            Some(Spanned::new(
                QualifiedIdentifier::As(ident, sort),
                Span::new(start, rparen.span.end),
            ))
        } else if name == "_" {
            self.next(); // consume '_'
            let sym = self.parse_symbol()?;
            let _sym_span_start = start; // approximate
            let mut indices = Vec::new();
            while !self.peek_is(TokenKind::RParen) {
                indices.push(self.parse_index()?);
            }
            let rparen = self.expect_rparen()?;
            let ident = Identifier::Indexed(sym, indices);
            let ident_span = Span::new(start, rparen.span.end);
            Some(Spanned::new(
                QualifiedIdentifier::Simple(Spanned::new(ident, ident_span)),
                ident_span,
            ))
        } else {
            // It's a function application or other compound — return simple ident
            let ident = self.parse_spanned_identifier()?;
            let span = Span::new(start, ident.span.end);
            Some(Spanned::new(QualifiedIdentifier::Simple(ident), span))
        }
    }

    fn parse_index(&mut self) -> Option<Index> {
        let tok = self.next()?;
        match tok.kind {
            TokenKind::Numeral => {
                let val = self.text(&tok).parse::<u64>().unwrap_or(0);
                Some(Index::Numeral(val))
            }
            TokenKind::Symbol | TokenKind::QuotedSymbol => {
                let name = if tok.kind == TokenKind::QuotedSymbol {
                    let raw = self.text(&tok);
                    Symbol {
                        name: raw[1..raw.len() - 1].to_string(),
                        quoted: true,
                    }
                } else {
                    Symbol {
                        name: self.text(&tok).to_string(),
                        quoted: false,
                    }
                };
                Some(Index::Symbol(name))
            }
            _ => {
                self.error(tok.span, "expected index (numeral or symbol)");
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // Sorted variables
    // -----------------------------------------------------------------------

    fn parse_sorted_var(&mut self) -> Option<Spanned<SortedVar>> {
        let lparen = self.expect_lparen()?;
        let name = self.parse_spanned_symbol()?;
        let sort = self.parse_sort()?;
        let rparen = self.expect_rparen()?;
        Some(Spanned::new(
            SortedVar { name, sort },
            Span::new(lparen.span.start, rparen.span.end),
        ))
    }

    // -----------------------------------------------------------------------
    // Attributes
    // -----------------------------------------------------------------------

    fn parse_keyword(&mut self) -> Option<Spanned<String>> {
        let tok = self.expect(TokenKind::Keyword)?;
        let text = self.text(&tok).to_string();
        Some(Spanned::new(text, tok.span))
    }

    fn parse_attribute(&mut self) -> Option<Attribute> {
        let keyword = self.parse_keyword()?;

        // Check if the next token could be an attribute value
        let value = match self.peek() {
            Some(tok) if tok.kind != TokenKind::Keyword && tok.kind != TokenKind::RParen => {
                Some(self.parse_attribute_value()?)
            }
            _ => None,
        };

        Some(Attribute { keyword, value })
    }

    fn parse_attribute_value(&mut self) -> Option<Spanned<AttributeValue>> {
        let tok = self.peek()?;
        let _start = tok.span.start;

        match tok.kind {
            TokenKind::Numeral
            | TokenKind::Decimal
            | TokenKind::Hexadecimal
            | TokenKind::Binary
            | TokenKind::StringLiteral => {
                let tok = self.next().unwrap();
                let text = self.text(&tok).to_string();
                Some(Spanned::new(
                    AttributeValue::Constant(text),
                    tok.span,
                ))
            }
            TokenKind::Symbol | TokenKind::QuotedSymbol => {
                let sym = self.parse_spanned_symbol()?;
                let span = sym.span;
                Some(Spanned::new(AttributeValue::Symbol(sym.node), span))
            }
            TokenKind::LParen => {
                let lparen = self.next().unwrap();
                let mut items = Vec::new();
                while !self.peek_is(TokenKind::RParen) {
                    if self.peek().is_none() {
                        break;
                    }
                    items.push(self.parse_sexpr()?);
                }
                let rparen = self.expect_rparen()?;
                Some(Spanned::new(
                    AttributeValue::SExpr(items),
                    Span::new(lparen.span.start, rparen.span.end),
                ))
            }
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // S-expressions
    // -----------------------------------------------------------------------

    fn parse_sexpr(&mut self) -> Option<Spanned<SExpr>> {
        let tok = self.peek()?;
        let _start = tok.span.start;

        match tok.kind {
            TokenKind::Numeral
            | TokenKind::Decimal
            | TokenKind::Hexadecimal
            | TokenKind::Binary
            | TokenKind::StringLiteral => {
                let tok = self.next().unwrap();
                let c = self.parse_constant(&tok);
                Some(Spanned::new(SExpr::Constant(c), tok.span))
            }
            TokenKind::Symbol | TokenKind::QuotedSymbol => {
                let sym = self.parse_spanned_symbol()?;
                let span = sym.span;
                Some(Spanned::new(SExpr::Symbol(sym.node), span))
            }
            TokenKind::Keyword => {
                let kw = self.parse_keyword()?;
                let span = kw.span;
                Some(Spanned::new(SExpr::Keyword(kw.node), span))
            }
            TokenKind::LParen => {
                let lparen = self.next().unwrap();
                let mut items = Vec::new();
                while !self.peek_is(TokenKind::RParen) {
                    if self.peek().is_none() {
                        break;
                    }
                    items.push(self.parse_sexpr()?);
                }
                let rparen = self.expect_rparen()?;
                Some(Spanned::new(
                    SExpr::List(items),
                    Span::new(lparen.span.start, rparen.span.end),
                ))
            }
            _ => {
                let tok = self.next().unwrap();
                self.error(tok.span, "unexpected token in s-expression");
                None
            }
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn parse_constant(&self, tok: &Token) -> Constant {
        let text = self.text(tok);
        match tok.kind {
            TokenKind::Numeral => Constant::Numeral(text.parse::<u64>().unwrap_or(0)),
            TokenKind::Decimal => Constant::Decimal(text.to_string()),
            TokenKind::Hexadecimal => Constant::Hexadecimal(text.to_string()),
            TokenKind::Binary => Constant::Binary(text.to_string()),
            TokenKind::StringLiteral => Constant::String(self.string_value(tok)),
            _ => Constant::String(text.to_string()),
        }
    }

    fn string_value(&self, tok: &Token) -> String {
        let raw = self.text(tok);
        // Strip quotes and unescape ""
        let inner = &raw[1..raw.len() - 1];
        inner.replace("\"\"", "\"")
    }

    fn parse_numeral_value(&mut self) -> Option<u64> {
        let tok = self.expect(TokenKind::Numeral)?;
        let text = self.text(&tok);
        Some(text.parse::<u64>().unwrap_or(0))
    }

    fn peek_is(&mut self, kind: TokenKind) -> bool {
        matches!(self.peek(), Some(t) if t.kind == kind)
    }

    fn peek_is_symbol(&mut self, name: &str) -> bool {
        match self.peek() {
            Some(t) if t.kind == TokenKind::Symbol => {
                &self.src[t.span.start as usize..t.span.end as usize] == name
            }
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty() {
        let result = parse("");
        assert!(result.script.commands.is_empty());
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_parse_set_logic() {
        let result = parse("(set-logic QF_LIA)");
        assert_eq!(result.script.commands.len(), 1);
        match &result.script.commands[0].node {
            Command::SetLogic(sym) => assert_eq!(sym.node.name, "QF_LIA"),
            other => panic!("expected SetLogic, got {:?}", other),
        }
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_parse_declare_fun() {
        let result = parse("(declare-fun x () Int)");
        assert_eq!(result.script.commands.len(), 1);
        match &result.script.commands[0].node {
            Command::DeclareFun(name, params, sort) => {
                assert_eq!(name.node.name, "x");
                assert!(params.is_empty());
                match &sort.node {
                    Sort::Simple(Identifier::Simple(s)) => assert_eq!(s.name, "Int"),
                    other => panic!("expected simple sort, got {:?}", other),
                }
            }
            other => panic!("expected DeclareFun, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_declare_const() {
        let result = parse("(declare-const y Bool)");
        match &result.script.commands[0].node {
            Command::DeclareConst(name, _sort) => {
                assert_eq!(name.node.name, "y");
            }
            other => panic!("expected DeclareConst, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_assert() {
        let result = parse("(assert (> x 0))");
        assert_eq!(result.script.commands.len(), 1);
        match &result.script.commands[0].node {
            Command::Assert(_) => {}
            other => panic!("expected Assert, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_check_sat() {
        let result = parse("(check-sat)");
        match &result.script.commands[0].node {
            Command::CheckSat => {}
            other => panic!("expected CheckSat, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_push_pop() {
        let result = parse("(push 1)(pop 1)");
        assert_eq!(result.script.commands.len(), 2);
        match &result.script.commands[0].node {
            Command::Push(n) => assert_eq!(*n, 1),
            other => panic!("expected Push, got {:?}", other),
        }
        match &result.script.commands[1].node {
            Command::Pop(n) => assert_eq!(*n, 1),
            other => panic!("expected Pop, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_define_fun() {
        let src = "(define-fun add ((x Int) (y Int)) Int (+ x y))";
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        match &result.script.commands[0].node {
            Command::DefineFun(def) => {
                assert_eq!(def.name.node.name, "add");
                assert_eq!(def.params.len(), 2);
            }
            other => panic!("expected DefineFun, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_let_term() {
        let src = "(assert (let ((x 1)) (+ x 2)))";
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    }

    #[test]
    fn test_parse_quantifier() {
        let src = "(assert (forall ((x Int)) (> x 0)))";
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    }

    #[test]
    fn test_parse_annotated_term() {
        let src = "(assert (! (> x 0) :named foo))";
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    }

    #[test]
    fn test_parse_indexed_identifier() {
        let src = "(assert (= (_ bv0 32) x))";
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    }

    #[test]
    fn test_parse_unknown_command() {
        let src = "(my-custom-command arg1 42)";
        let result = parse(src);
        assert_eq!(result.script.commands.len(), 1);
        match &result.script.commands[0].node {
            Command::Unknown(name, args) => {
                assert_eq!(name, "my-custom-command");
                assert_eq!(args.len(), 2);
            }
            other => panic!("expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn test_error_recovery() {
        // Malformed first command, valid second
        let src = "(assert )(check-sat)";
        let result = parse(src);
        // Should recover and parse check-sat
        assert!(!result.diagnostics.is_empty());
        let check_sat_found = result
            .script
            .commands
            .iter()
            .any(|c| matches!(c.node, Command::CheckSat));
        assert!(check_sat_found, "should recover and parse check-sat");
    }

    #[test]
    fn test_full_script() {
        let src = r#"
(set-logic QF_LIA)
(declare-fun x () Int)
(declare-fun y () Int)
(assert (> x 0))
(assert (< y 10))
(assert (= (+ x y) 7))
(check-sat)
(get-model)
(exit)
"#;
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(result.script.commands.len(), 9);
    }

    #[test]
    fn test_declare_datatypes() {
        let src = r#"
(declare-datatypes ((Color 0)) (
  ((Red) (Green) (Blue))
))
"#;
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        match &result.script.commands[0].node {
            Command::DeclareDatatypes(sorts, decs) => {
                assert_eq!(sorts.len(), 1);
                assert_eq!(sorts[0].0.node.name, "Color");
                assert_eq!(decs[0].node.constructors.len(), 3);
            }
            other => panic!("expected DeclareDatatypes, got {:?}", other),
        }
    }

    #[test]
    fn test_comments_ignored() {
        let src = "; comment\n(check-sat) ; inline comment\n";
        let result = parse(src);
        assert_eq!(result.script.commands.len(), 1);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn test_declare_sort_no_arity() {
        // Z3 extension: declare-sort without arity defaults to 0
        let result = parse("(declare-sort MySort)");
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        match &result.script.commands[0].node {
            Command::DeclareSort(name, arity) => {
                assert_eq!(name.node.name, "MySort");
                assert_eq!(*arity, 0);
            }
            other => panic!("expected DeclareSort, got {:?}", other),
        }
    }

    #[test]
    fn test_z3_legacy_declare_datatypes() {
        // Z3 legacy syntax: (declare-datatypes () ((TypeName (Ctor1) (Ctor2 (field Sort)))))
        let src = "(declare-datatypes () ((Fuel (ZFuel) (SFuel (prec Fuel)))))";
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        match &result.script.commands[0].node {
            Command::DeclareDatatypes(sorts, decs) => {
                assert_eq!(sorts.len(), 1);
                assert_eq!(sorts[0].0.node.name, "Fuel");
                assert_eq!(decs[0].node.constructors.len(), 2);
                assert_eq!(decs[0].node.constructors[0].node.name.node.name, "ZFuel");
                assert_eq!(decs[0].node.constructors[1].node.name.node.name, "SFuel");
                assert_eq!(decs[0].node.constructors[1].node.selectors.len(), 1);
                assert_eq!(decs[0].node.constructors[1].node.selectors[0].name.node.name, "prec");
            }
            other => panic!("expected DeclareDatatypes, got {:?}", other),
        }
    }

    #[test]
    fn test_identifier_span_precision() {
        // Verify that simple identifier references have tight spans
        let src = "(assert (= foo bar))";
        let result = parse(src);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        match &result.script.commands[0].node {
            Command::Assert(term) => match &term.node {
                Term::Application(func, args) => {
                    // '=' should have a tight span
                    let eq_span = func.span;
                    assert_eq!(&src[eq_span.start as usize..eq_span.end as usize], "=");
                    // 'foo' and 'bar' should have tight spans
                    for arg in args {
                        match &arg.node {
                            Term::QualifiedIdentifier(QualifiedIdentifier::Simple(ident)) => {
                                let text =
                                    &src[ident.span.start as usize..ident.span.end as usize];
                                assert!(
                                    text == "foo" || text == "bar",
                                    "unexpected ident text: '{}'",
                                    text
                                );
                            }
                            other => panic!("expected QualifiedIdentifier, got {:?}", other),
                        }
                    }
                }
                other => panic!("expected Application, got {:?}", other),
            },
            other => panic!("expected Assert, got {:?}", other),
        }
    }
}
