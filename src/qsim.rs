//! Quantum simulator for Silq.
//!
//! Maintains a full state vector and interprets Silq AST against it.
//! Supports:
//! - State vector representation (sparse mapping from basis states to complex amplitudes)
//! - Named qubit variable bindings
//! - Quantum gates (H, X, Y, Z, phase, rotation, CNOT)
//! - Measurement (state collapse)
//! - Classical values alongside quantum values
//! - Control flow (if/else, for, while, with blocks)

use crate::ast::{Declaration, Expression, Interner, LiteralValue, Id};
use num_complex::Complex64;
use num_traits::Zero;
use std::collections::{BTreeMap, HashMap};

// Type aliases for readability
type Amplitude = Complex64;
type Probability = f64;

// =============================================================================
// Quantum Values
// =============================================================================

/// A value in the Silq runtime system (quantum and classical).
#[derive(Debug, Clone)]
pub enum Value {
    /// Boolean (classical or measured qubit).
    Bool { val: bool },
    /// Integer (big integer for arbitrary precision).
    Int(num_bigint::BigInt),
    /// Fixed-width integer.
    IntFixed { val: u64, bits: u32, signed: bool },
    /// Floating-point value.
    Float(f64),
    /// Complex value.
    Complex(Complex64),
    /// Rational value.
    Rational(num_bigint::BigInt, num_bigint::BigInt),
    /// Quantum variable reference (index into the quantum state).
    QVar { index: usize, name: Id },
    /// Tuple of values.
    Tuple(Vec<Value>),
    /// Array/vector of values.
    Array(Vec<Value>),
    /// Unit value.
    Unit,
    /// A function closure.
    Closure {
        func_id: Id,
        captures: HashMap<usize, Value>,
    },
    /// Error value.
    Error(String),
}

impl Value {
    /// Check if this value is classical (not a quantum variable).
    pub fn is_classical(&self) -> bool {
        !matches!(self, Value::QVar { .. })
    }

    /// Try to extract a boolean value.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool { val } => Some(*val),
            _ => None,
        }
    }

    /// Try to extract an integer value.
    pub fn as_int(&self) -> Option<num_bigint::BigInt> {
        match self {
            Value::Int(n) => Some(n.clone()),
            Value::IntFixed { val, .. } => Some(num_bigint::BigInt::from(*val)),
            _ => None,
        }
    }

    /// Try to extract an f64 value.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(n) => {
                let (sign, digits) = n.to_u64_digits();
                if digits.len() <= 1 {
                    Some(digits.get(0).copied().unwrap_or(0) as f64 * if sign == num_bigint::Sign::Minus { -1.0 } else { 1.0 })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn display(&self) -> String {
        match self {
            Value::Bool { val } => format!("{}", val),
            Value::Int(n) => format!("{}", n),
            Value::IntFixed { val, .. } => format!("{}", val),
            Value::Float(f) => format!("{}", f),
            Value::Complex(c) => format!("{}", c),
            Value::Rational(n, d) => format!("{}/{}", n, d),
            Value::QVar { index, .. } => format!("qubit[{}]", index),
            Value::Tuple(elems) => {
                let s: Vec<String> = elems.iter().map(|v| v.display()).collect();
                format!("({})", s.join(", "))
            }
            Value::Array(elems) => {
                let s: Vec<String> = elems.iter().map(|v| v.display()).collect();
                format!("[{}]", s.join(", "))
            }
            Value::Unit => "()".to_string(),
            Value::Closure { func_id, .. } => format!("<closure #{}>", func_id.0),
            Value::Error(e) => format!("<error: {}>", e),
        }
    }
}

// =============================================================================
// Quantum State
// =============================================================================

/// Represents a computational basis state for a set of qubits.
/// Each qubit's value (0 or 1) is stored in a bit vector.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BasisState {
    /// Bit vector: least significant bit = qubit 0.
    bits: Vec<u8>,
    /// Number of qubits.
    n: usize,
}

impl BasisState {
    pub fn new(n: usize) -> Self {
        BasisState { bits: vec![0; n], n }
    }

    pub fn from_bitstring(s: &str) -> Self {
        let n = s.len();
        let mut bits = vec![0; n];
        for (i, ch) in s.chars().rev().enumerate() {
            bits[i] = if ch == '1' { 1 } else { 0 };
        }
        BasisState { bits, n }
    }

