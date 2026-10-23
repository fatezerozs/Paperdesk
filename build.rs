fn main() {
    println!("cargo:rerun-if-changed=assets/paperdesk.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let icon = std::fs::canonicalize("assets/paperdesk.ico").unwrap();
        let script = out.join("paperdesk.rc");
        let resource = out.join("paperdesk.res");
        std::fs::write(
            &script,
            format!(
                "1 ICON \"{}\"\n",
                icon.display().to_string().replace('\\', "\\\\")
            ),
        )
        .unwrap();
        let sdk = std::path::Path::new("C:/Program Files (x86)/Windows Kits/10/bin");
        let mut compilers: Vec<_> = std::fs::read_dir(sdk)
            .expect("Windows SDK required")
            .filter_map(Result::ok)
            .map(|p| p.path().join("x64/rc.exe"))
            .filter(|p| p.is_file())
            .collect();
        compilers.sort();
        let status =
            std::process::Command::new(compilers.last().expect("Windows SDK rc.exe required"))
                .arg("/nologo")
                .arg(format!("/fo{}", resource.display()))
                .arg(script)
                .status()
                .unwrap();
        assert!(status.success(), "Windows icon compilation failed");
        println!("cargo:rustc-link-arg={}", resource.display());
    }
}
