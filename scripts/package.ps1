<#
.SYNOPSIS
  Builds a release zip: dist\tca-neo-bridge-<version>-x64.zip.
.DESCRIPTION
  Requires the Rust toolchain, Visual Studio Build Tools (C++) and the Windows Driver Kit (see docs/BUILDING.md).
  The driver package is shipped unsigned; install-driver.ps1 signs it on the user's PC. The zip content is
  checked against an explicit allowlist so no build byproducts or private files can slip in.
#>
param([string]$Version)
$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
if (-not $Version) {
    $Version = (Select-String -Path (Join-Path $root 'daemon\Cargo.toml') -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
}

# Keep build-machine paths (user name, checkout folder) out of panic messages in the shipped binary.
$registry = Join-Path $env:USERPROFILE '.cargo\registry\src'
$env:RUSTFLAGS = "--remap-path-prefix=$registry=crates --remap-path-prefix=$root=tca-neo-bridge --remap-path-prefix=$env:USERPROFILE=~"
try {
    cargo build --release --quiet --manifest-path (Join-Path $root 'Cargo.toml') -p neo-daemon -p neo-config; if ($LASTEXITCODE) { throw 'build failed' }
} finally {
    Remove-Item Env:\RUSTFLAGS
}
& (Join-Path $PSScriptRoot 'third-party-licenses.ps1')
& (Join-Path $root 'driver\build.ps1') -DriverVersion "$Version.0"

$name = "tca-neo-bridge-$Version-x64"
$stage = Join-Path $root "dist\$name"
Remove-Item -Recurse -Force $stage, "$stage.zip" -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force (Join-Path $stage 'driver') | Out-Null
foreach ($exe in 'neo-daemon.exe', 'neo-config.exe') { Copy-Item (Join-Path $root "target\release\$exe") $stage }

# Smoke launch: the shipped app opens its window, finds nothing on the port and closes itself with code 0. A
# release-only failure (a feature only test builds turn on) stops the build here instead of on a tester's PC.
$handshake = Join-Path ([IO.Path]::GetTempPath()) 'neo-config-smoke.txt'
$smokeErr = Join-Path ([IO.Path]::GetTempPath()) 'neo-config-smoke.err'
Set-Content $handshake ('1 ' + ('0' * 48)) -Encoding ASCII
$smoke = Start-Process (Join-Path $stage 'neo-config.exe') -RedirectStandardInput $handshake -RedirectStandardError $smokeErr -PassThru
if (-not $smoke.WaitForExit(60000)) { $smoke.Kill(); throw 'neo-config.exe smoke launch did not close by itself' }
if ($smoke.ExitCode -ne 0) { throw "neo-config.exe smoke launch failed (0x{0:X}): {1}" -f $smoke.ExitCode, (Get-Content $smokeErr -Raw) }
Remove-Item $handshake, $smokeErr
foreach ($file in 'README.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md', 'THIRD_PARTY_LICENSES.md', 'SECURITY.md') { Copy-Item (Join-Path $root $file) $stage }
$package = Join-Path $root 'driver\out\package'
foreach ($file in 'neohid.dll', 'neohid.inf', 'neohid.cat') { Copy-Item (Join-Path $package $file) (Join-Path $stage 'driver') }
Copy-Item (Join-Path $root 'driver\LICENSE-MS-PL.txt') (Join-Path $stage 'driver')
foreach ($file in 'install-driver.ps1', 'uninstall-driver.ps1') { Copy-Item (Join-Path $PSScriptRoot $file) (Join-Path $stage 'driver') }
Get-FileHash (Join-Path $stage 'driver\neohid.*') -Algorithm SHA256 |
    ForEach-Object { '{0}  {1}' -f $_.Hash.ToLowerInvariant(), (Split-Path $_.Path -Leaf) } |
    Set-Content (Join-Path $stage 'driver\SHA256SUMS') -Encoding ASCII

$expected = @('LICENSE', 'README.md', 'SECURITY.md', 'THIRD_PARTY_LICENSES.md', 'THIRD_PARTY_NOTICES.md', 'neo-config.exe', 'neo-daemon.exe',
    'driver\LICENSE-MS-PL.txt', 'driver\SHA256SUMS', 'driver\install-driver.ps1', 'driver\neohid.cat',
    'driver\neohid.dll', 'driver\neohid.inf', 'driver\uninstall-driver.ps1') | Sort-Object
$actual = Get-ChildItem $stage -Recurse -File | ForEach-Object { $_.FullName.Substring($stage.Length + 1) } | Sort-Object
if (Compare-Object $expected $actual) { throw "Unexpected package content:`n$(Compare-Object $expected $actual | Out-String)" }

Compress-Archive -Path "$stage\*" -DestinationPath "$stage.zip"
$hash = (Get-FileHash "$stage.zip" -Algorithm SHA256).Hash.ToLowerInvariant()
"$hash  $name.zip" | Set-Content (Join-Path $root "dist\$name.zip.sha256") -Encoding ASCII
"Built dist\$name.zip ($hash)"
