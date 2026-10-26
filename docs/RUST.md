# easygo-http Rust 代码说明

整个 crate 是一层 HTTP 外壳：axum 收 JSON，校验后交给单独的 SDK 线程，由它去调 C 接口。Rust 源文件一共 9 个，外加 `Cargo.toml` 和 `build.rs`。

## 整体怎么串起来

```text
src/main.rs          Linux 进程入口，#[tokio::main]
src/lib.rs           库入口：日志、读环境变量、启动 HTTP；Android 的 JNI 也在这里
src/api.rs           路由和每个接口的处理函数
src/sdk.rs           把阻塞的 SDK 调用放进一条专用线程
src/events.rs        SDK 回调 → 广播，给 SSE 用
src/ffi.rs           和 C 头文件对齐的结构体、函数声明
src/ffi_stub.rs      Mac/Windows 上的假 SDK，只为了本地能跑 HTTP
src/codes.rs         SDK 返回码 → 名字和 HTTP 状态码
src/error.rs         参数错误等，统一成 JSON 响应
build.rs             编译期：决定链真 SDK 还是编 stub
```

一次 `POST /verify` 的路径是：`main` 进 `run_server`，axum 调 `verify`，`verify` 调 `Sdk::call(...).await`。`call` 把闭包放进队列，`easygo-sdk` 线程里执行 `eg_verify`，结果经 `oneshot` 回到这个 `async fn`，再写成 JSON。

## 这份代码里反复出现的写法

| 写法 | 意思 |
|------|------|
| `fn 名字(...) -> T` | 普通函数，调用时立刻算完，返回 `T` |
| `async fn 名字(...) -> T` | 异步函数。编译后是一个 `Future`，要 `.await` 才会往下走。函数体里遇到 `.await` 会让出线程，等 I/O |
| `?` | 用在 `Result` 或 `Option` 上。失败就从当前函数返回这个错误，成功就取出里面的值 |
| `Result<T, E>` | 要么成功值 `T`，要么错误 `E`。`Ok(...)` / `Err(...)` |
| `Option<T>` | 要么有值 `Some(v)`，要么没有 `None` |
| `impl Trait` | 参数或返回值只要满足某个行为。`impl Into<String>` 表示传进来的东西能转成 `String` |
| `#[derive(...)]` | 编译器自动实现这些 trait。`Clone` 可复制，`Debug` 可打印，`Deserialize` 可从 JSON 解析 |
| `#[cfg(条件)]` | 条件编译。不满足时这段代码根本不存在于这次编译的产物里 |
| `move \|\| { ... }` | 闭包。`move` 表示把用到的变量所有权带走，闭包可以送到别的线程 |
| `unsafe { ... }` | 编译器不再替你检查这段内存是否安全。调用 `extern "C"` 必须写在 `unsafe` 里 |
| `&str` / `String` | `&str` 是借来的字符串切片；`String` 是自己拥有、可增长的字符串 |
| `*const T` / `*mut T` | C 风格裸指针。`const` 只读，`mut` 可写。Rust 不会自动管它们的生命周期 |

`#[tokio::main]` 是属性宏。它把 `async fn main` 展开成：创建多线程 Tokio 运行时，再 `block_on` 跑这个异步函数。进程会停在这里，直到服务器退出。

## `Cargo.toml`：包怎么声明

```toml
[package]
name = "easygo-http"
version = "0.1.0"
edition = "2021"

[lib]
name = "easygo_http"
crate-type = ["cdylib", "rlib"]

[[bin]]
name = "easygo-http"
path = "src/main.rs"
```

`edition = "2021"` 是语言版本，决定语法规则。`[lib]` 同时产出两种库：`cdylib` 是给 Android JNI 加载的 `.so`，`rlib` 是给同仓库的二进制链接用的 Rust 库。`[[bin]]` 是可执行文件，入口指定为 `src/main.rs`。

