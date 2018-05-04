use std::collections::HashMap;
use std::result;

use hir::error::{Error, ErrorKind};
use hir::loader::LibraryName;
use hir::ns::NsDatum;
use hir::scope::Binding;
use hir::util::{expect_arg_count, expect_ident, expect_ident_and_span};
use syntax::span::Span;

type Result<T> = result::Result<T, Error>;
type Bindings = HashMap<String, Binding>;

/// Input to an (import) filter
///
/// This tracks the bindings and the terminal name of the original library
struct FilterInput {
    bindings: Bindings,

    /// The terminal name of the library the bindings came from
    ///
    /// This is to support `(prefixed)`. For example, the terminal name of `[scheme base]` would be
    /// `base` and if `(prefixed)` was used it would prepend `base/` to all of its identifiers.
    terminal_name: String,
}

struct LowerImportContext<F>
where
    F: FnMut(Span, &LibraryName) -> Result<Bindings>,
{
    load_library: F,
}

impl<F> LowerImportContext<F>
where
    F: FnMut(Span, &LibraryName) -> Result<Bindings>,
{
    fn lower_library_import(&mut self, span: Span, name_data: Vec<NsDatum>) -> Result<FilterInput> {
        let mut name_components = name_data
            .into_iter()
            .map(|datum| expect_ident(datum).map(|ident| ident.into_name()))
            .collect::<Result<Vec<String>>>()?;

        let terminal_name = name_components.pop().unwrap();
        let library_name = LibraryName::new(name_components, terminal_name.clone());
        let bindings = (self.load_library)(span, &library_name)?;

        Ok(FilterInput {
            bindings,
            terminal_name,
        })
    }

    fn lower_import_filter(
        &mut self,
        apply_span: Span,
        filter_name: &str,
        filter_input: FilterInput,
        mut arg_data: Vec<NsDatum>,
    ) -> Result<FilterInput> {
        match filter_name {
            "only" => {
                let inner_bindings = filter_input.bindings;
                let only_bindings = arg_data
                    .into_iter()
                    .map(|arg_datum| {
                        let (ident, span) = expect_ident_and_span(arg_datum)?;

                        if let Some(binding) = inner_bindings.get(ident.name()) {
                            Ok((ident.into_name(), binding.clone()))
                        } else {
                            Err(Error::new(
                                span,
                                ErrorKind::UnboundSymbol(ident.into_name()),
                            ))
                        }
                    })
                    .collect::<Result<Bindings>>()?;

                Ok(FilterInput {
                    bindings: only_bindings,
                    terminal_name: filter_input.terminal_name,
                })
            }
            "except" => {
                let mut except_bindings = filter_input.bindings;
                for arg_datum in arg_data {
                    let (ident, span) = expect_ident_and_span(arg_datum)?;

                    if except_bindings.remove(ident.name()).is_none() {
                        return Err(Error::new(
                            span,
                            ErrorKind::UnboundSymbol(ident.into_name()),
                        ));
                    }
                }

                Ok(FilterInput {
                    bindings: except_bindings,
                    terminal_name: filter_input.terminal_name,
                })
            }
            "rename" => {
                expect_arg_count(apply_span, &arg_data, 1)?;
                let arg_datum = arg_data.pop().unwrap();

                if let NsDatum::Map(_, vs) = arg_datum {
                    let mut rename_bindings = filter_input.bindings;

                    for (from_datum, to_datum) in vs {
                        let (from_ident, from_span) = expect_ident_and_span(from_datum)?;
                        let to_ident = expect_ident(to_datum)?;

                        match rename_bindings.remove(from_ident.name()) {
                            Some(binding) => {
                                rename_bindings.insert(to_ident.into_name(), binding);
                            }
                            None => {
                                return Err(Error::new(
                                    from_span,
                                    ErrorKind::UnboundSymbol(from_ident.into_name()),
                                ));
                            }
                        }
                    }

                    Ok(FilterInput {
                        bindings: rename_bindings,
                        terminal_name: filter_input.terminal_name,
                    })
                } else {
                    Err(Error::new(
                        arg_datum.span(),
                        ErrorKind::IllegalArg(
                            "(rename) expects a map of identifier renames".to_owned(),
                        ),
                    ))
                }
            }
            "prefix" => {
                expect_arg_count(apply_span, &arg_data, 1)?;
                let prefix_ident = expect_ident(arg_data.pop().unwrap())?;

                let prefix_bindings = filter_input
                    .bindings
                    .into_iter()
                    .map(|(name, binding)| (format!("{}{}", prefix_ident.name(), name), binding))
                    .collect();

                Ok(FilterInput {
                    bindings: prefix_bindings,
                    terminal_name: filter_input.terminal_name,
                })
            }
            "prefixed" => {
                expect_arg_count(apply_span, &arg_data, 0)?;
                let FilterInput {
                    bindings,
                    terminal_name,
                } = filter_input;

                let prefixed_bindings = bindings
                    .into_iter()
                    .map(|(name, binding)| (format!("{}/{}", &terminal_name, name), binding))
                    .collect();

                Ok(FilterInput {
                    bindings: prefixed_bindings,
                    terminal_name,
                })
            }
            _ => Err(Error::new(
                apply_span,
                ErrorKind::IllegalArg(
                    "unknown import filter; must be `only`, `except`, `rename`, `prefix` or `prefixed`"
                        .to_owned(),
                ),
            )),
        }
    }

    fn lower_import_set(&mut self, import_set_datum: NsDatum) -> Result<FilterInput>
    where
        F: FnMut(Span, &LibraryName) -> Result<Bindings>,
    {
        let span = import_set_datum.span();
        match import_set_datum {
            NsDatum::Vec(_, vs) => {
                if vs.is_empty() {
                    return Err(Error::new(
                        span,
                        ErrorKind::IllegalArg(
                            "library name requires a least one element".to_owned(),
                        ),
                    ));
                }

                return self.lower_library_import(span, vs);
            }
            NsDatum::List(_, mut vs) => {
                // Each filter requires a filter identifier and an inner import set
                if vs.len() >= 2 {
                    let arg_data = vs.split_off(2);
                    let inner_import_datum = vs.pop().unwrap();
                    let filter_ident = expect_ident(vs.pop().unwrap())?;

                    let filter_input = self.lower_import_set(inner_import_datum)?;
                    return self.lower_import_filter(
                        span,
                        filter_ident.name(),
                        filter_input,
                        arg_data,
                    );
                }
            }
            _ => {}
        }

        Err(Error::new(
            span,
            ErrorKind::IllegalArg(
                "import set must either be a library name vector or an applied filter".to_owned(),
            ),
        ))
    }
}

