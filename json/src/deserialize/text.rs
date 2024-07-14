use super::number::{Num, NumBuilder};
use crate::{CollectionType, ValueType};
use crate::{JsonDeserializer, JsonOutliner};
use serdere::{prefix, Deserializer, NameMap, Outliner, TextReader};
use std::hash::{BuildHasher, Hasher};
use std::num::NonZeroU32;
use DeserializeErrorMessage::*;

/// A [`JsonDeserializer`] which reads from a [`TextReader`].
pub struct TextDeserializer<Reader: TextReader> {
    config: TextDeserializerConfig,
    reader: Reader,
    outline: Outline<Reader::Position>,
    state: DeserializerState,
    error_pos: Reader::Position,
}

/// Encapsulates the configuration options for a [`TextDeserializer`].
#[derive(Debug, Clone, Copy)]
pub struct TextDeserializerConfig {
    /// Indicates whether the parser accepts JS-style comments where whitespace is expected.
    pub allow_comments: bool,
    // TODO: Allow trailing comma
}

impl TextDeserializerConfig {
    /// Gets a parsing configuration which strictly follows the JSON specification.
    pub const fn strict() -> Self {
        Self {
            allow_comments: false,
        }
    }

    /// Gets the most permissive parsing configuration.
    pub const fn permissive() -> Self {
        Self {
            allow_comments: true,
        }
    }
}

impl Default for TextDeserializerConfig {
    fn default() -> Self {
        Self::strict()
    }
}

/// Encapsulates the information about a JSON document that has already been read, but maybe
/// not processed.
struct Outline<Position> {
    stack_items: Vec<StackItem<Position>>,
    lookback_items: Vec<LookbackItem<Position>>,
    lookback_keys: hashbrown::raw::RawTable<LookbackKey>,
    lookback_data: Vec<u8>,
}

/// Describes an opened object or array on the deserializer stack.
struct StackItem<Position> {
    /// The position of the start (`{` or `[`) of the container.
    pos: Position,

    /// The index for the first item in `lookback_items` that belongs to this container.
    first_child_index: usize,

    /// The type of the collection for this stack item.
    collection_type: CollectionType,
}

/// Describes an object entry or list item that has been read, but not yet returned by a
/// [`TextDeserializer`].
struct LookbackItem<Position> {
    /// The position of the first character that is part of the value for this item.
    pos: Position,

    /// The index in `lookback_data` that begins the data for this item. This data includes
    /// the key for the entry (if applicable) and extra data for its value.
    data_index: usize,

    /// Identifies the index in `lookback_items` where the next item with same container as this
    /// item would be, or `usize::MAX` if this is *known* to be the last item in the container.
    /// The last item in a collection that has not yet been fully read would have
    /// `next_sibling_index` set past the end of `lookback_items`, where the next item would be
    /// added.
    next_sibling_index: usize,

    /// Combines the length (in bytes) of the key string for this entry, along with a flag that
    /// indicates whether it is in `lookback_keys` (i.e. is in an open object). The flag is the
    /// first bit of the value.
    key_len_active: u32,

    /// Describes the value for this item. If [`None`] than this item is just a placeholder for a
    /// value that has already been read.
    value: Option<LookbackValue>,
}

/// Describes a value that has been read, but not yet returned by a [`TextDeserializer`].
/// Supplementary data may be available in `lookback_data`.
#[derive(Debug)]
enum LookbackValue {
    /// A JSON string. The string data comes from the slice of `lookback_data` between the end
    /// of the entry key and the start of the data for the next item.
    String,

    /// A JSON number. The digit data comes from the slice of `lookback_data` between the end
    /// of the entry key and the start of the data for the next item. Each digit is encoded in
    /// 4 bits, with 2 digits per byte.
    Number {
        /// Is the number negated?
        negate: bool,

        /// The base-10 exponent for this number.
        exp: i16,
    },

    /// A JSON object. The entry data comes from the following items in `lookback_items`.
    Object {
        /// Indicates whether the object has any entries.
        has_entries: bool,
    },

    /// A JSON array. The item data comes from the following items in `lookback_items`.
    Array {
        /// Indicates whether the array has any items.
        has_items: bool,
    },

    /// A JSON boolean.
    Bool(bool),

    // The JSON constant `null`.
    Null,
}

/// Describes the overall state of a [`TextDeserializer`].
enum DeserializerState {
    /// There is a value at the top of the deserialization stack and `reader` is positioned at
    /// the first character for the value.
    StreamingValue,

    /// There is a value at the top of the deserialization stack and it's value is stored in
    /// `lookback_items` at `index`.
    LookbackValue {
        index: usize,
        streaming_depth: Option<NonZeroU32>,
    },

    /// There is a value at the top of the deserialization stack and it is a virtual `null`
    /// literal.
    NullValue {
        key: Option<&'static str>,
        at_start: bool,
        streaming_depth: Option<NonZeroU32>,
    },

    /// There is an opened collection (object or array) at the top of the deserialization stack,
    /// or the deserialization stack is empty. If the depth of the collection is greater than
    /// `streaming_depth`, it has already been fully read from `reader`. If its depth is equal to
    /// `streaming_depth`, `reader` is positioned immediately after the previously-read item, or
    /// after the brace (`{` or `[`) that begins the collection.
    ///
    /// or the deserialization stack is empty and `reader` is positioned immediately after the
    /// previously-read item, or after the brace (`{` or `[`) that begins the collection.
    Collection {
        at_start: bool,

        /// The depth of the first opened collection in the deserialization stack that has not
        /// yet been fully read.
        streaming_depth: Option<NonZeroU32>,
    },

    /// There is an opened string at the top of the deserialization stack and `reader` is
    /// positioned before the next character for the string.
    StreamingString {
        /// Indicates whether this string is the key for an object entry. If `true`, after reading
        /// the string, the value for the entry should be pushed onto the stack.
        is_key: bool,
    },

    /// There is an opened string at the top of the deserialization stack and its data can be
    /// found in the specified range of `lookback_data`.
    LookbackString {
        head_index: usize,
        end_index: usize,

        /// If not [`None`], indicates that this string is the key for an object entry. After
        /// reading the string, the specified value in `lookback_items` should be pushed onto the
        /// stack.
        value_index: Option<usize>,

        /// The depth of the first opened collection in the deserialization stack that has not
        /// yet been fully read.
        streaming_depth: Option<NonZeroU32>,
    },
}

/// Identifies a entry key that belongs to a entry in `lookback_items` for an object that is
/// currently opened on the stack.
struct LookbackKey {
    /// The depth of the stack item the entry for this key is for.
    depth: NonZeroU32,

    /// The index of the [`LookbackItem`] corresponding to the entry for this key.
    index: usize,
}

/// An error message that indicates that a value in [`LookbackItem`]s has already been read.
const VALUE_ALREADY_READ: &str = "value already read";

/// An error message that indicates that the top of the deserialization stack was expected to
/// be a value, but it isn't.
const NOT_VALUE: &str = "top of the deserialization stack is not value";

/// An error message that indicates that the top of the deserialization stack was expected to
/// be an opened collection, but it isn't.
const NOT_COLLECTION: &str = "top of the deserialization stack is not an opened collection";

/// An error message that indicates that the top of the deserialization stack was expected to
/// be an opened object, but it isn't.
const NOT_OBJECT: &str = "top of the deserialization stack is not an opened object";

/// An error message that indicates that the top of the deserialization stack was expected to
/// be an opened array, but it isn't.
const NOT_ARRAY: &str = "top of the deserialization stack is not an opened array";

/// An error message that indicates that the top of the deserialization stack was expected to
/// be an opened string, but it isn't.
const NOT_STRING: &str = "top of the deserialization stack is not an opened string";

impl<Reader: TextReader> TextDeserializer<Reader> {
    /// Constructs a new [`TextDeserializer`] for reading a JSON value from a [`TextReader`].
    /// The stack initially consists of a single value item.
    pub fn new(
        config: TextDeserializerConfig,
        mut reader: Reader,
    ) -> Result<Self, DeserializeError<Reader::Position>> {
        reader.skip_whitespace(config.allow_comments)?;
        let error_pos = reader.position();
        Ok(Self {
            config,
            reader,
            outline: Outline::default(),
            state: DeserializerState::StreamingValue,
            error_pos,
        })
    }

