# Contributing

Keep the application local, read-only, and metadata-only. Preserve upstream MIT notices when porting code. Prefer isolated provider/parser/pricing changes over new services or frameworks.

Use Node 22.12+ and current Rust stable. Desktop builds use macOS/Xcode or Windows 10/11 x64 with the MSVC Rust toolchain, Visual Studio C++ Build Tools, Windows SDK and WebView2. Run the focused tests while developing; before completing a change run:

```sh
npm ci
npm test
npm run build
cargo fmt --manifest-path crates/core/Cargo.toml -- --check
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
```

Package on the target OS using the [README instructions](README.md#build-from-source).
Verify UI changes in the actual Tauri app. Windows CI compilation and synthetic
provider/watcher tests do not establish real-account or interactive desktop
acceptance; report those separately. Add synthetic fixtures for accounting/provider
bugs, including paths with spaces and Unicode when changing Windows discovery.
Never upload real Codex logs or authentication material.

Describe the observed problem, final behavior, source/version assumptions and actual validation in pull requests. New prices need official source URLs, a verification date, an explicit pricing mode, and any promotion/context caveats. Unknown fields remain unavailable rather than guessed.

Keep native watcher work bounded and recovering, and preserve a fixed cadence for
active controlled audits. Parser changes must keep event identities, unknown-model
handling and per-turn audit controls intact. Do not infer Standard/Fast-off from
missing service-tier fields. The [research record](docs/RESEARCH-2026-09-12.md)
separates upstream inspiration from copied code and documents deferred replay
assumptions. Keep Chinese and English labels in sync without changing machine
fields in JSON/CSV exports.

Releases use semver, source tags, macOS `.app.zip`/DMG and Windows x64 NSIS artifacts,
and SHA256 checksums. A release without Apple notarization or Windows code signing
must say so. Keep Windows beta and Auditor preview limits explicit until their
respective acceptance criteria are met. Downloadable files must not contain a
developer database, exports or real usage snapshots. A public source push is not
the same as verified, publicly downloadable release assets.
