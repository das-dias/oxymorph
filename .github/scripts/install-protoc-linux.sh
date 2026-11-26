#!/bin/sh
# Runs inside the maturin-action manylinux/musllinux containers, which do not see the
# protoc that arduino/setup-protoc puts on the host. Alpine (musllinux) ships protoc;
# the glibc containers get the upstream release for their own architecture, verified
# against SHA-256 sums pinned here (protobuf publishes none; computed from the v31.1
# release assets over HTTPS).
set -eu
if command -v apk >/dev/null 2>&1; then
    apk add --no-cache protoc
else
    case "$(uname -m)" in
        x86_64) arch=x86_64 sha256=96553041f1a91ea0efee963cb16f462f5985b4d65365f3907414c360044d8065 ;;
        aarch64) arch=aarch_64 sha256=6c554de11cea04c56ebf8e45b54434019b1cd85223d4bbd25c282425e306ecc2 ;;
        *) echo "unsupported architecture $(uname -m)" >&2; exit 1 ;;
    esac
    archive="/tmp/protoc-31.1-linux-${arch}.zip"
    curl -sSfL -o "$archive" "https://github.com/protocolbuffers/protobuf/releases/download/v31.1/protoc-31.1-linux-${arch}.zip"
    echo "${sha256}  ${archive}" | sha256sum -c -
    python3 - "$archive" <<'PY'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as archive:
    for name in archive.namelist():
        if name == "bin/protoc" or name.startswith("include/"):
            archive.extract(name, "/usr/local")
PY
    chmod +x /usr/local/bin/protoc
fi
protoc --version
