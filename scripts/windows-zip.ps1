# windows-zip.ps1: the release build, its icon inside, in a zip.
Set-Location (Join-Path $PSScriptRoot '..') -ErrorAction Stop
cargo build --release --locked
if ($LASTEXITCODE -ne 0) { throw "windows-zip: cargo build failed" }
$version = ((cargo pkgid) -split '[#@]')[-1]
if ($version -notmatch '^[0-9]') { throw "windows-zip: cargo pkgid names no version" }
$arch = @{ AMD64 = 'x86_64'; ARM64 = 'aarch64' }[$env:PROCESSOR_ARCHITECTURE]
if (-not $arch) { throw "windows-zip: no name for $env:PROCESSOR_ARCHITECTURE" }
$target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }
$out = Join-Path $target 'release'
$zip = Join-Path $out "ely-tea-party-$version-windows-$arch.zip"
Compress-Archive -Force -Path (Join-Path $out 'ely-tea-party.exe') -DestinationPath $zip -ErrorAction Stop
Write-Output "windows-zip: $zip"
