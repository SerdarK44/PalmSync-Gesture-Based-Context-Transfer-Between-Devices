# Known Issues

## Critical
_(none)_

## High Priority
- [ ] **Transport is not TLS** — post-handshake payloads are E2EE (AES-256-GCM authenticated encryption), so confidentiality and integrity hold on the LAN; only message sizes/timing are observable. `wss://` was intentionally deferred: it would add self-signed cert distribution + pinning to every deployment for little gain over message-level E2EE. Revisit if PalmSync ever runs over untrusted networks.

## Medium Priority
- [ ] **Camera detection is classical CV, not ML** — skin-colour segmentation + convexity-defect finger counting (no models). Works offline and is validated by `detector_test`, but is sensitive to lighting and skin-toned backgrounds. Good enough for a deliberate fist/open gesture; not a precise hand tracker.
- [ ] **UI requires .NET 8 Desktop Runtime** on target machines (framework-dependent publish). Installer detects it and warns with a download link, but does not auto-install.
- [ ] **Binaries unsigned** — SmartScreen/AV may flag the installer.
- [ ] **Debug C++ builds do not run under WDAC** — use Release (documented in TESTING.md).
- [ ] **Session code embeds a LAN IP** — only reachable on the same local network (by design). Roaming/relay across networks is out of scope.

## Low Priority / Backlog
- [ ] Global hotkeys (Ctrl+Alt+P) not implemented in tray UI.
- [ ] TR/EN localization not implemented.
- [ ] Coverage tooling (cargo-llvm-cov) not integrated; coverage is estimated from test inventory, not measured.
- [ ] `SlaveNode` has no automatic reconnect loop; `brain::daemon::run_slave` retries only at startup.

## Fixed (regression-tested)
- [x] **REG-002 / notify race** — `SlaveNode::disconnect()` used to leak background tasks and keep TCP open (master registry leaked entries). Fixed with `notify_one()` permit semantics; pinned by `reg_002_disconnect_cleans_registry`.
- [x] **Handshake replay** — the old handshake sent a static SHA-256(password); a captured value could be replayed. Replaced with a 4-step nonce challenge-response (`init → challenge → proof → result`); the wire never carries a reusable secret. Pinned by `replayed_proof_from_old_session_rejected` and `proof_is_nonce_bound_replay_resistant`.
- [x] **Session-0 capture** — the daemon ran as a LocalSystem Windows service in session 0, where it cannot see the user's VS Code window title or clipboard. The tray app now launches the daemon as a child process **in the user's session**, so capture/clipboard work and no admin is needed for daily use. Config moved to `%LOCALAPPDATA%\PalmSync`.
- [x] **IP/role setup friction** — replaced manual master/slave + IP config with a one-screen "Start Session / Connect" flow. The host auto-detects its LAN IP and shows a short code (encodes IP+port) + generated password; the other machine just types them. No IP, port, or role ever shown to the user.
- [x] **Camera gesture detection never worked** — the old Eye used `readNetFromONNX` against MediaPipe `.task`/`.tflite` files (format mismatch, never ran). Replaced with a model-free classical CV detector (skin segmentation + convexity defects): fist = GRAB, open hand = DROP. Launchable from the Home window (📷 button), with a live preview. Verified by `detector_test` (fist openness 0.00 vs open hand 0.50).

---
Status: Last updated 2026-07-20