    /// Assuming that the top item on the stack is a value, pops it from the stack and returns it,
    /// interpreting it as a number.
    pub fn read_number<T: Num>(&mut self) -> Result<T, DeserializeError<Reader::Position>> {
        match self.state {
            DeserializerState::StreamingValue => {
                self.state = DeserializerState::Collection {
                    at_start: false,
                    streaming_depth: self.outline.top_depth(),
                };
                self.error_pos = self.reader.position();
                self.reader.read_number()
            }
            DeserializerState::LookbackValue {
                index,
                streaming_depth,
            } => {
                let (pos, value, data) = self.outline.take_value(index);
                if let LookbackValue::Number { negate, exp } = value {
                    self.state = DeserializerState::Collection {
                        at_start: false,
                        streaming_depth,
                    };
                    self.error_pos = pos.clone();
                    let mut builder: T::Builder = Default::default();
                    for buf in data.iter().copied() {
                        let digit_0 = buf & 0xF;
                        if digit_0 == 0xF {
                            break;
                        }
                        if !builder.push_digit(digit_0) {
                            return Err(DeserializeError::new(
                                pos.clone(),
                                DeserializeErrorMessage::NumberOverflow,
                            ));
                        }
                        let digit_1 = (buf >> 4) & 0xF;
                        if digit_1 == 0xF {
                            break;
                        }
                        if !builder.push_digit(digit_1) {
                            return Err(DeserializeError::new(
                                pos.clone(),
                                DeserializeErrorMessage::NumberOverflow,
                            ));
                        }
                    }
                    let exp = i32::from(exp);
                    T::from_builder(builder, negate, exp)
                        .ok_or_else(|| DeserializeError::new(pos.clone(), NumberOverflow))
                } else {
                    Err(DeserializeError::new(
                        pos.clone(),
                        DeserializeErrorMessage::ExpectedNumber,
                    ))
                }
            }
            DeserializerState::NullValue { key, .. } => {
                Err(self.error_unexpected_virtual_null(key))
            }
            _ => panic!("{}", NOT_VALUE),
        }
    }

    /// Verifies that there is no more data to read and closes the [`TextDeserializer`].
    pub fn close(mut self) -> Result<(), DeserializeError<Reader::Position>> {
        assert!(matches!(
            self.state,
            DeserializerState::Collection {
                at_start: false,
                streaming_depth: None
            }
        ));
        assert!(self.outline.stack_items.is_empty());
        self.reader.skip_whitespace(self.config.allow_comments)?;
        self.reader.read_eof()?;
        Ok(())
    }

    /// Constructs an error in response to an attempt to read a virtual `null` as anything other
    /// than a `null` literal.
    fn error_unexpected_virtual_null(
        &self,
        key: Option<&'static str>,
    ) -> DeserializeError<Reader::Position> {
        if let Some(key) = key {
            DeserializeError::new(
                self.outline.stack_items.last().unwrap().pos.clone(),
                DeserializeErrorMessage::MissingKey(key.to_owned()),
            )
        } else {
            todo!()
        }
    }
}

impl<Reader: TextReader> Outliner for TextDeserializer<Reader> {
    type Error = DeserializeError<Reader::Position>;

    fn supports_null(&self) -> bool {
        true
    }

    fn pop_null(&mut self) -> Result<(), Self::Error> {
        match self.state {
            DeserializerState::StreamingValue => {
                self.state = DeserializerState::Collection {
                    at_start: false,
                    streaming_depth: self.outline.top_depth(),
                };
                self.error_pos = self.reader.position();
                match self.reader.next() {
                    Some('n') => {
                        if !self.reader.read_exact("ull") {
                            return Err(DeserializeError::new(
                                self.error_pos.clone(),
                                InvalidLiteral,
                            ));
                        }
                        Ok(())
                    }
                    Some(_) => Err(DeserializeError::new(self.error_pos.clone(), ExpectedNull)),
                    None => Err(DeserializeError::new(self.error_pos.clone(), UnexpectedEof)),
                }
            }
            DeserializerState::LookbackValue {
                index,
                streaming_depth,
            } => {
                let (pos, value, data) = self.outline.take_value(index);
                if let LookbackValue::Null = value {
                    debug_assert!(data.is_empty());
                    self.state = DeserializerState::Collection {
                        at_start: false,
                        streaming_depth,
                    };
                    self.error_pos = pos.clone();
                    Ok(())
                } else {
                    Err(DeserializeError::new(
                        pos.clone(),
                        DeserializeErrorMessage::ExpectedNull,
                    ))
                }
            }
            DeserializerState::NullValue {
                at_start,
                streaming_depth,
                ..
            } => {
                // TODO: Update `error_pos`
                self.state = DeserializerState::Collection {
                    at_start,
                    streaming_depth,
                };
                Ok(())
            }
            _ => panic!("{}", NOT_VALUE),
        }
    }

    fn open_str(&mut self) -> Result<(), Self::Error> {
        match self.state {
            DeserializerState::StreamingValue => {
                let pos = self.reader.position();
                match self.reader.next() {
                    Some('"') => {
                        self.state = DeserializerState::StreamingString { is_key: false };
                        Ok(())
                    }
                    Some(_) => Err(DeserializeError::new(
                        pos,
                        DeserializeErrorMessage::ExpectedString,
                    )),
                    None => Err(DeserializeError::new(pos, UnexpectedEof)),
                }
            }
            DeserializerState::LookbackValue {
                index,
                streaming_depth,
            } => {
                let (pos, value, data) = self.outline.take_value(index);
                if let LookbackValue::String = value {
                    let data = data.as_ptr_range();
                    let base_ptr = self.outline.lookback_data.as_ptr();
                    self.state = DeserializerState::LookbackString {
                        head_index: data.start as usize - base_ptr as usize,
                        end_index: data.end as usize - base_ptr as usize,
                        value_index: None,
                        streaming_depth,
                    };
                    Ok(())
                } else {
                    Err(DeserializeError::new(
                        pos.clone(),
                        DeserializeErrorMessage::ExpectedString,
                    ))
                }
            }
            DeserializerState::NullValue { key, .. } => {
                Err(self.error_unexpected_virtual_null(key))
            }
            _ => panic!("{}", NOT_VALUE),
        }
    }

    fn close_str(&mut self) -> Result<(), Self::Error> {
        todo!()
    }

    fn open_struct(&mut self, type_name: Option<&'static str>) -> Result<(), Self::Error> {
        super::open_struct(self, type_name)
    }

    fn push_field(&mut self, name: &'static str) -> Result<(), Self::Error> {
        super::push_field(self, name)
    }

    fn close_struct(&mut self) -> Result<(), Self::Error> {
        super::close_struct(self)
    }

    fn open_tuple(&mut self, type_name: Option<&'static str>) -> Result<(), Self::Error> {
        let _ = type_name;
        self.open_list()?;
        Ok(())
    }

    fn push_element(&mut self) -> Result<(), Self::Error> {
        self.push_item()
    }

    fn close_tuple(&mut self) -> Result<(), Self::Error> {
        self.close_list()
    }

    fn push_item(&mut self) -> Result<(), Self::Error> {
        super::push_item(self)
    }

    fn close_list(&mut self) -> Result<(), Self::Error> {
        super::close_list(self)
    }
}

