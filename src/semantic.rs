//! Semantic analysis for Silq.
//!
//! Performs name resolution, type inference, and basic semantic checking.

use crate::ast::{Declaration, Expression, Interner};
use crate::errors::Location;
use crate::scope::Scope;

/// Semantic analyzer state.
pub struct SemanticAnalyzer {
    /// The interner (for string lookups).
    pub interner: Interner,
    /// The current scope chain.
    pub scope: Scope,
    /// Type variable counter for fresh type variables.
    next_tvar: usize,
    /// Error count.
    errors: usize,
}

impl SemanticAnalyzer {
    /// Create a new semantic analyzer with a given scope.
    pub fn new(interner: Interner, scope: Scope) -> Self {
        SemanticAnalyzer {
            interner,
            scope,
            next_tvar: 0,
            errors: 0,
        }
    }

    /// Generate a fresh type variable.
    fn fresh_tvar(&mut self) -> Expression {
        let idx = self.next_tvar;
        self.next_tvar += 1;
        Expression::TypeVar { loc: Location::default(), index: idx }
    }

    /// Register an error.
    fn error(&mut self, msg: &str) {
        self.errors += 1;
        eprintln!("[semantic error] {}", msg);
    }

    /// Perform semantic analysis on a list of top-level expressions.
    pub fn semantic_program(&mut self, exprs: &mut [Expression]) {
        for expr in exprs.iter_mut() {
            self.semantic_expr(expr);
        }
    }

    /// Perform semantic analysis on a single expression.
    fn semantic_expr(&mut self, expr: &mut Expression) {
        match expr {
            Expression::TypeDecl(decl) => {
                self.semantic_declaration(decl);
            }

            Expression::Identifier { name, meaning, .. } => {
                // Try to resolve the identifier in the current scope
                if let Some(decl) = self.scope.lookup(*name) {
                    *meaning = Some(Box::new(decl.clone()));
                }
            }

            Expression::Binary { left, right, .. } => {
                self.semantic_expr(left);
                self.semantic_expr(right);
            }

            Expression::Call { function, arguments, callee, .. } => {
                self.semantic_expr(function);

                // Try to resolve the function
                if let Expression::Identifier { name, .. } = function.as_ref() {
                    if let Some(decl) = self.scope.lookup(*name) {
                        *callee = Some(Box::new(decl.clone()));
                    }
                }

                for arg in arguments.iter_mut() {
                    self.semantic_expr(arg);
                }
            }

            Expression::IfThenElse { condition, then_branch, else_branch, .. } => {
                self.semantic_expr(condition);
                self.semantic_expr(then_branch);
                if let Some(else_br) = else_branch {
                    self.semantic_expr(else_br);
                }
            }

            Expression::Let { name, value, body, type_ann, .. } => {
                // Create a variable declaration for the new binding
                let decl = Declaration::new_var(
                    Location::default(),
                    *name,
                    type_ann.as_ref().map(|t| t.as_ref().clone()),
                    None,
                    false,
                    crate::ast::CaptureAnnotation::None,
                );
                self.scope.insert(*name, decl);

                self.semantic_expr(value);
                self.semantic_expr(body);
            }

            Expression::Compound { statements, .. } => {
                for stmt in statements.iter_mut() {
                    self.semantic_expr(stmt);
                }
            }

            Expression::Return { expr, .. } => {
                if let Some(e) = expr {
                    self.semantic_expr(e);
                }
            }

            Expression::ForLoop { iterable, body, .. } => {
                self.semantic_expr(iterable);
                self.semantic_expr(body);
            }

            Expression::WhileLoop { condition, body, .. } => {
                self.semantic_expr(condition);
                self.semantic_expr(body);
            }

            Expression::Assign { target, value, .. } => {
                self.semantic_expr(target);
                self.semantic_expr(value);
            }

            Expression::Forget { variable, .. } => {
                self.semantic_expr(variable);
            }

            Expression::Assert { condition, .. } => {
                self.semantic_expr(condition);
            }

            Expression::Lambda { body, .. } => {
                self.semantic_expr(body);
            }

            Expression::TypeAnnotation { expr, ty, .. } => {
                self.semantic_expr(expr);
                self.semantic_expr(ty);
            }

            Expression::With { controller, body, .. } => {
                self.semantic_expr(controller);
                self.semantic_expr(body);
            }

            Expression::Index { expr, index, .. } => {
                self.semantic_expr(expr);
                self.semantic_expr(index);
            }

            Expression::UnaryMinus { expr, .. }
            | Expression::UnaryPlus { expr, .. }
            | Expression::LogicalNot { expr, .. }
            | Expression::BitwiseNot { expr, .. }
            | Expression::Typeof { expr, .. } => {
                self.semantic_expr(expr);
            }

            // Leaf expressions - nothing to analyze
            Expression::Literal { .. }
            | Expression::Error { .. }
            | Expression::Placeholder { .. }
            | Expression::Wildcard { .. }
            | Expression::Type { .. }
            | Expression::TypeVar { .. } => {}

            Expression::Slice { expr, start, end, .. } => {
                self.semantic_expr(expr);
                self.semantic_expr(start);
                self.semantic_expr(end);
            }
            Expression::Comma { left, right, .. } => {
                self.semantic_expr(left);
                self.semantic_expr(right);
            }

            Expression::Field { expr, .. } => {
                self.semantic_expr(expr);
            }

            Expression::Tuple { elements, .. }
            | Expression::Vector { elements, .. } => {
                for el in elements.iter_mut() {
                    self.semantic_expr(el);
                }
            }

            Expression::Concat { left, right, .. } => {
                self.semantic_expr(left);
                self.semantic_expr(right);
            }

            Expression::Repeat { count, body, .. } => {
                self.semantic_expr(count);
                self.semantic_expr(body);
            }
        }
    }

    /// Perform semantic analysis on a declaration.
    fn semantic_declaration(&mut self, decl: &mut Declaration) {
        match decl {
            Declaration::FunctionDef { .. } => {
                // Clone from the original declaration, not the destructured fields
                let func_decl = decl.clone();
                let func_name = match decl {
                    Declaration::FunctionDef { name, .. } => *name,
                    _ => unreachable!(),
                };
                let func_params = match decl {
                    Declaration::FunctionDef { params, .. } => params.clone(),
                    _ => unreachable!(),
                };
                let mut func_body = match decl {
                    Declaration::FunctionDef { body, .. } => body.clone(),
                    _ => unreachable!(),
                };
                self.scope.insert(func_name, func_decl);

                // Create a new scope for the function body
                let old_scope = self.scope.clone();
                self.scope = self.scope.child();

                // Add parameters to the scope
                for param in func_params.iter() {
                    if let Declaration::VarDecl { name: pname, .. } = param {
                        self.scope.insert(*pname, param.clone());
                    }
                }

                // Analyze the body
                self.semantic_expr(&mut *func_body);

                // Restore the old scope
                self.scope = old_scope;
            }

            Declaration::DatDecl { name, .. } => {
                self.scope.insert(*name, decl.clone());
            }

            Declaration::VarDecl { .. } => {
                // Variable declarations are handled when they appear in context
            }

            Declaration::Import { .. } => {
                // Import semantics handled by the module system
            }
        }
    }

    /// Get the number of errors encountered.
    pub fn error_count(&self) -> usize {
        self.errors
    }
}
