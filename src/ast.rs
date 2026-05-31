//! AST (Abstract Syntax Tree) definitions for Silq.
//!
//! This module defines all node types in the Silq AST hierarchy:
//! - Expressions (the core node type)
//! - Declarations (variable, function, data type declarations)
//! - Types (numeric, product, tuple, array, vector, etc.)
//! - Identifiers (interned string names)

use crate::errors::Location;
use crate::token::TokenType;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

// =============================================================================
// Semantic State
// =============================================================================

/// The semantic processing state of an AST node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemState {
    /// Initial state, before any semantic analysis.
    Initial,
    /// Semantic analysis has started for this node.
    Started,
    /// Passive state (in the middle of analysis).
    Passive,
    /// Semantic analysis completed successfully.
    Completed,
    /// The expression has been evaluated (constant fold).
    Evaluated,
    /// An error occurred during semantic analysis.
    Error,
}

impl Default for SemState {
    fn default() -> Self { SemState::Initial }
}

// =============================================================================
// String Interning for Identifiers
// =============================================================================

/// A unique ID counter for generating fresh names.
static ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// An interned identifier. In the D version this is a pointer, in Rust
/// we use an index into a global interning table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Id(pub usize);

impl Id {
    /// Create a fresh unique Id.
    pub fn fresh() -> Self {
        Id(ID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

/// Global string interner for identifiers.
pub struct Interner {
    strings: Vec<String>,
    map: HashMap<String, usize>,
}

impl Interner {
    pub fn new() -> Self {
        let mut interner = Interner {
            strings: Vec::new(),
            map: HashMap::new(),
        };
        // Reserve 0 for empty/invalid
        interner.strings.push(String::new());
        interner.map.insert(String::new(), 0);
        interner
    }

    /// Intern a string and return its Id.
    pub fn intern(&mut self, s: &str) -> Id {
        if let Some(&id) = self.map.get(s) {
            return Id(id);
        }
        let id = self.strings.len();
        self.strings.push(s.to_string());
        self.map.insert(s.to_string(), id);
        Id(id)
    }

    /// Look up the string for an Id.
    pub fn lookup(&self, id: Id) -> &str {
        self.strings.get(id.0).map(|s| s.as_str()).unwrap_or("")
    }
}

// =============================================================================
// Base AST Node
// =============================================================================

/// Base trait for all AST nodes.
pub trait Node {
    fn location(&self) -> &Location;
    fn semantic_state(&self) -> SemState;
    fn set_semantic_state(&mut self, state: SemState);
}

// =============================================================================
// Types
// =============================================================================

/// Annotation for function parameters (const, moved, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureAnnotation {
    None,
    Const,
    Moved,
    Once,
    Spent,
}

impl Default for CaptureAnnotation {
    fn default() -> Self { CaptureAnnotation::None }
}

/// Function-level annotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Annotation {
    None,
    Mfree,    // measurement-free
    Qfree,    // quantum-free
    Lifted,   // lifted (classical re-interpretation)
    Wild,     // wildcard (anything goes)
}

impl Default for Annotation {
    fn default() -> Self { Annotation::None }
}

/// Silq numeric type hierarchy: Bool <: N <: Z <: Q <: R <: C
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NumericType {
    Bool,       // Boolean
    Nat,        // Natural numbers (N)
    Int,        // Integers (Z)
    Rat,        // Rational numbers (Q)
    Real,       // Real numbers (R)
    Complex,    // Complex numbers (C)
}

impl NumericType {
    /// Is this numeric type a subtype of `other`?
    pub fn is_subtype_of(self, other: NumericType) -> bool {
        self <= other
    }

    /// The name of this numeric type in Silq.
    pub fn type_name(self) -> &'static str {
        match self {
            NumericType::Bool => "𝔹",
            NumericType::Nat => "ℕ",
            NumericType::Int => "ℤ",
            NumericType::Rat => "ℚ",
            NumericType::Real => "ℝ",
            NumericType::Complex => "ℂ",
        }
    }

    /// ASCII alias for the type name.
    pub fn ascii_name(self) -> &'static str {
        match self {
            NumericType::Bool => "B",
            NumericType::Nat => "N",
            NumericType::Int => "Z",
            NumericType::Rat => "Q",
            NumericType::Real => "R",
            NumericType::Complex => "C",
        }
    }
}

