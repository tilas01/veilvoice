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

. "$(dirname "$0")/actions_key.sh"

# GitHub matches a signature to an account by an identity on the registered
# public key whose address is the committer's. When the key in the secret has
# none, the run adds one to its own copy, which the primary key can certify
# here, and hands over the public half with it as the artifact
# signing-public-key. That file is what to paste into GitHub, under Settings,
# SSH and GPG keys. Nothing secret is in it, and the secret is not changed:
# signing needs no identity, only the registered public key does.
if ! printf '%s\n' "$listing" | grep '^uid:' | grep -q "<$address>"; then
  "$RUNNER_TEMP/gpg-loopback" --quiet --quick-add-uid "$primary" "tilas01 <$address>"
  say ""
  say "The key carried no identity with $address, so GitHub could not match it to tilas01's commits."
  say "This run added one. Download the artifact signing-public-key from this run and paste the"
  say "file's contents at https://github.com/settings/gpg/new"
fi
mkdir -p "$RUNNER_TEMP/public"
gpg --armor --export "$primary" > "$RUNNER_TEMP/public/veilvoice-signing-key.asc"
gpg --show-keys --with-colons "$RUNNER_TEMP/public/veilvoice-signing-key.asc" \
  | awk -F: '$1=="uid" {print "public key identity:", $10}' | tee -a "${GITHUB_STEP_SUMMARY:-/dev/null}"

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
