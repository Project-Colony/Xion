//! Build script: embeds the Windows resources into the executable.
//!
//! Two resources: the application manifest (`xion.manifest`), and a version
//! resource. SignPath signs only an .exe whose ProductName is the project name
//! and whose ProductVersion is set, and Windows shows FileDescription as the
//! program's name in Task Manager and file dialogs.

fn main() {
    println!("cargo:rerun-if-changed=xion.manifest");

    // `#[cfg(target_os = "windows")]` in a build script describes the HOST, not
    // the target, so it silently skips the resource when cross-compiling to
    // Windows from Linux. `CARGO_CFG_TARGET_OS` is the target.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    // Generated rather than written by hand, so the version in the resource is
    // always the one release-please wrote to Cargo.toml. Forward slashes: the
    // resource compiler accepts them, and they need no escaping.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let manifest = format!("{manifest_dir}/xion.manifest").replace('\\', "/");
    let env = |name: &str| std::env::var(name).expect(name);
    let (major, minor, patch) = (
        env("CARGO_PKG_VERSION_MAJOR"),
        env("CARGO_PKG_VERSION_MINOR"),
        env("CARGO_PKG_VERSION_PATCH"),
    );
    let version = env("CARGO_PKG_VERSION");
    let rc = format!(
        r#"1 24 "{manifest}"

1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "Project Colony"
      VALUE "FileDescription", "Xion"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "xion"
      VALUE "LegalCopyright", "GPL-3.0-or-later"
      VALUE "OriginalFilename", "xion.exe"
      VALUE "ProductName", "Xion"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    );
    let out = std::path::Path::new(&env("OUT_DIR")).join("xion.rc");
    std::fs::write(&out, rc).expect("writing xion.rc");

    // `CompilationResult` is `#[must_use]`: a broken resource must not pass
    // silently, or the produced binary would be missing its manifest.
    let result = embed_resource::compile(&out, embed_resource::NONE);
    match result {
        // `manifest_required()` would also reject NotAttempted, which is what a
        // Linux host without a resource compiler reports. That would make
        // `cargo check --target x86_64-pc-windows-msvc` impossible from Linux,
        // so NotAttempted only warns; a real compilation failure still aborts.
        embed_resource::CompilationResult::NotAttempted(reason) => {
            println!(
                "cargo:warning=xion.rc not compiled ({reason}); the binary will have no manifest"
            );
        }
        embed_resource::CompilationResult::Failed(error) => {
            panic!("compiling xion.rc failed: {error}");
        }
        embed_resource::CompilationResult::Ok | embed_resource::CompilationResult::NotWindows => {}
    }
}