/// A fixed-width integer type: int[n] or uint[n].
#[derive(Debug, Clone, PartialEq)]
pub struct FixedIntTy {
    pub bits: u32,
    pub is_signed: bool,
    pub is_classical: bool,
}

/// A modular integer type: Zmod[N] or Zstar[N].
#[derive(Debug, Clone, PartialEq)]
pub struct ZModTy {
    pub n: u64,
    pub is_star: bool,      // Zstar (multiplicative group)
    pub is_classical: bool,
}

// Forward declarations
pub type ExprPtr = Box<Expression>;
pub type ExprList = Vec<Expression>;

/// All type variants in the Silq type system.
/// Note: In Silq, types ARE expressions (dependent types), so Type is an enum variant of Expression.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    /// One of the base numeric types: B, N, Z, Q, R, C.
    Numeric(NumericType),
    /// Fixed-width integer: int[n], uint[n].
    FixedInt(FixedIntTy),
    /// Modular integer: Zmod[N], Zstar[N].
    ZMod(ZModTy),
    /// A custom data type (declared with `dat`).
    Aggregate {
        name: Id,
        type_args: Vec<Expression>,
    },
    /// The unit type (similar to void/() in other languages).
    Unit,
    /// The bottom type (empty/never).
    Bottom,
    /// A tuple type: (T1, T2, ..., Tn).
    Tuple(Vec<Expression>),
    /// An array type (variable-length): T[].
    Array(ExprPtr),
    /// A vector type (fixed-length): T^n.
    Vector {
        element: ExprPtr,
        length: ExprPtr,
    },
    /// A string type.
    String,
    /// A function type: (params) -> return_type.
    /// Also used for dependent product types (Pi types).
    Product {
        params: Vec<Expression>,
        domain: ExprPtr,
        codomain: ExprPtr,
        annotation: Annotation,
    },
    /// Classical version of a type: !T.
    Classical(ExprPtr),
    /// Quantum numeric type (supertype of all quantum numerics).
    QNumeric,
    /// A type variable (used during inference).
    TypeVar(usize),
    /// The type of types (kind).
    TypeMeta {
        variant: TypeMetaKind,
    },
    /// A context type (implicit closure environment).
    Context(Vec<(Id, Expression)>),
}

/// Kinds in the kind system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeMetaKind {
    /// The universal kind: *
    Star,
    /// Classical type kind.
    Ctype,
    /// Quantum type kind.
    Qtype,
    /// Expression type kind.
    Etype,
    /// Unit type kind.
    Utype,
}

impl TypeKind {
    /// Check if this type is classical.
    pub fn is_classical(&self) -> bool {
        match self {
            TypeKind::Classical(_) => true,
            TypeKind::TypeMeta { variant: TypeMetaKind::Ctype } => true,
            TypeKind::Unit => true,
            // Numeric types are classical unless marked otherwise
            TypeKind::Numeric(_) => false, // In Silq, B is quantum, !B is classical
            TypeKind::Aggregate { .. } => false,
            TypeKind::FixedInt(fit) => fit.is_classical,
            TypeKind::ZMod(zm) => zm.is_classical,
            _ => false,
        }
    }

    /// Check if this type is quantum (non-classical).
    pub fn is_quantum(&self) -> bool {
        !self.is_classical()
    }

    /// Create a classical version of a numeric type.
    pub fn make_classical(&self) -> Option<TypeKind> {
        match self {
            TypeKind::Numeric(nt) => Some(TypeKind::Numeric(*nt)),
            _ => None,
        }
    }
}

// =============================================================================
// Expression Nodes
// =============================================================================

