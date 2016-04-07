use std::fmt;
use std::error::Error as StdError;

use {OSError, Error};


impl OSError {
    pub fn explain(self) -> Error {
        let text = self.1.explain();
        Error(self.1, self.0, text)
    }
}

impl fmt::Display for OSError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}: {}", self.1, self.0)
    }
}

impl StdError for OSError {
    fn cause(&self) -> Option<&StdError> {
        Some(&self.0)
    }
    fn description(&self) -> &str {
        self.0.description()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}: {} ({})", self.0, self.1, self.2)
    }
}

impl StdError for Error {
    fn cause(&self) -> Option<&StdError> {
        Some(&self.1)
    }
    fn description(&self) -> &str {
       self.1.description()
    }
}
