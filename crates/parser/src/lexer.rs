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

use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    LParen,
    RParen,
    Numeral,
    Decimal,
    Hexadecimal,
    Binary,
    StringLiteral,
    Symbol,
    QuotedSymbol,
    Keyword,
    Comment,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    fn new(kind: TokenKind, start: u32, end: u32) -> Self {
        Self {
            kind,
            span: Span::new(start, end),
        }
    }
}

/// A streaming lexer for SMT-LIB v2.6 (with Z3 extensions).
///
/// Operates on a byte slice and yields tokens one at a time.
pub struct Lexer<'a> {
    src: &'a [u8],
    pos: u32,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: 0,
        }
    }

    #[inline]
    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos as usize).copied()
    }

    #[inline]
    fn advance(&mut self) -> Option<u8> {
        let b = self.src.get(self.pos as usize).copied()?;
        self.pos += 1;
        Some(b)
    }

    fn skip_whitespace(&mut self) {
        while let Some(b) = self.peek() {
            match b {
                b' ' | b'\t' | b'\n' | b'\r' => {
                    self.pos += 1;
                }
                _ => break,
            }
        }
    }

    pub fn next_token(&mut self) -> Option<Token> {
        self.skip_whitespace();
        let start = self.pos;
        let b = self.peek()?;

        match b {
            b'(' => {
                self.advance();
                Some(Token::new(TokenKind::LParen, start, self.pos))
            }
            b')' => {
                self.advance();
                Some(Token::new(TokenKind::RParen, start, self.pos))
            }
            b';' => Some(self.lex_comment(start)),
            b'"' => Some(self.lex_string(start)),
            b'|' => Some(self.lex_quoted_symbol(start)),
            b':' => Some(self.lex_keyword(start)),
            b'#' => Some(self.lex_hash_literal(start)),
            b'0'..=b'9' => Some(self.lex_number(start)),
            _ if is_symbol_char(b) => Some(self.lex_symbol(start)),
            _ => {
                self.advance();
                Some(Token::new(TokenKind::Error, start, self.pos))
            }
        }
    }

    fn lex_comment(&mut self, start: u32) -> Token {
        // Skip everything until end of line
        while let Some(b) = self.peek() {
            if b == b'\n' || b == b'\r' {
                break;
            }
            self.advance();
        }
        Token::new(TokenKind::Comment, start, self.pos)
    }

    fn lex_string(&mut self, start: u32) -> Token {
        self.advance(); // skip opening "
        loop {
            match self.advance() {
                Some(b'"') => {
                    // SMT-LIB v2.6: "" inside a string is an escaped quote
                    if self.peek() == Some(b'"') {
                        self.advance();
                    } else {
                        break;
                    }
                }
                Some(_) => {}
                None => break, // unterminated string
            }
        }
        Token::new(TokenKind::StringLiteral, start, self.pos)
    }

    fn lex_quoted_symbol(&mut self, start: u32) -> Token {
        self.advance(); // skip opening |
        loop {
            match self.advance() {
                Some(b'|') => break,
                Some(b'\\') => {
                    // Some implementations allow backslash escapes in quoted symbols
                    self.advance();
                }
                Some(_) => {}
                None => break, // unterminated
            }
        }
        Token::new(TokenKind::QuotedSymbol, start, self.pos)
    }

    fn lex_keyword(&mut self, start: u32) -> Token {
        self.advance(); // skip :
        while let Some(b) = self.peek() {
            if is_symbol_char(b) {
                self.advance();
            } else {
                break;
            }
        }
        Token::new(TokenKind::Keyword, start, self.pos)
    }

    fn lex_hash_literal(&mut self, start: u32) -> Token {
        self.advance(); // skip #
        match self.peek() {
            Some(b'x' | b'X') => {
                self.advance();
                while let Some(b) = self.peek() {
                    if b.is_ascii_hexdigit() {
                        self.advance();
                    } else {
                        break;
                    }
                }
                Token::new(TokenKind::Hexadecimal, start, self.pos)
            }
            Some(b'b' | b'B') => {
                self.advance();
                while let Some(b) = self.peek() {
                    if b == b'0' || b == b'1' {
                        self.advance();
                    } else {
                        break;
                    }
                }
                Token::new(TokenKind::Binary, start, self.pos)
            }
            _ => {
                // Unknown # literal, treat as error
                Token::new(TokenKind::Error, start, self.pos)
            }
        }
    }

    fn lex_number(&mut self, start: u32) -> Token {
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }
        if self.peek() == Some(b'.') {
            // Check that a digit follows the dot (otherwise it's just a numeral)
            if self
                .src
                .get((self.pos + 1) as usize)
                .is_some_and(|b| b.is_ascii_digit())
            {
                self.advance(); // skip .
                while let Some(b) = self.peek() {
                    if b.is_ascii_digit() {
                        self.advance();
                    } else {
                        break;
                    }
                }
                return Token::new(TokenKind::Decimal, start, self.pos);
            }
        }
        Token::new(TokenKind::Numeral, start, self.pos)
    }

    fn lex_symbol(&mut self, start: u32) -> Token {
        while let Some(b) = self.peek() {
            if is_symbol_char(b) {
                self.advance();
            } else {
                break;
            }
        }
        Token::new(TokenKind::Symbol, start, self.pos)
    }

    /// Get the text slice for a token.
    pub fn text(&self, token: &Token) -> &'a str {
        let s = self.src.get(token.span.start as usize..token.span.end as usize);
        // Safety: SMT-LIB is ASCII-compatible; we validated during lexing
        s.map(|b| std::str::from_utf8(b).unwrap_or("<invalid utf8>"))
            .unwrap_or("")
    }

    /// Get the full source text.
    pub fn source(&self) -> &'a str {
        std::str::from_utf8(self.src).unwrap_or("")
    }
}

