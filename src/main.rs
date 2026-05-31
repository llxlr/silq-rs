//! Silq quantum programming language - CLI entry point.
//!
//! Usage:
//!   silq [options] <file.slq>
//!
//! Options:
//!   --run          Execute the program using the quantum simulator
//!   --compile      Compile to HQIR format
//!   --check        Perform static checking only (no execution)
//!   --help         Show help
//!   --trace        Trace execution
//!   --verbose      Verbose output

use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 || args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        print_help();
        process::exit(if args.len() < 2 { 1 } else { 0 });
    }

    // Parse options
    let mut run_mode = true; // Default: interpret/run
    let mut compile_mode = false;
    let mut check_only = false;
    let mut trace = false;
    let mut verbose = false;
    let mut show_state = false;
    let mut files: Vec<String> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--run" => run_mode = true,
            "--run" if i + 1 < args.len() => {
                // --run can take an optional expression argument
                run_mode = true;
            }
            "--compile" => {
                compile_mode = true;
                run_mode = false;
            }
            "--check" => {
                check_only = true;
                run_mode = false;
            }
            "--trace" => trace = true,
            "--verbose" | "-v" => verbose = true,
            "--dump" | "--dump-state" => show_state = true,
            "--help" | "-h" => {
                print_help();
                process::exit(0);
            }
            arg if arg.starts_with("--") => {
                eprintln!("silq: unknown option: {}", arg);
                process::exit(1);
            }
            arg => {
                files.push(arg.to_string());
            }
        }
        i += 1;
    }

    if files.is_empty() {
        eprintln!("silq: no input files");
        process::exit(1);
    }

    // Process each file
    for file_path in &files {
        if verbose {
            eprintln!("silq: processing {}", file_path);
        }

        let result = process_file(file_path, run_mode, compile_mode, check_only, trace, verbose, show_state);

        match result {
            Ok(_) => {
                if verbose {
                    eprintln!("silq: {} processed successfully", file_path);
                }
            }
            Err(msg) => {
                eprintln!("silq: error processing {}: {}", file_path, msg);
                process::exit(1);
            }
        }
    }
}

/// Process a single Silq source file.
fn process_file(path: &str, run: bool, compile: bool, check_only: bool,
                trace: bool, verbose: bool, dump_state: bool) -> Result<(), String> {
    // Read source file
    let source = fs::read_to_string(path)
        .map_err(|e| format!("cannot read file: {}", e))?;

    // Create interner and lexer
    let mut interner = silq::ast::Interner::new();
    let mut lexer = silq::Lexer::new(&source);

    // Parse
    let mut parser = silq::Parser::new(&mut lexer, &mut interner);
    let mut ast = parser.parse_program();

    if verbose {
        eprintln!("silq: parsed {} top-level expressions", ast.len());
    }

    if ast.is_empty() {
        return Err("empty program".into());
    }

    // Semantic analysis
    let mut scope = silq::scope::Scope::global();
    let mut analyzer = silq::semantic::SemanticAnalyzer::new(silq::ast::Interner::new(), scope);
    analyzer.semantic_program(&mut ast);

    // Check mode only
    if check_only {
        if analyzer.error_count() > 0 {
            return Err(format!("{} semantic errors", analyzer.error_count()));
        }
        println!("silq: static check passed for {}", path);
        return Ok(());
    }

    // Compile mode (HQIR)
    if compile {
        let mut hqir = silq::hqir::HqirWriter::new();
        hqir.compile(&ast);
        println!("{}", hqir.into_string());
        return Ok(());
    }

    // Run mode (quantum simulator)
    if run {
        let mut sim_interpreter = silq::qsim::QSim::new(interner);
        sim_interpreter.set_trace(trace);

        match sim_interpreter.run(&ast) {
            Ok(value) => {
                if verbose {
                    println!("result: {}", value.display());
                }
                if dump_state {
                    println!("{}", sim_interpreter.dump_state());
                }
                Ok(())
            }
            Err(msg) => {
                Err(format!("runtime error: {}", msg))
            }
        }
    } else {
        Ok(())
    }
}

/// Print help text.
fn print_help() {
    println!(r#"Silq - A high-level quantum programming language (Rust implementation)

USAGE:
    silq [options] <file.slq>...

OPTIONS:
    --run                Execute the program using the quantum simulator (default)
    --compile            Compile to HQIR (High-level Quantum IR) format
    --check              Perform static checking only, without execution
    --trace              Trace execution step by step
    --verbose, -v        Verbose output
    --dump, --dump-state Dump the quantum state after execution
    --help, -h           Show this help message

EXAMPLES:
    silq --run examples/bell.slq
    silq --compile my_algorithm.slq
    silq --check --verbose test.slq

SILQ LANGUAGE FEATURES:
    - High-level quantum programming with automatic uncomputation
    - Linear type system for quantum resources
    - Dependent types
    - Built-in quantum gates: H, X, Y, Z, phase, CNOT, measure

FOR MORE INFORMATION:
    https://silq.ethz.ch
    https://github.com/tgehr/silq
"#);
}