pub fn lower_import_set<F>(import_set_datum: NsDatum, load_library: F) -> Result<Bindings>
where
    F: FnMut(Span, &LibraryName) -> Result<Bindings>,
{
    let mut lic = LowerImportContext { load_library };
    lic.lower_import_set(import_set_datum)
        .map(|filter_input| filter_input.bindings)
}

#[cfg(test)]
mod test {
    use super::*;
    use hir::ns::NsId;
    use hir::prim::Prim;
    use syntax::span::{t2s, EMPTY_SPAN};

    fn load_test_library(_: Span, library_name: &LibraryName) -> Result<Bindings> {
        if library_name == &LibraryName::new(vec!["lib".to_owned()], "test".to_owned()) {
            let mut bindings = HashMap::new();
            bindings.insert("quote".to_owned(), Binding::Prim(Prim::Quote));
            bindings.insert("if".to_owned(), Binding::Prim(Prim::If));

            Ok(bindings)
        } else {
            Err(Error::new(EMPTY_SPAN, ErrorKind::LibraryNotFound))
        }
    }

    fn bindings_for_import_set(datum: &str) -> Result<HashMap<String, Binding>> {
        use syntax::parser::datum_from_str;

        let test_ns_id = NsId::new(0);

        let import_set_datum =
            NsDatum::from_syntax_datum(test_ns_id, datum_from_str(datum).unwrap());

        lower_import_set(import_set_datum, load_test_library)
    }

    #[test]
    fn basic_import() {
        let j = "[lib test]";
        let bindings = bindings_for_import_set(j).unwrap();

        assert_eq!(bindings["quote"], Binding::Prim(Prim::Quote));
        assert_eq!(bindings["if"], Binding::Prim(Prim::If));
    }

    #[test]
    fn library_not_found() {
        let j = "[not found]";
        let err = Error::new(EMPTY_SPAN, ErrorKind::LibraryNotFound);

        assert_eq!(err, bindings_for_import_set(j).unwrap_err());
    }

    #[test]
    fn only_filter() {
        let j = "(only [lib test] quote)";
        let bindings = bindings_for_import_set(j).unwrap();

        assert_eq!(bindings["quote"], Binding::Prim(Prim::Quote));
        assert_eq!(false, bindings.contains_key("if"));

        let j = "(only [lib test] quote ifz)";
        let t = "                       ^^^ ";
        let err = Error::new(t2s(t), ErrorKind::UnboundSymbol("ifz".to_owned()));

        assert_eq!(err, bindings_for_import_set(j).unwrap_err());
    }

    #[test]
    fn except_filter() {
        let j = "(except [lib test] if)";
        let bindings = bindings_for_import_set(j).unwrap();

        assert_eq!(bindings["quote"], Binding::Prim(Prim::Quote));
        assert_eq!(false, bindings.contains_key("if"));

        let j = "(except [lib test] ifz)";
        let t = "                   ^^^ ";
        let err = Error::new(t2s(t), ErrorKind::UnboundSymbol("ifz".to_owned()));

        assert_eq!(err, bindings_for_import_set(j).unwrap_err());
    }

    #[test]
    fn rename_filter() {
        let j = "(rename [lib test] {quote new-quote, if new-if})";
        let bindings = bindings_for_import_set(j).unwrap();

        assert_eq!(bindings["new-quote"], Binding::Prim(Prim::Quote));
        assert_eq!(bindings["new-if"], Binding::Prim(Prim::If));

        let j = "(rename [lib test] {ifz new-ifz})";
        let t = "                    ^^^          ";
        let err = Error::new(t2s(t), ErrorKind::UnboundSymbol("ifz".to_owned()));

        assert_eq!(err, bindings_for_import_set(j).unwrap_err());
    }

    #[test]
    fn prefix_filter() {
        let j = "(prefix [lib test] new-)";
        let bindings = bindings_for_import_set(j).unwrap();

        assert_eq!(bindings["new-quote"], Binding::Prim(Prim::Quote));
        assert_eq!(bindings["new-if"], Binding::Prim(Prim::If));
    }

    #[test]
    fn prefixed_filter() {
        let j = "(prefixed [lib test])";
        let bindings = bindings_for_import_set(j).unwrap();

        assert_eq!(bindings["test/quote"], Binding::Prim(Prim::Quote));
        assert_eq!(bindings["test/if"], Binding::Prim(Prim::If));
    }
}