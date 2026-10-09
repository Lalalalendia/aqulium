use wasm_bindgen::prelude::*;

mod kernel;

/// Encodes and transfers a JavaScript string on every call.
#[wasm_bindgen]
pub fn scan_markers(input: &str) -> u32 {
    kernel::scan_bytes(input.as_bytes())
}

/// Keep the UTF-8 representation in Wasm linear memory between calls.
/// The constructor includes the one-time conversion and copy.
#[wasm_bindgen]
pub struct PreloadedScanner {
    data: Vec<u8>,
}
#[wasm_bindgen]
impl PreloadedScanner {
    #[wasm_bindgen(constructor)]
    pub fn new(input: &str) -> PreloadedScanner {
        PreloadedScanner { data: input.as_bytes().to_vec() }
    }
    pub fn scan(&self) -> u32 {
        kernel::scan_bytes(&self.data)
    }
    pub fn byte_len(&self) -> usize {
        self.data.len()
    }
}
