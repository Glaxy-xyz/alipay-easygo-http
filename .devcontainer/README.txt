RustRover / 日常开发（唯一推荐环境）

1. Remote Development → Dev Containers → 打开本仓库
2. 容器内：cargo run --release（或 Run main.rs），访问 http://127.0.0.1:8093
3. POST /init Body：docs/examples/init.json

Mac 宿主机 cargo run 为 stub，无真实 SDK。
