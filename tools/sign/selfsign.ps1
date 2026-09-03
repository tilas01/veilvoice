# Generate VeilVoice's self-signed code certificate on Windows, and sign the app
# manifest with it. For the maintainer, at release time.
#
#   powershell -File tools/sign/selfsign.ps1 -NewCert       # once
#   powershell -File tools/sign/selfsign.ps1 -Sign DIR      # sign DIR\APPMANIFEST.json
#
# Same design as tools/sign/selfsign.sh: a self-signed certificate is trust on
# first use, not a certificate authority, and it does not replace the OpenPGP
# signature over SHA256SUMS. The manifest is detached, so signing it does not
# touch the binaries or their reproducibility. The private key stays in the
# user's certificate store and is never exported to the repository.
#
# On Windows the certificate is created with codeSigning usage so it can also be
# used with signtool to Authenticode-sign the .exe files for a local or
# enterprise build -- but note that signing a binary in place changes it and so
# breaks reproducible builds, which is why the detached manifest is the default
# and the in-place path is opt-in and off the reproducible route.
#
# SPDX-License-Identifier: GPL-3.0-or-later

param(
    [switch]$NewCert,
    [string]$Sign,
    [switch]$Fingerprint
)
$ErrorActionPreference = "Stop"

function New-VVCert {
    $cert = New-SelfSignedCertificate `
        -Type CodeSigningCert `
        -Subject "CN=tilas01, O=VeilVoice, OU=Code Signing" `
        -KeyAlgorithm ECDSA_nistP256 `
        -CertStoreLocation "Cert:\CurrentUser\My" `
        -NotAfter (Get-Date).AddYears(10)
    # Export only the public certificate. The private key stays in the store.
    Export-Certificate -Cert $cert -FilePath "veilvoice-code-cert.cer" | Out-Null
    Write-Host "Created certificate in Cert:\CurrentUser\My and wrote veilvoice-code-cert.cer (public, publish this)."
    Write-Host "Thumbprint (publish beside the OpenPGP fingerprint):"
    Write-Host "  $($cert.Thumbprint)"
    Write-Host "The private key is in your certificate store and was not exported."
}

function Get-VVCert {
    $cert = Get-ChildItem Cert:\CurrentUser\My |
        Where-Object { $_.Subject -like "*CN=tilas01*VeilVoice*" -or $_.Subject -like "*O=VeilVoice*" } |
        Sort-Object NotAfter -Descending | Select-Object -First 1
    if (-not $cert) { throw "no VeilVoice code certificate found; run -NewCert first" }
    return $cert
}

function Sign-Manifest {
    param($Dir)
    $manifest = Join-Path $Dir "APPMANIFEST.json"
    if (-not (Test-Path $manifest)) { throw "no APPMANIFEST.json in $Dir; run tools/sign/manifest.py first" }
    $cert = Get-VVCert
    # A detached CMS/PKCS7 signature over the manifest bytes.
    $bytes = [System.IO.File]::ReadAllBytes($manifest)
    $content = New-Object System.Security.Cryptography.Pkcs.ContentInfo(,$bytes)
    $signed = New-Object System.Security.Cryptography.Pkcs.SignedCms($content, $true)
    $signer = New-Object System.Security.Cryptography.Pkcs.CmsSigner($cert)
    $signed.ComputeSignature($signer)
    [System.IO.File]::WriteAllBytes("$manifest.p7s", $signed.Encode())
    Write-Host "wrote $manifest.p7s"
}

if ($NewCert)      { New-VVCert }
elseif ($Fingerprint) { Write-Host (Get-VVCert).Thumbprint }
elseif ($Sign)     { Sign-Manifest $Sign }
else { Write-Host "usage: selfsign.ps1 -NewCert | -Fingerprint | -Sign DIR" }
