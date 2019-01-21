use runtime::abitype;
use runtime::boxed::TypeTag;

use crate::mir::tagset::TypeTagSet;
use crate::mir::value::Value;
use crate::ty;

fn specific_boxed_abi_type_for_type_tag(type_tag: TypeTag) -> abitype::BoxedABIType {
    match type_tag {
        TypeTag::TopPair => abitype::BoxedABIType::Pair(&abitype::BoxedABIType::Any),
        TypeTag::TopVector => abitype::BoxedABIType::Vector(&abitype::BoxedABIType::Any),
        other_tag => abitype::BoxedABIType::DirectTagged(other_tag),
    }
}
fn specific_abi_type_for_type_tag(type_tag: TypeTag) -> abitype::ABIType {
    match type_tag {
        TypeTag::Int => abitype::ABIType::Int,
        TypeTag::Float => abitype::ABIType::Float,
        TypeTag::Char => abitype::ABIType::Char,
        other_tag => specific_boxed_abi_type_for_type_tag(other_tag).into(),
    }
}

fn specific_boxed_abi_type_for_type_tags(possible_type_tags: TypeTagSet) -> abitype::BoxedABIType {
    use runtime::abitype::EncodeBoxedABIType;
    use runtime::boxed;

    if possible_type_tags.len() == 1 {
        let single_type_tag = possible_type_tags.into_iter().next().unwrap();
        specific_boxed_abi_type_for_type_tag(single_type_tag)
    } else if possible_type_tags == [TypeTag::TopPair, TypeTag::Nil].iter().collect() {
        boxed::TopList::BOXED_ABI_TYPE
    } else if possible_type_tags == [TypeTag::Float, TypeTag::Int].iter().collect() {
        boxed::Num::BOXED_ABI_TYPE
    } else if possible_type_tags == [TypeTag::True, TypeTag::False].iter().collect() {
        boxed::Bool::BOXED_ABI_TYPE
    } else {
        abitype::BoxedABIType::Any
    }
}

fn specific_abi_type_for_type_tags(possible_type_tags: TypeTagSet) -> abitype::ABIType {
    if possible_type_tags.len() == 1 {
        let single_type_tag = possible_type_tags.into_iter().next().unwrap();
        specific_abi_type_for_type_tag(single_type_tag)
    } else if possible_type_tags == [TypeTag::True, TypeTag::False].iter().collect() {
        abitype::ABIType::Bool
    } else {
        specific_boxed_abi_type_for_type_tags(possible_type_tags).into()
    }
}

/// Returns a specific ABI type to encode the given mono type
pub fn specific_abi_type_for_mono(mono: &ty::Mono) -> abitype::ABIType {
    specific_abi_type_for_type_tags(mono.into())
}

fn specific_type_for_values<'v, F, T>(
    possible_values: impl Iterator<Item = &'v Value>,
    tagset_to_type: F,
) -> T
where
    F: FnOnce(TypeTagSet) -> T,
{
    use crate::mir::value::types::possible_type_tags_for_value;

    let possible_type_tags = possible_values
        .map(possible_type_tags_for_value)
        .fold(TypeTagSet::new(), |acc, type_tags| acc | type_tags);

    tagset_to_type(possible_type_tags)
}

/// Returns a specific boxed ABI type to encode the given set of possible values
pub fn specific_boxed_abi_type_for_values<'v>(
    possible_values: impl Iterator<Item = &'v Value>,
) -> abitype::BoxedABIType {
    specific_type_for_values(possible_values, specific_boxed_abi_type_for_type_tags)
}

/// Returns a specific ABI type to compactly encode the given set of possible values
pub fn specific_abi_type_for_values<'v>(
    possible_values: impl Iterator<Item = &'v Value>,
) -> abitype::ABIType {
    specific_type_for_values(possible_values, specific_abi_type_for_type_tags)
}

/// Return a specific ABI type to compactly encode the given value
pub fn specific_abi_type_for_value(value: &Value) -> abitype::ABIType {
    specific_abi_type_for_values(std::iter::once(value))
}