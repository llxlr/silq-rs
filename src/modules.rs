//! Module system for Silq.
//!
//! Handles module importing, prelude loading, and the module cache.

use crate::ast::{Expression, Interner};
#[cfg(not(target_arch = "wasm32"))]
use crate::ast::Declaration;
#[cfg(not(target_arch = "wasm32"))]
use crate::errors::Location;
use crate::lexer::Lexer;
use crate::parser::Parser;
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

/// The module cache stores parsed and semantically-analyzed modules.
#[derive(Default)]
pub struct ModuleCache {
    /// Cached modules: path -> list of top-level expressions.
    modules: HashMap<String, Vec<Expression>>,
    /// Import search paths.
    #[cfg(not(target_arch = "wasm32"))]
    import_paths: Vec<PathBuf>,
}

impl ModuleCache {
    pub fn new() -> Self {
        ModuleCache {
            modules: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            import_paths: Vec::new(),
        }
    }

    /// Add an import search path.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn add_path(&mut self, path: &str) {
        self.import_paths.push(PathBuf::from(path));
    }

    /// Import a module by name, parsing it if not yet cached.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn import_module(&mut self, name: &str,
                         interner: &mut Interner,
                         _err_handler: &mut dyn crate::errors::ErrorHandler) -> Option<&Vec<Expression>> {
        // Check cache first
        if self.modules.contains_key(name) {
            return self.modules.get(name);
        }

        // Search for the module file
        let path = self.resolve_module_path(name)?;
        let source = fs::read_to_string(&path).ok()?;

        // Parse the module
        let mut lexer = Lexer::new(&source);
        let mut parser = Parser::new(&mut lexer, interner);
        let ast = parser.parse_program();

        self.modules.insert(name.to_string(), ast);
        self.modules.get(name)
    }

    /// Resolve a module name to a file path.
    #[cfg(not(target_arch = "wasm32"))]
    fn resolve_module_path(&self, module_name: &str) -> Option<PathBuf> {
        // Convert module name to path: a.b.c -> a/b/c.slq
        let rel_path = module_name.replace('.', "/") + ".slq";

        // Check in each import path
        for base in &self.import_paths {
            let full = base.join(&rel_path);
            if full.exists() {
                return Some(full);
            }
        }

        // Check relative to current directory
        if Path::new(&rel_path).exists() {
            return Some(PathBuf::from(&rel_path));
        }

        None
    }
}

/// Load the prelude (standard library).
/// This is a built-in operation that loads the standard library definitions.
pub fn load_prelude(cache: &mut ModuleCache,
                    interner: &mut Interner,
                    _err_handler: &mut dyn crate::errors::ErrorHandler) -> Vec<Expression> {
    // The prelude is embedded in the compiler
    let prelude_source = include_str!("../library/prelude.slq");

    let mut lexer = Lexer::new(prelude_source);
    let mut parser = Parser::new(&mut lexer, interner);
    let ast = parser.parse_program();

    cache.modules.insert("prelude".to_string(), ast.clone());
    ast
}

/// Import a module from a specific source file path.
#[cfg(not(target_arch = "wasm32"))]
pub fn import_module(path: &str,
                     interner: &mut Interner,
                     cache: &mut ModuleCache,
                     err_handler: &mut dyn crate::errors::ErrorHandler) -> Option<Vec<Expression>> {
    let source = fs::read_to_string(path).ok()?;

    // Also load the prelude
    let _prelude_asts = load_prelude(cache, interner, err_handler);

    let mut lexer = Lexer::new(&source);
    let mut parser = Parser::new(&mut lexer, interner);
    let mut ast = parser.parse_program();
    ast.insert(0, Expression::TypeDecl(Box::new(Declaration::Import {
        loc: Location::default(),
        path: "prelude".into(),
    })));

    Some(ast)
}

/// Parse source code into an AST, including prelude.
pub fn parse_source(source: &str, interner: &mut Interner,
                     _err_handler: &mut dyn crate::errors::ErrorHandler) -> Vec<Expression> {
    // Load prelude first
    let prelude_source = include_str!("../library/prelude.slq");
    let mut prelude_lexer = Lexer::new(prelude_source);
    let mut prelude_parser = Parser::new(&mut prelude_lexer, interner);
    let mut prelude_asts = prelude_parser.parse_program();

    // Parse user source
    let mut lexer = Lexer::new(source);
    let mut parser = Parser::new(&mut lexer, interner);
    let mut user_asts = parser.parse_program();

    // Combine: prelude first, then user code
    prelude_asts.append(&mut user_asts);
    prelude_asts
}
