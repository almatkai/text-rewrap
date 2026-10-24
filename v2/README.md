# Text Rewrap — Native V2

A Rust port of rembridge's MIT Text Rewrap extension, pinned to Raycast
`6734d85992290066fd31a7f5bee3cca3d3545a1a`, `extensions/text-rewrap`.
The native host contract is Tezbar beta
`fd300e276356587726566026f072563eb9c34cb1`.

`extension-v2.json` declares `com.tezbar.native.text-rewrap`, replacing
`raycast.text-rewrap`. It preserves all three command names/titles/icons and
both preference definitions. macOS and Windows verification is **not-run**.
Compilation and contract tests do not establish desktop OS integration.

## Use and compatibility

- **Text Rewrap from Form** exposes Text, Width, and “Copy result to Clipboard?”.
- **Text Rewrap from Clipboard** and **Text Rewrap from Selection** expose Width
  and the same checkbox. They read their source only after Submit validates Width.
- “Rewrap Text” is primary. A truthy result adds “Rewrapped Text” as a
  `Form.Description` and the “Copy Rewrapped Text” action. Empty results expose
  neither. A checked checkbox still writes an empty result, as the original does.
  Unchecked never writes automatically; the explicit Copy action still copies.

Width defaults to the preference `defaultWidth || "80"`: **an empty preference
opens at 80**, despite the original preference description. Clear the submitted
Width to unwrap. `defaultCopyToClipboard` defaults to true and preserves false.
Preferences come exclusively from the launch payload. Drafts/results live in
session memory; this adapter performs no preference writes, persistent storage,
or V1 form-data migration.

Width validation follows JavaScript trim and default-radix `parseInt`. Thus
`80junk` and `80.5` mean 80, `1e3` means 1, and `0x10` means 16. Zero and negatives
are invalid. Extremely large integers can become positive Infinity: this is
valid and reports “Text re-wrapped at Infinity characters”. The internal width
uses an explicit Infinity variant, never an invalid JSON numeric value.

Paragraph processing follows the original `/\n\s*\n/` splitter, single-LF merge,
JavaScript whitespace collapse/trim, and greedy word wrapping. Length is UTF-16
units, so emoji and variation selectors count exactly as in JavaScript. Long
words remain unbroken. U+0085 is not JavaScript whitespace; BOM and line separators
are. CRLF uses the exact regex behavior. Unwrap still normalizes paragraphs and
whitespace. The pure utility returns text unchanged for width <= 0; forms reject
those widths. NaN utility parity is tested separately from accepted form widths.

## Native UI and failure adaptations

The beta `Form` renderer consumes `value` instead of React `defaultValue`.
Fields are submit-only, including Checkbox; there are no server-controlled
checkbox/dropdown actions in this extension. Field IDs are scoped to a launch
and stay stable across submit, refresh, validation, and copy replies. A relaunch
gets new IDs. Unchanged server defaults let beta reconciliation retain newer
client drafts even if they were typed while a response was queued. TextArea
keeps multiline input; Enter outside it submits, and Enter inside it inserts a
newline. The renderer harness also exercises actual controlled Checkbox and
Dropdown rejection semantics with synthetic fields, without adding such fields
to this extension.

Original toast titles/messages appear as honest in-form Status descriptions,
and invalid Width also has a field error. No toast/HUD effects or public RPC
are invented. Empty clipboard and unavailable/failed selection retain the
original error wording. Read responses must be strings. Clipboard transport or
malformed replies show a retryable read error. The previous output and current
submitted draft remain intact after a failed read or validation.

Result state is set before automatic copy, as in the original. Every copy path
validates the real beta reply `{ "ok": true }`; error/non-null, success/false or
malformed success envelopes are failures even alongside ok/true. A failed write
preserves output. After an automatic copy failure, submit the same inputs to retry that exact write without
rereading clipboard/selection. Changing inputs starts a new computation. For a
truthy result the Copy action retries manual or automatic failures using the stored result; an empty result
can retry through the same-input Submit. Copy payload text is ignored. Action
IDs rotate on accepted Submit and are scoped to the launch; old actions and
cross-command actions are rejected before effects. Dispose clears all state.

Selection uses **`core:platform:selected-text`, `[]`**, whose beta platform
handler returns a string. It never substitutes the clipboard. The host owns
foreground selection, accessibility permissions and focus handling; this
adapter does not hide the launcher or simulate focus. Selection and clipboard
text is never logged. Real selection/focus behavior remains untested.

