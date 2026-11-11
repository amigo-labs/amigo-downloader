//! QuickJS as a WebAssembly module: the only place plugin JavaScript runs.
//!
//! The module has no system interface. It imports one function to call a
//! host function and (through `rquickjs-sys`) a clock; everything else is
//! computation inside its own linear memory. The executor in
//! `krake-plugin-runtime` (`wasm.rs`) drives it through [`call`].
//!
//! Wire format (the executor, `krake-plugin-runtime`'s `wasm.rs`, mirrors it):
//!
//! - request: `[op u8][nargs u8]` then per argument `[len u32 LE][bytes]`
//! - response: `[tag u8][bytes]`, tag 0 = no value, 1 = value, 2 = error
//! - host call arguments: `[nargs u8]` then per argument
//!   `[present u8][len u32 LE][bytes]`; the reply is a response frame
//!
//! Buffers cross the boundary as (pointer, length); a `u64` return packs
//! them as `pointer << 32 | length`. The receiver frees with [`dealloc`].
//!
//! The guest sets no deadline, memory or stack limit of its own: the
//! executor interrupts it, bounds its linear memory and its stack.

use std::cell::RefCell;
use std::ffi::CString;

use rquickjs::function::Rest;
use rquickjs::{Context, Ctx, FromJs, Function, IntoJs, Object, Runtime, Value, qjs};

#[link(wasm_import_module = "krake")]
unsafe extern "C" {
    /// Call host function `index` of the table passed at init. Returns a
    /// response frame allocated with [`alloc`], packed as `ptr << 32 | len`.
    fn host_call(index: u32, args: *const u8, len: u32) -> u64;
}

const TAG_NONE: u8 = 0;
const TAG_SOME: u8 = 1;
const TAG_ERR: u8 = 2;

const OP_INIT: u8 = 0;
const OP_EVAL: u8 = 1;
const OP_EVAL_STRING: u8 = 2;
const OP_COMPILE: u8 = 3;
const OP_EVAL_BYTECODE: u8 = 4;
const OP_EXPORT_STRING: u8 = 5;
const OP_HAS_EXPORT: u8 = 6;
const OP_EXPORT_JSON: u8 = 7;
const OP_REQUIRE_FN: u8 = 8;
const OP_PENDING_JOB: u8 = 9;
const OP_BIND: u8 = 10;
/// Misbehave on purpose, so the executor's tests can prove that a trapping
/// or hostile guest is contained. Only the executor can send an op; plugin
/// JavaScript cannot reach this.
const OP_SELF_TEST: u8 = 255;

/// JS-visible shape of a host function's result (`host_fns::Ret`).
#[derive(Clone, Copy)]
enum Ret {
    Unit,
    Str,
    OptStr,
    Bool,
    OptNum,
    StrArray,
}

/// One row of the host-function table.
struct HostFn {
    console: bool,
    name: String,
    required: usize,
    optional: usize,
    ret: Ret,
}

struct State {
    context: Context,
    _runtime: Runtime,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

type OpResult = Result<Option<Vec<u8>>, String>;

/// Allocate `len` bytes in guest memory for the host to fill.
#[unsafe(no_mangle)]
pub extern "C" fn alloc(len: u32) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Free a buffer obtained from [`alloc`] or returned by [`call`].
///
/// # Safety
/// `ptr` and `len` must describe exactly one such buffer, not freed before.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: u32) {
    // SAFETY: per the contract the buffer came from a Vec<u8> with this
    // capacity; length 0 drops no elements.
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len as usize) });
}

/// Run one request frame and return the response frame.
///
/// # Safety
/// `ptr` and `len` must describe a readable request buffer in guest memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn call(ptr: *const u8, len: u32) -> u64 {
    // SAFETY: per the contract the executor wrote `len` bytes at `ptr`.
    let request = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let result = match parse_request(request) {
        Some((op, args)) => run(op, &args),
        None => Err("malformed request frame".into()),
    };
    leak(response(result))
}

fn response(result: OpResult) -> Vec<u8> {
    match result {
        Ok(None) => vec![TAG_NONE],
        Ok(Some(value)) => frame(TAG_SOME, &value),
        Err(message) => frame(TAG_ERR, message.as_bytes()),
    }
}

fn frame(tag: u8, bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 1);
    out.push(tag);
    out.extend_from_slice(bytes);
    out
}

