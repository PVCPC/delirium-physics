#!/bin/sh
# Delirium room for macOS and Linux. Double-click on a Mac; `sh delirium-room.command` on Linux.
cd "$(dirname "$0")" || exit 1
# On its own (downloaded from the release page): fetch this version's source beside it, then run the copy in there.
VERSION=0.1.6
if [ ! -f Cargo.toml ]; then
  if [ ! -f "delirium-physics-$VERSION/Cargo.toml" ]; then
    echo "Downloading Delirium Physics $VERSION source..."
    curl -sSfL "https://github.com/PVCPC/delirium-physics/archive/refs/tags/v$VERSION.tar.gz" | tar -xz || { echo "Download failed."; exit 1; }
  fi
  exec sh "delirium-physics-$VERSION/delirium-room.command" "$@"
fi
if [ "$(uname)" = Darwin ] && ! xcode-select -p >/dev/null 2>&1; then
  echo "Installing Apple's command line tools: click Install in the window that opens, then run this again."
  xcode-select --install
  exit 1
fi
PATH="$HOME/.cargo/bin:$PATH"
if ! command -v cargo >/dev/null; then
  echo "Installing Rust (into your home folder)..."
  curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal || exit 1
fi
echo "Building the room (the first time takes 5-10 minutes)..."
cargo build --release -p delirium-room --manifest-path Cargo.toml || exit 1
exec target/release/delirium-room "$@"
