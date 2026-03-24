//! # ie-js
//!
//! JavaScript engine integration using Boa.
//! Provides DOM bindings and Web API surface.

use anyhow::Result;
use boa_engine::Context;

pub struct JsRuntime {
    context: Context,
}

impl JsRuntime {
    pub fn new() -> Result<Self> {
        let context = Context::default();
        Ok(Self { context })
    }

    pub fn eval(&mut self, _source: &str) -> Result<()> {
        todo!("JS evaluation with DOM bindings")
    }
}