依赖里 `axum` 是 HTTP 框架，`tokio` 打开了多线程运行时、网络、同步、时间和信号。`jni` 只在 `target_os = "android"` 时才编译进去。`[profile.release]` 里 `lto = true` 表示发布构建做全程序链接优化。

## `build.rs`：编译之前先决定链什么

`build.rs` 在编译 `.rs` 之前由 Cargo 执行，本身是普通 Rust。它用 `println!("cargo:...")` 给编译器下指令。

- `cargo::rustc-check-cfg=cfg(easygo_stub)`：声明 `easygo_stub` 这个条件是合法的，避免编译器警告。
- `cargo:rerun-if-changed=...`：这些文件变了就重新跑 `build.rs`。
- 目标系统不是 Linux 也不是 Android 时，打印 `cargo:rustc-cfg=easygo_stub`。之后源码里的 `#[cfg(easygo_stub)]` 就会生效，链的是 `ffi_stub.rs`，不碰厂商 `.so`。
- Linux / Android 上调用 `cc::Build`：`.cpp(true).std("c++11")` 用 C++11 编译 `shim/easygo_shim.cpp`，再 `cargo:rustc-link-lib=dylib=pos_manager` 链接 `libpos_manager.so`。

## `src/main.rs`：Linux 进程入口

```rust
#[tokio::main]
async fn main() {
    init_logging();
    let addr: String = env_or("EASYGO_HTTP_ADDR", "127.0.0.1:8080".to_string());
    let queue_capacity = bounded_env("EASYGO_QUEUE_CAPACITY", 64, 1, 4096);
    // ...
    run_server(addr, queue_capacity, call_timeout).await;
}
```

`main` 只做三件事：初始化日志、从环境变量读监听地址、队列长度和超时，然后 `.await` 服务器。`let addr: String = ...` 里的 `: String` 是显式类型标注，因为 `env_or` 是泛型，编译器需要知道要解析成什么类型。超时为 `0` 时打一条警告并改回 30000 毫秒。

## `src/lib.rs`：库的公共入口

`pub mod api;` 这种语句把同目录的 `api.rs` 挂成子模块，并对外公开。文件开头的 `//!` 是模块文档，描述这个 crate 同时给 Linux 二进制和 Android JNI 用。

工具函数：

- `now_ms`：当前 Unix 毫秒。`and_then` 是「上一步有值才继续」；任何转换失败就 `unwrap_or(0)`。
- `env_or<T: FromStr>`：读环境变量并解析成 `T`。解析失败或变量不存在就用默认值。`FromStr` 是「能从字符串解析」这个约束。
- `bounded_env`：同样读一个整数，再用 `.clamp(min, max)` 夹在范围内。越界会打日志，但用的是夹完的值。
- `init_logging`：按 `RUST_LOG` 装 tracing。`try_init` 失败被 `let _ =` 丢掉，所以重复调用不会崩。

`run_server` 是真正的服务函数：

```rust
pub async fn run_server(addr: String, queue_capacity: usize, call_timeout: Duration) {
    ffi::check_abi();
    let events = events::EventBus::install(256);
    let sdk = sdk::Sdk::start(queue_capacity, call_timeout);
    let app = api::router(api::AppState { sdk, events });
    let listener = tokio::net::TcpListener::bind(&addr).await;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await;
}
```

`AppState { sdk, events }` 是结构体字面量。字段名和变量名相同时可以只写一次。`&addr` 是把 `String` 借出去，所有权还在 `addr` 上。`shutdown_signal` 在 Unix 上用 `tokio::select!` 同时等 Ctrl+C 和 SIGTERM，哪个先到就结束。`select!` 是宏，大括号里每个分支都是一个 `.await`。

文件后半段 `#[cfg(target_os = "android")]` 包着 JNI。`#[no_mangle] pub extern "C" fn Java_com_easygo_http_NativeLib_startServer` 是给 Kotlin 按 JNI 命名规则调用的 C 函数：不改编函数名，使用 C 调用约定。它取出 Java 字符串后，`std::thread::spawn` 起一个操作系统线程，线程里再建 Tokio 运行时并 `block_on(run_server(...))`。JNI 函数本身马上返回 `0`，服务器在后台跑。`-1` 是地址字符串拿不到，`-2` 是线程没创建成功。

