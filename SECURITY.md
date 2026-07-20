# Security Policy

## Reporting Vulnerabilities

Please report security vulnerabilities via a private GitHub security advisory
or to the project maintainers directly. Do not open public issues for
unpatched vulnerabilities.

## Threat Model

PalmSync is a **cloudless LAN P2P** system. The primary threats are:

- Eavesdropping on workspace state (file paths, clipboard content) on the local network
- Unauthorized nodes joining the sync network
- Malicious/malformed snapshots triggering unwanted actions (e.g. path traversal into `code` CLI invocation)
- DoS via connection or message flooding

## Security Measures (implemented)

| Layer | Measure |
|---|---|
| Transport | End-to-end AES-256-GCM encryption of all post-handshake messages |
| Key derivation | Domain-separated SHA-256 of the shared password (`palmsync-e2ee-v1:` prefix) |
| Integrity | AES-GCM authenticated encryption tag (tamper ⇒ decrypt failure) |
| Authentication | Nonce-based challenge-response handshake (init → challenge → proof → result); proof = SHA-256(password_hash : per-connection nonce), replay-resistant. Argon2id available for at-rest password storage (`shared::auth`) |
| Input validation | Snapshot validation on both master and slave: path traversal, control chars, size limits, node-id charset (`shared::validation`) |
| DoS protection | Per-IP connection rate limit (10/min), per-node message rate limit (100/10s), 32 MB max payload |
| Clipboard | 10 MB size cap, private-key/API-key pattern detection with audit warning, secure wipe (`clear_secure`) |
| Audit logging | `audit.log` records auth attempts, grab/drop syncs, and security events (rate limits, decrypt failures, invalid snapshots) |

## Known Limitations / Roadmap

- [ ] Transport is WS + message-level E2EE, not WSS/TLS — metadata (message sizes, timing) is visible on the wire. Intentionally deferred: AES-256-GCM already provides confidentiality + integrity on the LAN, and WSS would add per-deployment cert distribution/pinning. Revisit for untrusted-network use.
- [ ] Binaries are unsigned; code signing planned before public distribution.
- [ ] No key rotation; changing the password re-keys everything manually.

## Development Practices

```bash
cargo clippy --all-targets   # 0 warnings policy
cargo audit                  # dependency vulnerability scan
cargo test                   # includes crypto tamper/negative tests
```

- Default password (`palmsync`) MUST be changed in `config.json` before real use.
- Logs never contain clipboard contents or passwords.
- The Windows service runs with default service privileges; do not grant it admin.
