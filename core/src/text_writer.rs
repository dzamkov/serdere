/// An interface for writing characters to a stream.
pub trait TextWriter {
    /// The type of error that can occur while writing to the stream.
    type Error: std::error::Error;

    /// Writes a character to the stream.
    fn write_char(&mut self, ch: char) -> Result<(), Self::Error>;

    /// Writes a string to the stream.
    fn write_str(&mut self, str: &str) -> Result<(), Self::Error> {
        for ch in str.chars() {
            self.write_char(ch)?
        }
        Ok(())
    }
}

impl<T: TextWriter + ?Sized> TextWriter for &'_ mut T {
    type Error = T::Error;
    fn write_char(&mut self, ch: char) -> Result<(), Self::Error> {
        (**self).write_char(ch)
    }

    fn write_str(&mut self, str: &str) -> Result<(), Self::Error> {
        (**self).write_str(str)
    }
}

impl TextWriter for String {
    type Error = std::convert::Infallible;
    fn write_char(&mut self, ch: char) -> Result<(), Self::Error> {
        self.push(ch);
        Ok(())
    }

    fn write_str(&mut self, str: &str) -> Result<(), Self::Error> {
        self.push_str(str);
        Ok(())
    }
}

/// A [`TextWriter`] which writes to a [`std::io::Write`] with UTF-8 encoding. This writer has no
/// internal buffering, so it is recommended to use a [`std::io::BufWriter`] for data that is not
/// already in memory.
pub struct Utf8Writer<W: std::io::Write>(W);

impl<W: std::io::Write> Utf8Writer<W> {
    /// Constructs a new [`Utf8Writer`] which writes to the given destination.
    pub fn new(dest: W) -> Self {
        Self(dest)
    }

    /// Gets the underlying writer.
    pub fn into_inner(self) -> W {
        self.0
    }
}

impl<W: std::io::Write> TextWriter for Utf8Writer<W> {
    type Error = std::io::Error;
    fn write_char(&mut self, ch: char) -> Result<(), Self::Error> {
        let mut buf = [0; 4];
        self.0.write_all(ch.encode_utf8(&mut buf).as_bytes())
    }

    fn write_str(&mut self, str: &str) -> Result<(), Self::Error> {
        self.0.write_all(str.as_bytes())
    }
}