/// Hand a buffer to the host: capacity equals length, so `dealloc(ptr, len)`
/// frees it.
fn leak(bytes: Vec<u8>) -> u64 {
    let boxed = bytes.into_boxed_slice();
    let len = boxed.len() as u64;
    let ptr = Box::into_raw(boxed) as *mut u8 as u64;
    (ptr << 32) | len
}

fn parse_request(data: &[u8]) -> Option<(u8, Vec<&[u8]>)> {
    let (&op, rest) = data.split_first()?;
    let (&nargs, mut rest) = rest.split_first()?;
    let mut args = Vec::with_capacity(nargs as usize);
    for _ in 0..nargs {
        let (len, tail) = rest.split_first_chunk::<4>()?;
        let len = u32::from_le_bytes(*len) as usize;
        if tail.len() < len {
            return None;
        }
        let (arg, tail) = tail.split_at(len);
        args.push(arg);
        rest = tail;
    }
    Some((op, args))
}

fn text<'a>(args: &[&'a [u8]], i: usize) -> Result<&'a str, String> {
    let bytes = args.get(i).ok_or_else(|| format!("missing argument {i}"))?;
    std::str::from_utf8(bytes).map_err(|_| format!("argument {i} is not UTF-8"))
}

fn run(op: u8, args: &[&[u8]]) -> OpResult {
    if op == OP_INIT {
        return init();
    }
    if op == OP_SELF_TEST {
        return self_test(text(args, 0)?);
    }
    STATE.with(|state| {
        let state = state.borrow();
        let state = state.as_ref().ok_or("guest is not initialized")?;
        state.context.with(|ctx| match op {
            OP_EVAL => eval(&ctx, text(args, 0)?, text(args, 1)?),
            OP_EVAL_STRING => eval_string(&ctx, text(args, 0)?, text(args, 1)?),
            OP_COMPILE => compile(&ctx, text(args, 0)?, text(args, 1)?),
            OP_EVAL_BYTECODE => {
                let bytecode = args.first().ok_or("missing argument 0")?;
                eval_bytecode(&ctx, bytecode, text(args, 1)?)
            }
            OP_EXPORT_STRING => export_string(&ctx, text(args, 0)?),
            OP_HAS_EXPORT => has_export(&ctx, text(args, 0)?),
            OP_EXPORT_JSON => export_json(&ctx, text(args, 0)?),
            OP_REQUIRE_FN => require_fn(&ctx, text(args, 0)?),
            OP_PENDING_JOB => Ok(Some(flag(ctx.execute_pending_job()))),
            OP_BIND => {
                let table = parse_table(text(args, 0)?)?;
                register(&ctx, table, text(args, 1)?)?;
                Ok(None)
            }
            _ => Err(format!("unknown op {op}")),
        })
    })
}

fn self_test(kind: &str) -> OpResult {
    match kind {
        // What a memory-safety bug typically ends in: `unreachable`.
        "trap" => std::process::abort(),
        // SAFETY (all three): deliberately invalid arguments; the executor
        // must reject them without reading outside this module's memory.
        "bad-index" => unsafe {
            host_call(u32::MAX, std::ptr::null(), 0);
        },
        "bad-pointer" => unsafe {
            host_call(0, 0xffff_fff0 as *const u8, 64);
        },
        "bad-frame" => unsafe {
            let frame = [9u8];
            host_call(0, frame.as_ptr(), 1);
        },
        _ => return Err(format!("unknown self-test {kind}")),
    }
    Err("the executor accepted a hostile host call".into())
}

fn flag(value: bool) -> Vec<u8> {
    if value {
        b"true".to_vec()
    } else {
        b"false".to_vec()
    }
}

// ---------------------------------------------------------------------------
// Engine operations. Error texts match `krake-plugin-runtime`'s engine so a
// plugin author sees the same message under either engine.
// ---------------------------------------------------------------------------

/// Render an rquickjs error; a thrown JS value is replaced by its message.
fn describe(ctx: &Ctx<'_>, e: rquickjs::Error) -> String {
    if !e.is_exception() {
        return e.to_string();
    }
    let thrown = ctx.catch();
    if let Some(ex) = thrown.as_exception() {
        match ex.message() {
            Some(msg) => msg,
            None => "unknown JS exception".to_string(),
        }
    } else if let Some(s) = thrown.as_string() {
        s.to_string()
            .unwrap_or_else(|_| "unprintable JS exception".into())
    } else {
        // Any other thrown value (object, number, null): its JS string form,
        // as `String(value)` gives it.
        thrown
            .get::<rquickjs::convert::Coerced<String>>()
            .map(|s| s.0)
            .unwrap_or_else(|_| "unprintable JS exception".into())
    }
}

