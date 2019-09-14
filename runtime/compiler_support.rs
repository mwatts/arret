//! Internal functions called by compiled Arret code
//!
//! Calls to these functions are generated by the compiler. They should not be called from user
//! code.

use std::{alloc, panic, process};

use crate::boxed;
use crate::boxed::prelude::*;
use crate::boxed::refs::Gc;
use crate::boxed::type_info::TypeInfo;
use crate::class_map::{ClassMap, ClassRef};
use crate::intern::{GlobalName, Interner};
use crate::task::Task;

type TaskEntry = extern "C" fn(&mut Task);

#[export_name = "arret_runtime_launch_task"]
pub extern "C" fn launch_task(
    global_names: *const GlobalName,
    classmap_classes: *const ClassRef<'static>,
    entry: TaskEntry,
) {
    let interner = Interner::with_global_names(global_names);
    let class_map = ClassMap::with_const_classes(classmap_classes);

    let type_info = TypeInfo::new(interner, class_map);
    let mut task = Task::with_type_info(type_info);

    if let Err(err) = panic::catch_unwind(panic::AssertUnwindSafe(|| entry(&mut task))) {
        if let Some(message) = err.downcast_ref::<String>() {
            eprintln!("{}", message);
        } else {
            eprintln!("Unexpected panic type");
        };

        process::exit(1);
    };
}

#[export_name = "arret_runtime_alloc_cells"]
pub extern "C" fn alloc_cells(task: &mut Task, count: u32) -> *mut boxed::Any {
    task.heap_mut().alloc_cells(count as usize)
}

#[export_name = "arret_runtime_alloc_record_data"]
pub extern "C" fn alloc_record_data(size: u64, align: u32) -> *mut u8 {
    unsafe {
        let layout = alloc::Layout::from_size_align_unchecked(size as usize, align as usize);
        alloc::alloc(layout)
    }
}

#[export_name = "arret_runtime_equals"]
pub extern "C" fn equals(task: &Task, lhs: Gc<boxed::Any>, rhs: Gc<boxed::Any>) -> bool {
    lhs.eq_in_heap(task.as_heap(), &rhs)
}

#[export_name = "arret_runtime_panic_with_string"]
pub unsafe extern "C" fn panic_with_string(message_bytes: *const u8, message_len: u32) {
    let message_vec: Vec<u8> =
        std::slice::from_raw_parts(message_bytes, message_len as usize).into();

    panic!(String::from_utf8_unchecked(message_vec));
}
