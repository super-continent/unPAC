use nom::error::{ErrorKind, ParseError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PacError {
    #[error("Could not parse File Entry")]
    FileEntry,
    #[error("Parser error `{0:?}`")]
    Nom(ErrorKind),
}

impl<I> ParseError<I> for PacError {
    fn from_error_kind(_input: I, kind: ErrorKind) -> Self {
        Self::Nom(kind)
    }

    fn append(_: I, _: ErrorKind, other: Self) -> Self {
        other
    }
}