fn eval(ctx: &Ctx<'_>, source: &str, what: &str) -> OpResult {
    ctx.eval::<Value<'_>, _>(source)
        .map_err(|e| format!("{what}: {}", describe(ctx, e)))?;
    Ok(None)
}

fn eval_string(ctx: &Ctx<'_>, code: &str, what: &str) -> OpResult {
    // `String`, not `Option<String>`: a value-less result (a forgotten
    // `return`) is the conversion error the native engine reports.
    let value: String = ctx
        .eval(code)
        .map_err(|e| format!("{what}: {}", describe(ctx, e)))?;
    Ok(Some(value.into_bytes()))
}

fn exports<'js>(ctx: &Ctx<'js>) -> Result<Object<'js>, String> {
    ctx.globals()
        .get("__plugin_exports")
        .map_err(|e| format!("__plugin_exports not found: {e}"))
}

fn export_value<'js>(ctx: &Ctx<'js>, name: &str) -> Result<Value<'js>, String> {
    exports(ctx)?
        .get(name)
        .map_err(|e| format!("reading export '{name}': {}", describe(ctx, e)))
}

fn export_string(ctx: &Ctx<'_>, name: &str) -> OpResult {
    let value = export_value(ctx, name)?;
    let result = if value.is_function() {
        let func =
            Function::from_js(ctx, value).map_err(|e| format!("'{name}' is not callable: {e}"))?;
        func.call::<_, String>(())
            .map_err(|e| format!("{name}() failed: {}", describe(ctx, e)))?
    } else if value.is_string() {
        String::from_js(ctx, value).map_err(|e| format!("'{name}' is not a string: {e}"))?
    } else if value.is_undefined() || value.is_null() {
        return Err(format!("Export '{name}' not found"));
    } else {
        ctx.eval::<String, _>(format!("String(__plugin_exports.{name})"))
            .map_err(|e| format!("Could not coerce '{name}' to string: {}", describe(ctx, e)))?
    };
    Ok(Some(result.into_bytes()))
}

fn has_export(ctx: &Ctx<'_>, name: &str) -> OpResult {
    let value = export_value(ctx, name)?;
    Ok(Some(flag(!(value.is_undefined() || value.is_null()))))
}

/// `name_lit` is the export name as a JS string literal.
fn export_json(ctx: &Ctx<'_>, name_lit: &str) -> OpResult {
    let script = format!(
        "(function() {{ var v = __plugin_exports[{name_lit}]; \
         return v === undefined || v === null ? null : JSON.stringify(v); }})()"
    );
    let value: Option<String> = ctx
        .eval(script)
        .map_err(|e| format!("reading export {name_lit}: {}", describe(ctx, e)))?;
    Ok(value.map(String::into_bytes))
}

fn require_fn(ctx: &Ctx<'_>, name: &str) -> OpResult {
    let value = export_value(ctx, name)?;
    if value.is_function() {
        Ok(None)
    } else if value.is_undefined() || value.is_null() {
        Err(format!("missing required function '{name}'"))
    } else {
        Err(format!(
            "'{name}' must be a function, got {}",
            value.type_name()
        ))
    }
}

fn take_exception(ctx: &Ctx<'_>) -> String {
    describe(ctx, rquickjs::Error::Exception)
}

