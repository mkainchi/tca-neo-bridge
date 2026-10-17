<#
.SYNOPSIS
  Installs (or reinstalls) the TCA-NEO bridge virtual NEO driver. Requires administrator rights.

.DESCRIPTION
  Run by the tray menu ("Install driver…" / "Reinstall driver…") through a UAC prompt, or by hand from an
  elevated PowerShell. Steps:
    1. Verify the shipped package (neohid.dll, neohid.inf, neohid.cat) against SHA256SUMS.
    2. Remove any earlier neohid package and earlier certificates of this project.
    3. Copy the package to %ProgramData%\tca-neo-bridge\driver, create a single-use code-signing certificate,
       sign the copies with it, export only its public part and delete its private key.
    4. Trust that public certificate (LocalMachine Root + TrustedPublisher), check the signatures, and stage
       the package with pnputil. The virtual device itself is created only while the daemon emulates.
  Because the private key is deleted right after signing, the trusted certificate can never sign anything else.
#>
param([switch]$Reinstall, [string]$Log)
$ErrorActionPreference = 'Stop'
# pnputil writes in the console's OEM code page; decode it as such so localized output logs correctly.
[Console]::OutputEncoding = [Text.Encoding]::GetEncoding([Globalization.CultureInfo]::CurrentCulture.TextInfo.OEMCodePage)
$Subject = 'CN=tca-neo-bridge driver (single use)'
$Files = 'neohid.dll', 'neohid.inf', 'neohid.cat'

function Say([string]$Message) {
    $line = '{0:yyyy-MM-dd HH:mm:ss} {1}' -f (Get-Date), $Message
    Write-Host $line
    if ($Log) { Add-Content -Path $Log -Value $line -Encoding UTF8 }
}

function Remove-Trust {
    foreach ($store in 'Root', 'TrustedPublisher') {
        Get-ChildItem "Cert:\LocalMachine\$store" | Where-Object Subject -eq $Subject | ForEach-Object {
            Remove-Item $_.PSPath
            Say "Removed earlier certificate $($_.Thumbprint) from LocalMachine\$store."
        }
    }
}

function Remove-Packages {
    $blocks = ((pnputil /enum-drivers) -join "`n") -split "`n\s*`n"
    foreach ($block in $blocks | Where-Object { $_ -match '(?i)neohid\.inf' }) {
        if ($block -match '(oem\d+\.inf)') {
            pnputil /delete-driver $matches[1] /uninstall /force | Out-Null
            Say "Removed earlier driver package $($matches[1]) (exit $LASTEXITCODE)."
        }
    }
}

try {
    if ($Log) { New-Item -ItemType Directory -Force (Split-Path $Log) | Out-Null }
    Say ("Starting driver {0}." -f $(if ($Reinstall) { 'reinstallation' } else { 'installation' }))
    $principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Administrator rights are required.' }

    # 1. Integrity of the shipped files.
    $sums = @{}
    foreach ($line in Get-Content (Join-Path $PSScriptRoot 'SHA256SUMS')) {
        if ($line -match '^([0-9a-f]{64})\s+(\S+)$') { $sums[$matches[2]] = $matches[1] }
    }
    foreach ($file in $Files) {
        $actual = (Get-FileHash (Join-Path $PSScriptRoot $file) -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($sums[$file] -ne $actual) { throw "$file does not match SHA256SUMS; download the release again." }
    }
    Say 'Package files match SHA256SUMS.'

    # 2. Start clean: earlier packages and certificates of this project.
    Remove-Packages
    Remove-Trust

    # 3. Sign copies with a single-use certificate, then destroy its private key.
    $work = Join-Path $env:ProgramData 'tca-neo-bridge\driver'
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $work | Out-Null
    foreach ($file in $Files) { Copy-Item (Join-Path $PSScriptRoot $file) $work }
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject $Subject -CertStoreLocation Cert:\CurrentUser\My `
        -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 -KeyExportPolicy NonExportable -NotAfter (Get-Date).AddYears(10)
    try {
        foreach ($file in 'neohid.dll', 'neohid.cat') {
            $result = Set-AuthenticodeSignature -FilePath (Join-Path $work $file) -Certificate $cert -HashAlgorithm SHA256
            if (-not $result.SignerCertificate) { throw "Signing $file failed: $($result.StatusMessage)" }
        }
        Export-Certificate -Cert $cert -FilePath (Join-Path $work 'neohid-signing.cer') -Type CERT | Out-Null
    } finally {
        Remove-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)" -DeleteKey
    }
    if (Test-Path "Cert:\CurrentUser\My\$($cert.Thumbprint)") { throw 'The signing key could not be deleted.' }
    Say "Signed with single-use certificate $($cert.Thumbprint); its private key is deleted."

    # 4. Trust, verify, stage.
    try {
        foreach ($store in 'Root', 'TrustedPublisher') {
            Import-Certificate -FilePath (Join-Path $work 'neohid-signing.cer') -CertStoreLocation "Cert:\LocalMachine\$store" | Out-Null
        }
        foreach ($file in 'neohid.dll', 'neohid.cat') {
            $status = (Get-AuthenticodeSignature (Join-Path $work $file)).Status
            if ($status -ne 'Valid') { throw "$file signature is $status after trusting the certificate." }
        }
        $output = pnputil /add-driver (Join-Path $work 'neohid.inf')
        $output | ForEach-Object { Say "pnputil: $_" }
        if ($LASTEXITCODE) { throw "pnputil could not stage the driver (exit $LASTEXITCODE)." }
    } catch {
        Remove-Trust
        throw
    }
    Say 'Driver installed. Start emulation from the TCA-NEO bridge tray menu.'
    exit 0
} catch {
    Say "FAILED: $($_.Exception.Message)"
    if (-not $Log) { Read-Host 'Press Enter to close' | Out-Null }
    exit 1
}
