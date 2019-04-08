use std::hash::{Hash, Hasher};
use std::{fmt, marker, mem};

use crate::abitype::{BoxedABIType, EncodeBoxedABIType};
use crate::boxed::refs::Gc;
use crate::boxed::{AllocType, Any, AsHeap, Boxed, Header, Heap, TypeTag};

const MAX_16BYTE_INLINE_LENGTH: usize = ((16 - 8) / mem::size_of::<Gc<Any>>());
const MAX_32BYTE_INLINE_LENGTH: usize = ((32 - 8) / mem::size_of::<Gc<Any>>());

const MAX_INLINE_LENGTH: usize = MAX_32BYTE_INLINE_LENGTH;

#[repr(C, align(16))]
pub struct Vector<T: Boxed = Any> {
    header: Header,
    inline_length: u32,
    padding: [u8; 24],
    phantom: marker::PhantomData<T>,
}

impl<T: Boxed> Boxed for Vector<T> {}

impl<T: Boxed> Vector<T> {
    pub fn new(heap: &mut impl AsHeap, values: &[Gc<T>]) -> Gc<Vector<T>> {
        let alloc_type = if values.len() <= MAX_16BYTE_INLINE_LENGTH {
            // 1 cell inline
            AllocType::Heap16
        } else {
            // 2 cell inline or large
            AllocType::Heap32
        };

        let header = Header {
            type_tag: TypeTag::Vector,
            alloc_type,
        };

        let boxed = unsafe {
            if values.len() <= MAX_INLINE_LENGTH {
                let mut inline_vec: InlineVector<T> = InlineVector {
                    header,
                    inline_length: values.len() as u32,
                    values: mem::uninitialized(),
                };
                inline_vec.values[0..values.len()].copy_from_slice(values);

                mem::transmute(inline_vec)
            } else {
                let large_vec = LargeVector {
                    header,
                    inline_length: (MAX_INLINE_LENGTH + 1) as u32,
                    values: values.into(),
                };

                mem::transmute(large_vec)
            }
        };

        heap.as_heap_mut().place_box(boxed)
    }

    pub fn from_values<V, F>(
        heap: &mut impl AsHeap,
        values: impl Iterator<Item = V>,
        cons: F,
    ) -> Gc<Vector<T>>
    where
        F: Fn(&mut Heap, V) -> Gc<T>,
    {
        let heap = heap.as_heap_mut();

        let elems: Vec<Gc<T>> = values.map(|v| cons(heap, v)).collect();
        Self::new(heap, elems.as_slice())
    }

    fn is_inline(&self) -> bool {
        self.inline_length <= (MAX_INLINE_LENGTH as u32)
    }

    fn as_repr(&self) -> Repr<'_, T> {
        if self.is_inline() {
            Repr::Inline(unsafe { &*(self as *const Vector<T> as *const InlineVector<T>) })
        } else {
            Repr::Large(unsafe { &*(self as *const Vector<T> as *const LargeVector<T>) })
        }
    }

    pub fn len(&self) -> usize {
        match self.as_repr() {
            Repr::Inline(inline) => inline.inline_length as usize,
            Repr::Large(large) => large.values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.inline_length == 0
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Gc<T>> {
        match self.as_repr() {
            Repr::Inline(inline) => inline.values[0..self.len()].iter(),
            Repr::Large(large) => large.values.iter(),
        }
    }

    pub(crate) fn values_mut(&mut self) -> &mut [Gc<T>] {
        unsafe {
            if self.is_inline() {
                &mut (*(self as *mut Vector<T> as *mut InlineVector<T>)).values[0..self.len()]
            } else {
                &mut (*(self as *mut Vector<T> as *mut LargeVector<T>)).values
            }
        }
    }
}

impl<T: Boxed> PartialEq for Vector<T> {
    fn eq(&self, other: &Vector<T>) -> bool {
        self.iter().eq(other.iter())
    }
}

impl<T: Boxed> fmt::Debug for Vector<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        formatter.write_str("Vector(")?;
        formatter.debug_list().entries(self.iter()).finish()?;
        formatter.write_str(")")
    }
}

impl<T: Boxed> EncodeBoxedABIType for Vector<T>
where
    T: EncodeBoxedABIType,
{
    const BOXED_ABI_TYPE: BoxedABIType = BoxedABIType::Vector(&T::BOXED_ABI_TYPE);
}

#[repr(C, align(16))]
pub struct InlineVector<T: Boxed> {
    header: Header,
    inline_length: u32,
    values: [Gc<T>; MAX_INLINE_LENGTH],
}

#[repr(C, align(16))]
pub struct LargeVector<T: Boxed> {
    header: Header,
    inline_length: u32,
    values: Vec<Gc<T>>,
}

enum Repr<'a, T: Boxed>
where
    T: 'a,
{
    Inline(&'a InlineVector<T>),
    Large(&'a LargeVector<T>),
}

impl<T: Boxed> Hash for Vector<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        TypeTag::Vector.hash(state);
        state.write_usize(self.len());
        for value in self.iter() {
            value.hash(state);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::boxed::heap::Heap;
    use std::mem;

    #[test]
    fn sizes() {
        assert_eq!(32, mem::size_of::<Vector<Any>>());
        assert_eq!(32, mem::size_of::<InlineVector<Any>>());
        assert_eq!(32, mem::size_of::<LargeVector<Any>>());
    }

    #[test]
    fn equality() {
        use crate::boxed::Int;

        let mut heap = Heap::empty();

        let boxed1 = Int::new(&mut heap, 1);
        let boxed2 = Int::new(&mut heap, 2);
        let boxed3 = Int::new(&mut heap, 3);

        let forward_vec1 = Vector::new(&mut heap, &[boxed1, boxed2, boxed3]);
        let forward_vec2 = Vector::new(&mut heap, &[boxed1, boxed2, boxed3]);
        let reverse_vec = Vector::new(&mut heap, &[boxed3, boxed2, boxed1]);

        assert_ne!(forward_vec1, reverse_vec);
        assert_eq!(forward_vec1, forward_vec2);
    }

    #[test]
    fn fmt_debug() {
        use crate::boxed::Int;

        let mut heap = Heap::empty();

        let forward_vec = Vector::from_values(&mut heap, [1, 2, 3].iter().cloned(), Int::new);

        assert_eq!(
            "Vector([Int(1), Int(2), Int(3)])",
            format!("{:?}", forward_vec)
        );
    }
}
