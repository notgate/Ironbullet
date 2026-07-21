@echo off
setlocal enabledelayedexpansion
set "REPO=%~dp0.."
for %%I in ("%REPO%") do set "REPO=%%~fI"
call "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat" -arch=x64 -host_arch=x64
if errorlevel 1 exit /b %errorlevel%
set "PATH=C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin;%PATH%"
set "CMAKE_ASM_NASM_COMPILER=%REPO%\.build-tools\nasm\nasm-2.16.01\nasm.exe"
if not exist "%CMAKE_ASM_NASM_COMPILER%" (
  echo Portable NASM is missing: %CMAKE_ASM_NASM_COMPILER%
  exit /b 1
)
for %%I in ("%CMAKE_ASM_NASM_COMPILER%") do set "PATH=%%~dpI;%PATH%"
set "LIBCLANG_PATH=%REPO%\.build-tools\llvm\clang+llvm-18.1.8-x86_64-pc-windows-msvc\bin"
if not exist "%LIBCLANG_PATH%\libclang.dll" (
  echo Portable LLVM is missing: %LIBCLANG_PATH%\libclang.dll
  exit /b 1
)
set "PATH=%LIBCLANG_PATH%;%PATH%"
cd /d "%REPO%\sidecar"
go build -trimpath -ldflags="-s -w" -o reqflow-sidecar.exe .
if errorlevel 1 exit /b %errorlevel%
cd /d "%REPO%"
cargo build --release
if errorlevel 1 exit /b %errorlevel%

set "XRAY_ARCHIVE=%REPO%\.build-tools\xray\v26.3.27\Xray-windows-64.zip"
set "XRAY_DIR=%REPO%\.build-tools\xray\v26.3.27\runtime"
if not exist "%XRAY_ARCHIVE%" (
  echo Bundled Xray archive is missing: %XRAY_ARCHIVE%
  exit /b 1
)
if not exist "%XRAY_DIR%\xray.exe" (
  powershell -NoProfile -ExecutionPolicy Bypass -Command "Expand-Archive -LiteralPath '%XRAY_ARCHIVE%' -DestinationPath '%XRAY_DIR%' -Force"
  if errorlevel 1 exit /b %errorlevel%
)
if not exist "%XRAY_DIR%\xray.exe" (
  echo Xray extraction failed: %XRAY_DIR%\xray.exe
  exit /b 1
)

set "DIST=%REPO%\dist\ironbullet-v0.6.2-rc.5-windows-x64"
if exist "%DIST%" rmdir /s /q "%DIST%"
mkdir "%DIST%"
copy /y "%REPO%\target\release\ironbullet.exe" "%DIST%\ironbullet.exe" >nul
copy /y "%REPO%\sidecar\reqflow-sidecar.exe" "%DIST%\reqflow-sidecar.exe" >nul
copy /y "%XRAY_DIR%\xray.exe" "%DIST%\xray.exe" >nul
copy /y "%XRAY_DIR%\LICENSE" "%DIST%\XRAY-LICENSE.txt" >nul

powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-ChildItem -LiteralPath '%DIST%' -File | Get-FileHash -Algorithm SHA256 | ForEach-Object { '{0}  {1}' -f $_.Hash.ToLower(), (Split-Path -Leaf $_.Path) } | Set-Content -Encoding ascii '%DIST%\release-manifest.sha256'; Compress-Archive -Path '%DIST%\*' -DestinationPath '%REPO%\dist\ironbullet-v0.6.2-rc.5-windows-x64.zip' -Force"
if errorlevel 1 exit /b %errorlevel%
echo Built bundle: %REPO%\dist\ironbullet-v0.6.2-rc.5-windows-x64.zip
