fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../branding/frost.ico");
        res.set("FileDescription", "WinIsland");
        res.set("ProductName", "WinIsland");
        res.set("OriginalFilename", "winisland.exe");
        res.set("LegalCopyright", "WinIsland");
        res.compile().expect("failed to compile Windows resources");
    }
    println!("cargo:rerun-if-changed=../branding/frost.ico");
}
