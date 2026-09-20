//! Just enough JSON to read a calibration file: objects, arrays, numbers,
//! strings, literals. Kept in-crate so `keystone` stays dependency-free.

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub(crate) fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub(crate) fn number(&self) -> Option<f64> {
        match self {
            Json::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub(crate) fn boolean(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub(crate) fn array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }
}

pub(crate) fn parse(text: &str) -> Result<Json, String> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        at: 0,
    };
    let value = parser.value()?;
    parser.space();
    if parser.at < parser.bytes.len() {
        return Err(parser.error("unexpected text after the value"));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn error(&self, what: &str) -> String {
        let line = 1 + self.bytes[..self.at.min(self.bytes.len())]
            .iter()
            .filter(|&&b| b == b'\n')
            .count();
        format!("not valid JSON (line {line}): {what}")
    }

    fn space(&mut self) {
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }

    fn eat(&mut self, byte: u8) -> bool {
        self.space();
        let hit = self.bytes.get(self.at) == Some(&byte);
        if hit {
            self.at += 1;
        }
        hit
    }

    fn value(&mut self) -> Result<Json, String> {
        self.space();
        match self.bytes.get(self.at) {
            Some(b'{') => {
                self.at += 1;
                let mut fields = Vec::new();
                if !self.eat(b'}') {
                    loop {
                        self.space();
                        let key = self.string()?;
                        if !self.eat(b':') {
                            return Err(self.error("expected `:` after a key"));
                        }
                        fields.push((key, self.value()?));
                        if self.eat(b'}') {
                            break;
                        }
                        if !self.eat(b',') {
                            return Err(self.error("expected `,` or `}`"));
                        }
                    }
                }
                Ok(Json::Object(fields))
            }
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                if !self.eat(b']') {
                    loop {
                        items.push(self.value()?);
                        if self.eat(b']') {
                            break;
                        }
                        if !self.eat(b',') {
                            return Err(self.error("expected `,` or `]`"));
                        }
                    }
                }
                Ok(Json::Array(items))
            }
            Some(b'"') => self.string().map(Json::String),
            Some(_) => {
                let start = self.at;
                while self
                    .bytes
                    .get(self.at)
                    .is_some_and(|b| !b.is_ascii_whitespace() && !b",]}".contains(b))
                {
                    self.at += 1;
                }
                let word = std::str::from_utf8(&self.bytes[start..self.at]).unwrap_or("");
                match word {
                    "null" => Ok(Json::Null),
                    "true" => Ok(Json::Bool(true)),
                    "false" => Ok(Json::Bool(false)),
                    _ => word
                        .parse()
                        .map(Json::Number)
                        .map_err(|_| self.error(&format!("`{word}` is not a value"))),
                }
            }
            None => Err(self.error("the file ends early")),
        }
    }

    fn string(&mut self) -> Result<String, String> {
        if self.bytes.get(self.at) != Some(&b'"') {
            return Err(self.error("expected a string"));
        }
        self.at += 1;
        let mut out = Vec::new();
        loop {
            match self.bytes.get(self.at) {
                Some(b'"') => {
                    self.at += 1;
                    return String::from_utf8(out).map_err(|_| self.error("invalid UTF-8"));
                }
                Some(b'\\') => {
                    let escaped = match self.bytes.get(self.at + 1) {
                        Some(b'n') => b'\n',
                        Some(b't') => b'\t',
                        Some(&other @ (b'"' | b'\\' | b'/')) => other,
                        _ => return Err(self.error("unsupported escape")),
                    };
                    out.push(escaped);
                    self.at += 2;
                }
                Some(&byte) => {
                    out.push(byte);
                    self.at += 1;
                }
                None => return Err(self.error("unterminated string")),
            }
        }
    }
}
