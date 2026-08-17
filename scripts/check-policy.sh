#!/bin/sh
set -eu

required_files='README.md AGENTS.md ARCHITECTURE.md ATTRIBUTION.md source.lock.json perception.lock.json world.manifest.json style.lock.json render.manifest.json release.json assurance/evidence.toml'

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

if ! grep -q '"google_content_permitted": false' source.lock.json; then
    echo 'Google production content must remain explicitly disabled' >&2
    exit 1
fi

if grep -R -n --exclude='check-policy.sh' --exclude-dir=node_modules --exclude-dir=.venv --exclude-dir=dist --exclude-dir=target '[—–]' .github scripts crates web perception styles docs ARCHITECTURE.md README.md ATTRIBUTION.md 2>/dev/null; then
    echo 'em dash or en dash is prohibited by repository writing policy' >&2
    exit 1
fi

if grep -R -n -E 'uses:[[:space:]]+[^[:space:]#]+@(v[0-9]+|stable|main|master)([[:space:]#]|$)' .github/workflows; then
    echo 'GitHub Actions must use an immutable reviewed commit reference' >&2
    exit 1
fi

if ! grep -Fq 'types: [synchronize, reopened, labeled, unlabeled]' .github/workflows/ci.yml; then
    echo 'pull request CI must wait for contract labels and rerun when they change' >&2
    exit 1
fi

echo 'repository policy passed'
