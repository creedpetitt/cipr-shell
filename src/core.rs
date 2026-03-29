use std::fs;
use std::path::{Path, PathBuf};

use crate::ast::NodeArena;
use crate::codegen::Codegen;
use crate::parser::Parser;
use crate::scanner::Scanner;
use crate::type_checker::TypeChecker;

pub struct Core {
    pub arena: NodeArena,
}

impl Core {
    pub fn new() -> Self {
        Self {
            arena: NodeArena::new(),
        }
    }

    pub fn run_file(&mut self, path: &str, build_mode: bool) -> Result<(), String> {
        let source = fs::read_to_string(path).map_err(|e| e.to_string())?;

        let path_obj = Path::new(path);
        let ext = path_obj
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("");

        // Compute output binary path
        let file_stem = path_obj
            .file_stem()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("out");
        let parent = path_obj.parent().unwrap_or(Path::new(""));
        let out_path = parent.join(file_stem);

        if ext == "cipr" {
            // The prelude is auto-injected
            let prelude_inject = "include \"src/lib/prelude.cipr\";\n";
            let full_source = format!("{}{}", prelude_inject, source);
            self.run(&full_source, build_mode, out_path.to_str().unwrap_or("out"))
        } else {
            Err("Unsupported file extension. Expected .cipr".to_string())
        }
    }

    pub fn run(&mut self, source: &str, build_mode: bool, out_bin: &str) -> Result<(), String> {
        let (tokens, scan_error) = Scanner::new(source).scan_tokens();

        if scan_error {
            return Err("Scanner errors occurred.".to_string());
        }

        let mut visited_files = std::collections::HashSet::new();
        let mut parser = Parser::new(&tokens, &mut self.arena, &mut visited_files);
        let root = parser.parse();

        if parser.had_error {
            return Err("Parser errors occurred.".to_string());
        }

        if let Some(root_id) = root {
            let mut type_checker = TypeChecker::new(&mut self.arena);
            type_checker.check(root_id);
            if type_checker.had_error {
                return Err("Type Error occurred.".to_string());
            }
            let context = inkwell::context::Context::create();
            let module = context.create_module("main");
            let builder = context.create_builder();

            let mut codegen = Codegen::new(&context, &builder, &module, &self.arena);
            if let Err(e) = codegen.compile(root_id) {
                return Err(format!("Codegen Error: {}", e));
            }

            if let Err(e) = module.verify() {
                let err_str: inkwell::support::LLVMString = e;
                return Err(format!("LLVM Verification Error: {}", err_str.to_string()));
            }

            Self::link_and_emit(&module, out_bin, build_mode)?;
            Ok(())
        } else {
            Ok(())
        }
    }

    /// Writes LLVM IR to disk, compiles with llc, links with gcc + C runtime,
    /// and optionally executes the resulting binary.
    fn link_and_emit(
        module: &inkwell::module::Module,
        out_bin: &str,
        build_mode: bool,
    ) -> Result<(), String> {
        let obj_path = format!("{}.o", out_bin);

        // Compile IR → object file via inkwell
        eprintln!("Compiling IR to object code...");
        inkwell::targets::Target::initialize_native(&inkwell::targets::InitializationConfig::default()).map_err(|e| e.to_string())?;
        let triple = inkwell::targets::TargetMachine::get_default_triple();
        let target = inkwell::targets::Target::from_triple(&triple).map_err(|e| e.to_string())?;
        let target_machine = target
            .create_target_machine(
                &triple,
                "generic",
                "",
                inkwell::OptimizationLevel::Aggressive,
                inkwell::targets::RelocMode::PIC,
                inkwell::targets::CodeModel::Default,
            )
            .ok_or_else(|| "Failed to create target machine".to_string())?;

        target_machine
            .write_to_file(module, inkwell::targets::FileType::Object, std::path::Path::new(&obj_path))
            .map_err(|e| e.to_string())?;

        // Compile runtime C modules (recursive) into object files
        let runtime_c_files = collect_runtime_c_files(Path::new("src/runtime"))?;
        let runtime_include_args = [
            "-Isrc/runtime".to_string(),
            "-Isrc/runtime/vendor/akari/include".to_string(),
            "-Isrc/runtime/vendor/akari/vendor/picohttpparser".to_string(),
            "-Isrc/runtime/vendor/akari/vendor/jsmn".to_string(),
            "-include".to_string(),
            "src/runtime/http_config.h".to_string(),
        ];

        let mut extra_cflags = Vec::new();
        if let Ok(raw) = std::env::var("CIPR_CFLAGS") {
            extra_cflags.extend(raw.split_whitespace().map(|s| s.to_string()));
        }

        let mut runtime_objects = Vec::new();
        for (i, c_file) in runtime_c_files.iter().enumerate() {
            let runtime_obj = format!("{}.runtime.{}.o", out_bin, i);
            let mut cc_args = vec!["-O3".to_string(), "-c".to_string()];
            cc_args.extend(runtime_include_args.iter().cloned());
            cc_args.extend(extra_cflags.iter().cloned());
            cc_args.push(c_file.to_string_lossy().into_owned());
            cc_args.push("-o".to_string());
            cc_args.push(runtime_obj.clone());

            let cc_ok = std::process::Command::new("gcc")
                .args(&cc_args)
                .status()
                .map_err(|e| format!("Failed to invoke gcc (runtime compile): {}", e))?;
            if !cc_ok.success() {
                return Err(format!(
                    "gcc runtime compilation failed for {}",
                    c_file.display()
                ));
            }
            runtime_objects.push(runtime_obj);
        }

        // Link Cipr object + runtime objects
        eprintln!("C Runtime Linked...");
        let mut gcc_args = vec![obj_path.clone()];
        gcc_args.extend(runtime_objects.iter().cloned());
        gcc_args.extend(["-o".to_string(), out_bin.to_string()]);
        let gcc_ok = std::process::Command::new("gcc")
            .args(&gcc_args)
            .status()
            .map_err(|e| format!("Failed to invoke gcc: {}", e))?;
        if !gcc_ok.success() {
            return Err("gcc linking failed!".to_string());
        }

        // Run binary (unless --build mode), then clean up intermediates
        if !build_mode {
            let run_ok = std::process::Command::new(format!("./{}", out_bin))
                .status()
                .map_err(|e| format!("Failed to run executable: {}", e))?;
            if !run_ok.success() {
                return Err("Program execution failed.".to_string());
            }
            let _ = std::fs::remove_file(&obj_path);
            for runtime_obj in runtime_objects {
                let _ = std::fs::remove_file(runtime_obj);
            }
            let _ = std::fs::remove_file(out_bin);
        } else {
            eprintln!("Build finished: ./{}", out_bin);
        }

        Ok(())
    }
}

fn collect_runtime_c_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_runtime_c_files_inner(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_runtime_c_files_inner(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| {
        format!(
            "Failed to read runtime directory '{}': {}",
            dir.display(),
            e
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_runtime_c_files_inner(&path, out)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("c") {
            out.push(path);
        }
    }
    Ok(())
}
