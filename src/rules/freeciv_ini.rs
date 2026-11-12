//! Parser for Freeciv's native section-file format.
//!
//! This is deliberately a syntax layer. It preserves source order and repeated
//! sections; ruleset-specific validation and include resolution belong to the
//! loader above it. The accepted grammar follows the constructs in Freeciv's
//! `utility/registry_ini.c` and the pinned `civ2civ3` files.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) struct SectionFile {
    pub(crate) sections: Vec<Section>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Section {
    pub(crate) name: String,
    pub(crate) entries: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) key: String,
    pub(crate) value: Value,
    pub(crate) line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Value {
    String(String),
    Integer(i64),
    /// Native Freeciv float tokens retain their source spelling.
    Float(String),
    Bool(bool),
    List(Vec<Value>),
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<Value>>,
    },
    Empty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ParseError {
    pub(crate) line: usize,
    pub(crate) context: String,
    pub(crate) message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.context.is_empty() {
            write!(f, "line {}: {}", self.line, self.message)
        } else {
            write!(f, "line {} [{}]: {}", self.line, self.context, self.message)
        }
    }
}

impl std::error::Error for ParseError {}

pub(crate) fn parse(input: &str) -> Result<SectionFile, ParseError> {
    Parser::new(input).parse()
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    section: Option<String>,
    key: Option<String>,
}

