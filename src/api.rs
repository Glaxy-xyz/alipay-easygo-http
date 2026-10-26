use std::convert::Infallible;
use std::ffi::CString;
use std::sync::atomic::Ordering;

use axum::extract::{FromRequest, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{async_trait, Json, Router};
use serde::de::DeserializeOwned;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use serde_json::{json, Value};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};

use crate::error::ApiError;
use crate::events::EventBus;
use crate::sdk::Sdk;
use crate::{codes, ffi, now_ms};

#[derive(Clone)]
pub struct AppState {
    pub sdk: Sdk,
    pub events: &'static EventBus,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/init", post(init))
        .route("/reset", post(reset))
        .route("/status", get(status))
        .route("/version", get(version))
        .route("/verify", post(verify))
        .route("/orders/upload", post(upload_order))
        .route("/orders/store", post(store_offline_order))
        .route("/orders/offline", delete(delete_offline_order))
        .route("/device/gps", put(set_gps))
        .route("/device/bound", put(set_bound))
        .route("/device/route", put(set_route))
        .route("/device/power-protection", put(set_power_protection))
        .route("/firmware/download", post(download_file))
        .route("/tasks/:task_no/response", post(task_response))
        .route("/config/features", put(set_features))
        .route("/config/institutions", put(set_institutions))
        .route("/events", get(events_stream))
        .route("/events/latest", get(events_latest))
        .with_state(state)
}

type ApiResult = Result<Response, ApiError>;

/// JSON body extractor that keeps the `{code, message}` error shape.
struct JsonBody<T>(T);

#[async_trait]
impl<S, T> FromRequest<S> for JsonBody<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(JsonBody(value)),
            Err(err) => Err(ApiError::new(err.status(), err.body_text())),
        }
    }
}

fn sdk_reply(code: i32, data: Option<Value>) -> Response {
    let mut body = json!({ "code": code, "message": codes::name(code) });
    if let Some(data) = data {
        body["data"] = data;
    }
    (codes::http_status(code), Json(body)).into_response()
}

fn cstring(field: &str, value: &str, max_len: usize) -> Result<CString, ApiError> {
    if value.len() > max_len {
        return Err(ApiError::bad_request(format!(
            "{field} must be at most {max_len} bytes"
        )));
    }
    CString::new(value).map_err(|_| ApiError::bad_request(format!("{field} must not contain NUL")))
}

fn required(field: &str, value: &str, max_len: usize) -> Result<CString, ApiError> {
    if value.is_empty() {
        return Err(ApiError::bad_request(format!("{field} must not be empty")));
    }
    cstring(field, value, max_len)
}

/// Accepts `true`/`false` or `1`/`0`, since the SDK and demo use both.
fn bool_or_int<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Flag {
        Bool(bool),
        Int(i64),
    }
    match Flag::deserialize(d)? {
        Flag::Bool(b) => Ok(b),
        Flag::Int(0) => Ok(false),
        Flag::Int(1) => Ok(true),
        Flag::Int(n) => Err(D::Error::custom(format!("expected 0 or 1, got {n}"))),
    }
}

/// Decode a C string, never reading past `reported_len` or the first NUL.
fn c_buf_to_string(buf: &[u8], reported_len: i32) -> String {
    let n = clamp_len(reported_len, buf.len());
    let slice = &buf[..n];
    let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).into_owned()
}

fn c_str_buf(buf: &[u8]) -> String {
    c_buf_to_string(buf, i32::try_from(buf.len()).unwrap_or(i32::MAX))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn clamp_len(len: i32, cap: usize) -> usize {
    usize::try_from(len).unwrap_or(0).min(cap)
}

const DATA_PATH_MAX: usize = 1024;

/// Empty or whitespace becomes `"./"`. A missing trailing slash is added, and
/// that extra byte counts toward the limit so a 1024-byte path is not silently
/// turned into 1025.
fn normalize_data_path(raw: &str) -> Result<CString, ApiError> {
    let trimmed = raw.trim();
    let mut path = if trimmed.is_empty() {
        "./".to_string()
    } else {
        trimmed.to_string()
    };
    if !path.ends_with('/') {
        path.push('/');
    }
    if path.len() > DATA_PATH_MAX {
        return Err(ApiError::bad_request(format!(
            "data_path must be at most {DATA_PATH_MAX} bytes including the trailing '/'"
        )));
    }
    cstring("data_path", &path, DATA_PATH_MAX)
}

fn required_port(value: &str) -> Result<CString, ApiError> {
    if value.is_empty() || value.len() > 5 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ApiError::bad_request(
            "port must be an integer from 1 to 65535",
        ));
    }
    match value.parse::<u32>() {
        Ok(p) if (1..=65535).contains(&p) => cstring("port", value, 5),
        _ => Err(ApiError::bad_request(
            "port must be an integer from 1 to 65535",
        )),
    }
}

