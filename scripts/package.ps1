<#
.SYNOPSIS
  Builds and packages PILLVYN release artifacts into dist/:
  - PILLVYN-Setup.exe (+ .sha256)
  - PILLVYN_<version>_x64-setup.exe (+ .sha256)
  - PILLVYN-Portable.exe
  - PILLVYN-Windows.zip
  - latest.json (for the in-app SHA-256 self-updater)
#>
param(
  [switch]$SkipBuild,
  [switch]$Offline
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dist = Join-Path $root 'dist'
$cargoToml = Join-Path $root 'src-tauri\Cargo.toml'
$version = (Select-String -Path $cargoToml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
if (-not $version) { throw "Could not read version from $cargoToml" }

if (-not $SkipBuild) {
  Push-Location (Join-Path $root 'src-tauri')
  try {
    $args = @('build', '--release', '--locked')
    if ($Offline) { $args += '--offline' }
    & cargo @args
    if ($LASTEXITCODE -ne 0) { throw "cargo build --release failed ($LASTEXITCODE)" }
  } finally {
    Pop-Location
  }
}

$bin = Join-Path $root 'src-tauri\target\release\pillvyn.exe'
if (-not (Test-Path $bin)) { throw "Release binary not found at $bin" }

New-Item -ItemType Directory -Force $dist | Out-Null
Get-ChildItem -LiteralPath $dist -Force | Remove-Item -Recurse -Force

$nsis = "${env:ProgramFiles(x86)}\NSIS\makensis.exe"
if (-not (Test-Path $nsis)) { throw "NSIS makensis.exe not found at $nsis" }

Push-Location $root
try {
  & $nsis /V2 "/DVERSION=$version" installer/pillvyn.nsi
  if ($LASTEXITCODE -ne 0) { throw "makensis failed ($LASTEXITCODE)" }
} finally {
  Pop-Location
}

$setup = Join-Path $dist 'PILLVYN-Setup.exe'
$versioned = Join-Path $dist "PILLVYN_${version}_x64-setup.exe"
Copy-Item $setup $versioned -Force

$sha = (Get-FileHash $setup -Algorithm SHA256).Hash.ToLowerInvariant()
[System.IO.File]::WriteAllText("$setup.sha256", "$sha  PILLVYN-Setup.exe`n")
[System.IO.File]::WriteAllText("$versioned.sha256", "$sha  PILLVYN_${version}_x64-setup.exe`n")

Copy-Item $bin (Join-Path $dist 'PILLVYN-Portable.exe') -Force

$zipDir = Join-Path $dist 'zip'
New-Item -ItemType Directory -Force $zipDir | Out-Null
Copy-Item $bin (Join-Path $zipDir 'PILLVYN.exe') -Force
Copy-Item (Join-Path $root 'LICENSE'), (Join-Path $root 'README.md'), (Join-Path $root 'CHANGELOG.md') $zipDir -Force
Compress-Archive -Path "$zipDir\*" -DestinationPath (Join-Path $dist 'PILLVYN-Windows.zip') -Force
Remove-Item -Recurse -Force $zipDir

$latestJson = @{
  version   = $version
  sha256    = $sha
  published = (Get-Date).ToUniversalTime().ToString('o')
} | ConvertTo-Json
[System.IO.File]::WriteAllText((Join-Path $dist 'latest.json'), "$latestJson`n")

Write-Host "Packaged PILLVYN v$version into $dist"
Get-ChildItem -LiteralPath $dist | Select-Object Name, Length | Format-Table -AutoSize
