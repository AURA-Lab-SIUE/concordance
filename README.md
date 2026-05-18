# Concordance

**A topic-agnostic term checker.** Provide your own wordlist (or build one by extracting from a document); scan any text against it. Runs entirely on your computer. No upload. No AI.

Built by **[AURA Lab](https://aura-lab-siue.github.io/)** at Southern Illinois University Edwardsville. Distributed in collaboration with **[SIM DAD LLC](https://simdadllc.com)**, which signs the desktop installers (Windows + macOS) and operates the Gumroad listing as a donation conduit for the lab.

## Status

In active development.

## What it does

Two operations:

- **Extract a wordlist** from any document. Tokenize text, optionally drop stop-words (English / Spanish / French / German / Portuguese / Italian / Dutch), rank by frequency. Save the result as JSON for reuse.
- **Check text against a wordlist.** Upload a wordlist JSON (or use one you just extracted), then scan your text. Two modes:
  - **Find** — hits are wordlist terms that appear in your text. Useful for compliance review (banned-word checking, style audits, brand-language scans).
  - **Find missing** — hits are wordlist terms that do NOT appear in your text. Useful for proposal alignment (extract a CFP's vocabulary, then check your proposal uses it).

The tool ships with **no built-in wordlists**. You bring your own. Two example wordlists live in [`data/presets/`](data/presets/) for download if you want a starting point.

## Example wordlists in `data/presets/`

These are repo artifacts, not built-in defaults. Download what fits your use case, or build your own:

- **`nsf-2025-leaked.json`** (104 terms) — the leaked NSF decision-tree word list. Source-of-truth for the [AURA Lab Banned Word Checker](https://aura-lab-siue.github.io/banned-word-checker/) sibling product.
- **`aura-lab-extended.json`** (447 terms) — the leaked list plus a community-compiled defensive superset.

You can also drop in any JSON in the Concordance preset format:

```json
{
  "name": "My Wordlist",
  "description": "Optional description",
  "terms": [
    { "term": "diversity", "source": "optional provenance tag" },
    { "term": "equity" }
  ]
}
```

## Architecture

- Pure-Rust engine (`src-tauri/src/engine.rs`) using `regex`, `serde`, `stop-words`
- Tauri 2 desktop shell, vanilla HTML/CSS/JS frontend
- Tri-platform: Windows (`.exe` via NSIS) + macOS (`.dmg` notarized) + Linux (`.deb` + `.rpm` + `.AppImage`)
- No telemetry, no upload, no AI

## Sibling product

The [Banned Word Checker](https://aura-lab-siue.github.io/banned-word-checker/) web tool is the opinionated cousin: it ships the NSF leaked list baked in and runs the leaked 3-stage decision tree. Concordance is the generic engine the same lab built that BWC on top of.

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

Cross-platform builds happen automatically on push via GitHub Actions (`windows-latest` + `macos-latest` x2 + `ubuntu-22.04`).
