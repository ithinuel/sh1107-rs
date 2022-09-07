fn main() {
    if cfg!(feature = "pico-explorer")
        || cfg!(feature = "pico-explorer-pio")
        || cfg!(feature = "pico-explorer-minimal")
        || cfg!(feature = "promicro")
        || cfg!(feature = "rpi-pico")
    {
        println!("cargo:rustc-link-arg=-Tdefmt.x");
    }
}
