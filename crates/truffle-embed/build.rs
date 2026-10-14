//! Builds llama.cpp (third_party/llama.cpp, pinned at b11461 = 4d756bc) as static libraries with DigUp's no-logits
//! patch, then the C shim against its headers. One binary, nothing to ship beside it:
//! - macOS: Metal, its shaders embedded in the library (compiled on first use), plus Accelerate for BLAS;
//! - Linux: the CPU backend for a portable baseline (x86-64-v3: AVX2, FMA, F16C; arm64: NEON + dotprod),
//!   without OpenMP so no libgomp is needed; `--features vulkan` or `cuda` add a GPU.
//! Vectors must not change across builds: tests/reference.rs checks them against DigUp's.

use std::path::{Path, PathBuf};
use std::process::Command;

const COMMIT: &str = "4d756bc72bf00a4aacf410ae15a2d315f3db400d";

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let source = std::env::var_os("TRUFFLE_LLAMA_SOURCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../third_party/llama.cpp"));
    if !source.join("include/llama.h").is_file() {
        panic!("no llama.cpp in {}: run `git submodule update --init`", source.display());
    }
    check_commit(&source);
    patch(&source, &manifest.join("llama.cpp-no-logits.patch"));

    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let mut cfg = cmake::Config::new(&source);
    cfg.profile("Release")
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("GGML_NATIVE", "OFF")
        .define("GGML_OPENMP", "OFF")
        .define("LLAMA_BUILD_MTMD", "ON")
        .define("LLAMA_BUILD_COMMON", "OFF")
        .define("LLAMA_BUILD_TOOLS", "OFF")
        .define("LLAMA_BUILD_SERVER", "OFF")
        .define("LLAMA_BUILD_EXAMPLES", "OFF")
        .define("LLAMA_BUILD_TESTS", "OFF")
        .define("LLAMA_BUILD_APP", "OFF")
        .define("LLAMA_BUILD_UI", "OFF")
        .define("LLAMA_OPENSSL", "OFF")
        .define("LLAMA_CURL", "OFF")
        .build_target("mtmd");
    if os == "macos" {
        cfg.define("GGML_METAL", "ON").define("GGML_METAL_EMBED_LIBRARY", "ON").define("CMAKE_OSX_DEPLOYMENT_TARGET", "13.0");
    } else {
        if arch == "x86_64" {
            for flag in ["GGML_SSE42", "GGML_AVX", "GGML_AVX2", "GGML_FMA", "GGML_F16C", "GGML_BMI2"] {
                cfg.define(flag, "ON");
            }
        }
        if std::env::var_os("CARGO_FEATURE_VULKAN").is_some() {
            cfg.define("GGML_VULKAN", "ON");
        }
        if std::env::var_os("CARGO_FEATURE_CUDA").is_some() {
            cfg.define("GGML_CUDA", "ON");
        }
    }
    if let Ok(jobs) = std::env::var("NUM_JOBS") {
        cfg.env("CMAKE_BUILD_PARALLEL_LEVEL", jobs);
    }
    let dst = cfg.build();
    let build = dst.join("build");

    // Static libraries wherever CMake put them (single- or multi-config generators).
    let mut dirs = Vec::new();
    collect_lib_dirs(&build, &mut dirs);
    for d in &dirs {
        println!("cargo:rustc-link-search=native={}", d.display());
    }
    let mut libs = vec!["mtmd", "llama", "ggml", "ggml-cpu", "ggml-base"];
    if os == "macos" {
        libs.splice(3..3, ["ggml-metal", "ggml-blas"]);
    }
    if std::env::var_os("CARGO_FEATURE_VULKAN").is_some() {
        libs.insert(3, "ggml-vulkan");
    }
    if std::env::var_os("CARGO_FEATURE_CUDA").is_some() {
        libs.insert(3, "ggml-cuda");
    }
    for lib in libs {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    if dirs.iter().any(|d| d.join("libvendor-hash.a").is_file()) {
        println!("cargo:rustc-link-lib=static=vendor-hash");
    }
    if os == "macos" {
        for fw in ["Metal", "MetalKit", "Foundation", "Accelerate"] {
            println!("cargo:rustc-link-lib=framework={fw}");
        }
        println!("cargo:rustc-link-lib=dylib=c++");
        // Objective-C's @available calls __isPlatformVersionAtLeast, from clang's runtime, which clang adds when it
        // links and rustc's link line leaves out.
        if let Ok(out) = Command::new("clang").arg("--print-runtime-dir").output()
            && out.status.success()
        {
            let dir = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if Path::new(&dir).join("libclang_rt.osx.a").is_file() {
                println!("cargo:rustc-link-search=native={dir}");
                println!("cargo:rustc-link-lib=static=clang_rt.osx");
            }
        }
    } else {
        println!("cargo:rustc-link-lib=dylib=stdc++");
        if std::env::var_os("CARGO_FEATURE_VULKAN").is_some() {
            println!("cargo:rustc-link-lib=dylib=vulkan");
        }
    }

    cc::Build::new()
        .file(manifest.join("shim/truffle_llama.c"))
        .include(source.join("include"))
        .include(source.join("ggml/include"))
        .include(source.join("tools/mtmd"))
        .warnings(false)
        .compile("truffle_llama_shim");
    println!("cargo:rerun-if-changed=shim/truffle_llama.c");
    println!("cargo:rerun-if-changed=llama.cpp-no-logits.patch");
    println!("cargo:rerun-if-env-changed=TRUFFLE_LLAMA_SOURCE");
}

fn check_commit(source: &Path) {
    let head = Command::new("git").arg("-C").arg(source).args(["rev-parse", "HEAD"]).output();
    if let Ok(out) = head
        && out.status.success()
    {
        let head = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if head != COMMIT {
            panic!("llama.cpp is at {head}, not {COMMIT} (b11461): vectors would differ from the index's");
        }
    }
}

/// Without the patch, every input token costs 1 MiB of logits an embedding model never produces (262k vocabulary).
/// Applied in place, once: `git apply --check` fails when it already is.
fn patch(source: &Path, patch: &Path) {
    let git = |args: &[&str]| Command::new("git").arg("-C").arg(source).args(args).arg(patch).output().is_ok_and(|o| o.status.success());
    if git(&["apply", "--check"]) {
        assert!(git(&["apply"]), "applying {} failed", patch.display());
    } else if !git(&["apply", "--reverse", "--check"]) {
        panic!("{} neither applies nor is applied", patch.display());
    }
}

fn collect_lib_dirs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut has_lib = false;
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_lib_dirs(&p, out);
        } else if p.extension().is_some_and(|x| x == "a") {
            has_lib = true;
        }
    }
    if has_lib {
        out.push(dir.to_path_buf());
    }
}
