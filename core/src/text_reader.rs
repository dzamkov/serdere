use std::io;
use std::borrow::Cow;

/// An interface for reading characters from a stream.
pub trait TextReader {
    // TODO: Pass IO errors up

    /// Returns the next character in the stream and advances by one character, or [`None`] if the
    /// end of the stream has been reached.
    fn next(&mut self) -> Option<char>;

    /// Gets the next character in the stream without advancing.
    fn peek(&self) -> Option<char>;

    /// Identifies a position in the input stream.
    type Position: Ord + Clone + std::fmt::Debug + std::fmt::Display;

    /// Gets the current position in the stream.
    fn position(&self) -> Self::Position;

    /// Checks whether the given string is a prefix for the remainder of the input stream,
    /// advancing past it if so. Otherwise, the stream will be left in an undefined position.
    fn read_exact(&mut self, str: &str) -> bool {
        for char in str.chars() {
            if self.next() != Some(char) {
                return false;
            }
        }
        true
    }

    /// Reads characters until a character is read for which `pred` returns a non-[`None`] value.
    /// Returns a string of the characters read up to, but not including, the terminating character.
    /// The stream is advanced past the terminating character. If the end of the input stream is
    /// reached before a terminating character is found, this returns [`None`].
    fn read_until<R>(&mut self, mut pred: impl FnMut(char) -> Option<R>) -> Option<(Cow<str>, R)> {
        let mut str = String::new();
        loop {
            let ch = self.next()?;
            if let Some(end) = pred(ch) {
                return Some((Cow::Owned(str), end));
            } else {
                str.push(ch);
            }
        }
    }
}

impl<'a> TextReader for &'a str {
    fn next(&mut self) -> Option<char> {
        let mut chars = self.chars();
        let res = chars.next();
        *self = chars.as_str();
        res
    }

    fn peek(&self) -> Option<char> {
        self.chars().next()
    }

    type Position = StrPosition<'a>;
    fn position(&self) -> StrPosition<'a> {
        StrPosition(self)
    }

    fn read_until<R>(&mut self, mut pred: impl FnMut(char) -> Option<R>) -> Option<(Cow<str>, R)> {
        let start = *self;
        loop {
            let suffix = *self;
            let ch = self.next()?;
            if let Some(end) = pred(ch) {
                return Some((Cow::Borrowed(prefix(start, suffix)), end));
            }
        }
    }
}

/// Given a source string, and a direct reference to a suffix of that string, returns the
/// complementary prefix of the string.
pub fn prefix<'a>(source: &'a str, suffix: &str) -> &'a str {
    let byte_offset = (suffix.as_ptr() as usize).wrapping_sub(source.as_ptr() as usize);
    source.split_at(byte_offset).0
}

/// A [`TextReader`] position in a `&str` buffer.
#[derive(Clone, Copy)]
pub struct StrPosition<'a>(&'a str);

impl PartialEq for StrPosition<'_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl Eq for StrPosition<'_> {}

impl PartialOrd for StrPosition<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for StrPosition<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.0.as_ptr()).cmp(&other.0.as_ptr())
    }
}

impl std::fmt::Debug for StrPosition<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::fmt::Display for StrPosition<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO: Don't display the whole string. Only a portion is needed for context
        write!(f, "around {:?}", self.0)
    }
}

/// A [`TextReader`] which reads from a [`std::io::Read`] with UTF-8 encoding, tracking position
/// using [`LineColumnPosition`]. This reader has no internal buffering, so it is recommended to
/// use a [`std::io::BufReader`] for data that is not already in memory.
pub struct Utf8Reader<R: std::io::Read> {
    source: R,
    pos: LineColumnPosition,
    peek: Option<char>
}

/// A position in an input source recorded using lines and columns.
#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct LineColumnPosition {
    /// Line number, starting at 0.
    pub line: usize,

    /// Column number, starting at 0.
    pub column: usize
}

impl<R: std::io::Read> Utf8Reader<R> {
    /// Constructs a new [`Utf8Reader`] which reads from the given source.
    pub fn new(mut source: R) -> io::Result<Self> {
        let peek = read_utf8(&mut source)?;
        Ok(Self {
            source,
            pos: LineColumnPosition::default(),
            peek
        })
    }
}

impl<R: std::io::Read> TextReader for Utf8Reader<R> {
    fn next(&mut self) -> Option<char> {
        // Advance position
        match self.peek {
            Some('\t') => self.pos.column += 4,
            Some('\n') => {
                self.pos.line += 1;
                self.pos.column = 0;
            }
            Some('\r') => (),
            Some(_) => self.pos.column += 1,
            None => return None
        }

        // Peek next character
        let old = self.peek;
        self.peek = read_utf8(&mut self.source).unwrap(); // TODO: Bubble up error
        old
    }

    fn peek(&self) -> Option<char> {
        self.peek
    }

    type Position = LineColumnPosition;
    fn position(&self) -> Self::Position {
        self.pos
    }
}

impl std::fmt::Display for LineColumnPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "at line {}, column {}", self.line + 1, self.column + 1)
    }
}

/// Reads a single [`char`] from a stream, assuming UTF-8 encoding. Returns [`None`] if the stream
/// has no data remaining and returns an error if an invalid or partial character is encountered.
pub fn read_utf8(r: &mut (impl std::io::Read + ?Sized)) -> std::io::Result<Option<char>> {
    let mut x = 0;
    if r.read(std::slice::from_mut(&mut x))? == 0 {
        return Ok(None);
    }
    if x < 0b10000000 {
        return Ok(Some(x.into()));
    } 
    let ch = if x < 0b11100000 {
        let mut buf = [0u8; 1];
        r.read_exact(&mut buf)?;
        let ch = (x as u32) & 0b00011111;
        let ch = (ch << 6) | (buf[0] & 0b00111111) as u32;
        char::from_u32(ch)
    } else if x < 0b11110000 {
        let mut buf = [0u8; 2];
        r.read_exact(&mut buf)?;
        let ch = (x as u32) & 0b00001111;
        let ch = (ch << 6) | (buf[0] & 0b00111111) as u32;
        let ch = (ch << 6) | (buf[1] & 0b00111111) as u32;
        char::from_u32(ch)
    } else if x < 0b11111000 {
        let mut buf = [0u8; 3];
        r.read_exact(&mut buf)?;
        let ch = (x as u32) & 0b00000111;
        let ch = (ch << 6) | (buf[0] & 0b00111111) as u32;
        let ch = (ch << 6) | (buf[1] & 0b00111111) as u32;
        let ch = (ch << 6) | (buf[2] & 0b00111111) as u32;
        char::from_u32(ch)
    } else {
        None
    };
    if let Some(ch) = ch {
        Ok(Some(ch))
    } else {
        // Error
        todo!();
    }
}

#[test]
fn test_read_utf8() {
    let str = "ab\u{0014}\u{0162}\u{0651}\u{1485}\u{0482}\u{95832}";
    let mut bytes = str.as_bytes();
    for ch in str.chars() {
        assert_eq!(read_utf8(&mut bytes).unwrap(), Some(ch));
    }
    assert_eq!(read_utf8(&mut bytes).unwrap(), None);
}