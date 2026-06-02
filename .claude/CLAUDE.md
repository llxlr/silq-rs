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
cargo test               # Run all tests (10 unit tests)
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
├── ast.rs           # AST (Expression 37 variants, Declaration, TypeKind, Interner)
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

`QState` uses sparse `BTreeMap<BasisState, Complex64>` for state vectors. `alloc_qubit()` initializes the state vector with `|0⟩` amplitude via tensor product. Gates are applied as 2×2 unitary matrices (`apply_1q_gate`). `phase()` applies global phase via `apply_global_phase(phi)` — multiplying all amplitudes by e^(i*phi). Measurement uses `fastrand` for true random number generation and collapses with probability normalization.

### Type System: Classical vs Quantum

Per Silq spec: `ℕ, ℤ, ℚ, ℝ` are **inherently classical** (no `!` prefix needed). `𝔹` and `ℂ` are **quantum by default** — `!𝔹` / `!ℂ` makes them classical. This is enforced in both `TypeKind::is_classical()` (ast.rs) and `is_classical_type()` (conversion.rs).

### Interner Sharing

`Interner` must be **shared** between parser and semantic analyzer (not re-created) so that interned `Id` values match. `Interner` derives `Clone` for this purpose — use `interner.clone()` when ownership is consumed.

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
| Lexer | Full (R/r raw strings, Unicode, nested comments) |
| Parser | Full (:= assignment, λ params, element assignment) |
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
