use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};

// TODO: rewrite it with thiserror?
#[derive(Debug)]
pub struct Error {
    msg: String,
}

impl Error {
    pub fn from_msg_err<M: ToString, E: StdError>(msg: M, err: &E) -> Self {
        Self {
            msg: format!("{}: {}", msg.to_string(), err),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        None
    }

    fn cause(&self) -> Option<&dyn StdError> {
        self.source()
    }
}

impl From<&str> for Error {
    fn from(value: &str) -> Self {
        Self {
            msg: value.to_string(),
        }
    }
}

impl From<String> for Error {
    fn from(value: String) -> Self {
        Self { msg: value }
    }
}
