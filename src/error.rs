use std::fmt;

#[derive(Debug)]
pub(super) enum Error {
    Hyper(hyper::Error),
    Http(hyper::http::Error),
    Application,
    ApplicationPanic(String),
}

impl Default for Error {
    fn default() -> Self {
        Error::Application
    }
}

impl From<hyper::Error> for Error {
    fn from(e: hyper::Error) -> Self {
        Error::Hyper(e)
    }
}

impl From<hyper::http::Error> for Error {
    fn from(e: hyper::http::Error) -> Self {
        Error::Http(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Hyper(e) => e.fmt(f),
            Error::Http(e) => e.fmt(f),
            Error::Application => f.write_str("Unspecified application error"),
            Error::ApplicationPanic(s) => f.write_str(&*format!("Application panicked: {}", s)),
        }
    }
}

impl std::error::Error for Error {}
