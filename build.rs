use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/active.ico");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR 누락"));
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR 누락"));
    let target_env = env::var("CARGO_CFG_TARGET_ENV").expect("CARGO_CFG_TARGET_ENV 누락");

    let mut resources = winres::WindowsResource::new();
    resources.set_icon("assets/active.ico");

    if target_env == "gnu" {
        compile_gnu_resource(&resources, &out_dir, &manifest_dir)
            .expect("GNU Windows 리소스 아이콘 포함 실패");
    } else {
        resources
            .compile()
            .expect("Windows 리소스 아이콘 포함 실패");
    }
}

fn compile_gnu_resource(
    resources: &winres::WindowsResource,
    out_dir: &Path,
    manifest_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let rc_path = out_dir.join("resource.rc");
    let object_path = out_dir.join("resource.o");

    resources.write_resource_file(&rc_path)?;

    let status = Command::new("windres")
        .current_dir(manifest_dir)
        .arg(format!("-I{}", manifest_dir.display()))
        .arg("--input-format=rc")
        .arg("--output-format=coff")
        .arg("-i")
        .arg(&rc_path)
        .arg("-o")
        .arg(&object_path)
        .status()?;

    if !status.success() {
        return Err("windres 리소스 컴파일 실패".into());
    }

    println!("cargo:rustc-link-arg-bins={}", object_path.display());

    Ok(())
}
