//! easygo-http 核心库。
//!
//! 同时支持：
//! * **Linux 二进制**（`[[bin]] easygo-http`，通过 `src/main.rs` 入口）
//! * **Android JNI 共享库**（`[lib] cdylib`，目标 `armv7-linux-androideabi` /
//!   `aarch64-linux-android`，由 `android/` 目录的 Kotlin 服务调用）

pub mod api;
pub mod codes;
pub mod error;
pub mod events;
pub mod ffi;
#[cfg(easygo_stub)]
pub mod ffi_stub;
pub mod sdk;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── 公共工具函数 ───────────────────────────────────────────────────────────────

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|d| i64::try_from(d.as_millis()).ok())
        .unwrap_or(0)
}

pub fn env_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

pub fn bounded_env(key: &str, default: usize, min: usize, max: usize) -> usize {
    let value = env_or(key, default);
    let used = value.clamp(min, max);
    if used != value {
        tracing::warn!(key, value, used, "env value clamped");
    }
    used
}

/// 初始化日志（允许多次调用：后续调用静默忽略）。
pub fn init_logging() {
    use tracing_subscriber::EnvFilter;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();
}

// ── HTTP 服务主体 ──────────────────────────────────────────────────────────────

/// 启动 HTTP 服务器并阻塞直到关闭信号（Linux/二进制模式）或连接关闭（Android）。
///
/// * `addr`           — 监听地址，例如 `"0.0.0.0:8093"`
/// * `queue_capacity` — SDK 调用队列容量（1–4096）
/// * `call_timeout`   — SDK 单次调用超时
pub async fn run_server(addr: String, queue_capacity: usize, call_timeout: Duration) {
    ffi::check_abi();

    #[cfg(easygo_stub)]
    tracing::warn!(
        "easygo_stub: 本地 stub 模式，无真实 EasyGo SDK；验码/上传等功能会返回错误。\
         请在 Dev Container（Linux armv7）中使用真实 SDK。"
    );

    let events = events::EventBus::install(256);
    let sdk = sdk::Sdk::start(queue_capacity, call_timeout);
    let app = api::router(api::AppState { sdk, events });

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("cannot bind {addr}: {e}"));
    tracing::info!(%addr, queue_capacity, ?call_timeout, "easygo-http listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = signal(SignalKind::terminate()).expect("cannot install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

// ── Android JNI ───────────────────────────────────────────────────────────────

#[cfg(target_os = "android")]
mod android_jni {
    use super::*;
    use jni::objects::{JClass, JString};
    use jni::sys::{jint, jlong};
    use jni::JNIEnv;

    /// JNI 入口：`NativeLib.startServer(addr, queueCapacity, timeoutMs)`
    ///
    /// 在独立 OS 线程中创建 Tokio runtime 并运行 HTTP 服务器。
    /// 调用后立即返回（非阻塞）。
    ///
    /// 返回值：`0` 成功，`-1` 地址字符串获取失败，`-2` 线程启动失败。
    #[no_mangle]
    pub extern "C" fn Java_com_easygo_http_NativeLib_startServer(
        mut env: JNIEnv,
        _class: JClass,
        addr: JString,
        queue_capacity: jint,
        timeout_ms: jlong,
    ) -> jint {
        let addr: String = match env.get_string(&addr) {
            Ok(s) => s.into(),
            Err(_) => return -1,
        };
        let queue_capacity = (queue_capacity as usize).clamp(1, 4096);
        let call_timeout = Duration::from_millis(timeout_ms.unsigned_abs());

        init_logging();

        std::thread::Builder::new()
            .name("easygo-server".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime");
                rt.block_on(run_server(addr, queue_capacity, call_timeout));
            })
            .map(|_| 0_i32)
            .unwrap_or(-2)
    }
}
