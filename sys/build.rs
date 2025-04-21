#[cfg(not(feature = "bindgen"))]
fn main() {
    println!("cargo:rustc-link-lib=dc_core");
}

#[cfg(feature = "bindgen")]
fn main() {
    println!("cargo:rustc-link-lib=dc_core");
    println!("cargo:rerun-if-changed=../lib/include/dc_core.h");

    let bindings = bindgen::Builder::default()
        .header("../lib/include/dc_core.h")
        .allowlist_file(".+/dc_core.h")
        .use_core()
        .override_abi(bindgen::Abi::CUnwind, "Dc.+Func")
        .prepend_enum_name(false)
        .generate()
        .expect("Unable to generate bindings");

    let mut buf = Vec::new();
    bindings
        .write(Box::new(&mut buf))
        .expect("Couldn't write bindings");
    let s = replace(String::from_utf8(buf).unwrap());
    std::fs::write("src/bindings.rs", s).expect("Cannot write");
}

// Replace _ptr in DcMsg to NonNull
#[cfg(feature = "bindgen")]
fn replace(s: String) -> String {
    let mut buf = String::new();

    let mut dc_msg_found = false;
    for line in s.lines() {
        if dc_msg_found && line.trim_start().starts_with("pub _ptr") {
            buf.push_str("// ");
            buf.push_str(line);
            buf.push('\n');
            buf.push_str("    pub _ptr: ::core::ptr::NonNull<::core::ffi::c_void>,\n");
            dc_msg_found = false;
            continue;
        }

        if line.starts_with("pub struct DcMsg") {
            dc_msg_found = true;
        }
        buf.push_str(line);
        buf.push('\n');
    }

    buf
}
