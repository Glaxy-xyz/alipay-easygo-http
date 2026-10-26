# easygo-http

EasyGo POS SDK（armhf 1.0.0.30）的 HTTP 封装：Rust + axum，经 C++ shim 调用 SDK。

## 日常开发只用这一条（Mac / RustRover）

**实际在用的环境：`.devcontainer/`**（不是根目录 `Dockerfile`）。

| 项目 | 说明 |
|------|------|
| 镜像定义 | [`.devcontainer/Dockerfile`](.devcontainer/Dockerfile) — Rust 工具链 + 交叉编译 + QEMU，**不在镜像里编译**你的代码 |
| 启动 compose | [`.devcontainer/docker-compose.yml`](.devcontainer/docker-compose.yml) |
| 代码 | 挂载到容器 `/workspace`（`src/`、`Cargo.toml` 在容器里 `cargo build/run`） |
| 运行方式 | 交叉编译 **armhf** 二进制，用 **qemu-arm-static** 执行 |
| 访问地址 | **http://127.0.0.1:8093**（`/healthz`、`/version`） |

**RustRover：** Remote Development → Dev Containers → 打开本仓库。  
**终端：**

```bash
docker compose -f .devcontainer/docker-compose.yml up -d
docker compose -f .devcontainer/docker-compose.yml exec -d dev \
  env EASYGO_HTTP_ADDR=0.0.0.0:8093 EASYGO_CALL_TIMEOUT_MS=120000 \
  easygo-run-armhf /workspace/target/armv7-unknown-linux-gnueabihf/release/easygo-http
```

`POST /init` 示例 Body：[docs/examples/init.json](docs/examples/init.json)。

**Mac 本机 `cargo run`（8080）** 是 **stub**，无真实 SDK，仅改 HTTP 时用；**不算** POS 联调环境。

**发布镜像（与日常开发无关）：** 根目录 [`Dockerfile`](Dockerfile) 在镜像内 `cargo build`，用于 CI / 部署。

---

## 仓库结构

```
easygo-http/
├── sdk/                 EasyGo 官方 SDK（与实现同仓，便于提交与构建）
│   ├── include/         pos_*.h
│   ├── lib/             libpos_manager.so 及依赖 .so
│   ├── rsa_public_key.pem
│   ├── config.example.txt
│   ├── certs/           本地 TLS（不提交 git，见 .gitignore）
│   └── data/            运行时 pos_manager.db 等
├── shim/                C++ 适配层（map/list/string → JSON）
└── src/                 HTTP 服务
```

## 构建

Linux **armhf** 或交叉编译到 `armv7-unknown-linux-gnueabihf`：

```bash
export LD_LIBRARY_PATH="$PWD/sdk/lib:$LD_LIBRARY_PATH"
cargo build --release
```

仅检查 Rust 类型（Mac 上，不链接 SDK）：

```bash
cargo check
```

可选：`export EASYGO_SDK_DIR=/path/to/sdk`（需含 `include/` 与 `lib/` 子目录）。

## 运行

```bash
export LD_LIBRARY_PATH="$PWD/sdk/lib:$LD_LIBRARY_PATH"
export EASYGO_HTTP_ADDR=127.0.0.1:8080
./target/release/easygo-http
```

将 `sdk/certs/` 下放入 `ca.pem`、`client.pem`、`client.key`（勿提交仓库）。  
`POST /init` 中证书路径示例：`sdk/certs/ca.pem`，字段含义见 `sdk/config.example.txt`。

## HTTP 接口（摘要）

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | /init | 初始化 SDK |
| POST | /verify | 验码 |
| POST | /orders/upload | 上传订单 |
| GET | /status | 状态 |
| GET | /events | SSE 推送（config/firmware/price/download） |

完整路由见 `src/api.rs`。接口测试说明（含入参、curl 示例）见 [docs/API-TEST.md](docs/API-TEST.md)。

Android 真机 / 正式环境（内嵌 Linux SDK + qemu，无需厂商 Android 库）见 [docs/ANDROID-DEPLOY.md](docs/ANDROID-DEPLOY.md)。

## Docker

在**本仓库根目录**：

```bash
docker build -f Dockerfile -t easygo-http .
```

## 提交代码说明

- **会提交**：Rust/C++ 源码、`sdk/include/`、`sdk/lib/`、`rsa_public_key.pem`、`config.example.txt`
- **不提交**：`sdk/certs/*`、`sdk/data/*`、`target/`（见 `.gitignore`）

若公司规范不允许提交二进制 `.so`，可从仓库移除 `sdk/lib/` 并在 CI 中从 EasyGo 制品库拉取，本地开发仍保持相同目录结构即可。
