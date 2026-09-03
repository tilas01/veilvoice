#!/bin/sh
# Verify a VeilVoice release against its self-signed code certificate.
#
#   tools/sign/verify.sh DIR
#
# Checks, in order:
#   1. the manifest's signature, against the published certificate
#   2. the certificate's fingerprint, against the one you trust
#   3. every binary, against the now-trusted manifest
#
# This is the code-certificate counterpart to `veilvoice verify`, which uses the
# OpenPGP key. Either one on its own establishes the download; both together is
# two independent identities agreeing. The OpenPGP one is primary.
#
# SPDX-License-Identifier: GPL-3.0-or-later

set -eu

DIR="${1:-.}"
CERT="${VV_CERT:-veilvoice-code-cert.pem}"
MANIFEST="$DIR/APPMANIFEST.json"
SIG="$MANIFEST.sig"

# Published fingerprint of the code certificate. Compare what you import against
# this and against the copy on the website before trusting it.
EXPECTED_FPR="${VV_CERT_FPR:-}"

have() { command -v "$1" >/dev/null 2>&1; }
have openssl || { echo "openssl is required" >&2; exit 1; }
[ -f "$CERT" ]     || { echo "no certificate at $CERT (set VV_CERT)" >&2; exit 1; }
[ -f "$MANIFEST" ] || { echo "no APPMANIFEST.json in $DIR" >&2; exit 1; }
[ -f "$SIG" ]      || { echo "no APPMANIFEST.json.sig in $DIR" >&2; exit 1; }

# 1. The signature over the manifest.
if openssl cms -verify -binary -in "$SIG" -inform PEM -content "$MANIFEST" \
        -certfile "$CERT" -CAfile "$CERT" -purpose any -out /dev/null 2>/dev/null; then
    echo "ok   signature over APPMANIFEST.json is valid"
else
    echo "FAIL the signature over APPMANIFEST.json did not verify" >&2
    exit 2
fi

# 2. The certificate fingerprint, if one to check against was given.
GOT_FPR=$(openssl x509 -in "$CERT" -noout -fingerprint -sha256 | sed 's/.*=//')
if [ -n "$EXPECTED_FPR" ]; then
    norm() { printf '%s' "$1" | tr -d ': ' | tr 'a-f' 'A-F'; }
    if [ "$(norm "$GOT_FPR")" = "$(norm "$EXPECTED_FPR")" ]; then
        echo "ok   certificate fingerprint matches the one you trust"
    else
        echo "FAIL certificate fingerprint does not match" >&2
        echo "     expected $EXPECTED_FPR" >&2
        echo "     found    $GOT_FPR" >&2
        exit 2
    fi
else
    echo "note fingerprint is $GOT_FPR"
    echo "     set VV_CERT_FPR, or compare it against the website, before trusting."
fi

# 3. Every binary against the manifest, reusing the Python checker so there is
#    one implementation of "does this file match the manifest".
if have python3 && [ -f "tools/sign/manifest.py" ]; then
    python3 tools/sign/manifest.py "$DIR" --check
else
    echo "note skipping the per-binary check (needs python3 and the repo checkout)"
fi
