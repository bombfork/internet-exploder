use boa_engine::{
    Context, JsValue, NativeFunction, js_string, object::ObjectInitializer, property::Attribute,
};

/// Register the `console` global object with log/warn/error/info methods.
pub fn register_console(context: &mut Context) {
    let log = NativeFunction::from_copy_closure(|_this, args, _ctx| {
        let msg = format_args_to_string(args);
        tracing::info!(target: "ie_js::console", "{msg}");
        Ok(JsValue::undefined())
    });

    let warn = NativeFunction::from_copy_closure(|_this, args, _ctx| {
        let msg = format_args_to_string(args);
        tracing::warn!(target: "ie_js::console", "{msg}");
        Ok(JsValue::undefined())
    });

    let error = NativeFunction::from_copy_closure(|_this, args, _ctx| {
        let msg = format_args_to_string(args);
        tracing::error!(target: "ie_js::console", "{msg}");
        Ok(JsValue::undefined())
    });

    let info = NativeFunction::from_copy_closure(|_this, args, _ctx| {
        let msg = format_args_to_string(args);
        tracing::info!(target: "ie_js::console", "{msg}");
        Ok(JsValue::undefined())
    });

    let console = ObjectInitializer::new(context)
        .function(log, js_string!("log"), 0)
        .function(warn, js_string!("warn"), 0)
        .function(error, js_string!("error"), 0)
        .function(info, js_string!("info"), 0)
        .build();

    let _ = context.register_global_property(js_string!("console"), console, Attribute::all());
}

fn format_args_to_string(args: &[JsValue]) -> String {
    args.iter()
        .map(|v| match v {
            JsValue::String(s) => s.to_std_string_escaped(),
            JsValue::Integer(n) => n.to_string(),
            JsValue::Rational(n) => n.to_string(),
            JsValue::Boolean(b) => b.to_string(),
            JsValue::Null => "null".to_string(),
            JsValue::Undefined => "undefined".to_string(),
            _ => format!("{v:?}"),
        })
        .collect::<Vec<_>>()
        .join(" ")
}
