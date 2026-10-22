#!/usr/bin/env bash
# Bump the version, commit, tag and push; the tag starts the GitHub release workflow.
#   ./release.sh patch          0.1.0 -> 0.1.1
#   ./release.sh minor          0.1.0 -> 0.2.0
#   ./release.sh major          0.1.0 -> 1.0.0
#   ./release.sh 0.3.0          set an exact version
# Options: -y / --yes skips the confirmation, -n / --dry-run only shows the plan.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]:-$0}")"

YES=0 DRY=0 BUMP=""
for a in "$@"; do
  case "$a" in
    -y|--yes) YES=1 ;;
    -n|--dry-run) DRY=1 ;;
    -*) echo "Unknown option: $a" >&2; exit 2 ;;
    *) BUMP="$a" ;;
  esac
done
[ -n "$BUMP" ] || { sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//' >&2; exit 2; }

command -v node >/dev/null || { echo "Node.js is required to compute the version." >&2; exit 1; }
git rev-parse --git-dir >/dev/null 2>&1 || { echo "Not a git repository." >&2; exit 1; }

CURRENT=$(node -p "require('./src-tauri/tauri.conf.json').version")
NEXT=$(BUMP="$BUMP" CURRENT="$CURRENT" node -e '
  const [a, b, c] = process.env.CURRENT.split(".").map(Number);
  const k = process.env.BUMP;
  const v = k === "patch" ? [a, b, c + 1] : k === "minor" ? [a, b + 1, 0] : k === "major" ? [a + 1, 0, 0] : null;
  if (v) console.log(v.join("."));
  else if (/^\d+\.\d+\.\d+$/.test(k)) console.log(k);
  else { console.error("Version must be patch, minor, major or X.Y.Z"); process.exit(1); }
')
TAG="v$NEXT"
BRANCH=$(git symbolic-ref --short -q HEAD || true)
REMOTE=origin

[ -n "$BRANCH" ] || { echo "Detached HEAD: check out a branch first." >&2; exit 1; }
git remote get-url "$REMOTE" >/dev/null 2>&1 || { echo "No '$REMOTE' remote. Add one first (git remote add origin <url>)." >&2; exit 1; }
[ "$NEXT" != "$CURRENT" ] || { echo "Version is already $CURRENT." >&2; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "Working tree has uncommitted changes. Commit or stash them first." >&2; git status --short >&2; exit 1; }
git rev-parse -q --verify "refs/tags/$TAG" >/dev/null && { echo "Tag $TAG already exists locally." >&2; exit 1; }

echo "==> Checking $REMOTE"
git fetch --quiet "$REMOTE" "$BRANCH" --tags 2>/dev/null || true
git ls-remote --exit-code --tags "$REMOTE" "refs/tags/$TAG" >/dev/null 2>&1 && { echo "Tag $TAG already exists on $REMOTE." >&2; exit 1; }
if git rev-parse -q --verify "$REMOTE/$BRANCH" >/dev/null 2>&1 && [ "$(git rev-list --count "HEAD..$REMOTE/$BRANCH")" != 0 ]; then
  echo "$BRANCH is behind $REMOTE/$BRANCH. Pull first." >&2
  exit 1
fi

echo
echo "  Release  $CURRENT -> $NEXT  (tag $TAG)"
echo "  Branch   $BRANCH  ->  $REMOTE ($(git remote get-url "$REMOTE"))"
echo "  Updates  package.json, src-tauri/tauri.conf.json, src-tauri/Cargo.toml, src-tauri/Cargo.lock"
echo
[ "$DRY" = 1 ] && { echo "Dry run: nothing changed."; exit 0; }
if [ "$YES" != 1 ]; then
  read -r -p "Bump, commit, tag and push? [y/N] " ans
  case "$ans" in y|Y|yes) ;; *) echo "Cancelled."; exit 1 ;; esac
fi

# Replace only the app's own (first) version line in each file. perl instead of sed because
# BSD sed (macOS) has no "0,/re/" first-match address.
export NEXT
perl -0pi -e 's/^(  "version": ")[^"]*"/$1$ENV{NEXT}"/m' package.json src-tauri/tauri.conf.json
perl -0pi -e 's/^version = "[^"]*"/version = "$ENV{NEXT}"/m' src-tauri/Cargo.toml
if [ -f src-tauri/Cargo.lock ]; then
  awk -v v="$NEXT" '
    prev == "name = \"better-docker-desktop\"" && /^version = / { print "version = \"" v "\""; prev = $0; next }
    { print; prev = $0 }
  ' src-tauri/Cargo.lock > src-tauri/Cargo.lock.new && mv src-tauri/Cargo.lock.new src-tauri/Cargo.lock
fi

# Fail before committing if any file did not take the new version.
for f in package.json src-tauri/tauri.conf.json; do
  [ "$(node -p "require('./$f').version")" = "$NEXT" ] || { echo "$f was not updated; restoring." >&2; git checkout -- .; exit 1; }
done
grep -q "^version = \"$NEXT\"" src-tauri/Cargo.toml || { echo "Cargo.toml was not updated; restoring." >&2; git checkout -- .; exit 1; }

git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml
[ -f src-tauri/Cargo.lock ] && git add src-tauri/Cargo.lock 2>/dev/null || true
git commit -m "Release $TAG"
git tag -a "$TAG" -m "Better Docker Desktop $TAG"

echo "==> Pushing $BRANCH and $TAG"
if ! git push --atomic "$REMOTE" "HEAD:refs/heads/$BRANCH" "refs/tags/$TAG"; then
  echo "Push failed. The commit and tag exist locally. Retry the push, or undo with:" >&2
  echo "  git tag -d $TAG && git reset --hard HEAD~1" >&2
  exit 1
fi

echo
echo "Pushed $TAG. The release workflow is building Windows and macOS now."
URL=$(git remote get-url "$REMOTE" | sed -E 's#^git@[^:]+:#https://github.com/#; s#\.git$##')
echo "Watch it: $URL/actions"
