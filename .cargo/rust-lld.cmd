@echo off
setlocal

for /f "delims=" %%I in ('rustc --print sysroot') do set "RUST_LLD=%%I\lib\rustlib\x86_64-pc-windows-msvc\bin\rust-lld.exe"

if not exist "%RUST_LLD%" (
    echo rust-lld was not found in the active Rust toolchain 1>&2
    exit /b 1
)

"%RUST_LLD%" %* /ignore:4099
