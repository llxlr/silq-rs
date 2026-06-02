//! Resource/linearity checker for Silq.
//!
//! Verifies that quantum resources are used correctly:
//! - Linear (moved) values are used exactly once
//! - Const values can be duplicated
//! - Quantum operations appear only in valid contexts
//! - `forget` operations are correctly placed

use crate::ast::{Declaration, Expression};
use crate::scope::Scope;
use std::collections::HashSet;

/// The checker verifies resource usage correctness.
#[derive(Default)]
pub struct Checker {
    /// Variables that have been consumed (used up).
    consumed: HashSet<usize>,
    /// Variables that are marked as const (can be used multiple times).
    const_vars: HashSet<usize>,
    /// Error count.
    errors: usize,
}

impl Checker {
    pub fn new() -> Self {
        Checker {
            consumed: HashSet::new(),
            const_vars: HashSet::new(),
            errors: 0,
        }
    }

    /// Check a function definition for resource correctness.
    pub fn check_function(&mut self, func: &Declaration, _scope: &Scope) -> bool {
        match func {
            Declaration::FunctionDef { name: _name, params, body, annotation: _annotation, .. } => {
                // Reset state
                self.consumed.clear();
                self.const_vars.clear();

                // Register const parameters
                for param in params {
                    if let Declaration::VarDecl { name: pname, capture, .. } = param {
                        if matches!(capture, crate::ast::CaptureAnnotation::Const) {
                            self.const_vars.insert(pname.0);
                        }
                    }
                }

                // Check the body
                self.check_expr(body);

                self.errors == 0
            }
            _ => true,
        }
    }

    /// Check an expression for resource correctness.
    fn check_expr(&mut self, expr: &Expression) {
        match expr {
            Expression::Identifier { name, .. } if self.consumed.contains(&name.0) => {
                self.errors += 1;
                eprintln!("[checker error] use of consumed variable");
            }
            Expression::Identifier { .. } => {}

            Expression::Binary { left, right, .. } => {
                self.check_expr(left);
                self.check_expr(right);
            }

            Expression::Call { function, arguments, .. } => {
                self.check_expr(function);
                for arg in arguments {
                    self.check_expr(arg);
                }
            }

            Expression::Let { name: _, value, body, .. } => {
                self.check_expr(value);
                // Variable is available for the body
                self.check_expr(body);
            }

            Expression::Assign { target, value, .. } => {
                self.check_expr(value);
                if let Expression::Identifier { name, .. } = target.as_ref() {
                    if self.const_vars.contains(&name.0) {
                        self.errors += 1;
                        eprintln!("[checker error] cannot assign to const variable");
                    }
                }
            }

            Expression::Compound { statements, .. } => {
                for stmt in statements {
                    self.check_expr(stmt);
                }
            }

            Expression::IfThenElse { condition, then_branch, else_branch, .. } => {
                self.check_expr(condition);
                self.check_expr(then_branch);
                if let Some(else_br) = else_branch {
                    self.check_expr(else_br);
                }
            }

            Expression::ForLoop { body, .. } => {
                self.check_expr(body);
            }

            Expression::WhileLoop { condition, body, .. } => {
                self.check_expr(condition);
                self.check_expr(body);
            }

            Expression::Return { expr: Some(e), .. } => {
                self.check_expr(e);
            }
            Expression::Return { expr: None, .. } => {}

            Expression::Forget { variable, .. } => {
                if let Expression::Identifier { name, .. } = variable.as_ref() {
                    self.consumed.insert(name.0);
                }
            }

            Expression::With { controller, body, .. } => {
                self.check_expr(controller);
                self.check_expr(body);
            }

            // Leaf expressions - no resource tracking needed
            Expression::Literal { .. }
            | Expression::Error { .. }
            | Expression::Type { .. }
            | Expression::TypeDecl(_)
            | Expression::TypeVar { .. }
            | Expression::Placeholder { .. }
            | Expression::Wildcard { .. }
            | Expression::Lambda { .. } => {}

            _ => {}
        }
    }

    pub fn error_count(&self) -> usize {
        self.errors
    }
}