    /// Get the value of qubit i.
    pub fn get(&self, i: usize) -> u8 {
        if i < self.bits.len() { self.bits[i] } else { 0 }
    }

    /// Set the value of qubit i.
    pub fn set(&mut self, i: usize, val: u8) {
        if i >= self.bits.len() {
            self.bits.resize(i + 1, 0);
            self.n = self.bits.len();
        }
        self.bits[i] = val & 1;
    }

    /// Get the integer value of qubits [start..start+n].
    pub fn get_range(&self, start: usize, n: usize) -> u64 {
        let mut val = 0u64;
        for i in 0..n {
            val |= (self.get(start + i) as u64) << i;
        }
        val
    }

    /// Set the integer value of qubits [start..start+n].
    pub fn set_range(&mut self, start: usize, n: usize, val: u64) {
        for i in 0..n {
            self.set(start + i, ((val >> i) & 1) as u8);
        }
    }

    /// Toggle qubit i.
    pub fn toggle(&self, i: usize) -> BasisState {
        let mut new = self.clone();
        if i < new.bits.len() {
            new.bits[i] ^= 1;
        }
        new
    }

    pub fn display(&self) -> String {
        let s: String = self.bits.iter().rev().map(|b| if *b == 1 { '1' } else { '0' }).collect();
        format!("|{}⟩", s)
    }
}

// =============================================================================
// Quantum State Vector
// =============================================================================

/// The quantum state: a sparse mapping from basis states to complex amplitudes.
#[derive(Debug, Clone)]
pub struct QState {
    /// Sparse state vector: basis state -> amplitude.
    pub amplitudes: BTreeMap<BasisState, Amplitude>,
    /// Number of qubits in the system.
    pub num_qubits: usize,
    /// Named variables: name -> (start_qubit_index, num_qubits_used).
    pub variables: HashMap<usize, (usize, usize)>,
    /// Classical variable bindings.
    pub classical_vars: HashMap<usize, Value>,
    /// Next available qubit index.
    next_qubit: usize,
}

impl QState {
    /// Create a new empty quantum state.
    pub fn new() -> Self {
        let mut amps = BTreeMap::new();
        amps.insert(BasisState::new(0), Amplitude::new(1.0, 0.0));
        QState {
            amplitudes: amps,
            num_qubits: 0,
            variables: HashMap::new(),
            classical_vars: HashMap::new(),
            next_qubit: 0,
        }
    }

    /// Allocate a new qubit initialized to |0⟩, associated with a variable.
    pub fn alloc_qubit(&mut self, name: Id) -> usize {
        let qidx = self.next_qubit;
        self.next_qubit += 1;
        self.num_qubits += 1;
        self.variables.insert(name.0, (qidx, 1));
        qidx
    }

    /// Get the qubit index for a variable.
    pub fn get_qubit(&self, name: Id) -> Option<usize> {
        self.variables.get(&name.0).map(|&(qidx, _)| qidx)
    }

    /// Store a classical value for a variable.
    pub fn store_classical(&mut self, name: Id, value: Value) {
        self.classical_vars.insert(name.0, value);
    }

    /// Get a classical value for a variable.
    pub fn get_classical(&self, name: Id) -> Option<&Value> {
        self.classical_vars.get(&name.0)
    }

    /// Apply a single-qubit unitary gate.
    pub fn apply_1q_gate(&mut self, qubit: usize, gate: [[Amplitude; 2]; 2]) {
        let mut new_amplitudes = BTreeMap::new();
        for (state, amp) in &self.amplitudes {
            let bit = state.get(qubit) as usize;
            let new_state = state.toggle(qubit);
            // |0⟩ -> gate[0][0]|0⟩ + gate[1][0]|1⟩
            // |1⟩ -> gate[0][1]|0⟩ + gate[1][1]|1⟩
            *new_amplitudes.entry(state.clone()).or_insert(Amplitude::zero()) += amp * gate[0][bit];
            *new_amplitudes.entry(new_state).or_insert(Amplitude::zero()) += amp * gate[1][bit];
        }
        self.amplitudes = new_amplitudes;
    }

