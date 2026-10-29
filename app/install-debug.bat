@echo off
setlocal EnableExtensions EnableDelayedExpansion

REM Lingxi Android: build + install debug APK to connected phone
REM Requires: USB debugging enabled, adb device online, JDK 17, Android SDK
REM Multiple online devices: lists them and prompts for a number.

cd /d "%~dp0"

set "GRADLE_HOME=C:\rock\coding\tool\gradle-8.10-bin\gradle-8.10"
set "JAVA_HOME=C:\Program Files\Eclipse Adoptium\jdk-17.0.12.7-hotspot"
set "ANDROID_HOME=%LOCALAPPDATA%\Android\Sdk"
set "ANDROID_SDK_ROOT=%ANDROID_HOME%"
set "PATH=%JAVA_HOME%\bin;%GRADLE_HOME%\bin;%ANDROID_HOME%\platform-tools;%PATH%"

echo === Lingxi installDebug ===
echo GRADLE_HOME=%GRADLE_HOME%
echo JAVA_HOME=%JAVA_HOME%
echo ANDROID_HOME=%ANDROID_HOME%
echo.

if not exist "%GRADLE_HOME%\bin\gradle.bat" (
  echo [ERROR] Gradle not found: %GRADLE_HOME%\bin\gradle.bat
  exit /b 1
)
if not exist "%JAVA_HOME%\bin\java.exe" (
  echo [ERROR] JDK 17 not found: %JAVA_HOME%
  echo Edit JAVA_HOME in this script if your JDK path differs.
  exit /b 1
)
if not exist "%ANDROID_HOME%\platform-tools\adb.exe" (
  echo [ERROR] Android SDK platform-tools not found: %ANDROID_HOME%
  exit /b 1
)

echo --- adb devices ---
adb devices -l
echo.

REM Collect online devices (state=device) into SERIAL_1..N
set "COUNT=0"
for /f "tokens=1,2*" %%A in ('adb devices -l 2^>nul') do (
  if /I "%%B"=="device" (
    set /a COUNT+=1
    set "SERIAL_!COUNT!=%%A"
    set "META_!COUNT!=%%C"
  )
)

if %COUNT% EQU 0 (
  echo [ERROR] No device/emulator online ^(state=device^).
  echo Enable USB debugging and reconnect, or close stale emulator entries.
  exit /b 1
)

if !COUNT! EQU 1 (
  set "SERIAL=!SERIAL_1!"
  echo Using device: !SERIAL_1! !META_1!
) else (
  echo Multiple devices online — pick one:
  for /L %%I in (1,1,!COUNT!) do (
    echo   [%%I] !SERIAL_%%I!  !META_%%I!
  )
  echo.
  set /p "PICK=Enter number (1-!COUNT!): "
  set "SERIAL="
  for /L %%I in (1,1,!COUNT!) do (
    if "!PICK!"=="%%I" set "SERIAL=!SERIAL_%%I!"
  )
  if not defined SERIAL (
    echo [ERROR] Invalid selection: !PICK!
    exit /b 1
  )
  echo Using device: !SERIAL!
)
echo.

set "ANDROID_SERIAL=!SERIAL!"

echo --- gradle :app:installDebug ---
call "%GRADLE_HOME%\bin\gradle.bat" ":app:installDebug" --no-daemon
if errorlevel 1 (
  echo.
  echo [ERROR] Build/install failed.
  exit /b 1
)

echo.
echo --- launch app ---
adb -s !SERIAL! shell am start -n com.lingxi.app/.MainActivity
echo.
echo Done. View logs:  adb -s !SERIAL! logcat --pid=^(adb -s !SERIAL! shell pidof -s com.lingxi.app^)
endlocal
