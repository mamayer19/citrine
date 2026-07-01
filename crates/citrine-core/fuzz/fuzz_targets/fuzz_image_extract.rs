#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = citrine_core::helpers::extract_palette(data, 8);
});
