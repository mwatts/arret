use std::hash::{Hash, Hasher};
use std::iter::FusedIterator;
use std::marker::PhantomData;
use std::{fmt, mem};

use crate::abitype::{BoxedABIType, EncodeBoxedABIType};
use crate::boxed::refs::Gc;
use crate::boxed::*;
use crate::intern::Interner;

#[repr(C, align(16))]
pub struct Pair<T: Boxed = Any> {
    header: Header,
    list_length: usize,
    pub(crate) head: Gc<T>,
    pub(crate) rest: Gc<List<T>>,
}

impl<T: Boxed> Boxed for Pair<T> {
    fn header(&self) -> Header {
        self.header
    }
}

impl<T: Boxed> EncodeBoxedABIType for Pair<T>
where
    T: EncodeBoxedABIType,
{
    const BOXED_ABI_TYPE: BoxedABIType = BoxedABIType::Pair(&T::BOXED_ABI_TYPE);
}

impl<T: Boxed> Pair<T> {
    pub fn size() -> BoxSize {
        // TODO: It'd be nice to expose this as const BOX_SIZE: BoxSize once `if` is allowed in
        // const contexts
        if mem::size_of::<Self>() == 16 {
            BoxSize::Size16
        } else if mem::size_of::<Self>() == 32 {
            BoxSize::Size32
        } else {
            unreachable!("Unsupported pair size!")
        }
    }

    pub fn len(&self) -> usize {
        self.list_length
    }

    pub fn is_empty(&self) -> bool {
        // This is to make Clippy happy since we have `len`
        false
    }

    pub fn head(&self) -> Gc<T> {
        self.head
    }

    pub fn rest(&self) -> Gc<List<T>> {
        self.rest
    }

    pub fn as_list_ref(&self) -> Gc<List<T>> {
        unsafe { Gc::new(&*(self as *const _ as *const List<T>)) }
    }
}

impl<T> PartialEq for Pair<T>
where
    T: Boxed + PartialEq,
{
    fn eq(&self, rhs: &Pair<T>) -> bool {
        (self.head == rhs.head) && (self.rest == rhs.rest)
    }
}

impl<T> Hash for Pair<T>
where
    T: Boxed + Hash,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        TypeTag::Pair.hash(state);
        self.head().hash(state);
        self.rest().hash(state);
    }
}

impl<T> fmt::Debug for Pair<T>
where
    T: Boxed + fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        self.as_list_ref().fmt(formatter)
    }
}

type PairInput<T> = (Gc<T>, Gc<List<T>>);

impl<T: Boxed> ConstructableFrom<PairInput<T>> for Pair<T> {
    fn size_for_value(_: &PairInput<T>) -> BoxSize {
        Self::size()
    }

    fn construct(value: PairInput<T>, alloc_type: AllocType, _: &mut Interner) -> Pair<T> {
        Pair {
            header: Header {
                type_tag: TypeTag::Pair,
                alloc_type,
            },
            head: value.0,
            rest: value.1,
            list_length: value.1.len() + 1,
        }
    }
}

#[repr(C, align(16))]
pub struct List<T: Boxed = Any> {
    header: Header,
    list_length: usize,
    phantom: PhantomData<T>,
}

pub enum ListSubtype<'a, T: Boxed>
where
    T: 'a,
{
    Pair(&'a Pair<T>),
    Nil,
}

impl<T: Boxed> List<T> {
    /// Creates a new fixed sized list containing the passed `elems`
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        heap: &mut impl AsHeap,
        elems: impl DoubleEndedIterator<Item = Gc<T>>,
    ) -> Gc<List<T>> {
        Self::new_with_tail(heap, elems, Self::empty())
    }

    /// Creates a list with a head of `elems` and the specified tail list
    pub fn new_with_tail(
        heap: &mut impl AsHeap,
        elems: impl DoubleEndedIterator<Item = Gc<T>>,
        tail: Gc<List<T>>,
    ) -> Gc<List<T>> {
        // TODO: This is naive; we could use a single multi-cell allocation instead
        elems.rfold(tail, |tail, elem| {
            Pair::new(heap, (elem, tail)).as_list_ref()
        })
    }

    /// Creates a list from the passed element constructor input
    ///
    /// This can potentially be faster than constructing the list and elements separately.
    pub fn from_values<V>(heap: &mut impl AsHeap, values: impl Iterator<Item = V>) -> Gc<List<T>>
    where
        T: ConstructableFrom<V>,
    {
        let elems = values.map(|v| T::new(heap, v)).collect::<Vec<Gc<T>>>();
        Self::new(heap, elems.into_iter())
    }

    pub fn empty() -> Gc<List<T>> {
        unsafe { Gc::new(&NIL_INSTANCE as *const Nil as *const List<T>) }
    }

    pub fn as_subtype(&self) -> ListSubtype<'_, T> {
        match self.header.type_tag {
            TypeTag::Pair => {
                ListSubtype::Pair(unsafe { &*(self as *const List<T> as *const Pair<T>) })
            }
            TypeTag::Nil => ListSubtype::Nil,
            other => {
                unreachable!("Unexpected type tag: {:?}", other);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.list_length
    }

    pub fn is_empty(&self) -> bool {
        self.header.type_tag == TypeTag::Nil
    }

    pub fn iter(&self) -> ListIterator<T> {
        ListIterator {
            head: unsafe { Gc::new(self as *const Self) },
        }
    }
}

impl<T> PartialEq for List<T>
where
    T: Boxed + PartialEq,
{
    fn eq(&self, other: &List<T>) -> bool {
        self.iter().eq(other.iter())
    }
}

impl<T> Hash for List<T>
where
    T: Boxed + Hash,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self.as_subtype() {
            ListSubtype::Pair(pair) => pair.hash(state),
            ListSubtype::Nil => NIL_INSTANCE.hash(state),
        }
    }
}

