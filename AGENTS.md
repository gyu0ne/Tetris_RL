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

If the Docker daemon is unavailable, implementation may continue, but verification remains explicitly blocked until the container commands run.

## Engine boundaries

- `engine-core` contains deterministic, rules-agnostic mechanics.
- TETR.IO-specific literals must eventually live in `rules-tetrio` with source confidence and fixture IDs.
- Do not label provisional spawn, RNG, kick, timing, attack, or garbage behavior as confirmed TETR.IO conformance.
- Integer/fixed-point state transitions only; no floating point in the authoritative engine.
