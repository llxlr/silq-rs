//! Compiler options and language configuration.

/// Language variant (silq vs psi).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Silq,
    Psi,
}

impl Default for Language {
    fn default() -> Self {
        Language::Silq
    }
}

/// Compiler configuration options.
#[derive(Debug, Clone)]
pub struct Options {
    /// Language variant.
    pub language: Language,
    /// Paths to search for imported modules.
    pub import_paths: Vec<String>,
    /// Maximum iterations for fixed-point computations.
    pub inference_limit: u32,
    /// Allow unsafe const capture of quantum variables.
    pub allow_unsafe_capture_const: bool,
    /// Remove loops during HQIR compilation.
    pub remove_loops: bool,
    /// Split components flag.
    pub split_components: bool,
    /// Random seed for quantum simulation.
    pub seed: Option<u64>,
    /// Repeat count for --repeat.
    pub repeat: u64,
    /// Verbose output.
    pub verbose: bool,
    /// Trace execution.
    pub trace: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            language: Language::default(),
            import_paths: vec![],
            inference_limit: 30,
            allow_unsafe_capture_const: false,
            remove_loops: false,
            split_components: false,
            seed: None,
            repeat: 1,
            verbose: false,
            trace: false,
        }
    }
}
