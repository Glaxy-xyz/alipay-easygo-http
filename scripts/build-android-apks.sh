#!/usr/bin/env bash
# 生成 32 位 / 64 位两份 debug APK 到 dist/
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export JAVA_HOME="${JAVA_HOME:-$HOME/android-toolchain/jdk17/jdk-17.0.20.1+1/Contents/Home}"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export PATH="$JAVA_HOME/bin:${GRADLE_HOME:-$HOME/android-toolchain/gradle/gradle-8.6}/bin:$PATH"

"$ROOT/scripts/prepare-android-linux-runtime.sh" all
cd "$ROOT/android"
gradle assembleArm32Debug assembleArm64Debug --no-daemon

mkdir -p "$ROOT/dist"
cp "$ROOT/android/app/build/outputs/apk/arm32/debug/easygo-http-arm32-debug.apk" "$ROOT/dist/"
cp "$ROOT/android/app/build/outputs/apk/arm64/debug/easygo-http-arm64-debug.apk" "$ROOT/dist/"
ls -lh "$ROOT/dist"/easygo-http-arm*-debug.apk
echo "OK dist/easygo-http-arm32-debug.apk  dist/easygo-http-arm64-debug.apk"
