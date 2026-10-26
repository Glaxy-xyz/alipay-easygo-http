use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(easygo_stub)");
    println!("cargo:rerun-if-env-changed=EASYGO_SDK_DIR");
    println!("cargo:rerun-if-changed=shim/easygo_shim.h");
    println!("cargo:rerun-if-changed=shim/easygo_shim.cpp");

    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS");
    // macOS / Windows 使用 stub（不链接真实 SDK）。
    if target_os != "linux" && target_os != "android" {
        println!("cargo:rustc-cfg=easygo_stub");
        println!(
            "cargo:warning=macOS/Windows stub build: HTTP 可运行，SDK/MQTT 无效。真实 SDK 请用 Dev Container (Linux armv7) 或 Android build。"
        );
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // Android 只能链接厂商提供的 Bionic 版 .so。Linux armhf 的 sdk/lib 不能用。
    // 缺少对应 ABI 时打 stub 包，HTTP 可在模拟器里启动。
    if target_os == "android" {
        let abi = android_abi();
        let android_sdk = env::var("EASYGO_ANDROID_SDK_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| manifest_dir.join("sdk-android"));
        let sdk_lib = android_sdk.join("lib").join(abi);
        let pos = sdk_lib.join("libpos_manager.so");
        if !pos.is_file() {
            println!("cargo:rustc-cfg=easygo_stub");
            println!(
                "cargo:warning=Android stub build: {} 不存在。APK 可启动 HTTP；验码/上传需要厂商 Android 版 libpos_manager.so。",
                pos.display()
            );
            return;
        }
        let sdk_include = if android_sdk.join("include").is_dir() {
            android_sdk.join("include")
        } else {
            manifest_dir.join("sdk/include")
        };
        compile_shim(&sdk_include, &sdk_lib);
        return;
    }

    let sdk_root = env::var("EASYGO_SDK_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest_dir.join("sdk"));
    let sdk_include = sdk_root.join("include");
    let sdk_lib = sdk_root.join("lib");

    for p in [&sdk_include, &sdk_lib] {
        if !p.is_dir() {
            panic!(
                "SDK path missing {} — copy EasyGo armhf SDK into sdk/include and sdk/lib",
                p.display()
            );
        }
    }

    compile_shim(&sdk_include, &sdk_lib);
}

fn android_abi() -> &'static str {
    match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("arm") => "armeabi-v7a",
        Ok("aarch64") => "arm64-v8a",
        Ok("x86") => "x86",
        Ok("x86_64") => "x86_64",
        Ok(other) => panic!("unsupported Android arch: {other}"),
        Err(_) => panic!("CARGO_CFG_TARGET_ARCH is unset"),
    }
}

fn compile_shim(sdk_include: &PathBuf, sdk_lib: &PathBuf) {
    for p in [sdk_include, sdk_lib] {
        if !p.is_dir() {
            panic!(
                "SDK path missing {} — copy EasyGo SDK into sdk/include and sdk/lib",
                p.display()
            );
        }
    }

    cc::Build::new()
        .cpp(true)
        .std("c++11")
        .file("shim/easygo_shim.cpp")
        .include("shim")
        .include(sdk_include)
        .compile("easygo_shim");

    println!("cargo:rustc-link-search=native={}", sdk_lib.display());
    println!("cargo:rustc-link-lib=dylib=pos_manager");

    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_env == "gnu" {
        println!("cargo:rustc-link-arg=-Wl,-rpath-link,{}", sdk_lib.display());
        println!("cargo:rustc-link-arg=-Wl,--allow-shlib-undefined");
    }
}
