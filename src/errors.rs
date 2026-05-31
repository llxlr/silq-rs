//! Error handling and diagnostic infrastructure.

use colored::*;
use std::fmt;

/// Location in source code.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Location {
    pub line: u32,      // 1-based
    pub col: u32,       // 1-based
    pub offset: usize,  // 0-based byte offset
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// Severity level of a diagnostic message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorType {
    Error,
    RunError,
    Warning,
    Note,
    Message,
}

impl ErrorType {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorType::Error => "error",
            ErrorType::RunError => "runtime error",
            ErrorType::Warning => "warning",
            ErrorType::Note => "note",
            ErrorType::Message => "message",
        }
    }
}

/// Error handler trait - different implementations for terminal, JSON, etc.
pub trait ErrorHandler {
    /// Report an error.
    fn error(&mut self, loc: &Location, msg: &str);
    /// Report a runtime error.
    fn run_error(&mut self, loc: &Location, msg: &str);
    /// Report a warning.
    fn warning(&mut self, loc: &Location, msg: &str);
    /// Report a note (contextual information).
    fn note(&mut self, loc: &Location, msg: &str);
    /// Report an informational message.
    fn message(&mut self, msg: &str);

    /// Return the number of errors reported so far.
    fn error_count(&self) -> usize;

    /// Check if any errors have been reported.
    fn has_errors(&self) -> bool {
        self.error_count() > 0
    }

    /// Flush any buffered output.
    fn flush(&mut self) {}
}

/// Simple terminal-based error handler.
pub struct SimpleErrorHandler {
    pub errors: Vec<(Location, String, ErrorType)>,
}

impl SimpleErrorHandler {
    pub fn new() -> Self {
        SimpleErrorHandler { errors: vec![] }
    }

    fn emit(&mut self, loc: &Location, msg: &str, etype: ErrorType) {
        self.errors.push((loc.clone(), msg.to_string(), etype));
        match etype {
            ErrorType::Error | ErrorType::RunError => {
                eprintln!(
                    "{} {}: {}",
                    loc,
                    etype.as_str().red().bold(),
                    msg
                );
            }
            ErrorType::Warning => {
                eprintln!(
                    "{} {}: {}",
                    loc,
                    etype.as_str().yellow().bold(),
                    msg
                );
            }
            ErrorType::Note => {
                eprintln!(
                    "{} {}: {}",
                    loc,
                    etype.as_str().cyan().bold(),
                    msg
                );
            }
            ErrorType::Message => {
                eprintln!("{}", msg);
            }
        }
    }
}

impl ErrorHandler for SimpleErrorHandler {
    fn error(&mut self, loc: &Location, msg: &str) {
        self.emit(loc, msg, ErrorType::Error);
    }

    fn run_error(&mut self, loc: &Location, msg: &str) {
        self.emit(loc, msg, ErrorType::RunError);
    }

    fn warning(&mut self, loc: &Location, msg: &str) {
        self.emit(loc, msg, ErrorType::Warning);
    }

    fn note(&mut self, loc: &Location, msg: &str) {
        self.emit(loc, msg, ErrorType::Note);
    }

    fn message(&mut self, msg: &str) {
        eprintln!("{}", msg);
    }

    fn error_count(&self) -> usize {
        self.errors.iter().filter(|(_, _, t)| {
            matches!(t, ErrorType::Error | ErrorType::RunError)
        }).count()
    }
}

/// JSON-based error handler for tooling integration.
pub struct JsonErrorHandler {
    pub diagnostics: Vec<json::Value>,
}

impl JsonErrorHandler {
    pub fn new() -> Self {
        JsonErrorHandler { diagnostics: vec![] }
    }

