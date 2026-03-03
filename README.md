# Cipr-Shell

![Build Status](https://github.com/Creed-Petitt/cipr-shell/actions/workflows/cmake.yml/badge.svg)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS-lightgrey)
![License](https://img.shields.io/badge/license-MIT-blue)

**Cipr** is a lightweight, zero-dependency scripting language designed for systems administration, network automation, and security testing. Built in C++17, it combines the familiar syntax of C-family languages with high-level native primitives for socket networking, process management, and file I/O.


## Key Features

*   **Zero Dependencies**: Compiles to a single, standalone binary.
*   **WebAssembly Powered**: Run the entire interpreter engine in the browser.
*   **Network Native**: Support for TCP sockets (`listen`, `accept`) and HTTP (`http_get`).
*   **System Integration**: Direct access to process lists (`ps`, `kill`) and environment variables.
*   **Extensible**: Module system via `include()` and persistent libraries.
*   **Modern C++**: Built using Recursive Descent Parsing with an AST Arena architecture.

## Try it Online

You can run Cipr scripts directly in your browser without installing anything. The playground is powered by WebAssembly and includes a live AST explorer.

**[Cipr Web Playground](https://cipr.creedpetitt.dev)**

## Installation

### One-Step Install (Linux/macOS)
```bash
git clone https://github.com/Creed-Petitt/cipr-shell.git
cd cipr-shell
./install.sh
```

### Manual Build
Requirements: CMake 3.20+, C++17 Compiler (GCC/Clang).
```bash
mkdir build && cd build
cmake .. -DCMAKE_BUILD_TYPE=Release
make
sudo cp cipr /usr/local/bin/
```

### Docker
Run Cipr in an isolated container without installing:
```bash
docker build -t cipr .
docker run -it cipr
```

## Quick Start

Launch the interactive shell:
```bash
cipr
```

### 1. Hello World
```js
let name = "Hacker";
echo "Hello, " + name;
```

### 2. Web Scraping
```js
let html = http_get("http://example.com");
let title = extract(html, "<title>", "</title>");
echo "Target Title: " + title;
```

### 3. TCP Server
Create a listening server in 5 lines of code.
```js
let srv = listen(8080);
echo "Listening on 8080...";

while (true) {
    let client = accept(srv);
    send(client, "Welcome to Cipr v1.0\n");
    close(client);
}
```

### 4. Fast Port Scanner
Scan targets for open ports in seconds.
```js
let target = "127.0.0.1";
for (let p = 8000; p < 8005; p = p + 1) {
    let fd = connect(target, p);
    if (fd > 0) {
        echo "[+] OPEN: " + p;
        close(fd);
    }
}
```

## Standard Library Reference

| Module | Functions | Description |
| :--- | :--- | :--- |
| **System** | `ls`, `ps`, `kill`, `env`, `run`, `cd`, `cwd` | OS interaction and process management. |
| **Network** | `http_get`, `listen`, `accept`, `connect`, `send` | TCP sockets and HTTP clients. |
| **File I/O** | `read_file`, `write_file`, `include`, `save_lib` | File operations and script modularity. |
| **Data** | `extract`, `split`, `trim`, `hex`, `base64` | String parsing and cryptographic encoding. |
| **Utilities** | `rand`, `sleep`, `time`, `clock` | Timing, delays, and randomization. |


## Documentation
For complete language syntax and API details, see the **[Language Manual](docs/MANUAL.md)**.

Type `help()` in the shell to see all commands.
Type `man("net")` for network commands.


## Architecture

Cipr is implemented as a tree-walk interpreter:
1.  **Scanner**: Tokenizes source code into a stream.
2.  **Parser**: recursive descent parser constructs an Abstract Syntax Tree (AST).
3.  **Arena**: AST Nodes are stored in a `std::deque` based Memory Arena for stability and performance.
4.  **Interpreter**: Traverses the AST to execute logic, managing scopes via chained Environments.

## Roadmap

*   [ ] **v1.1**: Pure C Port (Removal of STL).
*   [ ] **v1.2**: Bytecode Compiler & Stack VM.
*   [ ] **v1.3**: Native JSON Support.

## License

MIT

