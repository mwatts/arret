mod matcher;
mod expander;
mod linkvars;

use std::collections::{HashMap, HashSet};

use syntax::span::Span;
use hir::scope::{Binding, Ident, NsIdAlloc, NsValue, Prim, Scope};
use hir::error::{Error, Result};
use hir::macros::matcher::match_rule;
use hir::macros::expander::expand_rule;
use hir::macros::linkvars::{link_vars, VarLinks};

#[derive(PartialEq, Eq, Debug, Hash)]
pub enum MacroVar {
    Bound(Binding),
    Unbound(String),
}

impl MacroVar {
    fn from_ident(scope: &Scope, ident: &Ident) -> MacroVar {
        match scope.get(ident) {
            Some(binding) => MacroVar::Bound(binding),
            None => MacroVar::Unbound(ident.name().clone()),
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct SpecialVars {
    zero_or_more: Option<MacroVar>,
    literals: HashSet<MacroVar>,
}

impl SpecialVars {
    fn is_wildcard(&self, var: &MacroVar) -> bool {
        *var == MacroVar::Bound(Binding::Prim(Prim::Wildcard))
    }

    fn is_literal(&self, var: &MacroVar) -> bool {
        self.literals.contains(var)
    }

    fn is_zero_or_more(&self, var: &MacroVar) -> bool {
        if let Some(ref zero_or_more) = self.zero_or_more {
            zero_or_more == var
        } else {
            false
        }
    }

    fn starts_with_zero_or_more(&self, scope: &Scope, data: &[NsValue]) -> bool {
        if let Some(&NsValue::Ident(_, ref next_ident)) = data.get(1) {
            let next_var = MacroVar::from_ident(scope, next_ident);
            self.is_zero_or_more(&next_var)
        } else {
            false
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct Rule {
    pattern: Vec<NsValue>,
    template: NsValue,
    var_links: VarLinks,
}

#[derive(PartialEq, Debug)]
pub struct Macro {
    special_vars: SpecialVars,
    rules: Vec<Rule>,
}

#[derive(Debug)]
pub struct MatchData {
    vars: HashMap<MacroVar, NsValue>,
    // The outside vector is the subpatterns; the inside vector contains the zero or more matches
    subpatterns: Vec<Vec<MatchData>>,
}

impl MatchData {
    fn new() -> MatchData {
        MatchData {
            vars: HashMap::new(),
            subpatterns: vec![],
        }
    }
}

impl Macro {
    pub fn new(special_vars: SpecialVars, rules: Vec<Rule>) -> Macro {
        Macro {
            special_vars,
            rules,
        }
    }
}

pub fn lower_macro_rule(
    scope: &Scope,
    self_ident: &Ident,
    special_vars: &SpecialVars,
    rule_datum: NsValue,
) -> Result<Rule> {
    let (span, mut rule_values) = if let NsValue::Vector(span, vs) = rule_datum {
        (span, vs)
    } else {
        return Err(Error::IllegalArg(
            rule_datum.span(),
            "Expected a macro rule vector".to_owned(),
        ));
    };

    if rule_values.len() != 2 {
        return Err(Error::IllegalArg(
            span,
            "Expected a macro rule vector with two elements".to_owned(),
        ));
    }

    let template = rule_values.pop().unwrap();
    let pattern_data = rule_values.pop().unwrap();

    let pattern = if let NsValue::List(span, mut vs) = pattern_data {
        if vs.len() < 1 {
            return Err(Error::IllegalArg(
                span,
                "Macro rule patterns must contain at least the name of the macro".to_owned(),
            ));
        }

        match vs.remove(0) {
            NsValue::Ident(_, ref ident) if ident.name() == self_ident.name() => {}
            other => {
                return Err(Error::IllegalArg(
                    other.span(),
                    "Macro rule patterns must start with the name of the macro".to_owned(),
                ));
            }
        }

        vs
    } else {
        return Err(Error::IllegalArg(
            pattern_data.span(),
            "Expected a macro rule pattern list".to_owned(),
        ));
    };

    let var_links = link_vars(scope, special_vars, pattern.as_slice(), &template)?;

    Ok(Rule {
        pattern,
        template,
        var_links,
    })
}

pub fn lower_macro_rules(
    scope: &Scope,
    span: Span,
    self_ident: &Ident,
    mut macro_rules_data: Vec<NsValue>,
) -> Result<Macro> {
    if macro_rules_data.len() != 2 {
        return Err(Error::WrongArgCount(span, 2));
    }

    let rules_datum = macro_rules_data.pop().unwrap();
    let literals_datum = macro_rules_data.pop().unwrap();

    let literals = if let NsValue::Set(_, vs) = literals_datum {
        vs.into_iter()
            .map(|v| {
                if let NsValue::Ident(_, ref ident) = v {
                    Ok(MacroVar::from_ident(scope, ident))
                } else {
                    Err(Error::IllegalArg(
                        v.span(),
                        "Pattern literal must be a symbol".to_owned(),
                    ))
                }
            })
            .collect::<Result<HashSet<MacroVar>>>()?
    } else {
        return Err(Error::IllegalArg(
            literals_datum.span(),
            "Expected set of pattern literals".to_owned(),
        ));
    };

    let rules_values = if let NsValue::Vector(_, vs) = rules_datum {
        vs
    } else {
        return Err(Error::IllegalArg(
            rules_datum.span(),
            "Expected a vector of syntax rules".to_owned(),
        ));
    };

    let default_zero_or_more = MacroVar::Bound(Binding::Prim(Prim::Ellipsis));
    let zero_or_more = if literals.contains(&default_zero_or_more) {
        None
    } else {
        Some(default_zero_or_more)
    };

    let special_vars = SpecialVars {
        zero_or_more,
        literals,
    };

    let rules = rules_values
        .into_iter()
        .map(|rule_datum| lower_macro_rule(scope, self_ident, &special_vars, rule_datum))
        .collect::<Result<Vec<Rule>>>()?;

    Ok(Macro::new(special_vars, rules))
}

pub fn expand_macro(
    ns_id_alloc: &mut NsIdAlloc,
    scope: &Scope,
    span: Span,
    mac: &Macro,
    arg_data: Vec<NsValue>,
) -> Result<(Scope, NsValue)> {
    for rule in mac.rules.iter() {
        let match_result = match_rule(scope, &mac.special_vars, rule, arg_data.as_slice());

        if let Ok(match_data) = match_result {
            return Ok(expand_rule(
                ns_id_alloc,
                scope,
                &mac.special_vars,
                match_data,
                &rule.var_links,
                &rule.template,
            ));
        }
    }

    Err(Error::NoMacroRule(span))
}

#[cfg(test)]
use hir::scope::NsId;
#[cfg(test)]
use syntax::parser::data_from_str;
#[cfg(test)]
use syntax::span::t2s;

#[cfg(test)]
fn test_ns_id() -> NsId {
    NsId::new(0)
}

#[cfg(test)]
fn macro_rules_for_str(data_str: &str) -> Result<Macro> {
    let test_data = data_from_str(data_str).unwrap();

    let full_span = Span {
        lo: 0,
        hi: data_str.len() as u32,
    };

    let test_ns_data = test_data
        .into_iter()
        .map(|datum| NsValue::from_value(datum, test_ns_id()))
        .collect::<Vec<NsValue>>();

    let self_ident = Ident::new(test_ns_id(), "self".to_owned());
    lower_macro_rules(&Scope::new_empty(), full_span, &self_ident, test_ns_data)
}

#[test]
fn wrong_arg_count() {
    let j = "#{} [] []";
    let t = "^^^^^^^^^";

    let err = Error::WrongArgCount(t2s(t), 2);
    assert_eq!(err, macro_rules_for_str(j).unwrap_err());
}

#[test]
fn non_set_literals() {
    let j = "[] []";
    let t = "^^   ";

    let err = Error::IllegalArg(t2s(t), "Expected set of pattern literals".to_owned());
    assert_eq!(err, macro_rules_for_str(j).unwrap_err());
}

#[test]
fn non_symbol_literal() {
    let j = "#{one 2} []";
    let t = "      ^    ";

    let err = Error::IllegalArg(t2s(t), "Pattern literal must be a symbol".to_owned());
    assert_eq!(err, macro_rules_for_str(j).unwrap_err());
}

#[test]
fn empty_rules() {
    let j = "#{} []";

    let special_vars = SpecialVars {
        zero_or_more: Some(MacroVar::Bound(Binding::Prim(Prim::Ellipsis))),
        literals: HashSet::new(),
    };

    let expected = Macro::new(special_vars, vec![]);
    assert_eq!(expected, macro_rules_for_str(j).unwrap());
}

#[test]
fn rule_with_non_vector() {
    let j = "#{} [1]";
    let t = "     ^ ";

    let err = Error::IllegalArg(t2s(t), "Expected a macro rule vector".to_owned());
    assert_eq!(err, macro_rules_for_str(j).unwrap_err());
}

#[test]
fn rule_with_not_enough_elements() {
    let j = "#{} [[(self)]]";
    let t = "     ^^^^^^^^ ";

    let err = Error::IllegalArg(
        t2s(t),
        "Expected a macro rule vector with two elements".to_owned(),
    );
    assert_eq!(err, macro_rules_for_str(j).unwrap_err());
}

#[test]
fn rule_with_non_list_pattern() {
    let j = "#{} [[self 1]]";
    let t = "      ^^^^    ";

    let err = Error::IllegalArg(t2s(t), "Expected a macro rule pattern list".to_owned());
    assert_eq!(err, macro_rules_for_str(j).unwrap_err());
}

#[test]
fn rule_with_empty_pattern_list() {
    let j = "#{} [[() 1]]";
    let t = "      ^^    ";

    let err = Error::IllegalArg(
        t2s(t),
        "Macro rule patterns must contain at least the name of the macro".to_owned(),
    );
    assert_eq!(err, macro_rules_for_str(j).unwrap_err());
}

#[test]
fn rule_with_non_self_pattern() {
    let j = "#{} [[(notself) 1]]";
    let t = "       ^^^^^^^     ";

    let err = Error::IllegalArg(
        t2s(t),
        "Macro rule patterns must start with the name of the macro".to_owned(),
    );
    assert_eq!(err, macro_rules_for_str(j).unwrap_err());
}
