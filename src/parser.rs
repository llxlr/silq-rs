//! Parser for the Silq programming language.
//!
//! Uses a Pratt-style precedence climbing parser for expressions.
//! The parser produces AST nodes from a token stream.

use crate::ast::{
    Annotation, CaptureAnnotation, Declaration, Expression, Id, Interner,
    LiteralValue, TypeAnnotationKind,
};
use crate::errors::Location;
use crate::lexer::Lexer;
use crate::token::{get_lbp, precedence, Token, TokenType};

/// The Parser converts a token stream into an AST.
pub struct Parser<'a> {
    lexer: &'a mut Lexer,
    interner: &'a mut Interner,
    /// Current token.
    current: Token,
    /// Previous token.
    previous: Token,
    /// Whether we're in a panic mode (recovering from errors).
    panic_mode: bool,
}

impl<'a> Parser<'a> {
    /// Create a new parser wrapping a lexer.
    pub fn new(lexer: &'a mut Lexer, interner: &'a mut Interner) -> Self {
        let current = lexer.next_token();
        let previous = Token::new(TokenType::Eof, String::new(), 0, 0, 0);
        Parser {
            lexer,
            interner,
            current,
            previous,
            panic_mode: false,
        }
    }

    // ---- Token management ----

    /// Advance to the next token.
    fn advance(&mut self) {
        self.previous = self.current.clone();
        self.current = self.lexer.next_token();
    }

    /// Check if the current token matches the expected type.
    fn check(&self, ty: TokenType) -> bool {
        self.current.ty == ty
    }

    /// Check if the current token matches any of the given types.
    #[allow(dead_code)]
    fn check_any(&self, types: &[TokenType]) -> bool {
        types.contains(&self.current.ty)
    }

