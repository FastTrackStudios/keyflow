# Keyflow for iOS

Two bundles ship together:

| bundle | what it is | id |
|---|---|---|
| **the app** | chart library + editor, Dioxus over the Rust crate | `app.fasttrackstudio.keyflow` |
| **the keyboard** | a system keyboard for Keyflow syntax, usable in any app | `app.fasttrackstudio.keyflow.keyboard` |

The keyboard is the reason this app exists. Writing a chart on a phone is
unpleasant today not because the editor is bad but because the *keyboard*
is: `|` is two planes deep, `♭` and `♯` are not on it at all, and a section
header is eight taps. Musician Keyboard solved the same problem for chord
symbols; this does it for Keyflow.

## Why the keyboard is Swift, and why its layout is not

An iOS custom keyboard is a `UIInputViewController` — an **app extension**,
which the system loads into the host app's process. It must be Swift or
Objective-C; there is no way to make it a Rust binary.

But the interesting part of a *language* keyboard is not the view. It is
knowing which tokens are legal here, what a tap should insert, and what to
offer next — knowledge the Keyflow parser already has. Duplicating that in
Swift would guarantee the keyboard and the language drift apart the first
time the grammar changes.

So the split is:

- **`src/keyboard.rs`** owns the layout, what each key inserts, and the
  contextual suggestions. It is tested against the real parser — the test
  `section_keys_insert_headers_the_parser_accepts` will fail if a key ever
  types something Keyflow cannot read.
- **`KeyflowKeyboard/`** (Swift) draws that layout and forwards taps to
  `UITextDocumentProxy`. It holds no vocabulary of its own.

The Rust side is reached through a small C ABI. `KeyboardPreview` in the
app renders the same layout through the same `apply` function, so the
preview and the shipped keyboard cannot disagree about what a key does.

## Constraints the extension imposes

These are why `keyboard.rs` looks the way it does, and they are not
negotiable:

- **No network.** Unless the user grants "Full Access", which most do not,
  and which should never be required for the keyboard to work.
- **A hard memory ceiling** — roughly 30–50 MB before the system kills the
  extension. This is why the keyboard links the layout and the parser, not
  Engraver: chart *rendering* stays in the app.
- **No document access.** `UITextDocumentProxy` exposes the text
  immediately before and after the caret, not the document. That is why
  `suggestions()` takes a fragment rather than a parsed `Chart`.
- **A separate sandbox.** The extension cannot see the app's documents
  directory. Charts are shared through an **App Group**
  (`group.app.fasttrackstudio.keyflow`), which both bundles declare as an
  entitlement; `library_path()` resolves to that container on device.

## Building

```bash
# On a Mac, inside the repo's nix dev shell.
cd apps/mobile && ./ios/build-ios.sh              # app only
cd apps/mobile && ./ios/build-ios.sh --sim <udid> # and install on a simulator
```

The env dance in `build-ios.sh` is required, not defensive: nixpkgs ships a
fake xcbuild `xcrun` whose SDK environment breaks Xcode's, so an iOS
cross-compile needs the real `xcrun` first on `PATH` and the nix SDK
variables unset.

## Status

The app half builds and runs. The Swift extension is **not yet written** —
`KeyflowKeyboard/` does not exist. What is done is the part that had to be
decided first: the layout, the key semantics, and the tests that keep them
honest against the parser. Writing the extension means adding an Xcode
target, the App Group entitlement on both bundles, and the C ABI shim over
`KeyboardLayout`.
