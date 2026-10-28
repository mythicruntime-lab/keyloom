param([switch]$Refresh)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$version = (Get-Content -LiteralPath (Join-Path $root 'package.json') -Raw | ConvertFrom-Json).version
$name = "Keyloom-$version-windows-x64"
$artifacts = Join-Path $root 'artifacts'
$destination = Join-Path $artifacts $name
$exe = Join-Path $root 'src-tauri\target\release\keyloom.exe'
$installer = Join-Path $root "src-tauri\target\release\bundle\nsis\Keyloom_${version}_x64-setup.exe"
foreach ($required in @($exe, $installer)) {
    if (!(Test-Path -LiteralPath $required -PathType Leaf)) { throw "Build first; missing $required" }
}
if (Test-Path -LiteralPath $destination) {
    if (!$Refresh) { throw 'Package directory already exists. Use -Refresh to replace only this generated package.' }
    $resolved = (Resolve-Path -LiteralPath $destination).Path
    $expected = [IO.Path]::GetFullPath($destination)
    if ($resolved -ne $expected -or !(Split-Path $resolved -Parent).Equals([IO.Path]::GetFullPath($artifacts), [StringComparison]::OrdinalIgnoreCase)) { throw 'Package path validation failed.' }
    if ((Get-Item -LiteralPath $resolved).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Refusing to replace a linked package directory.' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Copy-Item -LiteralPath $exe -Destination (Join-Path $destination 'Keyloom.exe')
foreach ($file in @('LICENSE','THIRD_PARTY_NOTICES.md','README.md','ROADMAP.md','CHANGELOG.md')) {
    Copy-Item -LiteralPath (Join-Path $root $file) -Destination $destination
}
Copy-Item -LiteralPath (Join-Path $root 'licenses') -Destination $destination -Recurse
$docs = Join-Path $destination 'docs'
New-Item -ItemType Directory -Path $docs | Out-Null
foreach ($file in @('architecture.md','dependencies.md','hardware-checklist.md','midi-policy.md','test-report.md','timing.md')) {
    Copy-Item -LiteralPath (Join-Path $root "docs\$file") -Destination $docs
}
Copy-Item -LiteralPath (Join-Path $root 'docs\screenshots') -Destination $docs -Recurse
Copy-Item -LiteralPath $installer -Destination (Join-Path $artifacts (Split-Path $installer -Leaf)) -Force
$zip = Join-Path $artifacts "$name.zip"
# Some upstream notices have epoch timestamps outside ZIP's 1980–2107 range.
# Normalize only staged metadata; preserve every third-party file's bytes.
Get-ChildItem -LiteralPath $destination -File -Recurse | Where-Object {
    $_.LastWriteTime.Year -lt 1980 -or $_.LastWriteTime.Year -gt 2107
} | ForEach-Object { $_.LastWriteTime = [datetime]'2000-01-01T12:00:00' }
Compress-Archive -LiteralPath $destination -DestinationPath $zip -Force
$hashes = Get-FileHash -LiteralPath @($zip, (Join-Path $artifacts (Split-Path $installer -Leaf)), (Join-Path $destination 'Keyloom.exe')) -Algorithm SHA256
$hashes | ForEach-Object { "$($_.Hash.ToLowerInvariant())  $($_.Path.Substring($artifacts.Length + 1))" } | Set-Content -LiteralPath (Join-Path $artifacts 'SHA256SUMS.txt')
"Portable app: $destination"
"ZIP: $zip"
"Installer: $(Join-Path $artifacts (Split-Path $installer -Leaf))"