/// The main Expression type - all nodes in the Silq AST are expressions.
///
/// Silq is an expression-oriented language: even statements like return and if
/// are expressions.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// An error expression (produced when parsing fails).
    Error {
        loc: Location,
        message: String,
    },

    /// A literal value: integer, float, rational, string, boolean.
    Literal {
        loc: Location,
        value: LiteralValue,
        /// The inferred/computed type of this literal.
        ty: Option<Box<Expression>>,
    },

    /// A variable or type identifier reference.
    Identifier {
        loc: Location,
        name: Id,
        /// What this identifier refers to (resolved during semantic analysis).
        meaning: Option<Box<Declaration>>,
        /// Whether this is in a classical context.
        classical: bool,
    },

    /// A placeholder / hole (used during parsing/inference).
    Placeholder {
        loc: Location,
    },

    /// A wildcard expression `_`.
    Wildcard {
        loc: Location,
    },

    /// The `typeof` operator.
    Typeof {
        loc: Location,
        expr: ExprPtr,
    },

    // ---- Unary expressions ----

    /// Unary plus: +e.
    UnaryPlus {
        loc: Location,
        expr: ExprPtr,
    },
    /// Unary minus (negation): -e.
    UnaryMinus {
        loc: Location,
        expr: ExprPtr,
    },
    /// Logical not: !e.
    LogicalNot {
        loc: Location,
        expr: ExprPtr,
    },
    /// Bitwise not: ~e.
    BitwiseNot {
        loc: Location,
        expr: ExprPtr,
    },

    // ---- Binary expressions ----

    /// A binary operation: e1 op e2.
    Binary {
        loc: Location,
        op: TokenType,
        left: ExprPtr,
        right: ExprPtr,
    },

    // ---- Call expressions ----

    /// Function call: f(args).
    Call {
        loc: Location,
        function: ExprPtr,
        arguments: Vec<Expression>,
        /// Resolved function definition.
        callee: Option<Box<Declaration>>,
    },

    /// Index expression: e[i].
    Index {
        loc: Location,
        expr: ExprPtr,
        index: ExprPtr,
    },

    /// Slice expression: e[start..end].
    Slice {
        loc: Location,
        expr: ExprPtr,
        start: ExprPtr,
        end: ExprPtr,
    },

    /// Field/projection access: e.field.
    Field {
        loc: Location,
        expr: ExprPtr,
        field: Id,
    },

    // ---- Composite expressions ----

    /// Tuple literal: (e1, e2, ..., en).
    Tuple {
        loc: Location,
        elements: Vec<Expression>,
    },

    /// Vector literal.
    Vector {
        loc: Location,
        elements: Vec<Expression>,
    },

    /// Concatenation: e1 ~ e2.
    Concat {
        loc: Location,
        left: ExprPtr,
        right: ExprPtr,
    },

    // ---- Control flow ----

    /// let-binding: `let name = value in body` or `name := value`
    Let {
        loc: Location,
        name: Id,
        /// Optional type annotation.
        type_ann: Option<ExprPtr>,
        value: ExprPtr,
        body: ExprPtr,
    },

    /// Assignment: `var ← value`.
    Assign {
        loc: Location,
        target: ExprPtr,
        value: ExprPtr,
    },

    /// Lambda expression: `λ(params) => body`.
    Lambda {
        loc: Location,
        params: Vec<Expression>,
        body: ExprPtr,
        annotation: Annotation,
    },

    /// If-then-else: `if cond then then_br else else_br`.
    IfThenElse {
        loc: Location,
        condition: ExprPtr,
        then_branch: ExprPtr,
        else_branch: Option<ExprPtr>,
    },

    /// With block (quantum-controlled execution): `with ctl do body`.
    With {
        loc: Location,
        controller: ExprPtr,
        body: ExprPtr,
    },

    /// For loop: `for var in expr { body }`.
    ForLoop {
        loc: Location,
        variable: Id,
        iterable: ExprPtr,
        body: ExprPtr,
    },

    /// While loop: `while cond { body }`.
    WhileLoop {
        loc: Location,
        condition: ExprPtr,
        body: ExprPtr,
    },

    /// Repeat loop: `repeat n { body }`.
    Repeat {
        loc: Location,
        count: ExprPtr,
        body: ExprPtr,
    },

    // ---- Other expressions ----

    /// Return expression: `return expr`.
    Return {
        loc: Location,
        expr: Option<ExprPtr>,
    },

    /// Comma expression: `e1, e2` (used for parameter tupling).
    Comma {
        loc: Location,
        left: ExprPtr,
        right: ExprPtr,
    },

    /// A compound expression block: `{ e1; e2; ... eN }`.
    Compound {
        loc: Location,
        statements: Vec<Expression>,
    },

    /// Type annotation: `e : T` or `e as T` or `e coerce T` or `e pun T`.
    TypeAnnotation {
        loc: Location,
        expr: ExprPtr,
        ty: ExprPtr,
        kind: TypeAnnotationKind,
    },

    /// Forget (consume) a variable: `forget(var)`.
    Forget {
        loc: Location,
        variable: ExprPtr,
    },

    /// Assert expression: `assert(cond)`.
    Assert {
        loc: Location,
        condition: ExprPtr,
        message: Option<String>,
    },

    // ---- Type-level expressions ----

    /// A type expression: `T` used as a type.
    Type { loc: Location, kind: TypeKind },

    /// A type imported from a declaration.
    TypeDecl(Box<Declaration>),

    /// Type variable reference.
    TypeVar {
        loc: Location,
        index: usize,
    },
}