The Copy action retains source `{modifiers:["cmd"],key:"c"}` metadata. This beta's
Form/index/action overlay does **not** bind arbitrary action shortcuts. Use the
Actions menu and select Copy Rewrapped Text. Meta+K opens Actions in the beta;
there is no verified Windows Ctrl+K/Ctrl+C mapping. No new keyboard binding is
claimed. Full native window rendering and real keyboard/focus QA remain not-run.

## Compiled-in integration

This is a source package, not an external Rust loader. Installing its JSON alone
does not execute Rust. Vendor this complete directory at the stable host path
`src-tauri/native-extensions/text-rewrap` and add to the host Cargo dependencies:

```toml
tezbar-text-rewrap = { path = "native-extensions/text-rewrap" }
```

The default library contains the production engine only. In
`src-tauri/src/native_core/extensions/mod.rs`, register the concrete unchanged
adapter using the host's real common/SDK modules:

```rust
mod v2_text_rewrap {
    use tezbar_text_rewrap::engine;
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/native-extensions/text-rewrap/src/tezbar_adapter.rs"));
}
```

Add `text-rewrap` to `v2_runtime::slug`, route `commands("text-rewrap")` to
`v2_text_rewrap::commands()`, and route `handle` to
`v2_text_rewrap::handle(ctx, command, operation, payload, state).await`.
Add the manifest object to the bundled `v2_catalog.json` and rebuild the host.
The beta SDK/common contract is fixed; host changes beyond that pin require
another compatibility check. The host already provides serde/serde_json.

Copy all three PNGs unchanged to
`src/renderer/public/extensions/icons/`:
`text-rewrap.png`, `text-rewrap-form.png`, `text-rewrap-clipboard.png`.
Manifest URLs start `/extensions/icons/`. Selection shares the clipboard icon.
The main PNG is 3,194 bytes, SHA-256
`605539b2bad407e75ad6fad90363bdaf7dddf65a8ac2d1f2ea3865d93869523f`.
`fixtures/icons.json` records all original icon hashes and pinned upstream URLs.

## Verification and reproduction

Rust 1.85 or later, no Node or host checkout required:

```sh
cargo test --all-features --locked
cargo fmt --check
rustfmt --edition 2021 --check src/tezbar_adapter.rs
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo +1.85.0 test --all-features --locked
cargo +1.85.0 clippy --all-targets --all-features --locked -- -D warnings
cargo check --target x86_64-pc-windows-gnu --all-targets --all-features --locked
cargo clippy --target x86_64-pc-windows-gnu --all-targets --all-features --locked -- -D warnings
cargo package --allow-dirty
```

`beta-contract` compiles the **unchanged production adapter** beside byte-exact
beta common/SDK fixtures. Only OS/event/router boundaries are stubbed; tests
perform no OS clipboard reads/writes or profile access. The five upstream
common/SDK tests use temporary fixture directories. Frozen original-TypeScript
oracles cover 533 validation/message cases, 13,581 text/width cases, 280 pure
utility cases, and nine component/preference scenarios. Adapter tests cover
all three commands, malformed read/write envelopes, failures/retries, empty
outputs, Infinity, false checkboxes, draft retention, and stale actions.

Optional regeneration and actual beta renderer/schema checks need Node and the
tool dependencies. Run from this directory:

```sh
pnpm --dir tools install --ignore-scripts
node tools/oracle.mjs
UPDATE_NATIVE_VIEWS=1 cargo test --all-features --test renderer_views
node tools/renderer.mjs
node tools/schema.mjs
node tools/node_modules/vitest/vitest.mjs run --root fixtures/original src/utils/rewrapText.test.ts src/utils/validation.test.ts
```

The oracle transpiles and executes original TS/TSX utilities/components with
mocked React/Raycast boundaries; expected wrapping is never reimplemented in JS.
Selection console output from the source is suppressed. The renderer check
executes the beta parser, reconciliation and component event/render functions
in a lightweight React/DOM boundary harness; it is not a browser screenshot or
OS test. The full pinned JSON Schema is evaluated with Ajv. Provenance/icon
hashes and license presence are also checked by Rust package tests.

A disposable V1 copy passes `pnpm test` (one metadata smoke test), typecheck,
and the 35 original source Vitest tests. V1 files and its root lockfile remain
unchanged. See `NOTICES`, `LICENSE` and `fixtures/beta/LICENSE` for MIT extension
and Apache-2.0 beta fixture terms.