fn required_file_name(value: &str) -> Result<CString, ApiError> {
    if value.contains('/') || value.contains('\\') || value == "." || value == ".." {
        return Err(ApiError::bad_request(
            "file_name must be a file name, not a path",
        ));
    }
    required("file_name", value, 255)
}

fn institution_json(body: &Value) -> Result<CString, ApiError> {
    let Some(items) = body.as_array() else {
        return Err(ApiError::bad_request(
            "body must be a JSON array of institution numbers",
        ));
    };
    for (i, item) in items.iter().enumerate() {
        match item {
            Value::String(s) if !s.is_empty() && !s.contains('\0') => {}
            Value::Number(n) if n.as_i64().is_some() || n.as_u64().is_some() => {}
            _ => {
                return Err(ApiError::bad_request(format!(
                    "institutions[{i}] must be a non-empty string or an integer"
                )));
            }
        }
    }
    cstring("body", &body.to_string(), 4096)
}

fn is_event_topic(topic: &str) -> bool {
    matches!(topic, "config" | "firmware" | "price" | "download")
}

async fn healthz() -> &'static str {
    "ok"
}

// ── lifecycle ───────────────────────────────────────────────────────────────

#[derive(Deserialize, Clone, Copy)]
struct GpsRequest {
    longitude: f64,
    latitude: f64,
    velocity: f64,
    azimuth: f64,
}

impl GpsRequest {
    fn validate(&self) -> Result<(), ApiError> {
        let all_finite = [self.longitude, self.latitude, self.velocity, self.azimuth]
            .iter()
            .all(|v| v.is_finite());
        if !all_finite
            || !(-180.0..=180.0).contains(&self.longitude)
            || !(-90.0..=90.0).contains(&self.latitude)
            || self.velocity < 0.0
            || !(-360.0..=360.0).contains(&self.azimuth)
        {
            return Err(ApiError::bad_request(
                "gps out of range: longitude [-180,180], latitude [-90,90], velocity >= 0, azimuth [-360,360]",
            ));
        }
        Ok(())
    }
}

fn default_data_path() -> String {
    "./".into()
}

#[derive(Deserialize)]
struct InitRequest {
    url: String,
    port: String,
    device_no: String,
    vehicle_no: String,
    route_no: String,
    app_id: String,
    #[serde(default = "default_data_path")]
    data_path: String,
    username: String,
    password: String,
    ca_file: String,
    cert_file: String,
    key_file: String,
    gps: Option<GpsRequest>,
}

async fn init(State(st): State<AppState>, JsonBody(req): JsonBody<InitRequest>) -> ApiResult {
    if let Some(gps) = &req.gps {
        gps.validate()?;
    }
    let url = required("url", &req.url, 255)?;
    let port = required_port(&req.port)?;
    let device_no = required("device_no", &req.device_no, 64)?;
    let vehicle_no = required("vehicle_no", &req.vehicle_no, 64)?;
    let route_no = required("route_no", &req.route_no, 64)?;
    let app_id = required("app_id", &req.app_id, 64)?;
    let data_path = normalize_data_path(&req.data_path)?;
    let username = required("username", &req.username, 128)?;
    let password = required("password", &req.password, 256)?;
    let ca_file = required("ca_file", &req.ca_file, 1024)?;
    let cert_file = required("cert_file", &req.cert_file, 1024)?;
    let key_file = required("key_file", &req.key_file, 1024)?;
    let gps = req.gps;
    let initialized = st.sdk.initialized.clone();

    // The initialized check runs on the SDK thread so concurrent inits serialize.
    let outcome = st
        .sdk
        .call(move || {
            if initialized.load(Ordering::SeqCst) {
                return None;
            }
            if let Some(g) = gps {
                // SAFETY: plain values. NOT_INITIALIZED is ignored: this setter
                // runs before eg_init, and some builds report that until init.
                let gps_code =
                    unsafe { ffi::eg_set_gps_data(g.longitude, g.latitude, g.velocity, g.azimuth) };
                if gps_code != codes::SUCCESS && gps_code != codes::NOT_INITIALIZED {
                    return Some(gps_code);
                }
            }
            let cfg = ffi::InitConfig {
                url: url.as_ptr(),
                port: port.as_ptr(),
                device_no: device_no.as_ptr(),
                vehicle_no: vehicle_no.as_ptr(),
                route_no: route_no.as_ptr(),
                app_id: app_id.as_ptr(),
                data_path: data_path.as_ptr(),
                username: username.as_ptr(),
                password: password.as_ptr(),
                ca_file: ca_file.as_ptr(),
                cert_file: cert_file.as_ptr(),
                key_file: key_file.as_ptr(),
            };
            // SAFETY: all pointers come from CStrings alive until the end of this closure.
            let code = unsafe { ffi::eg_init(&cfg) };
            if code == codes::SUCCESS {
                initialized.store(true, Ordering::SeqCst);
            }
            Some(code)
        })
        .await?;

    match outcome {
        Some(code) => Ok(sdk_reply(code, None)),
        None => Err(ApiError::new(
            StatusCode::CONFLICT,
            "SDK already initialized; POST /reset first",
        )),
    }
}

