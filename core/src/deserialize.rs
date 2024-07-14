#[allow(unused_imports)]
use crate::Serializer;
use crate::{NameMap, Outliner, Struct, Value};
use std::borrow::Cow;

/// An interface for loading arbitrarily-complex data from a data source. This uses a stack-based
/// API, described in [`Outliner`].
pub trait Deserializer: Outliner {
    /// Assuming that the top item on the stack is a value, pops it and interprets it as a
    /// [`bool`].
    fn get_bool(&mut self) -> Result<bool, Self::Error>;

    /// Assuming that the top item on the stack is a value, pops it and interprets it as an
    /// [`i8`].
    fn get_i8(&mut self) -> Result<i8, Self::Error>;

    /// Assuming that the top item on the stack is a value, pops it and interprets it as an
    /// [`i16`].
    fn get_i16(&mut self) -> Result<i16, Self::Error>;

    /// Assuming that the top item on the stack is a value, pops it and interprets it as an
    /// [`i32`].
    fn get_i32(&mut self) -> Result<i32, Self::Error>;

    /// Assuming that the top item on the stack is a value, pops it and interprets it as an
    /// [`i64`].
    fn get_i64(&mut self) -> Result<i64, Self::Error>;

    /// Assuming that the top item on the stack is a value, pops it and interprets it as a
    /// [`u8`].
    fn get_u8(&mut self) -> Result<u8, Self::Error>;

    /// Assuming that the top item on the stack is a value, pops it and interprets it as a
    /// [`u16`].
    fn get_u16(&mut self) -> Result<u16, Self::Error>;

    /// Assuming that the top item on the stack is a value, pops it and interprets it as a
    /// [`u32`].
    fn get_u32(&mut self) -> Result<u32, Self::Error>;

    /// Assuming that the top item on the stack is a value, pops it and interprets it as a
    /// [`u64`].
    fn get_u64(&mut self) -> Result<u64, Self::Error>;

    /// Assuming that the top item on the stack is a value, pops it and interprets it as an
    /// [`f32`].
    fn get_f32(&mut self) -> Result<f32, Self::Error>;

    /// Assuming that the top item on the stack is a value, pops it and interprets it as an
    /// [`f64`].
    fn get_f64(&mut self) -> Result<f64, Self::Error>;

    /// Assuming that the top item on the stack is a value, pops it and interprets it as a
    /// [`char`].
    fn get_char(&mut self) -> Result<char, Self::Error>;

    /// Assuming the top item on the stack is a string, tries getting the first character
    /// from it. If one exists, it will be returned. Otherwise, the string will be popped from
    /// the stack and this will return `Ok(None)`.
    fn next_char(&mut self) -> Result<Option<char>, Self::Error>;

    /// Assuming that the top item on the stack is an opened string, reads the remainder of it,
    /// and then pops it. This will return a direct reference to the string data if possible.
    fn flush_str(&mut self) -> Result<Cow<str>, Self::Error> {
        let mut str = String::new();
        while let Some(ch) = self.next_char()? {
            str.push(ch);
        }
        Ok(Cow::Owned(str))
    }

    /// Assuming that the top item on the stack is an opened string, skips the remainder of it
    /// and pops it.
    fn skip_str(&mut self) -> Result<(), Self::Error> {
        while self.next_char()?.is_some() {}
        Ok(())
    }

    /// Assuming that the top item on the stack is a value, pops it and returns it, interpreting
    /// it as a string.
    fn read_str(&mut self) -> Result<Cow<str>, Self::Error> {
        self.open_str()?;
        self.flush_str()
    }

    /// Assuming that the top item on the stack is an opened string, uses the remainder of it
    /// to perform a lookup into `names`, then pops it.
    fn flush_name(&mut self, names: &'static NameMap<usize>) -> Result<usize, Self::Error> {
        let mut lookup = names.lookup();
        while let Some(ch) = self.next_char()? {
            lookup.write_char(ch);
        }
        lookup
            .result()
            .copied()
            .ok_or_else(|| self.error_invalid_name(names))
    }

    /// Assuming that the top item on the stack is a value, pops it and returns it, interpreting
    /// it as a name in the given [`NameMap`].
    fn get_name(&mut self, names: &'static NameMap<usize>) -> Result<usize, Self::Error> {
        self.open_str()?;
        self.flush_name(names)
    }

    /// Assuming that the top item on the stack is a value, pops it and interprets it as an enum
    /// tag. The names of the possible tags (or a subset of them) are provided by a given
    /// [`NameMap`]. Depending on the underlying serialization format, this may accept a string,
    /// an integer index, or both.
    fn get_tag(
        &mut self,
        max_index: usize,
        names: &'static NameMap<usize>,
    ) -> Result<usize, Self::Error>;

