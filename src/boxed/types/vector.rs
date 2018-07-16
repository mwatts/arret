use boxed::refs::Gc;
use boxed::{Any, BoxSize, Boxed, ConstructableFrom, Header, TypeTag, TypeTagged};

#[repr(C, align(16))]
pub struct Vector<T>
where
    T: Boxed,
{
    pub header: Header,
    pub values: Vec<Gc<T>>,
}

impl<T> Boxed for Vector<T> where T: Boxed {}

impl<T> TypeTagged for Vector<T>
where
    T: Boxed,
{
    const TYPE_TAG: TypeTag = TypeTag::TopVector;
}

impl<'a, T> ConstructableFrom<&'a [Gc<T>]> for Vector<T>
where
    T: Boxed,
{
    fn size_for_value(_: &&[Gc<T>]) -> BoxSize {
        BoxSize::Size32
    }

    fn new_with_header(values: &[Gc<T>], header: Header) -> Vector<T> {
        Vector {
            header,
            values: values.into(),
        }
    }
}

#[repr(C, align(16))]
pub struct TopVector {
    pub header: Header,
}

impl TopVector {
    fn as_vector(&self) -> &Vector<Any> {
        unsafe { &*(self as *const TopVector as *const Vector<Any>) }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::mem;

    #[test]
    fn sizes() {
        assert_eq!(32, mem::size_of::<Vector<Any>>());
    }
}
