fn main() {
    println!("cargo:rerun-if-changed=assets/icon/opentrack.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon/opentrack.ico");
        res.compile().expect("could not embed the Windows icon");
    }
}
