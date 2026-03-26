mod ast;
mod ast_printer;
mod codegen;
mod core;
mod parser;
mod scanner;
mod symbol_table;
mod token;
mod type_checker;

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut cipr = core::Core::new();

    if args.len() == 1 {
        cipr.run_prompt();
    } else {
        let build_mode = args.len() == 3 && args[1] == "--build";
        let file_path_index = if build_mode { 2 } else { 1 };

        if args.len() < 2 || (args.len() == 2 && build_mode) || args.len() > 3 {
            println!("Usage: cipr [--build] <script>");
            std::process::exit(64);
        }

        let file_path = &args[file_path_index];
        if let Err(e) = cipr.run_file(file_path, build_mode) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    }
}
