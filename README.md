# Text Rewrap 📝✨

Rewrap text at your preferred width!!

Great for copying to/from git commits, re-formatting markdowns and more!

## ✨ Features

- 📏 **Custom Width Control** - Set any character width (e.g., 80, 100, 120) or leave blank for no wrapping (effectively an "unwrap")
- 📋 **Clipboard Integration** - Rewrap text directly from your clipboard or paste it in
- 📤 **One-Click Copy** - Automatically copy results to clipboard

## 🚀 How to Use

### Method 1: Rewrap from Form
1. Open the "Text Rewrap Form" command
2. Enter your text into the form
3. Choose your desired width (e.g. `80`) or leave blank for no text wrapping
4. Hit Enter! ✅

### Method 2: Rewrap from Clipboard
1. Copy some text to your clipboard
2. Open the "Rewrap Text Clipboard" command
3. Choose your desired width (e.g. `80`) or leave blank for no text wrapping
4. Hit Enter! ✅

## 💡 Use Cases

- 📖 Formatting text for README files or documentation
- 📄 Cleaning up text copied from PDFs
- 💬 Formatting code comments to a specific width
- 📝 Preparing text for platforms with character width limits

## ⚙️ Options

- **Width**: Enter a number (e.g., `80`, `100`, `120`) or leave blank for no wrapping
- **Copy to Clipboard**: If checked, the resulting text will be copied to your clipboard ready for you to use

## 🦀 Native Rust installable version (Tezbar)

This repository also builds a standalone, installable Tezbar extension:
`com.tezbar.native.text-rewrap` (schema 3, `execution: process`, protocol 1),
replacing `raycast.text-rewrap`. The original Raycast TypeScript source above
(`src/`, `assets/`, `package.json`) is preserved unchanged.

- `extension.json` — package manifest: all three commands, both preferences, icons,
  and the narrow capabilities `host.clipboard.read`, `host.clipboard.write`,
  `host.selection.read`.
- `rust/` — process entry point. It compiles the reviewed V2 adapter and engine in
  [`v2/`](v2/README.md) **unchanged**, served by `tezbar-extension-sdk` (git rev
  `aaec571673ab64a85a136cf6bd054ef660c4d93c`). The adapter's clipboard/selection
  channels map one-to-one onto typed host requests; any other channel fails closed.
  The process boundary normalizes saved checkbox strings, emits supported toast
  effects for action feedback, and presents generated output in a multiline
  `Form.TextArea`. The current host has no read-only TextArea property: this is
  an editable preview; Copy always uses the stored engine result, as its hint says.
- Behavior, width parsing, failure wording, retries and stale-action rejection are
  documented in [v2/README.md](v2/README.md). The V2 notes about compiling into the
  host are historical: this package needs no host source changes.

### Download and install

[Native Rust 0.1.0 release](https://github.com/almatkai/text-rewrap/releases/tag/v0.1.0): [macOS Apple Silicon ZIP](https://github.com/almatkai/text-rewrap/releases/download/v0.1.0/text-rewrap-aarch64-apple-darwin.zip), [Windows x64 ZIP](https://github.com/almatkai/text-rewrap/releases/download/v0.1.0/text-rewrap-x86_64-pc-windows-msvc.zip), and [trusted install reference](https://github.com/almatkai/text-rewrap/releases/download/v0.1.0/extension-release.json).

Download the trusted JSON and your platform ZIP, then import them with:

```sh
tezbar --extensions-v3-import extension-release.json text-rewrap-<target>.zip
```

Both native builds, extracted-package protocol checks and Tezbar CLI imports into isolated profiles passed. [Validation and exact hashes](https://github.com/almatkai/text-rewrap/releases/download/v0.1.0/validation.json) record the tested source. Real desktop clipboard, selection and focus QA remains pending. Store discovery requires a Tezbar catalog update.

Build and verify (macOS arm64):

```sh
export CARGO_TARGET_DIR="$PWD/target"
cargo +1.85.0 test -j2 --locked               # real-pipe process tests, fake host
(cd v2 && CARGO_TARGET_DIR="$PWD/../target/v2" cargo test -j2 --all-features --locked)
cargo +1.85.0 build -j2 --release --locked --target aarch64-apple-darwin
python3 scripts/package.py --target aarch64-apple-darwin \
  --binary target/aarch64-apple-darwin/release/tezbar-text-rewrap
python3 -I scripts/smoke-package.py dist/text-rewrap-aarch64-apple-darwin.zip
# Optional host checkout argument is read-only; uses its installed Node dependencies.
node scripts/renderer-contract.cjs /path/to/Tezbar target/debug/tezbar-text-rewrap
```

Rust 1.85 is the declared minimum. The license inventory also rejects locked
release dependencies declaring a newer MSRV. `scripts/vendor-licenses.py` vendors
exact normal/build dependency notices for both published targets; rerun it after
lockfile changes. Packaging runs `--check` before writing an archive and fails on
missing or stale documents/inventory. The SDK LICENSE and NOTICE are included.

The renderer contract check runs real subprocess scenarios through the current
host's actual FormRuntime (server-rendered into a DOM) and notice-selection code,
checking checkbox state, wrapped lines, blank paragraphs, Copy failure and retry.
It performs no desktop or OS clipboard effects.

The ZIP holds `extension.json`, `bin/tezbar-text-rewrap[.exe]`, `resources/`,
`licenses/`, `LICENSE` and `NOTICE`, with a SHA-256 `files` inventory. The script
also writes archive and executable SHA-256 sidecars. Tests use recording host stubs
only. Real clipboard, selection and focus QA on macOS/Windows has **not** been run.

[Native V2 Rust port and integration guide](v2/README.md).