async fn reset(State(st): State<AppState>) -> ApiResult {
    let initialized = st.sdk.initialized.clone();
    let code = st
        .sdk
        .call(move || {
            // SAFETY: no arguments.
            let code = unsafe { ffi::eg_reset() };
            if code == codes::SUCCESS {
                initialized.store(false, Ordering::SeqCst);
            }
            code
        })
        .await?;
    Ok(sdk_reply(code, None))
}

async fn status(State(st): State<AppState>) -> ApiResult {
    // SAFETY: no arguments.
    let code = st.sdk.call(|| unsafe { ffi::eg_check_status() }).await?;
    Ok(sdk_reply(code, None))
}

async fn version(State(st): State<AppState>) -> ApiResult {
    let (code, version) = st
        .sdk
        .call(|| {
            let mut buf = [0u8; 128];
            // SAFETY: buf is writable for its full length.
            let code = unsafe { ffi::eg_get_version(buf.as_mut_ptr().cast(), buf.len() as i32) };
            (code, c_str_buf(&buf))
        })
        .await?;
    Ok(sdk_reply(code, Some(json!({ "version": version }))))
}

// ── business ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct VerifyRequest {
    qrcode: String,
    price_cent: i32,
}

async fn verify(State(st): State<AppState>, JsonBody(req): JsonBody<VerifyRequest>) -> ApiResult {
    if req.price_cent < 0 {
        return Err(ApiError::bad_request("price_cent must be >= 0"));
    }
    let qrcode = required("qrcode", &req.qrcode, ffi::QRCODE_MAX)?;
    let price_cent = req.price_cent;

    let (code, r) = st
        .sdk
        .call(move || {
            let mut out = ffi::VerifyResult::zeroed();
            // SAFETY: qrcode is NUL-terminated; out is a valid, writable eg_verify_result.
            let code = unsafe { ffi::eg_verify(qrcode.as_ptr(), price_cent, &mut *out) };
            (code, out)
        })
        .await?;

    let data = codes::is_success(code).then(|| {
        json!({
            "uid": c_buf_to_string(&r.uid, r.uid_len),
            "record": c_buf_to_string(&r.record, r.record_len),
            "ticket_type": c_str_buf(&r.ticket_type),
            "ticket_type_raw_hex": hex(&r.ticket_type[..8]),
            "user_reserve_hex": hex(&r.user_reserve[..clamp_len(r.user_reserve_len, ffi::RESERVE_CAP)]),
            "institution_reserve_hex":
                hex(&r.institution_reserve[..clamp_len(r.institution_reserve_len, ffi::RESERVE_CAP)]),
        })
    });
    Ok(sdk_reply(code, data))
}

#[derive(Deserialize)]
struct UploadOrderRequest {
    /// Epoch milliseconds; defaults to now.
    tap_time: Option<i64>,
    customer_no: String,
    ticket_type: String,
    offline_record: String,
    price_cent: i32,
    route_no: String,
    #[serde(deserialize_with = "bool_or_int")]
    bound: bool,
    stage_no: i32,
    journey_type: i32,
    /// Header limit is 24 bytes. Omitted or null lets the SDK generate one.
    order_number: Option<String>,
}

