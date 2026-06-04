# CLAUDE.md

This file provides guidance to Claude Code (claude.ai) when working with code in this repository.

## Project Overview

**silq-rs** is a Rust reimplementation of the Silq quantum programming language compiler. Silq is a high-level quantum programming language with automatic uncomputation and a linear type system, originally written in D (~55K lines). This Rust rewrite provides the same compilation pipeline with type safety and no garbage collector.

- **Language:** Rust 2021 edition
- **Test framework:** `cargo test`
- **Binary name:** `silq`
- **License:** BSL-1.0

## Build & Test Commands

```bash
cargo build              # Debug build (native)
cargo build --release    # Release build (native)
cargo test               # Run all tests (11 unit tests)
cargo test lexer         # Run specific test module
cargo test test_parse    # Run tests matching pattern
cargo fmt                # Format code
cargo clippy             # Lint (should be 0 warnings)
cargo run -- --help      # Run binary with args

# Cross-compilation
cargo build --release --target x86_64-pc-windows-msvc                        # Windows .exe (681 KB) + .dll (104 KB)
cargo build --release --target wasm32-unknown-unknown --no-default-features  # WASM .rlib (1.2 MB, for Rust lib use)
cargo build --release --target wasm32-unknown-unknown --no-default-features --features wasm  # WASM .wasm (352 KB, JS exports)
```

**Clippy status: 0 warnings.** All clippy warnings have been resolved across the codebase.

## Project Structure

```
src/
├── main.rs          # CLI entry point
├── lib.rs           # Library root, re-exports, WASM feature gate
├── token.rs         # Token types, keywords, operator precedence tables
├── lexer.rs         # UTF-8 lexer, Unicode math symbols, nested comments
├── parser.rs        # Pratt precedence-climbing recursive descent parser
├── ast.rs           # AST (Expression 35 variants, Declaration 4, TypeKind 16, LiteralValue 7)
├── scope.rs         # Nested symbol table with name interning
├── semantic.rs      # Name resolution and scope management
├── checker.rs       # Linear type resource checker (const/moved tracking)
├── consteval.rs     # Compile-time constant expression evaluation (including UnaryPlus)
├── conversion.rs    # Type classicality judgment (ℕ/ℤ/ℚ/ℝ inherent, 𝔹/ℂ by wrapper)
├── reverse.rs       # Automatic adjoint/uncomputation transformation
├── modules.rs       # Module import system, prelude loading via include_str!
├── errors.rs        # Error diagnostics (terminal + JSON), color conditional on `colored` feature
├── qsim.rs          # Quantum simulator: QState, Interpreter, gates, phase (global)
├── hqir.rs          # HQIR (High-level Quantum IR) backend stub
├── options.rs       # Compiler configuration (Language::Silq|Psi, Options struct)
└── wasm.rs          # WASM bindings (wasm32 only): run_silq, parse_silq, tokenize_silq
```

## Compilation Pipeline

```
Source (.slq) → Lexer → Parser → Semantic → Checker → Backend (QSim/HQIR)
```

## Key Design Decisions

### AST: Expression = Type

In Silq, types are expressions (dependent types). The Rust AST uses a single `Expression` enum where `Expression::Type { loc, kind: TypeKind }` represents type-level expressions. `TypeKind` has 16 variants (Numeric, FixedInt, ZMod, Aggregate, Unit, Bottom, Tuple, Array, Vector, String, Product, Classical, QNumeric, TypeVar, TypeMeta, Context).

### Identifier Interning

`Interner` maps strings to `Id(usize)` for O(1) comparison. All identifiers and field names use interned IDs.

### Pratt Parsing

`get_lbp(token_type)` returns left binding power. Precedence ranges from 10 (comma) to 160 (postfix). The parser cycles through `nud()` (prefix expressions) and `infix()` (binary/postfix).

### Quantum Simulation

`QState` uses sparse `BTreeMap<BasisState, Complex64>` for state vectors. `alloc_qubit()` initializes the state vector with `|0⟩` amplitude via tensor product. Gates are applied as 2×2 unitary matrices (`apply_1q_gate`). `phase()` applies global phase via `apply_global_phase(phi)` — multiplying all amplitudes by e^(i*phi). Measurement uses `fastrand` for true random number generation and collapses with probability normalization.

### Type System: Classical vs Quantum

Per Silq spec: `ℕ, ℤ, ℚ, ℝ` are **inherently classical** (no `!` prefix needed). `𝔹` and `ℂ` are **quantum by default** — `!𝔹` / `!ℂ` makes them classical. This is enforced in both `TypeKind::is_classical()` (ast.rs) and `is_classical_type()` (conversion.rs).