    /// Apply a controlled unitary gate: if control qubit is |1⟩, apply gate to target.
    pub fn apply_cnot(&mut self, control: usize, target: usize) {
        let mut new_amplitudes = BTreeMap::new();
        for (state, amp) in &self.amplitudes {
            if state.get(control) == 1 {
                // Target is flipped
                let new_state = state.toggle(target);
                *new_amplitudes.entry(new_state).or_insert(Amplitude::zero()) += amp;
            } else {
                *new_amplitudes.entry(state.clone()).or_insert(Amplitude::zero()) += amp;
            }
        }
        self.amplitudes = new_amplitudes;
    }

    /// Measure a qubit, collapsing the state.
    /// Returns the measured value (0 or 1) and updates the state.
    pub fn measure(&mut self, qubit: usize) -> u8 {
        // Compute probability of measuring |0⟩
        let mut prob0 = 0.0f64;
        let mut prob1 = 0.0f64;
        for (state, amp) in &self.amplitudes {
            let p = amp.norm_sqr();
            if state.get(qubit) == 0 {
                prob0 += p;
            } else {
                prob1 += p;
            }
        }

        // Random choice based on probabilities
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        prob0.to_bits().hash(&mut hasher);
        let rand_val = (hasher.finish() as f64) / (u64::MAX as f64);
        let result = if rand_val < prob0 { 0 } else { 1 };

        // Collapse the state
        let norm = if result == 0 { prob0.sqrt() } else { prob1.sqrt() };
        if norm < 1e-15 {
            // Near zero probability, reinitialize to |0⟩
            let mut amps = BTreeMap::new();
            let mut new_state = BasisState::new(self.num_qubits);
            new_state.set(qubit, result);
            amps.insert(new_state, Amplitude::new(1.0, 0.0));
            self.amplitudes = amps;
            return result;
        }

        let inv_norm = 1.0 / norm;
        let mut new_amplitudes = BTreeMap::new();
        for (state, amp) in &self.amplitudes {
            if state.get(qubit) == result {
                new_amplitudes.insert(state.clone(), amp * inv_norm);
            }
        }
        self.amplitudes = new_amplitudes;

        result
    }

    /// Dump the quantum state for debugging.
    pub fn dump(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!("Quantum state ({} qubits):\n", self.num_qubits));
        output.push_str("  Basis state    | Amplitude\n");
        output.push_str("  -------------- | ----------\n");
        for (state, amp) in &self.amplitudes {
            let p = amp.norm_sqr();
            if p > 1e-10 {
                output.push_str(&format!("  {}  {:.6}\n", state.display(), amp));
            }
        }
        output
    }
}

// =============================================================================
// Standard Quantum Gates
// =============================================================================

/// Hadamard gate matrix: H = 1/√2 [[1, 1], [1, -1]]
pub fn hadamard_gate() -> [[Amplitude; 2]; 2] {
    let s = 1.0 / (2.0f64).sqrt();
    [
        [Amplitude::new(s, 0.0), Amplitude::new(s, 0.0)],
        [Amplitude::new(s, 0.0), Amplitude::new(-s, 0.0)],
    ]
}

/// Pauli-X gate (NOT): X = [[0, 1], [1, 0]]
pub fn pauli_x_gate() -> [[Amplitude; 2]; 2] {
    [
        [Amplitude::zero(), Amplitude::new(1.0, 0.0)],
        [Amplitude::new(1.0, 0.0), Amplitude::zero()],
    ]
}

/// Pauli-Y gate: Y = [[0, -i], [i, 0]]
pub fn pauli_y_gate() -> [[Amplitude; 2]; 2] {
    [
        [Amplitude::zero(), Amplitude::new(0.0, -1.0)],
        [Amplitude::new(0.0, 1.0), Amplitude::zero()],
    ]
}

/// Pauli-Z gate: Z = [[1, 0], [0, -1]]
pub fn pauli_z_gate() -> [[Amplitude; 2]; 2] {
    [
        [Amplitude::new(1.0, 0.0), Amplitude::zero()],
        [Amplitude::zero(), Amplitude::new(-1.0, 0.0)],
    ]
}

/// Phase gate: P(phi) = [[1, 0], [0, e^{i*phi}]]
pub fn phase_gate(phi: f64) -> [[Amplitude; 2]; 2] {
    let e = Amplitude::new(phi.cos(), phi.sin());
    [
        [Amplitude::new(1.0, 0.0), Amplitude::zero()],
        [Amplitude::zero(), e],
    ]
}

