#!/bin/bash

set -e

echo "---Cipr Shell Installer v1.0---"

# Build
echo "[+] Building project..."
if ! command -v cmake &> /dev/null; then
    echo "Error: CMake is not installed. Please install cmake."
    exit 1
fi

if [ -d "build_install" ]; then
    rm -rf build_install
fi

mkdir -p build_install
cd build_install
cmake .. -DCMAKE_BUILD_TYPE=Release
cmake --build . --config Release

# Install Binary
echo "[+] Installing binary to /usr/local/bin..."
if [ -w /usr/local/bin ]; then
    cp cipr /usr/local/bin/cipr
else
    echo "    (Sudo required to write to /usr/local/bin)"
    sudo cp cipr /usr/local/bin/cipr
fi

# Setup User Environment
echo "[+] Setting up ~/.cipr environment..."
mkdir -p ~/.cipr/libs

if [ ! -f ~/.ciprrc ]; then
    echo "[+] Creating default ~/.ciprrc..."
    cat > ~/.ciprrc << 'EOF'

fn help() {
    echo "=== Cipr Shell v1.0 ===";
    echo "Type man(category) for details.";
    echo "Categories:";
    echo "  man(\"sys\")   - System (ls, ps, kill, env)";
    echo "  man(\"net\")   - Network (http, socket, server)";
    echo "  man(\"file\")  - File IO (read, write, include)";
    echo "  man(\"data\")  - Data (json, hex, base64)";
    echo "  man(\"util\")  - Utilities (rand, sleep, time)";
}

fn man(cat) {
    if (cat == "sys") {
        echo "--- System ---";
        echo "ls(path)      : List files";
        echo "ps()          : List processes";
        echo "kill(pid)     : Kill process";
        echo "run(cmd)      : Execute shell command";
        echo "env(name)     : Get env var";
        echo "cd(path)      : Change directory";
        echo "cwd()         : Get current directory";
    }
    if (cat == "net") {
        echo "--- Network ---";
        echo "http_get(url)       : GET Request";
        echo "http_post(url, body): POST Request";
        echo "listen(port)        : Start Server";
        echo "accept(fd)          : Accept Client";
        echo "connect(host, port) : Connect TCP";
        echo "send(fd, data)      : Send Raw";
        echo "recv(fd, size)      : Recv Raw";
        echo "close(fd)           : Close Socket";
    }
    if (cat == "file") {
        echo "--- File ---";
        echo "read_file(path)        : Read string";
        echo "write_file(path, data) : Write string";
        echo "include(path)          : Load script";
        echo "save_lib(name, code)   : Save to library";
    }
    if (cat == "data") {
        echo "--- Data ---";
        echo "extract(src, s, e) : Scrape string";
        echo "split(str, del)    : Split to array";
        echo "trim(str)          : Remove whitespace";
        echo "hex(str)           : To Hex";
        echo "base64_encode(s)   : To Base64";
        echo "base64_decode(s)   : From Base64";
    }
    if (cat == "util") {
        echo "--- Utility ---";
        echo "rand(max)   : Random number";
        echo "sleep(ms)   : Sleep (milliseconds)";
        echo "time()      : Current timestamp";
        echo "clock()     : CPU clock";
    }
}

fn save_lib(name, code) {
    let home = env("HOME");
    let path = home + "/.cipr/libs/" + name;
    write_file(path, code);
    echo "Library saved to: " + path;
}

echo "Cipr Shell v1.0 Loaded. Type help(); for info.";
EOF
else
    echo "[~] ~/.ciprrc already exists. Skipping config overwrite."
fi

# Cleanup
cd ..
rm -rf build_install


echo "[!] Installation Successful!"
echo "[!] Type 'cipr' to start."