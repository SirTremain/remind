# remind

## Contributing

This repo includes a pre-commit hook at `.githooks/pre-commit` that keeps SQLx metadata up to date.

1. Install SQLx CLI:
```sh
cargo install sqlx-cli
```
2. Enable repository hooks:
```sh
git config core.hooksPath .githooks
```

The hook runs:

- `cargo check --workspace --all-targets --all-features`
- `cargo sqlx prepare --check --workspace -- -p api --all-features --all-targets`
- If that fails, it runs `cargo sqlx prepare --workspace -- -p api --all-features --all-targets` and stages `.sqlx` updates automatically.
