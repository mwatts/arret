use std::sync::Arc;

use crate::span::Span;

pub type DataStr = Arc<str>;

#[derive(PartialEq, Debug, Clone)]
pub enum Datum {
    Bool(Span, bool),
    Char(Span, char),
    Int(Span, i64),
    Float(Span, f64),
    List(Span, Box<[Datum]>),
    Str(Span, DataStr),
    Sym(Span, DataStr),
    Vector(Span, Box<[Datum]>),
    Map(Span, Box<[(Datum, Datum)]>),
    Set(Span, Box<[Datum]>),
}

impl Datum {
    pub fn span(&self) -> Span {
        match self {
            Datum::Bool(span, _)
            | Datum::Char(span, _)
            | Datum::Int(span, _)
            | Datum::Float(span, _)
            | Datum::List(span, _)
            | Datum::Str(span, _)
            | Datum::Sym(span, _)
            | Datum::Vector(span, _)
            | Datum::Map(span, _)
            | Datum::Set(span, _) => *span,
        }
    }
}
