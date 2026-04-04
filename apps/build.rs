fn main() {
    let compile_result = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| slint_build::compile("ui/app-window.slint"))
        .expect("Failed to spawn Slint build thread")
        .join()
        .expect("Slint build thread panicked");
    compile_result.expect("Slint build failed");

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("resources/icons/icon.ico");
        res.set("ProductName", "Snappix");
        res.set("FileDescription", "Snappix Editor");
        res.set("InternalName", "Snappix");
        res.set("OriginalFilename", "snappix.exe");
        res.compile().expect("Windows resources failed");
    }
}
