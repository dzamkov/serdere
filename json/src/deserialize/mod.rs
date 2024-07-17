mod number;
mod text;

use crate::{CollectionType, JsonOutliner, ValueType};
#[allow(unused_imports)]
use serdere::Outliner;
use serdere::{Deserialize, Deserializer, StrPosition, TextReader};
use serdere::{NameMap, Value};
pub use text::*;

/// Extends [`Deserializer`] with JSON-specific functionality.
///
/// The following methods should use standard implementations:
///
/// | Method                     | Implementation    |
/// | -------------------------- | ----------------- |
/// | [`Deserializer::get_tag`]  | [`get_tag`]       |
/// | [`Outliner::open_struct`]  | [`open_struct`]   |
/// | [`Outliner::push_field`]   | [`push_field`]    |
/// | [`Outliner::close_struct`] | [`close_struct`]  |
/// | [`Outliner::push_item`]    | [`push_item`]     |
/// | [`Outliner::close_list`]   | [`close_list`]    |
pub trait JsonDeserializer: JsonOutliner + Deserializer {
    /// Assuming that the top item on the stack is a value, returns its JSON [`ValueType`].
    fn peek_value_type(&self) -> ValueType;

    /// Assuming that the top item on the stack is an opened collection, returns its
    /// [`CollectionType`].
    fn peek_collection_type(&self) -> CollectionType;

    /// Assuming that the top item on the stack is a value, pops it without deserialization.
    fn skip_value(&mut self) -> Result<(), Self::Error> {
        match self.peek_value_type() {
            ValueType::String => {
                self.open_str()?;
                self.skip_str()?;
            }
            ValueType::Number => {
                self.get_f32()?;
            }
            ValueType::Object => {
                self.open_object()?;
                self.skip_object()?;
            }
            ValueType::Array => {
                self.open_list()?;
                while self.next_item()? {
                    self.skip_value()?;
                }
            }
            ValueType::Bool => {
                self.get_bool()?;
            }
            ValueType::Null => self.pop_null()?,
        }
        Ok(())
    }

