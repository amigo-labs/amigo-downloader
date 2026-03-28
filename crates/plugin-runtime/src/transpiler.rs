//! TypeScript → JavaScript transpilation via SWC.
//!
//! Strips type annotations, interfaces, enums, and other TS-only syntax
//! to produce clean ES2023 JavaScript that QuickJS-NG can execute.

use std::path::Path;

use swc_common::input::SourceFileInput;
use swc_common::sync::Lrc;
use swc_common::{FileName, GLOBALS, Globals, Mark, SourceMap};
use swc_ecma_ast::{EsVersion, Pass, Program};
use swc_ecma_codegen::Emitter;
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_parser::lexer::Lexer;
use swc_ecma_parser::{Parser, Syntax, TsSyntax};
use swc_ecma_transforms_typescript::typescript;

/// Transpile TypeScript source code to JavaScript (ES2023).
///
/// Only strips type annotations — no downlevel transformation.
/// The output is valid ES2023 that QuickJS-NG can execute directly.
pub fn transpile(source: &str, filename: &str) -> Result<String, crate::Error> {
    GLOBALS.set(&Globals::new(), || {
        let cm: Lrc<SourceMap> = Lrc::new(SourceMap::default());
        let fm = cm.new_source_file(
            Lrc::new(FileName::Custom(filename.to_string())),
            source.to_string(),
        );

        // Parse as TypeScript
        let lexer = Lexer::new(
            Syntax::Typescript(TsSyntax {
                tsx: false,
                decorators: true,
                ..Default::default()
            }),
            EsVersion::Es2022,
            SourceFileInput::from(&*fm),
            None,
        );

        let mut parser = Parser::new_from(lexer);
        let module = parser.parse_module().map_err(|e| {
            crate::Error::Execution(format!("TypeScript parse error in {filename}: {e:?}"))
        })?;

        // Strip TypeScript types → plain JavaScript
        let unresolved_mark = Mark::new();
        let top_level_mark = Mark::new();
        let mut pass = typescript::strip(unresolved_mark, top_level_mark);
        let mut program = Program::Module(module);
        pass.process(&mut program);
        let module = match program {
            Program::Module(m) => m,
            _ => unreachable!(),
        };

        // Generate JavaScript output
        let mut buf = Vec::new();
        {
            let writer = JsWriter::new(cm.clone(), "\n", &mut buf, None);
            let mut emitter = Emitter {
                cfg: swc_ecma_codegen::Config::default().with_target(EsVersion::Es2022),
                cm: cm.clone(),
                comments: None,
                wr: writer,
            };
            emitter
                .emit_module(&module)
                .map_err(|e| crate::Error::Execution(format!("Code generation error: {e}")))?;
        }

        String::from_utf8(buf)
            .map_err(|e| crate::Error::Execution(format!("UTF-8 error in output: {e}")))
    })
}

/// Check if a file path is TypeScript.
pub fn is_typescript(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts") | Some("mts")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_type_annotations() {
        let ts = r#"function hello(name: string): string { return name; }"#;
        let js = transpile(ts, "test.ts").unwrap();
        assert!(js.contains("function hello"));
        assert!(!js.contains(": string"));
    }

    #[test]
    fn test_strip_interface() {
        let ts = r#"
interface Foo {
    bar: string;
    baz: number;
}
function test(): void {}
"#;
        let js = transpile(ts, "test.ts").unwrap();
        assert!(!js.contains("interface"));
        assert!(!js.contains("bar: string"));
        assert!(js.contains("function test"));
    }

    #[test]
    fn test_strip_type_alias() {
        let ts = r#"
type MyType = string | number;
type Callback = (x: number) => void;
const x = 42;
"#;
        let js = transpile(ts, "test.ts").unwrap();
        assert!(!js.contains("type MyType"));
        assert!(!js.contains("type Callback"));
        assert!(js.contains("42"));
    }

    #[test]
    fn test_strip_declare() {
        let ts = r#"
declare const amigo: { httpGet(url: string): string; };
function resolve(url: string) { return { url: url }; }
"#;
        let js = transpile(ts, "test.ts").unwrap();
        assert!(!js.contains("declare"));
        assert!(js.contains("function resolve"));
    }

    #[test]
    fn test_strip_as_cast() {
        let ts = r#"const x = (someValue as string).length;"#;
        let js = transpile(ts, "test.ts").unwrap();
        assert!(!js.contains(" as string"));
        assert!(js.contains(".length"));
    }

    #[test]
    fn test_preserve_js_logic() {
        let ts = r#"
function add(a: number, b: number): number {
    return a + b;
}
const result: number = add(1, 2);
"#;
        let js = transpile(ts, "test.ts").unwrap();
        assert!(js.contains("function add"));
        assert!(js.contains("return a + b"));
        assert!(js.contains("add(1, 2)"));
        assert!(!js.contains(": number"));
    }

    #[test]
    fn test_is_typescript() {
        assert!(is_typescript(Path::new("plugin.ts")));
        assert!(is_typescript(Path::new("plugin.mts")));
        assert!(!is_typescript(Path::new("plugin.js")));
        assert!(!is_typescript(Path::new("plugin.mjs")));
    }
}
