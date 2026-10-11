# SPDX-License-Identifier: GPL-3.0-or-later
param([string]$Executable = "$PSScriptRoot/../target/debug/deckweave-desktop.exe")
$ErrorActionPreference = 'Stop'
$exePath = (Resolve-Path -LiteralPath $Executable).Path
$workspace = (Resolve-Path -LiteralPath "$PSScriptRoot/..").Path
$resultDirectory = Join-Path $workspace 'test-results'
New-Item -ItemType Directory -Force -Path $resultDirectory | Out-Null
$runId = [guid]::NewGuid().ToString('N')
$runRoot = Join-Path $resultDirectory $runId
New-Item -ItemType Directory -Path $runRoot | Out-Null
$report = Join-Path $runRoot 'desktop-smoke.txt'
$env:DECKWEAVE_SMOKE_RESULT = $report
try {
    $process = Start-Process -FilePath $exePath -ArgumentList @('--portable', '--data-root', ('"' + $runRoot + '"')) -PassThru -WindowStyle Hidden -RedirectStandardError (Join-Path $runRoot 'stderr.log')
    if (-not $process.WaitForExit(60000)) {
        $process.Kill()
        throw 'Desktop smoke превысил 60 секунд'
    }
    $process.Refresh()
    if ($process.ExitCode -ne 0) { throw "Desktop smoke exit: $($process.ExitCode); см. $runRoot" }
    if (-not (Test-Path -LiteralPath $report)) { throw 'Smoke report отсутствует: нужна сборка с --features smoke-test' }
    $text = Get-Content -LiteralPath $report -Raw
    if (-not $text.StartsWith('PASS:')) { throw $text }
    # WebView2 принадлежит только этому уникальному test root, чужие процессы не трогаем.
    $cleanupDeadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $orphans = @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" |
            Where-Object { $_.CommandLine -and $_.CommandLine.Contains($runRoot) })
        if ($orphans.Count -eq 0) { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $cleanupDeadline)
    if ($orphans.Count -gt 0) { throw "После Exit остались WebView2 processes для $runRoot" }
    Write-Output $text
    Write-Output 'PASS: host завершён; WebView2 processes тестового root отсутствуют'
} finally {
    Remove-Item Env:DECKWEAVE_SMOKE_RESULT -ErrorAction SilentlyContinue
}
