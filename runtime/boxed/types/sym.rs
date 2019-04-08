use std::fmt;
use std::hash::{Hash, Hasher};

use crate::boxed::*;
use crate::intern::{InternedSym, Interner};

#[repr(C, align(16))]
pub struct Sym {
    header: Header,
    // TODO: We have room to fit a u32 hash value here which should help with re-interning heap
    // indexed symbols in new heaps
    pub(crate) interned: InternedSym,
}

impl Boxed for Sym {}
impl UniqueTagged for Sym {}

impl Sym {
    pub fn new(heap: &mut impl AsHeap, value: &str) -> Gc<Sym> {
        let heap = heap.as_heap_mut();
        let interned = heap.interner_mut().intern(value);

        heap.place_box(Sym {
            header: Header {
                type_tag: Self::TYPE_TAG,
                alloc_type: AllocType::Heap16,
            },
            interned,
        })
    }

    pub fn name<'a>(&'a self, interner: &'a Interner) -> &'a str {
        interner.unintern(&self.interned)
    }
}

impl PartialEq for Sym {
    fn eq(&self, other: &Sym) -> bool {
        self.interned == other.interned
    }
}

impl Hash for Sym {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Self::TYPE_TAG.hash(state);
        self.interned.hash(state);
    }
}

impl fmt::Debug for Sym {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(formatter, "Sym({:?})", self.interned)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::boxed::heap::Heap;
    use std::mem;

    #[test]
    fn sizes() {
        assert_eq!(16, mem::size_of::<Sym>());
    }

    #[test]
    fn equality() {
        let mut heap = Heap::empty();

        let boxed_one1 = Sym::new(&mut heap, "one");
        let boxed_one2 = Sym::new(&mut heap, "one");
        let boxed_two = Sym::new(&mut heap, "two");

        assert_ne!(boxed_one1, boxed_two);
        assert_eq!(boxed_one1, boxed_one2);
    }

    #[test]
    fn fmt_debug() {
        let mut heap = Heap::empty();

        let boxed_one = Sym::new(&mut heap, "one");
        assert_eq!(r#"Sym('one)"#, format!("{:?}", boxed_one));
    }
}
