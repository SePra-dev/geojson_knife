fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("FileDescription", "GeoJSON Toolkit");
        res.set("ProductName", "GeoJSON Toolkit");
        res.set("LegalCopyright", "Apache License 2.0");
        match res.compile() {
            Ok(_) => println!("cargo:rerun-if-changed=assets/icon.ico"),
            Err(e) => {
                eprintln!("warning: failed to embed Windows resources: {e}");
            }
        }
    }
}