/// Compile `source` as a global script to QuickJS bytecode, without running
/// it.
fn compile(ctx: &Ctx<'_>, source: &str, filename: &str) -> OpResult {
    let what = format!("JS compile error in {filename}");
    let src = CString::new(source).map_err(|_| format!("{what}: source contains a NUL byte"))?;
    let name =
        CString::new(filename).map_err(|_| format!("{what}: filename contains a NUL byte"))?;
    let raw = ctx.as_raw().as_ptr();
    // SAFETY: `raw` is the live context we are inside of; `src` and `name`
    // are NUL-terminated and outlive the call. Every JSValue is freed exactly
    // once, and the JS_WriteObject buffer is released with js_free on the
    // same context.
    unsafe {
        let func = qjs::JS_Eval(
            raw,
            src.as_ptr(),
            source.len() as _,
            name.as_ptr(),
            (qjs::JS_EVAL_TYPE_GLOBAL | qjs::JS_EVAL_FLAG_COMPILE_ONLY) as i32,
        );
        if qjs::JS_IsException(func) {
            return Err(format!("{what}: {}", take_exception(ctx)));
        }
        let mut len = 0usize;
        let buf = qjs::JS_WriteObject(
            raw,
            (&raw mut len).cast(),
            func,
            qjs::JS_WRITE_OBJ_BYTECODE as i32,
        );
        qjs::JS_FreeValue(raw, func);
        if buf.is_null() {
            return Err(format!("{what}: {}", take_exception(ctx)));
        }
        let bytes = std::slice::from_raw_parts(buf, len).to_vec();
        qjs::js_free(raw, buf as *mut _);
        Ok(Some(bytes))
    }
}

/// Run bytecode produced by [`compile`]. QuickJS does not validate bytecode;
/// whatever a bad buffer does, it does inside this module's linear memory.
fn eval_bytecode(ctx: &Ctx<'_>, bytecode: &[u8], filename: &str) -> OpResult {
    let what = format!("JS eval error in {filename}");
    let raw = ctx.as_raw().as_ptr();
    // SAFETY: `raw` is the live context; the buffer outlives the call; each
    // value is freed exactly once (JS_EvalFunction consumes `func`).
    unsafe {
        let func = qjs::JS_ReadObject(
            raw,
            bytecode.as_ptr(),
            bytecode.len() as _,
            qjs::JS_READ_OBJ_BYTECODE as i32,
        );
        if qjs::JS_IsException(func) {
            return Err(format!("{what}: {}", take_exception(ctx)));
        }
        let result = qjs::JS_EvalFunction(raw, func);
        if qjs::JS_IsException(result) {
            return Err(format!("{what}: {}", take_exception(ctx)));
        }
        qjs::JS_FreeValue(raw, result);
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// Host functions
// ---------------------------------------------------------------------------

/// Parse the table the executor sends: one function per line,
/// `object\tname\trequired\toptional\tret`.
fn parse_table(table: &str) -> Result<Vec<HostFn>, String> {
    table
        .lines()
        .map(|line| {
            let bad = || format!("malformed host-function row: {line}");
            let mut cols = line.split('\t');
            let mut next = || cols.next().ok_or_else(bad);
            let console = match next()? {
                "krake" => false,
                "console" => true,
                _ => return Err(bad()),
            };
            let name = next()?.to_string();
            let required = next()?.parse().map_err(|_| bad())?;
            let optional = next()?.parse().map_err(|_| bad())?;
            let ret = match next()? {
                "unit" => Ret::Unit,
                "str" => Ret::Str,
                "optstr" => Ret::OptStr,
                "bool" => Ret::Bool,
                "optnum" => Ret::OptNum,
                "strarray" => Ret::StrArray,
                _ => return Err(bad()),
            };
            Ok(HostFn {
                console,
                name,
                required,
                optional,
                ret,
            })
        })
        .collect()
}

/// Create the QuickJS runtime. It has no host API until `OP_BIND`: a
/// plugin's module body runs before, and only defines exports.
fn init() -> OpResult {
    let runtime = Runtime::new().map_err(|e| format!("Failed to create QuickJS runtime: {e}"))?;
    // QuickJS measures its stack against the native stack pointer, which
    // means nothing in Wasm; the executor's Wasm stack limit is the guard.
    runtime.set_max_stack_size(0);
    let context =
        Context::full(&runtime).map_err(|e| format!("Failed to create QuickJS context: {e}"))?;
    STATE.with(|state| {
        *state.borrow_mut() = Some(State {
            context,
            _runtime: runtime,
        });
    });
    Ok(None)
}

fn js_err(msg: &str) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("string", "Error", msg)
}

fn register<'js>(ctx: &Ctx<'js>, table: Vec<HostFn>, shim: &str) -> Result<(), String> {
    let exec = |e: rquickjs::Error| e.to_string();
    let krake =
        Object::new(ctx.clone()).map_err(|e| format!("Failed to create krake object: {e}"))?;
    let console = Object::new(ctx.clone()).map_err(|e| format!("Failed to create console: {e}"))?;

    for (index, f) in table.into_iter().enumerate() {
        let target = if f.console { &console } else { &krake };
        let name = f.name.clone();
        let func = Function::new(
            ctx.clone(),
            move |ctx: Ctx<'js>, args: Rest<Value<'js>>| -> rquickjs::Result<Value<'js>> {
                let args = encode_args(&f, &args.0)?;
                let value = call_host(index as u32, &args).map_err(|e| js_err(&e))?;
                convert_ret(&ctx, f.ret, value)
            },
        )
        .map_err(exec)?;
        target.set(name, func).map_err(exec)?;
    }

    let global = ctx.globals();
    global.set("amigo", krake.clone()).map_err(exec)?;
    global.set("krake", krake).map_err(exec)?;
    global.set("console", console).map_err(exec)?;
    ctx.eval::<Value<'_>, _>(shim)
        .map_err(|e| format!("Failed to inject JS shim: {e}"))?;
    Ok(())
}

