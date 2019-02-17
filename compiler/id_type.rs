use std::rc::Rc;
use std::{fmt, hash, ops};

/// Builds a new ID type based off indexing in to a Vec lookup table
///
/// This stores the value internally at $ty (typically `u32`) while the interface uses `usize` to
/// support easy indexing.
#[macro_export]
macro_rules! new_indexing_id_type {
    ($name:ident, $ty:ty) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, PartialOrd)]
        pub struct $name($ty);

        impl $name {
            pub fn new(value: usize) -> $name {
                $name(value as $ty)
            }

            #[allow(unused)]
            pub fn new_entry_id<T>(lookup_vec: &mut Vec<T>, entry: T) -> $name {
                let id = Self::new(lookup_vec.len());
                lookup_vec.push(entry);
                id
            }

            #[allow(unused)]
            pub fn to_usize(&self) -> usize {
                self.0 as usize
            }
        }
    };
}

/// Builds a new ID type using a global counter
///
/// This allows allocating IDs without threading a mutable counter through multiple layers of
/// code.
#[macro_export]
macro_rules! new_global_id_type {
    ($id_name:ident) => {
        use std::num::NonZeroUsize;
        use std::sync::atomic::{AtomicUsize, Ordering};

        // These counters are very hot and shared between threads
        // They're not strongly correlated with each other so put them on different cachelines to
        // avoid bouncing them between CPUs. The value of 64 is just a guess; it's a typical value
        // and isn't needed for correctness.
        #[repr(align(64))]
        struct AlignedAtomicUsize(AtomicUsize);

        static NEXT_VALUE: AlignedAtomicUsize = AlignedAtomicUsize(AtomicUsize::new(1));

        #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, PartialOrd, Ord)]
        pub struct $id_name(NonZeroUsize);

        impl $id_name {
            /// Allocates a ID unique for the duration of compiler's execution
            pub fn alloc() -> Self {
                // We used relaxed ordering because the order doesn't actually matter; these are
                // used only for uniqueness
                let raw_id = NEXT_VALUE.0.fetch_add(1, Ordering::Relaxed);
                Self::new(raw_id)
            }

            /// Produces an iterator of IDs unique for the duration of compiler's execution
            ///
            /// This can be significantly more efficient than the equivalent number of calls to `alloc`
            #[allow(unused)]
            pub fn alloc_iter(length: usize) -> impl ExactSizeIterator<Item = Self> {
                let start_raw = NEXT_VALUE.0.fetch_add(length, Ordering::Relaxed);
                let end_raw = start_raw + length;

                (start_raw..end_raw).map(Self::new)
            }

            #[allow(unused)]
            pub fn to_usize(&self) -> usize {
                self.0.into()
            }

            fn new(raw_id: usize) -> Self {
                $id_name(NonZeroUsize::new(raw_id).unwrap())
            }
        }
    };
}

/// Builds a new ID type based off an arbitrary counter
#[macro_export]
macro_rules! new_counting_id_type {
    ($counter_name:ident, $id_name:ident) => {
        #[derive(Clone)]
        pub struct $counter_name(u32);

        impl $counter_name {
            pub fn new() -> $counter_name {
                $counter_name(1)
            }

            pub fn alloc(&mut self) -> $id_name {
                let id = $id_name(self.0);
                self.0 += 1;
                id
            }
        }

        #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, PartialOrd)]
        pub struct $id_name(u32);

        impl $id_name {
            #[allow(unused)]
            pub fn new(value: u32) -> $id_name {
                $id_name(value)
            }
        }
    };
}

/// Reference-counted pointer that uses pointer identity
///
/// Traits such as `Hash`, `Eq`, `Ord` etc. are implemented in terms of the value's memory location.
/// This means that the value returned by `RcId::new()` is considered equal to itself and its clones
/// regardless of the value it points to.
#[derive(Clone)]
pub struct RcId<T> {
    inner: Rc<T>,
}

impl<T> RcId<T> {
    pub fn new(value: T) -> Self {
        RcId {
            inner: Rc::new(value),
        }
    }
}

impl<T> ops::Deref for RcId<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.inner.deref()
    }
}

impl<T> PartialEq for RcId<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T> Eq for RcId<T> {}

impl<T> hash::Hash for RcId<T> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.inner.as_ref() as *const T as usize)
    }
}

impl<T> PartialOrd for RcId<T> {
    fn partial_cmp(&self, other: &RcId<T>) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for RcId<T> {
    fn cmp(&self, other: &RcId<T>) -> std::cmp::Ordering {
        (self.inner.as_ref() as *const T as usize).cmp(&(other.inner.as_ref() as *const T as usize))
    }
}

impl<T: fmt::Debug> fmt::Debug for RcId<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(formatter)
    }
}
