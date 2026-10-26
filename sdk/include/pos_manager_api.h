#ifndef POS_MANAGER_API_H
#define  POS_MANAGER_API_H
#include <stdio.h>
#include <iostream>
#include <map>
#include <vector>
#include <list>
#include  "pos_callback_params.h"
namespace alipayplus {

#define WHITE_LIST_USER                 (2)

#ifndef SUCCESS
#define SUCCESS                         (  1)
#endif

#ifndef MALFORMED_QRCODE
#define MALFORMED_QRCODE                ( -1)
#endif

#ifndef QRCODE_INFO_EXPIRED
#define QRCODE_INFO_EXPIRED             ( -2)
#endif

#ifndef QRCODE_KEY_EXPIRED
#define QRCODE_KEY_EXPIRED              ( -3)
#endif

#ifndef POS_PARAM_ERROR
#define POS_PARAM_ERROR                 ( -4)
#endif

#ifndef QUOTA_EXCEEDED
#define QUOTA_EXCEEDED                  ( -5)
#endif

#ifndef NO_ENOUGH_MEMORY
#define NO_ENOUGH_MEMORY                ( -6)
#endif

#ifndef SYSTEM_ERROR
#define SYSTEM_ERROR                    ( -7)
#endif

#ifndef CARDTYPE_UNSUPPORTED
#define CARDTYPE_UNSUPPORTED            ( -8)
#endif

#ifndef NOT_INITIALIZED
#define NOT_INITIALIZED                 ( -9)
#endif

#ifndef ILLEGAL_PARAM
#define ILLEGAL_PARAM                   (-10)
#endif

#ifndef PROTO_UNSUPPORTED
#define PROTO_UNSUPPORTED               (-11)
#endif

#ifndef QRCODE_DUPLICATED
#define QRCODE_DUPLICATED               (-12)
#endif

#ifndef INSTITUTION_NOT_SUPPORT
#define INSTITUTION_NOT_SUPPORT         (-13)
#endif

#define BLACK_LIST_USER         		    (-14)
#define DEVICE_NO_NOT_EXIST   		      (-15)
#define CLIENT_PARAM_ERROR           	  (-16)
#define TOKEN_ERROR 					          (-17)
#define SERVER_UNKNOWN_CODE             (-18)
#define SERVER_SYSTEM_ERROR             (-19)
#define NETWORK_ERROR                   (-20)
#define SERVER_RESPONSE_PARSE_ERROR     (-21)
#define TID_NOT_EXIST                   (-22)
#define SQLITE_DB_ERROR					        (-23)
#define USER_DUPLICATED                 (-24)

#ifndef POWER_OFF_PROTECTION
#define POWER_OFF_PROTECTION            (-25) // 断电保护阶段，不接收新业务
#endif

#ifndef OFFLINE_STORE_ORDER_PARAM_ERROR
#define OFFLINE_STORE_ORDER_PARAM_ERROR (-26)
#endif

#ifndef OFFLINE_STORE_ORDER_PATH_DISENABLE
#define OFFLINE_STORE_ORDER_PATH_DISENABLE (-27)
#endif
#ifndef DOWN_LOAD_FILE_ERROR
#define DOWN_LOAD_FILE_ERROR             (-28)
#endif

#ifndef SET_INSTITUTION_NO_ERROR
#define SET_INSTITUTION_NO_ERROR             (-29)
#endif

#ifndef  LOACL_DATA_EXPIRED
#define LOACL_DATA_EXPIRED                   (-30)
#endif

#ifndef WHITE_ROUTE_NOT_SUPPORT_GENERAL_USER
#define WHITE_ROUTE_NOT_SUPPORT_GENERAL_USER (-31)
#endif

#ifndef WHITE_LIST_CRC_ERROR
#define WHITE_LIST_CRC_ERROR                         (-32)
#endif

#ifndef WHITE_LIST_TEMP_CLEAR_ERROR
#define WHITE_LIST_TEMP_CLEAR_ERROR                         (-33)
#endif

#ifndef WHITE_LIST_CLEAR_ERROR
#define WHITE_LIST_CLEAR_ERROR                         (-34)
#endif

#ifndef WHITE_LIST_CONFIG_ERROR
#define WHITE_LIST_CONFIG_ERROR                         (-35)
#endif

