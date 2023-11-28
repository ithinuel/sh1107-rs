cfg_if::cfg_if! {
    if #[cfg(feature = "pico-explorer")] {
        include!("pico-explorer-boilerplate.rs");
    } else if #[cfg(feature = "pico-explorer-pio")] {
        include!("pico-explorer-pio-boilerplate.rs");
    } else if #[cfg(feature = "pico-explorer-minimal")] {
        include!("pico-explorer-minimal-boilerplate.rs");
    } else if #[cfg(feature = "promicro")] {
        include!("promicro-rp2040-boilerplate.rs");
    } else if #[cfg(feature = "rpi-pico")] {
        include!("rpi-pico-boilerplate.rs");
    } else {
        compile_error!("One platform feature must be selected");
    }
}

