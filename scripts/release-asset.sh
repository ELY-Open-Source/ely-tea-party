#!/bin/sh
# release-asset.sh FILE|--draft: drafts this version's GitHub release if missing, then puts FILE on it in place of its namesake.
set -eu
cd "$(dirname "$0")/.."
file=$1
version=$(cargo pkgid)
version=${version##*[#@]}
case $version in [0-9]*) ;; *) echo "release-asset: cargo pkgid names no version" >&2; exit 1 ;; esac
tag=v$version
if ! gh release view "$tag" >/dev/null 2>&1; then
  gh release create "$tag" --draft --target main --title "Ely Tea Party $version" --notes ""
  echo "release-asset: drafted $tag"
fi
[ "$file" = --draft ] && exit 0
[ -f "$file" ] || { echo "release-asset: no file $file" >&2; exit 1; }
gh release upload "$tag" "$file" --clobber
echo "release-asset: $(basename "$file") on $tag"
