#!/bin/sh
set -eu

required_files='README.md AGENTS.md ARCHITECTURE.md ATTRIBUTION.md source.lock.json perception.lock.json world.manifest.json style.lock.json render.manifest.json release.json assurance/evidence.toml .github/ISSUE_TEMPLATE/capability.yml .github/ISSUE_TEMPLATE/requirement.yml .github/ISSUE_TEMPLATE/task.yml'

for required_file in $required_files; do
    if [ ! -f "$required_file" ]; then
        echo "missing required policy file: $required_file" >&2
        exit 1
    fi
done

if find styles fixtures -type f | grep -Eiq '(^|/)(car|cars|person|people|vehicle|vehicles|bus|crane)([._/-]|$)'; then
    echo 'transient-named style or fixture asset is prohibited' >&2
    exit 1
fi

if ! grep -q '"google_content_permitted": true' source.lock.json; then
    echo 'the owner-authorized Google reference-capture decision must remain explicit' >&2
    exit 1
fi

if grep -R -n -E '^\*\*\* (Add|Update|Delete) File:' .github; then
    echo 'issue templates contain an unexpanded patch marker' >&2
    exit 1
fi

for issue_template in \
    .github/ISSUE_TEMPLATE/capability.yml \
    .github/ISSUE_TEMPLATE/requirement.yml \
    .github/ISSUE_TEMPLATE/task.yml \
    .github/ISSUE_TEMPLATE/research.yml \
    .github/ISSUE_TEMPLATE/decision.yml \
    .github/ISSUE_TEMPLATE/defect.yml; do
    if [ "$(grep -c '^name:' "$issue_template")" -ne 1 ] ||
        [ "$(grep -c '^body:' "$issue_template")" -ne 1 ] ||
        ! grep -q '^labels:' "$issue_template"; then
        echo "malformed issue template: $issue_template" >&2
        exit 1
    fi
done

if grep -R -n --exclude='check-policy.sh' --exclude-dir=node_modules --exclude-dir=.venv --exclude-dir=dist --exclude-dir=target '[—–]' .github scripts crates web perception styles docs ARCHITECTURE.md README.md ATTRIBUTION.md 2>/dev/null; then
    echo 'em dash or en dash is prohibited by repository writing policy' >&2
    exit 1
fi

if grep -R -n -E 'uses:[[:space:]]+[^[:space:]#]+@(v[0-9]+|stable|main|master)([[:space:]#]|$)' .github/workflows; then
    echo 'GitHub Actions must use an immutable reviewed commit reference' >&2
    exit 1
fi

if ! grep -Fq 'types: [opened, synchronize, reopened, edited, labeled, unlabeled]' .github/workflows/ci.yml; then
    echo 'pull request CI must cover code and contract-metadata changes' >&2
    exit 1
fi

echo 'repository policy passed'
