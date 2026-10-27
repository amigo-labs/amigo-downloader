//! The host-function table: every function a plugin can call, independent of
//! the JS engine and of where the call is executed.
//!
//! Each entry is either
//! - [`Route::Local`]: a pure helper (regex, crypto, URL, HTML) with the
//!   signature `fn(&[Option<String>]) -> Result<Option<String>, String>`. It
//!   runs wherever the plugin's JS runs — inside the plugin host process.
//! - [`Route::Parent`]: a stateful function (HTTP, cookies, storage, logging,
//!   notifications, captcha). It is forwarded through a [`HostDispatch`] to
//!   [`HostApi::dispatch`](crate::host_api::HostApi::dispatch) in the daemon,
//!   which owns the network, the cookie jars, the storage and the callbacks.
//!
//! Arguments and results are strings only (see
//! `docs/specs/plugin-runtime-wasm-portability.md` §1): `bool`, numbers and
//! string arrays travel as JSON text and are converted at the engine adapter
//! according to [`Ret`]. No engine type appears in a host function's
//! signature, so the same table serves the QuickJS adapter below, the IPC
//! boundary to the daemon, and any future engine.

use std::sync::Arc;

use rquickjs::function::Rest;
use rquickjs::{Ctx, Function, IntoJs, Object, Value};

use crate::host_api::HostApi;

/// Arguments of one host call: `None` for an omitted / `null` optional.
pub type HostArgs = Vec<Option<String>>;

/// Result of one host call: `Ok(None)` is "no value" (`undefined` in JS),
/// `Err` is rethrown as a JS `Error`.
pub type HostResult = Result<Option<String>, String>;

/// Executes [`Route::Parent`] host functions for one plugin. The plugin's
/// identity is bound into the implementation, never passed from JS.
pub trait HostDispatch: Send + Sync {
    fn call(&self, name: &str, args: HostArgs) -> HostResult;
}

/// Global object a function is installed on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Obj {
    Amigo,
    Console,
}

/// JS-visible shape of a function's result.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ret {
    /// `undefined`.
    Unit,
    /// A string.
    Str,
    /// A string or `undefined`.
    OptStr,
    /// A boolean, encoded as `"true"` / `"false"`.
    Bool,
    /// A number or `undefined`, encoded as decimal text.
    OptNum,
    /// An array of strings, encoded as a JSON array.
    StrArray,
}

/// Where a function executes.
#[derive(Clone, Copy)]
pub enum Route {
    Local(fn(&[Option<String>]) -> HostResult),
    Parent,
}

/// One host function.
#[derive(Clone, Copy)]
pub struct HostFn {
    pub object: Obj,
    pub name: &'static str,
    /// Leading arguments that must be JS strings.
    pub required: usize,
    /// Trailing arguments that may be omitted, `undefined` or `null`.
    pub optional: usize,
    pub ret: Ret,
    pub route: Route,
}

impl HostFn {
    /// Name as seen by [`HostDispatch`] / IPC: `console.log` for console
    /// methods, the bare name for `amigo.*`.
    pub fn qualified_name(&self) -> String {
        match self.object {
            Obj::Amigo => self.name.to_string(),
            Obj::Console => format!("console.{}", self.name),
        }
    }
}

const fn local(
    name: &'static str,
    required: usize,
    ret: Ret,
    f: fn(&[Option<String>]) -> HostResult,
) -> HostFn {
    HostFn {
        object: Obj::Amigo,
        name,
        required,
        optional: 0,
        ret,
        route: Route::Local(f),
    }
}

const fn parent(name: &'static str, required: usize, optional: usize, ret: Ret) -> HostFn {
    HostFn {
        object: Obj::Amigo,
        name,
        required,
        optional,
        ret,
        route: Route::Parent,
    }
}

const fn console(name: &'static str) -> HostFn {
    HostFn {
        object: Obj::Console,
        name,
        required: 1,
        optional: 0,
        ret: Ret::Unit,
        route: Route::Parent,
    }
}

/// Required argument `i`. The adapter has already checked arity, so a missing
/// one is a table bug, reported as an error rather than a panic.
fn arg(args: &[Option<String>], i: usize) -> Result<&str, String> {
    args.get(i)
        .and_then(|a| a.as_deref())
        .ok_or_else(|| format!("missing argument {i}"))
}

