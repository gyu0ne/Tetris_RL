# Repository Working Notes

The authoritative project rules are `.agent/RULE.md`, and every work turn begins by reading `.agent/CONTINUITY.md`.

## Container workflow

Do not install Rust or project dependencies on the host. Use the pinned container:

```text
docker compose build rust
docker compose run --rm rust cargo fmt --all --check
docker compose run --rm rust cargo clippy --workspace --all-targets -- -D warnings
docker compose run --rm rust cargo test --workspace --all-targets
docker compose run --rm rust cargo build --workspace --release
```

Manual solo mechanics testing uses the same containerized Rust engine:

```text
docker compose up --build playground
# open http://127.0.0.1:8787
docker compose stop playground
```

Python learning code uses the pinned CPU training image; no host Python packages are required:

```text
docker compose build training
docker compose run --rm training ruff format --check --config python/pyproject.toml python
docker compose run --rm training ruff check --config python/pyproject.toml python
docker compose run --rm training python -m unittest discover -s python/tests -v
```

Demonstration shards under `datasets/` are temporary and ignored. Retained checkpoints must contain the manifest and normalization needed to regenerate then delete those shards.

If the Docker daemon is unavailable, implementation may continue, but verification remains explicitly blocked until the container commands run.

## Engine boundaries

- `engine-core` contains deterministic, rules-agnostic mechanics.
- TETR.IO-specific literals must eventually live in `rules-tetrio` with source confidence and fixture IDs.
- Do not label provisional spawn, RNG, kick, timing, attack, or garbage behavior as confirmed TETR.IO conformance.
- Integer/fixed-point state transitions only; no floating point in the authoritative engine.
