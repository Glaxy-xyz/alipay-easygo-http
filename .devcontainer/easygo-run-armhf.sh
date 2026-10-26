#!/usr/bin/env bash
set -euo pipefail
export LD_LIBRARY_PATH="/opt/easygo/extra-lib:/workspace/sdk/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec qemu-arm-static -L /usr/arm-linux-gnueabihf "$@"