/// Rotation-X gate: Rx(theta) = cos(theta/2)I - i sin(theta/2)X
pub fn rot_x_gate(theta: f64) -> [[Amplitude; 2]; 2] {
    let c = (theta / 2.0).cos();
    let s = (theta / 2.0).sin();
    [
        [Amplitude::new(c, 0.0), Amplitude::new(0.0, -s)],
        [Amplitude::new(0.0, -s), Amplitude::new(c, 0.0)],
    ]
}

/// Rotation-Y gate: Ry(theta) = cos(theta)I - i sin(theta)Y
pub fn rot_y_gate(theta: f64) -> [[Amplitude; 2]; 2] {
    let c = (theta / 2.0).cos();
    let s = (theta / 2.0).sin();
    [
        [Amplitude::new(c, 0.0), Amplitude::new(-s, 0.0)],
        [Amplitude::new(s, 0.0), Amplitude::new(c, 0.0)],
    ]
}

/// Rotation-Z gate: Rz(theta) = [[e^{-iθ/2}, 0], [0, e^{iθ/2}]]
pub fn rot_z_gate(theta: f64) -> [[Amplitude; 2]; 2] {
    let ar = (theta / 2.0).cos();
    let ai = (theta / 2.0).sin();
    [
        [Amplitude::new(ar, -ai), Amplitude::zero()],
        [Amplitude::zero(), Amplitude::new(ar, ai)],
    ]
}

// =============================================================================
// Interpreter
// =============================================================================

/// The Silq interpreter: walks the AST and performs operations on the quantum state.
pub struct Interpreter {
    /// The quantum state.
    pub state: QState,
    /// The interner for string/id lookups.
    pub interner: Interner,
    /// Function definitions available.
    pub functions: HashMap<usize, Declaration>,
    /// Whether to trace execution.
    pub trace: bool,
}

impl Interpreter {
    pub fn new(interner: Interner) -> Self {
        Interpreter {
            state: QState::new(),
            interner,
            functions: HashMap::new(),
            trace: false,
        }
    }

    /// Register a function definition for later execution.
    pub fn register_function(&mut self, decl: &Declaration) {
        if let Declaration::FunctionDef { name, .. } = decl {
            self.functions.insert(name.0, decl.clone());
        }
    }

    /// Execute a program (list of top-level expressions).
    pub fn run_program(&mut self, program: &[Expression]) -> Result<Value, String> {
        // First pass: register function definitions
        for expr in program {
            if let Expression::TypeDecl(decl) = expr {
                self.register_function(decl);
            }
        }

        // Find and execute main
        let main_name = self.interner.intern("main");
        let main_decl = self.functions.values().find(|d| {
            if matches!(d, Declaration::FunctionDef { is_main: true, .. }) {
                return true;
            }
            if let Declaration::FunctionDef { name, .. } = d {
                if *name == main_name {
                    return true;
                }
            }
            false
        }).cloned();

        if let Some(main) = main_decl {
            self.call_function(&main, &[])
        } else {
            // No main function, evaluate each expression
            let mut last_value = Value::Unit;
            for expr in program {
                if matches!(expr, Expression::TypeDecl(_)) {
                    continue; // Skip function defs
                }
                last_value = self.eval(expr)?;
            }
            Ok(last_value)
        }
    }

