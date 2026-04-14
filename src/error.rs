use thiserror::Error;

/// Errors produced during YANG parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unexpected end of input")]
    UnexpectedEof,

    #[error("unexpected token at byte {pos}: expected {expected}, got {got:?}")]
    UnexpectedToken {
        pos: usize,
        expected: &'static str,
        got: String,
    },

    #[error("unterminated string literal starting at byte {0}")]
    UnterminatedString(usize),

    #[error("invalid escape sequence '\\{0}'")]
    InvalidEscape(char),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
