<#
.SYNOPSIS
  Removes the TCA-NEO bridge driver package and untrusts its certificates. Requires administrator rights.
.DESCRIPTION
  Quit the daemon first. Your profile in %LOCALAPPDATA%\tca-neo-bridge is not touched.
#>
$ErrorActionPreference = 'Stop'
$Subject = 'CN=tca-neo-bridge driver (single use)'
$principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Run this script from an elevated (administrator) PowerShell.' }
$removed = 0
$blocks = ((pnputil /enum-drivers) -join "`n") -split "`n\s*`n"
foreach ($block in $blocks | Where-Object { $_ -match '(?i)neohid\.inf' }) {
    if ($block -match '(oem\d+\.inf)') {
        pnputil /delete-driver $matches[1] /uninstall /force | Out-Null
        $removed++
    }
}
$untrusted = 0
foreach ($store in 'Root', 'TrustedPublisher') {
    Get-ChildItem "Cert:\LocalMachine\$store" | Where-Object Subject -eq $Subject | ForEach-Object { Remove-Item $_.PSPath; $untrusted++ }
}
Remove-Item -Recurse -Force (Join-Path $env:ProgramData 'tca-neo-bridge\driver') -ErrorAction SilentlyContinue
"Removed $removed driver package(s) and $untrusted certificate entr$(if ($untrusted -eq 1) { 'y' } else { 'ies' })."