fn json<T: serde::Serialize>(value: &T, empty: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| empty.into())
}

/// The complete host surface: 44 `amigo.*` functions and 3 `console.*`.
pub static HOST_FNS: &[HostFn] = &[
    // --- HTTP (parent: network, SSRF and domain checks, request budget) ---
    parent("__rawHttpGet", 1, 1, Ret::Str),
    parent("__rawHttpPost", 3, 1, Ret::Str),
    parent("__rawHttpHead", 1, 1, Ret::Str),
    parent("__rawHttpPostForm", 2, 1, Ret::Str),
    parent("__rawHttpGetBinary", 1, 1, Ret::Str),
    parent("__rawHttpFollowRedirects", 1, 1, Ret::Str),
    // --- Cookies (parent: per-plugin jars) ---
    parent("setCookie", 3, 0, Ret::Unit),
    parent("getCookie", 2, 0, Ret::OptStr),
    parent("clearCookies", 1, 0, Ret::Unit),
    // --- Regex ---
    local("regexMatch", 2, Ret::OptStr, |a| {
        Ok(HostApi::regex_match(arg(a, 0)?, arg(a, 1)?))
    }),
    local("regexMatchAll", 2, Ret::StrArray, |a| {
        Ok(Some(json(
            &HostApi::regex_match_all(arg(a, 0)?, arg(a, 1)?),
            "[]",
        )))
    }),
    local("regexReplace", 3, Ret::OptStr, |a| {
        Ok(HostApi::regex_replace(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?))
    }),
    local("regexTest", 2, Ret::Bool, |a| {
        Ok(Some(
            HostApi::regex_test(arg(a, 0)?, arg(a, 1)?).to_string(),
        ))
    }),
    local("__rawRegexSplit", 2, Ret::Str, |a| {
        Ok(Some(json(
            &HostApi::regex_split(arg(a, 0)?, arg(a, 1)?),
            "[]",
        )))
    }),
    // --- Encoding & crypto ---
    local("base64Decode", 1, Ret::Str, |a| {
        HostApi::base64_decode(arg(a, 0)?).map(Some)
    }),
    local("base64Encode", 1, Ret::Str, |a| {
        Ok(Some(HostApi::base64_encode(arg(a, 0)?)))
    }),
    local("md5", 1, Ret::Str, |a| Ok(Some(HostApi::md5(arg(a, 0)?)))),
    local("sha1", 1, Ret::Str, |a| Ok(Some(HostApi::sha1(arg(a, 0)?)))),
    local("sha256", 1, Ret::Str, |a| {
        Ok(Some(HostApi::sha256(arg(a, 0)?)))
    }),
    local("hmacSha256", 2, Ret::Str, |a| {
        HostApi::hmac_sha256(arg(a, 0)?, arg(a, 1)?).map(Some)
    }),
    local("aesDecryptCbc", 3, Ret::Str, |a| {
        HostApi::aes_decrypt_cbc(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?).map(Some)
    }),
    local("aesEncryptCbc", 3, Ret::Str, |a| {
        HostApi::aes_encrypt_cbc(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?).map(Some)
    }),
    // --- Logging (parent: the daemon's tracing subscriber) ---
    parent("logInfo", 1, 0, Ret::Unit),
    parent("logWarn", 1, 0, Ret::Unit),
    parent("logError", 1, 0, Ret::Unit),
    parent("logDebug", 1, 0, Ret::Unit),
    // --- Storage (parent: per-plugin namespace and quota) ---
    parent("storageGet", 1, 0, Ret::OptStr),
    parent("storageSet", 2, 0, Ret::Unit),
    parent("storageDelete", 1, 0, Ret::Unit),
    // --- URL ---
    local("urlResolve", 2, Ret::Str, |a| {
        HostApi::url_resolve(arg(a, 0)?, arg(a, 1)?).map(Some)
    }),
    local("__rawUrlParse", 1, Ret::Str, |a| {
        HostApi::url_parse(arg(a, 0)?).map(|v| Some(v.to_string()))
    }),
    local("urlFilename", 1, Ret::OptStr, |a| {
        Ok(HostApi::url_filename(arg(a, 0)?))
    }),
    // --- HTML ---
    local("__rawHtmlQueryAll", 2, Ret::Str, |a| {
        HostApi::html_query_all(arg(a, 0)?, arg(a, 1)?).map(|v| Some(json(&v, "[]")))
    }),
    local("htmlQueryText", 2, Ret::OptStr, |a| {
        HostApi::html_query_text(arg(a, 0)?, arg(a, 1)?)
    }),
    local("htmlQueryAttr", 3, Ret::OptStr, |a| {
        HostApi::html_query_attr(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?)
    }),
    local("__rawHtmlSearchMeta", 2, Ret::OptStr, |a| {
        let Ok(names) = serde_json::from_str::<Vec<String>>(arg(a, 1)?) else {
            return Ok(None);
        };
        Ok(HostApi::html_search_meta(arg(a, 0)?, &names))
    }),
    local("htmlExtractTitle", 1, Ret::OptStr, |a| {
        Ok(HostApi::html_extract_title(arg(a, 0)?))
    }),
    local("__rawHtmlHiddenInputs", 1, Ret::Str, |a| {
        Ok(Some(json(&HostApi::html_hidden_inputs(arg(a, 0)?), "{}")))
    }),
    local("__rawSearchJson", 2, Ret::OptStr, |a| {
        Ok(HostApi::search_json(arg(a, 0)?, arg(a, 1)?))
    }),
    local("__rawHtmlQueryAllAttrs", 3, Ret::Str, |a| {
        HostApi::html_query_all_attrs(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?)
            .map(|v| Some(json(&v, "[]")))
    }),
    // --- Utility ---
    local("parseDuration", 1, Ret::OptNum, |a| {
        Ok(HostApi::parse_duration(arg(a, 0)?).map(|n| n.to_string()))
    }),
    local("sanitizeFilename", 1, Ret::Str, |a| {
        Ok(Some(HostApi::sanitize_filename(arg(a, 0)?)))
    }),
    // --- Notifications & captcha (parent: UI callbacks) ---
    parent("notify", 2, 0, Ret::Unit),
    parent("__rawSolveCaptcha", 1, 1, Ret::Str),
    // --- Console bridge (parent: the daemon's tracing subscriber) ---
    console("log"),
    console("warn"),
    console("error"),
];

