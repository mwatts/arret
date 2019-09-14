use llvm_sys::core::*;
use llvm_sys::prelude::*;

use crate::codegen::fun_gen::FunCtx;
use crate::codegen::mod_gen::ModCtx;
use crate::codegen::panic_gen::gen_panic;
use crate::codegen::target_gen::TargetCtx;

pub struct CheckedIntOp {
    math_intrinsic_name: &'static [u8],
    result_name: &'static [u8],
    panic_message: &'static str,
}

pub const CHECKED_ADD: CheckedIntOp = CheckedIntOp {
    math_intrinsic_name: b"llvm.sadd.with.overflow.i64\0",
    result_name: b"sum\0",
    panic_message: "attempt to add with overflow",
};

pub const CHECKED_SUB: CheckedIntOp = CheckedIntOp {
    math_intrinsic_name: b"llvm.ssub.with.overflow.i64\0",
    result_name: b"difference\0",
    panic_message: "attempt to subtract with overflow",
};

pub const CHECKED_MUL: CheckedIntOp = CheckedIntOp {
    math_intrinsic_name: b"llvm.smul.with.overflow.i64\0",
    result_name: b"product\0",
    panic_message: "attempt to multiply with overflow",
};

pub(crate) fn gen_checked_int_math(
    tcx: &mut TargetCtx,
    mcx: &mut ModCtx<'_, '_, '_>,
    fcx: &mut FunCtx,
    int_op: &'static CheckedIntOp,
    llvm_lhs: LLVMValueRef,
    llvm_rhs: LLVMValueRef,
) -> LLVMValueRef {
    let CheckedIntOp {
        math_intrinsic_name,
        result_name,
        panic_message,
    } = int_op;

    unsafe {
        let llvm_i1 = LLVMInt1TypeInContext(tcx.llx);
        let llvm_i64 = LLVMInt64TypeInContext(tcx.llx);

        let mut return_type_members = [llvm_i64, llvm_i1];

        let llvm_return_type = LLVMStructTypeInContext(
            tcx.llx,
            return_type_members.as_mut_ptr(),
            return_type_members.len() as u32,
            0,
        );

        let llvm_param_types = &mut [llvm_i64, llvm_i64];

        let math_intrinsic_llvm_type = LLVMFunctionType(
            llvm_return_type,
            llvm_param_types.as_mut_ptr(),
            llvm_param_types.len() as u32,
            0,
        );

        let math_intrinsic_fun =
            mcx.get_function_or_insert(math_intrinsic_llvm_type, math_intrinsic_name, |_| {});

        let math_intrinsic_args = &mut [llvm_lhs, llvm_rhs];

        let llvm_result_with_overflow = LLVMBuildCall(
            fcx.builder,
            math_intrinsic_fun,
            math_intrinsic_args.as_mut_ptr(),
            math_intrinsic_args.len() as u32,
            b"result_with_overflow\0".as_ptr() as *const _,
        );

        let llvm_math_result = LLVMBuildExtractValue(
            fcx.builder,
            llvm_result_with_overflow,
            0,
            result_name.as_ptr() as *const _,
        );

        let llvm_overflow = LLVMBuildExtractValue(
            fcx.builder,
            llvm_result_with_overflow,
            1,
            b"overflow_flag\0".as_ptr() as *const _,
        );

        let overflow_block = LLVMAppendBasicBlockInContext(
            tcx.llx,
            fcx.function,
            b"overflow\0".as_ptr() as *const _,
        );

        let cont_block = LLVMAppendBasicBlockInContext(
            tcx.llx,
            fcx.function,
            b"no_overflow\0".as_ptr() as *const _,
        );

        LLVMBuildCondBr(fcx.builder, llvm_overflow, overflow_block, cont_block);

        LLVMPositionBuilderAtEnd(fcx.builder, overflow_block);
        gen_panic(tcx, mcx, fcx, panic_message);

        LLVMPositionBuilderAtEnd(fcx.builder, cont_block);
        llvm_math_result
    }
}
