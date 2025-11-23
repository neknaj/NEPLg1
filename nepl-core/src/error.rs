use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("failed to read source: {0}")]
    SourceIo(#[from] std::io::Error),
    #[error("unsupported emit format: {0}")]
    UnsupportedFormat(String),
    #[error("standard library directory was not found at {0}")]
    MissingStdlib(PathBuf),
    #[error("lex error at byte {position}: {message}")]
    LexError { position: usize, message: String },
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("semantic error: {0}")]
    SemanticError(String),
}
