param([ValidateSet('test','dev','package','check')][string]$Mode = 'package', [int]$Jobs = 2)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
# A very long inherited PATH exceeds cmd.exe's environment handling. This is
# process-local; no registry, user, or machine environment is modified.
$nodeDirectory = Split-Path (Get-Command node).Source
$gitDirectory = Split-Path (Get-Command git).Source
$env:PATH = "$nodeDirectory;$env:USERPROFILE\.cargo\bin;$env:SystemRoot\system32;$env:SystemRoot;$gitDirectory"
$env:CARGO_BUILD_JOBS = [string][Math]::Max(1,$Jobs)
if ($Mode -eq 'test') { cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=2 }
elseif ($Mode -eq 'check') { npm run check; if ($LASTEXITCODE -eq 0) { cargo check --manifest-path src-tauri/Cargo.toml } }
elseif ($Mode -eq 'dev') { npm run tauri -- dev }
else { npm run tauri -- build }
if ($LASTEXITCODE -ne 0) { throw "Keyloom $Mode failed ($LASTEXITCODE)" }