## `src/error.rs`：错误怎么变成 HTTP 响应

`ApiError` 有两个私有字段：HTTP 状态码和消息。字段没写 `pub`，外面只能通过 `new` 和 `bad_request` 构造。`message: impl Into<String>` 表示调用方可以传 `&str` 或 `String`，函数内部 `.into()` 统一成 `String`。

`impl IntoResponse for ApiError` 是给 axum 的接口。处理函数返回 `Err(ApiError)` 时，axum 会调用 `into_response`，得到：

```json
{ "code": null, "message": "..." }
```

状态码来自 `self.status`，比如参数不合法是 400。`json!({ ... })` 是 `serde_json` 的宏，花括号里按 JSON 的样子写 Rust 表达式。

## `src/codes.rs`：SDK 整数码

SDK 用 `i32` 表示结果：`1` 成功，`2` 白名单用户也当成功，负数是各种失败。`pub const SUCCESS: i32 = 1` 是编译期常量。

- `is_success`：只有 `1` 和 `2` 算成功。后面组 JSON 的 `data` 字段只在成功时出现。
- `name`：`match code` 把数字变成 `"SUCCESS"` 这类名字。`_` 是兜底，未知码叫 `"UNKNOWN"`。
- `http_status`：同一批数字再映射成 HTTP 状态。参数错是 400，黑名单是 403，二维码本身有问题是 422，网络或上游问题是 502，没初始化是 503，其余是 500。

完整对照见 [src/codes.rs](../src/codes.rs)。接口测试里的响应形状见 [API-TEST.md](API-TEST.md)。

## `src/ffi.rs`：Rust 侧的 C 接口

这个文件不执行业务，只描述 C 函数长什么样，让 Rust 能链接。

`#[repr(C)]` 强制结构体按 C 的内存布局排列（字段顺序、对齐）。没有它，Rust 可能重排字段，传给 C 就会读错。`InitConfig` 里全是 `*const c_char`，也就是 C 的 `const char*`，指向以 `\0` 结尾的字符串。`VerifyResult` 里 `[u8; UID_CAP + 1]` 是定长数组，`+ 1` 给结尾的 `\0` 留位置。

`pub type EventCallback = unsafe extern "C" fn(...)` 是函数指针类型别名：一个不会展开 Rust panic、按 C 约定调用的函数。

两段声明互相排斥：

```rust
#[cfg(easygo_stub)]
pub use crate::ffi_stub::*;

#[cfg(not(easygo_stub))]
extern "C" {
    pub fn eg_init(cfg: *const InitConfig) -> c_int;
    // ...
}
```

`pub use ...::*` 把 stub 模块里所有公开项重新导出，调用方仍然写 `ffi::eg_init`。真 SDK 时，`extern "C" { fn eg_init(...) }` 只是声明，函数体在 C++ shim 里。这些函数默认不安全，所以调用处都要 `unsafe`。

`check_abi` 在启动时调用 C 侧的 `sizeof`，和 Rust 的 `size_of::<InitConfig>()` 比较。不一致就 `assert_eq!` 直接 panic，避免带错布局跑下去。`size_of::<T>()` 里的 `::<T>` 是 turbofish，用来在表达式里指定泛型参数。

`VerifyResult::zeroed` 返回 `Box<Self>`。`Box` 把这块大约 4KB 的结构体放在堆上。`Self` 在 `impl VerifyResult` 里面就是 `VerifyResult` 自己。

## `src/ffi_stub.rs`：没有 SDK 时的替身

Mac 上编译会进这个文件。每个 `eg_*` 都写成 `#[no_mangle] pub unsafe extern "C" fn`，函数名和真 shim 一致，所以上层不用改。

