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
release-notes tag="":
    #!/usr/bin/env bash
    if [ -n "{{ tag }}" ]; then
        CURRENT_TAG="{{ tag }}"
    else
        CURRENT_TAG=$(git describe --tags --exact-match HEAD 2>/dev/null || echo "HEAD")
    fi
    PREV_TAG=$(git describe --tags --abbrev=0 "$CURRENT_TAG^" 2>/dev/null || echo "")
    REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)
    echo "## What's Changed"
    echo ""
    if [ -n "$PREV_TAG" ]; then
        commits=$(git log --pretty=format:"%H" "$PREV_TAG".."$CURRENT_TAG")
    else
        commits=$(git log --pretty=format:"%H" "$CURRENT_TAG")
    fi
    for hash in $commits; do
        message=$(git log -1 --pretty=format:"%s" "$hash" | perl -pe 's/(`[^`]*`)(*SKIP)(*FAIL)|</&lt;/g; s/(`[^`]*`)(*SKIP)(*FAIL)|>/&gt;/g' | sed -E 's/^([a-z]+)(\([^)]*\))?:/**\1\2:**/')
        author=$(gh api "/repos/$REPO/commits/$hash" --jq '.author.login // .commit.author.name')
        printf '* %s by @%s in %s\n' "$message" "$author" "$hash"
    done

# Generate and push release notes from git log since previous tag
push-release-notes tag="":
    #!/usr/bin/env bash
    if [ -n "{{ tag }}" ]; then
        CURRENT_TAG="{{ tag }}"
    else
        CURRENT_TAG=$(git describe --tags --exact-match HEAD 2>/dev/null || echo "HEAD")
    fi
    # Update release-notes
    gh release edit "${CURRENT_TAG}" --notes-file <(just release-notes "${CURRENT_TAG}")
