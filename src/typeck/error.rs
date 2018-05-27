use std::error;
use std::fmt;
use std::fmt::Display;

use reporting::{Level, Reportable};
use syntax::span::Span;

#[derive(PartialEq, Debug)]
pub enum ErrorKind {
    IsNotA(String, String),
    VarHasEmptyType(String, String),
    PolyUnionConflict(String, String),
    PredTypeErased(String, String),
    TopFunApply(String),
    RecursiveType,
    // Have, wanted
    TooManyArgs(usize, usize),
    InsufficientArgs(usize, usize),
}

#[derive(PartialEq, Debug)]
pub struct Error(Span, ErrorKind);

impl Error {
    pub fn new(span: Span, kind: ErrorKind) -> Error {
        Error(span, kind)
    }

    pub fn span(&self) -> Span {
        self.0
    }
}

impl Reportable for Error {
    fn level(&self) -> Level {
        Level::Error
    }

    fn message(&self) -> String {
        match self.1 {
            ErrorKind::IsNotA(ref sub, ref parent) => format!("`{}` is not a `{}`", sub, parent),
            ErrorKind::VarHasEmptyType(ref left, ref right) => {
                format!("inferred conflicting types `{}` and `{}`", left, right)
            }
            ErrorKind::PolyUnionConflict(ref left, ref right) => format!(
                "polymorphism prevents `{}` and `{}` from being members of the same union",
                left, right,
            ),
            ErrorKind::PredTypeErased(ref subject, ref testing) => format!(
                "`{}` cannot be tested to have the type `{}` at runtime due to type erasure",
                subject, testing
            ),
            ErrorKind::TopFunApply(ref top_fun) => format!(
                "cannot determine parameter types for top function type `{}`",
                top_fun
            ),
            ErrorKind::TooManyArgs(have, wanted) => {
                format!("too many arguments; wanted {}, have {}", wanted, have)
            }
            ErrorKind::InsufficientArgs(have, wanted) => {
                format!("insufficient arguments; wanted {}, have {}", wanted, have)
            }
            ErrorKind::RecursiveType => {
                "recursive usage requires explicit type annotation".to_owned()
            }
        }
    }

    fn span(&self) -> Span {
        self.span()
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        "Type check error"
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}