//! Lexer (tokenizer) for the Silq programming language.
//!
//! The lexer processes source text and produces a stream of tokens.
//! It supports:
//! - UTF-8 encoded source files
//! - Unicode math symbols (𝔹, ℕ, ℤ, ℚ, ℝ, ℂ, →, ⇒, ↦, ←, ×, etc.)
//! - String literals (regular, WYSIWYG, delimited)
//! - Character literals
//! - Numeric literals (decimal, hex, binary, float, rational)
//! - Single-line and nested block comments
//! - Annotations (@[...])

use crate::errors::Location;
use crate::token::{lookup_keyword, Token, TokenType};

/// The lexer processes source text and produces tokens.
pub struct Lexer {
    /// The source text (UTF-8 bytes).
    source: Vec<u8>,
    /// Current position in the source.
    pos: usize,
    /// Current line number (1-based).
    line: u32,
    /// Current column number (1-based).
    col: u32,
    /// Last token produced (for error recovery).
    last_token: Option<Token>,
    /// Peeked token (for lookahead).
    peeked: Option<Token>,
}

impl Lexer {
    /// Create a new lexer from source text.
    pub fn new(source: &str) -> Self {
        Lexer {
            source: source.as_bytes().to_vec(),
            pos: 0,
            line: 1,
            col: 1,
            last_token: None,
            peeked: None,
        }
    }

    /// Get the current location.
    fn location(&self) -> Location {
        Location {
            line: self.line,
            col: self.col,
            offset: self.pos,
        }
    }

    /// Check if we've reached the end of input.
    fn is_eof(&self) -> bool {
        self.pos >= self.source.len()
    }

    /// Peek at the current byte.
    fn peek_byte(&self) -> Option<u8> {
        self.source.get(self.pos).copied()
    }

    /// Peek at the byte at pos + offset.
    fn peek_byte_at(&self, offset: usize) -> Option<u8> {
        self.source.get(self.pos + offset).copied()
    }

    /// Advance one byte, updating line/col.
    fn advance(&mut self) -> Option<u8> {
        if self.pos >= self.source.len() {
            return None;
        }
        let ch = self.source[self.pos];
        self.pos += 1;
        if ch == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(ch)
    }

    /// Check if the next bytes match a string, and advance past them.
    #[allow(dead_code)]
    fn matches(&mut self, s: &[u8]) -> bool {
        let end = self.pos + s.len();
        if end <= self.source.len() && &self.source[self.pos..end] == s {
            self.pos = end;
            true
        } else {
            false
        }
    }

