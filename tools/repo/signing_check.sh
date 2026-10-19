#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Run by .github/workflows/signing.yml, where the signing key is a repository
# secret. Imports the key, says which key it is and which identities it
# carries, signs one commit with it on a branch of its own, asks GitHub whether
# that commit reads Verified, and deletes the branch again.
#
# Nothing secret is printed: a fingerprint, an identity and GitHub's verdict
# are all public. The key arrives in $KEY, armoured or base64 of either form,
# and its passphrase, if it has one, in $PASSPHRASE.
set -euo pipefail

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

if ! printf '%s\n' "$listing" | grep '^uid:' | grep -q "<$address>"; then
  say ""
  say "The key carries no identity with $address, so GitHub cannot match it to tilas01's commits."
  if "$RUNNER_TEMP/gpg-loopback" --quick-add-uid "$primary" "tilas01 <$address>" 2>/dev/null; then
    say "Added that identity here. The public half below now carries it: add it to"
    say "GitHub under Settings, SSH and GPG keys, New GPG key, replacing any older copy."
    say '```'
    gpg --armor --export "$primary" | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"
    say '```'
    # The log masks every line it shares with the secret, and an export
    # shares the public packet's lines, so the key is also carried in the
    # check commit, which stays fetchable by its hash after the branch goes.
    gpg --armor --export "$primary" > signing-public-key.asc
    git add signing-public-key.asc
  else
    say "Could not add one here (the primary key's secret part or its passphrase is missing)."
  fi
fi

git config user.name tilas01
git config user.email "$address"
git config gpg.program "$RUNNER_TEMP/gpg-loopback"
GIT_COMMITTER_DATE="$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  git commit --quiet --allow-empty -S"$signer!" \
  -m "Signing check, made by the signing workflow and deleted at once"
say "check commit $(git rev-parse HEAD)"
sha=$(git rev-parse HEAD)
git log -1 --format='local check: %G? %GF' | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"

branch="signing-check-$GITHUB_RUN_ID-$GITHUB_JOB"
git push --quiet origin "HEAD:refs/heads/$branch"
sleep 5
verdict=$(gh api "repos/$GITHUB_REPOSITORY/commits/$sha" \
  --jq '.commit.verification | "\(.verified) \(.reason)"')
git push --quiet origin --delete "$branch"
say ""
say "GitHub on the signed check commit $sha: $verdict"
say "signer $signer"
rm -f "$pp"
case "$verdict" in
  "true valid") ;;
  *) exit 1 ;;
esac
