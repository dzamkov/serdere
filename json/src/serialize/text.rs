use crate::{JsonSerializer, JsonOutliner};
use serdere::{Outliner, Serializer, TextWriter};

/// A [`JsonSerializer`] which writes to a [`TextWriter`].
pub struct TextSerializer<Writer: TextWriter> {
    writer: Writer,
    config: TextSerializerConfig,
    depth: u32,
    in_key: bool,
    at_first: bool,
}

/// Encapsulates the configuration options for a [`TextSerializer`].
#[derive(Debug, Clone, Copy)]
pub struct TextSerializerConfig {
    /// The character sequence used for one indentation level (e.g. "\t" or "    "). If [`None`],
    /// the written JSON will be compact, without any line breaks or indentation.
    pub indent: Option<&'static str>,
}

#[allow(clippy::derivable_impls)]
impl Default for TextSerializerConfig {
    fn default() -> Self {
        Self { indent: None }
    }
}

impl<Writer: TextWriter> TextSerializer<Writer> {
    /// Constructs a new [`TextSerializer`] for writing a JSON value to a [`TextWriter`].
    /// The stack initially consists of a single value item.
    pub fn new(config: TextSerializerConfig, writer: Writer) -> Self {
        Self {
            writer,
            config,
            depth: 0,
            in_key: false,
            at_first: false,
        }
    }

    /// Closes the serializer and returns the underlying [`TextWriter`].
    pub fn close(self) -> Writer {
        self.writer
    }
}

impl<Writer: TextWriter> Outliner for TextSerializer<Writer> {
    type Error = Writer::Error;

    fn supports_null(&self) -> bool {
        true
    }

    fn pop_null(&mut self) -> Result<(), Self::Error> {
        self.writer.write_str("null")
    }

    fn open_str(&mut self) -> Result<(), Self::Error> {
        self.writer.write_char('\"')
    }

    fn close_str(&mut self) -> Result<(), Self::Error> {
        self.writer.write_char('\"')?;
        if self.in_key {
            self.writer.write_str(": ")?;
            self.in_key = false;
        }
        Ok(())
    }

    fn open_struct(&mut self, _: Option<&'static str>) -> Result<(), Self::Error> {
        self.open_object()
    }

    fn push_field(&mut self, name: &'static str) -> Result<(), Self::Error> {
        self.push_entry(name)
    }

    fn close_struct(&mut self) -> Result<(), Self::Error> {
        self.close_object()
    }

    fn open_tuple(&mut self, type_name: Option<&'static str>) -> Result<(), Self::Error> {
        let _ = type_name;
        self.open_list_streaming()
    }

    fn push_element(&mut self) -> Result<(), Self::Error> {
        self.push_item()
    }

    fn close_tuple(&mut self) -> Result<(), Self::Error> {
        self.close_list()
    }

    fn push_item(&mut self) -> Result<(), Self::Error> {
        if let Some(indent) = self.config.indent {
            if self.at_first {
                self.at_first = false;
            } else {
                self.writer.write_char(',')?;
            }
            self.writer.write_char('\n')?;
            for _ in 0..self.depth {
                self.writer.write_str(indent)?;
            }
        } else if self.at_first {
            self.at_first = false;
        } else {
            self.writer.write_str(", ")?;
        }
        Ok(())
    }

    fn close_list(&mut self) -> Result<(), Self::Error> {
        self.depth -= 1;
        if self.at_first {
            self.at_first = false;
        } else if let Some(indent) = self.config.indent {
            self.writer.write_char('\n')?;
            for _ in 0..self.depth {
                self.writer.write_str(indent)?;
            }
        }
        self.writer.write_char(']')
    }
}

impl<Writer: TextWriter> JsonOutliner for TextSerializer<Writer> {
    fn open_object(&mut self) -> Result<(), Self::Error> {
        self.depth += 1;
        self.at_first = true;
        self.writer.write_char('{')
    }

    fn push_entry(&mut self, key: &str) -> Result<(), Self::Error> {
        self.add_entry()?;
        self.append_str(key)?;
        self.close_str()?;
        Ok(())
    }

