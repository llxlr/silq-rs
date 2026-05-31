//! Constant expression evaluation for Silq.
//!
//! Evaluates numeric and boolean expressions at compile time.
//! Used for type-level computations and constant folding.

use crate::ast::{Expression, LiteralValue};
use num_traits::Zero;

/// Evaluate a literal expression, returning the literal value.
pub fn eval_literal(expr: &Expression) -> Option<&LiteralValue> {
    match expr {
        Expression::Literal { value, .. } => Some(value),
        _ => None,
    }
}

/// Evaluate a binary operation on literal values.
pub fn eval_binary_literal(left: &LiteralValue, right: &LiteralValue,
                           op: crate::token::TokenType) -> Option<LiteralValue> {
    use crate::token::TokenType;

    match op {
        TokenType::Plus => {
            match (left, right) {
                (LiteralValue::Int(a), LiteralValue::Int(b)) => Some(LiteralValue::Int(a + b)),
                (LiteralValue::Float(a), LiteralValue::Float(b)) => Some(LiteralValue::Float(a + b)),
                _ => None,
            }
        }
        TokenType::Minus => {
            match (left, right) {
                (LiteralValue::Int(a), LiteralValue::Int(b)) => Some(LiteralValue::Int(a - b)),
                (LiteralValue::Float(a), LiteralValue::Float(b)) => Some(LiteralValue::Float(a - b)),
                _ => None,
            }
        }
        TokenType::Mul => {
            match (left, right) {
                (LiteralValue::Int(a), LiteralValue::Int(b)) => Some(LiteralValue::Int(a * b)),
                (LiteralValue::Float(a), LiteralValue::Float(b)) => Some(LiteralValue::Float(a * b)),
                _ => None,
            }
        }
        TokenType::Div => {
            match (left, right) {
                (LiteralValue::Int(a), LiteralValue::Int(b)) => {
                    if b.is_zero() { return None; }
                    Some(LiteralValue::Int(a / b))
                }
                (LiteralValue::Float(a), LiteralValue::Float(b)) => {
                    if *b == 0.0 { return None; }
                    Some(LiteralValue::Float(a / b))
                }
                _ => None,
            }
        }
        TokenType::Eq => {
            match (left, right) {
                (LiteralValue::Int(a), LiteralValue::Int(b)) => Some(LiteralValue::Bool(a == b)),
                (LiteralValue::Bool(a), LiteralValue::Bool(b)) => Some(LiteralValue::Bool(a == b)),
                (LiteralValue::Float(a), LiteralValue::Float(b)) => Some(LiteralValue::Bool(a == b)),
                _ => None,
            }
        }
        TokenType::Neq => {
            match (left, right) {
                (LiteralValue::Int(a), LiteralValue::Int(b)) => Some(LiteralValue::Bool(a != b)),
                (LiteralValue::Bool(a), LiteralValue::Bool(b)) => Some(LiteralValue::Bool(a != b)),
                (LiteralValue::Float(a), LiteralValue::Float(b)) => Some(LiteralValue::Bool(a != b)),
                _ => None,
            }
        }
        TokenType::Lt => {
            match (left, right) {
                (LiteralValue::Int(a), LiteralValue::Int(b)) => Some(LiteralValue::Bool(a < b)),
                (LiteralValue::Float(a), LiteralValue::Float(b)) => Some(LiteralValue::Bool(a < b)),
                _ => None,
            }
        }
        TokenType::Gt => {
            match (left, right) {
                (LiteralValue::Int(a), LiteralValue::Int(b)) => Some(LiteralValue::Bool(a > b)),
                (LiteralValue::Float(a), LiteralValue::Float(b)) => Some(LiteralValue::Bool(a > b)),
                _ => None,
            }
        }
        TokenType::Le => {
            match (left, right) {
                (LiteralValue::Int(a), LiteralValue::Int(b)) => Some(LiteralValue::Bool(a <= b)),
                (LiteralValue::Float(a), LiteralValue::Float(b)) => Some(LiteralValue::Bool(a <= b)),
                _ => None,
            }
        }
        TokenType::Ge => {
            match (left, right) {
                (LiteralValue::Int(a), LiteralValue::Int(b)) => Some(LiteralValue::Bool(a >= b)),
                (LiteralValue::Float(a), LiteralValue::Float(b)) => Some(LiteralValue::Bool(a >= b)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Attempt to evaluate an expression to a constant.
pub fn const_eval(expr: &Expression) -> Option<LiteralValue> {
    match expr {
        Expression::Literal { value, .. } => Some(value.clone()),
        Expression::Binary { op, left, right, .. } => {
            let lval = const_eval(left)?;
            let rval = const_eval(right)?;
            eval_binary_literal(&lval, &rval, *op)
        }
        Expression::UnaryMinus { expr, .. } => {
            let val = const_eval(expr)?;
            match val {
                LiteralValue::Int(n) => Some(LiteralValue::Int(-n)),
                LiteralValue::Float(f) => Some(LiteralValue::Float(-f)),
                _ => None,
            }
        }
        Expression::LogicalNot { expr, .. } => {
            let val = const_eval(expr)?;
            match val {
                LiteralValue::Bool(b) => Some(LiteralValue::Bool(!b)),
                _ => None,
            }
        }
        _ => None,
    }
}
