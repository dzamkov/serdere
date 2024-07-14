mod text;

use super::JsonOutliner;
use crate::{Serialize, Serializer, TextWriter, Value};
pub use text::*;

/// Extends [`Serializer`] with JSON-specific functionality.
pub trait JsonSerializer: JsonOutliner + Serializer {
    /// Assuming that the top item on the stack is a value, asserts that it is an ordered list
    /// with an unspecified number of items, popping it and pushing an opened list onto the stack.
    fn open_list_streaming(&mut self) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is an opened object, adds an entry to it. This
    /// pushes the value and opened key string onto the stack, in that order.
    fn add_entry(&mut self) -> Result<(), Self::Error>;
}

/// The standard implementation of [`Serializer::put_tag`] for a [`JsonSerializer`].
pub fn put_tag<S: JsonSerializer + ?Sized>(
    serializer: &mut S,
    max_index: usize,
    index: usize,
    name: Option<&'static str>,
) -> Result<(), S::Error> {
    let _ = max_index;
    if let Some(name) = name {
        serializer.put_str(name)
    } else {
        serializer.put_u64(index.try_into().unwrap())
    }
}

/// Serializes a value of type `T` to a [`TextWriter`], formatting it as JSON.
pub fn to_writer<Writer: TextWriter, T: Serialize<TextSerializer<Writer>> + ?Sized>(
    writer: Writer,
    value: &T,
) -> Result<(), Writer::Error> {
    to_writer_using(writer, value, &mut ())
}

/// Serializes a value of type `T` to a [`TextWriter`], formatting it as JSON.
pub fn to_writer_using<
    Writer: TextWriter,
    T: Serialize<TextSerializer<Writer>, Ctx> + ?Sized,
    Ctx: ?Sized,
>(
    writer: Writer,
    value: &T,
    context: &mut Ctx,
) -> Result<(), Writer::Error> {
    let mut s = TextSerializer::new(TextSerializerConfig::default(), writer);
    let mut done_flag = false;
    value.serialize(Value::new(&mut s, &mut done_flag), context)?;
    Ok(())
}

/// Serializes a value of type `T` as a JSON string.
pub fn to_str<T: for<'a> Serialize<TextSerializer<&'a mut String>> + ?Sized>(value: &T) -> String {
    to_str_using(value, &mut ())
}

/// Serializes a value of type `T` as a JSON string.
pub fn to_str_using<
    T: for<'a> Serialize<TextSerializer<&'a mut String>, Ctx> + ?Sized,
    Ctx: ?Sized,
>(
    value: &T,
    context: &mut Ctx,
) -> String {
    let mut str = String::new();
    to_writer_using(&mut str, value, context).unwrap();
    str
}