/// Encode JS arguments as a host-call frame. Required arguments must be
/// strings; optional ones may also be `undefined` or `null`; extra arguments
/// are ignored.
fn encode_args(f: &HostFn, args: &[Value<'_>]) -> rquickjs::Result<Vec<u8>> {
    let count = f.required + f.optional;
    let mut out = vec![count as u8];
    for i in 0..count {
        let optional = i >= f.required;
        match args.get(i) {
            Some(v) if v.is_string() => {
                let s = v
                    .as_string()
                    .map(|s| s.to_string())
                    .transpose()?
                    .unwrap_or_default();
                out.push(1);
                out.extend_from_slice(&(s.len() as u32).to_le_bytes());
                out.extend_from_slice(s.as_bytes());
            }
            None if optional => out.push(0),
            None => return Err(rquickjs::Error::new_from_js("undefined", "String")),
            Some(v) if optional && (v.is_undefined() || v.is_null()) => out.push(0),
            Some(v) => return Err(rquickjs::Error::new_from_js(v.type_name(), "String")),
        }
    }
    Ok(out)
}

fn call_host(index: u32, args: &[u8]) -> Result<Option<String>, String> {
    // SAFETY: `args` is a live buffer of this length; the executor reads it
    // during the call only.
    let packed = unsafe { host_call(index, args.as_ptr(), args.len() as u32) };
    let (ptr, len) = ((packed >> 32) as usize, (packed & 0xffff_ffff) as usize);
    if ptr == 0 || len == 0 {
        return Err("host call returned no frame".into());
    }
    // SAFETY: the executor allocated this frame with `alloc(len)` and filled
    // all `len` bytes; ownership passes to us.
    let reply = unsafe { Vec::from_raw_parts(ptr as *mut u8, len, len) };
    let body = || String::from_utf8_lossy(&reply[1..]).into_owned();
    match reply[0] {
        TAG_NONE => Ok(None),
        TAG_SOME => Ok(Some(body())),
        _ => Err(body()),
    }
}

/// Convert a host result to the JS shape its table row declares.
fn convert_ret<'js>(
    ctx: &Ctx<'js>,
    ret: Ret,
    value: Option<String>,
) -> rquickjs::Result<Value<'js>> {
    let missing = || js_err("host function returned no value");
    match ret {
        Ret::Unit => Ok(Value::new_undefined(ctx.clone())),
        Ret::Str => value.ok_or_else(missing)?.into_js(ctx),
        Ret::OptStr => value.into_js(ctx),
        Ret::Bool => (value.as_deref() == Some("true")).into_js(ctx),
        Ret::OptNum => value
            .map(|v| {
                v.parse::<f64>()
                    .map_err(|_| js_err("host function returned a non-number"))
            })
            .transpose()?
            .into_js(ctx),
        Ret::StrArray => {
            // Parsed by the engine's own JSON parser, so the guest needs no
            // JSON library; the round trip through Vec<String> rejects
            // anything that is not an array of strings.
            let malformed = |_| js_err("host function returned a malformed array");
            let parsed = ctx
                .json_parse(value.unwrap_or_else(|| "[]".into()))
                .map_err(malformed)?;
            let items = Vec::<String>::from_js(ctx, parsed).map_err(malformed)?;
            items.into_js(ctx)
        }
    }
}
