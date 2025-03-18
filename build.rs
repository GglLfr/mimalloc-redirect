use std::env::{set_var, var};

use cmake::Config;
use cmake_package::find_package;
use regex::Regex;

fn main() {
    if var("CARGO_CFG_TARGET_FAMILY")
        .unwrap()
        .split(',')
        .any(|f| f == "wasm")
    {
        println!(
            "cargo::error=WASM isn't supported by MiMalloc; please cfg-gate the dependency on `mimalloc-redirect`"
        );
        return;
    }

    let static_crt = var("CARGO_CFG_TARGET_FEATURE")
        .unwrap()
        .split(',')
        .any(|f| f == "crt-static");

    let mut config = Config::new("mimalloc-src");
    config
        .static_crt(static_crt)
        .define("MI_OVERRIDE", "OFF")
        .define("MI_BUILD_OBJECT", "OFF")
        .define("MI_BUILD_TESTS", "OFF");

    let env = &*var("CARGO_CFG_TARGET_ENV").unwrap();
    let (dst, is_static, splitter) = match &*var("CARGO_CFG_TARGET_OS").unwrap() {
        "windows" if matches!(env, "msvc") => {
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
                if static_crt { true } else { false },
                Regex::new("").unwrap(),
            )
        }
        "windows" | "linux" => {
            for wrap in [
                "malloc",
                "calloc",
                "realloc",
                "free",
                "aligned_alloc",
                "strdup",
                "strndup",
                "realpath",
            ] {
                println!("cargo::rustc-link-arg=-Wl,--wrap={wrap}")
            }

            let musl = matches!(env, "musl");
            (
                config
                    .define("MI_LIBC_MUSL", if musl { "ON" } else { "OFF" })
                    .define("MI_BUILD_SHARED", "OFF")
                    .define("MI_BUILD_STATIC", "ON")
                    .build(),
                true,
                Regex::new(r"(.*)/lib([^/]+)\.a.*").unwrap(),
            )
        }
        os => {
            println!(
                "cargo::error=OS `{os}` is not supported by `mimalloc-redirect` yet; please open an issue!"
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
        .target(if is_static {
            "mimalloc-static"
        } else {
            "mimalloc"
        })
        .expect("Supplied target doesn't exist!");

    // `cmake-package`'s `link()` is completely broken.
    for lib in target.link_libraries {
        let cap = splitter.captures(&lib).unwrap();

        println!(
            "cargo::rustc-link-search=native={}",
            cap.get(1).unwrap().as_str(),
        );

        println!(
            "cargo::rustc-link-lib={}={}",
            if is_static { "static" } else { "dylib" },
            cap.get(2).unwrap().as_str(),
        );
    }
}
