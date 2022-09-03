fn main() {
    if cfg!(feature = "pico-explorer") || cfg!(feature = "promicro") {
        println!("cargo:rustc-link-arg=-Tdefmt.x");
    }
}
