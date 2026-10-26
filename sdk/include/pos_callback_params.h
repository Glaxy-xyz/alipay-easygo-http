#ifndef POS_CALLBACK_PARAMS_H
#define POS_CALLBACK_PARAMS_H
#include <vector>
#include <map>
#include <string>
#include <iostream>
#include "pos_price_table.h"
#define KEY_MIN_PRICE            "min_price"
#define KEY_MAX_PRICE            "max_price"
#define KEY_BLACK_LIST_INTERVAL  "list_interval"
#define KEY_WHITE_LIST_INTERVAL  "white_list_interval"
#define KEY_BOARD_INTERVAL       "board_interval"
#define KEY_METRICS_INTERVAL     "metrics_interval"
#define KEY_SDK_KEY              "sdk_key"
#define KEY_SDK_KEY_VERSION      "sdk_key_version"
#define KEY_SDK_KEY_INTERVAL     "sdk_key_interval"
#define KEY_SERVER_URL           "server_url"
#define KEY_SERVER_PORT          "server_port"
#define KEY_TICKET_TYPE_ENABLE   "ticket_type_enable"
#define KEY_VOLUME               "volume"
#define KEY_FIREWARE_VERSION     "fireware_version"
#define KEY_FIREWARE_VERSION_NEW     "fireware_version_new"
#define KEY_PRICE_VERSION        "price_table_version"
#define KEY_CONFIG_VERSION       "config_version"
#define KEY_READ_ENABLE          "read_enable"
#define KEY_INSTITUTION_NO       "institution_no"
#define KEY_CERT_TYPE_LIST       "cert_type_list"
#define KEY_IF_SET_WHITELIST_RULES      "if_set_whiteList_rules"
#define KEY_WHITE_LIST_RETRY_COUNT      "white_list_retry_count"
#define KEY_CACHE_TIME_INTERVAL      "cache_time_interval"
#define KEY_PAGE_SIZE               "white_list_page_size"

using namespace std;
namespace alipayplus
{
    typedef struct _pos_firware_config_params
    {
        string version_no;      // 版本号
        string type;            // 更新类型, 0 立即更新, 1 运营结束以后, 2 指定时间, 3 回滚
        int64_t appointed_time; // 预约的更新时间,精确到毫秒
        string file_address;    // 下载文件地址
        bool force_update;      // 是否强制升级
    } POS_FIRWARE_CONFIG_PARAMS;

    typedef void (*pos_fireware_config_callback)(POS_FIRWARE_CONFIG_PARAMS *params, string task_no);

    typedef void (*pos_price_list_callback)(list<alipayplus::protobufwrapper::PriceTableObj *> *params);

    //配置更新透传回调函数
    typedef void (*pos_config_callback)(map<string, string> *params);

    typedef void (*pos_downlod_file_callback)(int result,char* file_path);
};

#endif //POS_CALLBACK_PARAMS_H
