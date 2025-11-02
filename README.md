# cargo-patch-source

A zero-fuss `cargo` subcommand that swaps dependencies to local or git sources and puts your `Cargo.toml` back the moment you’re done.

### How to use?

Let's say we have a local crate rattler that also has different versions than currently specified in the manifest.

```console
cargo patch-source apply --path ../rattler --pattern "rattler-*"
```

```diff
 [dependencies]
-rattler-one = "1.0.0"
-rattler-two = "2.0.0"
+rattler-one = "1.1.0"
+rattler-two = "2.2.0"

+[package.metadata.cargo-patch-source]
+original-versions = { rattler-one = "1.0.0", rattler-two = "2.0.0" }
+managed-patches = ["crates-io"]
+
+[patch.crates-io]
+rattler-one = { path = "../rattler/crates/rattler-one" }
+rattler-two = { path = "../rattler/crates/rattler-two" }
```

Run `cargo patch-source remove` when you want the manifest restored—original versions are reinstated and the patch section disappears.

## Install

```console
pixi global install --path .
cargo install --locked --path .
```

The subcommand is then available as `cargo patch-source`.

## Extra Usage

| Goal | Command |
| --- | --- |
| Use crates from a sibling workspace | `cargo patch-source apply --path ../workspace` |
| Sync just a subset (glob syntax) | `cargo patch-source apply --path ../workspace --pattern "rattler-*"` |
| Try a remote branch/tag/rev | `cargo patch-source apply --git https://github.com/org/repo --branch feature --pattern "crate-*"` |
| Target a different manifest | `cargo patch-source apply --path ../workspace --manifest-path other/Cargo.toml` |
| Undo all managed patches | `cargo patch-source remove [--manifest-path …]` |

Patterns accept `*` and `?`, are anchored to the crate name, and reuse the same glob helper for both local and git workflows.

## What It Tracks

- **Original versions** so `remove` can safely revert your dependency constraints.
- **Managed patch tables** so existing manual patches stay untouched.

Metadata is stored under `package.metadata.cargo-patch-source` (or `workspace.metadata…`) which Cargo ignores.

## Contributing & License

Pull requests are welcome. Released under the MIT license—see [LICENSE](LICENSE). Built on top of familiar crates like `clap`, `toml_edit`, `miette`, and friends.
