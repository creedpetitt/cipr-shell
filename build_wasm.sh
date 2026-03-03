#!/bin/bash
mkdir -p playground/wasm

if [ -f "$HOME/emsdk/emsdk_env.sh" ]; then
    source "$HOME/emsdk/emsdk_env.sh"
fi

echo "Compiling Cipr to WebAssembly..."

emcc -O3 -std=c++17 -fexceptions \
    -I./src \
    src/Scanner/*.cpp \
    src/Parser/*.cpp \
    src/AST/*.cpp \
    src/Environment/*.cpp \
    src/Interpreter/*.cpp \
    src/Native/*.cpp \
    src/Core/*.cpp \
    src/wasm_bindings.cpp \
    -o playground/wasm/cipr.js \
    -s WASM=1 \
    -s EXPORT_NAME="createCiprModule" \
    -s MODULARIZE=1 \
    -lembind \
    -D__EMSCRIPTEN__

echo "WebAssembly build complete! Output is in playground/wasm/"