/// Look up a [`Route::Parent`] function by its qualified name. The daemon
/// uses this to refuse calls the table does not route to it.
pub fn parent_fn(qualified: &str) -> Option<&'static HostFn> {
    HOST_FNS
        .iter()
        .find(|f| matches!(f.route, Route::Parent) && f.qualified_name() == qualified)
}

/// JavaScript shim that wraps raw JSON-returning functions into
/// object-returning ones. Injected after every table function is registered.
const JS_SHIM: &str = r#"
(function() {
    var a = amigo;

    // HTTP: wrap raw JSON-string returns into parsed objects
    a.httpGet = function(url, opts) {
        var hdr = (opts && opts.headers) ? JSON.stringify(opts.headers) : null;
        return JSON.parse(a.__rawHttpGet(url, hdr));
    };
    a.httpPost = function(url, body, contentType, opts) {
        var hdr = (opts && opts.headers) ? JSON.stringify(opts.headers) : null;
        return JSON.parse(a.__rawHttpPost(url, body, contentType, hdr));
    };
    a.httpHead = function(url, opts) {
        var hdr = (opts && opts.headers) ? JSON.stringify(opts.headers) : null;
        return JSON.parse(a.__rawHttpHead(url, hdr));
    };

    // Convenience: GET and parse body as JSON
    a.httpGetJson = function(url, opts) {
        var resp = a.httpGet(url, opts);
        resp.data = JSON.parse(resp.body);
        return resp;
    };

    // HTTP extensions
    a.httpPostForm = function(url, fields, opts) {
        var hdr = (opts && opts.headers) ? JSON.stringify(opts.headers) : null;
        return JSON.parse(a.__rawHttpPostForm(url, JSON.stringify(fields), hdr));
    };
    a.httpGetBinary = function(url, opts) {
        var hdr = (opts && opts.headers) ? JSON.stringify(opts.headers) : null;
        return a.__rawHttpGetBinary(url, hdr);
    };
    a.httpFollowRedirects = function(url, opts) {
        var hdr = (opts && opts.headers) ? JSON.stringify(opts.headers) : null;
        return a.__rawHttpFollowRedirects(url, hdr);
    };

    // HTML: batch attribute extraction
    a.htmlQueryAllAttrs = function(html, selector, attr) {
        return JSON.parse(a.__rawHtmlQueryAllAttrs(html, selector, attr));
    };

    // URL: parse returns object
    a.urlParse = function(url) {
        return JSON.parse(a.__rawUrlParse(url));
    };

    // HTML: arrays and objects
    a.htmlQueryAll = function(html, selector) {
        return JSON.parse(a.__rawHtmlQueryAll(html, selector));
    };
    a.htmlSearchMeta = function(html, names) {
        var arr = Array.isArray(names) ? names : [names];
        return a.__rawHtmlSearchMeta(html, JSON.stringify(arr));
    };
    a.htmlHiddenInputs = function(html) {
        return JSON.parse(a.__rawHtmlHiddenInputs(html));
    };
    a.searchJson = function(startPattern, html) {
        var s = a.__rawSearchJson(startPattern, html);
        return s ? JSON.parse(s) : null;
    };

    // Regex: split returns array
    a.regexSplit = function(pattern, text) {
        return JSON.parse(a.__rawRegexSplit(pattern, text));
    };

    // Captcha: wraps raw function
    a.solveCaptcha = function(imageUrl, captchaType) {
        return a.__rawSolveCaptcha(imageUrl, captchaType || "image");
    };

    // Safe deep property access: amigo.traverse(obj, ["a", "b", "c"]) or amigo.traverse(obj, "a.b.c")
    a.traverse = function(obj, path) {
        if (typeof path === "string") path = path.split(".");
        var cur = obj;
        for (var i = 0; i < path.length; i++) {
            if (cur == null) return null;
            cur = cur[path[i]];
        }
        return cur == null ? null : cur;
    };
})();
"#;