impl Expression {
    /// Get the location of this expression.
    pub fn loc(&self) -> &Location {
        match self {
            Expression::Error { ref loc, .. } => loc,
            Expression::Literal { ref loc, .. } => loc,
            Expression::Identifier { ref loc, .. } => loc,
            Expression::Placeholder { ref loc } => loc,
            Expression::Wildcard { ref loc } => loc,
            Expression::Typeof { ref loc, .. } => loc,
            Expression::UnaryPlus { ref loc, .. } => loc,
            Expression::UnaryMinus { ref loc, .. } => loc,
            Expression::LogicalNot { ref loc, .. } => loc,
            Expression::BitwiseNot { ref loc, .. } => loc,
            Expression::Binary { ref loc, .. } => loc,
            Expression::Call { ref loc, .. } => loc,
            Expression::Index { ref loc, .. } => loc,
            Expression::Slice { ref loc, .. } => loc,
            Expression::Field { ref loc, .. } => loc,
            Expression::Tuple { ref loc, .. } => loc,
            Expression::Vector { ref loc, .. } => loc,
            Expression::Concat { ref loc, .. } => loc,
            Expression::Let { ref loc, .. } => loc,
            Expression::Assign { ref loc, .. } => loc,
            Expression::Lambda { ref loc, .. } => loc,
            Expression::IfThenElse { ref loc, .. } => loc,
            Expression::With { ref loc, .. } => loc,
            Expression::ForLoop { ref loc, .. } => loc,
            Expression::WhileLoop { ref loc, .. } => loc,
            Expression::Repeat { ref loc, .. } => loc,
            Expression::Return { ref loc, .. } => loc,
            Expression::Comma { ref loc, .. } => loc,
            Expression::Compound { ref loc, .. } => loc,
            Expression::TypeAnnotation { ref loc, .. } => loc,
            Expression::Forget { ref loc, .. } => loc,
            Expression::Assert { ref loc, .. } => loc,
            Expression::Type { ref loc, .. } => loc,
            Expression::TypeDecl(d) => d.loc(),
            Expression::TypeVar { ref loc, .. } => loc,
        }
    }

    /// Check if this is a literal expression.
    pub fn is_literal(&self) -> bool {
        matches!(self, Expression::Literal { .. })
    }

    /// Check if this is an error expression.
    pub fn is_error(&self) -> bool {
        matches!(self, Expression::Error { .. })
    }

    /// Create a new error expression.
    pub fn new_error(loc: Location, msg: impl Into<String>) -> Self {
        Expression::Error { loc, message: msg.into() }
    }

    /// Create a new literal expression.
    pub fn new_literal(loc: Location, value: LiteralValue) -> Self {
        Expression::Literal { loc, value, ty: None }
    }

    /// Create a new identifier expression.
    pub fn new_identifier(loc: Location, name: Id) -> Self {
        Expression::Identifier { loc, name, meaning: None, classical: false }
    }

    /// Create a new binary expression.
    pub fn new_binary(loc: Location, op: TokenType, left: Expression, right: Expression) -> Self {
        Expression::Binary { loc, op, left: Box::new(left), right: Box::new(right) }
    }

    /// Create a new call expression.
    pub fn new_call(loc: Location, function: Expression, arguments: Vec<Expression>) -> Self {
        Expression::Call { loc, function: Box::new(function), arguments, callee: None }
    }

    /// Create a new compound expression.
    pub fn new_compound(loc: Location, statements: Vec<Expression>) -> Self {
        Expression::Compound { loc, statements }
    }

    /// Create a new let expression.
    pub fn new_let(loc: Location, name: Id, type_ann: Option<Expression>, value: Expression, body: Expression) -> Self {
        Expression::Let {
            loc,
            name,
            type_ann: type_ann.map(Box::new),
            value: Box::new(value),
            body: Box::new(body),
        }
    }

