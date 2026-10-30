fn main() {
    slint_build::compile("ui/app.slint").expect("compile AIR CONTROL interface");
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("ui/icon.ico");
        res.set("ProductName", "AIR CONTROL");
        res.set("FileDescription", "AIR CONTROL — Feel the digital world");
        res.set("LegalCopyright", "AIR CONTROL");
        if let Err(err) = res.compile() {
            println!("cargo:warning=window icon was not embedded: {err}");
        }
    }
}
