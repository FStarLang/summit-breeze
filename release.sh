#!/usr/bin/env bash
set -euo pipefail

# Automate the release process:
#   1. Bump versions in Cargo workspace and VS Code extension
#   2. Update Cargo.lock
#   3. Create a release commit and tag
#
# Usage:
#   ./release.sh <version>        # e.g. ./release.sh 0.3.0
#   ./release.sh --dry-run <ver>  # preview changes without committing

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
DRY_RUN=false

usage() {
    echo "Usage: $0 [--dry-run] <version>"
    echo ""
    echo "Examples:"
    echo "  $0 0.3.0"
    echo "  $0 --dry-run 0.3.0"
    exit 1
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) DRY_RUN=true; shift ;;
        -h|--help) usage ;;
        -*) echo "Unknown option: $1"; usage ;;
        *) VERSION="$1"; shift ;;
    esac
done

if [ -z "${VERSION:-}" ]; then
    usage
fi

# Validate semver format
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
    echo "ERROR: Version '$VERSION' is not valid semver (expected X.Y.Z or X.Y.Z-pre)"
    exit 1
fi

TAG="v$VERSION"

# Check for clean working tree (unless dry-run)
if [ "$DRY_RUN" = false ]; then
    if ! git -C "$REPO_ROOT" diff --quiet || ! git -C "$REPO_ROOT" diff --cached --quiet; then
        echo "ERROR: Working tree has uncommitted changes. Please commit or stash them first."
        exit 1
    fi
fi

# Check tag doesn't already exist
if git -C "$REPO_ROOT" rev-parse "$TAG" >/dev/null 2>&1; then
    echo "ERROR: Tag '$TAG' already exists."
    exit 1
fi

echo "==> Bumping version to $VERSION"

# 1. Update workspace Cargo.toml
CARGO_TOML="$REPO_ROOT/Cargo.toml"
echo "   Updating $CARGO_TOML"
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" "$CARGO_TOML"

# 2. Update VS Code extension package.json
PACKAGE_JSON="$REPO_ROOT/editors/vscode/package.json"
echo "   Updating $PACKAGE_JSON"
sed -i "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" "$PACKAGE_JSON"

# 3. Update Cargo.lock
echo "   Updating Cargo.lock"
(cd "$REPO_ROOT" && cargo update --workspace --quiet 2>&1)

echo ""
echo "==> Version changes:"
git -C "$REPO_ROOT" diff --stat
echo ""
git -C "$REPO_ROOT" diff

if [ "$DRY_RUN" = true ]; then
    echo ""
    echo "==> Dry run complete. Reverting changes..."
    git -C "$REPO_ROOT" checkout -- .
    echo "   Done. No changes were kept."
    exit 0
fi

# 4. Commit
echo ""
echo "==> Creating commit and tag..."
git -C "$REPO_ROOT" add -A
git -C "$REPO_ROOT" commit -m "Release $TAG"

# 5. Tag
git -C "$REPO_ROOT" tag -a "$TAG" -m "Release $TAG"

echo ""
echo "==> Release $TAG created successfully!"
echo ""
echo "Next steps:"
echo "  git push origin main $TAG"
