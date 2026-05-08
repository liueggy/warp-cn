#!/usr/bin/env bash
# warp-cn build script — sets up environment and provides fast check/build
# Usage:
#   source build.sh            # load env
#   source build.sh && check   # fast verify (seconds)
#   source build.sh && build   # full binary (minutes)

export PROTOC="/e/tmp/protoc/bin/protoc.exe"
export PROTOC_INCLUDE="/e/tmp/protoc/include"
export TMP="E:\\tmp"
export TEMP="E:\\tmp"
export TMPDIR="E:\\tmp"

# warp-cn specific build flags for low-memory machines (16GB)
export CARGO_BUILD_RUSTFLAGS="-C opt-level=0 -C debuginfo=0 -C codegen-units=65535 -C lto=off"
export RUST_MIN_STACK=67108864
export CARGO_BUILD_JOBS=1

function check() {
    cd /e/warp-cn
    echo ">>> cargo check -p warp (fast, no linking)"
    cargo check -p warp 2>&1 | tail -10
}

function build() {
    cd /e/warp-cn
    # kill running instance to avoid file-lock errors
    powershell -Command "Stop-Process -Name warp-oss -Force -ErrorAction SilentlyContinue" 2>/dev/null
    sleep 1
    rm -f target/debug/warp-oss.exe 2>/dev/null
    echo ">>> cargo build -p warp (with linking)"
    cargo build -p warp 2>&1 | tail -10
    if [ -f target/debug/warp-oss.exe ]; then
        echo ">>> Build OK: target/debug/warp-oss.exe"
    fi
}

echo "warp-cn build env loaded. Use: check  or  build"
