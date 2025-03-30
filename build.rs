use std::{
    env::{var, var_os},
    path::PathBuf,
};

use cmake::Config;

fn main() {
    if var("CARGO_CFG_TARGET_FAMILY").unwrap().split(',').any(|f| f == "wasm") {
        println!("cargo::error=MiMalloc doesn't support WebAssembly targets!");
        return
    }

    let static_crt = var("CARGO_CFG_TARGET_FEATURE").unwrap().split(',').any(|f| f == "crt-static");
    let (name, profile) = match (&*var("OPT_LEVEL").unwrap(), &*var("DEBUG").unwrap()) {
        ("0", ..) => ("mimalloc-debug", "Debug"),
        ("s" | "z", ..) => ("mimalloc", "MinSizeRel"),
        ("1" | "2" | "3", debug) => match debug {
            "false" => ("mimalloc", "Release"),
            "true" => ("mimalloc", "RelWithDebInfo"),
            debug => {
                println!("cargo::error=Invalid `DEBUG` value: `{debug}`.");
                return
            }
        },
        (level, ..) => {
            println!("cargo::error=Invalid `OPT_LEVEL` value: `{level}`.");
            return
        }
    };

    let mut config = Config::new("mimalloc-src");
    config
        .static_crt(static_crt)
        .profile(profile)
        .define("MI_OVERRIDE", "OFF")
        .define("MI_BUILD_OBJECT", "OFF")
        .define("MI_BUILD_TESTS", "OFF")
        .define("MI_INSTALL_TOPLEVEL", "ON");

    let arch = &*var("CARGO_CFG_TARGET_ARCH").unwrap();
    let os = &*var("CARGO_CFG_TARGET_OS").unwrap();
    let env = &*var("CARGO_CFG_TARGET_ENV").unwrap();

    enum Target {
        Windows { msvc: bool },
        Linux,
        Android,
    }

    use Target::*;
    let target = match (os, env) {
        ("windows", "msvc") => {
            if static_crt {
                println!(
                    "cargo::warning=`+crt-static` isn't a good idea for `mimalloc-redirect` as that'll disable application-wide redirection."
                )
            }

            config
                .define("MI_WIN_REDIRECT", "ON")
                .define("MI_BUILD_SHARED", if static_crt { "OFF" } else { "ON" })
                .define("MI_BUILD_STATIC", if static_crt { "ON" } else { "OFF" });

            Windows { msvc: true }
        }
        ("windows", "gnu") => {
            println!("cargo::warning=`*-windows-gnu` doesn't support application-wide redirection.");

            println!("cargo::rustc-link-lib=advapi32");
            config
                .define("MI_BUILD_SHARED", "OFF")
                .define("MI_BUILD_STATIC", "ON")
                .build();

            Windows { msvc: false }
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

            config
                .define("MI_LIBC_MUSL", if matches!(env, "musl") { "ON" } else { "OFF" })
                .define("MI_BUILD_SHARED", "OFF")
                .define("MI_BUILD_STATIC", "ON");

            if matches!(os, "android") {
                let Some(ndk_root) = var_os("ANDROID_NDK_HOME").or_else(|| var_os("ANDROID_NDK_ROOT")) else {
                    println!("cargo::error=`ANDROID_NDK_HOME` not found!");
                    return;
                };

                let mut toolchain = PathBuf::from(ndk_root);
                toolchain.push("build");
                toolchain.push("cmake");
                toolchain.push("android.toolchain.cmake");

                let min_version = var("ANDROID_PLATFORM")
                    .or_else(|_| var("ANDROID_NATIVE_API_LEVEL"))
                    .unwrap_or_else(|_| "21".into())
                    .parse::<u32>()
                    .unwrap_or_else(|_| {
                        println!("cargo::warning=Missing or invalid `ANDROID_PLATFORM` variable; defaulting to 21.");
                        21
                    });

                let android_arch = match arch {
                    "aarch64" => "arm64-v8a",
                    "arm" => "armeabi-v7a",
                    "x86_64" => "x86_64",
                    "x86" => "x86",
                    _ => {
                        println!("cargo::error=Unsupported Android architecture: `{arch}`!");
                        return;
                    }
                };

                config
                    .generator("Ninja")
                    .define("ANDROID_ABI", android_arch)
                    .define("ANDROID_PLATFORM", format!("{min_version}"))
                    .define("CMAKE_TOOLCHAIN_FILE", toolchain);

                Android
            } else {
                Linux
            }
        }
        (os, env) => {
            println!(
                "cargo::error=OS `{os}` with environment `{env}` is not supported by `mimalloc-redirect` yet; please open an issue!"
            );
            return;
        }
    };

    let dst = config.build();
    let Some(dst) = dst.to_str() else {
        println!("cargo::error=Non-unicode paths is unsupported!");
        return;
    };

    println!("cargo::rustc-link-search=native={dst}/lib");
    match target {
        Windows { msvc: true } => {
            if static_crt {
                println!("cargo::rustc-link-lib=static={name}");
            } else {
                println!("cargo::rustc-link-search=native={dst}/bin");
                println!("cargo::rustc-link-lib=dylib={name}.dll");
            }
        }
        Windows { msvc: false } | Linux | Android => {
            println!("cargo::rustc-link-lib=static={name}");
        }
    }
}
