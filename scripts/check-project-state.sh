#!/bin/sh
set -eu

iso_repo=${ISOMETRIC_REPOSITORY:-dbuddha/isometric-stanford}
iso_owner=${ISOMETRIC_PROJECT_OWNER:-dbuddha}
iso_project=${ISOMETRIC_PROJECT_NUMBER:-2}
iso_tmp=$(mktemp -d)
trap 'rm -rf "$iso_tmp"' EXIT HUP INT TERM

command -v gh >/dev/null 2>&1 || {
    echo 'GitHub CLI is required for the Project state audit' >&2
    exit 1
}

gh project item-list "$iso_project" \
    --owner "$iso_owner" \
    --limit 200 \
    --format json \
    --jq '.items[] | select(.content.type == "Issue") | select(.status == "Ready" or .status == "In Progress") | [.content.number, .status, .title] | @tsv' \
    >"$iso_tmp/active.tsv"

failed=0
while IFS="$(printf '\t')" read -r issue_number issue_status issue_title; do
    [ -n "$issue_number" ] || continue

    gh api "repos/$iso_repo/issues/$issue_number/dependencies/blocked_by" \
        --paginate \
        --jq '.[] | select(.state == "open") | [.number, .title] | @tsv' \
        >"$iso_tmp/blockers.tsv"

    if [ -s "$iso_tmp/blockers.tsv" ]; then
        echo "#$issue_number is $issue_status with unresolved blockers: $issue_title" >&2
        sed 's/^/  #/' "$iso_tmp/blockers.tsv" >&2
        failed=1
    fi
done <"$iso_tmp/active.tsv"

if [ "$failed" -ne 0 ]; then
    exit 1
fi

echo 'Project state passed: no Ready or In Progress issue has an open blocker'
