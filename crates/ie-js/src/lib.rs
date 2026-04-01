//! # ie-js
//!
//! JavaScript engine integration using Boa.
//! Provides DOM bindings and Web API surface.

mod console;

use anyhow::Result;
use boa_engine::{Context, Source};

pub struct JsRuntime {
    context: Context,
}

impl JsRuntime {
    pub fn new() -> Result<Self> {
        let mut context = Context::default();
        console::register_console(&mut context);
        Ok(Self { context })
    }

    /// Execute a JavaScript source string. Returns Ok on success, Err on JS error.
    pub fn execute(&mut self, source: &str) -> Result<()> {
        match self.context.eval(Source::from_bytes(source)) {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = e.to_string();
                tracing::warn!("JS error: {msg}");
                Err(anyhow::anyhow!("JS error: {msg}"))
            }
        }
    }

    /// Execute and return the string representation of the result.
    pub fn eval(&mut self, source: &str) -> Result<String> {
        match self.context.eval(Source::from_bytes(source)) {
            Ok(val) => {
                let result = val
                    .to_string(&mut self.context)
                    .map(|s| s.to_std_string_escaped())
                    .unwrap_or_else(|_| "undefined".to_string());
                Ok(result)
            }
            Err(e) => {
                let msg = e.to_string();
                tracing::warn!("JS error: {msg}");
                Err(anyhow::anyhow!("JS error: {msg}"))
            }
        }
    }
}

/// Execute all script contents from a parsed HTML page.
pub fn execute_scripts(scripts: &[String]) -> Vec<String> {
    let mut errors = Vec::new();
    let Ok(mut runtime) = JsRuntime::new() else {
        errors.push("failed to create JS runtime".to_string());
        return errors;
    };
    for script in scripts {
        if let Err(e) = runtime.execute(script) {
            errors.push(e.to_string());
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_creates() {
        JsRuntime::new().unwrap();
    }

    #[test]
    fn simple_eval() {
        let mut rt = JsRuntime::new().unwrap();
        let result = rt.eval("1 + 2").unwrap();
        assert_eq!(result, "3");
    }

    #[test]
    fn console_log_does_not_crash() {
        let mut rt = JsRuntime::new().unwrap();
        rt.execute("console.log('hello from JS')").unwrap();
    }

    #[test]
    fn console_warn_error_info() {
        let mut rt = JsRuntime::new().unwrap();
        rt.execute("console.warn('warning'); console.error('error'); console.info('info')")
            .unwrap();
    }

    #[test]
    fn js_error_does_not_panic() {
        let mut rt = JsRuntime::new().unwrap();
        let result = rt.execute("undefined_function()");
        assert!(result.is_err());
    }

    #[test]
    fn string_operations() {
        let mut rt = JsRuntime::new().unwrap();
        let result = rt.eval("'hello' + ' ' + 'world'").unwrap();
        assert_eq!(result, "hello world");
    }

    #[test]
    fn execute_scripts_helper() {
        let scripts = vec![
            "console.log('script 1')".to_string(),
            "var x = 42".to_string(),
            "bad syntax {{{{".to_string(),
        ];
        let errors = execute_scripts(&scripts);
        assert_eq!(errors.len(), 1); // only the bad syntax one
    }
}
