# Central Configuration

Vocument keeps product defaults, limits and policy in one obvious place.

The source code should use typed settings/config models that correspond to these documented values.

Do **not** scatter magic values through Swift, Kotlin or Rust.

## Files

- `app-defaults.toml` — user-visible default behaviour
- `performance-budgets.toml` — budgets/limits used by scheduling and QA
- `security-policy.toml` — safe defaults

## Secrets

Never store in repository config:

- passwords
- tokens
- private keys
- certificate secrets
- user document paths

Use OS secure storage/runtime state instead.
