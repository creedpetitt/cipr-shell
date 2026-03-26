use std::fs;
use std::io::{self, Write};
use std::path::Path;

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
        let ext = path_obj.extension().and_then(std::ffi::OsStr::to_str).unwrap_or("");
        
        // Compute output binary path
        let file_stem = path_obj.file_stem().and_then(std::ffi::OsStr::to_str).unwrap_or("out");
        let parent = path_obj.parent().unwrap_or(Path::new(""));
        let out_path = parent.join(file_stem);

        if ext == "cipr" {
            self.run(&source, build_mode, out_path.to_str().unwrap_or("out"))
        } else {
            Err("Unsupported file extension. Expected .cipr".to_string())
        }
    }

    pub fn run_prompt(&mut self) {
        let mut buffer = String::new();
        let mut brace_count: i32 = 0;

        loop {
            if buffer.is_empty() {
                print!("> ");
            } else {
                print!("... ");
            }
            io::stdout().flush().unwrap();

            let mut line = String::new();
            if io::stdin().read_line(&mut line).unwrap() == 0 {
                break;
            }

            for c in line.chars() {
                if c == '{' {
                    brace_count += 1;
                }
                if c == '}' {
                    brace_count -= 1;
                }
            }

            buffer.push_str(&line);

            if brace_count <= 0 && !buffer.trim().is_empty() {
                if let Err(e) = self.run(&buffer, false, "repl_out") {
                    eprintln!("{}", e);
                }
                buffer.clear();
                brace_count = 0;
            }
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

                // Verify module
                if let Err(e) = module.verify() {
                    let err_str: inkwell::support::LLVMString = e;
                    return Err(format!("LLVM Verification Error: {}", err_str.to_string()));
                }

                // Write LLVM IR to a file
                let ir_path = format!("{}.ll", out_bin);
                module.print_to_file(&ir_path).map_err(|e: inkwell::support::LLVMString| e.to_string())?;

                // Invoke llc-14 to compile IR to object file
                let obj_path = format!("{}.o", out_bin);
                println!("Compiling IR to object code...");
                let status_llc = std::process::Command::new("llc-14")
                    .args(["-O3", "-filetype=obj", "-relocation-model=pic", &ir_path, "-o", &obj_path])
                    .status()
                    .map_err(|e| format!("Failed to invoke llc-14: {}", e))?;

                if !status_llc.success() {
                    return Err("llc-14 compilation failed!".to_string());
                }

                // Invoke gcc to link object file with all C runtime modules
                println!("C Runtime Linked...");
                
                let mut gcc_args = vec![obj_path.clone()];
                if let Ok(entries) = std::fs::read_dir("src/runtime") {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("c") {
                            gcc_args.push(path.to_string_lossy().to_string());
                        }
                    }
                }
                gcc_args.push("-o".to_string());
                gcc_args.push(out_bin.to_string());

                let status_gcc = std::process::Command::new("gcc")
                    .args(&gcc_args)
                    .status()
                    .map_err(|e| format!("Failed to invoke gcc: {}", e))?;

                if !status_gcc.success() {
                    return Err("gcc linking failed!".to_string());
                }

                if !build_mode {
                    // Execute the built binary directly
                    let status_run = std::process::Command::new(format!("./{}", out_bin))
                        .status()
                        .map_err(|e| format!("Failed to run executable: {}", e))?;
                    
                    if !status_run.success() {
                        return Err("Program execution failed.".to_string());
                    }

                    // Clean up intermediate files and binary
                    let _ = std::fs::remove_file(ir_path);
                    let _ = std::fs::remove_file(obj_path);
                    let _ = std::fs::remove_file(out_bin);
                } else {
                    println!("Build finished: ./{}", out_bin);
                }

                Ok(())
        } else {
            Ok(())
        }
    }
}
