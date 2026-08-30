#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Float(f64),
    Str(String),
    Dollar,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Assign,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
    Bang,
    Newline,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
}

pub fn lex(src: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = src.trim_start_matches('\u{feff}').chars().collect();
    let mut out: Vec<Token> = Vec::new();
    let mut i = 0usize;
    let mut line = 1usize;

    macro_rules! push {
        ($t:expr) => {
            out.push(Token { tok: $t, line })
        };
    }

    while i < chars.len() {
        let c = chars[i];

        // Comments run to the end of the line, matching the .tas syntax.
        if c == ';' || c == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        if c == '\r' {
            i += 1;
            continue;
        }

        if c == '\n' {
            push!(Tok::Newline);
            line += 1;
            i += 1;
            continue;
        }

        if c == ' ' || c == '\t' {
            i += 1;
            continue;
        }

        if c.is_ascii_digit() {
            let start = i;
            let mut is_float = false;
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || (chars[i] == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()))
            {
                if chars[i] == '.' {
                    if is_float {
                        break;
                    }
                    is_float = true;
                }
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            if is_float {
                let v: f64 = text
                    .parse()
                    .map_err(|_| format!("line {line}: invalid number '{text}'"))?;
                push!(Tok::Float(v));
            } else {
                let v: i64 = text
                    .parse()
                    .map_err(|_| format!("line {line}: invalid number '{text}'"))?;
                push!(Tok::Int(v));
            }
            continue;
        }

        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            push!(Tok::Ident(text));
            continue;
        }

        if c == '"' {
            i += 1;
            let mut s = String::new();
            loop {
                if i >= chars.len() {
                    return Err(format!("line {line}: unterminated string literal"));
                }
                match chars[i] {
                    '"' => {
                        i += 1;
                        break;
                    }
                    '\\' => {
                        i += 1;
                        if i >= chars.len() {
                            return Err(format!("line {line}: unterminated escape sequence"));
                        }
                        let e = chars[i];
                        s.push(match e {
                            'n' => '\n',
                            't' => '\t',
                            '\\' => '\\',
                            '"' => '"',
                            other => other,
                        });
                        i += 1;
                    }
                    '\n' => return Err(format!("line {line}: unterminated string literal")),
                    other => {
                        s.push(other);
                        i += 1;
                    }
                }
            }
            push!(Tok::Str(s));
            continue;
        }

        let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
        let two_tok = match two.as_str() {
            "==" => Some(Tok::Eq),
            "!=" => Some(Tok::Ne),
            "<=" => Some(Tok::Le),
            ">=" => Some(Tok::Ge),
            "&&" => Some(Tok::AndAnd),
            "||" => Some(Tok::OrOr),
            _ => None,
        };
        if let Some(t) = two_tok {
            push!(t);
            i += 2;
            continue;
        }

        let one = match c {
            '$' => Tok::Dollar,
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            '{' => Tok::LBrace,
            '}' => Tok::RBrace,
            ',' => Tok::Comma,
            '=' => Tok::Assign,
            '+' => Tok::Plus,
            '-' => Tok::Minus,
            '*' => Tok::Star,
            '/' => Tok::Slash,
            '%' => Tok::Percent,
            '<' => Tok::Lt,
            '>' => Tok::Gt,
            '!' => Tok::Bang,
            other => return Err(format!("line {line}: unexpected character '{other}'")),
        };
        push!(one);
        i += 1;
    }

    out.push(Token {
        tok: Tok::Eof,
        line,
    });
    Ok(out)
}