    /// Evaluate an expression.
    pub fn eval(&mut self, expr: &Expression) -> Result<Value, String> {
        if self.trace {
            eprintln!("[trace] eval: {:?}", std::mem::discriminant(expr));
        }

        match expr {
            Expression::Literal { value, .. } => {
                self.eval_literal(value)
            }

            Expression::Identifier { name, meaning, .. } => {
                // Check if it has a meaning (resolved to declaration)
                if let Some(decl) = meaning {
                    match decl.as_ref() {
                        Declaration::FunctionDef { .. } => {
                            return Ok(Value::Closure {
                                func_id: *name,
                                captures: HashMap::new(),
                            });
                        }
                        _ => {}
                    }
                }
                // Look up in classical vars
                if let Some(val) = self.state.get_classical(*name) {
                    return Ok(val.clone());
                }
                // Look up in quantum vars
                if let Some(qidx) = self.state.get_qubit(*name) {
                    return Ok(Value::QVar { index: qidx, name: *name });
                }
                // Look up functions
                if let Some(decl) = self.functions.get(&name.0) {
                    return Ok(Value::Closure {
                        func_id: *name,
                        captures: HashMap::new(),
                    });
                }
                Err(format!("undefined variable: {}", self.interner.lookup(*name)))
            }

            Expression::Binary { op, left, right, .. } => {
                self.eval_binary(*op, left, right)
            }

            Expression::Call { function, arguments, callee, .. } => {
                // Check if it's a direct function call
                if let Some(decl) = callee.as_ref() {
                    return self.call_function(decl, arguments);
                }

                if let Expression::Identifier { name, .. } = function.as_ref() {
                    let func_name = self.interner.lookup(*name).to_string();
                    return self.call_builtin(&func_name, arguments);
                }

                // Evaluate function expression
                let func_val = self.eval(function)?;
                match func_val {
                    Value::Closure { func_id, captures } => {
                        let decl = self.functions.get(&func_id.0)
                            .cloned()
                            .ok_or_else(|| format!("undefined function: {}", self.interner.lookup(func_id)))?;
                        self.call_function(&decl, arguments)
                    }
                    _ => Err("not a function".into()),
                }
            }

            Expression::IfThenElse { condition, then_branch, else_branch, .. } => {
                let cond = self.eval(condition)?;
                match cond.as_bool() {
                    Some(true) => self.eval(then_branch),
                    Some(false) => {
                        if let Some(else_br) = else_branch {
                            self.eval(else_br)
                        } else {
                            Ok(Value::Unit)
                        }
                    }
                    _ => Err("if condition must be boolean".into()),
                }
            }

            Expression::Let { name, type_ann, value, body, .. } => {
                let val = self.eval(value)?;
                self.state.store_classical(*name, val);
                let result = self.eval(body)?;
                self.state.classical_vars.remove(&name.0);
                Ok(result)
            }

            Expression::Assign { target, value, .. } => {
                let val = self.eval(value)?;
                if let Expression::Identifier { name, .. } = target.as_ref() {
                    // Check if it's a quantum variable
                    if self.state.get_qubit(*name).is_some() {
                        // Assigning to a quantum variable is handled by the gate
                        return Err("direct quantum assignment not yet supported".into());
                    }
                    self.state.store_classical(*name, val.clone());
                    Ok(val)
                } else {
                    Err("assignment target must be a variable".into())
                }
            }

            Expression::Return { expr, .. } => {
                match expr {
                    Some(e) => self.eval(e),
                    None => Ok(Value::Unit),
                }
            }

            Expression::Compound { statements, .. } => {
                let mut last = Value::Unit;
                for stmt in statements {
                    last = self.eval(stmt)?;
                }
                Ok(last)
            }

            Expression::TypeAnnotation { expr, kind, .. } => {
                // For now, ignore type annotations at runtime
                self.eval(expr)
            }

            Expression::ForLoop { variable, iterable, body, .. } => {
                self.eval_for_loop(*variable, iterable, body)
            }

            Expression::WhileLoop { condition, body, .. } => {
                self.eval_while_loop(condition, body)
            }

            Expression::With { controller, body, .. } => {
                self.eval_with(controller, body)
            }

            Expression::Forget { variable, .. } => {
                // forget(var) - release a quantum variable
                self.eval_forget(variable)
            }

            Expression::Assert { condition, message, .. } => {
                let cond = self.eval(condition)?;
                if cond.as_bool() != Some(true) {
                    return Err(message.clone().unwrap_or_else(|| "assertion failed".into()));
                }
                Ok(Value::Unit)
            }

            Expression::Tuple { elements, .. } => {
                let mut vals = Vec::new();
                for e in elements {
                    vals.push(self.eval(e)?);
                }
                Ok(Value::Tuple(vals))
            }

            Expression::Lambda { params, body, annotation, .. } => {
                Err("lambda expressions not yet executed at runtime".into())
            }

            Expression::Type { .. } => Ok(Value::Unit),
            Expression::TypeDecl(_) => Ok(Value::Unit),
            Expression::TypeVar { .. } => Ok(Value::Unit),
            Expression::Placeholder { .. } | Expression::Wildcard { .. } => Ok(Value::Unit),
            Expression::Typeof { .. } => Ok(Value::Error("typeof not evaluated at runtime".into())),

            _ => {
                Err(format!("unimplemented expression: {:?}", std::mem::discriminant(expr)))
            }
        }
    }

