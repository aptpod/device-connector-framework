use anyhow::bail;
use dc_core::{
    ElementBuildable, ElementResult, ElementValue, Error, MsgReceiver, MsgType, Pipeline, Port,
};
use serde::Deserialize;
use serde_with::{serde_as, DurationMilliSecondsWithFrac};
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

/// Generate text message.
pub struct TextSrcElement {
    conf: TextSrcElementConf,
    count: usize,
}

/// Configuration type for `TextSrcElement`
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextSrcElementConf {
    /// Text to send.
    pub text: String,
    /// Duration of sending message.
    #[serde_as(as = "DurationMilliSecondsWithFrac<f64>")]
    #[serde(alias = "duration_ms")]
    pub interval_ms: Duration,
    /// The number of message repeatation until next sleep.
    #[serde(default)]
    #[serde(alias = "repeat_until_sleep")]
    pub repeat: usize,
}

impl ElementBuildable for TextSrcElement {
    type Config = TextSrcElementConf;

    const NAME: &'static str = "text-src";
    const DESCRIPTION: &'static str = "Generate text messages.";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| text | string | Test to send. |
| interval_ms | real | Interval to send in milli seconds |
| repeat | integer | The number of message repeatation until next interval. The default value is 1. |
"#;

    const SEND_PORTS: Port = 1;

    fn new(mut conf: Self::Config) -> Result<Self, Error> {
        if conf.repeat == 0 {
            conf.repeat = 1;
        }
        Ok(TextSrcElement { conf, count: 0 })
    }

    fn next(&mut self, pipeline: &mut Pipeline, _receiver: &mut MsgReceiver) -> ElementResult {
        let mut buf = pipeline.msg_buf(0);

        self.count += 1;
        if self.count == self.conf.repeat {
            sleep(self.conf.interval_ms);
            self.count = 0;
        }
        buf.write_all(self.conf.text.as_bytes())?;

        Ok(ElementValue::MsgBuf)
    }
}

/// Split text data by delimiter
pub struct SplitByDelimiterFilterElement {
    conf: SplitByDelimiterFilterElementConfig,
    buf: Vec<u8>,
    scanned: bool,
    delimiter_len: usize,
    parser: Parser,
}

#[derive(Debug, Deserialize)]
/// Configuration type for `SplitByDelimiterFilterElement`
pub struct SplitByDelimiterFilterElementConfig {
    /// Delimiter string
    #[serde(default = "default_delimiter")]
    pub delimiter: String,
    /// Buffer limit size
    #[serde(default = "default_limit_size")]
    pub limit_size: usize,
}

fn default_delimiter() -> String {
    "\n".into()
}

fn default_limit_size() -> usize {
    1024 * 1024 * 1024
}

impl ElementBuildable for SplitByDelimiterFilterElement {
    type Config = SplitByDelimiterFilterElementConfig;

    const NAME: &'static str = "split-by-delimiter-filter";
    const DESCRIPTION: &'static str = "Split and merge messages by delimiter.";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| delimiter | string | Delimiter string. |
| limit_size | integer | Maximam buffer size. The default value is 1GiB. |
"#;

    const RECV_PORTS: Port = 1;
    const SEND_PORTS: Port = 1;

    fn recv_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn send_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        if conf.delimiter.is_empty() {
            bail!("empty delimiter");
        }
        let delimiter = conf.delimiter.as_bytes().to_vec();
        let delimiter_len = delimiter.len();

        Ok(Self {
            conf,
            buf: Vec::with_capacity(1024),
            scanned: true,
            delimiter_len,
            parser: Parser {
                delimiter,
                stack: Vec::with_capacity(delimiter_len),
            },
        })
    }

    #[allow(clippy::collapsible_else_if)]
    fn next(&mut self, pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        loop {
            if self.buf.len() > self.conf.limit_size {
                bail!("reached buffer limit size");
            }

            if self.scanned || self.buf.is_empty() {
                let msg = receiver.recv(0)?;
                let msg_bytes = msg.as_bytes();

                if let Some(i) = self.parser.search_delimitor_end(msg_bytes)? {
                    let mut buf = pipeline.msg_buf(0);

                    if i + 1 > self.delimiter_len {
                        let delimiter_start = i + 1 - self.delimiter_len;
                        buf.write_all(&self.buf)?;
                        buf.write_all(&msg_bytes[0..delimiter_start])?;
                    } else {
                        let delimiter_start = self.buf.len() - self.delimiter_len + i + 1;
                        buf.write_all(&self.buf[0..delimiter_start])?;
                    }

                    self.buf.clear();
                    if msg_bytes.len() > i + 1 {
                        self.buf.extend_from_slice(&msg_bytes[(i + 1)..]);
                        self.scanned = false;
                    } else {
                        self.scanned = true;
                    }

                    return Ok(ElementValue::MsgBuf);
                } else {
                    self.buf.extend_from_slice(msg_bytes);
                    self.scanned = true;
                    continue;
                }
            } else {
                if let Some(i) = self.parser.search_delimitor_end(&self.buf)? {
                    let mut buf = pipeline.msg_buf(0);

                    let delimiter_start = i + 1 - self.delimiter_len;
                    buf.write_all(&self.buf[0..delimiter_start])?;

                    let remaining = self.buf.len() - (i + 1);
                    if remaining > 0 {
                        self.buf.copy_within((i + 1).., 0);
                        self.buf.resize_with(remaining, || unreachable!());
                        self.scanned = false;
                    } else {
                        self.buf.clear();
                        self.scanned = true;
                    }

                    return Ok(ElementValue::MsgBuf);
                } else {
                    self.scanned = true;
                    continue;
                }
            }
        }
    }
}

