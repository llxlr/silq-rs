//! Token types, keywords, and operator definitions for the Silq lexer.
//!
//! Silq supports Unicode math symbols as well as ASCII equivalents.

use std::fmt;

/// All token types recognized by the lexer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
    // Complex tokens (carry data)
    Identifier,     // variable/type names
    StringLit,      // string literals "..."
    CharLit,        // character literals '...'
    IntLit,         // integer literals (decimal, hex, binary)
    FloatLit,       // floating point literals
    RationalLit,    // rational literals (num\den)

    // Delimiters
    LParen,         // (
    RParen,         // )
    LBrace,         // {
    RBrace,         // }
    LBracket,       // [
    RBracket,       // ]
    Semicolon,      // ;
    Comma,          // ,
    Colon,          // :
    Dot,            // .
    Question,       // ?
    At,             // @
    Underscore,     // _
    Arrow,          // -> or →
    FatArrow,       // => or ⇒
    LeftArrow,      // <- or ←
    Mapsto,         // ↦ (maps to)

    // Assignment operators
    Assign,         // :=
    PlusAssign,     // +=
    MinusAssign,    // -=
    MulAssign,      // *=
    DivAssign,      // /=
    ModAssign,      // %=

    // Arithmetic operators
    Plus,           // +
    Minus,          // -
    Mul,            // * or ·
    Div,            // /
    Mod,            // %
    Power,          // ^
    Tilde,          // ~ (bitwise NOT / string concat)

    // Comparison operators
    Eq,             // ==
    Neq,            // != or ≠
    Lt,             // <
    Gt,             // >
    Le,             // <= or ≤
    Ge,             // >= or ≥

    // Logical operators
    And,            // && or ∧
    Or,             // || or ∨
    Xor,            // ⊻
    Not,            // ! or ¬
    Amp,            // & (bitwise AND)
    Pipe,           // | (bitwise OR)
    Caret,          // (bitwise XOR, distinct from power ^)

    // Shift operators
    Shl,            // <<
    Shr,            // >>
    UShr,           // >>> (unsigned/logical right shift)

    // Type operators
    Cross,          // × (product type)
    Classical,      // ! (classical type prefix)
    Pi,             // Π or ∏ (dependent product type)

    // Keywords
    KwDat,
    KwDef,
    KwTrue,
    KwFalse,
    KwIf,
    KwThen,
    KwElse,
    KwObserve,
    KwAssert,
    KwReturn,
    KwRepeat,
    KwFor,
    KwWhile,
    KwIn,
    KwCobserve,
    KwImport,
    KwAs,
    KwCoerce,
    KwPun,
    KwForget,
    KwTypeof,
    KwWild,
    KwLet,
    KwLambda,       // λ or lambda
    KwQuantum,
    KwConst,
    KwMoved,
    KwOnce,
    KwSpent,
    KwLifted,
    KwQfree,
    KwMfree,
    KwClassical,
    KwDo,
    KwWith,

    // Annotations
    Annotation,     // @[...]

    // Special
    Eof,
    Error,
}

