_help:
    @just -l

# Lint the project
lint:
    cargo fmt -- --check
    cargo clippy -- -D warnings
    actionlint

# Format the project
fmt:
    cargo fmt

# Build the project
build:
    cargo build --release

# Test the project
test:
    cargo test --all-features

# Generate release notes from git log since previous tag
release-notes:
    #!/usr/bin/env bash
    CURRENT_TAG=$(git describe --tags --exact-match HEAD 2>/dev/null || echo "HEAD")
    PREV_TAG=$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo "")
    REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
    echo "## What's Changed"
    echo ""
    if [ -n "$PREV_TAG" ]; then
        git log --pretty=format:"%H|%s" "$PREV_TAG"..HEAD
    else
        git log --pretty=format:"%H|%s"
    fi | while IFS='|' read -r hash message; do
        author=$(gh api "/repos/$REPO/commits/$hash" --jq '.author.login // .commit.author.name')
        echo "* $message by @$author in $hash"
    done
