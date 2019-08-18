use std::{fs, path};

use codespan_reporting::Diagnostic;

use arret_compiler::{
    emit_diagnostics_to_stderr, errors_to_diagnostics, print_program_mir, CompileCtx,
};

// We don't use this ourselves so overload it for the purposes of dumping MIR
const MIR_OUTPUT_TYPE: arret_compiler::OutputType = arret_compiler::OutputType::None;

fn try_compile_input_file(
    ccx: &CompileCtx,
    options: arret_compiler::GenProgramOptions<'_>,
    input_file: &arret_compiler::SourceFile,
    output_path: &path::Path,
    debug_info: bool,
) -> Result<(), Vec<Diagnostic>> {
    let hir = arret_compiler::lower_program(ccx, input_file).map_err(errors_to_diagnostics)?;

    let inferred_defs =
        arret_compiler::infer_program(hir.defs, hir.main_var_id).map_err(errors_to_diagnostics)?;

    let mut ehx = arret_compiler::EvalHirCtx::new(true);
    for inferred_def in inferred_defs {
        ehx.consume_def(inferred_def)?;
    }

    if ehx.should_collect() {
        ehx.collect_garbage();
    }

    let mir_program = ehx.into_built_program(hir.main_var_id)?;

    if options.output_type() == MIR_OUTPUT_TYPE {
        let mut output_file = fs::File::create(output_path).unwrap();
        print_program_mir(&mut output_file, &mir_program, Some(ccx.source_loader())).unwrap();
        return Ok(());
    }

    let debug_source_loader = if debug_info {
        Some(ccx.source_loader())
    } else {
        None
    };

    arret_compiler::gen_program(
        options,
        &hir.rust_libraries,
        &mir_program,
        output_path,
        debug_source_loader,
    );

    Ok(())
}

pub fn compile_input_file(
    ccx: &CompileCtx,
    input_file: &arret_compiler::SourceFile,
    target_triple: Option<&str>,
    output_path: &path::Path,
    debug_info: bool,
) -> bool {
    use std::ffi;

    let output_type = match output_path.extension().and_then(ffi::OsStr::to_str) {
        Some("mir") => MIR_OUTPUT_TYPE,
        Some("ll") => arret_compiler::OutputType::LLVMIR,
        Some("s") => arret_compiler::OutputType::Assembly,
        Some("o") => arret_compiler::OutputType::Object,
        _ => arret_compiler::OutputType::Executable,
    };

    let options = arret_compiler::GenProgramOptions::new()
        .with_target_triple(target_triple)
        .with_output_type(output_type)
        .with_llvm_opt(ccx.enable_optimisations());

    let result = try_compile_input_file(ccx, options, input_file, output_path, debug_info);

    if let Err(diagnostics) = result {
        emit_diagnostics_to_stderr(ccx.source_loader(), diagnostics);
        false
    } else {
        true
    }
}
