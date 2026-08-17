#!/bin/sh
set -eu

scripts/check-policy.sh
python3 scripts/test_validate_pr.py
python3 scripts/validate_manifests.py
python3 scripts/validate_world_fixtures.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
cargo test --workspace --doc --all-features --locked
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked
cargo deny check

mdbook build
mdbook test

perception/.venv/bin/ruff format --check perception
perception/.venv/bin/ruff check perception
perception/.venv/bin/mypy --config-file perception/pyproject.toml
perception/.venv/bin/pytest -c perception/pyproject.toml perception/tests
perception/.venv/bin/pip-audit --local --skip-editable

npm --prefix web run check
npm --prefix web run build
npm --prefix web test -- --run
npm --prefix web audit --omit=dev --audit-level=high
