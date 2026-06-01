//! HQIR (High-level Quantum Intermediate Representation) backend.
//!
//! Emits a text-based quantum IR format that can be consumed by
//! external quantum compilation tools.

use crate::ast::{Declaration, Expression};
use std::collections::HashMap;

/// The HQIR writer emits quantum IR text.
pub struct HqirWriter {
    /// Output buffer.
    output: String,
    /// Named registers.
    _registers: HashMap<String, usize>,
    /// Next register index.
    _next_reg: usize,
    /// Indentation level.
    indent: u32,
}

impl HqirWriter {
    pub fn new() -> Self {
        HqirWriter {
            output: String::new(),
            _registers: HashMap::new(),
            _next_reg: 0,
            indent: 0,
        }
    }

    /// Get the generated IR text.
    pub fn into_string(self) -> String {
        self.output
    }

    /// Write a line with indentation.
    fn writeln(&mut self, s: &str) {
        for _ in 0..self.indent {
            self.output.push_str("  ");
        }
        self.output.push_str(s);
        self.output.push('\n');
    }

    /// Compile a list of expressions to HQIR.
    pub fn compile(&mut self, _program: &[Expression]) {
        self.writeln("// HQIR output from Silq compiler (Rust)");
        self.writeln("");
        self.writeln("module silq_output {");
        self.indent += 1;

        // TODO: Walk the AST and emit HQIR for each expression
        self.writeln("// (HQIR backend is a stub - implement full compilation)");
        self.writeln("");

        self.indent -= 1;
        self.writeln("}");
    }

    /// Compile a single function to HQIR.
    pub fn compile_function(&mut self, func: &Declaration) {
        match func {
            Declaration::FunctionDef { name, params, .. } => {
                self.writeln(&format!("// function: {:?}", name));
                self.writeln(&format!("// params: {}", params.len()));
                self.writeln("// TODO: emit HQIR operations");
            }
            _ => {}
        }
    }

    /// Allocate a new quantum register.
    #[allow(dead_code)]
    fn alloc_qreg(&mut self, name: &str) -> String {
        let idx = self._next_reg;
        self._next_reg += 1;
        self._registers.insert(name.to_string(), idx);
        format!("%{}.{}", name, idx)
    }

    /// Allocate a new classical register.
    #[allow(dead_code)]
    fn alloc_creg(&mut self, name: &str) -> String {
        format!("${}.{}", name, 0)
    }
}
