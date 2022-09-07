fn main() {
    if cfg!(feature = "rpi-pico") || cfg!(feature = "pico-explorer") || cfg!(feature = "promicro") {
        println!("cargo:rustc-link-arg=-Tdefmt.x");
    }
}
