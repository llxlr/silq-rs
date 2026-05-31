//! Silq quantum programming language - Rust implementation.
//!
//! Silq is a high-level quantum programming language with a strong static
//! type system, developed at ETH Zurich. This is a Rust reimplementation
//! of the original D language compiler.
//!
//! ## Compilation Pipeline
//!
//! ```text
//! Source (.slq) → Lexer → Parser → Semantic Analysis → Checker → Backend (QSim/HQIR)
//! ```
//!
//! ## Key Features
//!
//! - Automatic uncomputation (reverse transformation)
//! - Linear type system for quantum resources
//! - Dependent types (types parameterized by values)
//! - Quantum/classical type distinction

pub mod token;
pub mod lexer;
pub mod ast;
pub mod parser;
pub mod scope;
pub mod semantic;
pub mod checker;
pub mod consteval;
pub mod conversion;
pub mod reverse;
pub mod modules;
pub mod errors;
pub mod qsim;
pub mod hqir;
pub mod options;

// Re-export key types for convenience
pub use ast::{
    Expression, Declaration, NumericType,
};
pub use lexer::Lexer;
pub use parser::Parser;
pub use errors::{ErrorHandler, SimpleErrorHandler};
pub use qsim::{QSim, QState, Interpreter};
pub use modules::import_module;
