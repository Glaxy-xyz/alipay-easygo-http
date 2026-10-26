# Android 真机与正式环境部署

当前 **没有** EasyGo 厂商提供的 Android（Bionic）版 `libpos_manager.so`。APK 的做法是：把现有 **Linux armhf** 的 `easygo-http` 和 SDK `.so` 打进安装包，用 `qemu-arm-static` 在 64 位 Android 上跑。

HTTP 接口与开发容器相同，默认监听 **`0.0.0.0:8093`**。接口说明见 [API-TEST.md](API-TEST.md)。

```
手机 App
  └── qemu-arm-static（内嵌）
        └── easygo-http + libpos_manager.so（Linux armhf）
              └── 监听 8093
```

---

## 1. 环境要求

### 手机 / 正式设备

| 项目 | 要求 |
|------|------|
| CPU | **64 位 ARM（arm64-v8a）**，不支持 32 位手机 |
| 系统 | Android 8.0（API 26）及以上 |
| 安装来源 | 允许「未知来源」或通过 `adb install` |
| 网络 | 能访问 EasyGo MQTT/业务地址（见 `POST /init`） |
| 权限 | 通知（前台服务）、网络；建议关闭电池优化 |

**不需要**在手机上单独安装 Termux、Debian、NDK、厂商 Android 库。这些都打进 APK 了。

### 打包机（出 APK 时才需要）

仅构建人员安装。现场装包的人只要拿到 APK。

| 软件 | 用途 | 说明 |
|------|------|------|
| Rust + `armv7-unknown-linux-gnueabihf` | 编 Linux 版 `easygo-http` | 与 Dev Container 同一套 |
| Android SDK Platform 34、Build-Tools 34 | Gradle 打 APK | 本仓库已用过的路径示例：`~/Library/Android/sdk` |
| NDK（可选） | 仅旧的 JNI stub 包需要 | 当前 Linux 内嵌方案 **不需要** |
| JDK 17 | Gradle | |
| Gradle 8.6 | 打 debug/release APK | |
| `qemu-arm-static`（Debian aarch64） | 打进 APK 的模拟器 | 静态 PIE，约 11MB |
| Debian armhf **glibc sysroot** | 给 Linux `.so` 提供 `libc.so.6` 等 | 约 5.5MB 压缩包 |
| OpenSSL **1.0.2** armhf | SDK 依赖 `libssl.so.1.0.2` | `libcrypto.so.1.0.2` 一并放入 |
| 本仓库 `sdk/lib/*.so` | EasyGo Linux 动态库 | 已有，不要换成别的 ABI |

仓库内辅助脚本：

- `scripts/build-android-apks.sh`：生成 **32 位** 和 **64 位** 两份 debug APK 到 `dist/`
- `scripts/prepare-android-linux-runtime.sh [arm32|arm64|all]`：分别打运行时 zip

---

## 2. 真机部署（现场安装）

### 2.1 安装 APK

把构建产物拷到手机，或 USB 调试：

按设备 CPU 选包（**不是**同一份 APK）：

| 设备 | APK |
|------|-----|
| 32 位 ARM（`armeabi-v7a`，如 XGD N96） | `dist/easygo-http-arm32-debug.apk` |
| 64 位 ARM（`arm64-v8a`，多数手机 / Genymotion arm64） | `dist/easygo-http-arm64-debug.apk` |

```bash
adb install -r dist/easygo-http-arm32-debug.apk   # 或 arm64
```

正式包：`gradle assembleArm32Release` / `assembleArm64Release`。

### 2.2 第一次打开

1. 打开 **EasyGo HTTP**。
2. Android 13+ 允许通知（前台服务保活）。
3. 应用会解压内嵌运行时（约十几秒），再启动 HTTP 服务。
4. 设置 → 应用 → EasyGo HTTP → **关闭电池优化**，避免灭屏后进程被杀。

### 2.3 确认服务

同一 Wi-Fi / 内网：

```bash
curl http://<手机IP>:8093/healthz
curl http://<手机IP>:8093/version
```

成功时 `/version` 为真实 SDK，例如：

```json
{"code":1,"data":{"version":"1.0.0.30"},"message":"SUCCESS"}
```

USB 调试时也可：

```bash
adb forward tcp:8093 tcp:8093
curl http://127.0.0.1:8093/version
```

日志：

```bash
adb logcat -s EasyGoService:I EasyGoService:E
```

出现 `easygo-http listening addr=0.0.0.0:8093` 即服务已起来。

### 2.4 证书与初始化

APK 里已经带上仓库的整个 `sdk/`（证书、头文件、动态库、空的 `data/`）。第一次打开 App 会解压到：

```text
/data/user/0/com.easygo.http/files/easygo-linux/sdk/
```

Linux 进程的工作目录是上一层 `easygo-linux/`。`POST /init` 用下面这组相对路径，每台设备都一样：

| 用途 | 相对路径 | 绝对路径 |
|------|----------|----------|
| 数据库目录 | `./sdk/data/` | `/data/user/0/com.easygo.http/files/easygo-linux/sdk/data/` |
| CA | `./sdk/certs/ca.pem` | `.../sdk/certs/ca.pem` |
| 客户端证书 | `./sdk/certs/client.pem` | `.../sdk/certs/client.pem` |
| 私钥 | `./sdk/certs/client.key` | `.../sdk/certs/client.key` |

