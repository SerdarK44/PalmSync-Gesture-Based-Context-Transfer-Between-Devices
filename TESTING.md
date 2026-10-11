# PalmSync — Test Guide

## Test Suites

| Suite | Location | Count | Command |
|---|---|---|---|
| Shared unit (crypto, auth, validation, audit, config, messages) | `shared/src/*` | 33 | `cargo test -p palmsync-shared` |
| Networking unit (handshake challenge-response, rpc, rate limiter) | `networking/src/*` | 10 | `cargo test -p palmsync-networking --lib` |
| Integration (master/slave over real WS) | `networking/tests/integration_tests.rs` | 10 | `cargo test -p palmsync-networking --test integration_tests` |
| Security (wire-level attacks, replay) | `networking/tests/security_tests.rs` | 8 | `cargo test -p palmsync-networking --test security_tests` |
| Regression (pinned bugs) | `networking/tests/regression_tests.rs` | 11 | `cargo test -p palmsync-networking --test regression_tests` |
| Load (manual) | `networking/tests/load_tests.rs` | 3 | `cargo test -p palmsync-networking --test load_tests -- --ignored --nocapture` |
| Brain unit (context capture, executor, clipboard, log rotation, IPC) | `brain/src/*` | 13 | `cargo test -p palmsync-brain` |
| C++ gesture FSM | `eye/tests/gesture_test.cpp` | 12 | `eye\build.ps1` (builds + runs) |
| C++ hand detector (face masking, hand lock-on) | `eye/tests/detector_test.cpp` | 10 | `palmsync-detector-test.exe` (eye build dir) |
| E2E binaries | `brain/src/bin/{e2e_test,eye_brain_e2e}.rs` | 2 | `cargo run --bin e2e_test` |

**Toplam: 107 otomatik test + 3 load testi + 2 E2E senaryosu**

## Quick Commands

```bash
# Everything (except load tests)
cargo test --workspace

# Static analysis — 0 warnings policy
cargo clippy --all-targets -- -D warnings

# Dependency vulnerabilities
cargo audit

# Performance benchmarks (criterion)
cargo bench -p palmsync-shared

# Load tests (100 concurrent connections, throughput)
cargo test -p palmsync-networking --test load_tests -- --ignored --nocapture
```

## Performance Baselines (2026-07-20, this machine)

| Metric | Result | Target |
|---|---|---|
| 100 concurrent slave connections | 100/100 OK in 0.11s | ≥95% success |
| JSON serialization throughput | ~75,000 msg/s | >10,000 msg/s |
| E2EE grab throughput (full pipeline) | ~6,400 grabs/s | — |
| Full seal→open roundtrip (4KB snapshot) | ~61 µs | <50 ms |
| AES-256-GCM encrypt 4KB | ~13 µs | — |
| AES-256-GCM encrypt 1MB | ~3.9 ms | — |
| Argon2id hash | ~17 ms | intentionally slow |

## CI

`.github/workflows/ci.yml` runs on push/PR:
1. **test-rust** — workspace tests, clippy (deny warnings), bench compile, cargo-audit
2. **load-test** — the 3 ignored load tests
3. **test-cpp** — gesture tests without OpenCV (`-DPALMSYNC_BUILD_APP=OFF`)
4. **test-ui** — dotnet build

## Notes

- WDAC: build outputs must live outside the Desktop tree (see `.cargo/config.toml`, `eye/build.ps1`, `ui/Directory.Build.props`).
- Load tests use `MasterNode::new_with_limits` because all test slaves share the loopback IP; production defaults are 10 conn/min/IP and 100 msg/10s/node.
- Clipboard tests touch the real Windows clipboard; run them on a machine where clobbering the clipboard is acceptable.
