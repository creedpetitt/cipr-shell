use crate::interpreter::{Callable, CiprError, Interpreter, RuntimeValue};

pub struct NativeSize;
impl Callable for NativeSize {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let len = match &args[0] {
            RuntimeValue::Array(a) => a.len(),
            RuntimeValue::Str(s) => s.len(),
            _ => 0,
        };
        Ok(RuntimeValue::Int(len as i64))
    }
    fn to_string(&self) -> String {
        "<native fn size>".to_string()
    }
}

pub struct NativeTrim;
impl Callable for NativeTrim {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        match &args[0] {
            RuntimeValue::Str(s) => Ok(RuntimeValue::Str(s.trim().to_string())),
            other => Ok(other.clone()),
        }
    }
    fn to_string(&self) -> String {
        "<native fn trim>".to_string()
    }
}

pub struct NativeSplit;
impl Callable for NativeSplit {
    fn arity(&self) -> usize {
        2
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let s = match &args[0] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Array(Vec::new())),
        };
        let delim = match &args[1] {
            RuntimeValue::Str(d) => d.clone(),
            _ => return Ok(RuntimeValue::Array(Vec::new())),
        };

        let parts: Vec<RuntimeValue> = s
            .split(&delim)
            .map(|p| RuntimeValue::Str(p.to_string()))
            .collect();
        Ok(RuntimeValue::Array(parts))
    }
    fn to_string(&self) -> String {
        "<native fn split>".to_string()
    }
}

pub struct NativeExtract;
impl Callable for NativeExtract {
    fn arity(&self) -> usize {
        3
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let src = match &args[0] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Null),
        };
        let start_delim = match &args[1] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Null),
        };
        let end_delim = match &args[2] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Null),
        };

        let s_pos = match src.find(&start_delim) {
            Some(pos) => pos + start_delim.len(),
            None => return Ok(RuntimeValue::Null),
        };

        let e_pos = match src[s_pos..].find(&end_delim) {
            Some(pos) => s_pos + pos,
            None => return Ok(RuntimeValue::Null),
        };

        Ok(RuntimeValue::Str(src[s_pos..e_pos].to_string()))
    }
    fn to_string(&self) -> String {
        "<native fn extract>".to_string()
    }
}