async fn upload_order(
    State(st): State<AppState>,
    JsonBody(req): JsonBody<UploadOrderRequest>,
) -> ApiResult {
    if req.price_cent < 0 {
        return Err(ApiError::bad_request("price_cent must be >= 0"));
    }
    let customer_no = required("customer_no", &req.customer_no, 64)?;
    let ticket_type = required("ticket_type", &req.ticket_type, ffi::TICKET_TYPE_CAP)?;
    let offline_record = required("offline_record", &req.offline_record, ffi::RECORD_CAP - 1)?;
    let route_no = required("route_no", &req.route_no, 64)?;
    let order_number = req
        .order_number
        .as_deref()
        .map(|s| required("order_number", s, ffi::ORDER_NUMBER_MAX))
        .transpose()?;
    let tap_time = req.tap_time.unwrap_or_else(now_ms);
    if tap_time < 0 {
        return Err(ApiError::bad_request(
            "tap_time must be a unix millisecond timestamp >= 0",
        ));
    }
    let (price_cent, bound, stage_no, journey_type) = (
        req.price_cent,
        req.bound as i32,
        req.stage_no,
        req.journey_type,
    );

    let (code, order_tid) = st
        .sdk
        .call(move || {
            let order = ffi::Order {
                tap_time,
                customer_no: customer_no.as_ptr(),
                ticket_type: ticket_type.as_ptr(),
                offline_record: offline_record.as_ptr(),
                price_cent,
                route_no: route_no.as_ptr(),
                bound,
                stage_no,
                journey_type,
                order_number: order_number
                    .as_ref()
                    .map_or(std::ptr::null(), |s| s.as_ptr()),
            };
            let mut tid = [0u8; 129];
            // SAFETY: pointers come from CStrings alive in this closure; tid is writable.
            let code =
                unsafe { ffi::eg_upload_order(&order, tid.as_mut_ptr().cast(), tid.len() as i32) };
            (code, c_str_buf(&tid))
        })
        .await?;

    let data = codes::is_success(code).then(|| json!({ "order_tid": order_tid }));
    Ok(sdk_reply(code, data))
}

#[derive(Deserialize)]
struct StoreOrderRequest {
    path: String,
    file_name: String,
}

async fn store_offline_order(
    State(st): State<AppState>,
    JsonBody(req): JsonBody<StoreOrderRequest>,
) -> ApiResult {
    let path = required("path", &req.path, 1024)?;
    let file_name = required_file_name(&req.file_name)?;
    // SAFETY: both pointers are NUL-terminated CStrings owned by the closure.
    let code = st
        .sdk
        .call(move || unsafe { ffi::eg_store_offline_order(path.as_ptr(), file_name.as_ptr()) })
        .await?;
    Ok(sdk_reply(code, None))
}

async fn delete_offline_order(State(st): State<AppState>) -> ApiResult {
    // SAFETY: no arguments.
    let code = st
        .sdk
        .call(|| unsafe { ffi::eg_delete_offline_order() })
        .await?;
    Ok(sdk_reply(code, None))
}

// ── device state ────────────────────────────────────────────────────────────

async fn set_gps(State(st): State<AppState>, JsonBody(g): JsonBody<GpsRequest>) -> ApiResult {
    g.validate()?;
    // SAFETY: plain values.
    let code = st
        .sdk
        .call(move || unsafe {
            ffi::eg_set_gps_data(g.longitude, g.latitude, g.velocity, g.azimuth)
        })
        .await?;
    Ok(sdk_reply(code, None))
}

#[derive(Deserialize)]
struct BoundRequest {
    #[serde(deserialize_with = "bool_or_int")]
    bound: bool,
}

async fn set_bound(State(st): State<AppState>, JsonBody(req): JsonBody<BoundRequest>) -> ApiResult {
    let bound = req.bound as i32;
    // SAFETY: plain value.
    let code = st
        .sdk
        .call(move || unsafe { ffi::eg_set_bound(bound) })
        .await?;
    Ok(sdk_reply(code, None))
}

#[derive(Deserialize)]
struct RouteRequest {
    route_no: String,
}

async fn set_route(State(st): State<AppState>, JsonBody(req): JsonBody<RouteRequest>) -> ApiResult {
    let route_no = required("route_no", &req.route_no, 64)?;
    // SAFETY: NUL-terminated CString owned by the closure; the shim copies it.
    let code = st
        .sdk
        .call(move || unsafe { ffi::eg_set_route_no(route_no.as_ptr()) })
        .await?;
    Ok(sdk_reply(code, None))
}

#[derive(Deserialize)]
struct PowerProtectionRequest {
    #[serde(deserialize_with = "bool_or_int")]
    enabled: bool,
}

