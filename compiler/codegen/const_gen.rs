use std::{ffi, mem};

use llvm_sys::core::*;
use llvm_sys::prelude::*;
use llvm_sys::{LLVMLinkage, LLVMUnnamedAddr};

use runtime::boxed;

use crate::codegen::mod_gen::ModCtx;
use crate::codegen::target_gen::TargetCtx;

fn annotate_private_global(llvm_global: LLVMValueRef) {
    unsafe {
        LLVMSetUnnamedAddress(llvm_global, LLVMUnnamedAddr::LLVMGlobalUnnamedAddr);
        LLVMSetGlobalConstant(llvm_global, 1 as i32);
        LLVMSetLinkage(llvm_global, LLVMLinkage::LLVMPrivateLinkage)
    }
}

pub fn gen_boxed_pair(
    tcx: &mut TargetCtx,
    mcx: &mut ModCtx<'_, '_, '_>,
    llvm_head: LLVMValueRef,
    llvm_rest: LLVMValueRef,
    llvm_length: LLVMValueRef,
) -> LLVMValueRef {
    unsafe {
        let type_tag = boxed::TypeTag::TopPair;
        let llvm_type = tcx.boxed_abi_to_llvm_struct_type(&type_tag.into());

        let members = &mut [
            tcx.llvm_box_header(type_tag.into_const_header()),
            llvm_length,
            llvm_head,
            llvm_rest,
        ];

        let llvm_value =
            LLVMConstNamedStruct(llvm_type, members.as_mut_ptr(), members.len() as u32);

        let global = LLVMAddGlobal(mcx.module, llvm_type, "const_pair\0".as_ptr() as *const _);
        LLVMSetInitializer(global, llvm_value);
        LLVMSetAlignment(global, mem::align_of::<boxed::TopPair>() as u32);

        annotate_private_global(global);
        global
    }
}

pub fn gen_boxed_inline_str(
    tcx: &mut TargetCtx,
    mcx: &mut ModCtx<'_, '_, '_>,
    value: &str,
) -> LLVMValueRef {
    unsafe {
        const MAX_INLINE_BYTES: usize = boxed::Str::MAX_INLINE_BYTES;

        let mut inline_buffer: [u8; MAX_INLINE_BYTES] = [0; MAX_INLINE_BYTES];
        inline_buffer[0..value.len()].copy_from_slice(value.as_bytes());

        let type_tag = boxed::TypeTag::Str;
        let inline_llvm_type = tcx.boxed_inline_str_llvm_type();
        let llvm_i8 = LLVMInt8TypeInContext(tcx.llx);

        let members = &mut [
            tcx.llvm_box_header(type_tag.into_const_header()),
            LLVMConstInt(llvm_i8, value.len() as u64, 0),
            LLVMConstStringInContext(
                tcx.llx,
                inline_buffer.as_mut_ptr() as *mut _,
                MAX_INLINE_BYTES as u32,
                1,
            ),
        ];

        let inline_llvm_value =
            LLVMConstNamedStruct(inline_llvm_type, members.as_mut_ptr(), members.len() as u32);

        let global = LLVMAddGlobal(
            mcx.module,
            inline_llvm_type,
            "const_str\0".as_ptr() as *const _,
        );
        LLVMSetInitializer(global, inline_llvm_value);
        LLVMSetAlignment(global, mem::align_of::<boxed::Str>() as u32);
        annotate_private_global(global);

        let llvm_type = tcx.boxed_abi_to_llvm_struct_type(&type_tag.into());
        LLVMConstBitCast(global, LLVMPointerType(llvm_type, 0))
    }
}

pub fn gen_boxed_sym(
    tcx: &mut TargetCtx,
    mcx: &mut ModCtx<'_, '_, '_>,
    value: &str,
) -> LLVMValueRef {
    let interned_sym = mcx.intern_name(value);

    unsafe {
        let type_tag = boxed::TypeTag::Sym;
        let boxed_llvm_type = tcx.boxed_abi_to_llvm_struct_type(&type_tag.into());
        let llvm_i64 = LLVMInt64TypeInContext(tcx.llx);

        let members = &mut [
            tcx.llvm_box_header(type_tag.into_const_header()),
            LLVMConstInt(llvm_i64, interned_sym.to_raw_u64(), 0),
        ];
        let boxed_llvm_value =
            LLVMConstNamedStruct(boxed_llvm_type, members.as_mut_ptr(), members.len() as u32);

        let global = LLVMAddGlobal(
            mcx.module,
            boxed_llvm_type,
            "const_sym\0".as_ptr() as *const _,
        );
        LLVMSetInitializer(global, boxed_llvm_value);
        LLVMSetAlignment(global, mem::align_of::<boxed::Sym>() as u32);
        annotate_private_global(global);

        global
    }
}

