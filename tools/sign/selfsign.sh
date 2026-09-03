#!/bin/sh
# Generate VeilVoice's self-signed code certificate, and sign an app manifest
# with it. For the maintainer, at release time; a user never runs this.
#
#   tools/sign/selfsign.sh --new-cert          # once: make the key and cert
#   tools/sign/selfsign.sh sign DIR            # sign DIR/APPMANIFEST.json
#
# What this is, and is honest about not being
# --------------------------------------------
# A self-signed certificate is not a certificate authority vouching for anyone.
# Windows SmartScreen will not trust it on sight, and it does not replace the
# OpenPGP signature over SHA256SUMS, which stays the primary check. What it adds
# is an identity a user or an organisation can choose to import once, after
# checking its fingerprint by hand -- the same trust-on-first-use shape as the
# OpenPGP key -- so that this publisher becomes known on their machines.
#
# The manifest is detached: it describes the binaries, it is not embedded in
# them, so signing it changes nothing about the binaries and does not touch
# reproducibility. That is deliberate and matches how the OpenPGP signature is
# over the hash list rather than over any binary in place.
#
# The private key never leaves the maintainer's machine and is never committed.
# Only the public certificate (veilvoice-code-cert.pem) and its fingerprint are
# published.
#
# SPDX-License-Identifier: GPL-3.0-or-later

set -eu

KEY="veilvoice-code-key.pem"
CERT="veilvoice-code-cert.pem"
SUBJECT="/CN=tilas01/O=VeilVoice/OU=Code Signing"
DAYS=3650

have() { command -v "$1" >/dev/null 2>&1; }
have openssl || { echo "openssl is required" >&2; exit 1; }

new_cert() {
    if [ -f "$KEY" ]; then
        echo "refusing to overwrite an existing $KEY" >&2
        echo "delete it yourself if you really mean to make a new identity." >&2
        exit 1
    fi
    # A code-signing certificate: an EC key (P-256), self-signed, with the
    # codeSigning extended key usage so a verifier can tell what it is for.
    openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 \
        -keyout "$KEY" -out "$CERT" -days "$DAYS" -nodes \
        -subj "$SUBJECT" \
        -addext "keyUsage=critical,digitalSignature" \
        -addext "extendedKeyUsage=codeSigning"
    chmod 600 "$KEY"
    echo
    echo "Wrote $KEY (keep this secret, never commit it) and $CERT (publish this)."
    echo "Its fingerprint, to publish beside the OpenPGP one:"
    fingerprint
}

fingerprint() {
    openssl x509 -in "$CERT" -noout -fingerprint -sha256 \
        | sed 's/.*=//'
}

sign() {
    dir="$1"
    manifest="$dir/APPMANIFEST.json"
    [ -f "$manifest" ] || { echo "no APPMANIFEST.json in $dir; run tools/sign/manifest.py first" >&2; exit 1; }
    [ -f "$KEY" ] || { echo "no $KEY; run --new-cert first" >&2; exit 1; }
    # A detached CMS signature over the manifest, carrying the certificate so a
    # verifier needs only the manifest and the signature.
    openssl cms -sign -binary -in "$manifest" -signer "$CERT" -inkey "$KEY" \
        -outform PEM -out "$manifest.sig" -nodetach
    echo "wrote $manifest.sig"
    echo "verify with: tools/sign/verify.sh $dir"
}

case "${1:-}" in
    --new-cert) new_cert ;;
    --fingerprint) fingerprint ;;
    sign) [ $# -ge 2 ] || { echo "usage: $0 sign DIR" >&2; exit 1; }; sign "$2" ;;
    *) echo "usage: $0 {--new-cert | --fingerprint | sign DIR}" >&2; exit 1 ;;
esac
