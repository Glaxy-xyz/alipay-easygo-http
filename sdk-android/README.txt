sdk-android/ — EasyGo SDK 的 Android 版本
==========================================

此目录需放置由 EasyGo 厂商提供的 Android 版原生库。

目录结构
--------
sdk-android/
├── include/          (可软链到 ../sdk/include，内容相同)
└── lib/
    ├── armeabi-v7a/  ARM 32-bit Android 库
    │   ├── libpos_manager.so
    │   └── (其他依赖 .so)
    └── arm64-v8a/    ARM 64-bit Android 库
        ├── libpos_manager.so
        └── (其他依赖 .so)

重要说明
--------
* sdk/lib/ 中的 libpos_manager.so 是 Linux armhf (glibc) 版本，
  无法在 Android (Bionic libc) 上直接使用。
* 请联系 EasyGo 厂商索取 Android 专用版本的 .so 文件。
* include/ 头文件与 Linux 版本共用，可直接软链：
    ln -sf ../sdk/include sdk-android/include

构建命令
--------
export ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/27.x.x
bash scripts/build-android.sh
