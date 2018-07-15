#![allow(dead_code)]
#![cfg_attr(feature = "cargo-clippy", warn(clippy))]

use std::collections::HashMap;

mod boxed;
use boxed::Gc;

#[derive(Debug)]
pub enum BoxedABIType {
    Direct(boxed::TypeTag),
    Vector(&'static BoxedABIType),
}

#[derive(Debug)]
pub enum ABIType {
    Bool,
    Char,
    Float,
    Int,
    Void,
    Boxed(BoxedABIType),
}

pub struct Task {
    heap: boxed::Heap,
}

impl Task {
    pub fn heap(&mut self) -> &mut boxed::Heap {
        &mut self.heap
    }
}

impl Default for Task {
    fn default() -> Task {
        Task {
            heap: boxed::Heap::with_capacity(32),
        }
    }
}

#[derive(Debug)]
pub struct ExternFun {
    takes_task: bool,
    params: &'static [ABIType],
    ret: ABIType,
    entry_point: &'static str,
}

trait EncodeABIType {
    const ABI_TYPE: ABIType;
}

trait EncodeBoxedABIType {
    const BOXED_ABI_TYPE: BoxedABIType;
}

impl EncodeABIType for f64 {
    const ABI_TYPE: ABIType = ABIType::Float;
}

impl EncodeABIType for i64 {
    const ABI_TYPE: ABIType = ABIType::Int;
}

impl EncodeABIType for char {
    const ABI_TYPE: ABIType = ABIType::Char;
}

impl EncodeABIType for () {
    const ABI_TYPE: ABIType = ABIType::Void;
}

impl<T> EncodeABIType for Gc<T>
where
    T: EncodeBoxedABIType,
{
    const ABI_TYPE: ABIType = ABIType::Boxed(T::BOXED_ABI_TYPE);
}

impl<T> EncodeBoxedABIType for T
where
    T: boxed::DirectTagged,
{
    const BOXED_ABI_TYPE: BoxedABIType = BoxedABIType::Direct(T::TYPE_TAG);
}

impl<T> EncodeBoxedABIType for boxed::Vector<T>
where
    T: EncodeBoxedABIType,
{
    const BOXED_ABI_TYPE: BoxedABIType = BoxedABIType::Vector(&T::BOXED_ABI_TYPE);
}

macro_rules! define_extern_fn {
    ($desc_name:ident = $func_name:ident($task_name:ident : &mut Task, $($param_name:ident : $rust_ty:ty),*) -> $ret:ty $body:block) => {
        #[no_mangle]
        pub extern "C" fn $func_name($task_name: &'static mut Task, $($param_name: $rust_ty),*) -> $ret {
            $body
        }

        const $desc_name: ExternFun = ExternFun {
            takes_task: true,
            params: &[
                $(<$rust_ty>::ABI_TYPE),*
            ],
            ret: <$ret>::ABI_TYPE,
            entry_point: stringify!($func_name),
        };
    };

    ($desc_name:ident = $func_name:ident($($param_name:ident : $rust_ty:ty),*) -> $ret:ty $body:block) => {
        #[no_mangle]
        pub extern "C" fn $func_name($($param_name: $rust_ty),*) -> $ret {
            $body
        }

        const $desc_name: ExternFun = ExternFun {
            takes_task: false,
            params: &[
                $(<$rust_ty>::ABI_TYPE),*
            ],
            ret: <$ret>::ABI_TYPE,
            entry_point: stringify!($func_name),
        };
    };
}

define_extern_fn! {
    HELLO_WORLD = hello_world(param1: Gc<boxed::Str>) -> i64 {
        println!("Hello, {}!", param1.as_str());
        42
    }
}

define_extern_fn! {
    TAKES_TASK = takes_task(task: &mut Task, _param1: Gc<boxed::Vector<boxed::Int>>) -> Gc<boxed::Float> {
        task.heap().new_box(64.0)
    }
}

define_extern_fn! {
     PRINT_NUM = print_num(number: i64) -> () {
         println!("Number is {}", number)
     }
}

fn main() {
    let mut task = Task::default();

    let sailor_str = task.heap().new_box::<boxed::Str, _>("sailorr");
    let number = hello_world(sailor_str);
    print_num(number);

    println!("Number entry point: '{}'", PRINT_NUM.entry_point);
    println!("TAKES_TASK takes task: '{}'", TAKES_TASK.takes_task);

    let mut exports = HashMap::<&'static str, ExternFun>::new();
    exports.insert("hello-world", HELLO_WORLD);
    exports.insert("print-num", PRINT_NUM);
}