    /// Skip whitespace and comments.
    fn skip_whitespace_and_comments(&mut self) {
        while !self.is_eof() {
            match self.peek_byte().unwrap() {
                // Whitespace
                b' ' | b'\t' | b'\r' | b'\n' => {
                    self.advance();
                }
                // Line comment: //...\n
                b'/' => {
                    if self.peek_byte_at(1) == Some(b'/') {
                        while !self.is_eof() && self.peek_byte() != Some(b'\n') {
                            self.advance();
                        }
                    } else if self.peek_byte_at(1) == Some(b'+') {
                        // Nested block comment: /+ ... +/
                        self.skip_nested_comment();
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
    }

    /// Skip a nested block comment: /+ ... +/ (can be nested).
    fn skip_nested_comment(&mut self) {
        // Skip the opening /+
        self.advance(); // /
        self.advance(); // +
        let mut depth: i32 = 1;
        while depth > 0 && !self.is_eof() {
            if self.peek_byte() == Some(b'/') && self.peek_byte_at(1) == Some(b'+') {
                self.advance();
                self.advance();
                depth += 1;
            } else if self.peek_byte() == Some(b'+') && self.peek_byte_at(1) == Some(b'/') {
                self.advance();
                self.advance();
                depth -= 1;
            } else {
                self.advance();
            }
        }
    }

    /// Read an identifier or keyword token.
    fn read_identifier(&mut self) -> Token {
        let start = self.pos;
        let loc = self.location();

        // Consume characters that can be in identifiers.
        // Silq allows Unicode letters, digits, underscores, primes ('), and Greek/math letters.
        while !self.is_eof() {
            let ch = self.peek_byte().unwrap();
            if ch.is_ascii_alphanumeric() || ch == b'_' || ch == b'\'' {
                self.advance();
            } else if ch >= 0x80 {
                // Unicode character - read the full UTF-8 sequence
                let seq_start = self.pos;
                self.advance();
                let s = std::str::from_utf8(&self.source[seq_start..self.pos])
                    .unwrap_or("");
                if let Some(c) = s.chars().next() {
                    if c.is_alphabetic() || c == '_' {
                        continue;
                    }
                }
                // Not a valid identifier continuation, backtrack
                self.pos = seq_start;
                break;
            } else {
                break;
            }
        }

        let text = std::str::from_utf8(&self.source[start..self.pos])
            .unwrap_or("")
            .to_string();

        // Check if it's a keyword
        if let Some(kw_type) = lookup_keyword(&text) {
            Token::new(kw_type, text, loc.line, loc.col, loc.offset)
        } else {
            Token::new(TokenType::Identifier, text, loc.line, loc.col, loc.offset)
        }
    }

    /// Read a numeric literal (integer, float, rational).
    fn read_number(&mut self) -> Token {
        let start = self.pos;
        let loc = self.location();
        let mut is_float = false;
        let mut is_hex = false;
        let mut is_binary = false;

        // Check for hex or binary prefix
        if self.peek_byte() == Some(b'0') {
            if self.peek_byte_at(1) == Some(b'x') || self.peek_byte_at(1) == Some(b'X') {
                is_hex = true;
                self.advance(); // 0
                self.advance(); // x
            } else if self.peek_byte_at(1) == Some(b'b') || self.peek_byte_at(1) == Some(b'B') {
                is_binary = true;
                self.advance(); // 0
                self.advance(); // b
            }
        }

        // Read digits
        while !self.is_eof() {
            let ch = self.peek_byte().unwrap();
            if is_hex {
                if ch.is_ascii_hexdigit() || ch == b'_' {
                    self.advance();
                } else {
                    break;
                }
            } else if is_binary {
                if ch == b'0' || ch == b'1' || ch == b'_' {
                    self.advance();
                } else {
                    break;
                }
            } else if ch.is_ascii_digit() || ch == b'_' {
                self.advance();
            } else if ch == b'.' && self.peek_byte_at(1).map_or(false, |c| c.is_ascii_digit()) {
                // Float literal
                is_float = true;
                self.advance(); // .
                while !self.is_eof() && self.peek_byte().unwrap().is_ascii_digit() {
                    self.advance();
                }
                // Optional exponent
                if self.peek_byte() == Some(b'e') || self.peek_byte() == Some(b'E') {
                    self.advance();
                    if self.peek_byte() == Some(b'+') || self.peek_byte() == Some(b'-') {
                        self.advance();
                    }
                    while !self.is_eof() && self.peek_byte().unwrap().is_ascii_digit() {
                        self.advance();
                    }
                }
            } else if ch == b'\\' {
                // Rational literal: num\den
                // Check if the next chars form a denominator
                break;
            } else if ch == b'e' || ch == b'E' {
                // Scientific notation
                if self.peek_byte_at(1).map_or(false, |c| c == b'+' || c == b'-' || c.is_ascii_digit()) {
                    is_float = true;
                    self.advance(); // e
                    if self.peek_byte() == Some(b'+') || self.peek_byte() == Some(b'-') {
                        self.advance();
                    }
                    while !self.is_eof() && self.peek_byte().unwrap().is_ascii_digit() {
                        self.advance();
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Check for rational literal
        if self.peek_byte() == Some(b'\\') {
            let _ = self.advance(); // \
            let _denom_start = self.pos;
            while !self.is_eof() && self.peek_byte().unwrap().is_ascii_digit() {
                self.advance();
            }
            let text = std::str::from_utf8(&self.source[start..self.pos])
                .unwrap_or("0").to_string();
            return Token::new(TokenType::RationalLit, text, loc.line, loc.col, loc.offset);
        }

        let text = std::str::from_utf8(&self.source[start..self.pos])
            .unwrap_or("0").to_string();

        if is_float {
            Token::new(TokenType::FloatLit, text, loc.line, loc.col, loc.offset)
        } else {
            Token::new(TokenType::IntLit, text, loc.line, loc.col, loc.offset)
        }
    }

    /// Read a string literal.
    fn read_string(&mut self) -> Token {
        let loc = self.location();
        let quote = self.advance().unwrap(); // opening quote
        let mut result = String::new();
        let mut escaped = false;

        // Check for WYSIWYG string: r"..."
        if quote == b'r' || quote == b'R' {
            if self.peek_byte() == Some(b'"') {
                self.advance(); // skip "
                while !self.is_eof() {
                    let ch = self.peek_byte().unwrap();
                    if ch == b'"' {
                        self.advance();
                        break;
                    }
                    result.push(ch as char);
                    self.advance();
                }
                return Token::new(TokenType::StringLit, result, loc.line, loc.col, loc.offset);
            }
        }

        // Regular string literal
        while !self.is_eof() {
            let ch = self.advance().unwrap();
            if escaped {
                match ch {
                    b'n' => result.push('\n'),
                    b'r' => result.push('\r'),
                    b't' => result.push('\t'),
                    b'\\' => result.push('\\'),
                    b'"' => result.push('"'),
                    b'\'' => result.push('\''),
                    b'0' => result.push('\0'),
                    c => result.push(c as char),
                }
                escaped = false;
            } else if ch == b'\\' {
                escaped = true;
            } else if ch == quote {
                break;
            } else {
                result.push(ch as char);
            }
        }

        Token::new(TokenType::StringLit, result, loc.line, loc.col, loc.offset)
    }

    /// Read a character literal: 'c', '\n', '\u{XXXX}'.
    fn read_char(&mut self) -> Token {
        let loc = self.location();
        self.advance(); // opening '
        let mut result = '?';

        if !self.is_eof() {
            let ch = self.advance().unwrap();
            if ch == b'\\' {
                if !self.is_eof() {
                    let ch = self.advance().unwrap();
                    match ch {
                        b'n' => result = '\n',
                        b'r' => result = '\r',
                        b't' => result = '\t',
                        b'\\' => result = '\\',
                        b'\'' => result = '\'',
                        b'"' => result = '"',
                        b'0' => result = '\0',
                        c => result = c as char,
                    }
                }
            } else {
                result = ch as char;
            }
        }

        // Skip closing quote
        if self.peek_byte() == Some(b'\'') {
            self.advance();
        }

        Token::new(TokenType::CharLit, result.to_string(), loc.line, loc.col, loc.offset)
    }

    /// Read a Unicode math token (single-char Unicode operators).
    fn read_unicode_token(&mut self, _first_byte: u8) -> Option<Token> {
        // Read the full UTF-8 sequence
        let seq_start = self.pos;
        self.advance();
        while !self.is_eof() && self.peek_byte().unwrap() >= 0x80 && self.peek_byte().unwrap() < 0xC0 {
            self.advance();
        }
        let seq = std::str::from_utf8(&self.source[seq_start..self.pos]).unwrap_or("");
        let ch = seq.chars().next();

        let loc = self.location();

        match ch {
            Some('·') => Some(Token::new(TokenType::Mul, "·".into(), loc.line, loc.col, loc.offset)),
            Some('×') => Some(Token::new(TokenType::Cross, "×".into(), loc.line, loc.col, loc.offset)),
            Some('∧') => Some(Token::new(TokenType::And, "∧".into(), loc.line, loc.col, loc.offset)),
            Some('∨') => Some(Token::new(TokenType::Or, "∨".into(), loc.line, loc.col, loc.offset)),
            Some('⊻') => Some(Token::new(TokenType::Xor, "⊻".into(), loc.line, loc.col, loc.offset)),
            Some('⊕') => Some(Token::new(TokenType::Tilde, "⊕".into(), loc.line, loc.col, loc.offset)),
            Some('¬') => Some(Token::new(TokenType::Not, "¬".into(), loc.line, loc.col, loc.offset)),
            Some('⊥') => Some(Token::new(TokenType::Identifier, "⊥".into(), loc.line, loc.col, loc.offset)),
            Some('⊤') => Some(Token::new(TokenType::Identifier, "⊤".into(), loc.line, loc.col, loc.offset)),
            Some('→') => Some(Token::new(TokenType::Arrow, "→".into(), loc.line, loc.col, loc.offset)),
            Some('⇒') => Some(Token::new(TokenType::FatArrow, "⇒".into(), loc.line, loc.col, loc.offset)),
            Some('←') => Some(Token::new(TokenType::LeftArrow, "←".into(), loc.line, loc.col, loc.offset)),
            Some('↦') => Some(Token::new(TokenType::Mapsto, "↦".into(), loc.line, loc.col, loc.offset)),
            Some('≠') => Some(Token::new(TokenType::Neq, "≠".into(), loc.line, loc.col, loc.offset)),
            Some('≤') => Some(Token::new(TokenType::Le, "≤".into(), loc.line, loc.col, loc.offset)),
            Some('≥') => Some(Token::new(TokenType::Ge, "≥".into(), loc.line, loc.col, loc.offset)),
            // Unicode identifiers (Greek/math letters)
            Some('α') | Some('β') | Some('γ') | Some('δ') | Some('ε')
                | Some('ζ') | Some('η') | Some('θ') | Some('ι') | Some('κ')
                | Some('λ') | Some('μ') | Some('ν') | Some('ξ') | Some('ο')
                | Some('π') | Some('ρ') | Some('σ') | Some('τ') | Some('υ')
                | Some('φ') | Some('χ') | Some('ψ') | Some('ω') => {
                let name = seq.to_string();
                Some(Token::new(TokenType::Identifier, name, loc.line, loc.col, loc.offset))
            }
            Some('𝔹') | Some('ℕ') | Some('ℤ') | Some('ℚ') | Some('ℝ') | Some('ℂ') => {
                let name = seq.to_string();
                Some(Token::new(TokenType::Identifier, name, loc.line, loc.col, loc.offset))
            }
            Some('Π') | Some('∏') => {
                Some(Token::new(TokenType::Pi, seq.into(), loc.line, loc.col, loc.offset))
            }
            _ => {
                // If it's a Unicode letter, treat as identifier
                if ch.map_or(false, |c| c.is_alphabetic()) {
                    Some(Token::new(TokenType::Identifier, seq.into(), loc.line, loc.col, loc.offset))
                } else {
                    None
                }
            }
        }
    }

    /// Read the next token from the source.
    pub fn next_token(&mut self) -> Token {
        // Return peeked token if available
        if let Some(tok) = self.peeked.take() {
            self.last_token = Some(tok.clone());
            return tok;
        }

        self.skip_whitespace_and_comments();

        if self.is_eof() {
            return Token::new(TokenType::Eof, String::new(), self.line, self.col, self.pos);
        }

        let loc = self.location();
        let ch = self.peek_byte().unwrap();

        // Annotation: @[...]
        if ch == b'@' && self.peek_byte_at(1) == Some(b'[') {
            self.advance(); // @
            self.advance(); // [
            let mut content = String::new();
            let mut depth = 1;
            while depth > 0 && !self.is_eof() {
                let c = self.advance().unwrap();
                match c {
                    b'[' => depth += 1,
                    b']' => depth -= 1,
                    _ => {}
                }
                if depth > 0 {
                    content.push(c as char);
                }
            }
            return Token::new(TokenType::Annotation, content, loc.line, loc.col, loc.offset);
        }

        // String literal
        if ch == b'"' || ch == b'r' && self.peek_byte_at(1) == Some(b'"') {
            return self.read_string();
        }

        // Character literal
        if ch == b'\'' {
            return self.read_char();
        }

        // Numeric literal
        if ch.is_ascii_digit() {
            // Check for . followed by digit (float starting with .)
            if ch == b'.' && self.peek_byte_at(1).map_or(false, |c| c.is_ascii_digit()) {
                return self.read_number();
            }
            return self.read_number();
        }

        // Dot: . can be float start or field access
        if ch == b'.' {
            if self.peek_byte_at(1).map_or(false, |c| c.is_ascii_digit()) {
                if self.pos > 0 && self.source[self.pos - 1].is_ascii_digit() {
                    // Part of a float started earlier, let read_number handle it
                }
                return self.read_number();
            }
            if self.peek_byte_at(1) == Some(b'.') && self.peek_byte_at(2) == Some(b'.') {
                // .. or ... (range operators - not yet fully supported)
                self.advance(); self.advance(); self.advance();
                return Token::new(TokenType::Dot, "...".into(), loc.line, loc.col, loc.offset);
            }
            if self.peek_byte_at(1) == Some(b'.') {
                self.advance(); self.advance();
                return Token::new(TokenType::Identifier, "..".into(), loc.line, loc.col, loc.offset);
            }
            self.advance();
            return Token::new(TokenType::Dot, ".".into(), loc.line, loc.col, loc.offset);
        }

        // Single-character tokens and multi-character operators
        match ch {
            // Unicode high bytes (0x80+)
            0x80..=0xFF => {
                if let Some(tok) = self.read_unicode_token(ch) {
                    return tok;
                }
                self.advance();
                return Token::new(TokenType::Error,
                    format!("unexpected unicode character at byte {}", ch),
                    loc.line, loc.col, loc.offset);
            }

            // Delimiters
            b'(' => { self.advance(); return Token::new(TokenType::LParen, "(".into(), loc.line, loc.col, loc.offset); }
            b')' => { self.advance(); return Token::new(TokenType::RParen, ")".into(), loc.line, loc.col, loc.offset); }
            b'{' => { self.advance(); return Token::new(TokenType::LBrace, "{".into(), loc.line, loc.col, loc.offset); }
            b'}' => { self.advance(); return Token::new(TokenType::RBrace, "}".into(), loc.line, loc.col, loc.offset); }
            b'[' => { self.advance(); return Token::new(TokenType::LBracket, "[".into(), loc.line, loc.col, loc.offset); }
            b']' => { self.advance(); return Token::new(TokenType::RBracket, "]".into(), loc.line, loc.col, loc.offset); }
            b';' => { self.advance(); return Token::new(TokenType::Semicolon, ";".into(), loc.line, loc.col, loc.offset); }
            b',' => { self.advance(); return Token::new(TokenType::Comma, ",".into(), loc.line, loc.col, loc.offset); }
            b'?' => { self.advance(); return Token::new(TokenType::Question, "?".into(), loc.line, loc.col, loc.offset); }
            b'@' => { self.advance(); return Token::new(TokenType::At, "@".into(), loc.line, loc.col, loc.offset); }

            // Multi-character operators
            b':' => {
                self.advance();
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    return Token::new(TokenType::Assign, ":=".into(), loc.line, loc.col, loc.offset);
                }
                return Token::new(TokenType::Colon, ":".into(), loc.line, loc.col, loc.offset);
            }
            b'=' => {
                self.advance();
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    return Token::new(TokenType::Eq, "==".into(), loc.line, loc.col, loc.offset);
                }
                if self.peek_byte() == Some(b'>') {
                    self.advance();
                    return Token::new(TokenType::FatArrow, "=>".into(), loc.line, loc.col, loc.offset);
                }
                return Token::new(TokenType::Assign, "=".into(), loc.line, loc.col, loc.offset);
            }
            b'!' => {
                self.advance();
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    return Token::new(TokenType::Neq, "!=".into(), loc.line, loc.col, loc.offset);
                }
                return Token::new(TokenType::Not, "!".into(), loc.line, loc.col, loc.offset);
            }
            b'<' => {
                self.advance();
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    return Token::new(TokenType::Le, "<=".into(), loc.line, loc.col, loc.offset);
                }
                if self.peek_byte() == Some(b'<') {
                    self.advance();
                    return Token::new(TokenType::Shl, "<<".into(), loc.line, loc.col, loc.offset);
                }
                if self.peek_byte() == Some(b'-') {
                    self.advance();
                    return Token::new(TokenType::LeftArrow, "<-".into(), loc.line, loc.col, loc.offset);
                }
                return Token::new(TokenType::Lt, "<".into(), loc.line, loc.col, loc.offset);
            }
            b'>' => {
                self.advance();
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    return Token::new(TokenType::Ge, ">=".into(), loc.line, loc.col, loc.offset);
                }
                if self.peek_byte() == Some(b'>') {
                    self.advance();
                    if self.peek_byte() == Some(b'>') {
                        self.advance();
                        return Token::new(TokenType::UShr, ">>>".into(), loc.line, loc.col, loc.offset);
                    }
                    return Token::new(TokenType::Shr, ">>".into(), loc.line, loc.col, loc.offset);
                }
                return Token::new(TokenType::Gt, ">".into(), loc.line, loc.col, loc.offset);
            }
            b'-' => {
                self.advance();
                if self.peek_byte() == Some(b'>') {
                    self.advance();
                    return Token::new(TokenType::Arrow, "->".into(), loc.line, loc.col, loc.offset);
                }
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    return Token::new(TokenType::MinusAssign, "-=".into(), loc.line, loc.col, loc.offset);
                }
                return Token::new(TokenType::Minus, "-".into(), loc.line, loc.col, loc.offset);
            }
            b'+' => {
                self.advance();
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    return Token::new(TokenType::PlusAssign, "+=".into(), loc.line, loc.col, loc.offset);
                }
                return Token::new(TokenType::Plus, "+".into(), loc.line, loc.col, loc.offset);
            }
            b'*' => {
                self.advance();
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    return Token::new(TokenType::MulAssign, "*=".into(), loc.line, loc.col, loc.offset);
                }
                return Token::new(TokenType::Mul, "*".into(), loc.line, loc.col, loc.offset);
            }
            b'/' => {
                self.advance();
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    return Token::new(TokenType::DivAssign, "/=".into(), loc.line, loc.col, loc.offset);
                }
                return Token::new(TokenType::Div, "/".into(), loc.line, loc.col, loc.offset);
            }
            b'%' => {
                self.advance();
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    return Token::new(TokenType::ModAssign, "%=".into(), loc.line, loc.col, loc.offset);
                }
                return Token::new(TokenType::Mod, "%".into(), loc.line, loc.col, loc.offset);
            }
            b'&' => {
                self.advance();
                if self.peek_byte() == Some(b'&') {
                    self.advance();
                    return Token::new(TokenType::And, "&&".into(), loc.line, loc.col, loc.offset);
                }
                return Token::new(TokenType::Amp, "&".into(), loc.line, loc.col, loc.offset);
            }
            b'|' => {
                self.advance();
                if self.peek_byte() == Some(b'|') {
                    self.advance();
                    return Token::new(TokenType::Or, "||".into(), loc.line, loc.col, loc.offset);
                }
                return Token::new(TokenType::Pipe, "|".into(), loc.line, loc.col, loc.offset);
            }
            b'^' => { self.advance(); return Token::new(TokenType::Power, "^".into(), loc.line, loc.col, loc.offset); }
            b'~' => {
                self.advance();
                if self.peek_byte() == Some(b'=') {
                    self.advance();
                    return Token::new(TokenType::Assign, "~=".into(), loc.line, loc.col, loc.offset);
                }
                return Token::new(TokenType::Tilde, "~".into(), loc.line, loc.col, loc.offset);
            }
            b'_' => { self.advance(); return Token::new(TokenType::Underscore, "_".into(), loc.line, loc.col, loc.offset); }

