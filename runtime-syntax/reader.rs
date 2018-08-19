use syntax::datum::Datum;

use runtime::boxed;
use runtime::boxed::prelude::*;
use runtime::boxed::refs::Gc;

/// Places a syntax datum on a box heap
pub fn box_syntax_datum(heap: &mut impl boxed::AsHeap, datum: &Datum) -> Gc<boxed::Any> {
    match datum {
        Datum::Bool(_, value) => boxed::Bool::singleton_ref(*value).as_any_ref(),
        Datum::Int(_, val) => boxed::Int::new(heap, *val).as_any_ref(),
        Datum::Float(_, val) => boxed::Float::new(heap, *val).as_any_ref(),
        Datum::Char(_, val) => boxed::Char::new(heap, *val).as_any_ref(),
        Datum::Str(_, val) => boxed::Str::new(heap, val.as_ref()).as_any_ref(),
        Datum::Sym(_, val) => boxed::Sym::new(heap, val.as_ref()).as_any_ref(),
        Datum::List(_, vs) => {
            let boxed_elems = vs
                .iter()
                .map(|elem| box_syntax_datum(heap, elem))
                .collect::<Vec<Gc<boxed::Any>>>();

            boxed::List::new(heap, boxed_elems.into_iter()).as_any_ref()
        }
        Datum::Vector(_, vs) => {
            let boxed_elems = vs
                .iter()
                .map(|elem| box_syntax_datum(heap, elem))
                .collect::<Vec<Gc<boxed::Any>>>();

            boxed::Vector::new(heap, boxed_elems.as_slice()).as_any_ref()
        }
        Datum::Map(_, _) => unimplemented!("Maps are not implemented"),
        Datum::Set(_, _) => unimplemented!("Sets are not implemented"),
    }
}

// This is indirectly tested by `writer`
