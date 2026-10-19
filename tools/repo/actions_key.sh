# SPDX-License-Identifier: GPL-3.0-or-later
#
# Sourced by the scripts the signing and rewrite workflows run. Imports the
# key in $KEY (armoured, or base64 of either form) into a keyring of the run's
# own, makes "$RUNNER_TEMP/gpg-loopback" a gpg that reads $PASSPHRASE from a
# file rather than an argument, prints the key's fingerprints and identities
# (public, never the secret part), and sets $primary, $signer (the signing
# subkey when there is one) and $address, tilas01's noreply address.

say() { printf '%s\n' "$*" | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"; }

export GNUPGHOME="$RUNNER_TEMP/gnupg"
mkdir -m 700 -p "$GNUPGHOME"
material="$RUNNER_TEMP/key"
if printf '%s' "$KEY" | grep -q -- '-----BEGIN PGP'; then
  printf '%s\n' "$KEY" > "$material"
else
  printf '%s' "$KEY" | base64 -d > "$material"
fi
gpg --batch --quiet --import "$material" 2>/dev/null || true
rm -f "$material"

pp="$RUNNER_TEMP/pp"
printf '%s' "${PASSPHRASE:-}" > "$pp"
cat > "$RUNNER_TEMP/gpg-loopback" <<EOF
#!/bin/sh
exec gpg --batch --pinentry-mode loopback --passphrase-file "$pp" "\$@"
EOF
chmod +x "$RUNNER_TEMP/gpg-loopback"

listing=$(gpg --list-secret-keys --with-colons)
if [ -z "$listing" ]; then
  say "No secret key could be imported from the secret."
  exit 1
fi
say "## The key in the secret"
say '```'
printf '%s\n' "$listing" | awk -F: '
  $1=="sec"||$1=="ssb" {kind=$1; caps=$12; stub=($15=="#") ? " (secret part absent)" : ""}
  $1=="fpr" {print kind, $10, "capabilities " caps stub}
  $1=="uid" {print "uid", $10}' | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
say '```'

primary=$(printf '%s\n' "$listing" | awk -F: '$1=="fpr"{print $10; exit}')
signer=$(printf '%s\n' "$listing" | awk -F: '
  $1=="sec"||$1=="ssb" {caps=$12; usable=($15!="#")}
  $1=="fpr" && caps ~ /s/ && usable {fpr=$10}
  END {print fpr}')
address="tilas01@users.noreply.github.com"