impl Parser {
    fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            line: 1,
            section: None,
            key: None,
        }
    }

    fn parse(mut self) -> Result<SectionFile, ParseError> {
        let mut file = SectionFile::default();
        while self.skip_blank_and_comments() {
            match self.peek() {
                Some('[') => {
                    let section = self.parse_section_header()?;
                    self.section = Some(section.clone());
                    file.sections.push(Section {
                        name: section,
                        entries: Vec::new(),
                    });
                }
                Some(_) => {
                    let entry = self.parse_entry()?;
                    let Some(section) = file.sections.last_mut() else {
                        return Err(self.error("entry appears before the first section"));
                    };
                    section.entries.push(entry);
                }
                None => break,
            }
        }
        Ok(file)
    }

    fn parse_section_header(&mut self) -> Result<String, ParseError> {
        let start_line = self.line;
        self.bump(); // [
        let mut name = String::new();
        while let Some(ch) = self.peek() {
            match ch {
                ']' => {
                    self.bump();
                    self.skip_horizontal_space();
                    if self
                        .peek()
                        .is_some_and(|c| c != '\n' && c != ';' && c != '#')
                    {
                        return Err(
                            self.error_at(start_line, "unexpected text after section header")
                        );
                    }
                    self.consume_comment();
                    self.consume_newline();
                    return Ok(name.trim().to_owned());
                }
                '\n' => return Err(self.error_at(start_line, "unterminated section header")),
                _ => name.push(self.bump().unwrap()),
            }
        }
        Err(self.error_at(start_line, "unterminated section header"))
    }

    fn parse_entry(&mut self) -> Result<Entry, ParseError> {
        let entry_line = self.line;
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch == '=' || ch == '\n' {
                break;
            }
            self.bump();
        }
        let head: String = self.chars[start..self.pos].iter().collect();
        let head = head.trim();

        // Freeciv includes are line directives rather than key/value pairs.
        // Keep them in the AST so the loader can resolve them at the right
        // point in the ordered stream.
        if head == "*include" || head.starts_with("*include ") || head.starts_with("*include\t") {
            self.pos = start;
            self.skip_horizontal_space();
            for expected in "*include".chars() {
                if self.bump() != Some(expected) {
                    return Err(self.error_at(entry_line, "malformed *include directive"));
                }
            }
            if !matches!(self.peek(), Some(' ' | '\t')) {
                return Err(self.error_at(entry_line, "expected a quoted path after *include"));
            }
            self.skip_horizontal_space();
            let path = self.parse_string_like()?;
            self.skip_horizontal_space();
            if self
                .peek()
                .is_some_and(|c| c != '\n' && c != ';' && c != '#')
            {
                return Err(self.error_at(entry_line, "unexpected text after *include path"));
            }
            self.consume_comment();
            self.consume_newline();
            return Ok(Entry {
                key: "*include".to_owned(),
                value: Value::String(path),
                line: entry_line,
            });
        }

        let key = head.to_owned();
        if key.is_empty() {
            return Err(self.error_at(entry_line, "expected a key or section header"));
        }
        if self.peek() != Some('=') {
            return Err(self.error_at(entry_line, "expected '=' after key"));
        }
        self.bump();
        self.key = Some(key.clone());
        let value = self.parse_value()?;
        self.key = None;
        Ok(Entry {
            key,
            value,
            line: entry_line,
        })
    }

    fn parse_value(&mut self) -> Result<Value, ParseError> {
        self.skip_horizontal_space();
        let mut crossed_newline = false;
        while self.peek() == Some(';') || self.peek() == Some('#') || self.peek() == Some('\n') {
            if self.peek() == Some('\n') {
                crossed_newline = true;
                self.bump();
            } else {
                self.consume_comment();
                if self.peek() == Some('\n') {
                    crossed_newline = true;
                    self.bump();
                }
            }
            self.skip_horizontal_space();
        }
        if self.peek().is_none()
            || (crossed_newline
                && self.peek() != Some('{')
                && (self.peek() == Some('[') || self.looks_like_assignment()))
        {
            return Ok(Value::Empty);
        }
        if self.peek() == Some('{') {
            return self.parse_table();
        }

        let mut values = Vec::new();
        loop {
            values.push(self.parse_atom()?);
            self.skip_horizontal_space();
            match self.peek() {
                Some(',') => {
                    self.bump();
                    self.skip_continuation_trivia();
                    if self.peek().is_none() {
                        return Err(self.error("expected a value after ','"));
                    }
                }
                Some(';') | Some('#') => {
                    self.consume_comment();
                    self.consume_newline();
                    break;
                }
                Some('\n') => {
                    self.bump();
                    break;
                }
                None => break,
                Some(ch) => {
                    return Err(self.error(&format!("unexpected character {ch:?} after value")));
                }
            }
        }
        if values.len() == 1 {
            Ok(values.pop().unwrap())
        } else {
            Ok(Value::List(values))
        }
    }

    fn parse_table(&mut self) -> Result<Value, ParseError> {
        self.bump(); // {
        let mut logical_rows: Vec<Vec<Value>> = Vec::new();
        let mut row = Vec::new();
        let mut need_value = true;
        loop {
            self.skip_horizontal_space();
            self.consume_comment();
            match self.peek() {
                Some('\n') => {
                    self.bump();
                    if !row.is_empty() && !need_value {
                        logical_rows.push(std::mem::take(&mut row));
                        need_value = true;
                    }
                }
                Some('}') => {
                    self.bump();
                    if !row.is_empty() {
                        logical_rows.push(row);
                    }
                    self.skip_horizontal_space();
                    if self
                        .peek()
                        .is_some_and(|c| c != '\n' && c != ';' && c != '#')
                    {
                        return Err(self.error("unexpected text after table"));
                    }
                    self.consume_comment();
                    self.consume_newline();
                    break;
                }
                None => return Err(self.error("unterminated table")),
                Some(',') => {
                    self.bump();
                    need_value = true;
                }
                Some(_) => {
                    if !need_value {
                        return Err(self.error("expected ',' or end of table row"));
                    }
                    row.push(self.parse_atom()?);
                    need_value = false;
                    self.skip_horizontal_space();
                    match self.peek() {
                        Some(',') => {
                            self.bump();
                            need_value = true;
                        }
                        Some('\n') => {
                            self.bump();
                            if !row.is_empty() && !need_value {
                                logical_rows.push(std::mem::take(&mut row));
                                need_value = true;
                            }
                        }
                        Some(';') | Some('#') => {
                            self.consume_comment();
                            if self.peek() == Some('\n') {
                                self.bump();
                            }
                            if !row.is_empty() && !need_value {
                                logical_rows.push(std::mem::take(&mut row));
                                need_value = true;
                            }
                        }
                        Some('}') => {}
                        None => return Err(self.error("unterminated table")),
                        Some(ch) => {
                            return Err(
                                self.error(&format!("unexpected character {ch:?} in table"))
                            );
                        }
                    }
                }
            }
        }

        let Some(header) = logical_rows.first() else {
            return Err(self.error("table is missing its column header row"));
        };
        let columns = header
            .iter()
            .enumerate()
            .map(|(index, value)| match value {
                Value::String(value) => Ok(value.clone()),
                _ => Err(self.error(&format!("table column {} is not a string", index + 1))),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Value::Table {
            columns,
            rows: logical_rows.into_iter().skip(1).collect(),
        })
    }

    fn parse_atom(&mut self) -> Result<Value, ParseError> {
        self.skip_horizontal_space();
        match self.peek() {
            Some('"') => self.parse_string_like().map(Value::String),
            Some('_') if self.peek_n(1) == Some('(') => self.parse_string_like().map(Value::String),
            Some('*') => self.parse_file_reference().map(Value::String),
            Some(ch) if ch == '-' || ch == '+' || ch.is_ascii_digit() => {
                let start = self.pos;
                if ch == '-' || ch == '+' {
                    self.bump();
                }
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.bump();
                }
                let mut is_float = false;
                if self.peek() == Some('.') {
                    is_float = true;
                    self.bump();
                    while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                        self.bump();
                    }
                }
                let raw: String = self.chars[start..self.pos].iter().collect();
                if is_float {
                    raw.parse::<f32>()
                        .map(|_| Value::Float(raw.clone()))
                        .map_err(|_| self.error(&format!("invalid float value {raw:?}")))
                } else {
                    raw.parse::<i64>()
                        .map(Value::Integer)
                        .map_err(|_| self.error(&format!("integer value {raw:?} is out of range")))
                }
            }
            Some(ch) if ch.is_ascii_alphabetic() => {
                let start = self.pos;
                while self
                    .peek()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    self.bump();
                }
                let raw: String = self.chars[start..self.pos].iter().collect();
                match raw.to_ascii_lowercase().as_str() {
                    "true" => Ok(Value::Bool(true)),
                    "false" => Ok(Value::Bool(false)),
                    _ => Err(self.error(&format!("unsupported bare value {raw:?}"))),
                }
            }
            Some(ch) => Err(self.error(&format!("unexpected character {ch:?} in value"))),
            None => Err(self.error("expected a value")),
        }
    }

    fn parse_string_like(&mut self) -> Result<String, ParseError> {
        let gettext = self.peek() == Some('_') && self.peek_n(1) == Some('(');
        if gettext {
            self.bump();
            self.bump();
            self.skip_horizontal_space();
        }
        let value = self.parse_quoted_string()?;
        if gettext {
            self.skip_horizontal_space();
            if self.peek() != Some(')') {
                return Err(self.error("expected ')' after gettext string"));
            }
            self.bump();
        }
        Ok(value)
    }

    fn parse_quoted_string(&mut self) -> Result<String, ParseError> {
        if self.peek() != Some('"') {
            return Err(self.error("expected a quoted string"));
        }
        self.bump();
        let mut result = String::new();
        loop {
            match self.bump() {
                Some('"') => return Ok(result),
                Some('\\') => match self.bump() {
                    Some('n') => result.push('\n'),
                    Some('\\') => result.push('\\'),
                    Some('"') => result.push('"'),
                    Some('\n') => result.push('\n'),
                    Some(other) => result.push(other),
                    None => return Err(self.error("unterminated escape in string")),
                },
                Some(ch) => result.push(ch),
                None => return Err(self.error("unterminated quoted string")),
            }
        }
    }

    fn parse_file_reference(&mut self) -> Result<String, ParseError> {
        self.bump(); // *
        let mut path = String::new();
        while let Some(ch) = self.peek() {
            if ch == '*' {
                self.bump();
                if path.is_empty() {
                    return Err(self.error("empty file reference"));
                }
                return Ok(path);
            }
            if ch == '\n' {
                return Err(self.error("unterminated file reference"));
            }
            path.push(self.bump().unwrap());
        }
        Err(self.error("unterminated file reference"))
    }

    fn skip_blank_and_comments(&mut self) -> bool {
        loop {
            self.skip_horizontal_space();
            if matches!(self.peek(), Some(';') | Some('#')) {
                self.consume_comment();
            }
            if self.peek() == Some('\n') {
                self.bump();
                continue;
            }
            return self.peek().is_some();
        }
    }

    fn skip_horizontal_space(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\r')) {
            self.bump();
        }
    }

    fn skip_continuation_trivia(&mut self) {
        loop {
            self.skip_horizontal_space();
            self.consume_comment();
            if self.peek() == Some('\n') {
                self.bump();
                continue;
            }
            break;
        }
        self.skip_horizontal_space();
    }

    fn looks_like_assignment(&self) -> bool {
        let mut quoted = false;
        let mut escaped = false;
        for ch in self.chars[self.pos..].iter().copied() {
            if ch == '\n' {
                break;
            }
            if !quoted && (ch == ';' || ch == '#') {
                break;
            }
            if quoted {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    quoted = false;
                }
            } else if ch == '"' {
                quoted = true;
            } else if ch == '=' {
                return true;
            }
        }
        false
    }

    fn consume_comment(&mut self) {
        if matches!(self.peek(), Some(';') | Some('#')) {
            while self.peek().is_some_and(|ch| ch != '\n') {
                self.bump();
            }
        }
    }

    fn consume_newline(&mut self) {
        if self.peek() == Some('\n') {
            self.bump();
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_n(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
        }
        Some(ch)
    }

    fn error(&self, message: &str) -> ParseError {
        self.error_at(self.line, message)
    }

    fn error_at(&self, line: usize, message: &str) -> ParseError {
        let context = match (&self.section, &self.key) {
            (Some(section), Some(key)) => format!("{section}.{key}"),
            (Some(section), None) => section.clone(),
            (None, Some(key)) => key.clone(),
            (None, None) => String::new(),
        };
        ParseError {
            line,
            context,
            message: message.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Value, parse};

    #[test]
    fn parses_lowercase_boolean_spellings() {
        let parsed = parse("[x]\nenabled = true\ndisabled = false\n").unwrap();
        assert_eq!(parsed.sections[0].entries[0].value, Value::Bool(true));
        assert_eq!(parsed.sections[0].entries[1].value, Value::Bool(false));
    }

    #[test]
    fn parses_sections_scalars_lists_gettext_and_comments() {
        let parsed = parse(
            r#"; preamble
[about]
name = _("CivCraft") # trailing
turns = 42
enabled = TRUE
disabled = FALSE
names = "one", _("two")
empty =
"#,
        )
        .unwrap();
        let section = &parsed.sections[0];
        assert_eq!(section.name, "about");
        assert_eq!(section.entries[0].value, Value::String("CivCraft".into()));
        assert_eq!(section.entries[1].value, Value::Integer(42));
        assert_eq!(section.entries[2].value, Value::Bool(true));
        assert_eq!(section.entries[3].value, Value::Bool(false));
        assert_eq!(
            section.entries[4].value,
            Value::List(vec![
                Value::String("one".into()),
                Value::String("two".into())
            ])
        );
        assert_eq!(section.entries[5].value, Value::Empty);
    }

    #[test]
    fn parses_freeciv_float_and_signed_integer_tokens() {
        let parsed = parse("[parameters]\nratio = -0.75\nshift = +2\n").unwrap();
        assert_eq!(
            parsed.sections[0].entries[0].value,
            Value::Float("-0.75".into())
        );
        assert_eq!(parsed.sections[0].entries[1].value, Value::Integer(2));
    }

    #[test]
    fn parses_multiline_strings_and_continued_vectors() {
        let parsed = parse(
            r#"[x]
text = _("first
second")
items = 1, ; continued
 2,
 3
"#,
        )
        .unwrap();
        assert_eq!(
            parsed.sections[0].entries[0].value,
            Value::String("first\nsecond".into())
        );
        assert_eq!(
            parsed.sections[0].entries[1].value,
            Value::List(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3)
            ])
        );
    }

    #[test]
    fn parses_tables_with_short_and_long_rows() {
        let parsed = parse("[x]\nrows =\n{\n\"a\", \"b\", \"c\"\n1, 2\n3, 4, 5, 6\n}\n").unwrap();
        assert_eq!(
            parsed.sections[0].entries[0].value,
            Value::Table {
                columns: vec!["a".into(), "b".into(), "c".into()],
                rows: vec![
                    vec![Value::Integer(1), Value::Integer(2)],
                    vec![
                        Value::Integer(3),
                        Value::Integer(4),
                        Value::Integer(5),
                        Value::Integer(6)
                    ]
                ]
            }
        );
    }

    #[test]
    fn preserves_duplicate_sections_and_include_directives() {
        let parsed = parse("[x]\na = 1\n*include \"other.ruleset\"\n[x]\nb = 2\n").unwrap();
        assert_eq!(parsed.sections.len(), 2);
        assert_eq!(parsed.sections[0].entries[1].key, "*include");
        assert_eq!(
            parsed.sections[0].entries[1].value,
            Value::String("other.ruleset".into())
        );
    }

    #[test]
    fn errors_report_line_and_section_key_context() {
        let error = parse("[thing]\nvalue = maybe\n").unwrap_err();
        assert_eq!(error.line, 2);
        assert_eq!(error.context, "thing.value");
        assert!(error.to_string().contains("unsupported bare value"));
    }
}