pub fn gen_global_interned_names(
    tcx: &mut TargetCtx,
    llvm_module: LLVMModuleRef,
    names: &[Box<str>],
) -> LLVMValueRef {
    unsafe {
        if names.is_empty() {
            return LLVMConstPointerNull(LLVMPointerType(tcx.global_interned_name_type(), 0));
        }

        let global_interned_name_llvm_type = tcx.global_interned_name_type();
        let usize_llvm_type = tcx.usize_llvm_type();

        let first_element_gep_indices = &mut [
            LLVMConstInt(LLVMInt32TypeInContext(tcx.llx), 0, 0),
            LLVMConstInt(LLVMInt32TypeInContext(tcx.llx), 0, 0),
        ];

        let mut llvm_names: Vec<LLVMValueRef> = names
            .iter()
            .map(|name| {
                let llvm_name_string = LLVMConstStringInContext(
                    tcx.llx,
                    name.as_bytes().as_ptr() as *mut _,
                    name.len() as u32,
                    1,
                );

                let name_global_name =
                    ffi::CString::new(format!("global_interned_name_{}", name)).unwrap();

                let llvm_name_global = LLVMAddGlobal(
                    llvm_module,
                    LLVMTypeOf(llvm_name_string),
                    name_global_name.as_ptr() as *const _,
                );
                LLVMSetInitializer(llvm_name_global, llvm_name_string);
                annotate_private_global(llvm_name_global);

                let llvm_name_string_ptr = LLVMConstGEP(
                    llvm_name_global,
                    first_element_gep_indices.as_mut_ptr(),
                    first_element_gep_indices.len() as u32,
                );

                let llvm_name_members = &mut [
                    LLVMConstInt(usize_llvm_type, name.len() as u64, 0),
                    llvm_name_string_ptr,
                ];

                LLVMConstNamedStruct(
                    global_interned_name_llvm_type,
                    llvm_name_members.as_mut_ptr(),
                    llvm_name_members.len() as u32,
                )
            })
            .collect();

        let llvm_names_value = LLVMConstArray(
            global_interned_name_llvm_type,
            llvm_names.as_mut_ptr(),
            llvm_names.len() as u32,
        );

        let global = LLVMAddGlobal(
            llvm_module,
            LLVMTypeOf(llvm_names_value),
            "global_interned_names\0".as_ptr() as *const _,
        );

        LLVMSetInitializer(global, llvm_names_value);
        annotate_private_global(global);

        LLVMConstGEP(
            global,
            first_element_gep_indices.as_mut_ptr(),
            first_element_gep_indices.len() as u32,
        )
    }
}

pub fn gen_boxed_int(
    tcx: &mut TargetCtx,
    mcx: &mut ModCtx<'_, '_, '_>,
    value: i64,
) -> LLVMValueRef {
    unsafe {
        let type_tag = boxed::TypeTag::Int;
        let llvm_type = tcx.boxed_abi_to_llvm_struct_type(&type_tag.into());
        let llvm_i64 = LLVMInt64TypeInContext(tcx.llx);

        let box_name = ffi::CString::new(format!("const_int_{}", value)).unwrap();

        let global = mcx.get_global_or_insert(llvm_type, box_name.as_bytes_with_nul(), || {
            let members = &mut [
                tcx.llvm_box_header(type_tag.into_const_header()),
                LLVMConstInt(llvm_i64, value as u64, 1),
            ];

            LLVMConstNamedStruct(llvm_type, members.as_mut_ptr(), members.len() as u32)
        });

        LLVMSetAlignment(global, mem::align_of::<boxed::Int>() as u32);
        annotate_private_global(global);
        global
    }
}

pub fn gen_boxed_float(
    tcx: &mut TargetCtx,
    mcx: &mut ModCtx<'_, '_, '_>,
    value: f64,
) -> LLVMValueRef {
    unsafe {
        let type_tag = boxed::TypeTag::Float;
        let llvm_type = tcx.boxed_abi_to_llvm_struct_type(&type_tag.into());
        let llvm_double = LLVMDoubleTypeInContext(tcx.llx);

        let members = &mut [
            tcx.llvm_box_header(type_tag.into_const_header()),
            LLVMConstReal(llvm_double, value),
        ];

        let llvm_value =
            LLVMConstNamedStruct(llvm_type, members.as_mut_ptr(), members.len() as u32);

        let global = LLVMAddGlobal(mcx.module, llvm_type, "const_float\0".as_ptr() as *const _);
        LLVMSetInitializer(global, llvm_value);
        LLVMSetAlignment(global, mem::align_of::<boxed::Float>() as u32);

        annotate_private_global(global);
        global
    }
}

pub fn gen_boxed_nil(tcx: &mut TargetCtx, mcx: &mut ModCtx<'_, '_, '_>) -> LLVMValueRef {
    tcx.ptr_to_singleton_box(mcx.module, boxed::TypeTag::Nil, b"ARRET_NIL\0")
}

pub fn gen_boxed_fun_thunk(
    tcx: &mut TargetCtx,
    mcx: &mut ModCtx<'_, '_, '_>,
    llvm_closure: LLVMValueRef,
    llvm_entry_point: LLVMValueRef,
) -> LLVMValueRef {
    unsafe {
        let type_tag = boxed::TypeTag::FunThunk;
        let llvm_type = tcx.boxed_abi_to_llvm_struct_type(&type_tag.into());

        let members = &mut [
            tcx.llvm_box_header(type_tag.into_const_header()),
            llvm_closure,
            llvm_entry_point,
        ];

        let llvm_value =
            LLVMConstNamedStruct(llvm_type, members.as_mut_ptr(), members.len() as u32);

        let global = LLVMAddGlobal(
            mcx.module,
            llvm_type,
            "const_fun_thunk\0".as_ptr() as *const _,
        );
        LLVMSetInitializer(global, llvm_value);
        LLVMSetAlignment(global, mem::align_of::<boxed::FunThunk>() as u32);

        annotate_private_global(global);
        global
    }
}
