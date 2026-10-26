#!/usr/bin/env bash
# =============================================================================
# scripts/build-android.sh — 将 easygo-http 编译为 Android JNI 库并打包 APK
# =============================================================================
#
# 前置条件：
#   1. Rust 工具链已安装（rustup）
#   2. Android NDK 已安装（建议 r26 / r27）
#   3. Android SDK + Gradle Wrapper（通过 Android Studio 安装，或手动）
#   4. 【重要】sdk-android/ 目录中放置 EasyGo SDK 的 Android 版本 .so 文件
#       sdk-android/
#       ├── include/          (同 sdk/include，或从 sdk/ 软链)
#       └── lib/
#           ├── armeabi-v7a/  libpos_manager.so 及其依赖
#           └── arm64-v8a/    libpos_manager.so 及其依赖
#
# 用法：
#   export ANDROID_NDK_HOME=/path/to/ndk     # 例如 ~/Library/Android/sdk/ndk/27.x.x
#   export ANDROID_SDK_HOME=/path/to/sdk     # 例如 ~/Library/Android/sdk
#   bash scripts/build-android.sh
#
# 可选变量：
#   EASYGO_ANDROID_SDK_DIR   EasyGo Android SDK 路径（默认 <project>/sdk-android）
#   API_LEVEL                最低 API level（默认 26）
#   TARGETS                  逗号分隔的 Rust target（默认见下方）
#   BUILD_TYPE               debug | release（默认 debug）
# =============================================================================

set -euo pipefail

# ── 路径 ─────────────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ANDROID_DIR="$PROJECT_DIR/android"

# ── 配置 ─────────────────────────────────────────────────────────────────────
: "${ANDROID_NDK_HOME:?请设置 ANDROID_NDK_HOME 环境变量，例如: export ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/27.x.x}"
EASYGO_ANDROID_SDK_DIR="${EASYGO_ANDROID_SDK_DIR:-$PROJECT_DIR/sdk-android}"
API_LEVEL="${API_LEVEL:-26}"
BUILD_TYPE="${BUILD_TYPE:-debug}"

# 默认编译两个 ABI；如只需一个可覆盖：TARGETS=armv7-linux-androideabi
IFS=',' read -ra TARGETS <<< "${TARGETS:-armv7-linux-androideabi,aarch64-linux-android}"

# ── 颜色输出 ─────────────────────────────────────────────────────────────────
GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'
info()    { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error()   { echo -e "${RED}[ERROR]${NC} $*" >&2; exit 1; }

# ── 检查依赖 ─────────────────────────────────────────────────────────────────
echo ""
echo "============================================================"
echo "   EasyGo HTTP — Android Build"
echo "============================================================"
info "NDK 路径      : $ANDROID_NDK_HOME"
info "EasyGo SDK    : $EASYGO_ANDROID_SDK_DIR"
info "API Level     : $API_LEVEL"
info "Build Type    : $BUILD_TYPE"
info "Targets       : ${TARGETS[*]}"
echo ""

command -v cargo  >/dev/null 2>&1 || error "未找到 cargo，请安装 Rust: https://rustup.rs"
command -v rustup >/dev/null 2>&1 || error "未找到 rustup"

[[ -d "$ANDROID_NDK_HOME" ]] || error "ANDROID_NDK_HOME 不存在: $ANDROID_NDK_HOME"

# ── 检测 NDK 宿主平台 ─────────────────────────────────────────────────────────
HOST_OS="$(uname -s)"
case "$HOST_OS" in
    Darwin)  NDK_HOST="darwin-x86_64" ;;
    Linux)   NDK_HOST="linux-x86_64"  ;;
    *)       error "不支持的宿主平台: $HOST_OS" ;;
esac
NDK_TOOLCHAIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$NDK_HOST"
[[ -d "$NDK_TOOLCHAIN" ]] || error "NDK toolchain 未找到: $NDK_TOOLCHAIN"

# ── 检查 EasyGo Android SDK ──────────────────────────────────────────────────
if [[ ! -d "$EASYGO_ANDROID_SDK_DIR/include" ]]; then
    warn "未找到 $EASYGO_ANDROID_SDK_DIR/include"
    warn "将使用 sdk/include 作为头文件目录（自动软链）"
    mkdir -p "$EASYGO_ANDROID_SDK_DIR"
    if [[ ! -e "$EASYGO_ANDROID_SDK_DIR/include" ]]; then
        ln -sf "$PROJECT_DIR/sdk/include" "$EASYGO_ANDROID_SDK_DIR/include"
    fi
fi

# ── 为每个 target 编译 ────────────────────────────────────────────────────────
cd "$PROJECT_DIR"

