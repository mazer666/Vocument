# Contributing to Vocument

Thank you for contributing to Vocument!

## 1. Developer Certificate of Origin (DCO)

Vocument uses the **Developer Certificate of Origin (DCO)** instead of a Contributor License Agreement (CLA). Contributors retain their copyright.

Every commit must include a `Signed-off-by` line (`git commit -s`):

```text
Signed-off-by: Your Name <your.email@example.com>
```

By signing off, you certify that you have the right to submit the contribution under the project's open-source license (**AGPL-3.0-or-later**).

---

## 2. Before Changing Anything

1. Read [`README.md`](README.md) and [`WHERE_TO_CHANGE.md`](WHERE_TO_CHANGE.md).
2. Check [`REQUIREMENTS.md`](REQUIREMENTS.md) for product behaviour rules.
3. Check [`ARCHITECTURE.md`](ARCHITECTURE.md) for module boundaries and architectural constraints.
4. Check [`AGENTS.md`](AGENTS.md) for coding standards, naming conventions, and review matrices.

---

## 3. Pull Request & Commit Checklist

- [ ] Requirements and architecture boundaries respected (`TransactionManager`, `FileBroker`, `NetworkBroker`, `PdfEngine`).
- [ ] Configuration defaults and budgets remain centralized under [`config/`](config/README.md).
- [ ] Unit/integration/acceptance tests added or updated.
- [ ] Documentation updated in the same change if behaviour, config, or structure changed.
- [ ] `./scripts/validate-repo.sh` passes locally.
- [ ] Commits are signed off (`git commit -s`).
