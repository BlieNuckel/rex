#!/bin/sh
set -eu

version=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
latest=$(git ls-remote --tags --refs origin 'v*' | sed 's|.*refs/tags/v||' | sort -V | tail -1)
newest=$(printf '%s\n' "$latest" "$version" | sort -V | tail -1)

if [ "$version" != "$latest" ] && [ "$newest" = "$version" ]; then
  release=true
else
  release=false
fi

echo "Cargo.toml $version, latest tag ${latest:-none}, release: $release" >&2
printf 'version=%s\nrelease=%s\n' "$version" "$release" >>"${GITHUB_OUTPUT:-/dev/stdout}"