  typedef struct _pos_verify_response_len
  {
    int uid_len;
    int record_len;
    int user_reserve_len;
    int institution_reserve_len;
  } POS_VERIFY_RESPONSE_LEN;

  typedef struct
  {
    char *url;
    char *port;
    char *device_no;
    char *vehicle_no;
    char *route_no;
    char *app_id;
    char *data_path;
    char *username;
    char *password;
    char *ca_file;
    char *cert_file;
    char *key_file;
  } INIT_POS_CONFIG;
  typedef struct _order
  {
    char *tid;
    char *customer_no;
    char *offline_record;
    int price_cent;
    char *time_stamp;
    char *ticket_type;
    char *route_no;
    int bound;
    int stage_no;
    int journey_type;
    _order *next;
  } OFFLINE_OREDER_LIST;

  typedef struct _order_info
  {
    int64_t tap_time;     //时间戳
    char *customer_no;    //用户ID
    char *ticket_type;    //票类型
    char *offline_record; //脱机记录
    int price_cent;       //价格
    char *route_no;       //线路号
    bool bound;           //线路方向
    int stage_no;         //价格阶段号
    int journey_type;     //行程类型
    char *order_number;   //用户自定义订单号,最大长度24.如果不传该字段,SDK内部会自动生成.功能与upload_order一样.
  } ORDER_INFO;

  typedef struct _order_response
  {
    char *order_tid;
  } POS_ORDER_RESPONSE;
  enum ORDER_TYPE
  {
    NORMAL,  //正常数据
    FAILURE, //上传失败数据
    ALL      //所有数据
  };
  typedef struct _switching_value
  {
    unsigned char close_white_list;
    unsigned char close_price_list_mssage;
    unsigned char close_rout_no_list_mssage;
  } FEATURE_SWITCHING_VALUE;
  extern "C" int init_pos_manager(INIT_POS_CONFIG config);
  extern "C" int init_pos_manager_callback(INIT_POS_CONFIG config,
                                           pos_config_callback config_callback,
                                           pos_fireware_config_callback fireware_callback,
                                           pos_price_list_callback price_callback);
  extern "C" int verify(char *qrcode, int price_cent, char *uid, char *record,
                        unsigned char *user_reserve, unsigned char *institution_reserve,
                        POS_VERIFY_RESPONSE_LEN *response_len, char *ticket_type);

  extern "C" int reset();
  extern "C" int check_status();

  extern "C" int set_gps_data(double longitude, double latitude,
                              double velocity, double azimuth);

  extern "C" void register_upadate_config_callback(pos_config_callback callback);

  extern "C" void unregister_upadate_config_callback();

  extern "C" void register_upadate_fireware_callback(pos_fireware_config_callback callback);

  extern "C" void unregister_upadate_fireware_callback();

  extern "C" void register_upadate_price_list_callback(pos_price_list_callback callback);

  extern "C" void unregister_upadate_price_list_callback();

  //原接口参数太多,把订单相关信息都放到了ORDER_INFO结构里面
  extern "C" int upload_order(ORDER_INFO orderInfo, POS_VERIFY_RESPONSE_LEN *response_len, POS_ORDER_RESPONSE *order_response);

  extern "C" char *get_version();
  // set SDK business statue
  extern "C" void set_power_off_protection(int is_enable);
  // set route number
  extern "C" void set_route_no(char *route_no);

  // set bound
  extern "C" void set_bound(bool bound);

  //上报task任务成功
  extern "C" int send_task_response(string task_no, bool success);

  /**
   * Description:  Mobile storage device to store data
   * Input:path is mobile storage device path
   * Return:1 stander success，other stander failure
   */
  extern "C" int store_offline_order(char *path, char *file_name);
  /**
   * Description:delete offline order
   * Input:N/A
   * Return: 1 stander success，other stander failure
   */
  extern "C" int delete_offline_order();

  extern "C" void download_file_with_callback(char *file_url, char *path, pos_downlod_file_callback callback, char *const task_no);
  extern "C" void set_feature_switch(FEATURE_SWITCHING_VALUE values);
  extern "C" int set_qr_institution_no_array_json(const char *qr_institution_no_array_json);

}

#endif //POS_MANAGER_API_H
