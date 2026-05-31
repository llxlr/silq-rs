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
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run all tests (10 unit tests)
cargo test lexer         # Run specific test module
cargo test test_parse    # Run tests matching pattern
cargo fmt                # Format code
cargo clippy             # Lint
cargo run -- --help      # Run binary with args
```

## Project Structure

```
src/
├── main.rs          # CLI entry point (196 lines)
├── lib.rs           # Library root, re-exports
├── token.rs         # Token types, keywords, operator precedence tables
├── lexer.rs         # UTF-8 lexer, Unicode math symbols, nested comments
├── parser.rs        # Pratt precedence-climbing recursive descent parser
├── ast.rs           # AST (Expression enum 37 variants, Declaration, TypeKind, Interner)
├── scope.rs         # Nested symbol table with name interning
├── semantic.rs      # Name resolution and scope management
├── checker.rs       # Linear type resource checker (const/moved tracking)
├── consteval.rs     # Compile-time constant expression evaluation
├── conversion.rs    # Numeric type conversion (Bool <: N <: Z <: Q <: R <: C)
├── reverse.rs       # Automatic adjoint/uncomputation transformation
├── modules.rs       # Module import system, prelude loading via include_str!
├── errors.rs        # Error diagnostics (terminal + JSON backends)
├── qsim.rs          # Quantum simulator: QState state vector, Interpreter, gates
├── hqir.rs          # HQIR (High-level Quantum IR) backend stub
└── options.rs       # Compiler configuration (Language::Silq|Psi, Options struct)
```

## Compilation Pipeline

```
Source (.slq) → Lexer → Rarser → Semantic → Checker → Backend (QSim/HQIR)
```

## Key Design Decisions

### AST: Expression = Type

In Silq, types are expressions (dependent types). The Rust AST uses a single `Expression` enum where `Expression::Type { loc, kind: TypeKind }` represents type-level expressions. `TypeKind` has 14 variants (Numeric, Product, Vector, Array, etc.).

### Identifier Interning

`Interner` maps strings to `Id(usize)` for O(1) comparison. All identifiers and field names use interned IDs.

### Pratt Parsing

`get_lbp(token_type)` returns left binding power. Precedence ranges from 10 (comma) to 160 (postfix). The parser cycles through `nud()` (prefix expressions) and `infix()` (binary/postfix).

### Quantum Simulation

`QState` uses sparse `BTreeMap<BasisState, Complex64>` for state vectors. Gates are applied as 2×2 unitary matrices. Measurement collapses with probability normalization.

### Standard Library

`library/prelude.slq` is embedded at compile time via `include_str!()`.

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
Expression       // 37 variants (Literal, Binary, Call, IfThenElse, Lambda, ...)
Declaration      // 4 variants (VarDecl, FunctionDef, DatDecl, Import)
TypeKind         // 14 variants (Numeric, Product, Vector, Array, Classical, ...)
NumericType      // 6 variants (Bool <: Nat <: Int <: Rat <: Real <: Complex)
TokenType        // 60+ token types

// Quantum simulator
QState           // amplitudes: BTreeMap<BasisState, Complex64>
Interpreter      // walks AST against QState
Value            // runtime values (Bool, Int, Float, QVar, Tuple, Closure, ...)
BasisState       // computational basis (Vec<u8> of 0/1 per qubit)
```

## Current Limitations (vs original D implementation)

| Feature | Status |
|---------|--------|
| Lexer | Full |
| Parser | Full |
| Type inference (HM-like) | Name resolution only |
| Operator lowerings | Not implemented |
| Full reverse transformation | Stub |
| HQIR backend | Stub |
| Linearity checker | Basic |

## Important Notes

- The `Expression::Type` variant changed from tuple `Type(TypeKind)` to struct `Type { loc, kind }` to support location tracking
- All `Expression::Type(TypeKind::...)` constructions must use `Expression::Type { loc: Location::default(), kind: TypeKind::... }`
- Pattern matching on `Expression::Type` must use `Expression::Type { kind: TypeKind::Numeric(nt), .. }` or `Expression::Type { .. }`
- The standard library uses Unicode math characters (𝔹, ℕ, ℤ, ℚ, ℝ, ℂ, ⊥, 𝟙) which the lexer handles
