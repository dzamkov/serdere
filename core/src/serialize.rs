#[allow(unused_imports)]
use crate::Deserializer;
use crate::{Outliner, Struct, Value};

/// An interface for writing arbitrarily-complex data to a data source. This uses a stack-based
/// API, described in [`Outliner`].
pub trait Serializer: Outliner {
    /// Assuming that the top item on the stack is a value, assigns it to the given [`bool`] and
    /// pops it.
    fn put_bool(&mut self, value: bool) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is a value, assigns it to the given [`i8`] and
    /// pops it.
    fn put_i8(&mut self, value: i8) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is a value, assigns it to the given [`i16`] and
    /// pops it.
    fn put_i16(&mut self, value: i16) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is a value, assigns it to the given [`i32`] and
    /// pops it.
    fn put_i32(&mut self, value: i32) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is a value, assigns it to the given [`i64`] and
    /// pops it.
    fn put_i64(&mut self, value: i64) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is a value, assigns it to the given [`u8`] and
    /// pops it.
    fn put_u8(&mut self, value: u8) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is a value, assigns it to the given [`u16`] and
    /// pops it.
    fn put_u16(&mut self, value: u16) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is a value, assigns it to the given [`u32`] and
    /// pops it.
    fn put_u32(&mut self, value: u32) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is a value, assigns it to the given [`u64`] and
    /// pops it.
    fn put_u64(&mut self, value: u64) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is a value, assigns it to the given [`f32`] and
    /// pops it.
    fn put_f32(&mut self, value: f32) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is a value, assigns it to the given [`f64`] and
    /// pops it.
    fn put_f64(&mut self, value: f64) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is a value, assigns it to the given [`char`] and
    /// pops it.
    fn put_char(&mut self, value: char) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is an opened string, appends the given character
    /// to it.
    fn append_char(&mut self, value: char) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is an opened string, appends the given string
    /// to it.
    fn append_str(&mut self, value: &str) -> Result<(), Self::Error> {
        for char in value.chars() {
            self.append_char(char)?
        }
        Ok(())
    }

    /// Assuming that the top item on the stack is a value, assigns it to the given string
    /// and pops it.
    fn put_str(&mut self, value: &str) -> Result<(), Self::Error> {
        self.open_str()?;
        self.append_str(value)?;
        self.close_str()
    }

    /// Assuming that the top item on the stack is a value, assigns it to an enum "tag". Depending
    /// on the underlying serialization format, this can be written as a string or an integer
    /// index.
    fn put_tag(
        &mut self,
        max_index: usize,
        index: usize,
        name: Option<&'static str>,
    ) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is a value, asserts that it is an ordered list
    /// with the given number of items, popping it and pushing an opened list onto the stack.
    fn open_list_sized(&mut self, len: usize) -> Result<(), Self::Error>;
}

/// A type which can be serialized using a seserializer of type `S` given access to a context
/// of type `Ctx`.
pub trait Serialize<S: Serializer + ?Sized, Ctx: ?Sized = ()> {
    /// Indicates whether `null` has a valid representation in this type. If `true`, the
    /// `serialize` function may write a `null` value.
    ///
    /// Wrappers over non-nullable types can use `null` as a "niche" to represent one extra value.
    const NULLABLE: bool;

    /// Writes a value of this type to the given [`Value`].
    fn serialize(&self, value: Value<S>, context: &mut Ctx) -> Result<(), S::Error>;
}

/// A [`Serialize`] which is serialized as a struct value. This can be used to inline/flatten
/// the content of one struct into another (assuming that field names don't clash).
pub trait SerializeStruct<S: Serializer + ?Sized, Ctx: ?Sized = ()>: Serialize<S, Ctx> {
    /// Writes a value of this type to the given [`Struct`].
    fn serialize_content(&self, st: &mut Struct<S>, context: &mut Ctx) -> Result<(), S::Error>;
}

/// The standard implementation of [`Serialize::serialize`] for a [`SerializeStruct`].
pub fn serialize_struct<
    T: SerializeStruct<S, Ctx> + ?Sized,
    S: Serializer + ?Sized,
    Ctx: ?Sized,
