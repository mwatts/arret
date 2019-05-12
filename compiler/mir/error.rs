use std::{error, fmt, result};

use codespan_reporting::{Diagnostic, Label};

use arret_syntax::span::Span;

use crate::mir::inliner::ApplyCookie;
use crate::reporting::LocTrace;

#[derive(Debug, PartialEq)]
pub struct Panic {
    loc_trace: LocTrace,
    message: String,
}

impl Panic {
    pub fn new(span: Span, message: String) -> Panic {
        Panic {
            loc_trace: span.into(),
            message,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Error {
    Panic(Panic),
    /// Internal error used to abort a recursive function application when a loop is detected
    AbortRecursion(ApplyCookie),
    /// Internal error indicating that a divergent value was encountered
    Diverged,
}

pub type Result<T> = result::Result<T, Error>;

impl Error {
    pub fn with_macro_invocation_span(self, span: Span) -> Error {
        match self {
            Error::Panic(Panic { loc_trace, message }) => Error::Panic(Panic {
                loc_trace: loc_trace.with_macro_invocation(span),
                message,
            }),
            other => other,
        }
    }
}

impl From<Error> for Diagnostic {
    fn from(error: Error) -> Diagnostic {
        if let Error::Panic(panic) = error {
            let diagnostic = Diagnostic::new_error(panic.message).with_label(
                Label::new_primary(panic.loc_trace.origin()).with_message("panicked here"),
            );

            return panic.loc_trace.label_macro_invocation(diagnostic);
        }

        panic!(
            "attempted to convert an internal {:?} flow control error to a diagnostic",
            error
        );
    }
}

impl From<Error> for Vec<Diagnostic> {
    fn from(error: Error) -> Vec<Diagnostic> {
        vec![error.into()]
    }
}

impl error::Error for Panic {}

impl fmt::Display for Panic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
