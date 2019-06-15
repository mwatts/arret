//! Interned symbols
//!
//! This uses a fixed 8 byte representation for interned symbol. They are associated with a
//! particular `Interner` instance which can return the original [`str`] name of the symbol.
//! Interned symbols from the same `Interner` can be compared directly without a reference to
//! the `Interner` instance.
//!
//! Symbol names of 8 bytes or less are encoded directly in the `InternedSym`` instance without
//! storing the name in the `Interner`. They are padded with a constant invalid UTF-8 sequence so
//! the length of the inline name can be recovered.
//!
//! The encoding for names larger than 8 bytes uses an index in to a [`Vec`] stored in the
//! `Interner`. The indexed representation is invalid UTF-8 so it cannot collide with a valid
//! symbol name.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::{fmt, ptr, str};

// UTF-8 sequences cannot start with 10xxxxxxx. This is pattern for the last continuation byte,
// but any 1 byte sequences are encoded directly. We can use these values freely without colliding
// with inline names.
const INLINE_FILL_BYTE: u8 = 0x80;
const LOCAL_INDEXED_FLAG: u8 = 0x81;
const GLOBAL_INDEXED_FLAG: u8 = 0x82;

const INLINE_SIZE: usize = 8;

#[repr(C)]
pub struct GlobalName {
    name_byte_length: usize,
    name_bytes: *const u8,
}

impl GlobalName {
    fn as_str(&self) -> &str {
        unsafe {
            let byte_slice = std::slice::from_raw_parts(self.name_bytes, self.name_byte_length);
            std::str::from_utf8_unchecked(byte_slice)
        }
    }
}

#[repr(align(8))]
#[derive(Copy, Clone)]
struct InternedIndexed {
    flag_byte: u8,
    _padding: [u8; 3],
    name_index: u32,
}

#[repr(align(8))]
#[derive(Copy, Clone)]
struct InternedInline {
    name_bytes: [u8; INLINE_SIZE],
}

impl InternedInline {
    fn as_str(&self) -> &str {
        // Find the first fill byte. If none is found assume our full inline size.
        let length = self
            .name_bytes
            .iter()
            .position(|byte| *byte == INLINE_FILL_BYTE)
            .unwrap_or(INLINE_SIZE);

        unsafe { str::from_utf8_unchecked(&self.name_bytes[0..length]) }
    }
}

#[repr(align(8))]
#[derive(Copy, Clone)]
pub union InternedSym {
    indexed: InternedIndexed,
    inline: InternedInline,
    raw: u64,
}

enum InternedRepr<'a> {
    Inline(&'a InternedInline),
    LocalIndexed(&'a InternedIndexed),
    GlobalIndexed(&'a InternedIndexed),
}

impl InternedSym {
    /// Tries to return an inline interned Sym
    ///
    /// This can be accomplished without an [`Interner`] as we don't need to add a name to the
    /// [`Interner`]'s index.
    pub fn try_from_inline_name(name: &str) -> Option<InternedSym> {
        if name.len() <= INLINE_SIZE {
            let mut interned_inline = InternedInline {
                name_bytes: [INLINE_FILL_BYTE; INLINE_SIZE],
            };

            unsafe {
                ptr::copy_nonoverlapping(
                    name.as_ptr(),
                    &mut interned_inline.name_bytes[0] as *mut u8,
                    name.len(),
                );
            }

            Some(InternedSym {
                inline: interned_inline,
            })
        } else {
            None
        }
    }

    pub unsafe fn from_global_index(index: u32) -> InternedSym {
        InternedSym {
            indexed: InternedIndexed {
                flag_byte: GLOBAL_INDEXED_FLAG,
                _padding: [0; 3],
                name_index: index,
            },
        }
    }

    pub fn to_raw_u64(self) -> u64 {
        unsafe { self.raw }
    }

    fn repr(&self) -> InternedRepr<'_> {
        unsafe {
            match self.indexed.flag_byte {
                LOCAL_INDEXED_FLAG => InternedRepr::LocalIndexed(&self.indexed),
                GLOBAL_INDEXED_FLAG => InternedRepr::GlobalIndexed(&self.indexed),
                _ => InternedRepr::Inline(&self.inline),
            }
        }
    }
}

impl PartialEq for InternedSym {
    fn eq(&self, other: &InternedSym) -> bool {
        unsafe { self.raw == other.raw }
    }
}

impl Eq for InternedSym {}

impl Hash for InternedSym {
    fn hash<H: Hasher>(&self, state: &mut H) {
        unsafe {
            state.write(&self.inline.name_bytes);
        }
    }
}

impl fmt::Debug for InternedSym {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self.repr() {
            InternedRepr::LocalIndexed(indexed) | InternedRepr::GlobalIndexed(indexed) => {
                // We don't have access to the `Interner` so we can't print our interned value
                write!(formatter, "`{:x}", indexed.name_index)
            }
            InternedRepr::Inline(inline) => write!(formatter, "'{}", inline.as_str()),
        }
    }
}

// TODO: This keeps two copies of the name. We can't simply keep a pointer to inside the `Vec`
// or `HashMap` as they might reallocate. We can fix this later.
pub struct Interner {
    names: Vec<Box<str>>,
    name_to_index: HashMap<Box<str>, u32>,
    /// Contains the highest static index + 1
    static_index_watermark: u32,
    global_names: *const GlobalName,
}

impl Interner {
    pub fn new() -> Interner {
        Self::with_global_names(std::ptr::null())
    }

    pub fn with_global_names(global_names: *const GlobalName) -> Interner {
        Interner {
            names: vec![],
            name_to_index: HashMap::new(),
            static_index_watermark: 0,
            global_names,
        }
    }