impl TokenType {
    /// Returns the string representation of a token type.
    pub fn as_static_str(self) -> &'static str {
        match self {
            TokenType::Identifier => "identifier",
            TokenType::StringLit => "string literal",
            TokenType::CharLit => "character literal",
            TokenType::IntLit => "integer literal",
            TokenType::FloatLit => "float literal",
            TokenType::RationalLit => "rational literal",
            TokenType::LParen => "'('",
            TokenType::RParen => "')'",
            TokenType::LBrace => "'{'",
            TokenType::RBrace => "'}'",
            TokenType::LBracket => "'['",
            TokenType::RBracket => "']'",
            TokenType::Semicolon => "';'",
            TokenType::Comma => "','",
            TokenType::Colon => "':'",
            TokenType::Dot => "'.'",
            TokenType::Question => "'?'",
            TokenType::At => "'@'",
            TokenType::Underscore => "'_'",
            TokenType::Arrow => "'->'",
            TokenType::FatArrow => "'=>'",
            TokenType::LeftArrow => "'<-'",
            TokenType::Mapsto => "'↦'",
            TokenType::Assign => "':='",
            TokenType::PlusAssign => "'+='",
            TokenType::MinusAssign => "'-='",
            TokenType::MulAssign => "'*='",
            TokenType::DivAssign => "'/='",
            TokenType::ModAssign => "'%='",
            TokenType::Plus => "'+'",
            TokenType::Minus => "'-'",
            TokenType::Mul => "'*'",
            TokenType::Div => "'/'",
            TokenType::Mod => "'%'",
            TokenType::Power => "'^'",
            TokenType::Tilde => "'~'",
            TokenType::Eq => "'=='",
            TokenType::Neq => "'!='",
            TokenType::Lt => "'<'",
            TokenType::Gt => "'>'",
            TokenType::Le => "'<='",
            TokenType::Ge => "'>='",
            TokenType::And => "'&&'",
            TokenType::Or => "'||'",
            TokenType::Xor => "'⊻'",
            TokenType::Not => "'!'",
            TokenType::Amp => "'&'",
            TokenType::Pipe => "'|'",
            TokenType::Caret => "'^'",
            TokenType::Shl => "'<<'",
            TokenType::Shr => "'>>'",
            TokenType::UShr => "'>>>'",
            TokenType::Cross => "'×'",
            TokenType::Classical => "'!'",
            TokenType::Pi => "'Π'",
            TokenType::KwDat => "'dat'",
            TokenType::KwDef => "'def'",
            TokenType::KwTrue => "'true'",
            TokenType::KwFalse => "'false'",
            TokenType::KwIf => "'if'",
            TokenType::KwThen => "'then'",
            TokenType::KwElse => "'else'",
            TokenType::KwObserve => "'observe'",
            TokenType::KwAssert => "'assert'",
            TokenType::KwReturn => "'return'",
            TokenType::KwRepeat => "'repeat'",
            TokenType::KwFor => "'for'",
            TokenType::KwWhile => "'while'",
            TokenType::KwIn => "'in'",
            TokenType::KwCobserve => "'cobserve'",
            TokenType::KwImport => "'import'",
            TokenType::KwAs => "'as'",
            TokenType::KwCoerce => "'coerce'",
            TokenType::KwPun => "'pun'",
            TokenType::KwForget => "'forget'",
            TokenType::KwTypeof => "'typeof'",
            TokenType::KwWild => "'wild'",
            TokenType::KwLet => "'let'",
            TokenType::KwLambda => "'lambda'",
            TokenType::KwQuantum => "'quantum'",
            TokenType::KwConst => "'const'",
            TokenType::KwMoved => "'moved'",
            TokenType::KwOnce => "'once'",
            TokenType::KwSpent => "'spent'",
            TokenType::KwLifted => "'lifted'",
            TokenType::KwQfree => "'qfree'",
            TokenType::KwMfree => "'mfree'",
            TokenType::KwClassical => "'classical'",
            TokenType::KwDo => "'do'",
            TokenType::KwWith => "'with'",
            TokenType::Annotation => "annotation",
            TokenType::Eof => "end of file",
            TokenType::Error => "error",
        }
    }
}

impl fmt::Display for TokenType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_static_str())
    }
}

/// A token produced by the lexer.
#[derive(Debug, Clone)]
pub struct Token {
    pub ty: TokenType,
    /// The source text of the token.
    pub text: String,
    /// Line number (1-based) in the source file.
    pub line: u32,
    /// Column number (1-based) in the source file.
    pub col: u32,
    /// Byte offset in the source file.
    pub offset: usize,
}

impl Token {
    pub fn new(ty: TokenType, text: String, line: u32, col: u32, offset: usize) -> Self {
        Token { ty, text, line, col, offset }
    }

    /// Check if this token is a keyword with the given text.
    pub fn is_keyword(&self, kw: &str) -> bool {
        matches!(self.ty, TokenType::Identifier) && self.text == kw
    }
}

