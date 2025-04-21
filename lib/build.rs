use cbindgen::{Builder, Config, EnumConfig, ExportConfig, Language};

const HEADER_FILE: &str = "include/dc_core.h";

fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    let mut config = Config::default();

    config.language = Language::C;
    config.cpp_compat = true;
    config.pragma_once = false;
    config.enumeration = EnumConfig {
        prefix_with_name: true,
        ..EnumConfig::default()
    };
    config.export = ExportConfig {
        include: vec!["META_DATA_SIZE".into(), "DcPlugin".into()],
        ..ExportConfig::default()
    };

    let mut buf = Vec::new();

    Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .expect("Unable to generate bindings")
        .write(&mut buf);

    let replaced = replace_opaque_structs(String::from_utf8(buf).unwrap());
    std::fs::write(HEADER_FILE, replaced).unwrap();
}

fn replace_opaque_structs(s: String) -> String {
    let regex =
        regex::Regex::new(r"typedef struct \w+ \{\s*uint8_t _data\[0\];\s*\} (\w+);").unwrap();

    regex.replace_all(&s, "typedef struct $1 $1;").to_string()
}
