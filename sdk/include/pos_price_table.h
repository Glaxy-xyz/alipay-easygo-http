#ifndef POS_PRICE_TABLE_H
#define POS_PRICE_TABLE_H
#include <string>
#include <map>
#include <list>
using namespace std;
namespace alipayplus {

namespace protobufwrapper {
class PriceTableObj{
    public:
        std::string route_no ;  // 线路编号
        std::string version_no  ;  // 版本号
        int32_t default_ticket_type ; // 默认票种, 目前默认票种采用 0 表示
        list<int32_t> stages ;  // 包含的所有阶梯数组，比如 [1,2,3,4,5], 固定票价没有阶梯就是 [1]
        list<int32_t> ticket_types ; // 包含的所有票种数组，比如 [0,1,2,3], 1表示成人, 2表示小童, 3表示长者, 0 为默认票种
        list<int32_t> journey_types ; // 包含的所有行程类型，比如 [0,1], 0表示全程, 1 表示半程;
        list<int32_t>  bound ; // 包含的所有来回程，比如 [0,1], 0表示去程, 1表示回程
        map<std::string,int32_t> price_table_map ; // 价格表, 由阶梯,票种,行程类型,来回程四个元素决定唯一的key , value 就是价格
        bool if_set_white_rule;    // 是否设置白名单规则
        bool if_not_can_go;             // 设置白名单规则时非名单内是否可乘车
        map<std::string,int32_t> white_price_table_map ;    // 白名单价格(同正常价格)
};
};
};
#endif //POS_PRICE_TABLE_H
