/*
 * Pure C facade over the EasyGo SDK. The SDK header exposes std::string,
 * std::map and std::list in callbacks and parameters, so nothing outside this
 * shim may include pos_manager_api.h.
 *
 * Every eg_* function except eg_set_event_callback must be called from one
 * thread at a time: the SDK does not document any thread-safety guarantees.
 */
#ifndef EASYGO_SHIM_H
#define EASYGO_SHIM_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* topic is one of "config", "firmware", "price", "download". Both strings are
 * only valid for the duration of the call. Invoked on SDK-owned threads. */
typedef void (*eg_event_cb)(const char *topic, const char *json);

typedef struct {
  const char *url;
  const char *port;
  const char *device_no;
  const char *vehicle_no;
  const char *route_no;
  const char *app_id;
  const char *data_path;
  const char *username;
  const char *password;
  const char *ca_file;
  const char *cert_file;
  const char *key_file;
} eg_init_config;

/* Buffer capacities from the v1.0.0.9 interface doc (minimum sizes). */
#define EG_UID_CAP 17
#define EG_RECORD_CAP 2048
/* Interface doc v1.0.0.9: user/institution reserve buffers must be at least 1024. */
#define EG_RESERVE_CAP 1024
#define EG_TICKET_TYPE_CAP 16
/* verify() rejects a qrcode whose strlen is greater than this. */
#define EG_QRCODE_MAX 1024

typedef struct {
  /* Lengths as reported back by the SDK in POS_VERIFY_RESPONSE_LEN. */
  int32_t uid_len;
  int32_t record_len;
  int32_t user_reserve_len;
  int32_t institution_reserve_len;
  char uid[EG_UID_CAP + 1];
  char record[EG_RECORD_CAP + 1];
  uint8_t user_reserve[EG_RESERVE_CAP];
  uint8_t institution_reserve[EG_RESERVE_CAP];
  uint8_t ticket_type[EG_TICKET_TYPE_CAP];
} eg_verify_result;

typedef struct {
  int64_t tap_time; /* epoch milliseconds */
  const char *customer_no;
  const char *ticket_type;
  const char *offline_record;
  int32_t price_cent;
  const char *route_no;
  int32_t bound;
  int32_t stage_no;
  int32_t journey_type;
  /* NULL lets the SDK generate one. Max length is EG_ORDER_NUMBER_MAX (header: 最大长度24). */
  const char *order_number;
} eg_order;

#define EG_ORDER_NUMBER_MAX 24

size_t eg_sizeof_init_config(void);
size_t eg_sizeof_verify_result(void);
size_t eg_sizeof_order(void);

void eg_set_event_callback(eg_event_cb cb);

int eg_init(const eg_init_config *cfg);
int eg_verify(const char *qrcode, int32_t price_cent, eg_verify_result *out);
int eg_upload_order(const eg_order *order, char *order_tid, int32_t order_tid_cap);
int eg_reset(void);
int eg_check_status(void);
int eg_set_gps_data(double longitude, double latitude, double velocity, double azimuth);
int eg_get_version(char *buf, int32_t cap);
int eg_set_power_off_protection(int32_t enable);
int eg_set_route_no(const char *route_no);
int eg_set_bound(int32_t bound);
int eg_send_task_response(const char *task_no, int32_t success);
int eg_store_offline_order(const char *path, const char *file_name);
int eg_delete_offline_order(void);
int eg_download_file(const char *url, const char *path, const char *task_no);
int eg_set_feature_switch(uint8_t close_white_list, uint8_t close_price_list_message,
                          uint8_t close_route_no_list_message);
int eg_set_qr_institution_no_array_json(const char *json);

#ifdef __cplusplus
}
#endif

#endif /* EASYGO_SHIM_H */
