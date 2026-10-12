param([string]$Exe, [string]$TestRoot, [string]$Midi, [string]$Device)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
if (!$Exe) { $Exe = Join-Path $root 'src-tauri\target\release\keyloom.exe' }
if (!$TestRoot) { $TestRoot = Join-Path $root ('test-output\native-' + (Get-Date -Format 'yyyyMMdd-HHmmss')) }
$Exe = (Resolve-Path -LiteralPath $Exe).Path
if (Test-Path -LiteralPath $TestRoot) { throw 'Choose a fresh test directory; the smoke test requires an empty library.' }
New-Item -ItemType Directory -Path $TestRoot -Force | Out-Null
$TestRoot = (Resolve-Path -LiteralPath $TestRoot).Path
$measurements = @()
foreach ($phase in @('exercise','restart')) {
    $argsList = @('--smoke-test', ('"' + $TestRoot + '"'))
    if ($phase -eq 'restart') { $argsList += '--smoke-restart' }
    if ($Midi) { $argsList += @('--smoke-midi', ('"' + (Resolve-Path -LiteralPath $Midi).Path + '"')) }
    if ($Device) { $argsList += @('--smoke-device', ('"' + $Device + '"')) }
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $app = Start-Process -FilePath $Exe -ArgumentList $argsList -WorkingDirectory $root -PassThru
    $samples = @()
    while (!$app.WaitForExit(500)) {
        if ($watch.Elapsed.TotalSeconds -gt 60) { Stop-Process -Id $app.Id -ErrorAction SilentlyContinue; throw "Native $phase timed out. Inspect $TestRoot." }
        $ids = @($app.Id)
        $next = @($app.Id)
        while ($next.Count -gt 0) {
            $children = @()
            foreach ($parentId in $next) { $children += @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $parentId" | Select-Object -ExpandProperty ProcessId) }
            $next = @($children | Where-Object { $_ -notin $ids })
            $ids += $next
        }
        $processes = @(Get-Process -Id $ids -ErrorAction SilentlyContinue)
        $samples += [pscustomobject]@{ elapsed_ms=$watch.Elapsed.TotalMilliseconds; processes=$processes.Count; cpu_seconds=($processes | Measure-Object CPU -Sum).Sum; working_set_bytes=($processes | Measure-Object WorkingSet64 -Sum).Sum }
    }
    $reportPath = Join-Path $TestRoot "smoke-$phase.json"
    if (!(Test-Path -LiteralPath $reportPath)) { throw "No native report. Close other Keyloom instances before running tests. Exit: $($app.ExitCode)" }
    $report = Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
    $measurements += [pscustomobject]@{phase=$phase;process_elapsed_ms=$watch.Elapsed.TotalMilliseconds;exit_code=$app.ExitCode;samples=$samples}
    $measurements | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $TestRoot 'process-metrics.json')
    if (!$report.passed) { throw "Native $phase failed: $($report.error). Report: $reportPath" }
    "Native $phase PASS: $($report.checks.Count) checks in $([Math]::Round($report.elapsed_ms,1)) ms."
}
"Reports: $TestRoot"
