fn main() {
    // Only compile resources on Windows
    #[cfg(target_os = "windows")]
    {
        embed_resource::compile("app.rc", embed_resource::NONE);
    }

    // Compile Slint UI files
    slint_build::compile("ui/main.slint").unwrap();
}