    fn make_diag(loc: &Location, msg: &str, severity: ErrorType) -> json::Value {
        let mut map = json::Value::object();
        map.insert("severity".into(), json::Value::str(severity.as_str()));
        map.insert("message".into(), json::Value::str(msg));
        let mut loc_map = json::Value::object();
        loc_map.insert("line".into(), json::Value::number(loc.line as f64));
        loc_map.insert("column".into(), json::Value::number(loc.col as f64));
        loc_map.insert("offset".into(), json::Value::number(loc.offset as f64));
        map.insert("location".into(), json::Value::Object(loc_map));
        json::Value::Object(map)
    }

    fn push(&mut self, loc: &Location, msg: &str, severity: ErrorType) {
        self.diagnostics.push(Self::make_diag(loc, msg, severity));
    }
}

impl ErrorHandler for JsonErrorHandler {
    fn error(&mut self, loc: &Location, msg: &str) {
        self.push(loc, msg, ErrorType::Error);
    }

    fn run_error(&mut self, loc: &Location, msg: &str) {
        self.push(loc, msg, ErrorType::RunError);
    }

    fn warning(&mut self, loc: &Location, msg: &str) {
        self.push(loc, msg, ErrorType::Warning);
    }

    fn note(&mut self, loc: &Location, msg: &str) {
        self.push(loc, msg, ErrorType::Note);
    }

    fn message(&mut self, msg: &str) {
        let mut map = json::Value::object();
        map.insert("severity".into(), json::Value::str("message"));
        map.insert("message".into(), json::Value::str(msg));
        self.diagnostics.push(json::Value::Object(map));
    }

    fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| {
            matches!(d.get("severity").and_then(|v| v.as_str()),
                     Some("error") | Some("runtime error"))
        }).count()
    }

    fn flush(&mut self) {
        use std::io::Write;
        for diag in &self.diagnostics {
            let json_str = json::to_string_pretty(diag).unwrap_or_default();
            let _ = std::io::stderr().write_all(json_str.as_bytes());
            let _ = std::io::stderr().write_all(b"\n");
        }
    }
}

// Minimal JSON value type for the JSON error handler.
pub mod json {
    use std::collections::BTreeMap;

    #[derive(Debug, Clone)]
    pub enum Value {
        Null,
        Bool(bool),
        Number(f64),
        Str(String),
        Array(Vec<Value>),
        Object(BTreeMap<String, Value>),
    }

    impl Value {
        pub fn null() -> Self { Value::Null }
        pub fn bool(b: bool) -> Self { Value::Bool(b) }
        pub fn number(n: f64) -> Self { Value::Number(n) }
        pub fn str(s: impl Into<String>) -> Self { Value::Str(s.into()) }
        pub fn array(items: Vec<Value>) -> Self { Value::Array(items) }
        pub fn object() -> BTreeMap<String, Value> { BTreeMap::new() }

        pub fn as_str(&self) -> Option<&str> {
            match self {
                Value::Str(s) => Some(s.as_str()),
                _ => None,
            }
        }

        pub fn get(&self, key: &str) -> Option<&Value> {
            match self {
                Value::Object(map) => map.get(key),
                _ => None,
            }
        }
    }

    pub fn to_string_pretty(val: &Value) -> Result<String, ()> {
        Ok(format_val(val, 0))
    }

    fn format_val(val: &Value, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        let pad1 = "  ".repeat(indent + 1);
        match val {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_string(),
            Value::Str(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            Value::Array(arr) => {
                if arr.is_empty() { return "[]".to_string(); }
                let items: Vec<String> = arr.iter()
                    .map(|v| format!("{}{}", pad1, format_val(v, indent + 1)))
                    .collect();
                format!("[\n{}\n{}]", items.join(",\n"), pad)
            }
            Value::Object(map) => {
                if map.is_empty() { return "{}".to_string(); }
                let items: Vec<String> = map.iter()
                    .map(|(k, v)| format!("{}\"{}\": {}", pad1, k, format_val(v, indent + 1)))
                    .collect();
                format!("{{\n{}\n{}}}", items.join(",\n"), pad)
            }
        }
    }
}