impl<Reader: TextReader> JsonOutliner for TextDeserializer<Reader> {
    fn open_object(&mut self) -> Result<(), Self::Error> {
        match self.state {
            DeserializerState::StreamingValue => {
                let pos = self.reader.position();
                match self.reader.next() {
                    Some('{') => {
                        self.outline.stack_items.push(StackItem {
                            pos,
                            first_child_index: self.outline.lookback_items.len(),
                            collection_type: CollectionType::Object,
                        });
                        self.state = DeserializerState::Collection {
                            at_start: true,
                            streaming_depth: self.outline.top_depth(),
                        };
                        Ok(())
                    }
                    Some(_) => Err(DeserializeError::new(
                        pos,
                        DeserializeErrorMessage::ExpectedObject,
                    )),
                    None => Err(DeserializeError::new(pos, UnexpectedEof)),
                }
            }
            DeserializerState::LookbackValue {
                index,
                streaming_depth,
            } => {
                let (pos, value, data) = self.outline.take_value(index);
                if let LookbackValue::Object { has_entries } = value {
                    debug_assert!(data.is_empty());
                    let pos = pos.clone();
                    self.state = DeserializerState::Collection {
                        at_start: true,
                        streaming_depth,
                    };
                    if has_entries {
                        // Push object onto stack
                        self.outline.stack_items.push(StackItem {
                            pos,
                            first_child_index: index + 1,
                            collection_type: CollectionType::Object,
                        });

                        // Add object keys to `lookback_keys`
                        let depth = self.outline.top_depth().unwrap();
                        let mut child_index = index + 1;
                        while child_index < self.outline.lookback_items.len() {
                            let item = &self.outline.lookback_items[child_index];
                            let hash = key_hash(depth, item.key_bytes(&self.outline.lookback_data));
                            self.outline.lookback_keys.insert_entry(
                                hash,
                                LookbackKey {
                                    depth,
                                    index: child_index,
                                },
                                |key| {
                                    key.hash(
                                        &self.outline.lookback_items,
                                        &self.outline.lookback_data,
                                    )
                                },
                            );
                            child_index = item.next_sibling_index;
                        }
                    } else {
                        // Push empty object onto stack
                        self.outline.stack_items.push(StackItem {
                            pos,
                            first_child_index: usize::MAX,
                            collection_type: CollectionType::Object,
                        });
                    }
                    Ok(())
                } else {
                    Err(DeserializeError::new(
                        pos.clone(),
                        DeserializeErrorMessage::ExpectedObject,
                    ))
                }
            }
            DeserializerState::NullValue { key, .. } => {
                Err(self.error_unexpected_virtual_null(key))
            }
            _ => panic!("{}", NOT_VALUE),
        }
    }

    fn push_entry(&mut self, key: &str) -> Result<(), Self::Error> {
        if self.try_push_entry(key)? {
            Ok(())
        } else {
            self.skip_object()?;
            Err(self.error_missing_entry(key.to_string()))
        }
    }

    fn close_object(&mut self) -> Result<(), Self::Error> {
        if self.next_entry()? {
            let key = self.flush_str()?.into_owned();
            self.skip_value()?;
            return Err(self.error_extra_entry(key));
        }
        Ok(())
    }
}

impl<Reader: TextReader> Deserializer for TextDeserializer<Reader> {
    fn get_bool(&mut self) -> Result<bool, Self::Error> {
        match self.state {
            DeserializerState::StreamingValue => {
                self.state = DeserializerState::Collection {
                    at_start: false,
                    streaming_depth: self.outline.top_depth(),
                };
                self.error_pos = self.reader.position();
                self.reader.read_bool()
            }
            DeserializerState::LookbackValue {
                index,
                streaming_depth,
            } => {
                let (pos, value, data) = self.outline.take_value(index);
                if let LookbackValue::Bool(value) = value {
                    debug_assert!(data.is_empty());
                    self.state = DeserializerState::Collection {
                        at_start: false,
                        streaming_depth,
                    };
                    self.error_pos = pos.clone();
                    Ok(value)
                } else {
                    Err(DeserializeError::new(
                        pos.clone(),
                        DeserializeErrorMessage::ExpectedBool,
                    ))
                }
            }
            DeserializerState::NullValue { key, .. } => {
                Err(self.error_unexpected_virtual_null(key))
            }
            _ => panic!("{}", NOT_VALUE),
        }
    }

    fn get_i8(&mut self) -> Result<i8, Self::Error> {
        self.read_number()
    }

    fn get_i16(&mut self) -> Result<i16, Self::Error> {
        self.read_number()
    }

    fn get_i32(&mut self) -> Result<i32, Self::Error> {
        self.read_number()
    }

    fn get_i64(&mut self) -> Result<i64, Self::Error> {
        self.read_number()
    }

    fn get_u8(&mut self) -> Result<u8, Self::Error> {
        self.read_number()
    }

    fn get_u16(&mut self) -> Result<u16, Self::Error> {
        self.read_number()
    }

    fn get_u32(&mut self) -> Result<u32, Self::Error> {
        self.read_number()
    }

    fn get_u64(&mut self) -> Result<u64, Self::Error> {
        self.read_number()
    }

    fn get_f32(&mut self) -> Result<f32, Self::Error> {
        self.read_number()
    }

    fn get_f64(&mut self) -> Result<f64, Self::Error> {
        self.read_number()
    }

    fn get_char(&mut self) -> Result<char, Self::Error> {
        todo!()
    }

    fn next_char(&mut self) -> Result<Option<char>, Self::Error> {
        match &mut self.state {
            DeserializerState::StreamingString { is_key } => match self.reader.next() {
                Some('"') => {
                    // TODO: Update `error_pos`
                    if *is_key {
                        self.reader.skip_past_colon(self.config.allow_comments)?;
                        self.reader.skip_whitespace(self.config.allow_comments)?;
                        self.state = DeserializerState::StreamingValue;
                    } else {
                        self.state = DeserializerState::Collection {
                            at_start: false,
                            streaming_depth: self.outline.top_depth(),
                        };
                    }
                    Ok(None)
                }
                Some('\\') => Ok(Some(self.reader.read_escape_sequence()?)),
                Some(ch) => Ok(Some(ch)),
                None => Err(DeserializeError::new(self.reader.position(), UnexpectedEof)),
            },
            DeserializerState::LookbackString {
                head_index,
                end_index,
                value_index,
                streaming_depth,
            } => {
                let end_index = *end_index;
                Ok(if *head_index < end_index {
                    let str_data = &self.outline.lookback_data[*head_index..end_index];
                    // SAFETY: We wrote this data ourselves using UTF-8 encoding
                    let str = unsafe { std::str::from_utf8_unchecked(str_data) };
                    let mut chs = str.chars();
                    let ch = chs.next().unwrap();
                    let str = chs.as_str();
                    *head_index =
                        str.as_ptr() as usize - self.outline.lookback_data.as_ptr() as usize;
                    Some(ch)
                } else {
                    if let Some(value_index) = value_index {
                        // Next item is the value for the entry where this string is the key
                        self.state = DeserializerState::LookbackValue {
                            index: *value_index,
                            streaming_depth: *streaming_depth,
                        };
                    } else {
                        self.state = DeserializerState::Collection {
                            at_start: false,
                            streaming_depth: *streaming_depth,
                        };
                    }
                    None
                })
            }
            _ => panic!("{}", NOT_STRING),
        }
    }

    fn get_tag(
        &mut self,
        max_index: usize,
        names: &'static NameMap<usize>,
    ) -> Result<usize, Self::Error> {
        super::get_tag(self, max_index, names)
    }

    fn check_null(&mut self) -> Result<bool, Self::Error> {
        Ok(if let ValueType::Null = self.peek_value_type() {
            self.pop_null()?;
            true
        } else {
            false
        })
    }

    fn open_list(&mut self) -> Result<Option<usize>, Self::Error> {
        match self.state {
            DeserializerState::StreamingValue => {
                let pos = self.reader.position();
                match self.reader.next() {
                    Some('[') => {
                        self.outline.stack_items.push(StackItem {
                            pos,
                            first_child_index: self.outline.lookback_items.len(),
                            collection_type: CollectionType::Array,
                        });
                        self.state = DeserializerState::Collection {
                            at_start: true,
                            streaming_depth: self.outline.top_depth(),
                        };
                        Ok(None)
                    }
                    Some(_) => Err(DeserializeError::new(
                        pos,
                        DeserializeErrorMessage::ExpectedArray,
                    )),
                    None => Err(DeserializeError::new(pos, UnexpectedEof)),
                }
            }
            DeserializerState::LookbackValue {
                index,
                streaming_depth,
            } => {
                let (pos, value, data) = self.outline.take_value(index);
                if let LookbackValue::Array { has_items } = value {
                    debug_assert!(data.is_empty());
                    let pos = pos.clone();
                    self.outline.stack_items.push(StackItem {
                        pos,
                        first_child_index: if has_items { index + 1 } else { usize::MAX },
                        collection_type: CollectionType::Array,
                    });
                    self.state = DeserializerState::Collection {
                        at_start: true,
                        streaming_depth,
                    };
                    Ok(None)
                } else {
                    Err(DeserializeError::new(
                        pos.clone(),
                        DeserializeErrorMessage::ExpectedArray,
                    ))
                }
            }
            DeserializerState::NullValue { key, .. } => {
                Err(self.error_unexpected_virtual_null(key))
            }
            _ => panic!("{}", NOT_VALUE),
        }
    }

