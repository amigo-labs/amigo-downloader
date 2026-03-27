//! QuickJS-NG engine wrapper.
//!
//! Manages the QuickJS runtime and provides isolated contexts for each plugin.

use std::time::Duration;

use rquickjs::{Context, Ctx, Function, Object, Runtime, Value};

/// Engine configuration.
pub struct EngineConfig {
    /// Max memory across all plugin contexts.
    pub max_memory: usize,
    /// Max stack size per context.
    pub max_stack_size: usize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_memory: 256 * 1024 * 1024, // 256MB
            max_stack_size: 1024 * 1024,   // 1MB
        }
    }
}

/// Central QuickJS-NG engine — one runtime, multiple contexts.
pub struct PluginEngine {
    runtime: Runtime,
}

impl PluginEngine {
    pub fn new(config: EngineConfig) -> Result<Self, crate::Error> {
        let runtime = Runtime::new().map_err(|e| {
            crate::Error::Execution(format!("Failed to create QuickJS runtime: {e}"))
        })?;

        runtime.set_memory_limit(config.max_memory);
        runtime.set_max_stack_size(config.max_stack_size);

        Ok(Self { runtime })
    }

    /// Create a new isolated context for a plugin.
    pub fn create_context(&self) -> Result<PluginContext, crate::Error> {
        let context = Context::full(&self.runtime).map_err(|e| {
            crate::Error::Execution(format!("Failed to create QuickJS context: {e}"))
        })?;

        Ok(PluginContext { context })
    }
}

/// An isolated QuickJS context for a single plugin.
pub struct PluginContext {
    context: Context,
}

impl PluginContext {
    /// Execute a block of code within this context.
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(Ctx<'_>) -> R,
    {
        self.context.with(f)
    }

    /// Load and evaluate JavaScript source code.
    pub fn eval_source(&self, source: &str, filename: &str) -> Result<(), crate::Error> {
        self.context.with(|ctx| {
            ctx.eval::<Value<'_>, _>(source).map_err(|e| {
                crate::Error::Execution(format!("JS eval error in {filename}: {e}"))
            })?;
            Ok(())
        })
    }

    /// Call a function stored in `__plugin_exports` and return the result as a string.
    pub fn call_export_string(&self, func_name: &str) -> Result<String, crate::Error> {
        self.context.with(|ctx| {
            let global = ctx.globals();
            let exports: Object<'_> = global
                .get("__plugin_exports")
                .map_err(|e| crate::Error::Execution(format!("__plugin_exports not found: {e}")))?;
            let func: Function<'_> = exports.get(func_name).map_err(|e| {
                crate::Error::Execution(format!("Export {func_name} not found: {e}"))
            })?;
            let result: String = func
                .call(())
                .map_err(|e| crate::Error::Execution(format!("{func_name}() failed: {e}")))?;
            Ok(result)
        })
    }

    /// Call the async `resolve(url)` function and return the result as JSON string.
    /// The JS function returns a Promise which we resolve synchronously via the event loop.
    pub fn call_resolve(&self, url: &str, timeout: Duration) -> Result<String, crate::Error> {
        let url = url.to_string();

        // We wrap the resolve call in a script that catches the Promise result
        let script = format!(
            r#"
            (function() {{
                var __resolve_result = null;
                var __resolve_error = null;
                var __resolve_done = false;

                var p = __plugin_exports.resolve("{url}");
                if (p && typeof p.then === 'function') {{
                    p.then(function(r) {{
                        __resolve_result = JSON.stringify(r);
                        __resolve_done = true;
                    }}).catch(function(e) {{
                        __resolve_error = String(e);
                        __resolve_done = true;
                    }});
                }} else {{
                    // Synchronous return
                    __resolve_result = JSON.stringify(p);
                    __resolve_done = true;
                }}

                return __resolve_done ? (__resolve_error ? "ERROR:" + __resolve_error : __resolve_result) : "PENDING";
            }})()
            "#,
            url = url.replace('\\', "\\\\").replace('"', "\\\""),
        );

        self.context.with(|ctx| {
            let result: String = ctx.eval(script).map_err(|e| {
                crate::Error::Execution(format!("resolve() eval failed: {e}"))
            })?;

            // If still pending, drive the event loop
            if result == "PENDING" {
                let deadline = std::time::Instant::now() + timeout;

                // Drive the QuickJS job queue until resolved
                loop {
                    if std::time::Instant::now() > deadline {
                        return Err(crate::Error::Timeout(timeout.as_secs()));
                    }

                    let has_jobs = ctx.execute_pending_job();

                    // Check if done
                    let check: String = ctx
                        .eval(r#"__resolve_done ? (__resolve_error ? "ERROR:" + __resolve_error : __resolve_result) : "PENDING""#)
                        .map_err(|e| crate::Error::Execution(format!("Job check failed: {e}")))?;

                    if check != "PENDING" {
                        return parse_resolve_result(&check);
                    }

                    if !has_jobs {
                        // No more jobs but still pending — something went wrong
                        return Err(crate::Error::Execution(
                            "resolve() returned a Promise that never settled".into(),
                        ));
                    }
                }
            }

            parse_resolve_result(&result)
        })
    }

    /// Evaluate raw JavaScript and return the result as a string.
    pub fn eval_js(&self, code: &str) -> Result<String, crate::Error> {
        self.context.with(|ctx| {
            let result: String = ctx
                .eval(code.to_string())
                .map_err(|e| crate::Error::Execution(format!("JS eval error: {e}")))?;
            Ok(result)
        })
    }
}

fn parse_resolve_result(result: &str) -> Result<String, crate::Error> {
    if let Some(err) = result.strip_prefix("ERROR:") {
        Err(crate::Error::Execution(err.to_string()))
    } else {
        Ok(result.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_create_context() {
        let engine = PluginEngine::new(EngineConfig::default()).unwrap();
        let ctx = engine.create_context().unwrap();

        ctx.with(|ctx| {
            let result: i32 = ctx.eval("1 + 2").unwrap();
            assert_eq!(result, 3);
        });
    }

    #[test]
    fn test_eval_source() {
        let engine = PluginEngine::new(EngineConfig::default()).unwrap();
        let ctx = engine.create_context().unwrap();

        ctx.eval_source(
            r#"
            var __plugin_exports = {};
            __plugin_exports.pluginId = function() { return "test"; };
            "#,
            "test.js",
        )
        .unwrap();

        let id = ctx.call_export_string("pluginId").unwrap();
        assert_eq!(id, "test");
    }

    #[test]
    fn test_eval_js() {
        let engine = PluginEngine::new(EngineConfig::default()).unwrap();
        let ctx = engine.create_context().unwrap();

        let result = ctx.eval_js(r#""hello" + " " + "world""#).unwrap();
        assert_eq!(result, "hello world");
    }
}
