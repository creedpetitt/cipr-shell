use crate::interpreter::{Callable, CiprError, Interpreter, RuntimeValue};

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub struct NativeHex;
impl Callable for NativeHex {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let s = match &args[0] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Null),
        };
        let hex: String = s.bytes().map(|b| format!("{b:02x}")).collect();
        Ok(RuntimeValue::Str(hex))
    }
    fn to_string(&self) -> String {
        "<native fn hex>".to_string()
    }
}

pub struct NativeBase64Encode;
impl Callable for NativeBase64Encode {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let s = match &args[0] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Null),
        };
        Ok(RuntimeValue::Str(b64_encode(s.as_bytes())))
    }
    fn to_string(&self) -> String {
        "<native fn base64_encode>".to_string()
    }
}

pub struct NativeBase64Decode;
impl Callable for NativeBase64Decode {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let s = match &args[0] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Null),
        };
        Ok(RuntimeValue::Str(b64_decode(&s)))
    }
    fn to_string(&self) -> String {
        "<native fn base64_decode>".to_string()
    }
}

fn b64_encode(input: &[u8]) -> String {
    let mut result = String::new();
    let chunks = input.chunks(3);
    for chunk in chunks {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(BASE64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(BASE64_CHARS[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(BASE64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(BASE64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn b64_decode(input: &str) -> String {
    let mut result = Vec::new();
    let bytes: Vec<u8> = input
        .bytes()
        .filter(|&b| b != b'=' && is_base64(b))
        .collect();

    let chunks = bytes.chunks(4);
    for chunk in chunks {
        let vals: Vec<u32> = chunk
            .iter()
            .map(|&b| BASE64_CHARS.iter().position(|&c| c == b).unwrap_or(0) as u32)
            .collect();

        let len = vals.len();
        if len >= 2 {
            result.push(((vals[0] << 2) | (vals[1] >> 4)) as u8);
        }
        if len >= 3 {
            result.push((((vals[1] & 0xF) << 4) | (vals[2] >> 2)) as u8);
        }
        if len >= 4 {
            result.push((((vals[2] & 0x3) << 6) | vals[3]) as u8);
        }
    }

    String::from_utf8_lossy(&result).to_string()
}

fn is_base64(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'+' || c == b'/'
}
