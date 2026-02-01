pub struct WasmRuntime;

impl WasmRuntime {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new()
    }
}
