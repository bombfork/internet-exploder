//! # ie-wasm
//!
//! WebAssembly execution via wasmtime.
//! Provides the runtime for `WebAssembly.instantiate()` and JS↔Wasm interop.

use anyhow::Result;
use wasmtime::{Engine, Instance, Linker, Module, Store, Val};

pub struct WasmRuntime {
    engine: Engine,
}

impl WasmRuntime {
    pub fn new() -> Result<Self> {
        let engine = Engine::default();
        Ok(Self { engine })
    }

    /// Instantiate a WebAssembly module from bytes (binary or WAT text).
    pub fn instantiate(&self, wasm_bytes: &[u8]) -> Result<WasmInstance> {
        let module = Module::new(&self.engine, wasm_bytes)?;
        let mut store = Store::new(&self.engine, ());
        let linker = Linker::new(&self.engine);
        let instance = linker.instantiate(&mut store, &module)?;
        Ok(WasmInstance { instance, store })
    }
}

pub struct WasmInstance {
    instance: Instance,
    store: Store<()>,
}

impl WasmInstance {
    /// Call an exported function by name with i32 arguments, returning i32 result.
    pub fn call_i32(&mut self, name: &str, args: &[i32]) -> Result<i32> {
        let func = self
            .instance
            .get_func(&mut self.store, name)
            .ok_or_else(|| anyhow::anyhow!("export '{}' not found", name))?;
        let params: Vec<Val> = args.iter().map(|&a| Val::I32(a)).collect();
        let mut results = vec![Val::I32(0)];
        func.call(&mut self.store, &params, &mut results)?;
        match results[0] {
            Val::I32(v) => Ok(v),
            _ => anyhow::bail!("expected i32 result"),
        }
    }

    /// Call an exported function with no args and no return.
    pub fn call_void(&mut self, name: &str) -> Result<()> {
        let func = self
            .instance
            .get_func(&mut self.store, name)
            .ok_or_else(|| anyhow::anyhow!("export '{}' not found", name))?;
        func.call(&mut self.store, &[], &mut [])?;
        Ok(())
    }

    /// Get list of exported function names.
    pub fn exports(&mut self) -> Vec<String> {
        self.instance
            .exports(&mut self.store)
            .filter_map(|e| {
                let name = e.name().to_string();
                if e.into_func().is_some() {
                    Some(name)
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_wat() -> &'static [u8] {
        b"(module
            (func (export \"add\") (param i32 i32) (result i32)
                local.get 0
                local.get 1
                i32.add)
        )"
    }

    #[test]
    fn runtime_creates() {
        WasmRuntime::new().unwrap();
    }

    #[test]
    fn instantiate_add_module() {
        let rt = WasmRuntime::new().unwrap();
        let mut instance = rt.instantiate(add_wat()).unwrap();
        assert!(instance.exports().contains(&"add".to_string()));
    }

    #[test]
    fn call_add_function() {
        let rt = WasmRuntime::new().unwrap();
        let mut instance = rt.instantiate(add_wat()).unwrap();
        let result = instance.call_i32("add", &[3, 4]).unwrap();
        assert_eq!(result, 7);
    }

    #[test]
    fn invalid_wasm_bytes() {
        let rt = WasmRuntime::new().unwrap();
        let result = rt.instantiate(b"not wasm");
        assert!(result.is_err());
    }

    #[test]
    fn call_nonexistent_export() {
        let rt = WasmRuntime::new().unwrap();
        let mut instance = rt.instantiate(add_wat()).unwrap();
        let result = instance.call_i32("nonexistent", &[]);
        assert!(result.is_err());
    }
}
