@echo off
setlocal
cd /d "%~dp0"

set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

where cargo >nul 2>nul
if errorlevel 1 (
  echo [ERROR] cargo not found. Install Rust from https://rustup.rs first.
  exit /b 1
)

for /f "tokens=2 delims== " %%v in ('findstr /b "version = " Cargo.toml') do set "VERSION=%%~v"
if "%VERSION%"=="" set "VERSION=0.1.0"

set "DIST=dist"
set "NAME=WA-Desk-portable-%VERSION%"
set "OUT=%DIST%\%NAME%"

echo [1/4] Building release binary...
cargo build --release -p wa-desktop
if errorlevel 1 goto :fail

echo [2/4] Preparing portable folder...
if exist "%OUT%" rmdir /s /q "%OUT%"
mkdir "%OUT%"
copy /y "target\release\wa-desktop.exe" "%OUT%\WA Desk.exe" >nul
if errorlevel 1 goto :fail

rem WebView2 is a Windows system component on Windows 10/11. If the build
rem produced a loader DLL, ship it next to the exe so older systems work too.
if exist "target\release\WebView2Loader.dll" (
  copy /y "target\release\WebView2Loader.dll" "%OUT%\WebView2Loader.dll" >nul
)

rem The AGPL requires the licence text to travel with the binary.
copy /y "LICENSE.md" "%OUT%\LICENSE.md" >nul
if errorlevel 1 goto :fail

> "%OUT%\README.txt" (
  echo WA Desk %VERSION% - portable build
  echo.
  echo Run "WA Desk.exe".
  echo.
  echo Uses the Microsoft Edge WebView2 runtime already included with
  echo Windows 10/11 and Microsoft Edge. No separate installer is needed.
  echo.
  echo Chat data, settings and media are stored in:
  echo   %%APPDATA%%\io.github.dirttw.wadesk
  echo To move the client to another PC, copy that folder as well.
  echo.
  echo This is an unofficial client, not affiliated with WhatsApp or Meta.
  echo.
  echo Licensed under the GNU Affero General Public License v3.0 only.
  echo See LICENSE.md in this folder for the full licence text.
  echo If you pass this copy on, the AGPL requires you to make the complete
  echo corresponding source code available to whoever receives it.
)

echo [3/4] Creating zip...
if exist "%DIST%\%NAME%.zip" del /q "%DIST%\%NAME%.zip"
powershell -NoProfile -ExecutionPolicy Bypass -Command "Compress-Archive -Path '%OUT%\*' -DestinationPath '%DIST%\%NAME%.zip' -Force"
if errorlevel 1 goto :fail

copy /y "%OUT%\WA Desk.exe" "%DIST%\WA Desk.exe" >nul

echo [4/4] Done.
echo.
echo   %DIST%\WA Desk.exe
echo   %DIST%\%NAME%.zip
echo.
exit /b 0

:fail
echo.
echo [ERROR] Build or packaging failed.
exit /b 1
