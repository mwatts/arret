use hir;
use syntax::span::Span;
use ty;

#[derive(Debug, PartialEq)]
pub enum Destruc {
    Scalar(Span, Scalar),
    List(Span, List),
}

#[derive(Debug, PartialEq)]
pub struct List {
    fixed: Vec<Destruc>,
    rest: Option<Box<Scalar>>,
}

impl List {
    pub fn new(fixed: Vec<Destruc>, rest: Option<Box<Scalar>>) -> List {
        List { fixed, rest }
    }
}

#[derive(Debug, PartialEq)]
pub struct Scalar {
    /// ID of the variable. If this is None it's treated as a wildcard.
    var_id: Option<hir::VarId>,
    source_name: String,
    ty: ty::Decl,
}

impl Scalar {
    pub fn new(var_id: Option<hir::VarId>, source_name: String, ty: ty::Decl) -> Scalar {
        Scalar {
            var_id,
            source_name,
            ty,
        }
    }
}