async fn set_power_protection(
    State(st): State<AppState>,
    JsonBody(req): JsonBody<PowerProtectionRequest>,
) -> ApiResult {
    let enabled = req.enabled as i32;
    let initialized = st.sdk.initialized.clone();
    // SAFETY: plain value. The interface doc requires a fresh init after protection is turned on.
    let code = st
        .sdk
        .call(move || {
            let code = unsafe { ffi::eg_set_power_off_protection(enabled) };
            if code == codes::SUCCESS && enabled != 0 {
                initialized.store(false, Ordering::SeqCst);
            }
            code
        })
        .await?;
    Ok(sdk_reply(code, None))
}

// ── firmware / tasks ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct DownloadRequest {
    url: String,
    path: String,
    task_no: Option<String>,
}

async fn download_file(
    State(st): State<AppState>,
    JsonBody(req): JsonBody<DownloadRequest>,
) -> ApiResult {
    let url = required("url", &req.url, 2048)?;
    let path = required("path", &req.path, 1024)?;
    let task_no = req
        .task_no
        .as_deref()
        .map(|s| required("task_no", s, 128))
        .transpose()?;
    let code = st
        .sdk
        .call(move || {
            let task_ptr = task_no.as_ref().map_or(std::ptr::null(), |s| s.as_ptr());
            // SAFETY: NUL-terminated CStrings (or null task_no); the shim copies them.
            unsafe { ffi::eg_download_file(url.as_ptr(), path.as_ptr(), task_ptr) }
        })
        .await?;
    if code != codes::SUCCESS {
        return Ok(sdk_reply(code, None));
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "code": code,
            "message": "download started; the result is published as SSE event 'download'",
        })),
    )
        .into_response())
}

#[derive(Deserialize)]
struct TaskResponseRequest {
    #[serde(deserialize_with = "bool_or_int")]
    success: bool,
}

async fn task_response(
    State(st): State<AppState>,
    Path(task_no): Path<String>,
    JsonBody(req): JsonBody<TaskResponseRequest>,
) -> ApiResult {
    let task_no = required("task_no", &task_no, 128)?;
    let success = req.success as i32;
    // SAFETY: NUL-terminated CString owned by the closure.
    let code = st
        .sdk
        .call(move || unsafe { ffi::eg_send_task_response(task_no.as_ptr(), success) })
        .await?;
    Ok(sdk_reply(code, None))
}

// ── config ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct FeaturesRequest {
    #[serde(deserialize_with = "bool_or_int")]
    close_white_list: bool,
    #[serde(deserialize_with = "bool_or_int")]
    close_price_list_message: bool,
    #[serde(deserialize_with = "bool_or_int")]
    close_route_no_list_message: bool,
}

async fn set_features(
    State(st): State<AppState>,
    JsonBody(req): JsonBody<FeaturesRequest>,
) -> ApiResult {
    let (w, p, r) = (
        req.close_white_list as u8,
        req.close_price_list_message as u8,
        req.close_route_no_list_message as u8,
    );
    // SAFETY: plain values.
    let code = st
        .sdk
        .call(move || unsafe { ffi::eg_set_feature_switch(w, p, r) })
        .await?;
    Ok(sdk_reply(code, None))
}

async fn set_institutions(
    State(st): State<AppState>,
    JsonBody(body): JsonBody<Value>,
) -> ApiResult {
    let json = institution_json(&body)?;
    // SAFETY: NUL-terminated CString owned by the closure.
    let code = st
        .sdk
        .call(move || unsafe { ffi::eg_set_qr_institution_no_array_json(json.as_ptr()) })
        .await?;
    Ok(sdk_reply(code, None))
}

// ── events ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct EventsQuery {
    topic: Option<String>,
}

