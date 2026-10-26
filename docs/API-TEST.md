# easygo-http 接口测试说明

默认基址（按环境变量 `EASYGO_HTTP_ADDR`，Dev Container / 本地脚本常用 **8093**）：

```text
BASE=http://127.0.0.1:8093
```

请求头：有 JSON 体的接口使用 `Content-Type: application/json`。

---

## 通用响应

### SDK 业务接口（多数 POST/PUT/GET `/status` 等）

HTTP 状态码由 SDK 返回码映射（见 [src/codes.rs](../src/codes.rs)）。Body 形如：

```json
{
  "code": 1,
  "message": "SUCCESS",
  "data": { }
}
```

- `code == 1` 或 `2`（`WHITE_LIST_USER`）视为成功；此时可能有 `data`。
- 失败时通常无 `data`，`message` 为错误名（如 `NOT_INITIALIZED`）。

### 参数校验失败（HTTP 400）

JSON 语法错误、缺字段、类型不对也是同一形状（状态码沿用解析结果，缺字段通常是 422）：

```json
{
  "code": null,
  "message": "field xxx ..."
}
```

### 其他 HTTP 层错误

| HTTP | 场景 |
|------|------|
| 409 | 重复 `POST /init`（需先 `POST /reset`） |
| 503 | SDK 队列满 |
| 504 | SDK 调用超时（默认 30s，`EASYGO_CALL_TIMEOUT_MS`） |

### 探活

| 方法 | 路径 | Body | 响应 |
|------|------|------|------|
| GET | `/healthz` | 无 | 纯文本 `ok`（非 JSON） |

```bash
curl -sS "$BASE/healthz"
```

---

## 推荐测试顺序

1. `GET /healthz` → `GET /version`（无需 init）
2. `POST /init`（需证书与 MQTT 参数）
3. `GET /status`；可选 `GET /events` 或 `GET /events/latest` 收 config/price
4. `POST /verify` / `POST /orders/upload` 等业务
5. 结束或换环境：`POST /reset`

---

## 生命周期

### GET `/version`

无入参。返回 SDK 版本。

**响应示例**

```json
{
  "code": 1,
  "message": "SUCCESS",
  "data": { "version": "1.0.0.30" }
}
```

```bash
curl -sS "$BASE/version" | jq .
```

---

### POST `/init`

初始化 SDK（仅一次；重复返回 409）。证书路径为**进程工作目录下的相对或绝对路径**。

| 字段 | 类型 | 必填 | 长度/说明 |
|------|------|------|-----------|
| `url` | string | 是 | ≤255，MQTT 服务器地址 |
| `port` | string | 是 | 十进制端口，1–65535，如 `"1883"` |
| `device_no` | string | 是 | ≤64 |
| `vehicle_no` | string | 是 | ≤64 |
| `route_no` | string | 是 | ≤64 |
| `app_id` | string | 是 | ≤64 |
| `data_path` | string | 否 | 缺省或空白为 `"./"`；自动补 `/`，补完后 ≤1024。空字符串不会变成 `"/"` |
| `username` | string | 是 | ≤128 |
| `password` | string | 是 | ≤256 |
| `ca_file` | string | 是 | ≤1024，**证书文件路径**，不是 PEM 正文 |
| `cert_file` | string | 是 | ≤1024，**证书文件路径** |
| `key_file` | string | 是 | ≤1024，**私钥文件路径** |

勿把 `-----BEGIN CERTIFICATE-----` 整段贴进 JSON；超过 1024 字节会报 `ca_file must be at most 1024 bytes`。
| `gps` | object | 否 | 见下表 |

**`gps` 对象**（可选，init 前写入 GPS）

| 字段 | 类型 | 范围 |
|------|------|------|
| `longitude` | number | [-180, 180] |
| `latitude` | number | [-90, 90] |
| `velocity` | number | ≥ 0 |
| `azimuth` | number | [-360, 360] |

**请求示例**（字段可参考 [sdk/config.example.txt](../sdk/config.example.txt)；Dev Container 证书在 `sdk/certs/`）

```json
{
  "url": "47.75.201.251",
  "port": "1883",
  "device_no": "PM001",
  "vehicle_no": "ZY9876",
  "route_no": "4",
  "app_id": "1000",
  "data_path": "./",
  "username": "alipayHK",
  "password": "alipayHK2020!",
  "ca_file": "sdk/certs/ca.pem",
  "cert_file": "sdk/certs/client.pem",
  "key_file": "sdk/certs/client.key",
  "gps": {
    "longitude": 114.1694,
    "latitude": 22.3193,
    "velocity": 0,
    "azimuth": 0
  }
}
```

```bash
curl -sS -X POST "$BASE/init" \
  -H 'Content-Type: application/json' \
  -d @docs/examples/init.json
```

**成功响应**：`code: 1`，无 `data`。

---

### POST `/reset`

无 Body。重置 SDK；成功后允许再次 `POST /init`。

```bash
curl -sS -X POST "$BASE/reset"
```

---

### GET `/status`

无入参。调用 SDK `check_status`。

```bash
curl -sS "$BASE/status" | jq .
```

---

## 验码与订单

### POST `/verify`

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `qrcode` | string | 是 | 二维码内容，≤1024 字节（SDK 超过此长度返回 `ILLEGAL_PARAM`） |
| `price_cent` | int | 是 | 票价（分），≥ 0 |

**请求示例**

```json
{
  "qrcode": "<扫码得到的字符串>",
  "price_cent": 100
}
```

**成功 `data` 字段**

| 字段 | 说明 |
|------|------|
| `uid` | 用户 ID |
| `record` | 脱机记录 |
| `ticket_type` | 票种 |
| `ticket_type_raw_hex` | 票种前 8 字节 hex |
| `user_reserve_hex` | 用户扩展字段 hex，缓冲区 1024 字节 |
| `institution_reserve_hex` | 机构扩展字段 hex，缓冲区 1024 字节 |