    fn next_item(&mut self) -> Result<bool, Self::Error> {
        let DeserializerState::Collection {
            at_start,
            streaming_depth,
        } = &mut self.state
        else {
            panic!("{}", NOT_COLLECTION)
        };
        let depth = self.outline.top_depth().unwrap();
        let array_info = self.outline.stack_items.last_mut().unwrap();
        array_info.assert_array();
        if *streaming_depth == Some(depth) {
            let has_item = if *at_start {
                self.reader.skip_to_first_item(self.config.allow_comments)?
            } else if self.reader.skip_to_next_item(self.config.allow_comments)? {
                self.reader.skip_whitespace(self.config.allow_comments)?;
                true
            } else {
                false
            };
            if has_item {
                self.state = DeserializerState::StreamingValue;
                Ok(true)
            } else {
                self.error_pos = self.outline.stack_items.pop().unwrap().pos;
                *at_start = false;
                *streaming_depth = NonZeroU32::new(u32::from(depth) - 1);
                Ok(false)
            }
        } else if let Some(index) = first_unread_child(
            &self.outline.lookback_items,
            &mut array_info.first_child_index,
        ) {
            self.state = DeserializerState::LookbackValue {
                index,
                streaming_depth: *streaming_depth,
            };
            Ok(true)
        } else {
            self.error_pos = self.outline.stack_items.pop().unwrap().pos;
            *at_start = false;
            Ok(false)
        }
    }

    fn error(&self, source: Box<dyn std::error::Error + Send + Sync>) -> Self::Error {
        DeserializeError::new(
            self.error_pos.clone(),
            DeserializeErrorMessage::Custom(source),
        )
    }

    fn error_missing_item(&self) -> Self::Error {
        DeserializeError::new(
            self.error_pos.clone(),
            DeserializeErrorMessage::MissingItems,
        )
    }

    fn error_extra_item(&self) -> Self::Error {
        let array_info = self.outline.stack_items.last().expect(NOT_COLLECTION);
        array_info.assert_array();
        DeserializeError::new(array_info.pos.clone(), DeserializeErrorMessage::ExcessItems)
    }
}

impl<Reader: TextReader> JsonDeserializer for TextDeserializer<Reader> {
    fn peek_value_type(&self) -> ValueType {
        match self.state {
            DeserializerState::StreamingValue => match self.reader.peek() {
                Some('"') => ValueType::String,
                Some('-' | '0'..='9') => ValueType::Number,
                Some('{') => ValueType::Object,
                Some('[') => ValueType::Array,
                Some('t' | 'f') => ValueType::Bool,
                Some('n') => ValueType::Null,
                _ => ValueType::Null,
            },
            DeserializerState::LookbackValue { index, .. } => {
                match self.outline.lookback_items[index]
                    .value
                    .as_ref()
                    .expect(VALUE_ALREADY_READ)
                {
                    LookbackValue::String => ValueType::String,
                    LookbackValue::Number { .. } => ValueType::Number,
                    LookbackValue::Object { .. } => ValueType::Object,
                    LookbackValue::Array { .. } => ValueType::Array,
                    LookbackValue::Bool(_) => ValueType::Bool,
                    LookbackValue::Null => ValueType::Null,
                }
            }
            DeserializerState::NullValue { .. } => ValueType::Null,
            _ => panic!("{}", NOT_VALUE),
        }
    }

    fn peek_collection_type(&self) -> CollectionType {
        self.outline
            .stack_items
            .last()
            .expect(NOT_COLLECTION)
            .collection_type
    }

    fn push_null(&mut self, key: Option<&'static str>) {
        let DeserializerState::Collection {
            at_start,
            streaming_depth,
        } = &self.state
        else {
            panic!("{}", NOT_COLLECTION)
        };
        self.state = DeserializerState::NullValue {
            key,
            at_start: *at_start,
            streaming_depth: *streaming_depth,
        };
    }

    fn try_push_entry(&mut self, key: &str) -> Result<bool, Self::Error> {
        let DeserializerState::Collection {
            at_start,
            streaming_depth,
        } = &mut self.state
        else {
            panic!("{}", NOT_COLLECTION)
        };
        let depth = self.outline.top_depth().unwrap();
        let obj_info = self.outline.stack_items.last_mut().unwrap();
        obj_info.assert_object();

        // Are there any active lookback keys?
        if first_unread_child(
            &self.outline.lookback_items,
            &mut obj_info.first_child_index,
        )
        .is_some()
        {
            // Look for name in lookback keys
            let hash = key_hash(depth, key.as_bytes());
            let entry = self
                .outline
                .lookback_keys
                .remove_entry(hash, |lookback_key| {
                    if lookback_key.depth == depth {
                        let item = &mut self.outline.lookback_items[lookback_key.index];
                        let key_bytes = item.key_bytes(&self.outline.lookback_data);
                        if key.as_bytes() == key_bytes {
                            // Clear active flag
                            item.key_len_active &= !1;
                            return true;
                        }
                    }
                    false
                });

            // Did we find an entry?
            if let Some(entry) = entry {
                self.state = DeserializerState::LookbackValue {
                    index: entry.index,
                    streaming_depth: *streaming_depth,
                };
                return Ok(true);
            }
        }

        // Are we still streaming data for this object?
        if *streaming_depth == Some(depth) {
            'read_entry: {
                // Skip to the start of the next entry key
                let key_pos = if *at_start {
                    self.reader
                        .skip_to_first_entry(self.config.allow_comments)?
                } else {
                    self.reader.skip_to_next_entry(self.config.allow_comments)?
                };
                let Some(mut key_pos) = key_pos else {
                    break 'read_entry;
                };

                // Look through entrys until we find a match.
                loop {
                    let data_index = self.outline.lookback_data.len();
                    let found = self
                        .reader
                        .read_str_bytes_into_or_match(key, &mut self.outline.lookback_data)?;
                    self.reader.skip_past_colon(self.config.allow_comments)?;
                    if found {
                        self.reader.skip_whitespace(self.config.allow_comments)?;
                        self.state = DeserializerState::StreamingValue;
                        return Ok(true);
                    } else {
                        // Read the entry value as a lookback item
                        let key_end_index = self.outline.lookback_data.len();
                        let key_len = key_end_index - data_index;
                        let key_len_active = if key_len <= usize::try_from(u32::MAX >> 1).unwrap() {
                            ((key_len as u32) << 1) | 1
                        } else {
                            return Err(DeserializeError::new(key_pos, KeyTooLong));
                        };
                        let hash = key_hash(
                            depth,
                            &self.outline.lookback_data[data_index..key_end_index],
                        );
                        let item = self.reader.read_lookback_value(
                            &self.config,
                            data_index,
                            key_len_active,
                            &mut self.outline,
                        )?;
                        self.outline.lookback_keys.insert_entry(
                            hash,
                            LookbackKey { depth, index: item },
                            |key| {
                                key.hash(&self.outline.lookback_items, &self.outline.lookback_data)
                            },
                        );

                        // Go to the start of the next entry
                        if let Some(pos) =
                            self.reader.skip_to_next_entry(self.config.allow_comments)?
                        {
                            key_pos = pos;
                        } else {
                            break 'read_entry;
                        }
                    }
                }
            }
            *streaming_depth = NonZeroU32::new(u32::from(depth) - 1);
        }

