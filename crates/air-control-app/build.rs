fn main() {
    slint_build::compile("ui/app.slint").expect("compile AirControl interface");
    #[cfg(windows)]
    embed_windows_icon();
}

#[cfg(windows)]
fn embed_windows_icon() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("ui/icon.ico");
    res.set("ProductName", "AirControl");
    res.set("FileDescription", "AirControl — жесты вместо мыши");
    res.set("LegalCopyright", "AirControl");
    if let Err(err) = res.compile() {
        println!("cargo:warning=window icon was not embedded: {err}");
    }
}
