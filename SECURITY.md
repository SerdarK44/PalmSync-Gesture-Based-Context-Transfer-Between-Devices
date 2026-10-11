# Security Policy

## Reporting Vulnerabilities

Please report security vulnerabilities via a private GitHub security advisory
or to the project maintainers directly. Do not open public issues for
unpatched vulnerabilities.

## Threat Model

PalmSync is a **cloudless LAN P2P** system. The primary threats are:

- Eavesdropping on workspace state (documents, URLs, screenshots, clipboard) on the local network
- Unauthorized nodes joining the session
- A malicious or malformed snapshot making the receiving machine run code
  (launching a program, opening a script with its interpreter, shell
  metacharacters in a URL or file name, path traversal on write)
- DoS via connection or message flooding, or dead links holding resources

## Security Measures (implemented)

| Layer | Measure |
|---|---|
| Transport | End-to-end AES-256-GCM encryption of all post-handshake messages |
| Key derivation | Domain-separated SHA-256 of the shared password (`palmsync-e2ee-v1:` prefix) |
| Integrity | AES-GCM authenticated encryption tag (tamper ⇒ decrypt failure) |
| Authentication | Nonce-based challenge-response handshake (init → challenge → proof → result); proof = SHA-256(password_hash : per-connection nonce), replay-resistant. Argon2id available for at-rest password storage (`shared::auth`) |
| Input validation | Every snapshot is validated on both master and slave (`shared::validation`): path traversal, control characters, size limits (text 2 MB, binary 8 MB, screenshot 2 MB base64), strict base64, app-name charset, allowed app kinds, node-id charset |
| Restore: programs | A program path received from the other machine is **never executed**. It is only a hint; the receiver looks the app up itself (App Paths registry, known install folders, PATH), requires a matching executable name, rejects network shares and Store-app paths, allows only a few known tools inside `%WINDIR%`, and never starts shells, script hosts, interpreters, installers or system tools (`brain::apps::may_launch`) |
| Restore: files | Files that run code when opened (`.exe .bat .cmd .ps1 .vbs .js .hta .py .lnk .msi .reg …`) are saved but only ever opened **in a text editor**, never with their default program. Received files are written only under `Documents\PalmSync\Gelen Dosyalar` (relative path components sanitized) and get a Mark-of-the-Web (`Zone.Identifier`, ZoneId=3) so Office uses Protected View and SmartScreen applies |
| Restore: other handlers | When the file type has no default program, other programs registered for the extension are tried ("Open with" list). Each candidate's command must resolve to a program allowed by the same rules (no shells, `rundll32`, script hosts, installers, network paths); the file is opened with `ShellExecuteExW` + class, never by building a command line |
| Restore: no shell | URLs, files and folders are opened with `ShellExecuteW` and programs with `CreateProcess` — never through `cmd /C start`, so `&`, `|`, `^` in received data cannot become commands |
| Capture | PowerShell helpers get window titles/names through environment variables, never inside script text (a web page title cannot inject code). No screenshot is taken of password managers, credential/UAC prompts or private/incognito browser windows |
| Liveness | Slaves ping every 10 s and drop a link silent for 30 s; the master closes slaves silent for 45 s |
| DoS protection | Per-IP connection rate limit (10/min), per-node message rate limit (100/10s), 32 MB max payload |
| Clipboard | 10 MB size cap, private-key/API-key pattern detection with audit warning |
| Audit logging | `audit.log` records auth attempts, grab/drop syncs, and security events (rate limits, decrypt failures, invalid snapshots) |

## Privacy notes

- A GRAB sends a JPEG of the window in front. It contains whatever that
  window shows (other tab names, notifications inside the app…). It travels
  encrypted and only to the machines in your session.
- Nothing is uploaded anywhere; there is no server.

## Known Limitations / Roadmap

- [ ] Transport is WS + message-level E2EE, not WSS/TLS — metadata (message sizes, timing) is visible on the wire. Intentionally deferred: AES-256-GCM already provides confidentiality + integrity on the LAN, and WSS would add per-deployment cert distribution/pinning. Revisit for untrusted-network use.
- [ ] Binaries are signed with a self-signed certificate; SmartScreen reputation needs a commercial code-signing certificate.
- [ ] No key rotation; changing the password re-keys everything manually.

## Development Practices

```bash
cargo clippy --workspace --all-targets -- -D warnings   # 0 warnings policy
cargo audit                                             # dependency vulnerability scan
cargo test --workspace                                  # includes crypto tamper and restore-policy tests
```

- Logs never contain clipboard contents, file contents or passwords.
- The daemon runs as a normal user process started by the tray app; it needs
  no administrator rights.