    /// Pushes a "virtual" `null` literal as a value onto the stack, assuming that the current top
    /// item is an opened collection. Optionally, the key associated with the `null` can be
    /// specified, which may be used in error messages.
    ///
    /// This functionality is necessary to implement [`Deserializer::load_field`] for a field that
    /// is not present in an object.
    fn push_null(&mut self, key: Option<&'static str>);

    /// Assuming that the top item on the stack is an opened object, checks whether it has an
    /// entry with the given key. If so, pushes the value of the entry on the stack and returns
    /// `true`. Otherwise, returns `false`.
    fn try_push_entry(&mut self, key: &str) -> Result<bool, Self::Error>;

    /// Assuming that the top item on the stack is an opened object, tries getting the next unread
    /// entry from it. If one exists, this will return `true` and push the value and opened key
    /// string onto the stack, in that order. Otherwise, this will return `false` and pop the
    /// object from the stack (i.e. closing it).
    fn next_entry(&mut self) -> Result<bool, Self::Error>;

    /// Assuming that the top item on the stack is an opened object, pops it without deserializing
    /// remaining entries.
    fn skip_object(&mut self) -> Result<(), Self::Error> {
        while self.next_entry()? {
            self.skip_str()?;
            self.skip_value()?;
        }
        Ok(())
    }

    /// Constructs an error which says that a particular key for an object is missing. If errors
    /// contain position information, the error will be tagged to the most recently popped item
    /// (which should be an object).
    fn error_missing_entry(&self, key: String) -> Self::Error;

    /// Assuming that there is an opened object on top of the stack, constructs an error which says
    /// there is an unexpected extra object entry. If errors contain position information, the
    /// error will be tagged to the object.
    fn error_extra_entry(&self, key: String) -> Self::Error;
}

/// The standard implementation of [`Deserializer::get_tag`] for a [`JsonDeserializer`].
pub fn get_tag<D: JsonDeserializer + ?Sized>(
    deserializer: &mut D,
    max_index: usize,
    names: &'static NameMap<usize>,
) -> Result<usize, D::Error> {
    if let ValueType::String = deserializer.peek_value_type() {
        deserializer.get_name(names)
    } else {
        let index = deserializer.get_u64()?;
        if let Ok(index) = index.try_into() {
            if index <= max_index {
                Ok(index)
            } else {
                Err(deserializer.error_invalid_index(index))
            }
        } else {
            Err(deserializer.error_invalid_index(usize::MAX))
        }
    }
}

/// The standard implementation of [`Outliner::open_struct`] for a [`JsonDeserializer`].
pub fn open_struct<D: JsonDeserializer + ?Sized>(
    deserializer: &mut D,
    type_name: Option<&'static str>,
) -> Result<(), D::Error> {
    let _ = type_name;
    match deserializer.peek_value_type() {
        ValueType::Object => deserializer.open_object(),
        ValueType::Array => {
            deserializer.open_list()?;
            Ok(())
        }
        _ => todo!(), // TODO: Error
    }
}

/// The standard implementation of [`Outliner::push_field`] for a [`JsonDeserializer`].
pub fn push_field<D: JsonDeserializer + ?Sized>(
    deserializer: &mut D,
    name: &'static str,
) -> Result<(), D::Error> {
    match deserializer.peek_collection_type() {
        CollectionType::Object => {
            if !deserializer.try_push_entry(name)? {
                deserializer.push_null(Some(name))
            }
        }
        CollectionType::Array => {
            if !deserializer.next_item()? {
                return Err(deserializer.error_missing_item());
            }
        }
    }
    Ok(())
}

/// The standard implementation of [`Outliner::close_struct`] for a [`JsonDeserializer`].
pub fn close_struct<D: JsonDeserializer + ?Sized>(deserializer: &mut D) -> Result<(), D::Error> {
    match deserializer.peek_collection_type() {
        CollectionType::Object => {
            deserializer.skip_object()?;
        }
        CollectionType::Array => {
            if deserializer.next_item()? {
                deserializer.skip_value()?;
                return Err(deserializer.error_extra_item());
            }
        }
    }
    Ok(())
}

/// The standard implementation of [`Outliner::push_item`] for a [`JsonDeserializer`].
pub fn push_item<D: JsonDeserializer + ?Sized>(deserializer: &mut D) -> Result<(), D::Error> {
    if !deserializer.next_item()? {
        return Err(deserializer.error_missing_item());
    }
    Ok(())
}

/// The standard implementation of [`Outliner::close_list`] for a [`JsonDeserializer`].
pub fn close_list<D: JsonDeserializer + ?Sized>(deserializer: &mut D) -> Result<(), D::Error> {
    if deserializer.next_item()? {
        deserializer.skip_value()?;
        return Err(deserializer.error_extra_item());
    }
    Ok(())
}

/// Deserializes a value of type `T` from a [`TextReader`], interpreting the text as JSON.
pub fn from_reader<Reader: TextReader, T: Deserialize<TextDeserializer<Reader>>>(
    reader: Reader,
) -> Result<T, DeserializeError<Reader::Position>> {
    from_reader_using(reader, &mut ())
}

/// Deserializes a value of type `T` from a [`TextReader`], interpreting the text as JSON.
pub fn from_reader_using<
    Reader: TextReader,
    T: Deserialize<TextDeserializer<Reader>, Ctx>,
    Ctx: ?Sized,
>(
    reader: Reader,
    context: &mut Ctx,
) -> Result<T, DeserializeError<Reader::Position>> {
    let mut d = TextDeserializer::new(TextDeserializerConfig::default(), reader)?;
    let res = Value::with(&mut d, |value| T::deserialize(value, context))?;
    d.close()?;
    Ok(res)
}

/// Deserializes a value of type `T` from a string, interpreting it as JSON.
pub fn from_str<'s, T: Deserialize<TextDeserializer<&'s str>>>(
    str: &'s str,
) -> Result<T, DeserializeError<StrPosition<'s>>> {
    from_str_using(str, &mut ())
}

/// Deserializes a value of type `T` from a string, interpreting it as JSON.
pub fn from_str_using<'s, T: Deserialize<TextDeserializer<&'s str>, Ctx>, Ctx: ?Sized>(
    str: &'s str,
    context: &mut Ctx,
) -> Result<T, DeserializeError<StrPosition<'s>>> {
    from_reader_using(str, context)
}
