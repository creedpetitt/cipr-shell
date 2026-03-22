use std::fs;

use crate::interpreter::{Callable, CiprError, Interpreter, RuntimeValue};

pub struct NativeReadFile;
impl Callable for NativeReadFile {
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
            _ => return Ok(RuntimeValue::Null),
        };
        match fs::read_to_string(&path) {
            Ok(contents) => Ok(RuntimeValue::Str(contents)),
            Err(_) => Ok(RuntimeValue::Str("Error: Open failed".to_string())),
        }
    }
    fn to_string(&self) -> String {
        "<native fn read_file>".to_string()
    }
}

pub struct NativeWriteFile;
impl Callable for NativeWriteFile {
    fn arity(&self) -> usize {
        2
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
        let data = match &args[1] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Bool(false)),
        };
        Ok(RuntimeValue::Bool(fs::write(&path, &data).is_ok()))
    }
    fn to_string(&self) -> String {
        "<native fn write_file>".to_string()
    }
}

pub struct NativeLs;
impl Callable for NativeLs {
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
            _ => ".".to_string(),
        };
        match fs::read_dir(&path) {
            Ok(entries) => {
                let mut list = Vec::new();
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        list.push(RuntimeValue::Str(name.to_string()));
                    }
                }
                Ok(RuntimeValue::Array(list))
            }
            Err(_) => Ok(RuntimeValue::Null),
        }
    }
    fn to_string(&self) -> String {
        "<native fn ls>".to_string()
    }
}
