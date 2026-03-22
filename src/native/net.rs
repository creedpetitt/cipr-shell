use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::fd::{FromRawFd, IntoRawFd};

use crate::interpreter::{Callable, CiprError, Interpreter, RuntimeValue};

pub struct NativeConnect;
impl Callable for NativeConnect {
    fn arity(&self) -> usize {
        2
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let host = match &args[0] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Int(-1)),
        };
        let port = match &args[1] {
            RuntimeValue::Int(n) => *n as u16,
            _ => return Ok(RuntimeValue::Int(-1)),
        };

        let addr = format!("{host}:{port}");
        match TcpStream::connect(&addr) {
            Ok(stream) => Ok(RuntimeValue::Int(stream.into_raw_fd() as i64)),
            Err(_) => Ok(RuntimeValue::Int(-1)),
        }
    }
    fn to_string(&self) -> String {
        "<native fn connect>".to_string()
    }
}

pub struct NativeSend;
impl Callable for NativeSend {
    fn arity(&self) -> usize {
        2
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let fd = match &args[0] {
            RuntimeValue::Int(n) => *n as i32,
            _ => return Ok(RuntimeValue::Int(-1)),
        };
        let data = match &args[1] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Int(-1)),
        };

        let mut stream = unsafe { TcpStream::from_raw_fd(fd) };
        let result = stream.write(data.as_bytes());
        let _ = stream.into_raw_fd(); // don't drop/close

        match result {
            Ok(n) => Ok(RuntimeValue::Int(n as i64)),
            Err(_) => Ok(RuntimeValue::Int(-1)),
        }
    }
    fn to_string(&self) -> String {
        "<native fn send>".to_string()
    }
}

pub struct NativeRecv;
impl Callable for NativeRecv {
    fn arity(&self) -> usize {
        2
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let fd = match &args[0] {
            RuntimeValue::Int(n) => *n as i32,
            _ => return Ok(RuntimeValue::Null),
        };
        let sz = match &args[1] {
            RuntimeValue::Int(n) => *n as usize,
            _ => return Ok(RuntimeValue::Null),
        };

        let mut stream = unsafe { TcpStream::from_raw_fd(fd) };
        let mut buf = vec![0u8; sz];
        let result = stream.read(&mut buf);
        let _ = stream.into_raw_fd();

        match result {
            Ok(n) if n > 0 => Ok(RuntimeValue::Str(
                String::from_utf8_lossy(&buf[..n]).to_string(),
            )),
            _ => Ok(RuntimeValue::Null),
        }
    }
    fn to_string(&self) -> String {
        "<native fn recv>".to_string()
    }
}

pub struct NativeClose;
impl Callable for NativeClose {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let fd = match &args[0] {
            RuntimeValue::Int(n) => *n as i32,
            _ => return Ok(RuntimeValue::Bool(false)),
        };
        unsafe {
            libc::close(fd);
        }
        Ok(RuntimeValue::Bool(true))
    }
    fn to_string(&self) -> String {
        "<native fn close>".to_string()
    }
}

pub struct NativeHttpGet;
impl Callable for NativeHttpGet {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let mut url = match &args[0] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Null),
        };

        if let Some(rest) = url.strip_prefix("http://") {
            url = rest.to_string();
        }

        let (host, path) = match url.find('/') {
            Some(pos) => (url[..pos].to_string(), url[pos..].to_string()),
            None => (url.clone(), "/".to_string()),
        };

        let (host, port) = match host.find(':') {
            Some(pos) => (host[..pos].to_string(), host[pos + 1..].to_string()),
            None => (host, "80".to_string()),
        };

        let addr = format!("{host}:{port}");
        let mut stream = match TcpStream::connect(&addr) {
            Ok(s) => s,
            Err(_) => return Ok(RuntimeValue::Null),
        };

        let req = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
        if stream.write_all(req.as_bytes()).is_err() {
            return Ok(RuntimeValue::Null);
        }

        let mut raw = String::new();
        let _ = stream.read_to_string(&mut raw);

        if let Some(pos) = raw.find("\r\n\r\n") {
            Ok(RuntimeValue::Str(raw[pos + 4..].to_string()))
        } else {
            Ok(RuntimeValue::Str(raw))
        }
    }
    fn to_string(&self) -> String {
        "<native fn http_get>".to_string()
    }
}

pub struct NativeHttpPost;
impl Callable for NativeHttpPost {
    fn arity(&self) -> usize {
        2
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let mut url = match &args[0] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Null),
        };
        let body = match &args[1] {
            RuntimeValue::Str(s) => s.clone(),
            _ => return Ok(RuntimeValue::Null),
        };

        if let Some(rest) = url.strip_prefix("http://") {
            url = rest.to_string();
        }

        let (host, path) = match url.find('/') {
            Some(pos) => (url[..pos].to_string(), url[pos..].to_string()),
            None => (url.clone(), "/".to_string()),
        };

        let (host, port) = match host.find(':') {
            Some(pos) => (host[..pos].to_string(), host[pos + 1..].to_string()),
            None => (host, "80".to_string()),
        };

        let addr = format!("{host}:{port}");
        let mut stream = match TcpStream::connect(&addr) {
            Ok(s) => s,
            Err(_) => return Ok(RuntimeValue::Null),
        };

        let req = format!(
            "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        if stream.write_all(req.as_bytes()).is_err() {
            return Ok(RuntimeValue::Null);
        }

        let mut raw = String::new();
        let _ = stream.read_to_string(&mut raw);

        if let Some(pos) = raw.find("\r\n\r\n") {
            Ok(RuntimeValue::Str(raw[pos + 4..].to_string()))
        } else {
            Ok(RuntimeValue::Str(raw))
        }
    }
    fn to_string(&self) -> String {
        "<native fn http_post>".to_string()
    }
}

pub struct NativeListen;
impl Callable for NativeListen {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let port = match &args[0] {
            RuntimeValue::Int(n) => *n as u16,
            _ => return Ok(RuntimeValue::Int(-1)),
        };

        let addr = format!("0.0.0.0:{port}");
        match TcpListener::bind(&addr) {
            Ok(listener) => Ok(RuntimeValue::Int(listener.into_raw_fd() as i64)),
            Err(_) => Ok(RuntimeValue::Int(-1)),
        }
    }
    fn to_string(&self) -> String {
        "<native fn listen>".to_string()
    }
}

pub struct NativeAccept;
impl Callable for NativeAccept {
    fn arity(&self) -> usize {
        1
    }
    fn call(
        &self,
        _: &mut Interpreter,
        args: Vec<RuntimeValue>,
    ) -> Result<RuntimeValue, CiprError> {
        let fd = match &args[0] {
            RuntimeValue::Int(n) => *n as i32,
            _ => return Ok(RuntimeValue::Int(-1)),
        };

        let listener = unsafe { TcpListener::from_raw_fd(fd) };
        let result = match listener.accept() {
            Ok((stream, _)) => RuntimeValue::Int(stream.into_raw_fd() as i64),
            Err(_) => RuntimeValue::Int(-1),
        };
        let _ = listener.into_raw_fd(); // don't drop/close
        Ok(result)
    }
    fn to_string(&self) -> String {
        "<native fn accept>".to_string()
    }
}
