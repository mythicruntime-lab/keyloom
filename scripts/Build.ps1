param([ValidateSet('restore','test','dev','package','bundle','check')][string]$Mode = 'package', [int]$Jobs = 2)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
# A very long inherited PATH exceeds cmd.exe's environment handling. This is
# process-local; no registry, user, or machine environment is modified.
$nodeDirectory = Split-Path (Get-Command node).Source
$gitDirectory = Split-Path (Get-Command git).Source
$env:PATH = "$nodeDirectory;$env:USERPROFILE\.cargo\bin;$env:SystemRoot\system32;$env:SystemRoot;$env:SystemRoot\System32\WindowsPowerShell\v1.0;$gitDirectory"
$env:CARGO_BUILD_JOBS = [string][Math]::Max(1,$Jobs)
if ($Mode -eq 'restore') { npm ci }
elseif ($Mode -eq 'test') { cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=$([Math]::Max(1,$Jobs)) }
elseif ($Mode -eq 'check') { npm run check; if ($LASTEXITCODE -eq 0) { cargo check --manifest-path src-tauri/Cargo.toml } }
elseif ($Mode -eq 'dev') { npm run tauri -- dev }
elseif ($Mode -eq 'bundle') { npm run tauri -- bundle --bundles nsis }
else { npm run tauri -- build }
if ($LASTEXITCODE -ne 0) { throw "Keyloom $Mode failed ($LASTEXITCODE)" }
if ($Mode -in @('package','bundle')) { & (Join-Path $PSScriptRoot 'Package.ps1') -Refresh }
