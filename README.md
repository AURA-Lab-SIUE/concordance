# Concordance

**Topic-agnostic term checker.** Check text against a wordlist, or bag-of-words any document into a custom list. Runs entirely on your computer. No upload. No AI.

Built by **[AURA Lab](https://aura-lab-siue.github.io/)** at Southern Illinois University Edwardsville. Distributed in collaboration with **[SIM DAD LLC](https://simdadllc.com)**, which signs the desktop installers (Windows + macOS) and operates the Gumroad listing as a donation conduit for the lab.

## Status

In active development. Scaffold phase; engine port from the [Banned Word Checker](https://github.com/aura-lab-siue/banned-word-checker) is underway.

## What it does

Two modes:

1. **Check mode.** Paste text. Choose a wordlist: a bundled preset, your own JSON, or a governing document Concordance will bag-of-words into a preset on the fly. See which terms hit, how often, and in what context. Two scan modes: flag-if-found ("banned-word-checker style") or flag-if-missing ("CFP-alignment style", which is good for proposal writers who want to make sure they're using the funder's vocabulary).
2. **Extract mode.** Upload a CFP, RFP, journal scope statement, job posting, or any governing document. Concordance tokenizes, removes stop-words, counts terms, and gives you a ranked frequency list. Save it as a preset to reuse with Check mode.

## Architecture

- Pure-Rust engine (`src-tauri/src/engine.rs`)
- Tauri 2 desktop shell with vanilla HTML/CSS/JS frontend
- Tri-platform: Windows (`.exe` via NSIS) + macOS (`.dmg` notarized) + Linux (`.deb` + `.rpm` + `.AppImage`)
- No telemetry, no upload, no AI

## Wordlists

Two bundled presets, available for independent download from the `data/presets/` folder:

- `nsf-2025-leaked.json`: the NSF leaked decision-tree list (~290 terms)
- `aura-lab-extended.json`: the larger 447-term superset (leaked + community-defensive)

You can also drop in your own preset JSON or have Concordance extract one from a governing document.

## Sibling product

The narrow [Banned Word Checker](https://aura-lab-siue.github.io/banned-word-checker/) web tool uses the NSF preset specifically and runs the leaked 3-stage decision tree. Concordance is its generalized, downloadable cousin.

## License

MIT. See [LICENSE](LICENSE).

## Distribution + donations

Concordance ships free on two channels:

- **AURA Lab GitHub Releases** with a donate-to-the-lab option
- **SIM DAD LLC Gumroad** (pay what you want, including $0)

All Gumroad proceeds are donated by SIM DAD LLC to AURA Lab via the SIUE Foundation. Pay what you want; the tool is free either way.

## Building locally

Prerequisites: Rust 1.77+, Node 18+, Tauri CLI 2.x.

```bash
npm install
npm run dev    # development build with hot reload
npm run build  # production build, bundles for current platform
```

Cross-platform builds happen automatically on push via GitHub Actions (`windows-latest` + `macos-latest` + `ubuntu-22.04`).
