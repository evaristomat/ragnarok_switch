//! Compile-time Brazilian Portuguese labels for the Horizon build.
//! Translating at the call site preserves player names and layout measurements.
#[macro_export]
macro_rules! pt {
    ($source:literal, $translated:literal) => {{
        #[cfg(target_os = "horizon")] { $translated }
        #[cfg(not(target_os = "horizon"))] { $source }
    }};
}
#[macro_export]
macro_rules! pt_format {
    ($source:literal, $translated:literal $(, $($args:tt)*)?) => {{
        #[cfg(target_os = "horizon")] { format!($translated $(, $($args)*)?) }
        #[cfg(not(target_os = "horizon"))] { format!($source $(, $($args)*)?) }
    }};
}
