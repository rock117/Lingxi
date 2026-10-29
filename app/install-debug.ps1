# Lingxi Android: build + install debug APK to connected phone
# Usage:  powershell -ExecutionPolicy Bypass -File .\install-debug.ps1
# Optional: .\install-debug.ps1 -Launch -Logcat

param(
    [switch]$Launch = $true,
    [switch]$Logcat,
    [string]$GradleHome = "C:\rock\coding\tool\gradle-8.10-bin\gradle-8.10",
    [string]$JavaHome = "C:\Program Files\Eclipse Adoptium\jdk-17.0.12.7-hotspot",
    [string]$AndroidHome = "$env:LOCALAPPDATA\Android\Sdk",
    [string]$Serial = ""
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

function Get-OnlineDevices {
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $lines = & $adb devices -l 2>&1
    } finally {
        $ErrorActionPreference = $prev
    }
    $list = @()
    foreach ($line in $lines) {
        $t = ("$line").Trim()
        if ($t -match '^(?<id>\S+)\s+device(\s+(?<rest>.*))?$') {
            $rest = $Matches['rest']
            $model = if ($rest -match 'model:(?<m>\S+)') { $Matches['m'] } else { "" }
            $product = if ($rest -match 'product:(?<p>\S+)') { $Matches['p'] } else { "" }
            $list += [pscustomobject]@{
                Serial  = $Matches['id']
                Model   = $model
                Product = $product
                Detail  = $rest
            }
        }
    }
    return $list
}

$devices = @(Get-OnlineDevices)
if ($devices.Count -eq 0) {
    Write-Host @"
未检测到手机/模拟器（无 state=device 的条目）。

请先任选其一：
  1) 真机：USB 连接 → 开启「开发者选项 / USB 调试」→ 手机上点「允许」
     然后执行: adb kill-server; adb start-server; adb devices
  2) 无线调试（Android 11+）: adb pair <ip:pairPort> 后再 adb connect <ip:port>
  3) 模拟器：Android Studio → Device Manager 启动一个 AVD
  4) 若列表里有 emulator-xxxx host 等脏项：adb kill-server 后重连真机

确认 `adb devices` 出现 device（不是 unauthorized / offline / host）后再重跑本脚本。
"@
    throw "No device/emulator online."
}

if ($Serial) {
    $hit = $devices | Where-Object { $_.Serial -eq $Serial } | Select-Object -First 1
    if (-not $hit) {
        throw "Serial '$Serial' not in online device list."
    }
    $chosen = $hit.Serial
} elseif ($devices.Count -eq 1) {
    $chosen = $devices[0].Serial
    Write-Host "Using device: $($devices[0].Serial) $($devices[0].Model)"
} else {
    Write-Host "Multiple devices online — pick one:"
    for ($i = 0; $i -lt $devices.Count; $i++) {
        $d = $devices[$i]
        $label = (@($d.Serial, $d.Model, $d.Product) | Where-Object { $_ }) -join "  "
        Write-Host ("  [{0}] {1}" -f ($i + 1), $label)
    }
    Write-Host ""
    $pick = Read-Host "Enter number (1-$($devices.Count))"
    $idx = 0
    if (-not [int]::TryParse($pick, [ref]$idx) -or $idx -lt 1 -or $idx -gt $devices.Count) {
        throw "Invalid selection: $pick"
    }
    $chosen = $devices[$idx - 1].Serial
    Write-Host "Using device: $chosen"
}
Write-Host ""

$env:ANDROID_SERIAL = $chosen

Write-Host "--- gradle :app:installDebug ---"
& $gradle ":app:installDebug" --no-daemon
if ($LASTEXITCODE -ne 0) {
    throw "Build/install failed (exit $LASTEXITCODE)."
}

if ($Launch) {
    Write-Host ""
    Write-Host "--- launch app ---"
    & $adb -s $chosen shell am start -n "com.lingxi.app/.MainActivity"
}

if ($Logcat) {
    Write-Host ""
    Write-Host "--- logcat (Ctrl+C to stop) ---"
    $pidOf = (& $adb -s $chosen shell pidof -s com.lingxi.app).Trim()
    if ($pidOf) {
        & $adb -s $chosen logcat --pid=$pidOf
    } else {
        & $adb -s $chosen logcat -s "Lingxi:*" "AndroidRuntime:E" "*:S"
    }
}

Write-Host ""
Write-Host "Done."
