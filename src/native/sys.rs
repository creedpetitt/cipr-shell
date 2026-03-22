use std::fs;

use crate::interpreter::{Callable, CiprError, Interpreter, RuntimeValue};

pub struct NativePs;
impl Callable for NativePs {
    fn arity(&self) -> usize {
        0
    }
    fn call(&self, _: &mut Interpreter, _: Vec<RuntimeValue>) -> Result<RuntimeValue, CiprError> {
        let mut list = Vec::new();

        let entries = match fs::read_dir("/proc") {
            Ok(e) => e,
            Err(_) => return Ok(RuntimeValue::Null),
        };

        for entry in entries.flatten() {
            let name = entry.file_name();
            let pid_str = name.to_string_lossy().to_string();

            if !pid_str.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }

            let comm_path = entry.path().join("comm");
            if let Ok(comm) = fs::read_to_string(&comm_path) {
                let comm = comm.trim();
                list.push(RuntimeValue::Str(format!("{pid_str}: {comm}")));
            }
        }

        Ok(RuntimeValue::Array(list))
    }
    fn to_string(&self) -> String {
        "<native fn ps>".to_string()
    }
}

pub struct NativeKill;
impl Callable for NativeKill {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let pid = match &args[0] {
            RuntimeValue::Int(n) => *n as i32,
            _ => return Ok(RuntimeValue::Bool(false)),
        };
        let result = unsafe { libc::kill(pid, libc::SIGTERM) };
        Ok(RuntimeValue::Bool(result == 0))
    }
    fn to_string(&self) -> String {
        "<native fn kill>".to_string()
    }
}
