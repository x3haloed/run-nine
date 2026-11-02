fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-arg=-T{dir}/link.ld");
    println!("cargo:rustc-link-arg=-z");
    println!("cargo:rustc-link-arg=max-page-size=4096");
    println!("cargo:rerun-if-changed=link.ld");
    println!("cargo:rerun-if-changed=src/boot.S");
}