    /// Create a new if-then-else expression.
    pub fn new_if(loc: Location, cond: Expression, then_br: Expression, else_br: Option<Expression>) -> Self {
        Expression::IfThenElse {
            loc,
            condition: Box::new(cond),
            then_branch: Box::new(then_br),
            else_branch: else_br.map(Box::new),
        }
    }

    /// Create a new lambda expression.
    pub fn new_lambda(loc: Location, params: Vec<Expression>, body: Expression, annotation: Annotation) -> Self {
        Expression::Lambda { loc, params, body: Box::new(body), annotation }
    }

    /// Create a type annotation expression.
    pub fn new_type_ann(loc: Location, expr: Expression, ty: Expression, kind: TypeAnnotationKind) -> Self {
        Expression::TypeAnnotation { loc, expr: Box::new(expr), ty: Box::new(ty), kind }
    }

    /// Create a unit type expression.
    pub fn unit_type() -> Self {
        Expression::Type { loc: Location::default(), kind: TypeKind::Unit }
    }
}

impl Default for Expression {
    fn default() -> Self {
        Expression::Error {
            loc: Location::default(),
            message: "uninitialized expression".into(),
        }
    }
}

// =============================================================================
// Literal Values
// =============================================================================

/// All literal value types in Silq.
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    /// Boolean literal: true/false.
    Bool(bool),
    /// Integer literal (big integer).
    Int(num_bigint::BigInt),
    /// Floating-point literal.
    Float(f64),
    /// Rational literal: rational(num, den).
    Rational(num_bigint::BigInt, num_bigint::BigInt),
    /// String literal.
    String(String),
    /// Character literal.
    Char(char),
    /// Unit literal.
    Unit,
}

impl std::fmt::Display for LiteralValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LiteralValue::Bool(b) => write!(f, "{}", b),
            LiteralValue::Int(n) => write!(f, "{}", n),
            LiteralValue::Float(x) => write!(f, "{}", x),
            LiteralValue::Rational(n, d) => write!(f, "({}/{})", n, d),
            LiteralValue::String(s) => write!(f, "\"{}\"", s),
            LiteralValue::Char(c) => write!(f, "'{}'", c),
            LiteralValue::Unit => write!(f, "()"),
        }
    }
}

// =============================================================================
// Type Annotation Kinds
// =============================================================================

/// The kind of type annotation: e:T, e as T, e coerce T, e pun T.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeAnnotationKind {
    /// Type assertion: e : T (check that e has type T).
    Colon,
    /// Type cast: e as T.
    As,
    /// Type coercion: e coerce T (implicit conversion).
    Coerce,
    /// Type pun: e pun T (reinterpret bits).
    Pun,
}

// =============================================================================
// Declaration Nodes
// =============================================================================

/// A declaration in the Silq language.
/// Declarations are also expressions (they evaluate to their declared name).
#[derive(Debug, Clone, PartialEq)]
pub enum Declaration {
    /// Variable declaration: name: Type := initializer.
    VarDecl {
        loc: Location,
        name: Id,
        /// Declared type (optional, can be inferred).
        dtype: Option<Box<Expression>>,
        /// Variable type (set during semantic analysis).
        vtype: Option<Box<Expression>>,
        /// Initializer expression.
        initializer: Option<Box<Expression>>,
        /// Whether this is a parameter declaration.
        is_parameter: bool,
        /// Parameter capture annotation.
        capture: CaptureAnnotation,
    },

    /// Function definition: def name(params) return_type body.
    FunctionDef {
        loc: Location,
        name: Id,
        /// Parameters.
        params: Vec<Declaration>,
        /// Return type expression.
        return_type: Option<Box<Expression>>,
        /// The function body.
        body: Box<Expression>,
        /// Annotation (qfree, mfree, lifted, wild).
        annotation: Annotation,
        /// Resolved function type.
        ftype: Option<Box<Expression>>,
        /// Captured variables from enclosing scope.
        captures: Vec<Id>,
        /// Whether this is the main function.
        is_main: bool,
        /// External name for primitive implementations.
        extern_name: Option<String>,
    },

