//! Kanata's s-expressions (research R2): lists, atoms, `"strings"`, `r#"…"#`,
//! `;;` line comments and `#| … |#` block comments. A single `;` is an atom
//! (it is a key name).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SExpr {
    Atom(String),
    List(Vec<SExpr>),
}

impl SExpr {
    pub(crate) fn atom(&self) -> Option<&str> {
        match self {
            SExpr::Atom(a) => Some(a),
            SExpr::List(_) => None,
        }
    }

    pub(crate) fn list(&self) -> Option<&[SExpr]> {
        match self {
            SExpr::List(items) => Some(items),
            SExpr::Atom(_) => None,
        }
    }
}

/// Prints the expression back in config syntax (for raw labels).
impl fmt::Display for SExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SExpr::Atom(a) => f.write_str(a),
            SExpr::List(items) => {
                f.write_str("(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str(")")
            }
        }
    }
}

/// Where the config stops being readable. Never includes config text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxError {
    pub line: u32,
    pub column: u32,
}

/// Parse all top-level forms. On error, also returns the forms read before it.
pub(crate) fn parse(text: &str) -> Result<Vec<SExpr>, (Vec<SExpr>, SyntaxError)> {
    let mut lexer = Lexer {
        chars: text.chars().collect(),
        pos: 0,
        line: 1,
        column: 1,
    };
    let mut top = Vec::new();
    // Open lists, each with the position of its `(`.
    let mut stack: Vec<(Vec<SExpr>, SyntaxError)> = Vec::new();

    loop {
        match lexer.next_token() {
            Err(at) => return Err((top, at)),
            Ok(None) => {
                return match stack.first() {
                    Some((_, open)) => Err((top, *open)),
                    None => Ok(top),
                };
            }
            Ok(Some((here, Token::Open))) => stack.push((Vec::new(), here)),
            Ok(Some((here, Token::Close))) => {
                let Some((items, _)) = stack.pop() else {
                    return Err((top, here));
                };
                push(&mut stack, &mut top, SExpr::List(items));
            }
            Ok(Some((_, Token::Atom(a)))) => push(&mut stack, &mut top, SExpr::Atom(a)),
        }
    }
}

fn push(stack: &mut [(Vec<SExpr>, SyntaxError)], top: &mut Vec<SExpr>, e: SExpr) {
    match stack.last_mut() {
        Some((items, _)) => items.push(e),
        None => top.push(e),
    }
}

enum Token {
    Open,
    Close,
    Atom(String),
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: u32,
    column: u32,
}

impl Lexer {
    fn here(&self) -> SyntaxError {
        SyntaxError {
            line: self.line,
            column: self.column,
        }
    }

    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.pos + ahead).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek(0)?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    /// The next token and where it starts.
    fn next_token(&mut self) -> Result<Option<(SyntaxError, Token)>, SyntaxError> {
        loop {
            let here = self.here();
            let Some(c) = self.peek(0) else {
                return Ok(None);
            };
            match c {
                c if c.is_whitespace() => {
                    self.bump();
                }
                ';' if self.peek(1) == Some(';') => {
                    while self.peek(0).is_some_and(|c| c != '\n') {
                        self.bump();
                    }
                }
                '#' if self.peek(1) == Some('|') => {
                    let start = self.here();
                    self.bump();
                    self.bump();
                    loop {
                        match (self.peek(0), self.peek(1)) {
                            (Some('|'), Some('#')) => {
                                self.bump();
                                self.bump();
                                break;
                            }
                            (Some(_), _) => {
                                self.bump();
                            }
                            (None, _) => return Err(start),
                        }
                    }
                }
                '(' => {
                    self.bump();
                    return Ok(Some((here, Token::Open)));
                }
                ')' => {
                    self.bump();
                    return Ok(Some((here, Token::Close)));
                }
                '"' => return self.string("\"").map(|t| Some((here, t))),
                'r' if self.peek(1) == Some('#') && self.peek(2) == Some('"') => {
                    self.bump();
                    self.bump();
                    return self.string("\"#").map(|t| Some((here, t)));
                }
                _ => {
                    let mut atom = String::new();
                    while let Some(c) = self.peek(0) {
                        if c.is_whitespace() || c == '(' || c == ')' {
                            break;
                        }
                        atom.push(c);
                        self.bump();
                    }
                    return Ok(Some((here, Token::Atom(atom))));
                }
            }
        }
    }

    /// A string starting at the opening `"`, up to `end`.
    fn string(&mut self, end: &str) -> Result<Token, SyntaxError> {
        let start = self.here();
        self.bump(); // opening quote
        let end: Vec<char> = end.chars().collect();
        let mut text = String::new();
        loop {
            if (0..end.len()).all(|i| self.peek(i) == Some(end[i])) {
                for _ in 0..end.len() {
                    self.bump();
                }
                return Ok(Token::Atom(text));
            }
            match self.bump() {
                Some(c) => text.push(c),
                None => return Err(start),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(s: &str) -> SExpr {
        SExpr::Atom(s.to_owned())
    }

    #[test]
    fn nested_lists() {
        let forms = parse("(a (b c) d)").unwrap();
        assert_eq!(
            forms,
            vec![SExpr::List(vec![
                atom("a"),
                SExpr::List(vec![atom("b"), atom("c")]),
                atom("d")
            ])]
        );
    }

    #[test]
    fn double_semicolon_is_a_comment_single_is_a_key() {
        let forms = parse("(defsrc ; l) ;; comment (not a form)\n(x)").unwrap();
        assert_eq!(forms.len(), 2);
        assert_eq!(forms[0].list().unwrap()[1], atom(";"));
    }

    #[test]
    fn block_comment_spanning_lines() {
        let forms = parse("#| one\n (two)\n three |#\n(x)").unwrap();
        assert_eq!(forms, vec![SExpr::List(vec![atom("x")])]);
    }

    #[test]
    fn strings() {
        let forms = parse(r##"(a "with space" r#"raw "quoted""#)"##).unwrap();
        let items = forms[0].list().unwrap();
        assert_eq!(items[1], atom("with space"));
        assert_eq!(items[2], atom(r#"raw "quoted""#));
    }

    #[test]
    fn unicode_atoms() {
        let forms = parse("(⏎ ‹⇧a 🔣λ)").unwrap();
        let items = forms[0].list().unwrap();
        assert_eq!(items[0], atom("⏎"));
        assert_eq!(items[1], atom("‹⇧a"));
        assert_eq!(items[2], atom("🔣λ"));
    }

    #[test]
    fn unbalanced_open_reports_its_position_and_keeps_earlier_forms() {
        let (forms, at) = parse("(ok)\n\n  (broken a b").unwrap_err();
        assert_eq!(forms.len(), 1);
        assert_eq!(at, SyntaxError { line: 3, column: 3 });
    }

    #[test]
    fn stray_close() {
        let (_, at) = parse("(a))").unwrap_err();
        assert_eq!(at, SyntaxError { line: 1, column: 4 });
    }

    #[test]
    fn displays_back_as_config_text() {
        let forms = parse("(tap-hold 200 200 esc (layer-while-held nav))").unwrap();
        assert_eq!(
            forms[0].to_string(),
            "(tap-hold 200 200 esc (layer-while-held nav))"
        );
    }
}
