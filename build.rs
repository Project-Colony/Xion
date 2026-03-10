fn main() {
    #[cfg(target_os = "windows")]
    {
        let _ = embed_resource::compile("xion.rc", embed_resource::NONE);
    }
}