    /// Data type declaration: dat Name[params] { fields }.
    DatDecl {
        loc: Location,
        name: Id,
        /// Type parameters.
        type_params: Vec<Declaration>,
        /// Fields of the data type.
        fields: Vec<Declaration>,
        /// Whether this is a quantum data type.
        is_quantum: bool,
    },

    /// Import declaration: import path.
    Import {
        loc: Location,
        path: String,
    },
}

impl Declaration {
    /// Get the location of this declaration.
    pub fn loc(&self) -> &Location {
        match self {
            Declaration::VarDecl { ref loc, .. } => loc,
            Declaration::FunctionDef { ref loc, .. } => loc,
            Declaration::DatDecl { ref loc, .. } => loc,
            Declaration::Import { ref loc, .. } => loc,
        }
    }

    /// Get the name of this declaration.
    pub fn name(&self) -> Option<Id> {
        match self {
            Declaration::VarDecl { name, .. } => Some(*name),
            Declaration::FunctionDef { name, .. } => Some(*name),
            Declaration::DatDecl { name, .. } => Some(*name),
            Declaration::Import { .. } => None,
        }
    }

    /// Create a new variable declaration.
    pub fn new_var(loc: Location, name: Id, dtype: Option<Expression>,
                   init: Option<Expression>, is_param: bool, cap: CaptureAnnotation) -> Self {
        Declaration::VarDecl {
            loc,
            name,
            dtype: dtype.map(Box::new),
            vtype: None,
            initializer: init.map(Box::new),
            is_parameter: is_param,
            capture: cap,
        }
    }

    /// Create a new function definition.
    pub fn new_function(loc: Location, name: Id, params: Vec<Declaration>,
                        return_type: Option<Expression>, body: Expression,
                        annotation: Annotation) -> Self {
        Declaration::FunctionDef {
            loc,
            name,
            params,
            return_type: return_type.map(Box::new),
            body: Box::new(body),
            annotation,
            ftype: None,
            captures: vec![],
            is_main: false,
            extern_name: None,
        }
    }

    /// Create a new data type declaration.
    pub fn new_dat(loc: Location, name: Id, type_params: Vec<Declaration>,
                   fields: Vec<Declaration>, is_quantum: bool) -> Self {
        Declaration::DatDecl { loc, name, type_params, fields, is_quantum }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Create a numeric type expression.
pub fn numeric_type(nt: NumericType) -> Expression {
    Expression::Type { loc: Location::default(), kind: TypeKind::Numeric(nt) }
}

/// Create a classical type expression (prefix `!`).
pub fn classical_type(inner: Expression) -> Expression {
    Expression::Type { loc: Location::default(), kind: TypeKind::Classical(Box::new(inner)) }
}

/// Create a type meta expression (kind).
pub fn type_meta(variant: TypeMetaKind) -> Expression {
    Expression::Type { loc: Location::default(), kind: TypeKind::TypeMeta { variant } }
}

/// Create a tuple type expression.
pub fn tuple_type(elements: Vec<Expression>) -> Expression {
    Expression::Type { loc: Location::default(), kind: TypeKind::Tuple(elements) }
}

/// Create a function type expression.
pub fn fun_type(domain: Expression, codomain: Expression, annotation: Annotation) -> Expression {
    Expression::Type { loc: Location::default(), kind: TypeKind::Product {
        params: vec![],
        domain: Box::new(domain),
        codomain: Box::new(codomain),
        annotation,
    }}
}

/// Create a vector type expression.
pub fn vector_type(element: Expression, length: Expression) -> Expression {
    Expression::Type { loc: Location::default(), kind: TypeKind::Vector {
        element: Box::new(element),
        length: Box::new(length),
    }}
}

/// Create an array type expression.
pub fn array_type(element: Expression) -> Expression {
    Expression::Type { loc: Location::default(), kind: TypeKind::Array(Box::new(element)) }
}

/// Create a fixed integer type.
pub fn fixed_int_type(bits: u32, is_signed: bool, is_classical: bool) -> Expression {
    Expression::Type { loc: Location::default(), kind: TypeKind::FixedInt(FixedIntTy { bits, is_signed, is_classical }) }
}

/// Create a Zmod type.
pub fn zmod_type(n: u64, is_star: bool, is_classical: bool) -> Expression {
    Expression::Type { loc: Location::default(), kind: TypeKind::ZMod(ZModTy { n, is_star, is_classical }) }
}