### Interner Sharing

`Interner` must be **shared** between parser and semantic analyzer (not re-created) so that interned `Id` values match. `Interner` derives `Clone` for this purpose — use `interner.clone()` when ownership is consumed.

### Standard Library

`library/prelude.slq` (487 lines) is embedded at compile time via `include_str!()`. Defines quantum gates (H, X, Y, Z, S, T, CNOT, phase, rotX/Y/Z), arithmetic (gcd, pow_mod), trigonometric functions, complex numbers, and data types (`dat int[n]`, `dat uint[n]`, `dat ℤmod[N]`, `dat ℤstar[N]`). No `__uminus_*` or operator lowering functions are in the prelude (these live in the D version's `library/__internal/operators.slq`).

### Interpreter Coverage (qsim.rs)

The `Interpreter::eval()` handles 17 of 35 Expression variants directly. The following variants are **not yet handled** in qsim.rs:

```rust
// AST variants present but eval falls through to the catch-all _ arm:
Error, Placeholder, Wildcard, Typeof,       // informational/error
UnaryPlus, UnaryMinus, LogicalNot, BitwiseNot, // unary ops (not lowered)
Index, Slice, Field,                         // projections
Vector, Concat,                              // composite literals
Repeat, Comma                                // control flow + tupling
```

Currently handled: Literal, Identifier, Binary (+, -, *, ==, !=, <, >), Call, Tuple, Let, Assign, Lambda (returns error), IfThenElse, With, ForLoop, WhileLoop, Return, Compound, TypeAnnotation (passthrough), Forget, Assert, Type, TypeDecl, TypeVar.

## Code Conventions

- Unit tests go in `#[cfg(test)] mod tests` at the bottom of source files
- Use `Expression::new_*` constructors for creating AST nodes
- Errors use `Result<T, String>` (not custom error types)
- Pattern match on AST enums exhaustively with `_` or `{ .. }` catch-alls
- Keyword tokens are checked via `self.current.ty` match arms, not string comparison
- The `Location` struct tracks (line, col, offset) and derives PartialEq

## Key Types Reference

```rust
// Core enums
Expression       // 35 variants: Error, Literal, Identifier, Placeholder, Wildcard, Typeof,
                 //   UnaryPlus, UnaryMinus, LogicalNot, BitwiseNot, Binary, Call, Index,
                 //   Slice, Field, Tuple, Vector, Concat, Let, Assign, Lambda, IfThenElse,
                 //   With, ForLoop, WhileLoop, Repeat, Return, Comma, Compound,
                 //   TypeAnnotation, Forget, Assert, Type, TypeDecl, TypeVar
Declaration      // 4 variants (VarDecl, FunctionDef, DatDecl, Import)
TypeKind         // 16 variants (Numeric, FixedInt, ZMod, Aggregate, Unit, Bottom,
                 //   Tuple, Array, Vector, String, Product, Classical, QNumeric,
                 //   TypeVar, TypeMeta, Context)
NumericType      // 6 variants (Bool <: Nat <: Int <: Rat <: Real <: Complex)
LiteralValue     // 7 variants (Bool, Int, Float, Rational, String, Char, Unit)
Annotation       // 5 variants (None, Mfree, Qfree, Lifted, Wild)
CaptureAnnotation // 6 variants (None, Const, Moved, Once, Spent)
TypeAnnotationKind // 4 variants (Colon, As, Coerce, Pun)
TokenType        // ~92 token types (6 literals, 16 delimiters, 6 assign, 7 arithmetic,
                 //   6 comparison, 7 logical, 3 shift, 3 type-ops, 35 keywords, 2 special)

// Quantum simulator
QState           // amplitudes: BTreeMap<BasisState, Complex64>
Interpreter      // walks AST against QState
Value            // runtime values (Bool, Int, IntFixed, Float, Complex, Rational,
                 //   QVar, Tuple, Array, Unit, Closure, Error)
BasisState       // computational basis (Vec<u8> of 0/1 per qubit)
```

## Current Limitations (vs original D implementation)

| Feature | Status |
|---------|--------|
| Lexer | Full (R/r raw strings, Unicode, nested comments) |
| Parser | Full (:= assignment, λ params, element assignment) **Note:** parenthesized tuples like `(a, b)` are parsed as `Binary { op: Comma }` and must be lowered to `Tuple` in semantic analysis |
| Quantum simulator | Qubit allocation, gates, measurement, global phase |
| Type classicality | ℕ/ℤ/ℚ/ℝ inherent, 𝔹/ℂ quantum-by-default |
| Type inference (HM-like) | Name resolution only |
| Operator lowerings | Not implemented |
| Full reverse transformation | Stub |
| HQIR backend | Stub |
| Linearity checker | Basic |

## WASM / Cross-Platform

The crate compiles to both native (Windows/Linux) and WASM targets:

| Feature | Native | WASM |
|---------|--------|------|
| `colored` terminal output | Yes | No (plain text) |
| `std::fs` module loading | Yes | No (cfg gated) |
| `std::process::exit` | Yes | No (returns Err) |
| Prelude loading (`include_str!`) | Yes | Yes |
| wasm-bindgen exports | No | Optional (`--features wasm`) |

**Cargo features:**
- `default = ["terminal"]` — enables colored terminal output
- `terminal` — pulls in `colored` crate
- `wasm` — pulls in `wasm-bindgen`, enables JS-exportable functions in `src/wasm.rs`

**WASM exports** (`src/wasm.rs`, wasm32 only):
- `run_silq(source: &str) -> String` — parse and execute Silq code
- `run_silq_dump(source: &str) -> String` — execute and dump quantum state
- `parse_silq(source: &str) -> String` — parse only, return AST debug
- `tokenize_silq(source: &str) -> String` — tokenize only

## Important Notes

- The `Expression::Type` variant changed from tuple `Type(TypeKind)` to struct `Type { loc, kind }` to support location tracking
- All `Expression::Type(TypeKind::...)` constructions must use `Expression::Type { loc: Location::default(), kind: TypeKind::... }`
- Pattern matching on `Expression::Type` must use `Expression::Type { kind: TypeKind::Numeric(nt), .. }` or `Expression::Type { .. }`
- The standard library uses Unicode math characters (𝔹, ℕ, ℤ, ℚ, ℝ, ℂ, ⊥, 𝟙) which the lexer handles


## Persistent Memory: Upstream D Implementation Gaps

Last updated: 2026-06-05

### Reference Project

- **Location:** `D:\Documents\GitHub\silq` (original Silq compiler in D, ~55K lines)
- **Maintainer:** Timon Gehr (actively maintained)
- **Status:** Ahead of silq-rs on multiple critical features (see gaps below)

### Key Gaps Between silq-rs and Upstream D

#### 1. Operator Lowering -- NOT implemented (BIGGEST BOTTLENECK)

- D version has `library/__internal/operators.slq` with 20+ operator overloads:
  - `__uminus_*`, `__umul_*`, `__uadd_*` (arithmetic)
  - Uses `@[__operator]` annotation to mark functions as operator lowering targets
- silq-rs has `library/prelude.slq` with NO operator lowering infrastructure
- This blocks most prelude operators and complex examples (including Shor's algorithm)

#### 2. No Runtime Type Conversion (evalType / convertTo)

- D version recently (commit 51d261d8) refactored `Interpreter.convertTo()` to extract `evalType()`
- `evalType()` recursively evaluates compound types at runtime:
  - Tuple types, vector types, array types
  - Fixed-width integer types (intN)
  - Zmod (modular integer) types
- silq-rs's `qsim.rs` skips type annotations entirely:
  ```rust
  Expression::TypeAnnotation { .. } => self.eval(expr)
  ```

#### 3. Linear Type Checker Is Basic

- D version recently improved borrow tracking from nested scopes (commit 4fe80a6a)
- D version added dependency tracking for qfree LHS calls (commit 974ef63b)
- silq-rs's `checker.rs` is basic (const/moved tracking only)
- silq-rs's `reverse.rs` (automatic adjoint/uncomputation) is a stub
- No qfree analysis or automatic uncomputation yet

#### 4. Zstar Type Exists but No Operations

- Prelude defines `dat Zstar[N:!N] quantum{}` type
- D version recently added `__uminus_x`/`__uminus_X` and arithmetic operators for Zstar
- silq-rs has no support for Zstar operations

### Recommended Development Priority

| Priority | Feature | Impact |
|----------|---------|--------|
| 1 | **Operator lowering** | Unlocks most prelude operators and all complex examples |
| 2 | **evalType / convertTo** | Enables type-aware runtime execution |
| 3 | **Linear checker improvements** | Borrow tracking, qfree analysis, uncomputation correctness |

### Notes

- D version's recent Shor algorithm rewrites and Ekera postprocessing tests require operator lowering to execute in silq-rs
- The Shor example is the canonical test case for full pipeline correctness
- When implementing operator lowering, study the D version's `library/__internal/operators.slq` and the `@[__operator]` annotation mechanism
- When implementing evalType, study the D version's `Interpreter.evalType()` in `source/silq/backend/interpreter.d`