            // Letters (identifiers/keywords)
            b'a'..=b'z' | b'A'..=b'Z' => {
                return self.read_identifier();
            }

            _ => {
                self.advance();
                Token::new(TokenType::Error, format!("unexpected character '{}'", ch as char),
                          loc.line, loc.col, loc.offset)
            }
        }
    }

    /// Peek at the next token without consuming it.
    pub fn peek_token(&mut self) -> Token {
        if self.peeked.is_none() {
            self.peeked = Some(self.next_token());
        }
        self.peeked.clone().unwrap()
    }

    /// Get the previously produced token.
    pub fn last_token(&self) -> Option<&Token> {
        self.last_token.as_ref()
    }

    /// Collect all remaining tokens into a vector.
    pub fn tokenize_all(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token();
            if tok.ty == TokenType::Eof {
                break;
            }
            tokens.push(tok);
        }
        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let mut lexer = Lexer::new("def f(x: B): B { return x; }");
        let tokens = lexer.tokenize_all();
        let types: Vec<TokenType> = tokens.iter().map(|t| t.ty).collect();
        assert_eq!(types,
            vec![TokenType::KwDef, TokenType::Identifier, TokenType::LParen,
                 TokenType::Identifier, TokenType::Colon, TokenType::Identifier,
                 TokenType::RParen, TokenType::Colon, TokenType::Identifier,
                 TokenType::LBrace, TokenType::KwReturn, TokenType::Identifier,
                 TokenType::Semicolon, TokenType::RBrace]);
    }

    #[test]
    fn test_numbers() {
        let mut lexer = Lexer::new("42 3.14 0xFF 3e10");
        let tokens = lexer.tokenize_all();
        assert_eq!(tokens[0].ty, TokenType::IntLit);
        assert_eq!(tokens[1].ty, TokenType::FloatLit);
        assert_eq!(tokens[2].ty, TokenType::IntLit);
        assert_eq!(tokens[3].ty, TokenType::FloatLit);
    }

    #[test]
    fn test_arrows() {
        let mut lexer = Lexer::new("-> => <-");
        let tokens = lexer.tokenize_all();
        assert_eq!(tokens[0].ty, TokenType::Arrow);
        assert_eq!(tokens[1].ty, TokenType::FatArrow);
        assert_eq!(tokens[2].ty, TokenType::LeftArrow);
    }

    #[test]
    fn test_keywords() {
        let mut lexer = Lexer::new("if then else while for in def dat import");
        let tokens = lexer.tokenize_all();
        assert_eq!(tokens[0].ty, TokenType::KwIf);
        assert_eq!(tokens[3].ty, TokenType::KwWhile);
        assert_eq!(tokens[6].ty, TokenType::KwDef);
        assert_eq!(tokens[8].ty, TokenType::KwImport);
    }

    #[test]
    fn test_comments() {
        let mut lexer = Lexer::new("x // this is a comment\ny");
        let tokens = lexer.tokenize_all();
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].text, "x");
        assert_eq!(tokens[1].text, "y");
    }
}
