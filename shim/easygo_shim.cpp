#include "easygo_shim.h"

#include <atomic>
#include <cstdio>
#include <cstring>
#include <list>
#include <map>
#include <string>

#include "pos_manager_api.h"

namespace {

std::atomic<eg_event_cb> g_event_cb(nullptr);

std::string to_std(const char *s) { return s ? std::string(s) : std::string(); }

// The SDK takes char* everywhere; hand it a writable, NUL-terminated buffer.
char *mut_ptr(std::string &s) { return &s[0]; }

void append_json_string(std::string &out, const std::string &s) {
  out += '"';
  for (unsigned char c : s) {
    switch (c) {
    case '"': out += "\\\""; break;
    case '\\': out += "\\\\"; break;
    case '\n': out += "\\n"; break;
    case '\r': out += "\\r"; break;
    case '\t': out += "\\t"; break;
    default:
      if (c < 0x20) {
        char buf[8];
        std::snprintf(buf, sizeof buf, "\\u%04x", c);
        out += buf;
      } else {
        out += static_cast<char>(c);
      }
    }
  }
  out += '"';
}

void append_int_list(std::string &out, const std::list<int32_t> &values) {
  out += '[';
  bool first = true;
  for (int32_t v : values) {
    if (!first) out += ',';
    first = false;
    out += std::to_string(v);
  }
  out += ']';
}

void append_int_map(std::string &out, const std::map<std::string, int32_t> &values) {
  out += '{';
  bool first = true;
  for (const auto &kv : values) {
    if (!first) out += ',';
    first = false;
    append_json_string(out, kv.first);
    out += ':';
    out += std::to_string(kv.second);
  }
  out += '}';
}

void emit(const char *topic, const std::string &json) {
  eg_event_cb cb = g_event_cb.load();
  if (cb) cb(topic, json.c_str());
}

// Callbacks run on SDK threads; exceptions must not escape into SDK code.

void on_config(std::map<std::string, std::string> *params) {
  try {
    std::string j = "{";
    if (params) {
      bool first = true;
      for (const auto &kv : *params) {
        if (!first) j += ',';
        first = false;
        append_json_string(j, kv.first);
        j += ':';
        append_json_string(j, kv.second);
      }
    }
    j += '}';
    emit("config", j);
  } catch (...) {
  }
}

void on_firmware(alipayplus::POS_FIRWARE_CONFIG_PARAMS *p, std::string task_no) {
  try {
    std::string j = "{\"task_no\":";
    append_json_string(j, task_no);
    if (p) {
      j += ",\"version_no\":";
      append_json_string(j, p->version_no);
      j += ",\"type\":";
      append_json_string(j, p->type);
      j += ",\"appointed_time\":" + std::to_string(p->appointed_time);
      j += ",\"file_address\":";
      append_json_string(j, p->file_address);
      j += ",\"force_update\":";
      j += p->force_update ? "true" : "false";
    }
    j += '}';
    emit("firmware", j);
  } catch (...) {
  }
}

void on_price(std::list<alipayplus::protobufwrapper::PriceTableObj *> *params) {
  try {
    std::string j = "[";
    if (params) {
      bool first = true;
      for (const auto *t : *params) {
        if (!t) continue;
        if (!first) j += ',';
        first = false;
        j += "{\"route_no\":";
        append_json_string(j, t->route_no);
        j += ",\"version_no\":";
        append_json_string(j, t->version_no);
        j += ",\"default_ticket_type\":" + std::to_string(t->default_ticket_type);
        j += ",\"stages\":";
        append_int_list(j, t->stages);
        j += ",\"ticket_types\":";
        append_int_list(j, t->ticket_types);
        j += ",\"journey_types\":";
        append_int_list(j, t->journey_types);
        j += ",\"bound\":";
        append_int_list(j, t->bound);
        j += ",\"price_table_map\":";
        append_int_map(j, t->price_table_map);
        j += ",\"if_set_white_rule\":";
        j += t->if_set_white_rule ? "true" : "false";
        j += ",\"if_not_can_go\":";
        j += t->if_not_can_go ? "true" : "false";
        j += ",\"white_price_table_map\":";
        append_int_map(j, t->white_price_table_map);
        j += '}';
      }
    }
    j += ']';
    emit("price", j);
  } catch (...) {
  }
}

void on_download(int result, char *file_path) {
  try {
    std::string j = "{\"result\":" + std::to_string(result) + ",\"file_path\":";
    append_json_string(j, to_std(file_path));
    j += '}';
    emit("download", j);
  } catch (...) {
  }
}

// The SDK may keep the char* it receives (init config, route number, async
// download arguments). Strings handed to it are therefore never freed.
std::string *retain(const char *s) { return new std::string(to_std(s)); }

} // namespace