for TARGET in "${TARGETS[@]}"; do
    echo ""
    echo "------------------------------------------------------------"
    info "编译 Rust target: $TARGET"
    echo "------------------------------------------------------------"

    # 确定 ABI 目录名 & NDK 三元组
    case "$TARGET" in
        armv7-linux-androideabi)
            ABI="armeabi-v7a"
            NDK_TRIPLE="armv7a-linux-androideabi${API_LEVEL}"
            ;;
        aarch64-linux-android)
            ABI="arm64-v8a"
            NDK_TRIPLE="aarch64-linux-android${API_LEVEL}"
            ;;
        i686-linux-android)
            ABI="x86"
            NDK_TRIPLE="i686-linux-android${API_LEVEL}"
            ;;
        x86_64-linux-android)
            ABI="x86_64"
            NDK_TRIPLE="x86_64-linux-android${API_LEVEL}"
            ;;
        *)
            warn "未知 target $TARGET，跳过"
            continue
            ;;
    esac

    # 安装 Rust target
    rustup target add "$TARGET" 2>/dev/null || true

    # NDK 工具路径
    CC_BIN="$NDK_TOOLCHAIN/bin/${NDK_TRIPLE}-clang"
    CXX_BIN="$NDK_TOOLCHAIN/bin/${NDK_TRIPLE}-clang++"
    AR_BIN="$NDK_TOOLCHAIN/bin/llvm-ar"

    [[ -f "$CC_BIN"  ]] || error "未找到 CC:  $CC_BIN"
    [[ -f "$CXX_BIN" ]] || error "未找到 CXX: $CXX_BIN"

    # EasyGo SDK 的 ABI 子目录（放置 Android 版 libpos_manager.so）
    ANDROID_SDK_LIB="$EASYGO_ANDROID_SDK_DIR/lib/$ABI"
    if [[ ! -d "$ANDROID_SDK_LIB" ]]; then
        warn "未找到 $ANDROID_SDK_LIB"
        warn "请将 EasyGo 提供的 Android $ABI 版 libpos_manager.so 放入该目录"
        warn "暂时跳过 $TARGET"
        continue
    fi

    # 设置 cc crate 使用的编译器环境变量
    TARGET_UPPER="${TARGET^^}"
    TARGET_ENV="${TARGET_UPPER//-/_}"

    export "CC_${TARGET_ENV}=$CC_BIN"
    export "CXX_${TARGET_ENV}=$CXX_BIN"
    export "AR_${TARGET_ENV}=$AR_BIN"
    export "CARGO_TARGET_${TARGET_ENV}_LINKER=$CC_BIN"
    export EASYGO_SDK_DIR="$EASYGO_ANDROID_SDK_DIR"

    # Cargo build flag
    CARGO_FLAGS=(--target "$TARGET" --lib)
    if [[ "$BUILD_TYPE" == "release" ]]; then
        CARGO_FLAGS+=(--release)
    fi

    cargo build "${CARGO_FLAGS[@]}"

    # 复制 .so 到 jniLibs
    JNI_DIR="$ANDROID_DIR/app/src/main/jniLibs/$ABI"
    mkdir -p "$JNI_DIR"

    SO_SRC="$PROJECT_DIR/target/$TARGET/${BUILD_TYPE}/libeasygo_http.so"
    [[ -f "$SO_SRC" ]] || error "未生成 .so: $SO_SRC"

    cp "$SO_SRC" "$JNI_DIR/libeasygo_http.so"
    info "→ $JNI_DIR/libeasygo_http.so"

    # 复制 EasyGo SDK .so（运行时依赖）
    find "$ANDROID_SDK_LIB" -maxdepth 1 -name "*.so" | while read -r f; do
        cp "$f" "$JNI_DIR/"
        info "→ $JNI_DIR/$(basename "$f")"
    done
done

# ── 初始化 Gradle Wrapper（若不存在）────────────────────────────────────────
cd "$ANDROID_DIR"
if [[ ! -f "gradlew" ]]; then
    warn "未找到 gradlew，尝试通过 gradle 生成..."
    if command -v gradle >/dev/null 2>&1; then
        gradle wrapper --gradle-version 8.6
    else
        error "未找到 gradlew 也未找到 gradle。\n请在 $ANDROID_DIR 中运行：\n  gradle wrapper  或者  用 Android Studio 打开项目"
    fi
fi

# ── 构建 APK ─────────────────────────────────────────────────────────────────
echo ""
echo "------------------------------------------------------------"
info "Gradle 构建 APK（$BUILD_TYPE）"
echo "------------------------------------------------------------"

if [[ "$BUILD_TYPE" == "release" ]]; then
    ./gradlew assembleRelease
    APK_PATH="$ANDROID_DIR/app/build/outputs/apk/release/app-release-unsigned.apk"
else
    ./gradlew assembleDebug
    APK_PATH="$ANDROID_DIR/app/build/outputs/apk/debug/app-debug.apk"
fi

echo ""
if [[ -f "$APK_PATH" ]]; then
    echo "============================================================"
    echo -e "${GREEN}✅  APK 构建成功${NC}"
    echo "   路径: $APK_PATH"
    echo ""
    echo "   安装到已连接设备:"
    echo "   adb install -r \"$APK_PATH\""
    echo ""
    echo "   查看 HTTP 服务日志:"
    echo "   adb logcat -s EasyGoService:D"
    echo "============================================================"
else
    error "APK 未生成，请检查 Gradle 输出"
fi
