use std::fmt;

/// Errors produced by the parser layer.
///
/// `lark_rs`'s own error types are mapped into this at the engine boundary
/// (`decisions/0002`); they must not leak into the public API.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// A grammar failed to build (programmer error; covered by tests).
    Grammar(String),
    /// The input did not parse. `line`/`column` are 1-based (as in Python Lark).
    Parse {
        message: String,
        line: usize,
        column: usize,
    },
    /// A transformer produced a value the schema rejects (mirrors a Pydantic
    /// `ValidationError`).
    Validation(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Grammar(m) => write!(f, "grammar error: {m}"),
            Error::Parse {
                message,
                line,
                column,
            } => write!(f, "parse error at line {line}, column {column}: {message}"),
            Error::Validation(m) => write!(f, "validation error: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<lark_rs::LarkError> for Error {
    fn from(e: lark_rs::LarkError) -> Self {
        use lark_rs::LarkError;
        match e {
            LarkError::Grammar(g) => Error::Grammar(format!("{g}")),
            LarkError::Parse(p) => Error::from(p),
        }
    }
}

impl From<lark_rs::ParseError> for Error {
    fn from(e: lark_rs::ParseError) -> Self {
        use lark_rs::ParseError;
        match e {
            ParseError::UnexpectedToken {
                token,
                token_type,
                line,
                col,
                expected,
            } => Error::Parse {
                message: format!(
                    "Unexpected token {token:?} ({token_type}) at line {line}, column {col}. Expected one of: {expected:?}"
                ),
                line,
                column: col,
            },
            ParseError::UnexpectedCharacter {
                ch,
                line,
                col,
                expected,
                ..
            } => Error::Parse {
                message: format!(
                    "Unexpected character {ch:?} at line {line}, column {col}. Expected: {expected}"
                ),
                line,
                column: col,
            },
            ParseError::UnexpectedEof {
                line,
                col,
                expected,
            } => Error::Parse {
                message: format!("Unexpected end of input. Expected: {expected:?}"),
                line,
                column: col,
            },
            ParseError::Postlex { msg } => Error::Parse {
                message: msg,
                line: 0,
                column: 0,
            },
        }
    }
}
