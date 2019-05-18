use arret_syntax::datum::DataStr;
use arret_syntax::span::Span;

use crate::id_type::ArcId;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Purity {
    Pure,
    Impure,
}

#[derive(PartialEq, Debug, Clone)]
pub struct PVar {
    span: Span,
    source_name: DataStr,
}

pub type PVarId = ArcId<PVar>;
pub type PVarIds = Vec<PVarId>;

impl PVar {
    pub fn new(span: Span, source_name: DataStr) -> PVar {
        PVar { span, source_name }
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Ref {
    Fixed(Purity),
    Var(PVarId),
}

impl From<Purity> for Ref {
    fn from(purity: Purity) -> Self {
        Ref::Fixed(purity)
    }
}

impl From<PVarId> for Ref {
    fn from(pvar_id: PVarId) -> Self {
        Ref::Var(pvar_id)
    }
}
