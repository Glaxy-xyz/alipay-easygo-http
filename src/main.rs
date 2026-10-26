//! Linux 二进制入口 — 从环境变量读取配置后调用 `run_server`。
use easygo_http::{bounded_env, env_or, init_logging, run_server};
use std::time::Duration;

#[tokio::main]
async fn main() {
    init_logging();

    let addr: String = env_or("EASYGO_HTTP_ADDR", "127.0.0.1:8080".to_string());
    let queue_capacity = bounded_env("EASYGO_QUEUE_CAPACITY", 64, 1, 4096);
    let timeout_ms = {
        let ms: u64 = env_or("EASYGO_CALL_TIMEOUT_MS", 30_000);
        if ms == 0 {
            tracing::warn!("EASYGO_CALL_TIMEOUT_MS=0 is invalid; using 30000");
            30_000
        } else {
            ms
        }
    };
    let call_timeout = Duration::from_millis(timeout_ms);

    run_server(addr, queue_capacity, call_timeout).await;
}
