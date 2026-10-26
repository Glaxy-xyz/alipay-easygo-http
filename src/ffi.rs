//! Declarations for `shim/easygo_shim.h`. Layouts must match the C header;
//! `check_abi` verifies the struct sizes at startup.

use std::mem::size_of;
use std::os::raw::c_char;
#[cfg(not(easygo_stub))]
use std::os::raw::c_int;

pub const UID_CAP: usize = 17;
pub const RECORD_CAP: usize = 2048;
/// Interface doc v1.0.0.9: reserve buffers are at least 1024 bytes.
pub const RESERVE_CAP: usize = 1024;
pub const TICKET_TYPE_CAP: usize = 16;
/// `verify` in libpos_manager.so rejects strlen(qrcode) > 1024.
pub const QRCODE_MAX: usize = 1024;
/// `ORDER_INFO.order_number` in pos_manager_api.h: 最大长度 24.
pub const ORDER_NUMBER_MAX: usize = 24;

#[repr(C)]
pub struct InitConfig {
    pub url: *const c_char,
    pub port: *const c_char,
    pub device_no: *const c_char,
    pub vehicle_no: *const c_char,
    pub route_no: *const c_char,
    pub app_id: *const c_char,
    pub data_path: *const c_char,
    pub username: *const c_char,
    pub password: *const c_char,
    pub ca_file: *const c_char,
    pub cert_file: *const c_char,
    pub key_file: *const c_char,
}

#[repr(C)]
pub struct VerifyResult {
    pub uid_len: i32,
    pub record_len: i32,
    pub user_reserve_len: i32,
    pub institution_reserve_len: i32,
    pub uid: [u8; UID_CAP + 1],
    pub record: [u8; RECORD_CAP + 1],
    pub user_reserve: [u8; RESERVE_CAP],
    pub institution_reserve: [u8; RESERVE_CAP],
    pub ticket_type: [u8; TICKET_TYPE_CAP],
}

impl VerifyResult {
    pub fn zeroed() -> Box<Self> {
        Box::new(Self {
            uid_len: 0,
            record_len: 0,
            user_reserve_len: 0,
            institution_reserve_len: 0,
            uid: [0; UID_CAP + 1],
            record: [0; RECORD_CAP + 1],
            user_reserve: [0; RESERVE_CAP],
            institution_reserve: [0; RESERVE_CAP],
            ticket_type: [0; TICKET_TYPE_CAP],
        })
    }
}

#[repr(C)]
pub struct Order {
    pub tap_time: i64,
    pub customer_no: *const c_char,
    pub ticket_type: *const c_char,
    pub offline_record: *const c_char,
    pub price_cent: i32,
    pub route_no: *const c_char,
    pub bound: i32,
    pub stage_no: i32,
    pub journey_type: i32,
    pub order_number: *const c_char,
}

pub type EventCallback = unsafe extern "C" fn(topic: *const c_char, json: *const c_char);

#[cfg(easygo_stub)]
pub use crate::ffi_stub::*;

#[cfg(not(easygo_stub))]
extern "C" {
    fn eg_sizeof_init_config() -> usize;
    fn eg_sizeof_verify_result() -> usize;
    fn eg_sizeof_order() -> usize;

    pub fn eg_set_event_callback(cb: Option<EventCallback>);

    pub fn eg_init(cfg: *const InitConfig) -> c_int;
    pub fn eg_verify(qrcode: *const c_char, price_cent: i32, out: *mut VerifyResult) -> c_int;
    pub fn eg_upload_order(
        order: *const Order,
        order_tid: *mut c_char,
        order_tid_cap: i32,
    ) -> c_int;
    pub fn eg_reset() -> c_int;
    pub fn eg_check_status() -> c_int;
    pub fn eg_set_gps_data(longitude: f64, latitude: f64, velocity: f64, azimuth: f64) -> c_int;
    pub fn eg_get_version(buf: *mut c_char, cap: i32) -> c_int;
    pub fn eg_set_power_off_protection(enable: i32) -> c_int;
    pub fn eg_set_route_no(route_no: *const c_char) -> c_int;
    pub fn eg_set_bound(bound: i32) -> c_int;
    pub fn eg_send_task_response(task_no: *const c_char, success: i32) -> c_int;
    pub fn eg_store_offline_order(path: *const c_char, file_name: *const c_char) -> c_int;
    pub fn eg_delete_offline_order() -> c_int;
    pub fn eg_download_file(
        url: *const c_char,
        path: *const c_char,
        task_no: *const c_char,
    ) -> c_int;
    pub fn eg_set_feature_switch(
        close_white_list: u8,
        close_price_list_message: u8,
        close_route_no_list_message: u8,
    ) -> c_int;
    pub fn eg_set_qr_institution_no_array_json(json: *const c_char) -> c_int;
}

pub fn check_abi() {
    // SAFETY: pure functions returning sizeof() of shim structs.
    let (init, verify, order) = unsafe {
        (
            eg_sizeof_init_config(),
            eg_sizeof_verify_result(),
            eg_sizeof_order(),
        )
    };
    assert_eq!(
        init,
        size_of::<InitConfig>(),
        "eg_init_config layout mismatch"
    );
    assert_eq!(
        verify,
        size_of::<VerifyResult>(),
        "eg_verify_result layout mismatch"
    );
    assert_eq!(order, size_of::<Order>(), "eg_order layout mismatch");
}
