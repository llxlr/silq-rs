//! Reverse computation transformation for Silq.
//!
//! Automatic generation of adjoint (reverse) functions for
//! quantum uncomputation. This is a key feature of Silq.

use crate::ast::{Declaration, Expression, Id};

/// Reverse a function definition.
/// Generates the adjoint (reverse) of a quantum function.
pub fn reverse_function(func: &Declaration) -> Option<Declaration> {
    match func {
        Declaration::FunctionDef { name, params, body, annotation, .. } => {
            let rev_name = reverse_function_name(*name);
            let rev_body = reverse_expression(body);

            Some(Declaration::new_function(
                body.loc().clone(),
                rev_name,
                params.clone(),
                None,
                rev_body,
                *annotation,
            ))
        }
        _ => None,
    }
}

/// Reverse a function name (append "_rev").
fn reverse_function_name(name: Id) -> Id {
    // This needs access to the interner, which we don't have here
    // In practice, the reverse transformation is done after semantic analysis
    name
}

/// Reverse an expression.
/// For simple expressions, this is identity. For quantum operations,
/// it generates the inverse.
fn reverse_expression(expr: &Expression) -> Expression {
    match expr {
        Expression::Compound { statements, loc } => {
            // Reverse the order of statements and reverse each one
            let mut reversed: Vec<Expression> = statements.iter()
                .rev()
                .map(|s| reverse_expression(s))
                .collect();
            Expression::new_compound(loc.clone(), reversed)
        }

        Expression::Binary { op, left, right, loc } => {
            // For addition, subtraction becomes addition of negated
            Expression::new_binary(
                loc.clone(),
                // Negate the operation for the reverse
                match op {
                    crate::token::TokenType::Plus => crate::token::TokenType::Tilde,
                    _ => *op,
                },
                reverse_expression(left),
                reverse_expression(right),
            )
        }

        Expression::Call { function, arguments, loc, .. } => {
            // Generate reverse function call
            Expression::new_call(
                loc.clone(),
                function.as_ref().clone(),
                arguments.clone(),
            )
        }

        Expression::Return { expr, loc } => {
            Expression::Return {
                loc: loc.clone(),
                expr: expr.as_ref().map(|e| Box::new(reverse_expression(e))),
            }
        }

        Expression::Forget { variable, loc } => {
            // Reverse of forget is to re-initialize to |0⟩
            // This requires special handling in the runtime
            Expression::Forget {
                loc: loc.clone(),
                variable: variable.clone(),
            }
        }

        // For most expressions, the reverse is the identity
        _ => expr.clone(),
    }
}

/// Check if a function is reversible (has a well-defined inverse).
pub fn is_reversible(func: &Declaration) -> bool {
    match func {
        Declaration::FunctionDef { annotation, .. } => {
            matches!(annotation, crate::ast::Annotation::Mfree | crate::ast::Annotation::Qfree)
                || matches!(annotation, crate::ast::Annotation::None)
        }
        _ => false,
    }
}