        // Key not found
        Ok(false)
    }

    fn next_entry(&mut self) -> Result<bool, Self::Error> {
        let DeserializerState::Collection {
            at_start,
            streaming_depth,
        } = &mut self.state
        else {
            panic!("{}", NOT_COLLECTION)
        };
        let depth = self.outline.top_depth().unwrap();
        let obj_info = self.outline.stack_items.last_mut().unwrap();
        obj_info.assert_object();

        // Check lookback items
        if let Some(item_index) = first_unread_child(
            &self.outline.lookback_items,
            &mut obj_info.first_child_index,
        ) {
            // Remove from `lookback_keys`
            let item = &mut self.outline.lookback_items[item_index];
            let key_data = item.key_bytes(&self.outline.lookback_data);
            let hash = key_hash(depth, key_data);
            let entry = self
                .outline
                .lookback_keys
                .remove_entry(hash, |lookback_key| {
                    lookback_key.depth == depth && lookback_key.index == item_index
                });
            assert!(entry.is_some());

            // Clear active flag
            item.key_len_active &= !1;

            // Return key and value
            let key_data = key_data.as_ptr_range();
            let base_ptr = self.outline.lookback_data.as_ptr();
            self.state = DeserializerState::LookbackString {
                head_index: key_data.start as usize - base_ptr as usize,
                end_index: key_data.end as usize - base_ptr as usize,
                value_index: Some(item_index),
                streaming_depth: *streaming_depth,
            };
            return Ok(true);
        }

        // Are we still streaming data for this object?
        if *streaming_depth == Some(depth) {
            // Skip to the start of the next entry key
            let key_pos = if *at_start {
                self.reader
                    .skip_to_first_entry(self.config.allow_comments)?
            } else {
                self.reader.skip_to_next_entry(self.config.allow_comments)?
            };
            if key_pos.is_some() {
                self.state = DeserializerState::StreamingString { is_key: true };
                return Ok(true);
            } else {
                *streaming_depth = NonZeroU32::new(u32::from(depth) - 1);
            }
        }

        // We've reached the end of the object. Pop it from the stack
        self.error_pos = self.outline.stack_items.pop().unwrap().pos;
        *at_start = false;
        Ok(false)
    }

    fn error_missing_entry(&self, key: String) -> Self::Error {
        DeserializeError::new(
            self.error_pos.clone(),
            DeserializeErrorMessage::MissingKey(key),
        )
    }

    fn error_extra_entry(&self, key: String) -> Self::Error {
        let obj_info = self.outline.stack_items.last().expect(NOT_COLLECTION);
        obj_info.assert_object();
        DeserializeError::new(obj_info.pos.clone(), DeserializeErrorMessage::ExtraKey(key))
    }
}

impl<Position> StackItem<Position> {
    /// Asserts that this stack item is for an object.
    pub fn assert_object(&self) {
        assert!(
            matches!(self.collection_type, CollectionType::Object),
            "{}",
            NOT_OBJECT
        );
    }

    /// Asserts that this stack item is for an array.
    pub fn assert_array(&self) {
        assert!(
            matches!(self.collection_type, CollectionType::Array),
            "{}",
            NOT_ARRAY
        );
    }
}

impl<Position> Default for Outline<Position> {
    fn default() -> Self {
        Self {
            stack_items: Vec::new(),
            lookback_items: Vec::new(),
            lookback_keys: Default::default(),
            lookback_data: Vec::new(),
        }
    }
}

impl<Position> Outline<Position> {
    /// Gets the depth of the top container on the deserialization stack, or [`None`] if no
    /// such container exists.
    pub fn top_depth(&self) -> Option<NonZeroU32> {
        NonZeroU32::new(self.stack_items.len().try_into().unwrap())
    }

    /// Asserts that the [`LookbackItem`] at the given index in `lookback_items` has an unread
    /// value, takes it, and returns it, along with the associated data for the value.
    pub fn take_value(&mut self, index: usize) -> (&Position, LookbackValue, &[u8]) {
        let item = &mut self.lookback_items[index];
        let value = item.value.take().expect(VALUE_ALREADY_READ);
        let key_len = item.key_len_active >> 1;
        let data_start_index = item.data_index + usize::try_from(key_len).unwrap();
        let pos = &self.lookback_items[index].pos;
        let data_end_index = self
            .lookback_items
            .get(index + 1)
            .map(|item| item.data_index)
            .unwrap_or(self.lookback_data.len());
        let data = &self.lookback_data[data_start_index..data_end_index];
        (pos, value, data)
    }

    /// Appends a [`LookbackItem`] to `lookback_items`. `next_sibling_index` will be set to
    /// the current value of `last_child_index` and `last_child_index` will be updated to the
    /// index of the new item.
    pub fn push_item(
        &mut self,
        last_child_index: &mut usize,
        pos: Position,
        data_index: usize,
        key_len_active: u32,
        value: LookbackValue,
    ) {
        // TODO: Garbage collection/compaction
        let prev_child_index = *last_child_index;
        *last_child_index = self.lookback_items.len();
        self.lookback_items.push(LookbackItem {
            pos,
            data_index,
            next_sibling_index: prev_child_index,
            key_len_active,
            value: Some(value),
        });
    }
}

/// Gets the index of the first [`LookbackItem`] which has a non-[`None`] value starting at
/// the given index and having the same parent as the item at the given index.
fn first_unread_child<Position>(
    lookback_items: &[LookbackItem<Position>],
    first_child_index: &mut usize,
) -> Option<usize> {
    let mut child_index = *first_child_index;
    loop {
        let child_item = lookback_items.get(child_index)?;
        if child_item.value.is_some() {
            *first_child_index = child_index;
            return Some(child_index);
        } else {
            child_index = child_item.next_sibling_index;
        }
    }
}

/// Computes the hash code for a [`LookbackKey`].
fn key_hash(depth: NonZeroU32, key_bytes: &[u8]) -> u64 {
    let mut hash = hashbrown::hash_map::DefaultHashBuilder::default().build_hasher();
    hash.write_u32(u32::from(depth));
    hash.write(key_bytes);
    hash.finish()
}

impl<Position> LookbackItem<Position> {
    /// Gets the bytes for the key string of the entry item.
    pub fn key_bytes<'a>(&self, lookback_data: &'a [u8]) -> &'a [u8] {
        let key_len = self.key_len_active >> 1;
        &lookback_data[self.data_index..(self.data_index + usize::try_from(key_len).unwrap())]
    }
}

impl<Position> std::fmt::Debug for LookbackItem<Position> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let key_len = self.key_len_active >> 1;
        let active = self.key_len_active & 1 > 0;
        f.debug_struct("LookbackItem")
            .field("data_index", &self.data_index)
            .field("next_sibling_index", &self.next_sibling_index)
            .field("key_len", &key_len)
            .field("active", &active)
            .field("value", &self.value)
            .finish_non_exhaustive()
    }
}

impl LookbackKey {
    /// Gets the hash code for this [`LookbackKey`].
    pub fn hash<Position>(
        &self,
        lookback_items: &[LookbackItem<Position>],
        lookback_data: &[u8],
    ) -> u64 {
        let item = &lookback_items[self.index];
        key_hash(self.depth, item.key_bytes(lookback_data))
    }
}

/// Contains JSON-related extension methods for [`TextReader`].
trait TextReaderExt: TextReader {
    /// Advances the stream past any whitespace characters.
    fn skip_whitespace(
        &mut self,
        allow_comments: bool,
    ) -> Result<(), DeserializeError<Self::Position>> {
        loop {
            match self.peek() {
                Some(' ' | '\n' | '\r' | '\t') => {
                    self.next();
                }
                Some('/') if allow_comments => {
                    self.next();
                    self.skip_comment()?;
                }
                _ => return Ok(()),
            }
        }
    }

    /// Advances the stream to the end of a comment, given that the initial slash (`/`) has already
    /// been consumed.
    fn skip_comment(&mut self) -> Result<(), DeserializeError<Self::Position>> {
        let pos = self.position();
        match self.next() {
            Some('/') => loop {
                match self.next() {
                    Some('\n') => return Ok(()),
                    Some(_) => (),
                    None => return Err(DeserializeError::new(self.position(), UnexpectedEof)),
                }
            },
            Some('*') => loop {
                match self.next() {
                    Some('*') => loop {
                        match self.next() {
                            Some('/') => return Ok(()),
                            Some('*') => (),
                            Some(_) => break,
                            None => {
                                return Err(DeserializeError::new(self.position(), UnexpectedEof))
                            }
                        }
                    },
                    Some(_) => (),
                    None => return Err(DeserializeError::new(self.position(), UnexpectedEof)),
                }
            },
            Some(_) => Err(DeserializeError::new(pos, UnexpectedChar)),
            None => Err(DeserializeError::new(pos, UnexpectedEof)),
        }
    }

    /// Advances the stream past either a quote (`"`), returning its position, or an end curly
    /// brace (`}`), returning [`None`]. Skips whitespace.
    fn skip_to_first_entry(
        &mut self,
        allow_comments: bool,
    ) -> Result<Option<Self::Position>, DeserializeError<Self::Position>> {
        loop {
            let pos = self.position();
            match self.next() {
                Some(' ' | '\n' | '\r' | '\t') => (),
                Some('/') if allow_comments => {
                    self.skip_comment()?;
                }
                Some('"') => return Ok(Some(pos)),
                Some('}') => return Ok(None),
                Some(_) => return Err(DeserializeError::new(pos, UnexpectedChar)),
                None => return Err(DeserializeError::new(pos, UnexpectedEof)),
            }
        }
    }

