# Security Policy

## 1. Security Architecture Principles

Vocument treats **all opened documents as untrusted input** (`SECARCH-001`):

- **Strict Offline Default (`NET-001`):** No automatic external network requests. All external network operations must pass through `NetworkBroker` with explicit policy approval.
- **No Automatic Code Execution (`SECARCH-002`, `SECARCH-003`):** PDF JavaScript, external links, embedded attachments, and 3D/multimedia objects are never executed automatically.
- **Controlled File Access (`FileBroker`):** Native platform sandboxes (macOS App Sandbox, Android Storage Access Framework) govern file access.
- **Verified Redaction (`REDACT-001`, `REDACT-002`):** Redaction permanently removes underlying text, OCR layers, and hidden objects, verified via reopen/inspection checks.
- **Secrets Storage:** Passwords, tokens, private keys, and certificate secrets are never stored in repository config or plain files; OS secure storage is mandatory.

---

## 2. Reporting a Vulnerability

Please **do not** report security vulnerabilities through public GitHub issues.

Instead, use GitHub's [Private Vulnerability Reporting](https://github.com/mazer666/Vocument/security/advisories/new) on this repository.

Include:
- Affected component and version/commit
- Minimal reproduction steps or proof-of-concept input (redacted of personal data)
- Impact assessment (e.g. crash, sandbox escape, redaction bypass, unintended network access)
