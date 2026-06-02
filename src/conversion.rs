//! Type conversion system for Silq.
//!
//! Handles implicit and explicit type conversions between numeric types,
//! type coercions, and type punning (reinterpretation).

use crate::ast::{Expression, NumericType, TypeKind};

/// Check if a value of type `from` can be implicitly converted to type `to`.
pub fn can_implicit_convert(from: &Expression, to: &Expression) -> bool {
    // If they're the same type, no conversion needed
    if from == to {
        return true;
    }

    match (get_numeric_type(from), get_numeric_type(to)) {
        (Some(ft), Some(tt)) => ft.is_subtype_of(tt),
        _ => false,
    }
}

/// Get the numeric type from an expression (if it's a numeric type expression).
fn get_numeric_type(expr: &Expression) -> Option<NumericType> {
    match expr {
        Expression::Type { kind: TypeKind::Numeric(nt), .. } => Some(*nt),
        Expression::Type { kind: TypeKind::Classical(inner), .. } => get_numeric_type(inner),
        _ => None,
    }
}

/// Check if an expression is a classical type.
pub fn is_classical_type(expr: &Expression) -> bool {
    match expr {
        Expression::Type { kind: TypeKind::Classical(_), .. } => true,
        // ℕ,ℤ,ℚ,ℝ are inherently classical; 𝔹,ℂ are quantum unless wrapped in Classical
        Expression::Type { kind: TypeKind::Numeric(nt), .. } => match nt {
            NumericType::Bool | NumericType::Complex => false,
            NumericType::Nat | NumericType::Int | NumericType::Rat | NumericType::Real => true,
        },
        Expression::Type { kind: TypeKind::FixedInt(ref fit), .. } => fit.is_classical,
        Expression::Type { kind: TypeKind::ZMod(ref zm), .. } => zm.is_classical,
        Expression::Type { kind: TypeKind::Unit, .. } => true,
        _ => false,
    }
}

/// Check if an expression is a quantum type.
pub fn is_quantum_type(expr: &Expression) -> bool {
    !is_classical_type(expr)
}