async fn events_stream(
    State(st): State<AppState>,
    Query(q): Query<EventsQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    if let Some(topic) = q.topic.as_deref() {
        if !is_event_topic(topic) {
            return Err(ApiError::bad_request(
                "topic must be config, firmware, price, or download",
            ));
        }
    }
    let topic = q.topic;
    let stream = BroadcastStream::new(st.events.subscribe()).filter_map(move |msg| match msg {
        Ok(ev) if topic.as_deref().map_or(true, |t| t == ev.topic) => {
            Some(Ok(Event::default().event(ev.topic).data(ev.data)))
        }
        Ok(_) => None,
        Err(BroadcastStreamRecvError::Lagged(n)) => {
            Some(Ok(Event::default().event("lagged").data(n.to_string())))
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn events_latest(State(st): State<AppState>) -> Json<Value> {
    Json(json!(st.events.latest()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_data_path_is_dot_slash_not_root() {
        assert_eq!(normalize_data_path("").unwrap().to_str().unwrap(), "./");
        assert_eq!(normalize_data_path("   ").unwrap().to_str().unwrap(), "./");
    }

    #[test]
    fn data_path_slash_counts_toward_the_limit() {
        assert_eq!(
            normalize_data_path("sdk/data").unwrap().to_str().unwrap(),
            "sdk/data/"
        );
        assert!(normalize_data_path(&"a".repeat(1023)).is_ok());
        assert!(normalize_data_path(&"a".repeat(1024)).is_err());
        assert!(normalize_data_path(&format!("{}/", "a".repeat(1023))).is_ok());
    }

    #[test]
    fn port_must_be_in_range() {
        assert!(required_port("1883").is_ok());
        assert!(required_port("65535").is_ok());
        assert!(required_port("0").is_err());
        assert!(required_port("65536").is_err());
        assert!(required_port("abc").is_err());
        assert!(required_port("").is_err());
    }

    #[test]
    fn file_name_rejects_paths() {
        assert!(required_file_name("orders.dat").is_ok());
        assert!(required_file_name("../orders.dat").is_err());
        assert!(required_file_name("a/b").is_err());
        assert!(required_file_name(r"a\b").is_err());
        assert!(required_file_name(".").is_err());
        assert!(required_file_name("..").is_err());
        assert!(required_file_name("").is_err());
    }

    #[test]
    fn institutions_reject_nested_values() {
        let ok = institution_json(&json!(["1001", 1002])).unwrap();
        assert_eq!(ok.to_str().unwrap(), r#"["1001",1002]"#);
        assert!(institution_json(&json!({"no": "1001"})).is_err());
        assert!(institution_json(&json!(["", "1001"])).is_err());
        assert!(institution_json(&json!([1.5])).is_err());
        assert!(institution_json(&json!([["1001"]])).is_err());
    }

    #[test]
    fn qrcode_limit_matches_sdk() {
        let ok = "x".repeat(ffi::QRCODE_MAX);
        assert!(required("qrcode", &ok, ffi::QRCODE_MAX).is_ok());
        let too_long = "x".repeat(ffi::QRCODE_MAX + 1);
        assert!(required("qrcode", &too_long, ffi::QRCODE_MAX).is_err());
    }

    #[test]
    fn order_number_limit_matches_sdk_header() {
        let ok = "x".repeat(ffi::ORDER_NUMBER_MAX);
        assert!(required("order_number", &ok, ffi::ORDER_NUMBER_MAX).is_ok());
        let too_long = "x".repeat(ffi::ORDER_NUMBER_MAX + 1);
        assert!(required("order_number", &too_long, ffi::ORDER_NUMBER_MAX).is_err());
    }

    #[test]
    fn c_string_respects_reported_length() {
        let buf = b"abcdef\0zzzz";
        assert_eq!(c_buf_to_string(buf, 3), "abc");
        assert_eq!(c_buf_to_string(buf, 100), "abcdef");
        assert_eq!(c_buf_to_string(buf, -1), "");
    }

    #[test]
    fn bool_flag_accepts_zero_and_one() {
        #[derive(Deserialize)]
        struct Flag {
            #[serde(deserialize_with = "bool_or_int")]
            v: bool,
        }
        assert!(serde_json::from_str::<Flag>(r#"{"v":true}"#).unwrap().v);
        assert!(!serde_json::from_str::<Flag>(r#"{"v":0}"#).unwrap().v);
        assert!(serde_json::from_str::<Flag>(r#"{"v":1}"#).unwrap().v);
        assert!(serde_json::from_str::<Flag>(r#"{"v":2}"#).is_err());
    }

    #[test]
    fn azimuth_matches_interface_doc() {
        let gps = |azimuth: f64| GpsRequest {
            longitude: 114.0,
            latitude: 22.0,
            velocity: 0.0,
            azimuth,
        };
        assert!(gps(-360.0).validate().is_ok());
        assert!(gps(360.0).validate().is_ok());
        assert!(gps(-360.1).validate().is_err());
        assert!(gps(360.1).validate().is_err());
    }

    #[test]
    fn event_topic_is_closed() {
        assert!(is_event_topic("price"));
        assert!(!is_event_topic(""));
        assert!(!is_event_topic("Price"));
    }
}
