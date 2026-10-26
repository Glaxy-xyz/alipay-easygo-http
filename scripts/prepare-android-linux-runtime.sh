#!/usr/bin/env bash
# 生成 Android 内嵌 Linux armhf 运行时 zip。
# 用法: prepare-android-linux-runtime.sh [arm32|arm64|all]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STAGE_ROOT="${ANDROID_LINUX_STAGE:-$HOME/android-toolchain/apk-runtime/stage}"
QEMU64="${QEMU_ARM_STATIC_AARCH64:-$HOME/android-toolchain/apk-runtime/qemu-arm-static}"
QEMU32="${QEMU_ARM_STATIC_ARMHF:-$HOME/android-toolchain/apk-runtime/qemu-arm-static-armhf}"
GLIBC_TGZ="${GLIBC_ARMHF_TGZ:-$HOME/android-toolchain/apk-runtime/glibc-armhf.tgz}"
BIN="$ROOT/target/armv7-unknown-linux-gnueabihf/release/easygo-http"
SDK_ROOT="$ROOT/sdk"
OPENSSL_LIB="${OPENSSL_ARMHF_LIB:-$HOME/android-toolchain/bundle/lib}"
TARGET="${1:-all}"

[[ -f "$GLIBC_TGZ" ]] || { echo "缺少 glibc sysroot: $GLIBC_TGZ" >&2; exit 1; }
[[ -x "$BIN" ]] || { echo "缺少 easygo-http 二进制" >&2; exit 1; }
[[ -d "$SDK_ROOT/lib" ]] || { echo "缺少 $SDK_ROOT/lib" >&2; exit 1; }
for cert in ca.pem client.pem client.key; do
  [[ -f "$SDK_ROOT/certs/$cert" ]] || { echo "缺少 $SDK_ROOT/certs/$cert" >&2; exit 1; }
done

copy_named() {
  local src_dir="$1" dest_dir="$2"
  shift 2
  mkdir -p "$dest_dir"
  local name
  for name in "$@"; do
    if [[ -e "$src_dir/$name" ]]; then
      cp -L "$src_dir/$name" "$dest_dir/$name"
    else
      echo "缺少 $src_dir/$name" >&2
      exit 1
    fi
  done
}

patch_interp() {
  python3 - "$1" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
data = bytearray(p.read_bytes())
old = b"/lib/ld-linux-armhf.so.3"
new = b"./ld.so"
off = data.find(old)
if off < 0:
    raise SystemExit("easygo-http missing PT_INTERP /lib/ld-linux-armhf.so.3")
data[off:off + len(old)] = new.ljust(len(old), b"\0")
p.write_bytes(data)
print(f"patched interpreter -> ./ld.so @ {off}")
PY
}

pack_one() {
  local abi="$1"
  local stage="$STAGE_ROOT/$abi"
  local out="$ROOT/android/app/src/$abi/assets/easygo-linux.zip"

  rm -rf "$stage"
  mkdir -p "$stage/glibc"

  cp "$BIN" "$stage/easygo-http"
  chmod 755 "$stage/easygo-http"

  # 原始 sdk/ 整目录打进运行时：certs、include、lib、rsa 公钥、示例配置。
  # cp -L 把 SONAME 符号链接展开成实体文件，Android 解压不会还原符号链接。
  # 不打包本机已经生成的 pos_manager.db。
  mkdir -p "$stage/sdk"
  cp -RL "$SDK_ROOT/." "$stage/sdk/"
  find "$stage/sdk/data" -type f ! -name '.gitkeep' -delete
  mkdir -p "$stage/sdk/data"
  [[ -f "$stage/sdk/data/.gitkeep" ]] || : > "$stage/sdk/data/.gitkeep"

  if [[ -f "$OPENSSL_LIB/libssl.so.1.0.2" ]]; then
    cp -a "$OPENSSL_LIB/libssl.so.1.0.2" "$OPENSSL_LIB/libcrypto.so.1.0.2" "$stage/sdk/lib/"
  else
    echo "缺少 OpenSSL 1.0.2: $OPENSSL_LIB" >&2
    exit 1
  fi

  for so in libpos_manager.so libsqlite3.so.0 libprotobuf.so.15 libcurl.so.4 \
            libpaho-mqtt3cs.so libssl.so.1.0.2 libcrypto.so.1.0.2; do
    [[ -f "$stage/sdk/lib/$so" ]] || { echo "缺少 sdk/lib/$so" >&2; exit 1; }
  done

  local glibc_unpack="$stage/.glibc-src"
  mkdir -p "$glibc_unpack"
  tar -xzf "$GLIBC_TGZ" -C "$glibc_unpack"
  local libdir="$glibc_unpack/lib/arm-linux-gnueabihf"
  mkdir -p "$stage/glibc/lib/arm-linux-gnueabihf" "$stage/glibc/usr/lib/arm-linux-gnueabihf"
  copy_named "$libdir" "$stage/glibc/lib" ld-linux-armhf.so.3
  copy_named "$libdir" "$stage/glibc/lib/arm-linux-gnueabihf" \
    libc.so.6 \
    libm.so.6 \
    libpthread.so.0 \
    libdl.so.2 \
    librt.so.1 \
    libresolv.so.2 \
    libgcc_s.so.1 \
    libnss_dns.so.2 \
    libnss_files.so.2
  copy_named "$glibc_unpack/usr/lib/arm-linux-gnueabihf" \
    "$stage/glibc/usr/lib/arm-linux-gnueabihf" \
    libstdc++.so.6
  rm -rf "$glibc_unpack"

  local zip_args=(easygo-http sdk glibc)

  if [[ "$abi" == "arm64" ]]; then
    [[ -x "$QEMU64" ]] || { echo "缺少 64 位 qemu-arm-static: $QEMU64" >&2; exit 1; }
    cp "$QEMU64" "$stage/qemu-arm-static"
    chmod 755 "$stage/qemu-arm-static"
    zip_args+=(qemu-arm-static)
  else
    # 32 位 POS 无法跑 qemu（commpage / ART seccomp）。直接用 glibc ld.so。
    # 厂商策略禁止 ptrace，所以在 ld.so 入口接住 SIGSYS。
    cp -L "$stage/glibc/lib/ld-linux-armhf.so.3" "$stage/ld.so"
    chmod 755 "$stage/ld.so"
    python3 "$ROOT/scripts/patch-ld-sigsys.py" "$stage/ld.so"
    python3 "$ROOT/scripts/patch-ld-sigsys.py" --libc "$stage/glibc/lib/arm-linux-gnueabihf/libc.so.6"
    patch_interp "$stage/easygo-http"
    zip_args+=(ld.so)
  fi

  mkdir -p "$(dirname "$out")"
  rm -f "$out"
  (
    cd "$stage"
    zip -qr "$out" "${zip_args[@]}"
  )
  ls -lh "$out"
  echo "OK $abi $out"
}

case "$TARGET" in
  arm32|arm64) pack_one "$TARGET" ;;
  all)
    pack_one arm32
    pack_one arm64
    ;;
  *)
    echo "用法: $0 [arm32|arm64|all]" >&2
    exit 1
    ;;
esac
