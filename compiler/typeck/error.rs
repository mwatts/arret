use std::fmt::Display;
use std::{error, fmt, iter};

use codespan_reporting::diagnostic::Diagnostic;

use arret_syntax::span::Span;

use crate::hir;
use crate::reporting::{new_label, LocTrace};
use crate::ty;
use crate::ty::purity;

#[derive(PartialEq, Debug, Copy, Clone)]
pub struct WantedArity {
    fixed_len: usize,
    has_rest: bool,
}

impl WantedArity {
    pub fn new(fixed_len: usize, has_rest: bool) -> WantedArity {
        WantedArity {
            fixed_len,
            has_rest,
        }
    }
}

impl fmt::Display for WantedArity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.has_rest {
            write!(f, "at least {}", self.fixed_len)
        } else {
            write!(f, "{}", self.fixed_len)
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct IsNotRetTy {
    value_poly: ty::Ref<ty::Poly>,
    ret_poly: ty::Ref<ty::Poly>,
    ret_ty_span: Option<Span>,
}

impl IsNotRetTy {
    pub fn new(
        value_poly: ty::Ref<ty::Poly>,
        ret_poly: ty::Ref<ty::Poly>,
        ret_ty_span: Option<Span>,
    ) -> IsNotRetTy {
        IsNotRetTy {
            value_poly,
            ret_poly,
            ret_ty_span,
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum ErrorKind {
    IsNotTy(ty::Ref<ty::Poly>, ty::Ref<ty::Poly>),
    IsNotFun(ty::Ref<ty::Poly>),
    IsNotPurity(ty::Ref<ty::Poly>, purity::Ref),
    IsNotRetTy(IsNotRetTy),
    VarHasEmptyType(ty::Ref<ty::Poly>, ty::Ref<ty::Poly>),
    TopFunApply(ty::Ref<ty::Poly>),
    RecursiveType,
    RecurWithoutFunTypeDecl,
    NonTailRecur,
    DependsOnError,
    WrongArity(usize, WantedArity),
    UnselectedPVar(purity::PVarId),
    UnselectedTVar(ty::TVarId),
}

#[derive(PartialEq, Debug, Clone)]
pub struct Error {
    loc_trace: LocTrace,
    kind: ErrorKind,
}

impl Error {
    pub fn new(span: Span, kind: ErrorKind) -> Error {
        Self::new_with_loc_trace(span.into(), kind)
    }

    pub fn new_with_loc_trace(loc_trace: LocTrace, kind: ErrorKind) -> Error {
        Error { loc_trace, kind }
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    pub fn with_macro_invocation_span(self, span: Span) -> Error {
        Error {
            loc_trace: self.loc_trace.with_macro_invocation(span),
            ..self
        }
    }
}

impl From<Error> for Diagnostic {
    fn from(error: Error) -> Diagnostic {
        let origin = error.loc_trace.origin();

        let diagnostic = match error.kind() {
            ErrorKind::IsNotFun(ref sub) => Diagnostic::new_error(format!(
                "expected function, found `{}`",
                hir::str_for_ty_ref(sub)
            ), new_label(origin,"application requires function")),

            ErrorKind::IsNotTy(ref sub, ref parent) => Diagnostic::new_error("mismatched types",new_label(origin,format!(
                    "`{}` is not a `{}`",
                    hir::str_for_ty_ref(sub),
                    hir::str_for_ty_ref(parent)
                ))),

            ErrorKind::IsNotPurity(ref fun, ref purity) => {
                use crate::ty::purity::Purity;

                let purity_str = if purity == &Purity::Pure.into() {
                    // `->` might be confusing here
                    "pure".into()
                } else {
                    format!("`{}`", hir::str_for_purity(purity))
                };

                Diagnostic::new_error("mismatched purities",
                    new_label(origin,format!(
                        "function of type `{}` is not {}",
                        hir::str_for_ty_ref(fun),
                        purity_str
                    )),
                )
            }

            ErrorKind::IsNotRetTy(IsNotRetTy {
                value_poly,
                ret_poly,
                ret_ty_span,
            }) => {
                let ret_poly_str = hir::str_for_ty_ref(ret_poly);
                let diagnostic = Diagnostic::new_error("mismatched types",
                    new_label(origin,format!(
                        "`{}` is not a `{}`",
                        hir::str_for_ty_ref(value_poly),
                        ret_poly_str
                    )),
                );

                if let Some(ret_ty_span) = ret_ty_span {
                    diagnostic.with_secondary_labels(
                        iter::once(
                        new_label(*ret_ty_span,format!(
                            "expected `{}` due to return type",
                            ret_poly_str
                        ))),
                    )
                } else {
                    diagnostic
                }
            }

            ErrorKind::VarHasEmptyType(ref current_type, ref required_type) => {
                Diagnostic::new_error("type annotation needed",
                    new_label(origin,format!(
                        "usage requires `{}` but variable has inferred type of `{}`",
                        hir::str_for_ty_ref(required_type),
                        hir::str_for_ty_ref(current_type)
                    )),
                )
            }

            ErrorKind::TopFunApply(ref top_fun) => Diagnostic::new_error(format!(
                "cannot determine parameter types for `{}`",
                hir::str_for_ty_ref(top_fun)
            ),new_label(origin,"at this application")),

            ErrorKind::WrongArity(have, ref wanted) => {
                let label_message = if wanted.fixed_len == 1 {
                    format!("expected {} argument", wanted)
                } else {
                    format!("expected {} arguments", wanted)
                };

                Diagnostic::new_error(format!(
                    "incorrect number of arguments: wanted {}, have {}",
                    wanted, have
                ),new_label(origin,label_message))
            }

            ErrorKind::RecursiveType => Diagnostic::new_error("type annotation needed",
                new_label(origin,
                    "recursive usage requires explicit type annotation")
            ),

            ErrorKind::RecurWithoutFunTypeDecl => Diagnostic::new_error("type annotation needed",
               new_label(origin,
                    "`(recur)` requires the function to have a complete type annotation"),
            ),

            ErrorKind::NonTailRecur => Diagnostic::new_error("non-tail `(recur)`",
                new_label(origin,
                    "`(recur)` must occur in a position where it immediately becomes the return value of a function"),
            ),

            ErrorKind::DependsOnError => {
                Diagnostic::new_error("type cannot be determined due to previous error",
                new_label(origin, "cannot infer type"))
            }

            ErrorKind::UnselectedPVar(pvar) => Diagnostic::new_error(format!(
                "cannot determine purity of purity variable `{}`",
                pvar.source_name()
            ),new_label(origin,"at this application"))
            .with_secondary_labels(iter::once(
                new_label(pvar.span(),"purity variable defined here"),
            )),

            ErrorKind::UnselectedTVar(tvar) => Diagnostic::new_error(format!(
                "cannot determine type of type variable `{}`",
                tvar.source_name()
            ),new_label(origin,"at this application"))
            .with_secondary_labels(iter::once(
               new_label(tvar.span(),"type variable defined here"),
            )),
        };

        error.loc_trace.label_macro_invocation(diagnostic)
    }
}

impl From<Error> for Vec<Diagnostic> {
    fn from(error: Error) -> Vec<Diagnostic> {
        vec![error.into()]
    }
}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let diagnostic: Diagnostic = self.clone().into();
        f.write_str(&diagnostic.message)
    }
}