    fn close_object(&mut self) -> Result<(), Self::Error> {
        self.depth -= 1;
        if self.at_first {
            self.at_first = false;
            self.writer.write_char('}')
        } else if let Some(indent) = self.config.indent {
            self.writer.write_char('\n')?;
            for _ in 0..self.depth {
                self.writer.write_str(indent)?;
            }
            self.writer.write_char('}')
        } else {
            self.writer.write_str(" }")
        }
    }
}

impl<Writer: TextWriter> Serializer for TextSerializer<Writer> {
    fn put_bool(&mut self, value: bool) -> Result<(), Self::Error> {
        self.writer.write_str(if value { "true" } else { "false" })
    }

    fn put_i8(&mut self, value: i8) -> Result<(), Self::Error> {
        let mut buffer = itoa::Buffer::new();
        self.writer.write_str(buffer.format(value))
    }

    fn put_i16(&mut self, value: i16) -> Result<(), Self::Error> {
        let mut buffer = itoa::Buffer::new();
        self.writer.write_str(buffer.format(value))
    }

    fn put_i32(&mut self, value: i32) -> Result<(), Self::Error> {
        let mut buffer = itoa::Buffer::new();
        self.writer.write_str(buffer.format(value))
    }

    fn put_i64(&mut self, value: i64) -> Result<(), Self::Error> {
        let mut buffer = itoa::Buffer::new();
        self.writer.write_str(buffer.format(value))
    }

    fn put_u8(&mut self, value: u8) -> Result<(), Self::Error> {
        let mut buffer = itoa::Buffer::new();
        self.writer.write_str(buffer.format(value))
    }

    fn put_u16(&mut self, value: u16) -> Result<(), Self::Error> {
        let mut buffer = itoa::Buffer::new();
        self.writer.write_str(buffer.format(value))
    }

    fn put_u32(&mut self, value: u32) -> Result<(), Self::Error> {
        let mut buffer = itoa::Buffer::new();
        self.writer.write_str(buffer.format(value))
    }

    fn put_u64(&mut self, value: u64) -> Result<(), Self::Error> {
        let mut buffer = itoa::Buffer::new();
        self.writer.write_str(buffer.format(value))
    }

    fn put_f32(&mut self, value: f32) -> Result<(), Self::Error> {
        // TODO: Special cases
        let mut buffer = ryu::Buffer::new();
        self.writer.write_str(buffer.format(value))
    }

    fn put_f64(&mut self, value: f64) -> Result<(), Self::Error> {
        // TODO: Special cases
        let mut buffer = ryu::Buffer::new();
        self.writer.write_str(buffer.format(value))
    }

    fn put_char(&mut self, value: char) -> Result<(), Self::Error> {
        self.open_str()?;
        self.append_char(value)?;
        self.close_str()
    }

    fn append_char(&mut self, value: char) -> Result<(), Self::Error> {
        match value {
            '\"' => self.writer.write_str("\\\""),
            '\\' => self.writer.write_str("\\\\"),
            '\x08' => self.writer.write_str("\\b"),
            '\x0C' => self.writer.write_str("\\f"),
            '\n' => self.writer.write_str("\\n"),
            '\r' => self.writer.write_str("\\r"),
            '\t' => self.writer.write_str("\\t"),
            value => self.writer.write_char(value),
        }
    }

    fn put_tag(
        &mut self,
        max_index: usize,
        index: usize,
        name: Option<&'static str>,
    ) -> Result<(), Self::Error> {
        super::put_tag(self, max_index, index, name)
    }

    fn open_list_sized(&mut self, len: usize) -> Result<(), Self::Error> {
        let _ = len;
        self.open_list_streaming()
    }
}

impl<Writer: TextWriter> JsonSerializer for TextSerializer<Writer> {
    fn open_list_streaming(&mut self) -> Result<(), Self::Error> {
        self.depth += 1;
        self.at_first = true;
        self.writer.write_char('[')
    }

    fn add_entry(&mut self) -> Result<(), Self::Error> {
        if self.at_first {
            self.at_first = false;
        } else {
            self.writer.write_char(',')?;
        }
        if let Some(indent) = self.config.indent {
            self.writer.write_char('\n')?;
            for _ in 0..self.depth {
                self.writer.write_str(indent)?;
            }
        } else {
            self.writer.write_char(' ')?;
        }
        self.in_key = true;
        self.writer.write_char('\"')
    }
}