    /// Assuming that the top item on the stack is a value, checks whether it is a `null` literal.
    /// If so, the value is popped and this returns `true`. Otherwise, the value is kept and this
    /// returns `false`. This method may only be called if [`Outliner::supports_null`] returns
    /// `true`.
    fn check_null(&mut self) -> Result<bool, Self::Error>;

    /// Assuming that the top item on the stack is a value, asserts that it is an ordered list
    /// with, popping it and pushing an opened list onto the stack.
    ///
    /// If known, the number of items in the list will be returned.
    fn open_list(&mut self) -> Result<Option<usize>, Self::Error>;

    /// Assuming the top item on the stack is a list, tries getting the next item from the list.
    /// If one exists, it will be pushed onto the stack as a value and this will return `true`.
    /// Otherwise, the list will be popped from the stack and this will return `false`.
    fn next_item(&mut self) -> Result<bool, Self::Error>;

    /// Constructs an error wrapping the given source error. If errors contain position
    /// information, the error will be tagged to the most recently popped item.
    fn error(&self, source: Box<dyn std::error::Error + Send + Sync>) -> Self::Error;

    /// Constructs an error which says that the previously-read value is not a valid name in
    /// the given [`NameMap`].
    fn error_invalid_name(&self, names: &'static NameMap<usize>) -> Self::Error {
        self.error(Box::new(InvalidNameError { expected: names }))
    }

    /// Constructs an error which says that the previously-read value is not a valid index.
    fn error_invalid_index(&self, max_index: usize) -> Self::Error {
        self.error(Box::new(InvalidIndexError { max_index }))
    }

    /// Constructs an error which says more list items were expected. If errors contain position
    /// information, the error will be tagged to the most recently popped item (which should be a
    /// list).
    fn error_missing_item(&self) -> Self::Error;

    /// Assuming that there is an opened list on top of the stack, constructs an error which says
    /// there is an unexpected extra list item. If errors contain position information, the error
    /// will be tagged to the list.
    fn error_extra_item(&self) -> Self::Error;
}

/// An [`std::error::Error`] which says that a read name was expected to be in a [`NameMap`],
/// but wasn't.
#[derive(Debug)]
pub struct InvalidNameError {
    pub expected: &'static NameMap<usize>,
}

impl std::fmt::Display for InvalidNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("name is not one of the allowed options (")?;
        let mut is_first = true;
        for (name, _) in self.expected.entries() {
            if is_first {
                is_first = false;
            } else {
                f.write_str(", ")?;
            }
            write!(f, "{:?}", name)?;
        }
        f.write_str(")")
    }
}

impl std::error::Error for InvalidNameError {}

/// An [`std::error::Error`] which says that a read index was out of bounds.
#[derive(thiserror::Error, Debug)]
#[error("index is greater than the maximum allowed index, {max_index}")]
pub struct InvalidIndexError {
    pub max_index: usize,
}

/// A type which can be deserialized using a deserializer of type `D` given access to a context
/// of type `Ctx`.
pub trait Deserialize<D: Deserializer + ?Sized, Ctx: ?Sized = ()>: Sized {
    /// Indicates whether `null` has a valid representation in this type. If `true`, the
    /// `deserialize` function should check for `null` before committing to deserializing
    /// a "real" value.
    ///
    /// Wrappers over non-nullable types can use `null` as a "niche" to represent one extra value.
    const NULLABLE: bool;

    /// Deserializes a value of this type from the given [`Value`].
    fn deserialize(value: Value<D>, context: &mut Ctx) -> Result<Self, D::Error>;
}

/// A [`Deserialize`] which is deserialized as a struct value. This can be used to inline/flatten
/// the content of one struct into another (assuming that field names don't clash).
///
/// The implementation of [`Deserialize::deserialize`] should be [`deserialize_struct`].
pub trait DeserializeStruct<D: Deserializer + ?Sized, Ctx: ?Sized = ()>:
    Deserialize<D, Ctx>
{
    /// Deserializes a value of this type from the given [`Struct`].
    fn deserialize_content(st: &mut Struct<D>, context: &mut Ctx) -> Result<Self, D::Error>;
}

