#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(src) = std::str::from_utf8(data) {
        let mut vm = ruja::Vm::new();
        vm.set_fuel(Some(100_000));
        let _ = vm.run(src);
    }
});
