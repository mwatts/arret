use std::fmt;
use std::hash::{Hash, Hasher};

use crate::boxed::*;

#[repr(C, align(16))]
pub struct Char {
    header: Header,
    value: char,
}

impl Boxed for Char {}
impl UniqueTagged for Char {}

impl Char {
    pub fn new(heap: &mut impl AsHeap, value: char) -> Gc<Char> {
        heap.as_heap_mut().place_box(Char {
            header: Header {
                type_tag: Self::TYPE_TAG,
                alloc_type: AllocType::Heap16,
            },
            value,
        })
    }

    pub fn value(&self) -> char {
        self.value
    }
}

impl PartialEq for Char {
    fn eq(&self, other: &Char) -> bool {
        self.value == other.value
    }
}

impl Hash for Char {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Self::TYPE_TAG.hash(state);
        self.value().hash(state)
    }
}

impl fmt::Debug for Char {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(formatter, "Char({:?})", self.value)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::boxed::heap::Heap;
    use std::mem;

    #[test]
    fn sizes() {
        assert_eq!(16, mem::size_of::<Char>());
    }

    #[test]
    fn equality() {
        let mut heap = Heap::empty();

        let boxed_a1 = Char::new(&mut heap, 'a');
        let boxed_a2 = Char::new(&mut heap, 'a');
        let boxed_b = Char::new(&mut heap, 'b');

        assert_ne!(boxed_a1, boxed_b);
        assert_eq!(boxed_a1, boxed_a2);
    }

    #[test]
    fn fmt_debug() {
        let mut heap = Heap::empty();

        let boxed_a = Char::new(&mut heap, 'a');
        assert_eq!("Char('a')", format!("{:?}", boxed_a));
    }
}
