@echo off
REM Build script for Windows

echo 🚀 Building abitur-analyzer...

REM Create build directory
if not exist builds mkdir builds

REM Build release version
echo Building release version...
cargo build --release

if %ERRORLEVEL% EQU 0 (
    echo ✅ Build successful!
    copy target\release\abitur-analyzer.exe builds\abitur-analyzer-windows-x64.exe
    echo.
    echo 📦 Built binary: builds\abitur-analyzer-windows-x64.exe
    echo.
    echo 💡 Usage: builds\abitur-analyzer-windows-x64.exe --snils "your-snils"
    echo.
) else (
    echo ❌ Build failed!
    exit /b 1
)

pause