    /// Advances the stream past either a comma (`,`) and a quote (`"`), returning its position,
    /// or an end curly brace (`}`), returning [`None`]. Skips whitespace.
    fn skip_to_next_entry(
        &mut self,
        allow_comments: bool,
    ) -> Result<Option<Self::Position>, DeserializeError<Self::Position>> {
        loop {
            let pos = self.position();
            match self.next() {
                Some(' ' | '\n' | '\r' | '\t') => (),
                Some('/') if allow_comments => {
                    self.skip_comment()?;
                }
                Some(',') => break,
                Some('}') => return Ok(None),
                Some(_) => return Err(DeserializeError::new(pos, UnexpectedChar)),
                None => return Err(DeserializeError::new(pos, UnexpectedEof)),
            }
        }
        loop {
            let pos = self.position();
            match self.next() {
                Some(' ' | '\n' | '\r' | '\t') => (),
                Some('/') if allow_comments => {
                    self.skip_comment()?;
                }
                Some('"') => return Ok(Some(pos)),
                Some(_) => return Err(DeserializeError::new(pos, UnexpectedChar)),
                None => return Err(DeserializeError::new(pos, UnexpectedEof)),
            }
        }
    }

    /// Advances the stream to the start of a value, returning `true`, or past the end of a
    /// square brace (`]`), returning `false`. Skips whitespace.
    fn skip_to_first_item(
        &mut self,
        allow_comments: bool,
    ) -> Result<bool, DeserializeError<Self::Position>> {
        loop {
            let pos = self.position();
            match self.peek() {
                Some(' ' | '\n' | '\r' | '\t') => {
                    self.next();
                }
                Some('/') if allow_comments => {
                    self.next();
                    self.skip_comment()?;
                }
                Some(']') => {
                    self.next();
                    return Ok(false);
                }
                Some(_) => return Ok(true),
                None => return Err(DeserializeError::new(pos, UnexpectedEof)),
            }
        }
    }

    /// Advances the stream past a comma (`,`), returning `true`, or past the end of a square
    /// brace (`]`), returning `false`. Skips whitespace.
    fn skip_to_next_item(
        &mut self,
        allow_comments: bool,
    ) -> Result<bool, DeserializeError<Self::Position>> {
        loop {
            let pos = self.position();
            match self.next() {
                Some(' ' | '\n' | '\r' | '\t') => (),
                Some('/') if allow_comments => {
                    self.skip_comment()?;
                }
                Some(',') => return Ok(true),
                Some(']') => return Ok(false),
                Some(_) => return Err(DeserializeError::new(pos, UnexpectedChar)),
                None => return Err(DeserializeError::new(pos, UnexpectedEof)),
            }
        }
    }

    /// Produces an error if the stream is not at the end of input.
    fn read_eof(&mut self) -> Result<(), DeserializeError<Self::Position>> {
        if self.peek().is_some() {
            Err(DeserializeError::new(self.position(), UnexpectedChar))
        } else {
            Ok(())
        }
    }

    /// Reads a JSON boolean.
    fn read_bool(&mut self) -> Result<bool, DeserializeError<Self::Position>> {
        let pos = self.position();
        match self.next() {
            Some('t') => {
                if !self.read_exact("rue") {
                    return Err(DeserializeError::new(pos, InvalidLiteral));
                }
                Ok(true)
            }
            Some('f') => {
                if !self.read_exact("alse") {
                    return Err(DeserializeError::new(pos, InvalidLiteral));
                }
                Ok(false)
            }
            Some(_) => Err(DeserializeError::new(pos, ExpectedBool)),
            None => Err(DeserializeError::new(pos, UnexpectedEof)),
        }
    }

    /// Reads a JSON number.
    fn read_number<T: Num>(&mut self) -> Result<T, DeserializeError<Self::Position>> {
        let mut builder: T::Builder = Default::default();
        let pos = self.position();
        let (negate, exp) = self.read_number_into_builder(&mut builder)?;
        T::from_builder(builder, negate, exp)
            .ok_or_else(|| DeserializeError::new(pos, NumberOverflow))
    }

    /// Reads a JSON number into a [`NumBuilder`], also returning whether the number is negated and
    /// what its base-10 exponent is.
    fn read_number_into_builder(
        &mut self,
        builder: &mut impl NumBuilder,
    ) -> Result<(bool, i32), DeserializeError<Self::Position>> {
        let pos = self.position();
        let mut negate = false;
        let mut decimal_exp = 0;
        'fractional: {
            'integral: {
                // Parse leading digit (and sign) of integral component.
                match self.next() {
                    Some('0') => match self.peek() {
                        Some('.') => {
                            self.next();
                            break 'integral;
                        }
                        Some('e' | 'E') => {
                            self.next();
                            break 'fractional;
                        }
                        _ => {
                            return Ok((false, 0));
                        }
                    },
                    Some(ch @ '1'..='9') => {
                        if !builder.push_digit((ch as u8) - b'0') {
                            return Err(DeserializeError::new(pos, NumberOverflow));
                        }
                    }
                    Some('-') => match self.next() {
                        Some('0') => match self.peek() {
                            Some('.') => {
                                self.next();
                                negate = true;
                                break 'integral;
                            }
                            Some('e' | 'E') => {
                                self.next();
                                negate = true;
                                break 'fractional;
                            }
                            _ => {
                                return Ok((true, 0));
                            }
                        },
                        Some(ch @ '1'..='9') => {
                            if !builder.push_digit((ch as u8) - b'0') {
                                return Err(DeserializeError::new(pos, NumberOverflow));
                            }
                            negate = true;
                        }
                        Some(_) => return Err(DeserializeError::new(pos, ExpectedNumber)),
                        None => return Err(DeserializeError::new(pos, UnexpectedEof)),
                    },
                    Some(_) => return Err(DeserializeError::new(pos, ExpectedNumber)),
                    None => return Err(DeserializeError::new(pos, UnexpectedEof)),
                }

                // Parse remaining digits of integral component
                loop {
                    match self.peek() {
                        Some(ch @ '0'..='9') => {
                            self.next();
                            if !builder.push_digit((ch as u8) - b'0') {
                                return Err(DeserializeError::new(pos, NumberOverflow));
                            }
                        }
                        Some('.') => {
                            self.next();
                            break 'integral;
                        }
                        Some('e' | 'E') => {
                            self.next();
                            break 'fractional;
                        }
                        _ => {
                            return Ok((negate, 0));
                        }
                    }
                }
            }

