# JNI 函数不能被混淆
-keep class com.easygo.http.NativeLib { *; }

# 保留 Service 和 Activity
-keep class com.easygo.http.EasyGoService { *; }
-keep class com.easygo.http.MainActivity { *; }
