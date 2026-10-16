#!/bin/sh
# Delirium room for macOS and Linux. Double-click on a Mac; `sh delirium-room.command` on Linux.
cd "$(dirname "$0")" || exit 1
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
