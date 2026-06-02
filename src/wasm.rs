//! WASM bindings for the Silq quantum programming language.
//!
//! Exposes the lexer, parser, and quantum simulator to JavaScript
//! via wasm-bindgen. Compile with:
//!   cargo build --target wasm32-unknown-unknown --no-default-features --features wasm
//!
//! Then use wasm-bindgen or wasm-pack to generate JS bindings.

use crate::ast::Interner;
use crate::lexer::Lexer;
use crate::parser::Parser;
#[cfg(feature = "wasm-bindgen")]
use crate::qsim::QSim;

/// Parse and run a Silq program string, returning the result as a string.
#[cfg(feature = "wasm-bindgen")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn run_silq(source: &str) -> String {
    let mut interner = Interner::new();
    let mut lexer = Lexer::new(source);
    let mut parser = Parser::new(&mut lexer, &mut interner);
    let program = parser.parse_program();

    let mut sim = QSim::new(interner);
    match sim.run(&program) {
        Ok(value) => value.display(),
        Err(msg) => format!("error: {}", msg),
    }
}

/// Parse Silq source code and return the AST structure as a debug string.
pub fn parse_silq(source: &str) -> String {
    let mut interner = Interner::new();
    let mut lexer = Lexer::new(source);
    let mut parser = Parser::new(&mut lexer, &mut interner);
    let ast = parser.parse_program();
    format!("{:#?}", ast)
}

/// Tokenize Silq source code and return tokens as a string.
pub fn tokenize_silq(source: &str) -> String {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize_all();
    let items: Vec<String> = tokens.iter()
        .map(|t| format!("{:?}: '{}'", t.ty, t.text))
        .collect();
    items.join("\n")
}

/// Run a Silq program with quantum state dump.
#[cfg(feature = "wasm-bindgen")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn run_silq_dump(source: &str) -> String {
    let mut interner = Interner::new();
    let mut lexer = Lexer::new(source);
    let mut parser = Parser::new(&mut lexer, &mut interner);
    let program = parser.parse_program();

    let mut sim = QSim::new(interner.clone());
    match sim.run(&program) {
        Ok(_) => sim.dump_state(),
        Err(msg) => format!("error: {}", msg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_silq_wasm() {
        let result = parse_silq("def main() { return 42; }");
        assert!(result.contains("TypeDecl"));
    }

    #[test]
    fn test_tokenize_silq_wasm() {
        let result = tokenize_silq("x + 1");
        assert!(result.contains("Identifier"));
        assert!(result.contains("Plus"));
    }
}
