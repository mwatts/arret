use std::marker::PhantomData;

use abitype::{BoxedABIType, EncodeBoxedABIType};
use boxed::refs::Gc;
use boxed::{Any, BoxSize, Boxed, ConstructableFrom, Header, TypeTag, TypeTagged};

#[repr(C, align(16))]
pub struct Pair<T>
where
    T: Boxed,
{
    pub header: Header,
    pub head: Gc<T>,
    pub rest: Gc<List<T>>,
    pub list_length: usize,
}

impl<T> Boxed for Pair<T> where T: Boxed {}

impl<T> TypeTagged for Pair<T>
where
    T: Boxed,
{
    const TYPE_TAG: TypeTag = TypeTag::TopPair;
}

impl<T> EncodeBoxedABIType for Pair<T>
where
    T: EncodeBoxedABIType + Boxed,
{
    const BOXED_ABI_TYPE: BoxedABIType = BoxedABIType::Pair(&T::BOXED_ABI_TYPE);
}

type PairInput<T> = (Gc<T>, Gc<List<T>>);

impl<T> ConstructableFrom<PairInput<T>> for Pair<T>
where
    T: Boxed,
{
    fn size_for_value(_: &PairInput<T>) -> BoxSize {
        BoxSize::Size32
    }

    fn new_with_header(value: PairInput<T>, header: Header) -> Pair<T> {
        Pair {
            header,
            head: value.0,
            rest: value.1,
            list_length: value.1.list_length() + 1,
        }
    }
}

#[repr(C, align(16))]
pub struct List<T>
where
    T: Boxed,
{
    pub header: Header,
    phantom: PhantomData<T>,
}

pub enum ListSubtype<'a, T>
where
    T: Boxed + 'a,
{
    Pair(&'a Pair<T>),
    Nil,
}

impl<T> List<T>
where
    T: Boxed,
{
    pub fn as_subtype(&self) -> ListSubtype<T> {
        match self.header.type_tag {
            TypeTag::TopPair => {
                ListSubtype::Pair(unsafe { &*(self as *const List<T> as *const Pair<T>) })
            }
            TypeTag::Nil => ListSubtype::Nil,
            other => {
                unreachable!("Unexpected type tag: {:?}", other);
            }
        }
    }

    pub fn list_length(&self) -> usize {
        match self.as_subtype() {
            ListSubtype::Pair(pair) => pair.list_length,
            ListSubtype::Nil => 0,
        }
    }
}

impl<T> Boxed for List<T> where T: Boxed {}

impl<T> EncodeBoxedABIType for List<T>
where
    T: EncodeBoxedABIType + Boxed,
{
    const BOXED_ABI_TYPE: BoxedABIType = BoxedABIType::List(&T::BOXED_ABI_TYPE);
}

#[repr(C, align(16))]
pub struct TopPair {
    pub header: Header,
}

impl TopPair {
    fn as_pair(&self) -> Gc<Pair<Any>> {
        unsafe { Gc::new(&*(self as *const TopPair as *const Pair<Any>)) }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::mem;

    #[test]
    fn sizes() {
        assert_eq!(32, mem::size_of::<Pair<Any>>());
    }
}