    /// Interns a symbol with the given name
    ///
    /// The `InternedSym` must be referenced by a boxed `Sym` before the next GC cycle.
    pub fn intern(&mut self, name: &str) -> InternedSym {
        if let Some(inline_interned) = InternedSym::try_from_inline_name(name) {
            return inline_interned;
        };

        if !self.global_names.is_null() {
            unimplemented!("interning symbols with global interned names");
        }

        let index = self.name_to_index.get(name).cloned().unwrap_or_else(|| {
            let index = self.names.len() as u32;
            self.names.push(name.into());
            self.name_to_index.insert(name.into(), index);

            index
        });

        InternedSym {
            indexed: InternedIndexed {
                flag_byte: LOCAL_INDEXED_FLAG,
                _padding: [0; 3],
                name_index: index,
            },
        }
    }

    /// Interns a static symbol with the given name
    ///
    /// This should only be used where it's not possible to GC root the [`InternedSym`]. This is
    /// currently only used by the JIT where we can't track [`InternedSym`] references in the
    /// generated code.
    pub fn intern_static(&mut self, name: &str) -> InternedSym {
        let interned_sym = self.intern(name);

        if let InternedRepr::LocalIndexed(indexed_sym) = interned_sym.repr() {
            self.static_index_watermark = indexed_sym.name_index + 1;
        }

        interned_sym
    }

    pub fn unintern<'a>(&'a self, interned: &'a InternedSym) -> &'a str {
        match interned.repr() {
            InternedRepr::LocalIndexed(indexed) => &self.names[indexed.name_index as usize],
            InternedRepr::GlobalIndexed(indexed) => unsafe {
                let global_name = &*self.global_names.offset(indexed.name_index as isize);
                global_name.as_str()
            },
            InternedRepr::Inline(inline) => inline.as_str(),
        }
    }

    /// Returns a clone of this interner usable for garbage collection
    ///
    /// This preserves the index of all static [`InternedSym`]s.
    pub(crate) fn clone_for_collect_garbage(&self) -> Self {
        if self.static_index_watermark == 0 {
            // Avoid iterating over our HashMap
            return Self::new();
        };

        let static_index_watermark = self.static_index_watermark;

        let names = self.names[0..static_index_watermark as usize].to_vec();
        let name_to_index = self
            .name_to_index
            .iter()
            .filter_map(|(name, idx)| {
                if *idx < self.static_index_watermark {
                    Some((name.clone(), *idx))
                } else {
                    None
                }
            })
            .collect();

        Interner {
            names,
            name_to_index,
            static_index_watermark,
            global_names: self.global_names,
        }
    }
}

impl Default for Interner {
    fn default() -> Interner {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::mem;

    #[test]
    fn sizes() {
        assert_eq!(8, mem::size_of::<InternedIndexed>());
        assert_eq!(8, mem::size_of::<InternedInline>());
        assert_eq!(8, mem::size_of::<InternedSym>());
    }

    #[test]
    fn equality() {
        let inline_name = "inline";
        let index_name = "This must be longer than eight bytes";

        let mut interner = Interner::new();

        let intern_inline1 = interner.intern(inline_name);
        let intern_inline2 = interner.intern(inline_name);
        assert_eq!(intern_inline1, intern_inline2);

        let intern_index1 = interner.intern(index_name);
        let intern_index2 = interner.intern(index_name);
        assert_eq!(intern_index1, intern_index2);

        // These should not be equal
        assert_ne!(intern_inline1, intern_index1);
    }

    #[test]
    fn fmt_debug() {
        let mut interner = Interner::new();

        let intern_inline = interner.intern("inline");
        assert_eq!("'inline", format!("{:?}", intern_inline));

        let intern_indexed = interner.intern("This is very long and can't be stored inline");
        assert_eq!("`0", format!("{:?}", intern_indexed));
    }

    #[test]
    fn roundtrip() {
        let mut interner = Interner::new();

        let test_names = [
            "",
            "short1",
            "short2",
            "exactly8",
            "Hello, world!",
            "This is another long test string",
        ];

        let mut previous_interneds = vec![];
        for &name in &test_names {
            let interned = interner.intern(name);
            assert_eq!(name, interner.unintern(&interned));

            // Make sure we don't equal any of our previous interned symbols
            assert!(!previous_interneds.contains(&interned));
            previous_interneds.push(interned);
        }
    }

    #[test]
    fn clone_for_collect_garbage() {
        let mut interner = Interner::new();
        interner.intern("one                ");
        interner.intern("two                ");
        interner.intern("three              ");

        assert_eq!(3, interner.names.len());
        assert_eq!(3, interner.name_to_index.len());

        // No static symbols; we should collect everything
        interner = interner.clone_for_collect_garbage();
        assert_eq!(0, interner.names.len());
        assert_eq!(0, interner.name_to_index.len());

        interner.intern("one                ");
        interner.intern_static("two         ");
        interner.intern("three              ");

        // We need to preserve the second symbol
        interner = interner.clone_for_collect_garbage();
        assert_eq!(2, interner.names.len());
        assert_eq!(2, interner.name_to_index.len());

        // We should be able to "promote" an existing symbol to static
        interner.intern("one-two-three-four");
        interner.intern_static("one-two-three-four");

        assert_eq!(3, interner.names.len());
        assert_eq!(3, interner.name_to_index.len());

        interner = interner.clone_for_collect_garbage();
        assert_eq!(3, interner.names.len());
        assert_eq!(3, interner.name_to_index.len());
    }
}
