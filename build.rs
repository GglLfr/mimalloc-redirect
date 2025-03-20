use std::{
    env::{set_var, var},
    sync::LazyLock,
};

use cmake::Config;
use cmake_package::find_package;
use regex::Regex;

fn main() {
    if var("CARGO_CFG_TARGET_FAMILY").unwrap().split(',').any(|f| f == "wasm") {
        println!("cargo::error=WASM isn't supported by MiMalloc; please cfg-gate the dependency on `mimalloc-redirect`");
        return;
    }

    let static_crt = var("CARGO_CFG_TARGET_FEATURE").unwrap().split(',').any(|f| f == "crt-static");

    let mut config = Config::new("mimalloc-src");
    config
        .static_crt(static_crt)
        .define("MI_OVERRIDE", "OFF")
        .define("MI_BUILD_OBJECT", "OFF")
        .define("MI_BUILD_TESTS", "OFF");

    let os = &*var("CARGO_CFG_TARGET_OS").unwrap();
    let env = &*var("CARGO_CFG_TARGET_ENV").unwrap();

    let (dst, is_static) = match (os, env) {
        ("windows", "msvc") => {
            if static_crt {
                println!(
                    "cargo::warning=`+crt-static` isn't a good idea for `mimalloc-redirect` as that'll disable application-wide redirection."
                )
            }

            (
                config
                    .define("MI_WIN_REDIRECT", "ON")
                    .define("MI_BUILD_SHARED", if static_crt { "OFF" } else { "ON" })
                    .define("MI_BUILD_STATIC", if static_crt { "ON" } else { "OFF" })
                    .build(),
                static_crt,
            )
        }
        ("windows", "gnu") => {
            println!("cargo::warning=`*-windows-gnu` doesn't support application-wide redirection.");

            println!("cargo::rustc-link-lib=advapi32");
            (
                config
                    .define("MI_BUILD_SHARED", "OFF")
                    .define("MI_BUILD_STATIC", "ON")
                    .build(),
                true,
            )
        }
        ("linux", "gnu" | "musl") | ("android", "") => {
            for wrap in [
                "malloc",
                "calloc",
                "realloc",
                "free",
                "aligned_alloc",
                "strdup",
                "strndup",
                "realpath",
                "posix_memalign",
                "memalign",
                "valloc",
                "pvalloc",
                "malloc_usable_size",
                "reallocf",
            ] {
                println!("cargo::rustc-link-arg=-Wl,--wrap={wrap}")
            }

            unsafe {
                set_var("CC", "aarch64-linux-android35-clang.cmd");
                set_var("CXX", "aarch64-linux-android35-clang++.cmd");

            }

            (
                config
                    .define("MI_LIBC_MUSL", if matches!(env, "musl") { "ON" } else { "OFF" })
                    .define("MI_BUILD_SHARED", "OFF")
                    .define("MI_BUILD_STATIC", "ON")
                    .generator("Ninja")
                    .define("ANDROID_ABI", "arm64-v8a")
                    .define(
                        "CMAKE_TOOLCHAIN_FILE",
                        "/Android/ndk/29.0.13113456/build/cmake/android.toolchain.cmake",
                    )
                    .define("ANDROID_PLATFORM", "35")
                    .build(),
                true,
            )
        }
        (os, env) => {
            println!(
                "cargo::error=OS `{os}` with environment `{env}` is not supported by `mimalloc-redirect` yet; please open an issue!"
            );
            return;
        }
    };

    // Safety: `build.rs` is single-threaded.
    // We need this for `find_package(...)` to work properly.
    unsafe { set_var("CMAKE_PREFIX_PATH", &dst) }
    let target = find_package("mimalloc")
        .version("2.2")
        .find()
        .expect("`find_package(...)` failed!")
        .target(if is_static { "mimalloc-static" } else { "mimalloc" })
        .expect("Supplied target doesn't exist!");

    // `cmake-package`'s `link()` is completely broken.
    for lib in target.link_libraries {
        match (os, env) {
            ("windows", "msvc") => {
                if is_static {
                    static SPLITTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(.*)/([^/]+)\.lib.*").unwrap());
                    let cap = SPLITTER.captures(&lib).unwrap();

                    println!("cargo::rustc-link-search=native={}", cap.get(1).unwrap().as_str());
                    println!("cargo::rustc-link-lib=static={}", cap.get(2).unwrap().as_str());
                } else {
                    static SPLITTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(.*)/bin/([^/]+)\.dll.*").unwrap());
                    let cap = SPLITTER.captures(&lib).unwrap();

                    println!("cargo::rustc-link-search=native={}/bin", cap.get(1).unwrap().as_str(),);
                    println!("cargo::rustc-link-search=native={}/lib", cap.get(1).unwrap().as_str(),);
                    println!("cargo::rustc-link-lib=dylib={}.dll", cap.get(2).unwrap().as_str(),);
                }
            }
            ("windows" | "linux", "gnu" | "musl") | ("android", "") => {
                static SPLITTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(.*)/lib([^/]+)\.a.*").unwrap());
                let cap = SPLITTER.captures(&lib).unwrap();

                println!("cargo::rustc-link-search=native={}", cap.get(1).unwrap().as_str());
                println!(
                    "cargo::rustc-link-lib={}={}",
                    if is_static { "static" } else { "dylib" },
                    cap.get(2).unwrap().as_str(),
                );
            }
            (os, env) => unreachable!("Unimplemented: (`{os}`, `{env}`)"),
        }
    }
}