impl<T> fmt::Debug for List<T>
where
    T: Boxed + fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        formatter.write_str("List(")?;
        formatter.debug_list().entries(self.iter()).finish()?;
        formatter.write_str(")")
    }
}

impl<T: Boxed> Boxed for List<T> {
    fn header(&self) -> Header {
        self.header
    }
}

impl DistinctTagged for List<Any> {
    fn has_tag(type_tag: TypeTag) -> bool {
        [TypeTag::Pair, TypeTag::Nil].contains(&type_tag)
    }
}

impl<T: Boxed> EncodeBoxedABIType for List<T>
where
    T: EncodeBoxedABIType,
{
    const BOXED_ABI_TYPE: BoxedABIType = BoxedABIType::List(&T::BOXED_ABI_TYPE);
}

pub struct ListIterator<T: Boxed> {
    head: Gc<List<T>>,
}

impl<T: Boxed> Iterator for ListIterator<T> {
    type Item = Gc<T>;

    fn next(&mut self) -> Option<Gc<T>> {
        // If we use `head` directly the borrow checker gets suspicious
        let head = unsafe { &*(self.head.as_ptr()) };

        match head.as_subtype() {
            ListSubtype::Pair(pair) => {
                self.head = pair.rest;
                Some(pair.head)
            }
            ListSubtype::Nil => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.head.len(), Some(self.head.len()))
    }
}

impl<T: Boxed> ExactSizeIterator for ListIterator<T> {}
impl<T: Boxed> FusedIterator for ListIterator<T> {}

#[repr(C, align(16))]
#[derive(Debug)]
pub struct Nil {
    header: Header,
    list_length: usize,
}

#[export_name = "ARRET_NIL"]
pub static NIL_INSTANCE: Nil = Nil {
    header: Header {
        type_tag: TypeTag::Nil,
        alloc_type: AllocType::Const,
    },
    list_length: 0,
};

impl Boxed for Nil {
    fn header(&self) -> Header {
        self.header
    }
}

impl UniqueTagged for Nil {}

impl PartialEq for Nil {
    fn eq(&self, _: &Nil) -> bool {
        true
    }
}

impl Hash for Nil {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Self::TYPE_TAG.hash(state);
        state.write_usize(&NIL_INSTANCE as *const _ as usize);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::boxed::heap::Heap;
    use crate::boxed::Int;
    use std::mem;

    #[test]
    fn sizes() {
        assert_eq!(16, mem::size_of::<Nil>());
        assert_eq!(16, mem::size_of::<List<Any>>());

        #[cfg(target_pointer_width = "64")]
        assert_eq!(32, mem::size_of::<Pair<Any>>());

        // We should be able to back in to 16 bytes on 32bit
        #[cfg(target_pointer_width = "32")]
        assert_eq!(16, mem::size_of::<Pair<Any>>());
    }

    #[test]
    fn equality() {
        use crate::boxed::Int;

        let mut heap = Heap::empty();

        let forward_list1 = List::<Int>::from_values(&mut heap, [1, 2, 3].iter().cloned());
        let forward_list2 = List::<Int>::from_values(&mut heap, [1, 2, 3].iter().cloned());
        let reverse_list = List::<Int>::from_values(&mut heap, [3, 2, 1].iter().cloned());

        assert_ne!(forward_list1, reverse_list);
        assert_eq!(forward_list1, forward_list2);
    }

    #[test]
    fn fmt_debug() {
        let mut heap = Heap::empty();
        let forward_list = List::<Int>::from_values(&mut heap, [1, 2, 3].iter().cloned());

        assert_eq!(
            "List([Int(1), Int(2), Int(3)])",
            format!("{:?}", forward_list)
        );
    }

    #[test]
    fn construct_and_iter() {
        let mut heap = Heap::empty();

        let boxed_list = List::<Int>::from_values(&mut heap, [1, 2, 3].iter().cloned());

        let mut boxed_list_iter = boxed_list.iter();
        assert_eq!(3, boxed_list_iter.len());

        for expected_num in &[1, 2, 3] {
            if let Some(boxed_int) = boxed_list_iter.next() {
                assert_eq!(*expected_num, boxed_int.value());
            } else {
                panic!("Iterator unexpectedly ended");
            }
        }

        assert_eq!(0, boxed_list_iter.len());
        assert_eq!(false, boxed_list_iter.next().is_some());
    }
}