参数名前加 `_`，例如 `_cfg`，表示这个参数故意不用，编译器不警告。`eg_init` 只把原子布尔 `INIT` 设为 true 并返回成功。`eg_verify`、上传、GPS 等走 `stub_sdk_call`：还没 init 就返回「未初始化」，已经 init 则返回「系统错误」，表示这里没有真验码。`eg_get_version` 往调用方缓冲区写入 `"stub-dev ..."`。

`write_cstr` 按 C 字符串规则写：最多写 `cap - 1` 个字节，最后一定放 `\0`。`std::ptr::copy_nonoverlapping` 是按字节拷贝。文件底部 `#[cfg(test)] mod tests` 只在 `cargo test` 时编译，检查截断后仍然以 `\0` 结尾。

`static INIT: AtomicBool` 是整个进程一份的原子布尔。`Ordering::SeqCst` 是最强的内存顺序，保证一个线程 `store` 之后，另一个线程 `load` 能看到。

## `src/sdk.rs`：异步世界和阻塞 SDK 的交界

SDK 没有线程安全说明，而且 `upload_order` 会卡住等网络。所以所有调用进同一条名为 `easygo-sdk` 的操作系统线程。

```rust
type Job = Box<dyn FnOnce() + Send + 'static>;

pub async fn call<R, F>(&self, f: F) -> Result<R, ApiError>
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    let (reply_tx, reply_rx) = oneshot::channel();
    let job: Job = Box::new(move || {
        let _ = reply_tx.send(f());
    });
    self.tx.try_send(job)?;
    match tokio::time::timeout(self.timeout, reply_rx).await {
        // ...
    }
}
```

写法拆开看：

- `Box<dyn FnOnce() + Send + 'static>`：堆上的闭包。`FnOnce` 表示只能调用一次；`Send` 表示可以交给别的线程；`'static` 表示闭包里不借栈上的临时引用，该带走的数据都带走了。`dyn` 是动态分发，队列里可以装各种不同的闭包。
- `where R: Send + 'static` 是泛型约束，和写在尖括号里一样。`R` 是闭包的返回值类型，`F` 是闭包类型。
- `std::sync::mpsc::sync_channel(容量)` 是标准库的同步有界队列。`try_send` 队列满了立刻失败，HTTP 层返回 503，不会无限排队。
- `oneshot::channel()` 是 Tokio 的一次性通道：SDK 线程 `send` 一次结果，`async fn` 在 `reply_rx` 上 `.await`。
- `tokio::time::timeout(时长, future)` 给这次等待加期限。超时返回 504。超时只是 HTTP 不再等，那次 SDK 调用仍在线程里跑，队列会被它占住直到返回。
- `catch_unwind` 包住每个任务。SDK 或闭包如果 panic，线程继续处理下一个任务，这次调用的 `oneshot` 发送端被丢掉，等待方看到通道关闭，返回 500「SDK call panicked」。

`Sdk` 派生了 `Clone`。克隆只复制队列的发送端 `SyncSender` 和 `Arc<AtomicBool>`，不会再起一条线程。`Arc` 是共享所有权的引用计数，`initialized` 被多个请求拿去在 SDK 线程里读写。

`start` 末尾调用 `eg_set_event_callback(Some(events::on_sdk_event))`，把事件回调注册进 SDK。`Option<函数指针>` 里的 `Some` 表示有回调，`None` 表示空指针。

## `src/events.rs`：回调怎么变成 SSE

SDK 在它自己的线程里调用 `on_sdk_event`。这个函数是 `unsafe extern "C"`，必须马上返回，也不能让 Rust panic 穿回 C++。所以整段用 `catch_unwind` 包住。

它把两根 `*const c_char` 用 `CStr::from_ptr` 读成 Rust 字符串。`to_string_lossy` 遇到非法 UTF-8 会换成替代字符，再 `into_owned` 得到自己拥有的 `String`。然后 `EventBus::publish`。

