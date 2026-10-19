#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Run by .github/workflows/rewrite.yml. Builds the signed rewrite of dev and
# main with the key in the repository's secrets, and hands the result over
# without pushing either branch.
#
# It cannot push them itself: the run's token may not create or change a
# workflow file, and a rewrite makes every commit that ever touched one anew.
# So the rewritten commits go into a git bundle, holding only what the
# current branches lack, and the bundle into one commit of its own on a
# branch of its own. The run prints that commit's hash and deletes the branch;
# the commit stays fetchable by its hash. A clone with ordinary push rights
# then fetches it, checks the bundle against the branches as they are, and
# pushes with `tools/repo/rewrite.py --push`, which refuses if either branch
# has moved since. Nothing secret is in the bundle: signatures are public.
set -euo pipefail

. "$(dirname "$0")/actions_key.sh"

git config user.name tilas01
git config user.email "$address"
git config gpg.program "$RUNNER_TEMP/gpg-loopback"
git fetch --quiet origin dev main

python3 tools/repo/rewrite.py --sign "$signer!" | tee -a "$GITHUB_STEP_SUMMARY"

# rewrite.py says so and stores nothing when no commit needs rewriting.
if ! git rev-parse -q --verify refs/rewrite/dev >/dev/null; then
  exit 0
fi

out="$RUNNER_TEMP/handover"
mkdir -p "$out"
for name in dev main old-dev old-main; do
  printf '%s %s\n' "$name" "$(git rev-parse "refs/rewrite/$name")"
done > "$out/tips.txt"
cp .git/rewrite-map.txt "$out/rewrite-map.txt"
git bundle create "$out/rewrite.bundle" refs/rewrite/dev refs/rewrite/main \
  --not origin/dev origin/main

tree=$(
  for file in rewrite.bundle rewrite-map.txt tips.txt; do
    printf '100644 blob %s\t%s\n' "$(git hash-object -w "$out/$file")" "$file"
  done | git mktree
)
handover=$(git commit-tree --no-gpg-sign -m "Rewrite handover from run $GITHUB_RUN_ID" "$tree")
branch="rewrite-handover-$GITHUB_RUN_ID"
git push --quiet origin "$handover:refs/heads/$branch"
git push --quiet origin --delete "$branch"
{
  echo
  echo "Handover commit: $handover"
  cat "$out/tips.txt"
} | tee -a "$GITHUB_STEP_SUMMARY"
rm -f "$pp"