            // Parse fractional component (we already read the decimal point)
            match self.next() {
                Some(ch @ '0'..='9') => {
                    decimal_exp -= 1;
                    if !builder.push_digit((ch as u8) - b'0') {
                        return Err(DeserializeError::new(pos, NumberOverflow));
                    }
                }
                Some(_) => return Err(DeserializeError::new(pos, ExpectedNumber)),
                None => return Err(DeserializeError::new(pos, UnexpectedEof)),
            }
            loop {
                match self.peek() {
                    Some(ch @ '0'..='9') => {
                        self.next();
                        decimal_exp -= 1;
                        if !builder.push_digit((ch as u8) - b'0') {
                            return Err(DeserializeError::new(pos, NumberOverflow));
                        }
                    }
                    Some('e' | 'E') => {
                        self.next();
                        break 'fractional;
                    }
                    _ => {
                        return Ok((negate, decimal_exp));
                    }
                }
            }
        }

        // Parse exponent (we already read the 'e'/'E').
        let mut exp_builder: u32 = 0;
        let mut negate_exp = false;
        match self.next() {
            Some(ch @ '0'..='9') => {
                exp_builder.push_digit((ch as u8) - b'0');
            }
            Some('+') => match self.next() {
                Some(ch @ '0'..='9') => {
                    exp_builder.push_digit((ch as u8) - b'0');
                }
                Some(_) => return Err(DeserializeError::new(pos, ExpectedNumber)),
                None => return Err(DeserializeError::new(pos, UnexpectedEof)),
            },
            Some('-') => match self.next() {
                Some(ch @ '0'..='9') => {
                    negate_exp = true;
                    exp_builder.push_digit((ch as u8) - b'0');
                }
                Some(_) => return Err(DeserializeError::new(pos, ExpectedNumber)),
                None => return Err(DeserializeError::new(pos, UnexpectedEof)),
            },
            Some(_) => return Err(DeserializeError::new(pos, ExpectedNumber)),
            None => return Err(DeserializeError::new(pos, UnexpectedEof)),
        }
        loop {
            match self.peek() {
                Some(ch @ '0'..='9') => {
                    self.next();
                    if !exp_builder.push_digit((ch as u8) - b'0') {
                        return Err(DeserializeError::new(pos, NumberOverflow));
                    }
                }
                _ => {
                    let Some(exp) = i32::from_builder(exp_builder, negate_exp, 0) else {
                        return Err(DeserializeError::new(pos, NumberOverflow));
                    };
                    let Some(exp) = exp.checked_add(decimal_exp) else {
                        return Err(DeserializeError::new(pos, NumberOverflow));
                    };
                    return Ok((negate, exp));
                }
            }
        }
    }

    /// Reads an escape sequence in a quoted string, following the backslash.
    fn read_escape_sequence(&mut self) -> Result<char, DeserializeError<Self::Position>> {
        let pos = self.position();
        Ok(match self.next() {
            Some('\"') => '\"',
            Some('\\') => '\\',
            Some('/') => '/',
            Some('b') => '\x08',
            Some('f') => '\x0C',
            Some('n') => '\n',
            Some('r') => '\r',
            Some('t') => '\t',
            Some('u') => todo!(),
            Some(_) => return Err(DeserializeError::new(pos, UnrecognizedEscape)),
            None => return Err(DeserializeError::new(self.position(), UnexpectedEof)),
        })
    }

    /// Reads a quoted string for a entry key, starting from the first character inside the quotes.
    /// If it matches the given expected string, returns `true`. Otherwise, returns `false` and
    /// appends the bytes for the actual key string to `data`.
    fn read_str_bytes_into_or_match(
        &mut self,
        expected: &str,
        data: &mut Vec<u8>,
    ) -> Result<bool, DeserializeError<Self::Position>> {
        let mut suffix = expected;
        loop {
            if let Some(exp_ch) = suffix.peek() {
                let act_ch = match self.next() {
                    Some('"') => {
                        // Actual key is shorter than expected key
                        data.extend_from_slice(prefix(expected, suffix).as_bytes());
                        return Ok(false);
                    }
                    Some('\\') => self.read_escape_sequence()?,
                    Some(ch) => ch,
                    None => return Err(DeserializeError::new(self.position(), UnexpectedEof)),
                };
                if exp_ch != act_ch {
                    // Keys have a discrepancy
                    data.extend_from_slice(prefix(expected, suffix).as_bytes());
                    data.extend_from_slice(act_ch.encode_utf8(&mut [0; 4]).as_bytes());
                    break;
                } else {
                    suffix.next();
                }
            } else {
                let extra_ch = match self.next() {
                    Some('"') => {
                        // Keys match
                        return Ok(true);
                    }
                    Some('\\') => self.read_escape_sequence()?,
                    Some(ch) => ch,
                    None => return Err(DeserializeError::new(self.position(), UnexpectedEof)),
                };

                // Actual key is longer than expected key
                data.extend_from_slice(expected.as_bytes());
                data.extend_from_slice(extra_ch.encode_utf8(&mut [0; 4]).as_bytes());
                break;
            }
        }

        // Append extra characters to data
        self.read_str_bytes_into(data)?;
        Ok(false)
    }

    /// Reads a quoted string into `data` as bytes, starting from the first character inside the
    /// quotes, and ending after the end quote.
    fn read_str_bytes_into(
        &mut self,
        data: &mut Vec<u8>,
    ) -> Result<(), DeserializeError<Self::Position>> {
        loop {
            let ch = match self.next() {
                Some('"') => break,
                Some('\\') => self.read_escape_sequence()?,
                Some(ch) => ch,
                None => return Err(DeserializeError::new(self.position(), UnexpectedEof)),
            };
            data.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
        }
        Ok(())
    }

    /// Reads a quoted string into `data`, starting from the first character inside the quotes, and
    /// ending after the end quote.
    fn read_str_into(&mut self, str: &mut String) -> Result<(), DeserializeError<Self::Position>> {
        loop {
            let ch = match self.next() {
                Some('"') => break,
                Some('\\') => self.read_escape_sequence()?,
                Some(ch) => ch,
                None => return Err(DeserializeError::new(self.position(), UnexpectedEof)),
            };
            str.push(ch);
        }
        Ok(())
    }

    /// Advances the stream until just past a colon (`:`), skipping whitespace.
    fn skip_past_colon(
        &mut self,
        allow_comments: bool,
    ) -> Result<(), DeserializeError<Self::Position>> {
        loop {
            let pos = self.position();
            match self.next() {
                Some(' ' | '\n' | '\r' | '\t') => (),
                Some('/') if allow_comments => {
                    self.skip_comment()?;
                }
                Some(':') => break,
                Some(_) => return Err(DeserializeError::new(pos, UnexpectedChar)),
                None => return Err(DeserializeError::new(pos, UnexpectedEof)),
            }
        }
        Ok(())
    }

    /// Reads a JSON value into a [`LookbackItem`] and appends it to `outline`. Returns the index
    /// of the item.
    fn read_lookback_value(
        &mut self,
        config: &TextDeserializerConfig,
        mut data_index: usize,
        mut key_len_active: u32,
        outline: &mut Outline<Self::Position>,
    ) -> Result<usize, DeserializeError<Self::Position>> {
        let pos = self.position();
        let mut depth = outline.top_depth().unwrap();
        let mut last_child_index = usize::MAX;
        let mut collection_type = CollectionType::Object; // Default value prior to being set.
        let end_depth = depth;

        // Keep reading until we are back to the depth we started at
        'next_value: loop {
            match self.peek() {
                Some(' ' | '\n' | '\r' | '\t') => {
                    // Skip leading whitespace
                    self.next();
                    continue;
                }
                Some('"') => {
                    let pos = self.position();
                    self.next();
                    self.read_str_bytes_into(&mut outline.lookback_data)?;
                    outline.push_item(
                        &mut last_child_index,
                        pos,
                        data_index,
                        key_len_active,
                        LookbackValue::String,
                    );
                }
                Some('-' | '0'..='9') => {
                    let pos = self.position();
                    let (negate, exp) = {
                        let mut builder = LookbackNumBuilder {
                            target: &mut outline.lookback_data,
                            buf: None,
                        };
                        self.read_number_into_builder(&mut builder)?
                    };
                    let Ok(exp) = exp.try_into() else {
                        return Err(DeserializeError::new(
                            pos,
                            DeserializeErrorMessage::NumberOverflow,
                        ));
                    };
                    outline.push_item(
                        &mut last_child_index,
                        pos,
                        data_index,
                        key_len_active,
                        LookbackValue::Number { negate, exp },
                    );
                }
                Some('{') => {
                    let start_pos = self.position();
                    self.next();

                    // Search for the start of the first entry of the object
                    if let Some(pos) = self.skip_to_first_entry(config.allow_comments)? {
                        outline.push_item(
                            &mut last_child_index,
                            start_pos,
                            data_index,
                            key_len_active,
                            LookbackValue::Object { has_entries: true },
                        );

                        // Read entry key
                        data_index = outline.lookback_data.len();
                        self.read_str_bytes_into(&mut outline.lookback_data)?;
                        let key_end_index = outline.lookback_data.len();
                        let key_len = key_end_index - data_index;
                        key_len_active = if key_len <= usize::try_from(u32::MAX >> 1).unwrap() {
                            // Temporarily set `active` to 1 to mark this as a entry, rather than
                            // an arrayitem.
                            ((key_len as u32) << 1) | 1
                        } else {
                            return Err(DeserializeError::new(pos, KeyTooLong));
                        };

                        // Start reading value
                        self.skip_past_colon(config.allow_comments)?;
                        // TODO: Return an error instead of panic
                        depth = depth.checked_add(1).expect("depth overflow");
                        last_child_index = usize::MAX;
                        collection_type = CollectionType::Object;
                        continue 'next_value;
                    } else {
                        // Add empty object to outline
                        outline.push_item(
                            &mut last_child_index,
                            start_pos,
                            data_index,
                            key_len_active,
                            LookbackValue::Object { has_entries: false },
                        );
                    }
                }
                Some('[') => {
                    let start_pos = self.position();
                    self.next();

                    // Search for start of the first item of the array
                    if self.skip_to_first_item(config.allow_comments)? {
                        outline.push_item(
                            &mut last_child_index,
                            start_pos,
                            data_index,
                            key_len_active,
                            LookbackValue::Array { has_items: true },
                        );

                        // Start reading item
                        data_index = outline.lookback_data.len();
                        key_len_active = 0;
                        // TODO: Return an error instead of panic
                        depth = depth.checked_add(1).expect("depth overflow");
                        last_child_index = usize::MAX;
                        collection_type = CollectionType::Array;
                        continue 'next_value;
                    } else {
                        // Add empty array to outline
                        outline.push_item(
                            &mut last_child_index,
                            start_pos,
                            data_index,
                            key_len_active,
                            LookbackValue::Array { has_items: false },
                        );
                    }
                }
                Some('t') => {
                    let pos = self.position();
                    self.next();
                    if !self.read_exact("rue") {
                        return Err(DeserializeError::new(
                            pos,
                            DeserializeErrorMessage::InvalidLiteral,
                        ));
                    }
                    outline.push_item(
                        &mut last_child_index,
                        pos,
                        data_index,
                        key_len_active,
                        LookbackValue::Bool(true),
                    );
                }
                Some('f') => {
                    let pos = self.position();
                    self.next();
                    if !self.read_exact("alse") {
                        return Err(DeserializeError::new(
                            pos,
                            DeserializeErrorMessage::InvalidLiteral,
                        ));
                    }
                    outline.push_item(
                        &mut last_child_index,
                        pos,
                        data_index,
                        key_len_active,
                        LookbackValue::Bool(false),
                    );
                }
                Some('n') => {
                    let pos = self.position();
                    self.next();
                    if !self.read_exact("ull") {
                        return Err(DeserializeError::new(
                            pos,
                            DeserializeErrorMessage::InvalidLiteral,
                        ));
                    }
                    outline.push_item(
                        &mut last_child_index,
                        pos,
                        data_index,
                        key_len_active,
                        LookbackValue::Null,
                    );
                }
                Some(_) => {
                    return Err(DeserializeError::new(
                        pos,
                        DeserializeErrorMessage::UnexpectedChar,
                    ))
                }
                None => {
                    return Err(DeserializeError::new(
                        pos,
                        DeserializeErrorMessage::UnexpectedEof,
                    ))
                }
            }

            // We read a value, are we done?
            while depth > end_depth {
                match collection_type {
                    CollectionType::Object => {
                        if let Some(pos) = self.skip_to_next_entry(config.allow_comments)? {
                            // Read entry key
                            data_index = outline.lookback_data.len();
                            self.read_str_bytes_into(&mut outline.lookback_data)?;
                            let key_end_index = outline.lookback_data.len();
                            let key_len = key_end_index - data_index;
                            key_len_active = if key_len <= usize::try_from(u32::MAX >> 1).unwrap() {
                                // Temporarily set `active` to 1 to mark this as a entry, rather than
                                // an array item.
                                ((key_len as u32) << 1) | 1
                            } else {
                                return Err(DeserializeError::new(pos, KeyTooLong));
                            };

                            // Read next value
                            self.skip_past_colon(config.allow_comments)?;
                            continue 'next_value;
                        }
                    }
                    CollectionType::Array => {
                        if self.skip_to_next_item(config.allow_comments)? {
                            data_index = outline.lookback_data.len();
                            key_len_active = 0;
                            continue 'next_value;
                        }
                    }
                }

                // Close current object
                let first_child_index =
                    correct_items(&mut outline.lookback_items, last_child_index);
                let parent_index = first_child_index - 1;
                depth = NonZeroU32::new(u32::from(depth) - 1).unwrap();
                last_child_index = parent_index;
                collection_type = if outline.lookback_items[parent_index].key_len_active & 1 > 0 {
                    CollectionType::Object
                } else {
                    CollectionType::Array
                };
            }

            // Return index of the initial item
            outline.lookback_items[last_child_index].next_sibling_index =
                outline.lookback_items.len();
            return Ok(last_child_index);
        }
    }
}