`EventBus` 放在 `static BUS: OnceLock<EventBus>` 里。`OnceLock` 保证全局只初始化一次。`install` 用 `get_or_init`：第一次创建，之后再调就返回已有的那份。里面两样东西：

- `tokio::sync::broadcast`：一个发送端，多个订阅者各收一份。容量 256。没人订阅时 `send` 的错误被 `let _ =` 忽略。
- `Mutex<HashMap<...>>`：每个 topic（`config`、`firmware`、`price`、`download`）留最近一条，给后连上的客户端用。`lock()` 若碰到「上一持有者 panic 了」的毒锁，`unwrap_or_else(|e| e.into_inner())` 把锁里的数据取回来继续用。

`publish` 先尝试把字符串解析成 JSON。解析失败就存成 JSON 字符串，不把回调弄崩。

## `src/api.rs`：路由和每个接口

`router` 把路径绑到函数上，`.with_state(state)` 让每个处理函数都能拿到同一份 `AppState`。`get` / `post` / `put` / `delete` 是 HTTP 方法。

```rust
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/init", post(init))
        .route("/verify", post(verify))
        // ...
        .route("/tasks/:task_no/response", post(task_response))
        .with_state(state)
}
```

`/tasks/:task_no/response` 里的 `:task_no` 是路径参数，处理函数用 `Path(task_no)` 接出来。

`type ApiResult = Result<Response, ApiError>` 是别名。成功时是已经生成的 HTTP 响应，失败时是 `ApiError`。

`JsonBody<T>(T)` 是元组结构体，括号里那一个字段没有名字，用 `.0` 访问。它实现 `FromRequest`，也就是 axum 的「从请求里提取参数」。先用 axum 自带的 `Json<T>` 解析；失败时不走 axum 默认错误页，而是改成 `{code: null, message}`。`#[async_trait]` 是因为带泛型的 trait 方法在这里不能直接写 `async fn`，宏把它展开成返回 `Pin<Box<dyn Future>>`。

### 请求进来之前的校验

这些都是普通 `fn`，在 `.await` 之前做完：