/// Returns true if `b` is a valid character in an unquoted SMT-LIB symbol.
/// Allowed: letters, digits, and ~ ! @ $ % ^ & * _ - + = < > . ? /
#[inline]
fn is_symbol_char(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'~' | b'!'
                | b'@'
                | b'$'
                | b'%'
                | b'^'
                | b'&'
                | b'*'
                | b'_'
                | b'-'
                | b'+'
                | b'='
                | b'<'
                | b'>'
                | b'.'
                | b'?'
                | b'/'
        )
}

/// Convenience: collect all tokens (excluding comments) from source.
pub fn tokenize(src: &str) -> Vec<Token> {
    let mut lexer = Lexer::new(src);
    let mut tokens = Vec::new();
    while let Some(tok) = lexer.next_token() {
        if tok.kind != TokenKind::Comment {
            tokens.push(tok);
        }
    }
    tokens
}

/// Convenience: collect all tokens including comments.
pub fn tokenize_with_comments(src: &str) -> Vec<Token> {
    let mut lexer = Lexer::new(src);
    let mut tokens = Vec::new();
    while let Some(tok) = lexer.next_token() {
        tokens.push(tok);
    }
    tokens
}

/// Get the text slice for a span within source.
pub fn span_text<'a>(src: &'a str, span: &Span) -> &'a str {
    &src[span.start as usize..span.end as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(src: &str) -> Vec<(TokenKind, &str)> {
        let mut lexer = Lexer::new(src);
        let mut out = Vec::new();
        while let Some(tok) = lexer.next_token() {
            let text = &src[tok.span.start as usize..tok.span.end as usize];
            out.push((tok.kind, text));
        }
        out
    }

    #[test]
    fn test_parens() {
        let tokens = lex("( )");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], (TokenKind::LParen, "("));
        assert_eq!(tokens[1], (TokenKind::RParen, ")"));
    }

    #[test]
    fn test_numeral() {
        let tokens = lex("0 42 007");
        assert_eq!(tokens.len(), 3);
        assert!(tokens.iter().all(|(k, _)| *k == TokenKind::Numeral));
        assert_eq!(tokens[1].1, "42");
    }

    #[test]
    fn test_decimal() {
        let tokens = lex("3.14 0.0");
        assert_eq!(tokens.len(), 2);
        assert!(tokens.iter().all(|(k, _)| *k == TokenKind::Decimal));
        assert_eq!(tokens[0].1, "3.14");
    }

    #[test]
    fn test_hex_and_binary() {
        let tokens = lex("#xFF #b1010");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0], (TokenKind::Hexadecimal, "#xFF"));
        assert_eq!(tokens[1], (TokenKind::Binary, "#b1010"));
    }

    #[test]
    fn test_string_literal() {
        let tokens = lex(r#""hello" "with""quotes""#);
        assert_eq!(tokens.len(), 2);
        assert!(tokens.iter().all(|(k, _)| *k == TokenKind::StringLiteral));
        assert_eq!(tokens[0].1, "\"hello\"");
        assert_eq!(tokens[1].1, "\"with\"\"quotes\"");
    }

    #[test]
    fn test_symbol() {
        let tokens = lex("foo bar+ baz_123 <= >=");
        assert!(tokens.iter().all(|(k, _)| *k == TokenKind::Symbol));
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].1, "foo");
        assert_eq!(tokens[3].1, "<=");
    }

    #[test]
    fn test_quoted_symbol() {
        let tokens = lex("|hello world| |with spaces|");
        assert_eq!(tokens.len(), 2);
        assert!(tokens.iter().all(|(k, _)| *k == TokenKind::QuotedSymbol));
        assert_eq!(tokens[0].1, "|hello world|");
    }

    #[test]
    fn test_keyword() {
        let tokens = lex(":status :named :pattern");
        assert_eq!(tokens.len(), 3);
        assert!(tokens.iter().all(|(k, _)| *k == TokenKind::Keyword));
        assert_eq!(tokens[0].1, ":status");
    }

    #[test]
    fn test_comment() {
        let tokens = lex("; this is a comment\n(assert true)");
        assert_eq!(tokens.len(), 5); // comment, (, assert, true, )
        assert_eq!(tokens[0], (TokenKind::Comment, "; this is a comment"));
        assert_eq!(tokens[1], (TokenKind::LParen, "("));
    }

    #[test]
    fn test_smt_snippet() {
        let src = r#"
(set-logic QF_LIA)
(declare-fun x () Int)
(assert (> x 0))
(check-sat)
"#;
        let tokens = tokenize(src);
        // Should have meaningful tokens without comments
        assert!(tokens.len() > 10);
        assert_eq!(tokens[0].kind, TokenKind::LParen);
    }

    #[test]
    fn test_z3_indexed_identifier() {
        let tokens = lex("(_ bv32 8)");
        assert_eq!(tokens[0], (TokenKind::LParen, "("));
        assert_eq!(tokens[1], (TokenKind::Symbol, "_"));
        assert_eq!(tokens[2], (TokenKind::Symbol, "bv32"));
        assert_eq!(tokens[3], (TokenKind::Numeral, "8"));
        assert_eq!(tokens[4], (TokenKind::RParen, ")"));
    }

    #[test]
    fn test_empty_input() {
        let tokens = lex("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_whitespace_only() {
        let tokens = lex("   \n\t\r  ");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_numeral_followed_by_dot_no_digit() {
        // "42." without a following digit should be numeral + error/symbol for dot
        let tokens = lex("42.)");
        assert_eq!(tokens[0], (TokenKind::Numeral, "42"));
        assert_eq!(tokens[1], (TokenKind::Symbol, "."));
        assert_eq!(tokens[2], (TokenKind::RParen, ")"));
    }
}
