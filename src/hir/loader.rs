use std::path::PathBuf;
use std::fs::File;
use std::io::prelude::*;

use syntax::datum::Datum;
use syntax::span::Span;
use syntax::parser::data_from_str_with_span_offset;
use hir::error::{Error, ErrorKind, Result};
use ctx::{CompileContext, LoadedFile};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct LibraryName {
    path: Vec<String>,
    terminal_name: String,
}

impl LibraryName {
    pub fn new(path: Vec<String>, terminal_name: String) -> LibraryName {
        LibraryName {
            path,
            terminal_name,
        }
    }
}

pub fn load_module_data(
    ccx: &mut CompileContext,
    span: Span,
    display_name: String,
    input_reader: &mut Read,
) -> Result<Vec<Datum>> {
    let span_offset = ccx.next_span_offset();

    let mut source = String::new();

    input_reader
        .read_to_string(&mut source)
        .map_err(|_| Error::new(span, ErrorKind::ReadError(display_name.clone())))?;

    let data = data_from_str_with_span_offset(&source, span_offset);

    // Add a space to allow us to position errors at EOF
    source.push(' ');

    // Track this file for diagnostic reporting
    ccx.add_loaded_file(LoadedFile::new(display_name, source));

    Ok(data?)
}

pub fn load_library_data(
    ccx: &mut CompileContext,
    span: Span,
    library_name: &LibraryName,
) -> Result<Vec<Datum>> {
    let mut path_buf = PathBuf::new();

    path_buf.push("stdlib");
    for path_component in library_name.path.iter() {
        path_buf.push(path_component);
    }

    path_buf.push(format!("{}.rsp", library_name.terminal_name));

    let display_name = path_buf.to_string_lossy().into_owned();
    let mut source_file =
        File::open(path_buf).map_err(|_| Error::new(span, ErrorKind::LibraryNotFound))?;

    load_module_data(ccx, span, display_name, &mut source_file)
}
