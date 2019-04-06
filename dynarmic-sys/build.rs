use cmake::Config;

fn main() {
    let dst = Config::new(".")
        .build();
    println!("cargo:rustc-link-search=native={}", dst.display());
    println!("cargo:rustc-link-lib=static=dynarmic_wrapper");
    println!("cargo:rustc-link-lib=static=dynarmic");
    println!("cargo:rustc-link-lib=static=fmtd");
}
