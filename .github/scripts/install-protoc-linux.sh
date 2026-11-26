#!/bin/sh
# Runs inside the maturin-action manylinux/musllinux containers, which do not see the
# protoc that arduino/setup-protoc puts on the host. Alpine (musllinux) ships protoc;
# the glibc containers get the upstream release for their own architecture.
set -eu
if command -v apk >/dev/null 2>&1; then
    apk add --no-cache protoc
else
    arch=$(uname -m | sed 's/aarch64/aarch_64/')
    curl -sSfL -o /tmp/protoc.zip "https://github.com/protocolbuffers/protobuf/releases/download/v31.1/protoc-31.1-linux-${arch}.zip"
    python3 -m zipfile -e /tmp/protoc.zip /usr/local
    chmod +x /usr/local/bin/protoc
fi
protoc --version
