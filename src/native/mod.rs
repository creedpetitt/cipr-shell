mod core_fns;
mod crypto;
mod file;
mod string;

#[cfg(not(target_arch = "wasm32"))]
mod net;
#[cfg(not(target_arch = "wasm32"))]
mod sys;

use crate::interpreter::Interpreter;
use std::rc::Rc;

pub fn register_all(interp: &mut Interpreter) {
    // Core
    interp.define_global("print", Rc::new(core_fns::NativePrint));
    interp.define_global("time", Rc::new(core_fns::NativeTime));
    #[cfg(not(target_arch = "wasm32"))]
    interp.define_global("run", Rc::new(core_fns::NativeRun));
    interp.define_global("env", Rc::new(core_fns::NativeEnv));
    interp.define_global("cwd", Rc::new(core_fns::NativeCwd));
    interp.define_global("cd", Rc::new(core_fns::NativeCd));
    interp.define_global("include", Rc::new(core_fns::NativeInclude));
    interp.define_global("rand", Rc::new(core_fns::NativeRand));
    interp.define_global("sleep", Rc::new(core_fns::NativeSleep));
    interp.define_global("exit", Rc::new(core_fns::NativeExit));

    // File
    interp.define_global("read_file", Rc::new(file::NativeReadFile));
    interp.define_global("write_file", Rc::new(file::NativeWriteFile));
    interp.define_global("ls", Rc::new(file::NativeLs));

    // String
    interp.define_global("size", Rc::new(string::NativeSize));
    interp.define_global("trim", Rc::new(string::NativeTrim));
    interp.define_global("split", Rc::new(string::NativeSplit));
    interp.define_global("extract", Rc::new(string::NativeExtract));

    // Net
    #[cfg(not(target_arch = "wasm32"))]
    {
        interp.define_global("connect", Rc::new(net::NativeConnect));
        interp.define_global("send", Rc::new(net::NativeSend));
        interp.define_global("recv", Rc::new(net::NativeRecv));
        interp.define_global("close", Rc::new(net::NativeClose));
        interp.define_global("http_get", Rc::new(net::NativeHttpGet));
        interp.define_global("http_post", Rc::new(net::NativeHttpPost));
        interp.define_global("listen", Rc::new(net::NativeListen));
        interp.define_global("accept", Rc::new(net::NativeAccept));
    }

    // Crypto
    interp.define_global("hex", Rc::new(crypto::NativeHex));
    interp.define_global("base64_encode", Rc::new(crypto::NativeBase64Encode));
    interp.define_global("base64_decode", Rc::new(crypto::NativeBase64Decode));

    // Sys
    #[cfg(not(target_arch = "wasm32"))]
    {
        interp.define_global("ps", Rc::new(sys::NativePs));
        interp.define_global("kill", Rc::new(sys::NativeKill));
    }
}

use crate::ast::CiprType;
use crate::type_checker::TypeChecker;

pub fn register_types(checker: &mut TypeChecker) {
    checker.define_global(
        "print",
        CiprType::Callable(vec![CiprType::Unknown], Box::new(CiprType::Void)),
    );
    checker.define_global(
        "time",
        CiprType::Callable(vec![], Box::new(CiprType::Float)),
    );

    #[cfg(not(target_arch = "wasm32"))]
    checker.define_global(
        "run",
        CiprType::Callable(vec![CiprType::Str], Box::new(CiprType::Str)),
    );

    checker.define_global(
        "env",
        CiprType::Callable(vec![CiprType::Str], Box::new(CiprType::Str)),
    );
    checker.define_global("cwd", CiprType::Callable(vec![], Box::new(CiprType::Str)));
    checker.define_global(
        "cd",
        CiprType::Callable(vec![CiprType::Str], Box::new(CiprType::Bool)),
    );
    checker.define_global(
        "include",
        CiprType::Callable(vec![CiprType::Str], Box::new(CiprType::Bool)),
    );
    checker.define_global(
        "rand",
        CiprType::Callable(vec![CiprType::Int], Box::new(CiprType::Int)),
    );
    checker.define_global(
        "sleep",
        CiprType::Callable(vec![CiprType::Int], Box::new(CiprType::Void)),
    );
    checker.define_global(
        "exit",
        CiprType::Callable(vec![CiprType::Int], Box::new(CiprType::Void)),
    );

    checker.define_global(
        "read_file",
        CiprType::Callable(vec![CiprType::Str], Box::new(CiprType::Str)),
    );
    checker.define_global(
        "write_file",
        CiprType::Callable(vec![CiprType::Str, CiprType::Str], Box::new(CiprType::Bool)),
    );
    checker.define_global(
        "ls",
        CiprType::Callable(
            vec![CiprType::Str],
            Box::new(CiprType::Array(Box::new(CiprType::Str))),
        ),
    );

    checker.define_global(
        "size",
        CiprType::Callable(vec![CiprType::Unknown], Box::new(CiprType::Int)),
    );
    checker.define_global(
        "trim",
        CiprType::Callable(vec![CiprType::Str], Box::new(CiprType::Str)),
    );
    checker.define_global(
        "split",
        CiprType::Callable(
            vec![CiprType::Str, CiprType::Str],
            Box::new(CiprType::Array(Box::new(CiprType::Str))),
        ),
    );
    checker.define_global(
        "extract",
        CiprType::Callable(
            vec![CiprType::Str, CiprType::Str, CiprType::Str],
            Box::new(CiprType::Str),
        ),
    );

    #[cfg(not(target_arch = "wasm32"))]
    {
        checker.define_global(
            "connect",
            CiprType::Callable(vec![CiprType::Str, CiprType::Int], Box::new(CiprType::Int)),
        );
        checker.define_global(
            "send",
            CiprType::Callable(vec![CiprType::Int, CiprType::Str], Box::new(CiprType::Int)),
        );
        checker.define_global(
            "recv",
            CiprType::Callable(vec![CiprType::Int, CiprType::Int], Box::new(CiprType::Str)),
        );
        checker.define_global(
            "close",
            CiprType::Callable(vec![CiprType::Int], Box::new(CiprType::Bool)),
        );
        checker.define_global(
            "http_get",
            CiprType::Callable(vec![CiprType::Str], Box::new(CiprType::Str)),
        );
        checker.define_global(
            "http_post",
            CiprType::Callable(vec![CiprType::Str, CiprType::Str], Box::new(CiprType::Str)),
        );
        checker.define_global(
            "listen",
            CiprType::Callable(vec![CiprType::Int], Box::new(CiprType::Int)),
        );
        checker.define_global(
            "accept",
            CiprType::Callable(vec![CiprType::Int], Box::new(CiprType::Int)),
        );
    }

    checker.define_global(
        "hex",
        CiprType::Callable(vec![CiprType::Str], Box::new(CiprType::Str)),
    );
    checker.define_global(
        "base64_encode",
        CiprType::Callable(vec![CiprType::Str], Box::new(CiprType::Str)),
    );
    checker.define_global(
        "base64_decode",
        CiprType::Callable(vec![CiprType::Str], Box::new(CiprType::Str)),
    );

    #[cfg(not(target_arch = "wasm32"))]
    {
        checker.define_global(
            "ps",
            CiprType::Callable(vec![], Box::new(CiprType::Array(Box::new(CiprType::Str)))),
        );
        checker.define_global(
            "kill",
            CiprType::Callable(vec![CiprType::Int], Box::new(CiprType::Bool)),
        );
    }
}
