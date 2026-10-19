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

if ! printf '%s\n' "$listing" | grep '^uid:' | grep -q "<$address>"; then
  say ""
  say "The key carries no identity with $address, so GitHub cannot match it to tilas01's commits."
  say "On the computer that holds the key, add the address and register the result:"
  say "gpg --quick-add-uid $primary 'tilas01 <$address>'"
  say "gpg --armor --export $primary   (paste into GitHub, Settings, SSH and GPG keys)"
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
