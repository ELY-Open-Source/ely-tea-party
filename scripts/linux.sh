#!/bin/sh
# linux.sh ARGS: runs `cargo ARGS` on the working tree in a Linux container.
set -eu
repo=$(cd "$(dirname "$0")/.." && pwd)
docker build -q -t ely-tea-party-linux - >/dev/null <<'EOF'
FROM rust:1.95
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libwayland-dev libxkbcommon-dev \
    libxkbcommon-x11-dev libxcb1-dev libx11-xcb-dev libfontconfig-dev libfreetype-dev zsh fish xvfb \
 && rm -rf /var/lib/apt/lists/* \
 && rustup component add clippy rustfmt
# Root ignores the file modes the tests check, as in CI.
RUN useradd --create-home ci \
 && mkdir -p /src /target /usr/local/cargo/registry /usr/local/cargo/git \
 && chown ci /src /target /usr/local/cargo/registry /usr/local/cargo/git
USER ci
ENV CARGO_TARGET_DIR=/target CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
WORKDIR /src
EOF
# Streamed, not shared, and dated now: cargo neither reads a stale file nor skips a changed one.
git -C "$repo" ls-files -co --exclude-standard -z |
  COPYFILE_DISABLE=1 tar -C "$repo" --no-xattrs --null -T - -cf - |
  docker run -i --rm \
    -v ely-linux-target:/target \
    -v ely-linux-registry:/usr/local/cargo/registry \
    -v ely-linux-git:/usr/local/cargo/git \
    ely-tea-party-linux sh -c 'tar -xmf - -C /src && exec cargo "$@"' cargo "$@"
