#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let fmt = citrine_core::formats::format_by_id("kitty").unwrap();
        let _ = fmt.import(s);
    }
});