#[derive(Default, Debug)]
struct Parser {
    delimiter: Vec<u8>,
    stack: Vec<u8>,
}

impl Parser {
    fn search_delimitor_end(&mut self, s: &[u8]) -> Result<Option<usize>, Error> {
        for (i, &c) in s.iter().enumerate() {
            if let Some(next_byte) = self.delimiter.get(self.stack.len()).copied() {
                if c == next_byte {
                    self.stack.push(c);
                }
            }

            if self.stack.len() == self.delimiter.len() {
                self.clear();
                return Ok(Some(i));
            }
        }
        Ok(None)
    }

    fn clear(&mut self) {
        self.stack.clear();
    }
}

/// Split json by value
pub struct JsonSplitFilterElement {
    conf: JsonSplitFilterElementConfig,
    buf: Vec<u8>,
    scanned: bool,
    parser: JsonParser,
}

/// Configuration type for `JsonSplitFilterElement`
#[derive(Debug, Deserialize)]
pub struct JsonSplitFilterElementConfig {
    #[serde(default = "default_limit_size")]
    pub limit_size: usize,
}

impl ElementBuildable for JsonSplitFilterElement {
    type Config = JsonSplitFilterElementConfig;

    const NAME: &'static str = "json-split-filter";
    const DESCRIPTION: &'static str = "Split and merge messages by json value.";
    const CONFIG_DOC: &'static str = r#"
| Field | Type | Description |
| --- | --- | --- |
| limit_size | integer | Maximam buffer size. The default value is 1GiB. |
"#;

    const RECV_PORTS: Port = 1;
    const SEND_PORTS: Port = 1;

    fn recv_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::any()]]
    }

    fn send_msg_types() -> Vec<Vec<MsgType>> {
        vec![vec![MsgType::from_mime("application/json").unwrap()]]
    }

    fn new(conf: Self::Config) -> Result<Self, Error> {
        Ok(Self {
            conf,
            buf: Vec::with_capacity(1024),
            scanned: true,
            parser: JsonParser::default(),
        })
    }

    #[allow(clippy::collapsible_else_if)]
    fn next(&mut self, pipeline: &mut Pipeline, receiver: &mut MsgReceiver) -> ElementResult {
        loop {
            if self.buf.len() > self.conf.limit_size {
                bail!("reached buffer limit size");
            }

            if self.scanned || self.buf.is_empty() {
                let msg = receiver.recv(0)?;
                let msg_bytes = msg.as_bytes();

                if let Some(i) = self.parser.search_json_end(msg_bytes)? {
                    let mut buf = pipeline.msg_buf(0);
                    buf.write_all(&self.buf)?;
                    buf.write_all(&msg_bytes[0..=i])?;
                    self.buf.clear();
                    if msg_bytes.len() > i + 1 {
                        self.buf.extend_from_slice(&msg_bytes[(i + 1)..]);
                        self.scanned = false;
                    } else {
                        self.scanned = true;
                    }

                    return Ok(ElementValue::MsgBuf);
                } else {
                    self.buf.extend_from_slice(msg_bytes);
                    self.scanned = true;
                    continue;
                }
            } else {
                if let Some(i) = self.parser.search_json_end(&self.buf)? {
                    let mut buf = pipeline.msg_buf(0);
                    buf.write_all(&self.buf[0..=i])?;
                    let remaining = self.buf.len() - (i + 1);
                    if remaining > 0 {
                        self.buf.copy_within((i + 1).., 0);
                        self.buf.resize_with(remaining, || unreachable!());
                        self.scanned = false;
                    } else {
                        self.buf.clear();
                        self.scanned = true;
                    }

                    return Ok(ElementValue::MsgBuf);
                } else {
                    self.scanned = true;
                    continue;
                }
            }
        }
    }
}

