use std::fmt;

#[derive(Debug)]
pub(crate) enum Error {
    //Io(std::io::Error),
    //Uri(uri::Error),
    Hyper(hyper::Error),
    Http(hyper::http::Error),
    Application,
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
        f.write_str(std::error::Error::description(self))
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        match self {
            Error::Hyper(e) => e.description(),
            Error::Http(e) => e.description(),
            Error::Application => "Unspecified application error",
        }
    }
}
