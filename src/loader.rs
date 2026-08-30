//! File loading, import resolution and top-level compilation entry points.
//!
//! This mirrors `main.rs`'s own `load_program`/`resolve_import`/`compile_program`
//! (kept as a separate, deliberately unchanged copy there for the CLI binary) so the
//! library surface doesn't depend on the binary crate. `compile_snippet` is the one
//! addition beyond what the CLI needs: it lets a caller compile a small piece of STAS
//! source (typically `import "std/..."` plus a single stdlib function call) as if it
//! were a real file, without having to write one to disk first.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::ast::{self, Program, Stmt};
use crate::interp;
use crate::lexer;
use crate::parser;

pub fn compile_file(path: &Path) -> Result<(Vec<String>, Vec<String>), String> {
    let program = load_program(path)?;
    compile_program(&program)
}

/// Compiles `source` (e.g. `"import \"std/all\"\ntas_sprint(90, 5)\n"`) as though it
/// were a file living in `virtual_dir` — real imports it references (like `std/all`)
/// are resolved and loaded from disk exactly as they would be for a real file, but
/// `source` itself never needs to exist on disk. Always compiles starting at frame 0;
/// callers that want the result at a different point in their own timeline should
/// re-base the returned lines' frames themselves (parse them, add an offset).
pub fn compile_snippet(source: &str, virtual_dir: &Path) -> Result<(Vec<String>, Vec<String>), String> {
    let tokens = lexer::lex(source)?;
    let parsed = parser::Parser::new(tokens).parse_program()?;
    let virtual_file = virtual_dir.join("snippet.stas");

    let mut program = Program {
        funcs: Vec::new(),
        main: Vec::new(),
    };
    let mut stack = Vec::new();
    let mut loaded = HashSet::new();
    for stmt in parsed.main {
        let Stmt::Import { path: import, line } = stmt else {
            program.main.push(stmt);
            continue;
        };
        let import_path = resolve_import(&virtual_file, &import).map_err(|e| format!("{line}: {e}"))?;
        let imported = load_program_inner(&import_path, &mut stack, &mut loaded)
            .map_err(|e| format!("{line}: while importing '{import}': {e}"))?;
        program.funcs.extend(imported.funcs);
        program.main.extend(imported.main);
    }
    program.funcs.extend(parsed.funcs);

    compile_program(&program)
}

fn compile_program(program: &Program) -> Result<(Vec<String>, Vec<String>), String> {
    let compiler = interp::Compiler::new(program)?;
    compiler.compile(program)
}

fn load_program(path: &Path) -> Result<Program, String> {
    let mut stack = Vec::new();
    let mut loaded = HashSet::new();
    load_program_inner(path, &mut stack, &mut loaded)
}

fn load_program_inner(
    path: &Path,
    stack: &mut Vec<PathBuf>,
    loaded: &mut HashSet<PathBuf>,
) -> Result<Program, String> {
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("cannot resolve '{}': {e}", path.display()))?;
    if stack.contains(&canonical) {
        let chain = stack
            .iter()
            .chain(std::iter::once(&canonical))
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(format!("import cycle detected: {chain}"));
    }
    if !loaded.insert(canonical.clone()) {
        return Ok(Program {
            funcs: Vec::new(),
            main: Vec::new(),
        });
    }

    let source = std::fs::read_to_string(&canonical)
        .map_err(|e| format!("cannot read '{}': {e}", canonical.display()))?;
    let tokens = lexer::lex(&source).map_err(|e| format!("{}: {e}", canonical.display()))?;
    let parsed = parser::Parser::new(tokens)
        .parse_program()
        .map_err(|e| format!("{}: {e}", canonical.display()))?;

    stack.push(canonical.clone());
    let mut program = Program {
        funcs: Vec::new(),
        main: Vec::new(),
    };
    for stmt in parsed.main {
        let ast::Stmt::Import { path: import, line } = stmt else {
            program.main.push(stmt);
            continue;
        };
        let import_path = resolve_import(&canonical, &import)
            .map_err(|e| format!("{}:{line}: {e}", canonical.display()))?;
        let imported = load_program_inner(&import_path, stack, loaded)
            .map_err(|e| format!("{}:{line}: while importing '{}': {e}", canonical.display(), import))?;
        program.funcs.extend(imported.funcs);
        program.main.extend(imported.main);
    }
    program.funcs.extend(parsed.funcs);
    stack.pop();
    Ok(program)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_snippet_calls_stdlib_function() {
        let (lines, _warnings) = compile_snippet(
            "import \"std/all\"\ntap_button(\"a\", 5)\n",
            Path::new("."),
        )
        .expect("snippet should compile");
        assert_eq!(
            lines,
            vec!["0 gamepad button down a", "+5 gamepad button up a"]
        );
    }
}

fn resolve_import(current_file: &Path, requested: &str) -> Result<PathBuf, String> {
    if requested.starts_with("std/") || requested.starts_with("std\\") {
        let relative = requested[4..].replace('\\', "/");
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("stdlib");
        path.push(relative);
        if path.extension().is_none() {
            path.set_extension("stas");
        }
        Ok(path)
    } else {
        let mut path = current_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(requested);
        if path.extension().is_none() {
            path.set_extension("stas");
        }
        Ok(path)
    }
}
