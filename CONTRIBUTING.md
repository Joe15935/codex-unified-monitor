# Contributing

Keep the application local, read-only, and metadata-only. Preserve upstream MIT notices when porting code. Prefer isolated provider/parser/pricing changes over new services or frameworks.

Use Node 22.12+, current Rust stable, and macOS/Xcode for desktop builds. Run `npm ci`, `npm test`, `npm run build`, and both Rust formatting checks. Verify UI changes in the actual Tauri app. Add synthetic fixtures for accounting/provider bugs; never upload real Codex logs or authentication material.

Describe the observed problem, final behavior, source/version assumptions and actual validation in pull requests. New prices need official source URLs, a verification date, an explicit pricing mode, and any promotion/context caveats. Unknown fields remain unavailable rather than guessed.

Releases use semver, source tags, `.app.zip`/DMG artifacts and SHA256 checksums. A release without Apple notarization must say so. Downloadable files must not contain a developer database, exports or real usage snapshots.
