use std::env;
use std::fs;
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rand::Rng;

use crate::interpreter::{Callable, CiprError, Interpreter, RuntimeValue};
use crate::parser::Parser;
use crate::scanner::Scanner;

pub struct NativePrint;
impl Callable for NativePrint {
    fn arity(&self) -> usize {
        1
    } // We can make it variadic later, but 1 for now
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        println!("{}", crate::interpreter::stringify_value(&args[0]));
        Ok(RuntimeValue::Null)
    }
    fn to_string(&self) -> String {
        "<native fn print>".to_string()
    }
}

pub struct NativeTime;
impl Callable for NativeTime {
    fn arity(&self) -> usize {
        0
    }
    fn call(&self, _: &mut Interpreter, _: Vec<RuntimeValue>) -> Result<RuntimeValue, CiprError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Ok(RuntimeValue::Float(duration.as_secs_f64()))
    }
    fn to_string(&self) -> String {
        "<native fn time>".to_string()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct NativeRun;
#[cfg(not(target_arch = "wasm32"))]
impl Callable for NativeRun {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let cmd = match &args[0] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Null),
        };
        let output = Command::new("sh").arg("-c").arg(&cmd).output();
        match output {
            Ok(o) => Ok(RuntimeValue::Str(
                String::from_utf8_lossy(&o.stdout).to_string(),
            )),
            Err(_) => Ok(RuntimeValue::Str("Error: Pipe failed".to_string())),
        }
    }
    fn to_string(&self) -> String {
        "<native fn run>".to_string()
    }
}

pub struct NativeEnv;
impl Callable for NativeEnv {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let key = match &args[0] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Null),
        };
        match env::var(&key) {
            Ok(val) => Ok(RuntimeValue::Str(val)),
            Err(_) => Ok(RuntimeValue::Null),
        }
    }
    fn to_string(&self) -> String {
        "<native fn env>".to_string()
    }
}

pub struct NativeCwd;
impl Callable for NativeCwd {
    fn arity(&self) -> usize {
        0
    }
    fn call(&self, _: &mut Interpreter, _: Vec<RuntimeValue>) -> Result<RuntimeValue, CiprError> {
        match env::current_dir() {
            Ok(p) => Ok(RuntimeValue::Str(p.to_string_lossy().to_string())),
            Err(_) => Ok(RuntimeValue::Null),
        }
    }
    fn to_string(&self) -> String {
        "<native fn cwd>".to_string()
    }
}

pub struct NativeCd;
impl Callable for NativeCd {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let path = match &args[0] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Bool(false)),
        };
        Ok(RuntimeValue::Bool(env::set_current_dir(&path).is_ok()))
    }
    fn to_string(&self) -> String {
        "<native fn cd>".to_string()
    }
}

pub struct NativeInclude;
impl Callable for NativeInclude {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        interp: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let filename = match &args[0] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Bool(false)),
        };

        // Try local path first, then ~/.cipr/libs/
        let source = if let Ok(s) = fs::read_to_string(&filename) {
            s
        } else if let Ok(home) = env::var("HOME") {
            let lib_path = format!("{home}/.cipr/libs/{filename}");
            match fs::read_to_string(&lib_path) {
                Ok(s) => s,
                Err(_) => return Ok(RuntimeValue::Bool(false)),
            }
        } else {
            return Ok(RuntimeValue::Bool(false));
        };

        let (tokens, scan_error) = Scanner::new(&source).scan_tokens();
        if scan_error {
            return Ok(RuntimeValue::Bool(false));
        }

        let mut parser = Parser::new(&tokens, &mut interp.arena);
        let root = parser.parse();
        if parser.had_error {
            return Ok(RuntimeValue::Bool(false));
        }

        if let Some(root_id) = root {
            interp.interpret(root_id);
        }

        Ok(RuntimeValue::Bool(true))
    }
    fn to_string(&self) -> String {
        "<native fn include>".to_string()
    }
}

pub struct NativeRand;
impl Callable for NativeRand {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let max = match &args[0] {
            RuntimeValue::Int(n) => *n,
            _ => return Ok(RuntimeValue::Int(0)),
        };
        if max <= 0 {
            return Ok(RuntimeValue::Int(0));
        }
        let mut rng = rand::rng();
        Ok(RuntimeValue::Int(rng.random_range(0..max)))
    }
    fn to_string(&self) -> String {
        "<native fn rand>".to_string()
    }
}

pub struct NativeSleep;
impl Callable for NativeSleep {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let ms = match &args[0] {
            RuntimeValue::Int(n) => *n as u64,
            _ => return Ok(RuntimeValue::Null),
        };
        thread::sleep(Duration::from_millis(ms));
        Ok(RuntimeValue::Null)
    }
    fn to_string(&self) -> String {
        "<native fn sleep>".to_string()
    }
}

pub struct NativeExit;
impl Callable for NativeExit {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let code = match &args[0] {
            RuntimeValue::Int(n) => *n as i32,
            _ => 0,
        };
        std::process::exit(code);
    }
    fn to_string(&self) -> String {
        "<native fn exit>".to_string()
    }
}
