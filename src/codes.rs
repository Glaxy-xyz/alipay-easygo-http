//! SDK return codes from `pos_manager_api.h` and their HTTP mapping.

use axum::http::StatusCode;

pub const SUCCESS: i32 = 1;
pub const WHITE_LIST_USER: i32 = 2;
pub const POS_PARAM_ERROR: i32 = -4;
pub const SYSTEM_ERROR: i32 = -7;
pub const NOT_INITIALIZED: i32 = -9;

pub fn is_success(code: i32) -> bool {
    code == SUCCESS || code == WHITE_LIST_USER
}

pub fn name(code: i32) -> &'static str {
    match code {
        1 => "SUCCESS",
        2 => "WHITE_LIST_USER",
        -1 => "MALFORMED_QRCODE",
        -2 => "QRCODE_INFO_EXPIRED",
        -3 => "QRCODE_KEY_EXPIRED",
        -4 => "POS_PARAM_ERROR",
        -5 => "QUOTA_EXCEEDED",
        -6 => "NO_ENOUGH_MEMORY",
        -7 => "SYSTEM_ERROR",
        -8 => "CARDTYPE_UNSUPPORTED",
        -9 => "NOT_INITIALIZED",
        -10 => "ILLEGAL_PARAM",
        -11 => "PROTO_UNSUPPORTED",
        -12 => "QRCODE_DUPLICATED",
        -13 => "INSTITUTION_NOT_SUPPORT",
        -14 => "BLACK_LIST_USER",
        -15 => "DEVICE_NO_NOT_EXIST",
        -16 => "CLIENT_PARAM_ERROR",
        -17 => "TOKEN_ERROR",
        -18 => "SERVER_UNKNOWN_CODE",
        -19 => "SERVER_SYSTEM_ERROR",
        -20 => "NETWORK_ERROR",
        -21 => "SERVER_RESPONSE_PARSE_ERROR",
        -22 => "TID_NOT_EXIST",
        -23 => "SQLITE_DB_ERROR",
        -24 => "USER_DUPLICATED",
        -25 => "POWER_OFF_PROTECTION",
        -26 => "OFFLINE_STORE_ORDER_PARAM_ERROR",
        -27 => "OFFLINE_STORE_ORDER_PATH_DISENABLE",
        -28 => "DOWN_LOAD_FILE_ERROR",
        -29 => "SET_INSTITUTION_NO_ERROR",
        -30 => "LOACL_DATA_EXPIRED",
        -31 => "WHITE_ROUTE_NOT_SUPPORT_GENERAL_USER",
        -32 => "WHITE_LIST_CRC_ERROR",
        -33 => "WHITE_LIST_TEMP_CLEAR_ERROR",
        -34 => "WHITE_LIST_CLEAR_ERROR",
        -35 => "WHITE_LIST_CONFIG_ERROR",
        _ => "UNKNOWN",
    }
}

pub fn http_status(code: i32) -> StatusCode {
    match code {
        1 | 2 => StatusCode::OK,
        -4 | -10 | -16 | -26 | -27 | -29 => StatusCode::BAD_REQUEST,
        -14 | -31 => StatusCode::FORBIDDEN,
        -22 => StatusCode::NOT_FOUND,
        -12 | -24 => StatusCode::CONFLICT,
        -1 | -2 | -3 | -8 | -11 | -13 => StatusCode::UNPROCESSABLE_ENTITY,
        -5 => StatusCode::TOO_MANY_REQUESTS,
        -15 | -17 | -18 | -19 | -20 | -21 | -28 => StatusCode::BAD_GATEWAY,
        -9 | -25 | -30 => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
