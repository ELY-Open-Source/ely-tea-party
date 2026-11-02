#!/bin/sh
# release-sums.sh: puts SHA256SUMS on this version's GitHub release, hashed from the three installers it holds, which the download store's mirror needs.
set -eu
cd "$(dirname "$0")/.."
version=$(cargo pkgid)
version=${version##*[#@]}
case $version in [0-9]*) ;; *) echo "release-sums: cargo pkgid names no version" >&2; exit 1 ;; esac
tag=v$version
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
set -- "ely-tea-party-$version-linux-x86_64.tar.gz" "ely-tea-party-$version-windows-x86_64.zip" "ely-tea-party-$version.dmg"
for file in "$@"; do
  gh release download "$tag" --pattern "$file" --dir "$work"
done
(cd "$work" && shasum -a 256 "$@" > SHA256SUMS)
scripts/release-asset.sh "$work/SHA256SUMS"