    /// Evaluate a literal value.
    fn eval_literal(&self, lit: &LiteralValue) -> Result<Value, String> {
        match lit {
            LiteralValue::Bool(b) => Ok(Value::Bool { val: *b }),
            LiteralValue::Int(n) => Ok(Value::Int(n.clone())),
            LiteralValue::Float(f) => Ok(Value::Float(*f)),
            LiteralValue::Rational(n, d) => Ok(Value::Rational(n.clone(), d.clone())),
            LiteralValue::String(s) => Ok(Value::Error(format!("strings not supported: {}", s))),
            LiteralValue::Char(c) => Ok(Value::Error(format!("chars not supported: {}", c))),
            LiteralValue::Unit => Ok(Value::Unit),
        }
    }

    /// Evaluate a binary operation.
    fn eval_binary(&mut self, op: crate::token::TokenType,
                   left: &Expression, right: &Expression) -> Result<Value, String> {
        use crate::token::TokenType;

        match op {
            TokenType::Plus => {
                let l = self.eval(left)?;
                let r = self.eval(right)?;
                self.eval_add(&l, &r)
            }
            TokenType::Minus => {
                let l = self.eval(left)?;
                let r = self.eval(right)?;
                self.eval_sub(&l, &r)
            }
            TokenType::Mul => {
                let l = self.eval(left)?;
                let r = self.eval(right)?;
                self.eval_mul(&l, &r)
            }
            TokenType::Eq => {
                let l = self.eval(left)?;
                let r = self.eval(right)?;
                Ok(Value::Bool { val: l.display() == r.display() })
            }
            TokenType::Neq => {
                let l = self.eval(left)?;
                let r = self.eval(right)?;
                Ok(Value::Bool { val: l.display() != r.display() })
            }
            TokenType::Lt => {
                let l = self.eval(left)?;
                let r = self.eval(right)?;
                Ok(Value::Bool { val: l.display() < r.display() })
            }
            TokenType::Gt => {
                let l = self.eval(left)?;
                let r = self.eval(right)?;
                Ok(Value::Bool { val: l.display() > r.display() })
            }
            _ => Err(format!("unsupported binary operator: {:?}", op)),
        }
    }

