---
type: status
status: active
zone: academic
last_modified: 2026-05-28T00:00:00-05:00
tldr: "Concordance v0.1.0 ready to tag and ship. Tauri 2 + Rust YAKE engine + Vanilla JS frontend. Tri-platform CI (Windows + macOS-arm64 + macOS-x86_64 + Linux) auto-builds and drafts GitHub Release on v* tags. Gaps closed 2026-05-28: README donation language corrected per 2026-05-27 decision (LLC dropped from cash path, SIUEF direct giving only); tauri.conf.json bundle.active flipped to true. Remaining: verify SIUEF giving URL placeholder, tag v0.1.0, sign Windows .exe with LLC YubiKey after CI run."
priority: normal
related: [ACAD-013, LLC-005-CRYPTO]
next_action_oneline: "Run clean-VM round-trip on the signed Concordance NSIS on PERS-005, commit the v0.1.0 corrections, then tag v0.1.0 to trigger the CI release path."
blockers: "Apple Developer secrets not yet on the AURA-Lab-SIUE/concordance repo (Mac CI sign + notarize will be skipped until set). Windows NSIS + MSI already signed locally 2026-05-28 and ready to attach to the draft GitHub Release. AURA Lab specific named fund vs Mass Comm earmark fund still unresolved (placeholder uses parent SIUE giving page)."
last_session: 2026-05-28T00:00:00-05:00
---

# Concordance: Status

id: ACAD-013

## Tasks

- [ ] Verify the SIUEF giving-page URL placeholder in README.md §Distribution + donations
- [ ] Set Apple Developer secrets on the GitHub repo (APPLE_CERTIFICATE, APPLE_CERTIFICATE_PASSWORD, APPLE_SIGNING_IDENTITY, APPLE_ID, APPLE_PASSWORD, APPLE_TEAM_ID)
- [ ] Commit + push README + STATUS + tauri.conf.json corrections to AURA-Lab-SIUE/concordance main
- [ ] Tag v0.1.0 and push to trigger the build.yml workflow
- [ ] After CI finishes: download the Windows .exe artifact, sign locally with YubiKey + SSL.com EV cert, re-upload to the draft GitHub Release
- [ ] Verify the macOS .dmg is signed + notarized by CI (Apple secrets must be present)
- [ ] Promote the draft release to public
- [ ] Cross-link useresero.app and any other LLC product sites to the new Concordance release

## Architecture summary

- **Engine**: Pure Rust at `src-tauri/src/engine.rs`. Uses `regex` + `serde` + `yake-rust 1.0.3` (English / Spanish / French / German / Portuguese / Italian / Dutch features). Two Tauri commands: `extract_preset(text, top_n, language, remove_stop_words, extra_stopwords) -> Preset` and `check_text(text, preset_json, mode) -> CheckResult`. Two modes for check: `flag-if-present` and `flag-if-missing`.
- **Shell**: Tauri 2 with `tauri-plugin-dialog` and `tauri-plugin-fs`. Window 1100 x 750, min 800 x 500.
- **Frontend**: Vanilla HTML + CSS + JS at `src/`. PDF.js + mammoth.js vendored locally at `src/vendor/`; document parsing (PDF / DOCX / TXT / MD) happens in-browser, air-gap compatible.
- **Bundling**: NSIS for Windows, DMG for macOS, .deb + .rpm + .AppImage for Linux. All targets configured via `bundle.targets: "all"`.
- **CI**: `.github/workflows/build.yml` builds the 4-platform matrix on push to main, PRs to main, and `v*` tags. Tags also auto-create a draft GitHub Release via `tauri-action`. Apple signing + notarization runs in-CI on macOS if secrets are set.

## Funding architecture (locked 2026-05-27)

Per the 2026-05-27 Mass Comm Development meeting, the 2026-05-18 LLC-tunnel plan (LLC operates Gumroad as donation conduit, donates proceeds to AURA Lab via SIUEF) is **retired** as 501(c)(3) self-dealing exposure. New architecture:

- **SIM DAD LLC**: signs the Windows + macOS installers as a service to the lab. No cash channel. No Gumroad listing for Concordance. No revenue routing.
- **AURA Lab**: distributes Concordance free on GitHub Releases. Operates the donation request.
- **End-user donations**: route DIRECTLY from supporter to SIUEF via the SIUEF giving page with AURA Lab earmark. No LLC intermediation.
- **Marketing copy rule**: never claim "proceeds donate to AURA Lab." Use "developed by AURA Lab; if Concordance is useful to you, consider supporting the lab" with a direct SIUEF link.

Source: [reference_donation_conduit_accounts.md](C:\Users\alexl\.claude\projects\C--life-os\memory\reference_donation_conduit_accounts.md).

## Recent Changes

- 2026-05-28 10:30: **Windows installers BUILT + SIGNED locally.** `pnpm tauri build` produced both NSIS (Concordance_0.1.0_x64-setup.exe, 2.15 MB) and MSI (Concordance_0.1.0_x64_en-US.msi) under src-tauri/target/release/bundle/. Cargo release-profile compile in 4m 05s. Both artifacts signed with SIM DAD LLC EV cert via Windows Kits 10.0.28000.0 signtool (NSIS hit the known YubiKey RPC flakiness 0x800706be on first attempt, signed clean on retry per the runbook). signtool verify /pa clean on both. Mac arm64 + Mac x86_64 + Linux .deb/.rpm/.AppImage builds remain CI-only (need Apple Developer secrets on the GitHub repo before next tag-trigger).
- 2026-05-28: **Adopted by life-os from LLC panel** (owner instruction). README donation language corrected to remove the retired LLC-tunnel framing per the 2026-05-27 decision. tauri.conf.json `bundle.active` flipped from false to true so CI runs actually produce installer artifacts. STATUS.md created from scratch (none existed at project root despite reference_donation_conduit_accounts citing one). Project is now release-ready pending: owner verification of the SIUEF giving URL placeholder, Apple Developer secrets on the repo, and Windows post-CI signing with LLC YubiKey.
- 2026-05-18: Tauri 2 scaffold + engine.rs YAKE implementation + frontend with Extract/Check tabs + PDF.js/mammoth.js vendored + tri-platform CI workflow + all icons + 2 example presets (nsf-2025-leaked.json 104 terms, aura-lab-extended.json 447 terms). Per `git log` timestamps on src-tauri/src/engine.rs and .github/workflows/build.yml.

## Decisions Log

- 2026-05-27: **Funding architecture: SIUEF direct giving only.** LLC-tunnel plan (LLC Gumroad as donation conduit) retired due to 501(c)(3) self-dealing exposure. LLC role limited to code-signing service.
- 2026-05-18: **MIT license** for the engine + shell. Free distribution on AURA Lab GitHub Releases.
- 2026-05-18: **No built-in wordlists.** Two example presets ship as repo artifacts in `data/presets/` for download; users bring their own JSON or extract from documents.
- 2026-05-18: **Tri-platform shipping.** Windows + macOS-arm64 + macOS-x86_64 + Linux (.deb + .rpm + .AppImage) via tauri-action in CI.
- 2026-05-18: **No telemetry, no upload, no AI.** Document parsing (PDF/DOCX/TXT) runs in-browser via vendored PDF.js + mammoth.js; engine runs in the Tauri Rust process; nothing leaves the user's machine.