/// Given that there there is a set of items in `items` that are linked in reverse through
/// `next_sibling_index`, corrects the link direction and returns the index of the first item in
/// the linked list.
fn correct_items<Position>(items: &mut [LookbackItem<Position>], last_child_index: usize) -> usize {
    let mut item_index = last_child_index;
    let mut item = &mut items[item_index];
    let mut prev_index = item.next_sibling_index;
    item.next_sibling_index = usize::MAX;
    loop {
        item.key_len_active &= !1; // Clear temporary entry flag
        if prev_index < usize::MAX {
            let n_item_index = prev_index;
            item = &mut items[n_item_index];
            prev_index = item.next_sibling_index;
            item.next_sibling_index = item_index;
            item_index = n_item_index;
        } else {
            break item_index;
        }
    }
}

impl<T: TextReader> TextReaderExt for T {}

/// A [`NumBuilder`] which stores digit data in a byte array.
struct LookbackNumBuilder<'a> {
    target: &'a mut Vec<u8>,
    buf: Option<u8>,
}

impl NumBuilder for LookbackNumBuilder<'_> {
    fn push_digit(&mut self, digit: u8) -> bool {
        if let Some(mut buf) = self.buf.take() {
            buf |= digit << 4;
            self.target.push(buf);
        } else {
            self.buf = Some(digit);
        }
        true
    }
}

impl Drop for LookbackNumBuilder<'_> {
    fn drop(&mut self) {
        if let Some(mut buf) = self.buf.take() {
            buf |= 0xF0;
            self.target.push(buf);
        } else {
            self.target.push(0xFF);
        }
    }
}

/// Describes an error that can occur when deserializing JSON.
pub struct DeserializeError<Position>(Box<DeserializeErrorInner<Position>>);

/// The inner data for a [`DeserializeError`].
struct DeserializeErrorInner<Position> {
    /// The position in the input stream where this error occured.
    pos: Position,

    /// Gets the message for this error.
    message: DeserializeErrorMessage,
}

/// A possible message for a [`DeserializeError`].
// TODO: Use `thiserror`
#[derive(Debug)]
pub enum DeserializeErrorMessage {
    Custom(Box<dyn std::error::Error + Send + Sync>),
    UnexpectedEof,
    UnexpectedChar,
    InvalidLiteral,
    ExpectedString,
    ExpectedNumber,
    ExpectedObject,
    ExpectedArray,
    ExpectedBool,
    ExpectedNull,
    NumberOverflow,
    UnrecognizedEscape,
    MissingKey(String),
    ExtraKey(String),
    KeyTooLong,
    MissingItems,
    ExcessItems,
}

impl<Position> DeserializeError<Position> {
    /// Constructs a new error with the given position and message.
    pub fn new(pos: Position, message: DeserializeErrorMessage) -> Self {
        Self(Box::new(DeserializeErrorInner { pos, message }))
    }

    /// Gets the position in the input stream where this error occurred.
    pub fn position(&self) -> &Position {
        &self.0.pos
    }

    /// Gets the message for this error.
    pub fn message(&self) -> &DeserializeErrorMessage {
        &self.0.message
    }
}

impl std::fmt::Display for DeserializeErrorMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Custom(source) => source.fmt(f),
            UnexpectedEof => f.write_str("unexpected EOF"),
            UnexpectedChar => f.write_str("unexpected character"),
            InvalidLiteral => f.write_str("invalid literal"),
            ExpectedString => f.write_str("expected string"),
            ExpectedNumber => f.write_str("expected number"),
            ExpectedObject => f.write_str("expected object"),
            ExpectedArray => f.write_str("expected array"),
            ExpectedBool => f.write_str("expected bool"),
            ExpectedNull => f.write_str("expected 'null'"),
            NumberOverflow => f.write_str("numeric overflow"),
            UnrecognizedEscape => f.write_str("unrecognized escape sequence"),
            MissingKey(key) => write!(f, "missing object key {:?}", key),
            ExtraKey(key) => write!(f, "extra object key {:?}", key),
            KeyTooLong => f.write_str("object key too long"),
            MissingItems => f.write_str("array has fewer items than expected"),
            ExcessItems => f.write_str("array has more items than expected"),
        }
    }
}

impl<Position: std::fmt::Debug> std::fmt::Debug for DeserializeError<Position> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("json::DeserializeError")
            .field("pos", self.position())
            .field("message", self.message())
            .finish()
    }
}

impl<Position: std::fmt::Display> std::fmt::Display for DeserializeError<Position> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.message(), self.position())
    }
}

impl<Position: std::fmt::Debug + std::fmt::Display> std::error::Error
    for DeserializeError<Position>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let DeserializeErrorMessage::Custom(source) = self.message() {
            Some(&**source)
        } else {
            None
        }
    }
}
