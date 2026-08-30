mod actions;
mod ast;
mod interp;
mod lexer;
mod parser;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
stasc - compiles a .stas script into a SoulsTAS .tas script

Usage:
    stasc <input.stas> [-o <output.tas>] [-d <dir>] [--stdout] [--quiet]

Options:
    -o, --output <file>  Exact output path (overrides --output-dir)
    -d, --output-dir <d> Folder for the generated .tas file (default: output)
        --stdout         Write the result to stdout instead of a file
    -q, --quiet          Do not print warnings
    -h, --help           Show this help

By default the result is written to output/<script name>.tas, relative to the
current working directory. The folder is created if it does not exist.
";

const DEFAULT_OUTPUT_DIR: &str = "output";

struct Args {
    input: PathBuf,
    output: Option<PathBuf>,
    output_dir: PathBuf,
    to_stdout: bool,
    quiet: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut output_dir = PathBuf::from(DEFAULT_OUTPUT_DIR);
    let mut to_stdout = false;
    let mut quiet = false;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            "-o" | "--output" => {
                let value = it.next().ok_or("missing value after -o/--output")?;
                output = Some(PathBuf::from(value));
            }
            "-d" | "--output-dir" => {
                let value = it.next().ok_or("missing value after -d/--output-dir")?;
                output_dir = PathBuf::from(value);
            }
            "--stdout" => to_stdout = true,
            "-q" | "--quiet" => quiet = true,
            other if other.starts_with('-') => {
                return Err(format!("unknown option '{other}'"));
            }
            other => {
                if input.is_some() {
                    return Err("more than one input file given".into());
                }
                input = Some(PathBuf::from(other));
            }
        }
    }

    Ok(Args {
        input: input.ok_or("no input file given")?,
        output,
        output_dir,
        to_stdout,
        quiet,
    })
}

#[cfg(test)]
fn compile(source: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let tokens = lexer::lex(source)?;
    let program = parser::Parser::new(tokens).parse_program()?;
    compile_program(&program)
}

fn compile_program(program: &ast::Program) -> Result<(Vec<String>, Vec<String>), String> {
    let compiler = interp::Compiler::new(&program)?;
    compiler.compile(&program)
}

fn load_program(path: &std::path::Path) -> Result<ast::Program, String> {
    let mut stack = Vec::new();
    let mut loaded = std::collections::HashSet::new();
    load_program_inner(path, &mut stack, &mut loaded)
}

fn load_program_inner(
    path: &std::path::Path,
    stack: &mut Vec<std::path::PathBuf>,
    loaded: &mut std::collections::HashSet<std::path::PathBuf>,
) -> Result<ast::Program, String> {
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
    // A module already fully loaded elsewhere in this compile (a "diamond"
    // import, e.g. two stdlib modules that both import std/gamepad) has
    // already contributed its functions once - importing it again must be a
    // no-op, or its funcs would be declared twice and fail as duplicates.
    if !loaded.insert(canonical.clone()) {
        return Ok(ast::Program {
            funcs: Vec::new(),
            main: Vec::new(),
        });
    }

    let source = std::fs::read_to_string(&canonical)
        .map_err(|e| format!("cannot read '{}': {e}", canonical.display()))?;
    let tokens = lexer::lex(&source)
        .map_err(|e| format!("{}: {e}", canonical.display()))?;
    let parsed = parser::Parser::new(tokens)
        .parse_program()
        .map_err(|e| format!("{}: {e}", canonical.display()))?;

    stack.push(canonical.clone());
    let mut program = ast::Program {
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

fn resolve_import(current_file: &std::path::Path, requested: &str) -> Result<std::path::PathBuf, String> {
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
            .unwrap_or_else(|| std::path::Path::new("."))
            .join(requested);
        if path.extension().is_none() {
            path.set_extension("stas");
        }
        Ok(path)
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;

    let program = load_program(&args.input)?;
    let (lines, warnings) = compile_program(&program)?;

    if !args.quiet {
        for w in &warnings {
            eprintln!("warning: {w}");
        }
    }

    let input_name = args
        .input
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| args.input.display().to_string());

    let mut text = format!("; Generated by stasc from {input_name}\n; Do not edit by hand.\n\n");
    for line in &lines {
        text.push_str(line);
        text.push('\n');
    }

    if args.to_stdout {
        print!("{text}");
        return Ok(());
    }

    let out_path = match args.output {
        Some(p) => p,
        None => args.output_dir.join(default_file_name(&args.input)),
    };
    if let Some(dir) = out_path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("cannot create folder '{}': {e}", dir.display()))?;
        }
    }
    std::fs::write(&out_path, text)
        .map_err(|e| format!("cannot write '{}': {e}", out_path.display()))?;

    if !args.quiet {
        let shown = out_path
            .canonicalize()
            .unwrap_or_else(|_| out_path.clone());
        eprintln!(
            "wrote {} action(s) to {}",
            lines.len(),
            shown.display().to_string().trim_start_matches(r"\\?\")
        );
    }
    Ok(())
}

fn default_file_name(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "script".to_string());
    PathBuf::from(format!("{stem}.tas"))
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::compile;

    fn out(src: &str) -> Vec<String> {
        compile(src).expect("compilation failed").0
    }

    #[test]
    fn basic_timing() {
        let lines = out("await focus\nwait 20\nkey down w\nwait 40\nkey up w\n");
        assert_eq!(
            lines,
            vec!["0 await focus", "+20 key down w", "+40 key up w"]
        );
    }

    #[test]
    fn functions_and_arguments() {
        let src = "\
fn tap(k, hold) {
    key down $k
    wait $hold
    key up $k
}
tap(\"w\", 5)
wait 10
tap(\"s\", 2)
";
        assert_eq!(
            out(src),
            vec![
                "0 key down w",
                "+5 key up w",
                "+10 key down s",
                "+2 key up s"
            ]
        );
    }

    #[test]
    fn variables_repeat_and_math() {
        let src = "\
const DELAY = 3
let n = 2
repeat $n {
    mouse button down left
    wait $DELAY * 2
    mouse button up left
    wait 1
}
";
        assert_eq!(
            out(src),
            vec![
                "0 mouse button down left",
                "+6 mouse button up left",
                "+1 mouse button down left",
                "+6 mouse button up left"
            ]
        );
    }

    #[test]
    fn conditionals_and_return_values() {
        let src = "\
fn double(x) {
    return $x * 2
}
let d = double(15)
at $d
gamepad axis stick_right_x 30000
if $d > 20 {
    fps 30
} else {
    fps 60
}
";
        assert_eq!(
            out(src),
            vec!["0 gamepad axis stick_right_x 30000", "+0 fps 30"]
        );
    }

    #[test]
    fn rejects_invalid_action_arguments() {
        assert!(compile("key sideways w\n").is_err());
        assert!(compile("gamepad stick left 90 5\n").is_err());
        assert!(compile("await something\n").is_err());
        assert!(compile("mouse move 10\n").is_err());
    }

    #[test]
    fn rejects_bad_scripts() {
        assert!(compile("wait -5\n").is_err());
        assert!(compile("const X = 1\nX = 2\n").is_err());
        assert!(compile("key down $missing\n").is_err());
        assert!(compile("nope(1)\n").is_err());
    }
}
