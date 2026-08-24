//! Build script: embeds the Windows resource file into the executable.

fn main() {
    println!("cargo:rerun-if-changed=xion.rc");
    println!("cargo:rerun-if-changed=xion.manifest");

    // `#[cfg(target_os = "windows")]` in a build script describes the HOST, not
    // the target, so it silently skips the resource when cross-compiling to
    // Windows from Linux. `CARGO_CFG_TARGET_OS` is the target.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    // `CompilationResult` is `#[must_use]`: a broken xion.rc must not pass
    // silently, or the produced binary would be missing its manifest.
    let result = embed_resource::compile("xion.rc", embed_resource::NONE);
    match result {
        // `manifest_required()` would also reject NotAttempted, which is what a
        // Linux host without a resource compiler reports. That would make
        // `cargo check --target x86_64-pc-windows-msvc` impossible from Linux,
        // so NotAttempted only warns; a real compilation failure still aborts.
        embed_resource::CompilationResult::NotAttempted(reason) => {
            println!(
                "cargo:warning=xion.rc non compilé ({reason}); le binaire n'aura pas de manifeste"
            );
        }
        embed_resource::CompilationResult::Failed(error) => {
            panic!("échec de la compilation de xion.rc : {error}");
        }
        embed_resource::CompilationResult::Ok | embed_resource::CompilationResult::NotWindows => {}
    }
}
