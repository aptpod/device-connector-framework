use cbindgen::{Builder, Config, EnumConfig, ExportConfig, Language};

fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

    let config = Config {
        language: Language::C,
        cpp_compat: true,
        pragma_once: true,
        enumeration: EnumConfig {
            prefix_with_name: true,
            ..EnumConfig::default()
        },
        export: ExportConfig {
            include: vec!["DcPlugin".into()],
            ..ExportConfig::default()
        },
        ..Config::default()
    };

    Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file("include/device_connector.h");
}
