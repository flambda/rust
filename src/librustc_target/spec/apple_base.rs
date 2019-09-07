use std::{env, path::Path};

use crate::spec::{LinkArgs, TargetOptions};

pub fn opts() -> TargetOptions {
    // ELF TLS is only available in macOS 10.7+. If you try to compile for 10.6
    // either the linker will complain if it is used or the binary will end up
    // segfaulting at runtime when run on 10.6. Rust by default supports macOS
    // 10.7+, but there is a standard environment variable,
    // MACOSX_DEPLOYMENT_TARGET, which is used to signal targeting older
    // versions of macOS. For example compiling on 10.10 with
    // MACOSX_DEPLOYMENT_TARGET set to 10.6 will cause the linker to generate
    // warnings about the usage of ELF TLS.
    //
    // Here we detect what version is being requested, defaulting to 10.7. ELF
    // TLS is flagged as enabled if it looks to be supported.
    let version = macos_deployment_target();

    TargetOptions {
        // macOS has -dead_strip, which doesn't rely on function_sections
        function_sections: false,
        dynamic_linking: true,
        executables: true,
        target_family: Some("unix".to_string()),
        is_like_osx: true,
        has_rpath: true,
        dll_prefix: "lib".to_string(),
        dll_suffix: ".dylib".to_string(),
        archive_format: "bsd".to_string(),
        pre_link_args: LinkArgs::new(),
        has_elf_tls: version >= (10, 7),
        abi_return_struct_as_int: true,
        emit_debug_gdb_scripts: false,
        .. Default::default()
    }
}

fn macos_deployment_target() -> (u32, u32) {
    let deployment_target = env::var("MACOSX_DEPLOYMENT_TARGET").ok();
    let version = deployment_target.as_ref().and_then(|s| {
        let mut i = s.splitn(2, '.');
        i.next().and_then(|a| i.next().map(|b| (a, b)))
    }).and_then(|(a, b)| {
        a.parse::<u32>().and_then(|a| b.parse::<u32>().map(|b| (a, b))).ok()
    });

    version.unwrap_or((10, 7))
}

pub fn macos_llvm_target(arch: &str) -> String {
    let (major, minor) = macos_deployment_target();
    format!("{}-apple-macosx{}.{}.0", arch, major, minor)
}

#[cfg(target_os = "macos")]
pub fn sysroot(sdk: &str) -> Result<Option<String>, String> {
    // Like Clang, allow the `SDKROOT` environment variable used by Xcode to define the sysroot.
    if let Some(sdk_root) = env::var("SDKROOT").ok() {
        let actual_sdk_path = sdk_path(sdk)?;
        let sdk_root_p = Path::new(&sdk_root);
        // Ignore `SDKROOT` if it's not a valid path.
        if !sdk_root_p.is_absolute() || sdk_root_p == Path::new("/") || !sdk_root_p.exists() {
            return Ok(Some(actual_sdk_path));
        }
        // Ignore `SDKROOT` if it's clearly set for the wrong platform, which may occur when we're
        // compiling a custom build script while targeting iOS for example.
        return Ok(Some(match sdk {
            "iphoneos" if sdk_root.contains("iPhoneSimulator.platform")
                || sdk_root.contains("MacOSX.platform") => actual_sdk_path,
            "iphonesimulator" if sdk_root.contains("iPhoneOS.platform")
                || sdk_root.contains("MacOSX.platform") => actual_sdk_path,
            "macosx" | "macosx10.15" if sdk_root.contains("iPhoneOS.platform")
                || sdk_root.contains("iPhoneSimulator.platform") => actual_sdk_path,
            _ => sdk_root,
        }))
    }
    Ok(None)
}

// `xcrun` is only available on macOS.
#[cfg(not(target_os = "macos"))]
pub fn sysroot(_sdk: &str) -> Result<Option<String>, String> {
    if let Some(sdk_root) = env::var("SDKROOT").ok() {
        let sdk_root_p = Path::new(&sdk_root);
        // Use `SDKROOT` only if it's a valid path.
        if sdk_root_p.is_absolute() && sdk_root_p != Path::new("/") && sdk_root_p.exists() {
            return Ok(Some(sdk_root));
        }
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
fn sdk_path(sdk_name: &str) -> Result<String, String> {
    let res = std::process::Command::new("xcrun")
        .arg("--show-sdk-path")
        .arg("-sdk")
        .arg(sdk_name)
        .output()
        .and_then(|output| {
            if output.status.success() {
                Ok(String::from_utf8(output.stdout).unwrap())
            } else {
                let error = String::from_utf8(output.stderr);
                let error = format!("process exit with error: {}", error.unwrap());
                Err(std::io::Error::new(std::io::ErrorKind::Other, &error[..]))
            }
        });
    match res {
        Ok(output) => Ok(output.trim().to_string()),
        Err(e) => Err(format!("failed to get {} SDK path: {}", sdk_name, e)),
    }
}
