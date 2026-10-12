# DS339 Minimal Clock

Rust desktop clock for the JONSBO DS339 (USB 345F:9132). Windows uses native
HID feature reports for control and dynamically loads the official libusb0.dll
for interface 3 / bulk OUT 0x04. Linux uses hidapi and rusb.

## Structure and constraints

- `src/ds339.rs`: HID control and panel status.
- `src/bulk_windows.rs`, `src/bulk.rs`: platform USB transport.
- `src/frame.rs`: native 376x960 BGR frame envelope; escape pixel 0xFF to 0xFE.
- `src/transport.rs`: shared 64 KiB frame chunking and fail-stop error context.
- `src/renderer.rs`, `src/clock.rs`: logical 960x376 rendering and output loop.
- `src/gui.rs`, `src/profile.rs`, `src/autostart.rs`: settings, profiles and tray.
- `docs/`: test pattern and troubleshooting evidence.
- `target/`, `dist/`: generated, ignored build artifacts.

Preserve profiles, existing user changes and OEM driver compatibility. Do not
retry partially timed-out USB frames without protocol evidence. Begin device
diagnosis with the CLI without arguments; it checks HID/panel status and opens
the bulk interface without changing display mode or sending pixels.
Obtain explicit approval before live display tests, stopping an installed app,
changing drivers/settings, or replacing installed executables. Keep backups.
Do not commit or publish without an explicit request.

## Checks

Stable Rust with rustfmt and clippy; Windows OEM libusb0.dll for physical tests.
Linux native dependencies are listed in README.md.

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo build --release --bins
```

Passing unit tests or a USB probe does not prove frames reach the physical
panel. Report live-transfer results separately from visible display verification.
Startup must initialize panel/GPIO and send two complete frames before enabling
the screen. See `docs/usb-troubleshooting.md` for the protocol evidence and the
bounded Windows hardware verification. Do not skip that initialization based
only on Device Manager recognition or a successful HID status read.

## Releases

Keep the package version in `Cargo.toml` and `Cargo.lock` aligned with the tag.
Add reviewed release notes under `docs/releases/<tag>.md`; the release workflow
uses that file when present and includes `docs/` in both platform archives.
Pushing a `v*` tag runs Windows/Linux verification and automatically publishes
a public GitHub Release after both builds pass. Confirm the repository, commit,
tag, push and public publication before pushing; do not treat it as draft-only.
