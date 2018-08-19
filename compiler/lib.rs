#![cfg_attr(feature = "cargo-clippy", warn(clippy))]
#![warn(rust_2018_idioms)]

#[macro_use]
mod id_type;

mod hir;
mod mir;
pub mod repl;
pub mod reporting;
mod source;
mod ty;
mod typeck;

pub use crate::hir::lowering::lower_program;
pub use crate::hir::PackagePaths;
pub use crate::source::{SourceFile, SourceFileId, SourceKind, SourceLoader};
pub use crate::typeck::infer::infer_program;