fn js_err(msg: &str) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("string", "Error", msg)
}

/// Convert JS arguments to [`HostArgs`] with the same rules rquickjs applies
/// to `String` / `Option<String>` parameters: required arguments must be
/// strings, optional ones may also be `undefined` or `null`. Extra arguments
/// are ignored.
fn convert_args(f: &HostFn, args: &[Value<'_>]) -> rquickjs::Result<HostArgs> {
    let mut out = Vec::with_capacity(f.required + f.optional);
    for i in 0..f.required + f.optional {
        let value = args.get(i);
        let optional = i >= f.required;
        match value {
            Some(v) if v.is_string() => {
                let s = v.as_string().map(|s| s.to_string()).transpose()?;
                out.push(s);
            }
            None => {
                if optional {
                    out.push(None);
                } else {
                    return Err(rquickjs::Error::new_from_js("undefined", "String"));
                }
            }
            Some(v) if optional && (v.is_undefined() || v.is_null()) => out.push(None),
            Some(v) => return Err(rquickjs::Error::new_from_js(v.type_name(), "String")),
        }
    }
    Ok(out)
}

/// Convert a host result to the JS shape declared by `ret`.
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
            let items: Vec<String> = serde_json::from_str(value.as_deref().unwrap_or("[]"))
                .map_err(|_| js_err("host function returned a malformed array"))?;
            items.into_js(ctx)
        }
    }
}

