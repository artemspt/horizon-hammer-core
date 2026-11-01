#!/usr/bin/env bash
# Builds every mod in mods/ and puts its .hmod in horizon-mods/, where the game looks.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cargo build --manifest-path "$root/Cargo.toml" -p cargo-horizon-mod
for manifest in "$root"/mods/*/horizon-mod.toml; do
    dir="$(dirname "$manifest")"
    built="$(cd "$dir" && "$root/target/debug/cargo-horizon-mod" horizon-mod build | sed -n 's/^Built //p')"
    cp "$built" "$root/horizon-mods/"
    echo "$(basename "$built")"
done
for manifest in "$root"/mods/*/horizon-board.toml; do
    dir="$(dirname "$manifest")"
    built="$(cd "$dir" && "$root/target/debug/cargo-horizon-mod" horizon-mod board build | sed -n 's/^Built //p')"
    mkdir -p "$root/horizon-boards"
    cp "$built" "$root/horizon-boards/"
    echo "$(basename "$built")"
done
