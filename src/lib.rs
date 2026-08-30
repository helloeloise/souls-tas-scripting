//! Library surface for stasc. Exposes the action-line grammar (`actions::validate`)
//! for external tools that build `.tas` lines without going through the compiler, and
//! `loader::compile_snippet` for tools (e.g. tas-studio) that want to invoke a stdlib
//! function and splice its compiled output into a hand/GUI-built script.

pub mod actions;
pub mod ast;
pub mod interp;
pub mod lexer;
pub mod loader;
pub mod parser;