/// Install every [`HOST_FNS`] entry under `globalThis.amigo` /
/// `globalThis.console`, then the [`JS_SHIM`] wrappers.
///
/// `dispatch` executes the [`Route::Parent`] functions; it carries the
/// plugin's identity (its scoped cookie jar, storage namespace, request
/// budget and domain allowlist), which JS can neither read nor change.
pub fn register_host_api<'js>(
    ctx: &Ctx<'js>,
    dispatch: Arc<dyn HostDispatch>,
) -> Result<(), crate::Error> {
    let exec = |e: rquickjs::Error| crate::Error::Execution(e.to_string());
    let amigo = Object::new(ctx.clone())
        .map_err(|e| crate::Error::Execution(format!("Failed to create amigo object: {e}")))?;
    let console = Object::new(ctx.clone())
        .map_err(|e| crate::Error::Execution(format!("Failed to create console: {e}")))?;

    for f in HOST_FNS {
        let qualified = f.qualified_name();
        let dispatch = Arc::clone(&dispatch);
        let func = Function::new(
            ctx.clone(),
            move |ctx: Ctx<'js>, args: Rest<Value<'js>>| -> rquickjs::Result<Value<'js>> {
                let args = convert_args(f, &args.0)?;
                let result = match f.route {
                    Route::Local(call) => call(&args),
                    Route::Parent => dispatch.call(&qualified, args),
                };
                let value = result.map_err(|e| js_err(&e))?;
                convert_ret(&ctx, f.ret, value)
            },
        )
        .map_err(exec)?;
        let target = match f.object {
            Obj::Amigo => &amigo,
            Obj::Console => &console,
        };
        target.set(f.name, func).map_err(exec)?;
    }

    let global = ctx.globals();
    global.set("amigo", amigo).map_err(exec)?;
    global.set("console", console).map_err(exec)?;

    // The shim keeps the Rust→JS boundary at plain strings while giving
    // plugins object-returning functions.
    ctx.eval::<Value<'_>, _>(JS_SHIM)
        .map_err(|e| crate::Error::Execution(format!("Failed to inject JS shim: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn table_has_the_documented_surface() {
        let amigo = HOST_FNS.iter().filter(|f| f.object == Obj::Amigo).count();
        let console = HOST_FNS.iter().filter(|f| f.object == Obj::Console).count();
        assert_eq!((amigo, console), (44, 3));

        let names: HashSet<String> = HOST_FNS.iter().map(HostFn::qualified_name).collect();
        assert_eq!(names.len(), HOST_FNS.len(), "duplicate host function name");
    }

    #[test]
    fn table_covers_every_function_the_shim_uses() {
        let names: HashSet<&str> = HOST_FNS.iter().map(|f| f.name).collect();
        let re = regex::Regex::new(r"a\.(__raw\w+)\(").unwrap();
        let used: Vec<&str> = re
            .captures_iter(JS_SHIM)
            .map(|c| c.get(1).unwrap().as_str())
            .collect();
        assert!(!used.is_empty());
        for name in used {
            assert!(
                names.contains(name),
                "shim calls {name}, which is not in HOST_FNS"
            );
        }
    }

    #[test]
    fn stateful_functions_are_routed_to_the_parent() {
        for name in [
            "__rawHttpGet",
            "__rawHttpPost",
            "__rawHttpHead",
            "__rawHttpPostForm",
            "__rawHttpGetBinary",
            "__rawHttpFollowRedirects",
            "setCookie",
            "getCookie",
            "clearCookies",
            "storageGet",
            "storageSet",
            "storageDelete",
            "notify",
            "__rawSolveCaptcha",
            "logInfo",
            "console.log",
        ] {
            assert!(parent_fn(name).is_some(), "{name} must run in the daemon");
        }
        assert!(parent_fn("md5").is_none(), "pure helpers stay local");
    }

    #[test]
    fn local_functions_encode_results_as_text() {
        let call = |name: &str, args: &[&str]| {
            let f = HOST_FNS.iter().find(|f| f.name == name).unwrap();
            let Route::Local(call) = f.route else {
                panic!("{name} is not local")
            };
            let args: HostArgs = args.iter().map(|a| Some((*a).to_string())).collect();
            call(&args)
        };
        assert_eq!(call("regexTest", &["a+", "caab"]), Ok(Some("true".into())));
        assert_eq!(
            call("regexMatchAll", &["\\d", "a1b2"]),
            Ok(Some(r#"["1","2"]"#.into()))
        );
        assert_eq!(call("parseDuration", &["1:30"]), Ok(Some("90".into())));
        assert_eq!(call("regexMatch", &["z", "abc"]), Ok(None));
        assert!(call("base64Decode", &["%%%"]).is_err());
    }
}