#[derive(Default, Debug)]
struct JsonParser {
    non_whitespace_found: bool,
    in_string: bool,
    escape: bool,
    stack: Vec<JsonStructure>,
    word: Vec<u8>,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum JsonStructure {
    Map,
    Array,
    String,
}

impl JsonParser {
    fn search_json_end(&mut self, s: &[u8]) -> Result<Option<usize>, Error> {
        for (i, &c) in s.iter().enumerate() {
            let is_ascii_whitespace = c.is_ascii_whitespace();
            if !is_ascii_whitespace {
                self.non_whitespace_found = true;
            }

            if !self.in_string {
                match c {
                    b'{' => {
                        self.push(JsonStructure::Map);
                    }
                    b'}' => {
                        self.pop(JsonStructure::Map)?;
                    }
                    b'[' => {
                        self.push(JsonStructure::Array);
                    }
                    b']' => {
                        self.pop(JsonStructure::Array)?;
                    }
                    b'"' => {
                        self.push(JsonStructure::String);
                        self.in_string = true;
                    }
                    c => {
                        if self.stack.is_empty() {
                            if !is_ascii_whitespace {
                                self.word.push(c);
                            } else if !self.word.is_empty() {
                                self.check_word()?;
                                self.clear();
                                return Ok(Some(i));
                            }
                        }
                    }
                }
            } else if c == b'"' && !self.escape {
                self.pop(JsonStructure::String)?;
                self.in_string = false;
            }

            self.escape = self.in_string && c == b'\\';

            if self.stack.is_empty() && self.word.is_empty() && self.non_whitespace_found {
                self.clear();
                return Ok(Some(i));
            }
        }
        Ok(None)
    }

    fn clear(&mut self) {
        debug_assert!(self.stack.is_empty());
        debug_assert!(!self.in_string);
        self.word.clear();
        self.escape = false;
        self.non_whitespace_found = false;
    }

    fn push(&mut self, sturcture: JsonStructure) {
        self.stack.push(sturcture);
    }

    fn pop(&mut self, structure: JsonStructure) -> Result<(), Error> {
        if self.stack.last().copied() == Some(structure) {
            self.stack.pop();
            Ok(())
        } else {
            bail!("json parse error");
        }
    }

    fn check_word(&mut self) -> Result<(), Error> {
        match &self.word[..] {
            &[b'n', b'u', b'l', b'l']
            | &[b't', b'r', b'u', b'e']
            | &[b'f', b'a', b'l', b's', b'e'] => Ok(()),
            w => {
                if w.iter().all(|c| c.is_ascii_digit()) {
                    Ok(())
                } else {
                    bail!("json parse error")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_parser_test() {
        let mut parser = Parser {
            delimiter: b",".to_vec(),
            ..Default::default()
        };

        assert_eq!(parser.search_delimitor_end(b"null,").unwrap(), Some(4));
        assert_eq!(parser.search_delimitor_end(b"000").unwrap(), None);
        assert_eq!(parser.search_delimitor_end(b"00,").unwrap(), Some(2));

        let mut parser = Parser {
            delimiter: b"EOS".to_vec(),
            ..Default::default()
        };

        assert_eq!(
            parser.search_delimitor_end(b"hogeEOSfuga").unwrap(),
            Some(6)
        );
        assert_eq!(parser.search_delimitor_end(b"000EO").unwrap(), None);
        assert_eq!(parser.search_delimitor_end(b"S111").unwrap(), Some(0));
    }

    #[test]
    fn json_parser_test() {
        let mut parser = JsonParser::default();

        assert_eq!(parser.search_json_end(b"null\n").unwrap(), Some(4));

        assert_eq!(parser.search_json_end(b"tr").unwrap(), None);
        assert_eq!(parser.search_json_end(b"ue\n").unwrap(), Some(2));

        assert_eq!(parser.search_json_end(br#"{ "value":"#).unwrap(), None);
        assert_eq!(parser.search_json_end(br#" "hoge"}"#).unwrap(), Some(7));

        assert_eq!(parser.search_json_end(br#"{"#).unwrap(), None);
        assert_eq!(
            parser.search_json_end(br#"  "value": "hoge","#).unwrap(),
            None
        );
        assert_eq!(
            parser.search_json_end(br#"  "array": [0, 1, 2]"#).unwrap(),
            None
        );
        assert_eq!(
            parser
                .search_json_end(br#""map": { "value": "fuga" }"#)
                .unwrap(),
            None
        );
        assert_eq!(parser.search_json_end(br#"}"#).unwrap(), Some(0));
    }
}
