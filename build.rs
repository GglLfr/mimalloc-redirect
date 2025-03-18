use std::env::var;

use cmake::Config;

const PATH: &'static str = "mimalloc-src";

fn main() {
    let static_crt =
        var("CARGO_CFG_TARGET_FEATURE").is_ok_and(|features| features.split(',').any(|feature| feature == "crt-static"));

    let mut config = Config::new(PATH);
    let profile = config.get_profile().to_string();

    if let Ok("msvc") = var("CARGO_CFG_TARGET_ENV").as_deref() {
        if static_crt {
            println!(
                "cargo::warning=`+crt-static` isn't a good idea for `mimalloc-redirect` as that'll disable application-wide redirection."
            )
        }

        config
            .static_crt(static_crt)
            .define("MI_OVERRIDE", "OFF")
            .define("MI_WIN_REDIRECT", "ON")
            .define("MI_BUILD_SHARED", if static_crt { "OFF" } else { "ON" })
            .define("MI_BUILD_STATIC", if static_crt { "ON" } else { "OFF" })
            .define("MI_BUILD_OBJECT", "OFF")
            .define("MI_BUILD_TESTS", "OFF");

        let dst = config.build();

        // Specify linkage search path and link the library.
        println!("cargo::rustc-link-search=native={}/bin", dst.display());
        println!("cargo::rustc-link-search=native={}/lib", dst.display());
        println!("cargo::rustc-link-lib=dylib={}.dll", match &*profile {
            "Debug" => "mimalloc-debug",
            "Release" | "RelWithDebInfo" | "MinSizeRel" => "mimalloc",
            other => {
                println!("cargo::error=Unrecognized CMAKE profile: `{other}`.");
                return
            }
        });
    }
}
