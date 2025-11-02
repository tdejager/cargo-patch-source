# cargo-patch-source

A Cargo extension to automatically apply dependency patch sections to `Cargo.toml` files. This tool is especially useful for local development workflows where you need to temporarily switch dependencies to local versions for testing and development.

## Features

-  **Apply patches from local workspaces** - Point to a local workspace and automatically patch all matching dependencies
-  **Apply patches from git repositories** - Patch dependencies from a specific git branch, tag, or revision
-  **Pattern filtering** - Use wildcards to selectively patch only specific crates
-  **Easy removal** - Clean removal of patches to restore original configuration
-  **Workspace support** - Works with both workspace and package `Cargo.toml` files
-  **Format preserving** - Uses `toml_edit` to maintain your TOML formatting and comments

## Installation

```bash
cargo install --path .
```

## Usage

### Apply Patches from a Local Workspace

Point to a local workspace directory and patch all matching dependencies:

```bash
cargo patch-source apply --path ../rattler
```

This will:
1. Discover all crates in the `../rattler` workspace
2. Find which of those crates are used in your current project
3. Add `[patch.crates-io]` entries pointing to the local paths
4. Store original versions for later restoration

### Apply Patches with Pattern Filtering

Use wildcards to only patch specific crates:

```bash
cargo patch-source apply --path ../rattler --pattern "rattler-*"
```

This will only patch crates whose names match the pattern `rattler-*`.

### Apply Patches from a Git Repository

Patch dependencies from a git repository:

```bash
# Use default branch
cargo patch-source apply --git https://github.com/prefix-dev/rattler --pattern "rattler-*"

# Use specific branch
cargo patch-source apply --git https://github.com/prefix-dev/rattler --branch main --pattern "rattler-*"

# Use specific tag
cargo patch-source apply --git https://github.com/prefix-dev/rattler --tag v1.0.0 --pattern "rattler-*"

# Use specific revision
cargo patch-source apply --git https://github.com/prefix-dev/rattler --rev abc123 --pattern "rattler-*"
```

**Note:** When using git sources, you must specify a `--pattern` to indicate which crates to patch.

### Remove Patches

Remove all managed patches:

```bash
cargo patch-source remove
```

This will:
1. Find all patch sections managed by `cargo-patch-source`
2. Remove them completely
3. Clean up the `[patch]` section if it's now empty

### Custom Manifest Path

Specify a custom `Cargo.toml` path:

```bash
cargo patch-source apply --path ../rattler --manifest-path path/to/Cargo.toml
cargo patch-source remove --manifest-path path/to/Cargo.toml
```

## How It Works

`cargo-patch-source` uses Cargo's `[patch]` section to temporarily override dependencies. It stores metadata using Cargo's official `metadata` convention (similar to `package.metadata.docs.rs`), which Cargo officially ignores.

### Before:

```toml
[dependencies]
rattler-one = "1.0.0"
rattler-two = "2.0.0"
```

### After applying patches:

```toml
[dependencies]
rattler-one = "1.0.0"
rattler-two = "2.0.0"

[package.metadata.cargo-patch-source]
original-versions = { rattler-one = "1.0.0", rattler-two = "2.0.0" }
managed-patches = ["crates-io"]

[patch.crates-io]
rattler-one = { path = "../rattler/crates/rattler-one" }
rattler-two = { path = "../rattler/crates/rattler-two" }
```

### After removing patches:

```toml
[dependencies]
rattler-one = "1.0.0"
rattler-two = "2.0.0"
```

The tool uses Cargo's metadata convention to track:
- **`original-versions`**: Original dependency versions for restoration
- **`managed-patches`**: Which patch sections are managed by the tool

This approach is idiomatic and officially supported by Cargo.

## Examples

### Example 1: Local Development Workflow

You're working on both a library and an application that uses it:

```bash
# Directory structure:
# ~/projects/
#   ├── my-app/
#   └── my-lib/

cd ~/projects/my-app

# Apply local patches for development
cargo patch-source apply --path ../my-lib

# Make changes to my-lib and test them immediately
cargo build

# When done, remove patches
cargo patch-source remove
```

### Example 2: Testing a Git Branch

Testing a specific branch of a dependency:

```bash
cargo patch-source apply \
  --git https://github.com/org/repo \
  --branch feature-branch \
  --pattern "crate-*"

# Run tests
cargo test

# Remove when done
cargo patch-source remove
```

### Example 3: Selective Patching

Only patch specific crates from a large workspace:

```bash
# Only patch rattler networking crates
cargo patch-source apply --path ../rattler --pattern "rattler-net*"

# Or only patch one specific crate
cargo patch-source apply --path ../rattler --pattern "rattler-digest"
```

## Pattern Syntax

Patterns support basic wildcards:
- `*` - Matches any characters
- `?` - Matches a single character
- Patterns are anchored (must match the entire crate name)

Examples:
- `rattler-*` - Matches `rattler-one`, `rattler-two`, etc.
- `*-sys` - Matches any crate ending with `-sys`
- `exact-name` - Matches only `exact-name`

## Development

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Running Locally

```bash
cargo run -- patch-source apply --help
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Acknowledgments

- Inspired by the [pixi](https://github.com/prefix-dev/pixi) project's `local_patch.py`
- Built with [clap](https://github.com/clap-rs/clap), [toml_edit](https://github.com/ordian/toml_edit), and [miette](https://github.com/zkat/miette)
