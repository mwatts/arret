use std::{error, result};
use std::fmt;
use std::fmt::Display;

use syntax::span::Span;
use syntax::error::Error as SyntaxError;
use reporting::{Level, Reportable};

#[derive(Debug, PartialEq, Clone)]
pub struct ErrorLoc {
    span: Span,
    macro_invocation_span: Option<Span>,
}

impl ErrorLoc {
    pub fn new(span: Span, macro_invocation_span: Option<Span>) -> ErrorLoc {
        ErrorLoc {
            span,
            macro_invocation_span,
        }
    }

    fn with_macro_invocation_span(self, macro_invocation_span: Span) -> ErrorLoc {
        ErrorLoc {
            span: self.span,
            macro_invocation_span: Some(macro_invocation_span),
        }
    }
}

impl From<Span> for ErrorLoc {
    fn from(span: Span) -> ErrorLoc {
        ErrorLoc::new(span, None)
    }
}

#[derive(Debug, PartialEq)]
pub enum ErrorKind {
    PrimRef,
    TyRef,
    MacroRef(String),
    UnboundSymbol(String),
    WrongArgCount(usize),
    IllegalArg(String),
    ExpectedSymbol,
    DefOutsideBody,
    ExportOutsideModule,
    NonDefInsideModule,
    LibraryNotFound,
    NoMacroRule,
    DuplicateMacroVar(String, Span),
    MultipleZeroOrMoreMatch(Span),
    NoVecDestruc,
    ValueAsTy,
    TypeErased(String, String),
    UserError(String),
    ReadError(String),
    SyntaxError(SyntaxError),
}

#[derive(Debug, PartialEq)]
pub struct Error {
    error_loc: ErrorLoc,
    kind: ErrorKind,
}

pub type Result<T> = result::Result<T, Error>;

impl Error {
    pub fn new(span: Span, kind: ErrorKind) -> Error {
        Error {
            error_loc: span.into(),
            kind,
        }
    }

    pub fn with_macro_invocation_span(self, span: Span) -> Error {
        Error {
            error_loc: self.error_loc.with_macro_invocation_span(span),
            kind: self.kind,
        }
    }
}

impl Reportable for Error {
    fn level(&self) -> Level {
        Level::Error
    }

    fn message(&self) -> String {
        match self.kind {
            ErrorKind::PrimRef => "cannot take the value of a primitive".to_owned(),
            ErrorKind::TyRef => "cannot take the value of a type".to_owned(),
            ErrorKind::MacroRef(ref sym) => format!("cannot take the value of macro: `{}`", sym),
            ErrorKind::UnboundSymbol(ref sym) => format!("unable to resolve symbol: `{}`", sym),
            ErrorKind::WrongArgCount(expected) => format!("wrong arg count; expected {}", expected),
            ErrorKind::IllegalArg(ref description) => format!("illegal argument: {}", description),
            ErrorKind::ExpectedSymbol => "expected symbol".to_owned(),
            ErrorKind::DefOutsideBody => "(def) outside module or function body".to_owned(),
            ErrorKind::ExportOutsideModule => "(export) outside of module body".to_owned(),
            ErrorKind::NonDefInsideModule => {
                "definition expected at the top-level of a module body".to_owned()
            }
            ErrorKind::LibraryNotFound => "library not found".to_owned(),
            ErrorKind::NoMacroRule => "no matching macro rule".to_owned(),
            ErrorKind::DuplicateMacroVar(ref sym, _) => {
                format!("duplicate macro variable: `{}`", sym)
            }
            ErrorKind::MultipleZeroOrMoreMatch(_) => {
                "multiple zero or more matches in the same sequence".to_owned()
            }
            ErrorKind::NoVecDestruc => {
                "vectors can only be used for type ascription in the form [name : Type]".to_owned()
            }
            ErrorKind::ValueAsTy => "value cannot be used as a type".to_owned(),
            ErrorKind::TypeErased(ref left, ref right) => {
                format!("type {} cannot be distinguished from {} at runtime due to type erasure; they cannot be members of the same union type",
                    left,
                    right,
                )
            }
            ErrorKind::UserError(ref message) => message.clone(),
            ErrorKind::ReadError(ref filename) => format!("error reading `{}`", filename),
            ErrorKind::SyntaxError(ref err) => err.message(),
        }
    }

    fn span(&self) -> Span {
        self.error_loc.span
    }

    fn macro_invocation_span(&self) -> Option<Span> {
        self.error_loc.macro_invocation_span
    }

    fn associated_report(&self) -> Option<Box<Reportable>> {
        match self.kind {
            ErrorKind::DuplicateMacroVar(_, span) => Some(Box::new(FirstDefHelp { span })),
            ErrorKind::MultipleZeroOrMoreMatch(span) => Some(Box::new(FirstDefHelp { span })),
            ErrorKind::SyntaxError(ref err) => err.associated_report(),
            _ => None,
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        "Lowering error"
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<SyntaxError> for Error {
    fn from(err: SyntaxError) -> Error {
        Error::new(err.span(), ErrorKind::SyntaxError(err))
    }
}

struct FirstDefHelp {
    span: Span,
}

impl Reportable for FirstDefHelp {
    fn level(&self) -> Level {
        Level::Help
    }

    fn span(&self) -> Span {
        self.span
    }

    fn message(&self) -> String {
        "first definition here".to_owned()
    }
}
