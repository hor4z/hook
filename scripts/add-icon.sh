#!/bin/sh
set -eu
src="${SHERPA:-../sherpa}/crates/sherpa-text/icons"
dst="$(dirname "$0")/../crates/koon-ui/icons"
for name in "$@"; do
  if [ ! -f "$src/$name.svg" ]; then
    echo "icon not found: $src/$name.svg" >&2
    exit 1
  fi
  cp "$src/$name.svg" "$dst/$name.svg"
  echo "added $name"
done