/// The standard implementation of [`Deserialize::deserialize`] for a [`DeserializeStruct`].
pub fn deserialize_struct<T: DeserializeStruct<D, Ctx>, D: Deserializer + ?Sized, Ctx: ?Sized>(
    value: Value<D>,
    context: &mut Ctx,
    type_name: Option<&'static str>,
) -> Result<T, D::Error> {
    let mut st = value.into_struct(type_name)?;
    let res = T::deserialize_content(&mut st, context)?;
    st.close()?;
    Ok(res)
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for bool {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.get_bool()
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for i8 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.get_i8()
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for i16 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.get_i16()
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for i32 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.get_i32()
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for i64 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.get_i64()
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for u8 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.get_u8()
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for u16 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.get_u16()
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for u32 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.get_u32()
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for u64 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.get_u64()
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for f32 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.get_f32()
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for f64 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.get_f64()
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for char {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.get_char()
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for String {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.get_str().map(Cow::into_owned)
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for () {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, context: &mut Ctx) -> Result<Self, D::Error> {
        deserialize_struct(value, context, None)
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> DeserializeStruct<D, Ctx> for () {
    fn deserialize_content(_: &mut Struct<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        Ok(())
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized, T: Deserialize<D, Ctx>> Deserialize<D, Ctx>
    for Option<T>
{
    const NULLABLE: bool = true;
    fn deserialize(value: Value<D>, context: &mut Ctx) -> Result<Self, D::Error> {
        if !T::NULLABLE && value.as_raw().supports_null() {
            let (d, done) = value.into_raw();
            if d.check_null()? {
                *done = true;
                Ok(None)
            } else {
                T::deserialize(Value::new(d, done), context).map(Some)
            }
        } else {
            // Fallback to using a regular struct
            let mut st = value.into_struct(Some("Option"))?;
            let has_value = st.field("has_value")?.get_bool()?;
            let res = if has_value {
                Some(T::deserialize(st.field("value")?, context)?)
            } else {
                None
            };
            st.close()?;
            Ok(res)
        }
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized, T0: Deserialize<D, Ctx>, T1: Deserialize<D, Ctx>>
    Deserialize<D, Ctx> for (T0, T1)
{
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, context: &mut Ctx) -> Result<Self, D::Error> {
        let mut tuple = value.into_tuple(None)?;
        let res = (
            tuple.element()?.get_using(context)?,
            tuple.element()?.get_using(context)?,
        );
        tuple.close()?;
        Ok(res)
    }
}

impl<
        D: Deserializer + ?Sized,
        Ctx: ?Sized,
        T0: Deserialize<D, Ctx>,
        T1: Deserialize<D, Ctx>,
        T2: Deserialize<D, Ctx>,
    > Deserialize<D, Ctx> for (T0, T1, T2)
{
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, context: &mut Ctx) -> Result<Self, D::Error> {
        let mut tuple = value.into_tuple(None)?;
        let res = (
            tuple.element()?.get_using(context)?,
            tuple.element()?.get_using(context)?,
            tuple.element()?.get_using(context)?,
        );
        tuple.close()?;
        Ok(res)
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized, T: Deserialize<D, Ctx>, const N: usize>
    Deserialize<D, Ctx> for [T; N]
{
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, context: &mut Ctx) -> Result<Self, D::Error> {
        let mut tuple = value.into_tuple(None)?;
        // TODO: Use `try_from_fn` when available:
        // https://github.com/rust-lang/rust/issues/89379
        let mut res: [Option<T>; N] = core::array::from_fn(|_| None);
        for el in res.iter_mut() {
            *el = Some(tuple.element()?.get_using(context)?);
        }
        tuple.close()?;
        Ok(res.map(|x| x.unwrap()))
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized, T: Deserialize<D, Ctx>> Deserialize<D, Ctx> for Vec<T> {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, context: &mut Ctx) -> Result<Self, D::Error> {
        let mut res = Vec::new();
        let mut list = value.into_list()?;
        while let Some(item) = list.next()? {
            res.push(item.get_using(context)?);
        }
        Ok(res)
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for std::num::NonZeroI8 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.validate_with(|value| {
            Ok(value.get_i8()?.try_into())
        })
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for std::num::NonZeroI16 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.validate_with(|value| {
            Ok(value.get_i16()?.try_into())
        })
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for std::num::NonZeroI32 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.validate_with(|value| {
            Ok(value.get_i32()?.try_into())
        })
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for std::num::NonZeroI64 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.validate_with(|value| {
            Ok(value.get_i64()?.try_into())
        })
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for std::num::NonZeroU8 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.validate_with(|value| {
            Ok(value.get_u8()?.try_into())
        })
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for std::num::NonZeroU16 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.validate_with(|value| {
            Ok(value.get_u16()?.try_into())
        })
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for std::num::NonZeroU32 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.validate_with(|value| {
            Ok(value.get_u32()?.try_into())
        })
    }
}

impl<D: Deserializer + ?Sized, Ctx: ?Sized> Deserialize<D, Ctx> for std::num::NonZeroU64 {
    const NULLABLE: bool = false;
    fn deserialize(value: Value<D>, _: &mut Ctx) -> Result<Self, D::Error> {
        value.validate_with(|value| {
            Ok(value.get_u64()?.try_into())
        })
    }
}