#!/bin/sh
# dpkg-shlibdeps reads the release ELF, not the build dependency list.
set -eu
cd "$(dirname "$0")/.."
. scripts/linux-common.sh
linux_init x86_64
linux_require dpkg-deb dpkg-shlibdeps desktop-file-validate strip
package=$work/debian/ely-tea-party
linux_stage "$package"
strip --strip-unneeded "$package/usr/bin/ely-tea-party" "$package/usr/lib/ely-tea-party/butler/node"
desktop-file-validate "$package/usr/share/applications/$id.desktop"
install -Dm644 packaging/linux/ely-tea-party.lintian-overrides \
    "$package/usr/share/lintian/overrides/ely-tea-party"
mkdir -p "$package/DEBIAN"
cat > "$work/debian/control" <<EOF
Source: ely-tea-party
Section: utils
Priority: optional
Maintainer: Ely Tea Party contributors <noreply@github.com>
Build-Depends: debhelper-compat (= 13)

Package: ely-tea-party
Architecture: amd64
Description: Terminal on GPUI and Ely
EOF
depends=$(cd "$work" && dpkg-shlibdeps -O -edebian/ely-tea-party/usr/bin/ely-tea-party -edebian/ely-tea-party/usr/lib/ely-tea-party/butler/node)
depends=${depends#shlibs:Depends=}
[ -n "$depends" ] || { echo "linux-deb: dpkg-shlibdeps returned no dependencies" >&2; exit 1; }
cat > "$work/changelog" <<EOF
ely-tea-party ($version) unstable; urgency=medium

  * Package the upstream release for Debian and Ubuntu.

 -- Ely Tea Party contributors <noreply@github.com>  $(date -R -u)
EOF
gzip -9 -n -c "$work/changelog" > "$package/usr/share/doc/ely-tea-party/changelog.gz"
size=$(du -sk "$package/usr" | cut -f1)
cat > "$package/DEBIAN/control" <<EOF
Package: ely-tea-party
Version: $version
Section: utils
Priority: optional
Architecture: amd64
Maintainer: Ely Tea Party contributors <noreply@github.com>
Installed-Size: $size
Depends: $depends, libwayland-client0, libwayland-egl1, libegl1, libvulkan1, git, curl, xdg-utils
Homepage: https://github.com/ELY-Open-Source/ely-tea-party
Description: Terminal on GPUI and Ely
 A GPU-rendered terminal with sessions, Git and file panels, shell integration
 and configurable appearance.
EOF
(cd "$package" && find usr -type f -exec md5sum {} +) > "$package/DEBIAN/md5sums"
dpkg-deb --root-owner-group --build "$package" "$out/ely-tea-party_${version}_amd64.deb"