>(
    target: Value<S>,
    value: &T,
    context: &mut Ctx,
    type_name: Option<&'static str>,
) -> Result<(), S::Error> {
    let mut st = target.into_struct(type_name)?;
    value.serialize_content(&mut st, context)?;
    st.close()
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for bool {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_bool(*self)
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for i8 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_i8(*self)
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for i16 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_i16(*self)
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for i32 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_i32(*self)
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for i64 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_i64(*self)
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for u8 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_u8(*self)
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for u16 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_u16(*self)
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for u32 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_u32(*self)
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for u64 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_u64(*self)
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for f32 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_f32(*self)
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for f64 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_f64(*self)
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for char {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_char(*self)
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for str {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_str(self)
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for String {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_str(self)
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for () {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, context: &mut Ctx) -> Result<(), S::Error> {
        serialize_struct(value, &(), context, None)
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> SerializeStruct<S, Ctx> for () {
    fn serialize_content(&self, _: &mut Struct<S>, _: &mut Ctx) -> Result<(), S::Error> {
        Ok(())
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized, T: Serialize<S, Ctx>> Serialize<S, Ctx> for Option<T> {
    const NULLABLE: bool = true;
    fn serialize(&self, value: Value<S>, context: &mut Ctx) -> Result<(), S::Error> {
        if !T::NULLABLE && value.as_raw().supports_null() {
            if let Some(inner) = self {
                inner.serialize(value, context)
            } else {
                let (s, done) = value.into_raw();
                s.pop_null()?;
                *done = true;
                Ok(())
            }
        } else {
            // Fallback to using a regular struct
            let mut st = value.into_struct(Some("Option"))?;
            let has_value = st.field("has_value")?;
            if let Some(inner) = self {
                has_value.put_bool(true)?;
                inner.serialize(st.field("value")?, context)?;
            } else {
                has_value.put_bool(false)?;
            }
            st.close()
        }
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized, T0: Serialize<S, Ctx>, T1: Serialize<S, Ctx>>
    Serialize<S, Ctx> for (T0, T1)
{
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, context: &mut Ctx) -> Result<(), S::Error> {
        let mut tuple = value.into_tuple(None)?;
        tuple.element()?.put_using(&self.0, context)?;
        tuple.element()?.put_using(&self.1, context)?;
        tuple.close()
    }
}

impl<
        S: Serializer + ?Sized,
        Ctx: ?Sized,
        T0: Serialize<S, Ctx>,
        T1: Serialize<S, Ctx>,
        T2: Serialize<S, Ctx>,
    > Serialize<S, Ctx> for (T0, T1, T2)
{
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, context: &mut Ctx) -> Result<(), S::Error> {
        let mut tuple = value.into_tuple(None)?;
        tuple.element()?.put_using(&self.0, context)?;
        tuple.element()?.put_using(&self.1, context)?;
        tuple.element()?.put_using(&self.2, context)?;
        tuple.close()
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized, T: Serialize<S, Ctx>, const N: usize> Serialize<S, Ctx>
    for [T; N]
{
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, context: &mut Ctx) -> Result<(), S::Error> {
        let mut tuple = value.into_tuple(None)?;
        for item in self.iter() {
            tuple.element()?.put_using(item, context)?;
        }
        tuple.close()
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized, T: Serialize<S, Ctx>> Serialize<S, Ctx> for Vec<T> {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, context: &mut Ctx) -> Result<(), S::Error> {
        let mut list = value.into_list_sized(self.len())?;
        for item in self.iter() {
            list.push()?.put_using(item, context)?;
        }
        list.close()
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for std::num::NonZeroI8 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_i8((*self).into())
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for std::num::NonZeroI16 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_i16((*self).into())
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for std::num::NonZeroI32 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_i32((*self).into())
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for std::num::NonZeroI64 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_i64((*self).into())
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for std::num::NonZeroU8 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_u8((*self).into())
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for std::num::NonZeroU16 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_u16((*self).into())
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for std::num::NonZeroU32 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_u32((*self).into())
    }
}

impl<S: Serializer + ?Sized, Ctx: ?Sized> Serialize<S, Ctx> for std::num::NonZeroU64 {
    const NULLABLE: bool = false;
    fn serialize(&self, value: Value<S>, _: &mut Ctx) -> Result<(), S::Error> {
        value.put_u64((*self).into())
    }
}