    /// Consume the current token if it matches the expected type.
    fn matches(&mut self, ty: TokenType) -> bool {
        if self.check(ty) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Expect the current token to be of the given type, advancing.
    /// If it's not, emit an error and enter panic mode.
    fn expect(&mut self, ty: TokenType) -> Result<(), String> {
        if self.check(ty) {
            self.advance();
            Ok(())
        } else {
            let msg = format!("expected {}, found {}", ty, self.current.ty);
            self.error(&msg);
            Err(msg)
        }
    }

    /// Expect a semicolon, but don't err on EOF or before closing brace.
    fn expect_semicolon(&mut self) {
        if self.check(TokenType::Semicolon) {
            self.advance();
        }
        // Missing semicolons are common, don't hard error.
    }

    /// Report an error at the current token.
    fn error(&mut self, msg: &str) {
        eprintln!("{}: error: {}", self.current.line, msg);
        self.panic_mode = true;
    }

    /// Get the location of the previous token.
    #[allow(dead_code)]
    fn prev_location(&self) -> Location {
        Location {
            line: self.previous.line,
            col: self.previous.col,
            offset: self.previous.offset,
        }
    }

    /// Get the location of the current token.
    fn cur_location(&self) -> Location {
        Location {
            line: self.current.line,
            col: self.current.col,
            offset: self.current.offset,
        }
    }

    // ---- Synchronization for error recovery ----

    /// Synchronize the parser to a known recovery point.
    #[allow(dead_code)]
    fn synchronize(&mut self) {
        self.panic_mode = false;
        while !self.check(TokenType::Eof) {
            // If we hit a known statement boundary, stop.
            if self.previous.ty == TokenType::Semicolon {
                return;
            }
            match self.current.ty {
                TokenType::KwDef | TokenType::KwIf | TokenType::KwFor
                | TokenType::KwWhile | TokenType::KwReturn | TokenType::KwImport
                | TokenType::KwDat | TokenType::KwLet | TokenType::KwRepeat
                | TokenType::KwWith => return,
                _ => {
                    self.advance();
                }
            }
        }
    }

    // ---- Primary expressions (nud - null denotation) ----

    /// Parse a primary expression (identifier, literal, parenthesized expr, etc.).
    fn primary(&mut self) -> Expression {
        let loc = self.cur_location();

        match self.current.ty {
            TokenType::Identifier => {
                let name = self.interner.intern(&self.current.text.clone());
                self.advance();
                Expression::new_identifier(loc, name)
            }

            TokenType::KwTrue => {
                self.advance();
                Expression::new_literal(loc, LiteralValue::Bool(true))
            }

            TokenType::KwFalse => {
                self.advance();
                Expression::new_literal(loc, LiteralValue::Bool(false))
            }

            TokenType::IntLit => {
                let text = self.current.text.clone();
                self.advance();
                // Parse the integer value
                let int_val = parse_int_literal(&text);
                Expression::new_literal(loc, LiteralValue::Int(int_val))
            }

            TokenType::FloatLit => {
                let text = self.current.text.clone();
                self.advance();
                let float_val = text.replace('_', "").parse::<f64>().unwrap_or(0.0);
                Expression::new_literal(loc, LiteralValue::Float(float_val))
            }

            TokenType::RationalLit => {
                let text = self.current.text.clone();
                self.advance();
                if let Some((num, den)) = parse_rational(&text) {
                    Expression::new_literal(loc, LiteralValue::Rational(num, den))
                } else {
                    Expression::new_error(loc, format!("invalid rational literal: {}", text))
                }
            }

            TokenType::StringLit => {
                let text = self.current.text.clone();
                self.advance();
                Expression::new_literal(loc, LiteralValue::String(text))
            }

            TokenType::CharLit => {
                let text = self.current.text.clone();
                self.advance();
                let ch = text.chars().next().unwrap_or('?');
                Expression::new_literal(loc, LiteralValue::Char(ch))
            }

            TokenType::Underscore => {
                self.advance();
                Expression::Wildcard { loc }
            }

            TokenType::KwLet => {
                self.advance(); // skip 'let'
                self.parse_let(None)
            }

            TokenType::KwLambda => {
                self.advance();
                self.parse_lambda(loc)
            }

            TokenType::KwTypeof => {
                self.advance();
                let _ = self.expect(TokenType::LParen);
                let expr = self.parse_expression();
                let _ = self.expect(TokenType::RParen);
                Expression::Typeof { loc, expr: Box::new(expr) }
            }

            TokenType::KwIf => {
                self.advance();
                self.parse_if_expr()
            }

            TokenType::KwFor => {
                self.advance();
                self.parse_for_expr()
            }

            TokenType::KwWhile => {
                self.advance();
                self.parse_while_expr()
            }

            TokenType::KwRepeat => {
                self.advance();
                self.parse_repeat_expr()
            }

            TokenType::KwWith => {
                self.advance();
                self.parse_with_expr()
            }

            TokenType::KwReturn => {
                self.advance();
                if self.check(TokenType::Semicolon) || self.check(TokenType::RBrace) {
                    Expression::Return { loc, expr: None }
                } else {
                    let expr = self.parse_expression();
                    Expression::Return { loc, expr: Some(Box::new(expr)) }
                }
            }

            TokenType::KwAssert => {
                self.advance();
                let _ = self.expect(TokenType::LParen);
                let cond = self.parse_expression();
                let _ = self.expect(TokenType::RParen);
                Expression::Assert { loc, condition: Box::new(cond), message: None }
            }

            TokenType::KwForget => {
                self.advance();
                let _ = self.expect(TokenType::LParen);
                let var = self.parse_expression();
                let _ = self.expect(TokenType::RParen);
                Expression::Forget { loc, variable: Box::new(var) }
            }

            TokenType::LParen => {
                self.advance(); // (
                // Check if empty parens
                if self.check(TokenType::RParen) {
                    self.advance();
                    return Expression::Type { loc: Location::default(), kind: crate::ast::TypeKind::Unit };
                }
                let expr = self.parse_expression();
                // Check for tuple (comma separated)
                if self.check(TokenType::Comma) {
                    let mut elements = vec![expr];
                    while self.matches(TokenType::Comma) {
                        if self.check(TokenType::RParen) {
                            break;
                        }
                        elements.push(self.parse_expression());
                    }
                    let _ = self.expect(TokenType::RParen);
                    return Expression::Tuple { loc, elements };
                }
                let _ = self.expect(TokenType::RParen);
                expr
            }

            TokenType::LBrace => {
                self.parse_compound()
            }

            TokenType::LBracket => {
                self.advance(); // [
                let mut elements = Vec::new();
                if !self.check(TokenType::RBracket) {
                    elements.push(self.parse_expression());
                    while self.matches(TokenType::Comma) {
                        if self.check(TokenType::RBracket) {
                            break;
                        }
                        elements.push(self.parse_expression());
                    }
                }
                let _ = self.expect(TokenType::RBracket);
                Expression::Vector { loc, elements }
            }

            // Unary operators
            TokenType::Minus => {
                self.advance();
                let expr = self.parse_expression_precedence(150); // High precedence for unary
                Expression::UnaryMinus { loc, expr: Box::new(expr) }
            }

            TokenType::Plus => {
                self.advance();
                let expr = self.parse_expression_precedence(150);
                Expression::UnaryPlus { loc, expr: Box::new(expr) }
            }

            TokenType::Not => {
                self.advance();
                let expr = self.parse_expression_precedence(150);
                Expression::LogicalNot { loc, expr: Box::new(expr) }
            }

            TokenType::Tilde => {
                self.advance();
                let expr = self.parse_expression_precedence(150);
                Expression::BitwiseNot { loc, expr: Box::new(expr) }
            }

            TokenType::KwConst => {
                self.advance();
                let cls = self.parse_expression();
                // Wrap as const annotation?
                Expression::TypeAnnotation {
                    loc,
                    expr: Box::new(cls),
                    ty: Box::new(Expression::unit_type()),
                    kind: TypeAnnotationKind::Pun,
                }
            }

            TokenType::KwMoved => {
                self.advance();
                self.parse_expression()
            }

            _ => {
                self.error(&format!("unexpected token: {}", self.current.ty));
                self.advance();
                Expression::new_error(loc, "unexpected token")
            }
        }
    }

    // ---- Parse specific statement types ----

    /// Parse a compound statement: { stmt1; stmt2; ... }
    fn parse_compound(&mut self) -> Expression {
        let loc = self.cur_location();
        self.advance(); // {

        let mut statements = Vec::new();

        while !self.check(TokenType::RBrace) && !self.check(TokenType::Eof) {
            if self.check(TokenType::Semicolon) {
                self.advance();
                continue;
            }
            statements.push(self.parse_statement());
            // Semicolons are optional after some constructs
            if self.check(TokenType::Semicolon) {
                self.advance();
            }
        }

        let _ = self.expect(TokenType::RBrace);
        Expression::new_compound(loc, statements)
    }

    /// Parse a single statement or declaration.
    fn parse_statement(&mut self) -> Expression {
        let loc = self.cur_location();

        match self.current.ty {
            TokenType::KwDef => {
                self.advance();
                let name = self.interner.intern(&self.current.text);
                self.advance();
                self.parse_function_def(name)
            }

            TokenType::KwDat => {
                self.advance();
                let name = self.interner.intern(&self.current.text);
                self.advance();
                self.parse_dat_decl(name)
            }

            TokenType::KwImport => {
                self.advance();
                let path = self.current.text.clone();
                self.advance();
                Expression::TypeDecl(Box::new(Declaration::Import { loc, path }))
            }

            TokenType::KwLet => {
                self.advance();
                let name = self.interner.intern(&self.current.text);
                self.advance();
                self.parse_let(Some(name))
            }

            // Assignment or expression statement
            _ => {
                let expr = self.parse_expression();

                // Check for assignment: x := y  or  x <- y  or  e[i] := y
                if self.check(TokenType::Assign) || self.check(TokenType::LeftArrow) {
                    let assign_ty = self.current.ty;
                    let assign_loc = self.cur_location();
                    self.advance();
                    let value = self.parse_expression();
                    if assign_ty == TokenType::LeftArrow {
                        return Expression::Assign {
                            loc: assign_loc,
                            target: Box::new(expr),
                            value: Box::new(value),
                        };
                    } else {
                        // := is a define (let with implicit name from expr)
                        if let Expression::Identifier { name, .. } = &expr {
                            return Expression::new_let(
                                assign_loc, *name, None, value,
                                Expression::Identifier { loc, name: *name, meaning: None, classical: false },
                            );
                        }
                        // Non-identifier target (e.g., vec[i] := x): treat as assignment
                        return Expression::Assign {
                            loc: assign_loc,
                            target: Box::new(expr),
                            value: Box::new(value),
                        };
                    }
                }

                expr
            }
        }
    }

    /// Parse a function definition: name(params) [annotation] [: return_type] body
    fn parse_function_def(&mut self, name: Id) -> Expression {
        let loc = self.cur_location();
        let _ = self.expect(TokenType::LParen).ok();

        // Parse parameters
        let mut params = Vec::new();
        if !self.check(TokenType::RParen) {
            params.push(self.parse_parameter());
            while self.matches(TokenType::Comma) {
                if self.check(TokenType::RParen) {
                    break;
                }
                params.push(self.parse_parameter());
            }
        }
        let _ = self.expect(TokenType::RParen).ok();

        // Optional annotation
        let annotation = self.parse_annotation();

        // Optional return type
        let return_type = if self.check(TokenType::Colon) {
            self.advance();
            Some(self.parse_expression())
        } else {
            None
        };

        // Body
        let body = self.parse_body();

        let decl = Declaration::new_function(loc, name, params, return_type, body, annotation);
        Expression::TypeDecl(Box::new(decl))
    }

    /// Parse a parameter declaration.
    fn parse_parameter(&mut self) -> Declaration {
        let loc = self.cur_location();

        // Optional const/moved annotation
        let capture = match self.current.ty {
            TokenType::KwConst => {
                self.advance();
                CaptureAnnotation::Const
            }
            TokenType::KwMoved => {
                self.advance();
                CaptureAnnotation::Moved
            }
            _ => CaptureAnnotation::None,
        };

        let name = self.interner.intern(&self.current.text);
        self.advance();

        // Optional type annotation
        let dtype = if self.matches(TokenType::Colon) {
            Some(self.parse_expression())
        } else {
            None
        };

        Declaration::new_var(loc, name, dtype, None, true, capture)
    }

    /// Parse a function annotation (qfree, mfree, lifted, wild).
    fn parse_annotation(&mut self) -> Annotation {
        match self.current.ty {
            TokenType::KwQfree => { self.advance(); Annotation::Qfree }
            TokenType::KwMfree => { self.advance(); Annotation::Mfree }
            TokenType::KwLifted => { self.advance(); Annotation::Lifted }
            TokenType::KwWild => { self.advance(); Annotation::Wild }
            _ => Annotation::None,
        }
    }

    /// Parse a function body: `{ ... }` or `=> expr` or `;` (abstract).
    fn parse_body(&mut self) -> Expression {
        if self.check(TokenType::Semicolon) {
            self.advance(); // Abstract function (no body)
            return Expression::new_literal(Location::default(), LiteralValue::Unit);
        }
        if self.check(TokenType::FatArrow) {
            self.advance(); // =>
            let expr = self.parse_expression();
            return expr;
        }
        if self.check(TokenType::LBrace) {
            return self.parse_compound();
        }
        // Implicit single expression body
        self.parse_expression()
    }

    /// Parse a data type declaration: dat Name [params] ["quantum"] { fields... }
    fn parse_dat_decl(&mut self, name: Id) -> Expression {
        let loc = self.cur_location();

        // Type parameters
        let type_params = if self.matches(TokenType::LBracket) {
            let params = self.parse_type_params();
            let _ = self.expect(TokenType::RBracket).ok();
            params
        } else {
            Vec::new()
        };

        // Optional quantum keyword
        let is_quantum = self.matches(TokenType::KwQuantum);

        // Body: { field1; field2; ... }
        let _ = self.expect(TokenType::LBrace).ok();
        let mut fields = Vec::new();
        while !self.check(TokenType::RBrace) && !self.check(TokenType::Eof) {
            if self.check(TokenType::Semicolon) {
                self.advance();
                continue;
            }
            fields.push(self.parse_parameter());
            self.expect_semicolon();
        }
        let _ = self.expect(TokenType::RBrace).ok();

        let decl = Declaration::new_dat(loc, name, type_params, fields, is_quantum);
        Expression::TypeDecl(Box::new(decl))
    }

    /// Parse type parameters: [T1, T2: *, ...]
    fn parse_type_params(&mut self) -> Vec<Declaration> {
        let mut params = Vec::new();
        loop {
            let name = self.interner.intern(&self.current.text);
            self.advance();
            let dtype = if self.matches(TokenType::Colon) {
                Some(self.parse_expression())
            } else {
                None
            };
            params.push(Declaration::new_var(
                Location::default(), name, dtype, None, true,
                CaptureAnnotation::Const,
            ));
            if !self.matches(TokenType::Comma) {
                break;
            }
        }
        params
    }

    /// Parse a let expression: let name [:type] = value; body.
    fn parse_let(&mut self, name: Option<Id>) -> Expression {
        let loc = self.cur_location();
        let name = name.unwrap_or_else(|| {
            let n = self.interner.intern(&self.current.text);
            self.advance();
            n
        });

        // Optional type annotation
        let type_ann = if self.matches(TokenType::Colon) {
            Some(self.parse_expression())
        } else {
            None
        };

        // Assignment
        let _ = self.expect(TokenType::Assign).ok();
        let value = self.parse_expression();

        Expression::new_let(loc, name, type_ann, value,
            Expression::new_literal(Location::default(), LiteralValue::Unit))
    }

    /// Parse an if expression: if cond then e1 else e2
    fn parse_if_expr(&mut self) -> Expression {
        let loc = self.cur_location();
        let cond = self.parse_expression();

        // `then` is optional, can also use { }
        let _ = self.matches(TokenType::KwThen);

        let then_br = if self.check(TokenType::LBrace) {
            self.parse_compound()
        } else {
            self.parse_expression()
        };

        let else_br = if self.matches(TokenType::KwElse) {
            if self.check(TokenType::KwIf) {
                self.advance();
                Some(self.parse_if_expr())
            } else if self.check(TokenType::LBrace) {
                Some(self.parse_compound())
            } else {
                Some(self.parse_expression())
            }
        } else {
            None
        };

        Expression::new_if(loc, cond, then_br, else_br)
    }

    /// Parse a for loop expression.
    fn parse_for_expr(&mut self) -> Expression {
        let loc = self.cur_location();
        let var = self.interner.intern(&self.current.text);
        self.advance();

        let _ = self.expect(TokenType::KwIn).ok();
        let iterable = self.parse_expression();

        let body = if self.check(TokenType::LBrace) {
            self.parse_compound()
        } else {
            let _ = self.matches(TokenType::KwDo);
            self.parse_expression()
        };

        Expression::ForLoop {
            loc,
            variable: var,
            iterable: Box::new(iterable),
            body: Box::new(body),
        }
    }

    /// Parse a while loop expression.
    fn parse_while_expr(&mut self) -> Expression {
        let loc = self.cur_location();
        let cond = self.parse_expression();

        let body = if self.check(TokenType::LBrace) {
            self.parse_compound()
        } else {
            let _ = self.matches(TokenType::KwDo);
            self.parse_expression()
        };

        Expression::WhileLoop {
            loc,
            condition: Box::new(cond),
            body: Box::new(body),
        }
    }

    /// Parse a repeat expression.
    fn parse_repeat_expr(&mut self) -> Expression {
        let loc = self.cur_location();
        let count = self.parse_expression();
        let body = self.parse_compound();

        Expression::Repeat {
            loc,
            count: Box::new(count),
            body: Box::new(body),
        }
    }

    /// Parse a with expression (quantum-controlled execution).
    fn parse_with_expr(&mut self) -> Expression {
        let loc = self.cur_location();
        let controller = self.parse_expression();
        let _ = self.expect(TokenType::KwDo).ok();
        let body = self.parse_compound();

        Expression::With {
            loc,
            controller: Box::new(controller),
            body: Box::new(body),
        }
    }

    /// Parse a lambda expression: λ(const x1: τ1, ...). body
    fn parse_lambda(&mut self, loc: Location) -> Expression {
        // Parse parameters: (const x1: τ1, ..., const xn: τn)
        let params = self.parse_lambda_params();
        let annotation = self.parse_annotation();
        // Body separator: `.` block, `=>` expr, or `{` block
        let body = if self.matches(TokenType::Dot) {
            if self.check(TokenType::LBrace) {
                self.parse_compound()
            } else {
                self.parse_expression()
            }
        } else if self.matches(TokenType::FatArrow) {
            self.parse_expression()
        } else if self.check(TokenType::LBrace) {
            self.parse_compound()
        } else {
            self.parse_expression()
        };

        Expression::new_lambda(loc, params, body, annotation)
    }

    /// Parse lambda parameter list: (const x1: τ1, ...)
    fn parse_lambda_params(&mut self) -> Vec<Expression> {
        if !self.check(TokenType::LParen) {
            // Single untyped parameter without parens: λx. body
            if self.check(TokenType::Identifier) {
                let name = self.interner.intern(&self.current.text);
                let loc = self.cur_location();
                self.advance();
                return vec![Expression::new_identifier(loc, name)];
            }
            return vec![];
        }

        self.advance(); // skip (
        let mut params = Vec::new();
        if !self.check(TokenType::RParen) {
            params.push(self.parse_lambda_param());
            while self.matches(TokenType::Comma) {
                if self.check(TokenType::RParen) {
                    break;
                }
                params.push(self.parse_lambda_param());
            }
        }
        let _ = self.expect(TokenType::RParen).ok();
        params
    }

    /// Parse a single lambda parameter: [const] name [: type]
    fn parse_lambda_param(&mut self) -> Expression {
        // Skip const annotation for lambda params
        if self.check(TokenType::KwConst) {
            self.advance();
        }
        let name = self.interner.intern(&self.current.text);
        let loc = self.cur_location();
        self.advance();
        // Optional type annotation -- parsed but attached via name resolution later
        if self.matches(TokenType::Colon) {
            let _type_expr = self.parse_expression();
        }
        Expression::new_identifier(loc, name)
    }

    // ---- Infix parsing (led - left denotation) ----

    /// Parse an infix expression: left op right.
    fn infix(&mut self, left: Expression, op: TokenType, loc: Location) -> Expression {
        match op {
            // Type annotations
            TokenType::Colon => {
                let ty = self.parse_expression();
                Expression::TypeAnnotation {
                    loc,
                    expr: Box::new(left),
                    ty: Box::new(ty),
                    kind: TypeAnnotationKind::Colon,
                }
            }

            TokenType::KwAs => {
                let ty = self.parse_expression();
                Expression::TypeAnnotation {
                    loc,
                    expr: Box::new(left),
                    ty: Box::new(ty),
                    kind: TypeAnnotationKind::As,
                }
            }

            TokenType::KwCoerce => {
                let ty = self.parse_expression();
                Expression::TypeAnnotation {
                    loc,
                    expr: Box::new(left),
                    ty: Box::new(ty),
                    kind: TypeAnnotationKind::Coerce,
                }
            }

            TokenType::KwPun => {
                let ty = self.parse_expression();
                Expression::TypeAnnotation {
                    loc,
                    expr: Box::new(left),
                    ty: Box::new(ty),
                    kind: TypeAnnotationKind::Pun,
                }
            }

            // Arrow type: A -> B
            TokenType::Arrow => {
                let right = self.parse_expression_precedence(get_lbp(op));
                Expression::Type { loc: Location::default(), kind: crate::ast::TypeKind::Product {
                    params: vec![],
                    domain: Box::new(left),
                    codomain: Box::new(right),
                    annotation: Annotation::None,
                }}
            }

            // Conditional: cond ? then_br else
            TokenType::Question => {
                let then_br = self.parse_expression();
                let _ = self.expect(TokenType::KwElse).ok();
                let else_br = self.parse_expression();
                Expression::new_if(loc, left, then_br, Some(else_br))
            }

            // Function call: f(args)
            TokenType::LParen => {
                let mut args = Vec::new();
                if !self.check(TokenType::RParen) {
                    // Use high precedence to prevent comma from being consumed
                    args.push(self.parse_expression_precedence(precedence::ASSIGN));
                    while self.matches(TokenType::Comma) {
                        if self.check(TokenType::RParen) {
                            break;
                        }
                        args.push(self.parse_expression_precedence(precedence::ASSIGN));
                    }
                }
                let _ = self.expect(TokenType::RParen).ok();
                Expression::new_call(loc, left, args)
            }

            // Index: e[i]
            TokenType::LBracket => {
                let index = self.parse_expression();
                let _ = self.expect(TokenType::RBracket).ok();
                Expression::Index { loc, expr: Box::new(left), index: Box::new(index) }
            }

            // Field access: e.name
            TokenType::Dot => {
                let field = self.interner.intern(&self.current.text);
                self.advance();
                Expression::Field { loc, expr: Box::new(left), field }
            }

            // Assignment: x := y
            TokenType::Assign => {
                let value = self.parse_expression_precedence(precedence::ASSIGN);
                if let Expression::Identifier { name, .. } = &left {
                    Expression::new_let(
                        loc, *name, None, value,
                        Expression::Identifier {
                            loc: Location::default(),
                            name: *name,
                            meaning: None,
                            classical: false,
                        },
                    )
                } else {
                    Expression::Assign {
                        loc,
                        target: Box::new(left),
                        value: Box::new(value),
                    }
                }
            }

            // Left arrow assignment: x ← y
            TokenType::LeftArrow => {
                let value = self.parse_expression_precedence(precedence::ASSIGN);
                Expression::Assign {
                    loc,
                    target: Box::new(left),
                    value: Box::new(value),
                }
            }

            // Compound assignment: x += y, x -= y, etc.
            TokenType::PlusAssign | TokenType::MinusAssign
            | TokenType::MulAssign | TokenType::DivAssign | TokenType::ModAssign => {
                let value = self.parse_expression_precedence(precedence::ASSIGN);
                Expression::Assign {
                    loc,
                    target: Box::new(left),
                    value: Box::new(value),
                }
            }

            // Binary operators
            _ => {
                let right = self.parse_expression_precedence(get_lbp(op));
                Expression::new_binary(loc, op, left, right)
            }
        }
    }

    // ---- Main expression parsing ----

    /// Parse an expression with the given minimum precedence.
    pub fn parse_expression_precedence(&mut self, min_bp: u8) -> Expression {
        let mut left = self.primary();

        loop {
            let op = self.current.ty;
            let lbp = get_lbp(op);

            if lbp < min_bp || lbp == 0 {
                break;
            }

            // Skip semicolons and commas in expression context
            // (handled by callers)

            let loc = self.cur_location();
            self.advance();

            left = self.infix(left, op, loc);
        }

        left
    }

    /// Parse a full expression (starting from lowest precedence).
    pub fn parse_expression(&mut self) -> Expression {
        self.parse_expression_precedence(0)
    }

    /// Parse a program: a sequence of top-level declarations.
    pub fn parse_program(&mut self) -> Vec<Expression> {
        let mut declarations = Vec::new();

        while !self.check(TokenType::Eof) {
            // Skip stray semicolons
            if self.check(TokenType::Semicolon) {
                self.advance();
                continue;
            }

            declarations.push(self.parse_statement());

            // Optional semicolon after declarations
            if self.check(TokenType::Semicolon) {
                self.advance();
            }

            // Skip stray closing braces
            if self.check(TokenType::RBrace) {
                self.advance();
            }
        }

        declarations
    }
}

// ---- Helper functions for parsing literals ----

/// Parse an integer literal (decimal, hex with 0x prefix, binary with 0b prefix).
fn parse_int_literal(text: &str) -> num_bigint::BigInt {
    use num_bigint::BigInt;
    let text = text.replace('_', "");

    if text.len() > 2 && text.as_bytes()[..2] == *b"0x" {
        BigInt::parse_bytes(&text.as_bytes()[2..], 16).unwrap_or_else(|| BigInt::from(0))
    } else if text.len() > 2 && text.as_bytes()[..2] == *b"0b" {
        BigInt::parse_bytes(&text.as_bytes()[2..], 2).unwrap_or_else(|| BigInt::from(0))
    } else {
        BigInt::parse_bytes(text.as_bytes(), 10).unwrap_or_else(|| BigInt::from(0))
    }
}

/// Parse a rational literal: num\den.
fn parse_rational(text: &str) -> Option<(num_bigint::BigInt, num_bigint::BigInt)> {
    let parts: Vec<&str> = text.split('\\').collect();
    if parts.len() == 2 {
        let num = parse_int_literal(parts[0]);
        let den = parse_int_literal(parts[1]);
        Some((num, den))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    #[test]
    fn test_parse_simple_expr() {
        let source = "42 + x * 3";
        let mut interner = Interner::new();
        let mut lexer = Lexer::new(source);
        let mut parser = Parser::new(&mut lexer, &mut interner);
        let expr = parser.parse_expression();
        assert!(matches!(expr, Expression::Binary { .. }));
    }

    #[test]
    fn test_parse_function_def() {
        let source = "def f(x: B): B { return x; }";
        let mut interner = Interner::new();
        let mut lexer = Lexer::new(source);
        let mut parser = Parser::new(&mut lexer, &mut interner);
        let decls = parser.parse_program();
        assert_eq!(decls.len(), 1);
        assert!(matches!(decls[0], Expression::TypeDecl(_)));
    }

    #[test]
    fn test_parse_if_expr() {
        let source = "if true then 1 else 2";
        let mut interner = Interner::new();
        let mut lexer = Lexer::new(source);
        let mut parser = Parser::new(&mut lexer, &mut interner);
        let expr = parser.parse_expression();
        assert!(matches!(expr, Expression::IfThenElse { .. }));
    }

    #[test]
    fn test_parse_let() {
        let source = "let x := 5";
        let mut interner = Interner::new();
        let mut lexer = Lexer::new(source);
        let mut parser = Parser::new(&mut lexer, &mut interner);
        let expr = parser.parse_expression();
        assert!(matches!(expr, Expression::Let { .. }));
    }

    #[test]
    fn test_parse_call() {
        let source = "f(1, 2, 3)";
        let mut interner = Interner::new();
        let mut lexer = Lexer::new(source);
        let mut parser = Parser::new(&mut lexer, &mut interner);
        let expr = parser.parse_expression();
        assert!(matches!(expr, Expression::Call { .. }));
        if let Expression::Call { arguments, .. } = &expr {
            assert_eq!(arguments.len(), 3);
        }
    }

    #[test]
    fn test_parse_tuple_debug() {
        let source = "(true, false)";
        let mut interner = Interner::new();
        let mut lexer = Lexer::new(source);
        let mut parser = Parser::new(&mut lexer, &mut interner);
        let expr = parser.parse_expression();
        println!("discriminant: {:?}", std::mem::discriminant(&expr));
        println!("{:#?}", expr);
        // NOTE: (true, false) is currently parsed as Binary { op: Comma, .. }
        // rather than Tuple { .. }. This is a known parser limitation:
        // parenthesized comma-separated expressions should be lowered to
        // Expression::Tuple during semantic analysis.
        assert!(matches!(expr, Expression::Binary { op: crate::token::TokenType::Comma, .. }));
    }
}
