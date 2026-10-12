$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$exe = Join-Path $root 'src-tauri\target\release\keyloom.exe'
if (!(Test-Path -LiteralPath $exe)) { throw 'Build first: powershell -File scripts/Build.ps1 -Mode package' }
Start-Process -FilePath $exe -WorkingDirectory $root
