//! Timer globals: setTimeout, clearTimeout, setInterval, clearInterval.
//!
//! For v1, `setTimeout` executes its callback immediately (no real delay).
//! `setInterval` is a no-op stub that returns an id.

use std::sync::atomic::{AtomicI32, Ordering};

use boa_engine::{Context, JsValue, NativeFunction, js_string};

static TIMEOUT_NEXT_ID: AtomicI32 = AtomicI32::new(1);
static INTERVAL_NEXT_ID: AtomicI32 = AtomicI32::new(10_000);

/// Register `setTimeout`, `clearTimeout`, `setInterval`, `clearInterval` globals.
pub fn register_timers(context: &mut Context) {
    let set_timeout = NativeFunction::from_copy_closure(|_this, args, ctx| {
        let callback = args.first().cloned().unwrap_or(JsValue::undefined());
        // v1: execute immediately, ignore delay
        if let Some(callable) = callback.as_callable() {
            let _ = callable.call(&JsValue::undefined(), &[], ctx);
        }
        let id = TIMEOUT_NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Ok(JsValue::from(id))
    });

    let clear_timeout =
        NativeFunction::from_copy_closure(|_this, _args, _ctx| Ok(JsValue::undefined()));

    let set_interval = NativeFunction::from_copy_closure(|_this, _args, _ctx| {
        let id = INTERVAL_NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Ok(JsValue::from(id))
    });

    let clear_interval =
        NativeFunction::from_copy_closure(|_this, _args, _ctx| Ok(JsValue::undefined()));

    let _ = context.register_global_callable(js_string!("setTimeout"), 1, set_timeout);
    let _ = context.register_global_callable(js_string!("clearTimeout"), 1, clear_timeout);
    let _ = context.register_global_callable(js_string!("setInterval"), 2, set_interval);
    let _ = context.register_global_callable(js_string!("clearInterval"), 1, clear_interval);
}
