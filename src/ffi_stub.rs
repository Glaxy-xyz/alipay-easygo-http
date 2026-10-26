//! macOS/Windows 宿主机 stub：仅用于本地跑 HTTP；不链接 EasyGo SDK。

use std::mem::size_of;
use std::os::raw::{c_char, c_int};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::codes;

use crate::ffi::{EventCallback, InitConfig, Order, VerifyResult};

static INIT: AtomicBool = AtomicBool::new(false);
static EVENT_CB: Mutex<Option<EventCallback>> = Mutex::new(None);

fn not_ready() -> c_int {
    codes::NOT_INITIALIZED
}

#[no_mangle]
pub unsafe extern "C" fn eg_sizeof_init_config() -> usize {
    size_of::<InitConfig>()
}

#[no_mangle]
pub unsafe extern "C" fn eg_sizeof_verify_result() -> usize {
    size_of::<VerifyResult>()
}

#[no_mangle]
pub unsafe extern "C" fn eg_sizeof_order() -> usize {
    size_of::<Order>()
}

#[no_mangle]
pub unsafe extern "C" fn eg_set_event_callback(cb: Option<EventCallback>) {
    *EVENT_CB.lock().unwrap_or_else(|e| e.into_inner()) = cb;
}

#[no_mangle]
pub unsafe extern "C" fn eg_init(_cfg: *const InitConfig) -> c_int {
    INIT.store(true, Ordering::SeqCst);
    codes::SUCCESS
}

#[no_mangle]
pub unsafe extern "C" fn eg_reset() -> c_int {
    INIT.store(false, Ordering::SeqCst);
    codes::SUCCESS
}

#[no_mangle]
pub unsafe extern "C" fn eg_check_status() -> c_int {
    if INIT.load(Ordering::SeqCst) {
        codes::SUCCESS
    } else {
        not_ready()
    }
}

/// Always writes a trailing NUL when `cap > 0`, including when `src` is truncated.
unsafe fn write_cstr(dst: *mut c_char, cap: i32, src: &[u8]) -> bool {
    if dst.is_null() || cap <= 0 {
        return false;
    }
    let max = (cap as usize) - 1;
    let n = src
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(src.len())
        .min(max);
    std::ptr::copy_nonoverlapping(src.as_ptr(), dst as *mut u8, n);
    *dst.add(n) = 0;
    true
}

#[no_mangle]
pub unsafe extern "C" fn eg_get_version(buf: *mut c_char, cap: i32) -> c_int {
    if !write_cstr(buf, cap, b"stub-dev (use Dev Container for real SDK)") {
        return codes::POS_PARAM_ERROR;
    }
    codes::SUCCESS
}

#[no_mangle]
pub unsafe extern "C" fn eg_verify(
    _qrcode: *const c_char,
    _price_cent: i32,
    _out: *mut VerifyResult,
) -> c_int {
    stub_sdk_call()
}

#[no_mangle]
pub unsafe extern "C" fn eg_upload_order(
    _order: *const Order,
    order_tid: *mut c_char,
    order_tid_cap: i32,
) -> c_int {
    let code = stub_sdk_call();
    if code == codes::SUCCESS {
        let _ = write_cstr(order_tid, order_tid_cap, b"stub-order-tid");
    }
    code
}

#[no_mangle]
pub unsafe extern "C" fn eg_set_gps_data(_: f64, _: f64, _: f64, _: f64) -> c_int {
    stub_sdk_call()
}

#[no_mangle]
pub unsafe extern "C" fn eg_set_power_off_protection(_enable: i32) -> c_int {
    stub_sdk_call()
}

#[no_mangle]
pub unsafe extern "C" fn eg_set_route_no(_route_no: *const c_char) -> c_int {
    stub_sdk_call()
}

#[no_mangle]
pub unsafe extern "C" fn eg_set_bound(_bound: i32) -> c_int {
    stub_sdk_call()
}

#[no_mangle]
pub unsafe extern "C" fn eg_send_task_response(_task_no: *const c_char, _success: i32) -> c_int {
    stub_sdk_call()
}

#[no_mangle]
pub unsafe extern "C" fn eg_store_offline_order(
    _path: *const c_char,
    _file_name: *const c_char,
) -> c_int {
    stub_sdk_call()
}

#[no_mangle]
pub unsafe extern "C" fn eg_delete_offline_order() -> c_int {
    stub_sdk_call()
}

#[no_mangle]
pub unsafe extern "C" fn eg_download_file(
    _url: *const c_char,
    _path: *const c_char,
    _task_no: *const c_char,
) -> c_int {
    stub_sdk_call()
}

#[no_mangle]
pub unsafe extern "C" fn eg_set_feature_switch(_: u8, _: u8, _: u8) -> c_int {
    stub_sdk_call()
}

#[no_mangle]
pub unsafe extern "C" fn eg_set_qr_institution_no_array_json(_json: *const c_char) -> c_int {
    stub_sdk_call()
}

fn stub_sdk_call() -> c_int {
    if INIT.load(Ordering::SeqCst) {
        codes::SYSTEM_ERROR
    } else {
        not_ready()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_truncated_buffer_is_still_terminated() {
        let mut buf = [0xABu8; 4];
        let code = unsafe { eg_get_version(buf.as_mut_ptr().cast(), 4) };
        assert_eq!(code, codes::SUCCESS);
        assert_eq!(&buf, b"stu\0");
    }
}
