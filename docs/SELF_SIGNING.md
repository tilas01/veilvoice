# The self-signed code certificate

VeilVoice publishes a **self-signed code-signing certificate**, alongside the
OpenPGP key that signs `SHA256SUMS`. This page is what it is for, what it is
not, and how to use it.

## What it is not

Say this first, because it is the part that is easy to oversell.

- It is **not a certificate authority** vouching for anyone. Nobody checked
  anyone's legal identity to issue it; the project signed its own.
- It does **not** make Windows SmartScreen trust VeilVoice on sight. A
  self-signed certificate is unknown to SmartScreen until you choose to trust
  it, exactly as it should be.
- It does **not replace** the OpenPGP signature over `SHA256SUMS`. That remains
  the primary check. If you only do one thing, do that one.

## What it is for

It is a second identity you can choose to trust once, the same way you already
trust the OpenPGP key: **compare its fingerprint by hand, decide to trust it,
and then let the tools check against it.** After that, files carrying VeilVoice's
signed app manifest can be verified against a certificate *you* decided to
accept.

That is worth having in two places:

- **On a machine or in an organisation that imports it to trusted publishers.**
  Once imported, this publisher is known, and Windows and some antivirus treat a
  known publisher differently from an unknown one. That can reduce the
  low-reputation false positives a brand-new application otherwise runs into.
- **As an independent second check.** The OpenPGP key and the code certificate
  are different keys, verified by different tools. Both agreeing is a stronger
  statement than either alone.

## How it is signed, and why that keeps builds reproducible

The certificate signs a small **detached** manifest, `APPMANIFEST.json`, which
lists each binary with its size and SHA-256. It does **not** sign the binaries
in place. Signing a binary changes it, and two people building the same source
would then get different files for a reason that has nothing to do with the
source. Keeping the signature detached is the same choice that puts the OpenPGP
signature over the hash list rather than inside a binary, and it is why
reproducible builds still work.

## Verifying a download against it

You need three files from the release: `APPMANIFEST.json`, its signature
(`.sig` on Unix, `.p7s` on Windows), and the certificate
`veilvoice-code-cert.pem` / `.cer`.

Unix or WSL:

```sh
VV_CERT_FPR="<fingerprint from the website>" \
  tools/sign/verify.sh path/to/unpacked/release
```

Windows PowerShell:

```powershell
powershell -File tools\sign\verify.ps1 path\to\unpacked\release `
  -ExpectedThumbprint "<thumbprint from the website>"
```

Both check the manifest's signature against the certificate, the certificate's
fingerprint against the one you pass in, and every binary against the manifest.

## Importing it as trusted (optional)

Only do this after checking the fingerprint against the copy on the website. It
is your decision, and it is only worth making if you understand it: you are
telling your system that files signed by this certificate come from a publisher
you accept.

**Windows, trusted publisher (per user):**

```powershell
Import-Certificate -FilePath veilvoice-code-cert.cer `
  -CertStoreLocation Cert:\CurrentUser\TrustedPublisher
```

To undo it, open `certmgr.msc`, find "tilas01 / VeilVoice" under Trusted
Publishers, and delete it.

**Linux / macOS:** there is no system-wide "trusted code publisher" store in the
Windows sense; verification is done with the `verify.sh` script above and the
published certificate. Keep the certificate somewhere you control and pass it
with `VV_CERT`.

## For the maintainer: creating and using it

```sh
tools/sign/manifest.py  dist/veilvoice-vX.Y.Z-linux-x86_64   # write APPMANIFEST.json
tools/sign/selfsign.sh  --new-cert                            # once, ever
tools/sign/selfsign.sh  sign dist/veilvoice-vX.Y.Z-linux-x86_64
```

The private key (`veilvoice-code-key.pem`, or the Windows certificate store
entry) never leaves the maintainer's machine and is never committed. Only the
public certificate and its fingerprint are published, on the website next to the
OpenPGP fingerprint.
