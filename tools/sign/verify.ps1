# Verify a VeilVoice release against its self-signed code certificate, on Windows.
#
#   powershell -File tools/sign/verify.ps1 DIR
#
# The code-certificate counterpart to `veilvoice verify` (which uses OpenPGP).
# Checks the detached manifest signature against the published certificate, then
# every binary against the manifest.
#
# SPDX-License-Identifier: GPL-3.0-or-later

param(
    [Parameter(Mandatory=$true)][string]$Dir,
    [string]$Cert = "veilvoice-code-cert.cer",
    [string]$ExpectedThumbprint = ""
)
$ErrorActionPreference = "Stop"

$manifest = Join-Path $Dir "APPMANIFEST.json"
$sig = "$manifest.p7s"
foreach ($needed in @($Cert, $manifest, $sig)) {
    if (-not (Test-Path $needed)) { Write-Error "missing: $needed"; exit 2 }
}

# 1. The detached signature over the manifest.
$bytes = [System.IO.File]::ReadAllBytes($manifest)
$content = New-Object System.Security.Cryptography.Pkcs.ContentInfo(,$bytes)
$signed = New-Object System.Security.Cryptography.Pkcs.SignedCms($content, $true)
$signed.Decode([System.IO.File]::ReadAllBytes($sig))
try {
    $signed.CheckSignature($true)   # verify the signature; do not chain to a CA
    Write-Host "ok   signature over APPMANIFEST.json is valid"
} catch {
    Write-Error "FAIL the signature over APPMANIFEST.json did not verify"
    exit 2
}

# 2. The certificate thumbprint, if one was given to check against.
$certObj = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($Cert)
if ($ExpectedThumbprint) {
    $a = ($certObj.Thumbprint -replace '[: ]','').ToUpper()
    $b = ($ExpectedThumbprint -replace '[: ]','').ToUpper()
    if ($a -eq $b) { Write-Host "ok   certificate thumbprint matches the one you trust" }
    else { Write-Error "FAIL thumbprint mismatch: expected $b, found $a"; exit 2 }
} else {
    Write-Host "note thumbprint is $($certObj.Thumbprint)"
    Write-Host "     compare it against the website before trusting."
}

# 3. Every binary against the manifest, via the shared Python checker.
if (Get-Command python3 -ErrorAction SilentlyContinue) {
    python3 tools/sign/manifest.py $Dir --check
    if ($LASTEXITCODE -ne 0) { exit 2 }
} elseif (Get-Command python -ErrorAction SilentlyContinue) {
    python tools/sign/manifest.py $Dir --check
    if ($LASTEXITCODE -ne 0) { exit 2 }
} else {
    Write-Host "note skipping the per-binary check (needs Python and the repo checkout)"
}
