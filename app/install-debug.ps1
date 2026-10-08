# Lingxi Android: build + install debug APK to connected phone
# Usage:  powershell -ExecutionPolicy Bypass -File .\install-debug.ps1
# Optional: .\install-debug.ps1 -Launch -Logcat

param(
    [switch]$Launch = $true,
    [switch]$Logcat,
    [string]$GradleHome = "C:\rock\coding\tool\gradle-8.10-bin\gradle-8.10",
    [string]$JavaHome = "C:\Program Files\Eclipse Adoptium\jdk-17.0.12.7-hotspot",
    [string]$AndroidHome = "$env:LOCALAPPDATA\Android\Sdk"
)

$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

$env:GRADLE_HOME = $GradleHome
$env:JAVA_HOME = $JavaHome
$env:ANDROID_HOME = $AndroidHome
$env:ANDROID_SDK_ROOT = $AndroidHome
$env:Path = "$JavaHome\bin;$GradleHome\bin;$AndroidHome\platform-tools;$env:Path"

Write-Host "=== Lingxi installDebug ==="
Write-Host "GRADLE_HOME=$GradleHome"
Write-Host "JAVA_HOME=$JavaHome"
Write-Host "ANDROID_HOME=$AndroidHome"
Write-Host ""

$gradle = Join-Path $GradleHome "bin\gradle.bat"
$java = Join-Path $JavaHome "bin\java.exe"
$adb = Join-Path $AndroidHome "platform-tools\adb.exe"

if (-not (Test-Path $gradle)) { throw "Gradle not found: $gradle" }
if (-not (Test-Path $java)) { throw "JDK 17 not found: $JavaHome (pass -JavaHome to override)" }
if (-not (Test-Path $adb)) { throw "adb not found: $adb" }

Write-Host "--- adb devices ---"
& $adb devices -l
Write-Host ""

# adb 无设备时会往 stderr 打 error；在 ErrorActionPreference=Stop 下会被当成终止异常
$state = ""
try {
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $state = (& $adb get-state 2>&1 | Out-String).Trim()
} finally {
    $ErrorActionPreference = $prev
}

if ($state -ne "device") {
    Write-Host @"
未检测到手机/模拟器（adb get-state='$state'）。

请先任选其一：
  1) 真机：USB 连接 → 开启「开发者选项 / USB 调试」→ 手机上点「允许」
     然后执行: adb kill-server; adb start-server; adb devices
  2) 无线调试（Android 11+）: adb pair <ip:pairPort> 后再 adb connect <ip:port>
  3) 模拟器：Android Studio → Device Manager 启动一个 AVD

确认 `adb devices` 出现 device（不是 unauthorized / offline）后再重跑本脚本。
"@
    throw "No device/emulator online."
}

Write-Host "--- gradle :app:installDebug ---"
& $gradle ":app:installDebug" --no-daemon
if ($LASTEXITCODE -ne 0) {
    throw "Build/install failed (exit $LASTEXITCODE)."
}

if ($Launch) {
    Write-Host ""
    Write-Host "--- launch app ---"
    & $adb shell am start -n "com.lingxi.app/.MainActivity"
}

if ($Logcat) {
    Write-Host ""
    Write-Host "--- logcat (Ctrl+C to stop) ---"
    $pidOf = (& $adb shell pidof -s com.lingxi.app).Trim()
    if ($pidOf) {
        & $adb logcat --pid=$pidOf
    } else {
        & $adb logcat -s "Lingxi:*" "AndroidRuntime:E" "*:S"
    }
}

Write-Host ""
Write-Host "Done."
