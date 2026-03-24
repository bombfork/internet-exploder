//! # ie-wasm
//!
//! WebAssembly execution via wasmtime.
//! Provides the runtime for `WebAssembly.instantiate()` and JS↔Wasm interop.

use anyhow::Result;
use wasmtime::{Engine, Module, Store};

pub struct WasmRuntime {
    engine: Engine,
}

impl WasmRuntime {
    pub fn new() -> Result<Self> {
        let engine = Engine::default();
        Ok(Self { engine })
    }

    pub fn instantiate(&self, _wasm_bytes: &[u8]) -> Result<WasmInstance> {
        let _module = Module::new(&self.engine, _wasm_bytes)?;
        let _store = Store::new(&self.engine, ());
        todo!("Instantiate module with imports and memory")
    }
}

pub struct WasmInstance {
    _store: Store<()>,
}
