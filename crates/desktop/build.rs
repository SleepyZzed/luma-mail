fn main() {
    glib_build_tools::compile_resources(
        &["../../data"],
        "../../data/luma.gresource.xml",
        "luma.gresource",
    );
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=../../data/windows.rc");
        println!("cargo:rerun-if-changed=../../data/luma.ico");
        let object = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap())
            .join("windows-resources.o");
        let status = std::process::Command::new("windres")
            .args([
                "-I",
                "../../data",
                "-i",
                "../../data/windows.rc",
                "-O",
                "coff",
                "-o",
            ])
            .arg(&object)
            .status()
            .expect("windres is required for the Windows icon and version resources");
        assert!(status.success(), "Windows resources failed");
        println!("cargo:rustc-link-arg={}", object.display());
    }
}