/// Mapping from keyword strings to token types.
pub fn lookup_keyword(word: &str) -> Option<TokenType> {
    match word {
        "dat" => Some(TokenType::KwDat),
        "def" => Some(TokenType::KwDef),
        "true" => Some(TokenType::KwTrue),
        "false" => Some(TokenType::KwFalse),
        "if" => Some(TokenType::KwIf),
        "then" => Some(TokenType::KwThen),
        "else" => Some(TokenType::KwElse),
        "observe" => Some(TokenType::KwObserve),
        "assert" => Some(TokenType::KwAssert),
        "return" => Some(TokenType::KwReturn),
        "repeat" => Some(TokenType::KwRepeat),
        "for" => Some(TokenType::KwFor),
        "while" => Some(TokenType::KwWhile),
        "in" => Some(TokenType::KwIn),
        "cobserve" => Some(TokenType::KwCobserve),
        "import" => Some(TokenType::KwImport),
        "as" => Some(TokenType::KwAs),
        "coerce" => Some(TokenType::KwCoerce),
        "pun" => Some(TokenType::KwPun),
        "forget" => Some(TokenType::KwForget),
        "typeof" => Some(TokenType::KwTypeof),
        "wild" => Some(TokenType::KwWild),
        "let" => Some(TokenType::KwLet),
        "lambda" | "λ" => Some(TokenType::KwLambda),
        "quantum" => Some(TokenType::KwQuantum),
        "const" => Some(TokenType::KwConst),
        "moved" => Some(TokenType::KwMoved),
        "once" => Some(TokenType::KwOnce),
        "spent" => Some(TokenType::KwSpent),
        "lifted" => Some(TokenType::KwLifted),
        "qfree" => Some(TokenType::KwQfree),
        "mfree" => Some(TokenType::KwMfree),
        "classical" => Some(TokenType::KwClassical),
        "do" => Some(TokenType::KwDo),
        "with" => Some(TokenType::KwWith),
        _ => None,
    }
}

/// Operator precedence levels for the Pratt parser.
pub mod precedence {
    pub const COMMA: u8 = 10;
    pub const ASSIGN: u8 = 20;
    pub const AS_COERCE: u8 = 30;
    pub const COLON: u8 = 31;
    pub const CONDITIONAL: u8 = 40;
    pub const OR: u8 = 50;
    pub const XOR: u8 = 55;
    pub const AND: u8 = 60;
    pub const BIT_OR: u8 = 70;
    pub const BIT_XOR: u8 = 80;
    pub const BIT_AND: u8 = 90;
    pub const RELATIONAL: u8 = 100;
    pub const SHIFT: u8 = 110;
    pub const ARROW: u8 = 115;
    pub const ADDITIVE: u8 = 120;
    pub const MULTIPLICATIVE: u8 = 130;
    pub const POWER: u8 = 150;
    pub const POSTFIX: u8 = 160;
}

/// Get the left binding power (precedence) of a binary operator token.
pub fn get_lbp(ty: TokenType) -> u8 {
    match ty {
        TokenType::Comma => precedence::COMMA,
        TokenType::Assign | TokenType::LeftArrow
            | TokenType::PlusAssign | TokenType::MinusAssign
            | TokenType::MulAssign | TokenType::DivAssign | TokenType::ModAssign => precedence::ASSIGN,
        TokenType::KwAs | TokenType::KwCoerce | TokenType::KwPun => precedence::AS_COERCE,
        TokenType::Colon => precedence::COLON,
        TokenType::Question => precedence::CONDITIONAL,
        TokenType::Or => precedence::OR,
        TokenType::Xor => precedence::XOR,
        TokenType::And => precedence::AND,
        TokenType::Pipe => precedence::BIT_OR,
        TokenType::Caret => precedence::BIT_XOR,
        TokenType::Amp => precedence::BIT_AND,
        TokenType::Eq | TokenType::Neq | TokenType::Lt | TokenType::Gt
            | TokenType::Le | TokenType::Ge => precedence::RELATIONAL,
        TokenType::Shl | TokenType::Shr | TokenType::UShr => precedence::SHIFT,
        TokenType::Arrow => precedence::ARROW,
        TokenType::Plus | TokenType::Minus | TokenType::Tilde => precedence::ADDITIVE,
        TokenType::Mul | TokenType::Div | TokenType::Mod | TokenType::Cross => precedence::MULTIPLICATIVE,
        TokenType::Power => precedence::POWER,
        TokenType::Dot | TokenType::LParen | TokenType::LBracket => precedence::POSTFIX,
        _ => 0,
    }
}