- `cstring`：长度超过上限，或中间有 `\0`，就 400。`CString` 是保证以 `\0` 结尾、中间没有 `\0` 的字符串，`.as_ptr()` 才能交给 C。
- `required`：再加一条「不能是空串」。
- `bool_or_int`：自定义反序列化。`#[serde(untagged)]` 表示不看标签，先试 `bool`，再试整数。只接受 `true`/`false` 或 `1`/`0`。`#[serde(deserialize_with = "bool_or_int")]` 把这个函数挂到某个字段上。`'de` 是反序列化期间那次借用的生命周期。
- `normalize_data_path`：空白变成 `"./"`；没有结尾 `/` 就补上；补上的那一字节也算进 1024 的上限。
- `required_port`：必须全是数字，数值在 1 到 65535。
- `required_file_name`：不能含 `/`、`\`，也不能是 `.` 或 `..`。
- `institution_json`：body 必须是数组，元素是非空字符串或整数。`let Some(items) = body.as_array() else { return Err(...) }` 是「匹配失败就提前返回」。
- `c_buf_to_string`：从 C 写回来的缓冲区取字符串。长度以 SDK 报告的长度和缓冲区大小中较小的为准，再在第一个 `\0` 处截断。
- `hex`：把字节格式化成两位十六进制。`{b:02x}` 是格式化语法，`02` 表示至少两位、不足补零，`x` 表示小写十六进制。

### 每个处理函数

共同写法是：

```rust
async fn verify(
    State(st): State<AppState>,
    JsonBody(req): JsonBody<VerifyRequest>,
) -> ApiResult
```

`State(st)` 和 `JsonBody(req)` 是模式解构：axum 填好这两个提取器，函数直接拿到里面的值。业务数据先在异步任务里校验并变成 `CString`，再 `move` 进闭包。闭包在 SDK 线程执行 `unsafe { ffi::eg_... }`。`CString` 必须活到 C 函数返回，所以它们在闭包里面，不能在 `.await` 之前就丢掉。

| 函数 | 方法与路径 | 做什么 |
|------|------------|--------|
| `healthz` | `GET /healthz` | 固定返回文本 `ok`，不碰 SDK |
| `init` | `POST /init` | 校验 URL、端口、设备号、证书路径等，可选先写 GPS。在 SDK 线程里看 `initialized`，已经初始化就返回 `None`，HTTP 回 409。成功后把原子标志设为 true |
| `reset` | `POST /reset` | `eg_reset`，成功则把 `initialized` 清掉 |
| `status` | `GET /status` | `eg_check_status`，原样返回 SDK 码 |
| `version` | `GET /version` | 128 字节缓冲区接版本字符串，放进 `data.version` |
| `verify` | `POST /verify` | 票价不能为负，二维码最长 1024。成功时把 uid、record、票种和两段 reserve 的十六进制放进 `data` |
| `upload_order` | `POST /orders/upload` | 组 `Order`。`tap_time` 省略则用当前毫秒。`order_number` 省略则传空指针，让 SDK 自己生成。成功时返回 `order_tid` |
| `store_offline_order` | `POST /orders/store` | 把离线订单写到指定文件名 |
| `delete_offline_order` | `DELETE /orders/offline` | 删离线订单 |
| `set_gps` | `PUT /device/gps` | 经纬度、速度、方位角先做范围检查，再 `eg_set_gps_data` |
| `set_bound` | `PUT /device/bound` | `bool` 转成 `i32` 传给 SDK |
| `set_route` | `PUT /device/route` | 设置线路号 |
| `set_power_protection` | `PUT /device/power-protection` | 打开断电保护且 SDK 返回成功时，把 `initialized` 清掉。接口约定是打开保护后要重新 init |
| `download_file` | `POST /firmware/download` | 开始下载。SDK 返回成功时 HTTP 用 202，结果以后从 SSE 的 `download` 事件来 |
| `task_response` | `POST /tasks/:task_no/response` | 把任务号和是否成功回给 SDK |
| `set_features` | `PUT /config/features` | 三个开关转成 `u8` |
| `set_institutions` | `PUT /config/institutions` | 校验 JSON 数组后整段再传给 SDK |
| `events_stream` | `GET /events` | 订阅广播，滤出指定 topic，做成 SSE。落后太多时发一条 `lagged` 事件 |
| `events_latest` | `GET /events/latest` | 返回每个 topic 最近一次的 JSON，不进入 SDK 队列 |

`sdk_reply` 把 SDK 的整数码收成统一 JSON：`code`、`message`，成功且有附加数据时再加 `data`。HTTP 状态用 `codes::http_status`。

`.then(|| ...)` 用在 `bool` 上：条件为真才执行闭包并得到 `Some`，否则是 `None`。`upload_order` 里 `order_number.as_ref().map_or(std::ptr::null(), |s| s.as_ptr())` 的意思是：没有订单号就传 C 的空指针。

`events_stream` 的返回类型 `Sse<impl Stream<Item = ...>>` 表示「某种流」，具体类型由函数体推断，调用方只知道它是 `Stream`。`filter_map` 丢弃不匹配的 topic。`KeepAlive::default()` 让 SSE 连接定期发心跳。

文件末尾的 `#[cfg(test)]` 测的是路径规范化、端口、文件名、布尔解析和 ABI 大小这些纯逻辑，不连真 SDK。

## 读代码时可以按这个顺序

先看 [src/main.rs](../src/main.rs) 和 [src/lib.rs](../src/lib.rs) 的 `run_server`，确认进程怎么起来。再看 [src/api.rs](../src/api.rs) 的 `router` 和一个代表接口 `verify`。然后看 [src/sdk.rs](../src/sdk.rs) 的 `call`，理解 `.await` 实际等的是另一条线程。最后看 [src/ffi.rs](../src/ffi.rs) 的 `#[repr(C)]` 和 `extern "C"`，那是和 C++ shim 对齐的边界。