    fn eval_add(&self, l: &Value, r: &Value) -> Result<Value, String> {
        match (l, r) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
            (Value::Float(a), Value::Int(b)) => {
                // Convert BigInt to f64 using string parsing
                let b_f64 = b.to_string().parse::<f64>().unwrap_or(0.0);
                Ok(Value::Float(a + b_f64))
            },
            (Value::Int(a), Value::Float(b)) => Ok(Value::Float(b.clone())),
            _ => Err(format!("cannot add {:?} and {:?}", std::mem::discriminant(l), std::mem::discriminant(r))),
        }
    }

    fn eval_sub(&self, l: &Value, r: &Value) -> Result<Value, String> {
        match (l, r) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
            _ => Err("cannot subtract".into()),
        }
    }

    fn eval_mul(&self, l: &Value, r: &Value) -> Result<Value, String> {
        match (l, r) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
            (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
            _ => Err("cannot multiply".into()),
        }
    }

    /// Call a function declaration.
    fn call_function(&mut self, decl: &Declaration, args: &[Expression]) -> Result<Value, String> {
        match decl {
            Declaration::FunctionDef { name, body, params, .. } => {
                let fn_name = self.interner.lookup(*name).to_string();

                // Handle built-in functions inline, user-defined functions below
                if fn_name == "print" || fn_name == "dump" || fn_name == "exit"
                    || fn_name == "H" || fn_name == "X" || fn_name == "Y" || fn_name == "Z"
                    || fn_name == "phase" || fn_name == "CNOT"
                    || fn_name == "measure" || fn_name == "dup"
                    || fn_name == "rotX" || fn_name == "rotY" || fn_name == "rotZ" {
                    self.call_builtin(&fn_name, args)
                } else {
                    // Evaluate arguments
                    let mut arg_vals = Vec::new();
                    for arg in args {
                        arg_vals.push(self.eval(arg)?);
                    }

                    // Bind parameters
                    for (i, param) in params.iter().enumerate() {
                        if i < arg_vals.len() {
                            if let Declaration::VarDecl { name: pname, .. } = param {
                                let val = &arg_vals[i];
                                match val {
                                    Value::QVar { index, .. } => {
                                        // Keep the quantum variable reference
                                        self.state.variables.insert(pname.0, (*index, 1));
                                    }
                                    _ => {
                                        self.state.store_classical(*pname, val.clone());
                                    }
                                }
                            }
                        }
                    }

                    // Execute body
                    let result = self.eval(body);

                    // Clean up parameter bindings (leave the function)
                    for param in params {
                        if let Declaration::VarDecl { name: pname, .. } = param {
                            self.state.classical_vars.remove(&pname.0);
                            self.state.variables.remove(&pname.0);
                        }
                    }

                    result
                }
            }
            _ => Err("not a function".into()),
        }
    }

    /// Call a built-in function by name.
    fn call_builtin(&mut self, name: &str, args: &[Expression]) -> Result<Value, String> {
        match name {
            "print" => {
                for arg in args {
                    let val = self.eval(arg)?;
                    print!("{}", val.display());
                }
                println!();
                Ok(Value::Unit)
            }
            "dump" => {
                println!("{}", self.state.dump());
                Ok(Value::Unit)
            }
            "exit" => {
                std::process::exit(0);
            }

            "H" | "X" | "Y" | "Z" if args.len() == 1 => {
                let arg = self.eval(&args[0])?;
                if let Value::QVar { index, .. } = arg {
                    match name {
                        "H" => self.state.apply_1q_gate(index, hadamard_gate()),
                        "X" => self.state.apply_1q_gate(index, pauli_x_gate()),
                        "Y" => self.state.apply_1q_gate(index, pauli_y_gate()),
                        "Z" => self.state.apply_1q_gate(index, pauli_z_gate()),
                        _ => unreachable!(),
                    }
                    Ok(Value::Unit)
                } else {
                    Err(format!("{} requires a qubit argument", name))
                }
            }

            "phase" if args.len() == 1 => {
                let phi = self.eval(&args[0])?;
                if let Some(phi_f) = phi.as_float() {
                    // Phase is applied globally, not to a specific qubit in Silq's semantics
                    // Apply to all qubits evenly
                    let gate = phase_gate(phi_f);
                    for i in 0..self.state.num_qubits {
                        self.state.apply_1q_gate(i, gate);
                    }
                    Ok(Value::Unit)
                } else {
                    Err("phase requires a real angle argument".into())
                }
            }

            "CNOT" if args.len() == 2 => {
                let ctl = self.eval(&args[0])?;
                let tgt = self.eval(&args[1])?;
                if let (Value::QVar { index: c, .. }, Value::QVar { index: t, .. }) = (&ctl, &tgt) {
                    self.state.apply_cnot(*c, *t);
                    Ok(Value::Unit)
                } else {
                    Err("CNOT requires two qubit arguments".into())
                }
            }

            "measure" if args.len() == 1 => {
                let arg = self.eval(&args[0])?;
                if let Value::QVar { index, name } = arg {
                    let result = self.state.measure(index);
                    let measured_val = Value::Bool { val: result == 1 };
                    self.state.store_classical(name, measured_val.clone());
                    Ok(measured_val)
                } else {
                    Err("measure requires a qubit argument".into())
                }
            }

            "dup" if args.len() == 1 => {
                // Classical duplication
                let val = self.eval(&args[0])?;
                if val.is_classical() {
                    Ok(val)
                } else {
                    Err("cannot duplicate quantum value".into())
                }
            }

            "rotX" if args.len() == 2 => {
                let theta = self.eval(&args[0])?;
                let q_arg = self.eval(&args[1])?;
                if let (Some(t), Value::QVar { index, .. }) = (theta.as_float(), q_arg) {
                    self.state.apply_1q_gate(index, rot_x_gate(t));
                    Ok(Value::Unit)
                } else {
                    Err("rotX requires (angle, qubit)".into())
                }
            }

            "rotY" if args.len() == 2 => {
                let theta = self.eval(&args[0])?;
                let q_arg = self.eval(&args[1])?;
                if let (Some(t), Value::QVar { index, .. }) = (theta.as_float(), q_arg) {
                    self.state.apply_1q_gate(index, rot_y_gate(t));
                    Ok(Value::Unit)
                } else {
                    Err("rotY requires (angle, qubit)".into())
                }
            }

            "rotZ" if args.len() == 2 => {
                let theta = self.eval(&args[0])?;
                let q_arg = self.eval(&args[1])?;
                if let (Some(t), Value::QVar { index, .. }) = (theta.as_float(), q_arg) {
                    self.state.apply_1q_gate(index, rot_z_gate(t));
                    Ok(Value::Unit)
                } else {
                    Err("rotZ requires (angle, qubit)".into())
                }
            }

            _ => {
                Err(format!("unknown built-in function: {}", name))
            }
        }
    }

    /// Evaluate a for loop.
    fn eval_for_loop(&mut self, _variable: Id, iterable: &Expression, body: &Expression) -> Result<Value, String> {
        let iter = self.eval(iterable)?;
        match iter {
            Value::Array(elems) => {
                let mut last = Value::Unit;
                for elem in elems {
                    self.state.store_classical(_variable, elem);
                    last = self.eval(body)?;
                }
                Ok(last)
            }
            Value::Tuple(elems) => {
                let mut last = Value::Unit;
                for elem in elems {
                    self.state.store_classical(_variable, elem);
                    last = self.eval(body)?;
                }
                Ok(last)
            }
            Value::Int(n) => {
                let n: i64 = n.try_into().map_err(|_| "range too large".to_string())?;
                let mut last = Value::Unit;
                for i in 0..n {
                    self.state.store_classical(_variable, Value::Int(num_bigint::BigInt::from(i)));
                    last = self.eval(body)?;
                }
                Ok(last)
            }
            _ => Err("for loop requires iterable argument".into()),
        }
    }

    /// Evaluate a while loop.
    fn eval_while_loop(&mut self, condition: &Expression, body: &Expression) -> Result<Value, String> {
        let mut last = Value::Unit;
        loop {
            let cond = self.eval(condition)?;
            if cond.as_bool() != Some(true) {
                break;
            }
            last = self.eval(body)?;
        }
        Ok(last)
    }

    /// Evaluate a `with` block (quantum-controlled execution).
    fn eval_with(&mut self, controller: &Expression, body: &Expression) -> Result<Value, String> {
        // In a real Silq implementation, the `with` block would apply
        // controlled versions of the operations in the body.
        // For now, execute the body directly.
        // TODO: Implement proper quantum control flow
        let _ctl = self.eval(controller)?;
        self.eval(body)
    }

    /// Evaluate a `forget` expression (release a quantum variable).
    fn eval_forget(&mut self, variable: &Expression) -> Result<Value, String> {
        if let Expression::Identifier { name, .. } = variable {
            // Remove the variable from variable bindings
            // Note: In a real Silq implementation, this would involve
            // uncomputation to ensure the qubit is in |0⟩ before releasing
            self.state.variables.remove(&name.0);
            eprintln!("[warning] forget({}) - qubit released (uncomputation not implemented)",
                self.interner.lookup(*name));
            Ok(Value::Unit)
        } else {
            Err("forget requires a variable identifier".into())
        }
    }
}

// =============================================================================
// QSim - Top-level Simulator Interface
// =============================================================================

/// The QSim struct is the main interface to the quantum simulator.
pub struct QSim {
    /// The interpreter.
    pub interpreter: Interpreter,
    /// Error handler callback.
    error_handler: Option<Box<dyn FnMut(&str)>>,
}

impl QSim {
    pub fn new(interner: Interner) -> Self {
        QSim {
            interpreter: Interpreter::new(interner),
            error_handler: None,
        }
    }

    /// Set the error handler.
    pub fn set_error_handler(&mut self, handler: Box<dyn FnMut(&str)>) {
        self.error_handler = Some(handler);
    }

    /// Enable/disable execution tracing.
    pub fn set_trace(&mut self, trace: bool) {
        self.interpreter.trace = trace;
    }

    /// Register a function definition.
    pub fn register_function(&mut self, decl: &Declaration) {
        self.interpreter.register_function(decl);
    }

    /// Run a program (list of top-level expression AST nodes).
    pub fn run(&mut self, program: &[Expression]) -> Result<Value, String> {
        self.interpreter.run_program(program)
    }

    /// Dump the current quantum state.
    pub fn dump_state(&self) -> String {
        self.interpreter.state.dump()
    }
}