```bash
curl -sS -X POST "$BASE/verify" \
  -H 'Content-Type: application/json' \
  -d '{"qrcode":"...","price_cent":100}' | jq .
```

---

### POST `/orders/upload`

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `tap_time` | int64 | 否 | 毫秒时间戳，默认当前时间 |
| `customer_no` | string | 是 | ≤64 |
| `ticket_type` | string | 是 | ≤16 |
| `offline_record` | string | 是 | ≤2047，通常来自 verify 的 `record` |
| `price_cent` | int | 是 | ≥ 0 |
| `route_no` | string | 是 | ≤64 |
| `bound` | bool 或 0/1 | 是 | 线路方向：true 上行，false 下行 |
| `stage_no` | int | 是 | 价格阶段号 |
| `journey_type` | int | 是 | 行程类型 |
| `order_number` | string | 否 | ≤24（SDK 头文件上限）；不传或 `null` 由 SDK 生成 |

**请求示例**

```json
{
  "customer_no": "user123",
  "ticket_type": "1",
  "offline_record": "<verify.record>",
  "price_cent": 100,
  "route_no": "4",
  "bound": false,
  "stage_no": 1,
  "journey_type": 0
}
```

**成功 `data`**

```json
{ "order_tid": "..." }
```

---

### POST `/orders/store`

离线订单写入外存。

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `path` | string | 是 | 设备路径，≤1024 |
| `file_name` | string | 是 | 文件名，≤255，不能含 `/` 或 `\` |

```json
{
  "path": "/mnt/usb/",
  "file_name": "orders.dat"
}
```

---

### DELETE `/orders/offline`

无 Body。删除离线订单数据。

```bash
curl -sS -X DELETE "$BASE/orders/offline"
```

---

## 设备状态

### PUT `/device/gps`

Body 同 init 中 `gps` 对象（四个 number，范围同上）。

```json
{
  "longitude": 114.17,
  "latitude": 22.32,
  "velocity": 10,
  "azimuth": 90
}
```

---

### PUT `/device/bound`

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `bound` | bool 或 0/1 | 是 | true 上行，false 下行 |

```json
{ "bound": true }
```

---

### PUT `/device/route`

| 字段 | 类型 | 必填 |
|------|------|------|
| `route_no` | string | 是，≤64 |

```json
{ "route_no": "4" }
```

---

### PUT `/device/power-protection`

| 字段 | 类型 | 必填 |
|------|------|------|
| `enabled` | bool 或 0/1 | 是 | 1 开启，0 关闭。开启成功后必须再 `POST /init` |

```json
{ "enabled": false }
```

---

## 固件与任务

### POST `/firmware/download`

异步下载；成功时 HTTP **202**，结果通过 SSE 事件 `download` 推送。

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `url` | string | 是 | ≤2048 |
| `path` | string | 是 | 本地保存路径，≤1024 |
| `task_no` | string | 否 | ≤128 |

```json
{
  "url": "https://example.com/firmware.bin",
  "path": "/tmp/firmware.bin",
  "task_no": "task-001"
}
```

**202 响应示例**

```json
{
  "code": 1,
  "message": "download started; the result is published as SSE event 'download'"
}
```

---

### POST `/tasks/:task_no/response`

路径参数：`task_no`（≤128）。Body：

| 字段 | 类型 | 必填 |
|------|------|------|
| `success` | bool 或 0/1 | 是 |

```bash
curl -sS -X POST "$BASE/tasks/task-001/response" \
  -H 'Content-Type: application/json' \
  -d '{"success": true}'
```

---

## 配置

### PUT `/config/features`

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `close_white_list` | bool 或 0/1 | 是 | 关闭白名单相关 |
| `close_price_list_message` | bool 或 0/1 | 是 | 关闭价目表消息 |
| `close_route_no_list_message` | bool 或 0/1 | 是 | 关闭线路列表消息 |

```json
{
  "close_white_list": false,
  "close_price_list_message": false,
  "close_route_no_list_message": false
}
```

---

### PUT `/config/institutions`

Body 为 **JSON 数组**（机构号：非空字符串或整数），序列化后总长 ≤4096。嵌套对象、空字符串、小数会返回 400。

```json
["1001", "1002"]
```

```bash
curl -sS -X PUT "$BASE/config/institutions" \
  -H 'Content-Type: application/json' \
  -d '["1001","1002"]'
```

---

## 事件（SSE）

### GET `/events`

Query：

| 参数 | 必填 | 说明 |
|------|------|------|
| `topic` | 否 | 只订阅：`config` / `firmware` / `price` / `download`。其它值返回 400 |

```bash
curl -N "$BASE/events"
curl -N "$BASE/events?topic=price"
```

事件名即 `topic`，`data` 为 JSON 字符串。订阅落后时会收到 event 名 `lagged`。

---

### GET `/events/latest`

无参数。返回各 topic 最近一次事件（JSON 对象，key 为 topic）。

```bash
curl -sS "$BASE/events/latest" | jq .
```

---

## SDK 返回码速查（`code` 字段）

| code | message（节选） |
|------|-----------------|
| 1 | SUCCESS |
| 2 | WHITE_LIST_USER |
| -1 ~ -35 | 见 [src/codes.rs](../src/codes.rs) 中 `name()` |
| -9 | NOT_INITIALIZED（未 init 或已 reset） |
| -20 | NETWORK_ERROR |

---

## 示例文件

Apifox / curl 使用 [docs/examples/init.json](examples/init.json)（与 [sdk/config.example.txt](../sdk/config.example.txt) 一致，证书为 Dev Container 内路径）。