extern "C" {

size_t eg_sizeof_init_config(void) { return sizeof(eg_init_config); }
size_t eg_sizeof_verify_result(void) { return sizeof(eg_verify_result); }
size_t eg_sizeof_order(void) { return sizeof(eg_order); }

void eg_set_event_callback(eg_event_cb cb) { g_event_cb.store(cb); }

int eg_init(const eg_init_config *c) {
  if (!c) return POS_PARAM_ERROR;
  const char *fields[] = {c->url,        c->port,      c->device_no, c->vehicle_no,
                          c->route_no,   c->app_id,    c->data_path, c->username,
                          c->password,   c->ca_file,   c->cert_file, c->key_file};
  for (const char *f : fields) {
    if (!f || !f[0]) return POS_PARAM_ERROR;
  }
  try {
    alipayplus::INIT_POS_CONFIG cfg;
    cfg.url = mut_ptr(*retain(c->url));
    cfg.port = mut_ptr(*retain(c->port));
    cfg.device_no = mut_ptr(*retain(c->device_no));
    cfg.vehicle_no = mut_ptr(*retain(c->vehicle_no));
    cfg.route_no = mut_ptr(*retain(c->route_no));
    cfg.app_id = mut_ptr(*retain(c->app_id));
    cfg.data_path = mut_ptr(*retain(c->data_path));
    cfg.username = mut_ptr(*retain(c->username));
    cfg.password = mut_ptr(*retain(c->password));
    cfg.ca_file = mut_ptr(*retain(c->ca_file));
    cfg.cert_file = mut_ptr(*retain(c->cert_file));
    cfg.key_file = mut_ptr(*retain(c->key_file));
    return alipayplus::init_pos_manager_callback(cfg, on_config, on_firmware, on_price);
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

int eg_verify(const char *qrcode, int32_t price_cent, eg_verify_result *out) {
  if (!qrcode || !out) return POS_PARAM_ERROR;
  if (std::strlen(qrcode) > EG_QRCODE_MAX) return ILLEGAL_PARAM;
  try {
    std::memset(out, 0, sizeof *out);
    alipayplus::POS_VERIFY_RESPONSE_LEN len;
    len.uid_len = EG_UID_CAP;
    len.record_len = EG_RECORD_CAP;
    len.user_reserve_len = EG_RESERVE_CAP;
    len.institution_reserve_len = EG_RESERVE_CAP;
    std::string qr(qrcode);
    int rc = alipayplus::verify(mut_ptr(qr), price_cent, out->uid, out->record, out->user_reserve,
                                out->institution_reserve, &len,
                                reinterpret_cast<char *>(out->ticket_type));
    out->uid[EG_UID_CAP] = '\0';
    out->record[EG_RECORD_CAP] = '\0';
    out->uid_len = len.uid_len;
    out->record_len = len.record_len;
    out->user_reserve_len = len.user_reserve_len;
    out->institution_reserve_len = len.institution_reserve_len;
    return rc;
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

int eg_upload_order(const eg_order *o, char *order_tid, int32_t order_tid_cap) {
  if (!o || !order_tid || order_tid_cap <= 0) return POS_PARAM_ERROR;
  if (!o->customer_no || !o->customer_no[0] || !o->ticket_type || !o->ticket_type[0] ||
      !o->offline_record || !o->offline_record[0] || !o->route_no || !o->route_no[0]) {
    return POS_PARAM_ERROR;
  }
  // NULL or "" means "not passed": the SDK generates the order number.
  // An empty std::string is not the same — mut_ptr would hand the SDK a
  // one-byte buffer, and it would not take the auto-generate path.
  if (o->order_number && o->order_number[0] && std::strlen(o->order_number) > EG_ORDER_NUMBER_MAX) {
    return POS_PARAM_ERROR;
  }
  try {
    std::string customer_no = to_std(o->customer_no);
    std::string ticket_type = to_std(o->ticket_type);
    std::string offline_record = to_std(o->offline_record);
    std::string route_no = to_std(o->route_no);
    std::string order_number;
    char *order_number_ptr = nullptr;
    if (o->order_number && o->order_number[0]) {
      order_number = o->order_number;
      order_number_ptr = mut_ptr(order_number);
    }

    alipayplus::ORDER_INFO info;
    info.tap_time = o->tap_time;
    info.customer_no = mut_ptr(customer_no);
    info.ticket_type = mut_ptr(ticket_type);
    info.offline_record = mut_ptr(offline_record);
    info.price_cent = o->price_cent;
    info.route_no = mut_ptr(route_no);
    info.bound = o->bound != 0;
    info.stage_no = o->stage_no;
    info.journey_type = o->journey_type;
    info.order_number = order_number_ptr;

    alipayplus::POS_VERIFY_RESPONSE_LEN len;
    len.uid_len = EG_UID_CAP;
    len.record_len = EG_RECORD_CAP;
    len.user_reserve_len = EG_RESERVE_CAP;
    len.institution_reserve_len = EG_RESERVE_CAP;

    alipayplus::POS_ORDER_RESPONSE resp;
    resp.order_tid = nullptr;
    int rc = alipayplus::upload_order(info, &len, &resp);
    // The SDK ignores the incoming order_tid and replaces it with a pointer
    // to a 128-byte buffer in its own stack frame. Copy that C string before
    // any other call reuses the stack. snprintf would do exactly that.
    const char *src = resp.order_tid;
    int32_t n = 0;
    if (src && order_tid_cap > 0) {
      const int32_t limit = order_tid_cap - 1 < 128 ? order_tid_cap - 1 : 128;
      while (n < limit && src[n] != '\0') {
        order_tid[n] = src[n];
        ++n;
      }
      order_tid[n] = '\0';
    } else if (order_tid_cap > 0) {
      order_tid[0] = '\0';
    }
    return rc;
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

int eg_reset(void) {
  try {
    return alipayplus::reset();
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

int eg_check_status(void) {
  try {
    return alipayplus::check_status();
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

int eg_set_gps_data(double longitude, double latitude, double velocity, double azimuth) {
  try {
    return alipayplus::set_gps_data(longitude, latitude, velocity, azimuth);
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

int eg_get_version(char *buf, int32_t cap) {
  if (!buf || cap <= 0) return POS_PARAM_ERROR;
  try {
    const char *v = alipayplus::get_version();
    std::snprintf(buf, static_cast<size_t>(cap), "%s", v ? v : "");
    return SUCCESS;
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

int eg_set_power_off_protection(int32_t enable) {
  try {
    alipayplus::set_power_off_protection(enable);
    return SUCCESS;
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

int eg_set_route_no(const char *route_no) {
  if (!route_no) return POS_PARAM_ERROR;
  try {
    alipayplus::set_route_no(mut_ptr(*retain(route_no)));
    return SUCCESS;
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

int eg_set_bound(int32_t bound) {
  try {
    alipayplus::set_bound(bound != 0);
    return SUCCESS;
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

int eg_send_task_response(const char *task_no, int32_t success) {
  if (!task_no) return POS_PARAM_ERROR;
  try {
    return alipayplus::send_task_response(std::string(task_no), success != 0);
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

int eg_store_offline_order(const char *path, const char *file_name) {
  if (!path || !file_name) return POS_PARAM_ERROR;
  try {
    std::string p(path), f(file_name);
    return alipayplus::store_offline_order(mut_ptr(p), mut_ptr(f));
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

int eg_delete_offline_order(void) {
  try {
    return alipayplus::delete_offline_order();
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

int eg_download_file(const char *url, const char *path, const char *task_no) {
  if (!url || !path) return POS_PARAM_ERROR;
  try {
    char *task = task_no ? mut_ptr(*retain(task_no)) : nullptr;
    alipayplus::download_file_with_callback(mut_ptr(*retain(url)), mut_ptr(*retain(path)),
                                            on_download, task);
    return SUCCESS;
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

int eg_set_feature_switch(uint8_t close_white_list, uint8_t close_price_list_message,
                          uint8_t close_route_no_list_message) {
  try {
    alipayplus::FEATURE_SWITCHING_VALUE v;
    v.close_white_list = close_white_list;
    v.close_price_list_mssage = close_price_list_message;
    v.close_rout_no_list_mssage = close_route_no_list_message;
    alipayplus::set_feature_switch(v);
    return SUCCESS;
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

int eg_set_qr_institution_no_array_json(const char *json) {
  if (!json) return POS_PARAM_ERROR;
  try {
    return alipayplus::set_qr_institution_no_array_json(json);
  } catch (...) {
    return SYSTEM_ERROR;
  }
}

} // extern "C"
