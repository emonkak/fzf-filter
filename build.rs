use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/fzf-native/fzf.c");
    println!("cargo:rerun-if-changed=src/fzf-native/fzf.h");

    let bindings = bindgen::Builder::default()
        .header("src/fzf-native/fzf.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .allowlist_function("fzf_.*")
        .allowlist_type("fzf_.*")
        .allowlist_var("fzf_.*")
        .generate()
        .expect("Unable to generate bindings");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    bindings
        .write_to_file(out_path.join("fzf_sys.rs"))
        .expect("Couldn't write bindings!");

    cc::Build::new()
        .file("src/fzf-native/fzf.c")
        .flag("-Wno-unused-parameter")
        .compile("fzf");
}