```json
{
  "url": "<MQTT 主机>",
  "port": "1883",
  "device_no": "<设备号>",
  "vehicle_no": "<车辆号>",
  "route_no": "<线路>",
  "app_id": "1000",
  "data_path": "./sdk/data/",
  "username": "<账号>",
  "password": "<密码>",
  "ca_file": "./sdk/certs/ca.pem",
  "cert_file": "./sdk/certs/client.pem",
  "key_file": "./sdk/certs/client.key"
}
```

不要把开发容器里的 `/workspace/sdk/...` 原样贴到真机上。换了证书后需要重新打包 APK。字段含义见 `sdk/config.example.txt`。

初始化成功后再调 `/verify`、`/orders/upload` 等。

### 2.5 保活与重启

| 情况 | 处理 |
|------|------|
| 用户划掉任务 | 前台通知还在时一般还能活；被强杀需重新打开 App |
| 关机重启 | 需再次打开 App（当前未做开机自启） |
| 灭屏 | 已申请 `WAKE_LOCK`；仍建议关电池优化 |
| 换端口 | 界面「监听地址」改为 `0.0.0.0:<端口>` 后重新启动服务 |

---

## 3. 正式环境建议

### 能用当前 APK 的场景

- 内测、车载/手持 **arm64 Android** 设备
- 只有 Linux armhf 的 EasyGo 库
- 同一局域网或专网访问 `http://<设备IP>:8093`

### 上线前清单

1. 使用 **release 签名** 的 APK（不要长期用 debug 签名）。
2. 每台设备独立的 `device_no`、证书，不要共用开发证书。
3. 防火墙放行设备访问 MQTT `url:port`（示例环境为 `1883`）。
4. 业务机访问设备 **8093**（或你们改过的端口）；生产网不要把端口暴露到公网。
5. `data_path` 指向应用私有目录，保证可写（SQLite、离线单）。
6. 压测验码/上传：qemu 转译比原生慢，以真机实测为准。
7. 监控：`GET /status`、`GET /events`；进程消失则重新打开 App。

### 不适合作为「商店正式版」的原因

- 内嵌 GNU/Linux 的 `qemu-arm-static` + glibc，部分厂商 ROM 可能禁止执行这类 ELF。
- 性能、耗电、兼容性都不如原生 JNI。
- 杀后台策略因品牌而异，保活无法做到和系统服务一样稳。

**正式量产（公交 POS 等）仍应向 EasyGo 要 Android arm64（Bionic）版 `libpos_manager.so` 及依赖，再打 JNI 包。** 当前 APK 是「只有 Linux 库」时的可运行方案。

### 若已有 Android 版 `.so`（后续切换）

把厂商库放到：

```text
sdk-android/lib/arm64-v8a/libpos_manager.so
sdk-android/lib/arm64-v8a/<其余 .so>
```

头文件可继续用 `sdk/include`。然后用 `scripts/build-android.sh` 编 JNI 版，不再走 qemu。

---

## 4. 从源码打 APK（构建人员）

仓库里已有 Linux 交叉编译产物时：

```bash
bash scripts/build-android-apks.sh
```

产物：

```text
dist/easygo-http-arm32-debug.apk
dist/easygo-http-arm64-debug.apk
```

`qemu-arm-static`（仅 64 位包）与 glibc 压缩包默认路径：

```text
~/android-toolchain/apk-runtime/qemu-arm-static          # aarch64，给 arm64 包
~/android-toolchain/apk-runtime/qemu-arm-static-armhf    # 可选，32 位包默认不用 qemu
~/android-toolchain/apk-runtime/glibc-armhf.tgz
~/android-toolchain/bundle/lib/libssl.so.1.0.2
~/android-toolchain/bundle/lib/libcrypto.so.1.0.2
```

可用环境变量覆盖：`QEMU_ARM_STATIC_AARCH64`、`GLIBC_ARMHF_TGZ`、`OPENSSL_ARMHF_LIB`。

Linux 版 `easygo-http` 仍按原流程交叉编译（Dev Container / `armv7-unknown-linux-gnueabihf`），输出：

```text
target/armv7-unknown-linux-gnueabihf/release/easygo-http
```

---

## 5. 端口对照

| 地址 | 进程 |
|------|------|
| `8093` | 开发容器，以及真机 / 模拟器上的 32 位、64 位 APK |

正式环境只部署手机/设备上的 **8093**（或自定义端口），不依赖开发容器。

---

## 6. 故障

| 现象 | 处理 |
|------|------|
| `/version` 为 `stub-dev` | 装的是旧 JNI stub 包，换成当前内嵌 Linux 的 APK |
| 安装失败 `INSTALL_FAILED_NO_MATCHING_ABIS` | 设备不是 arm64 |
| 打开后无 `/healthz` | `adb logcat -s EasyGoService`；是否解压失败、qemu 无法执行 |
| `POST /init` 证书错误或 SQLITE_DB_ERROR | 用 `./sdk/certs/ca.pem`、`./sdk/certs/client.pem`、`./sdk/certs/client.key`、`./sdk/data/`，不要用 `/workspace/...` |
| 灭屏后接口不通 | 关电池优化，保持前台通知 |
| 验码极慢或超时 | qemu 开销；增大 `EASYGO_CALL_TIMEOUT` 无入口时只能改代码后重打 APK |
