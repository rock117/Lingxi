@echo off
setlocal EnableExtensions

REM Lingxi Android: build + install debug APK to connected phone
REM Requires: USB debugging enabled, adb device online, JDK 17, Android SDK

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

REM Fail early if no device
adb get-state 1>nul 2>nul
if errorlevel 1 (
  echo [ERROR] No device/emulator online. Enable USB debugging and reconnect.
  exit /b 1
)

echo --- gradle :app:installDebug ---
call "%GRADLE_HOME%\bin\gradle.bat" ":app:installDebug" --no-daemon
if errorlevel 1 (
  echo.
  echo [ERROR] Build/install failed.
  exit /b 1
)

echo.
echo --- launch app ---
adb shell am start -n com.lingxi.app/.MainActivity
echo.
echo Done. View logs:  adb logcat --pid=^((adb shell pidof -s com.lingxi.app^)^)
endlocal
