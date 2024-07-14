use crate::deserialize::{Deserialize, DeserializeStruct, Deserializer};
use crate::serialize::{Serialize, SerializeStruct, Serializer};
use crate::{NameMap, Outliner};
use std::borrow::Cow;

/// A wrapper over an [`Outliner`] which has a value at the top of its stack.
/// 
/// This is a "helper" wrapper intended to provide a convenient interface and enforce correct usage
/// of the API.
#[must_use]
pub struct Value<'a, O: Outliner + ?Sized> {
    source: &'a mut O,
    done_flag: &'a mut bool,
}

impl<'a, O: Outliner + ?Sized> Value<'a, O> {
    /// Constructs a new [`Value`] wrapper over the given [`Outliner`], asserting that it has an
    /// value on top of its stack. `done_flag` will be set to true when the value is popped.
    pub fn new(source: &'a mut O, done_flag: &'a mut bool) -> Self {
        Self { source, done_flag }
    }

    /// Constructs a temporary [`Value`] wrapper over the given [`Outliner`], asserting that it
    /// has a value on top of its stack.
    pub fn with<R>(
        source: &mut O,
        f: impl FnOnce(Value<O>) -> Result<R, O::Error>,
    ) -> Result<R, O::Error> {
        let mut done = false;
        let res = f(Value::new(source, &mut done))?;
        assert!(done, "{}", INVALID_STATE_ERROR);
        Ok(res)
    }

    /// Gets an immutable reference to the underlying [`Outliner`] for this  [`Value`].
    pub fn as_raw(&self) -> &O {
        self.source
    }

    /// Gets the underlying [`Outliner`] and `done_flag` for this [`Value`]. It is the caller's
    /// responsibility to uphold the invariants normally kept by the [`Value`]: `done_flag`
    /// should be set to `true` when the value at the top of the stack has been popped, and no
    /// lower stack items should be affected.
    pub fn into_raw(self) -> (&'a mut O, &'a mut bool) {
        (self.source, self.done_flag)
    }

    /// Asserts that this value is a struct.
    pub fn into_struct(self, type_name: Option<&'static str>) -> Result<Struct<'a, O>, O::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.open_struct(type_name)?;
        Ok(Struct::new(self.source, self.done_flag))
    }

    /// Asserts that this value is a tuple.
    pub fn into_tuple(self, type_name: Option<&'static str>) -> Result<Tuple<'a, O>, O::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.open_tuple(type_name)?;
        Ok(Tuple::new(self.source, self.done_flag))
    }
}

impl<'a, S: Serializer + ?Sized> Value<'a, S> {
    /// Assigns this to the given concrete value.
    pub fn put<T: Serialize<S>>(self, value: &T) -> Result<(), S::Error> {
        T::serialize(value, self, &mut ())
    }

    /// Assigns this to the given concrete value.
    pub fn put_using<T: Serialize<S, Ctx>, Ctx: ?Sized>(
        self,
        value: &T,
        context: &mut Ctx,
    ) -> Result<(), S::Error> {
        T::serialize(value, self, context)
    }

    /// Assigns this value to the given [`bool`].
    pub fn put_bool(self, value: bool) -> Result<(), S::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.put_bool(value)?;
        *self.done_flag = true;
        Ok(())
    }

    /// Assigns this value to the given [`i8`].
    pub fn put_i8(self, value: i8) -> Result<(), S::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.put_i8(value)?;
        *self.done_flag = true;
        Ok(())
    }

    /// Assigns this value to the given [`i16`].
    pub fn put_i16(self, value: i16) -> Result<(), S::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.put_i16(value)?;
        *self.done_flag = true;
        Ok(())
    }

    /// Assigns this value to the given [`i32`].
    pub fn put_i32(self, value: i32) -> Result<(), S::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.put_i32(value)?;
        *self.done_flag = true;
        Ok(())
    }

    /// Assigns this value to the given [`i64`].
    pub fn put_i64(self, value: i64) -> Result<(), S::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.put_i64(value)?;
        *self.done_flag = true;
        Ok(())
    }

    /// Assigns this value to the given [`u8`].
    pub fn put_u8(self, value: u8) -> Result<(), S::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.put_u8(value)?;
        *self.done_flag = true;
        Ok(())
    }

    /// Assigns this value to the given [`u16`].
    pub fn put_u16(self, value: u16) -> Result<(), S::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.put_u16(value)?;
        *self.done_flag = true;
        Ok(())
    }

    /// Assigns this value to the given [`u32`].
    pub fn put_u32(self, value: u32) -> Result<(), S::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.put_u32(value)?;
        *self.done_flag = true;
        Ok(())
    }

    /// Assigns this value to the given [`u64`].
    pub fn put_u64(self, value: u64) -> Result<(), S::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.put_u64(value)?;
        *self.done_flag = true;
        Ok(())
    }

    /// Assigns this value to the given [`f32`].
    pub fn put_f32(self, value: f32) -> Result<(), S::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.put_f32(value)?;
        *self.done_flag = true;
        Ok(())
    }

    /// Assigns this value to the given [`f64`].
    pub fn put_f64(self, value: f64) -> Result<(), S::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.put_f64(value)?;
        *self.done_flag = true;
        Ok(())
    }

    /// Assigns this value to the given [`char`].
    pub fn put_char(self, value: char) -> Result<(), S::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.put_char(value)?;
        *self.done_flag = true;
        Ok(())
    }

    /// Assigns this value to the given [`str`].
    pub fn put_str(self, value: &str) -> Result<(), S::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.put_str(value)?;
        *self.done_flag = true;
        Ok(())
    }

    /// Assigns this value to an enum tag.
    pub fn put_tag(
        self,
        max_index: usize,
        index: usize,
        name: Option<&'static str>,
    ) -> Result<(), S::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.put_tag(max_index, index, name)?;
        *self.done_flag = true;
        Ok(())
    }

    /// Asserts that this value is a list with the given number of items.
    pub fn into_list_sized(self, len: usize) -> Result<List<'a, S>, S::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        self.source.open_list_sized(len)?;
        Ok(List::new(self.source, self.done_flag, Some(len)))
    }
}

impl<'a, D: Deserializer + ?Sized> Value<'a, D> {
    /// Interprets this value as the given type.
    pub fn get<T: Deserialize<D>>(self) -> Result<T, D::Error> {
        T::deserialize(self, &mut ())
    }

    /// Interprets this value as the given type.
    pub fn get_using<T: Deserialize<D, Ctx>, Ctx: ?Sized>(
        self,
        context: &mut Ctx,
    ) -> Result<T, D::Error> {
        T::deserialize(self, context)
    }

    /// Checks whether this value is `null`, a format-dependent literal representing either a
    /// default, or the absence of a "real" value. If this returns `true`, the value is consumed
    /// and should not be used again.
    pub fn check_null(&mut self) -> Result<bool, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        Ok(
            if self.source.supports_null() && self.source.check_null()? {
                *self.done_flag = true;
                true
            } else {
                false
            },
        )
    }

    /// Interprets this value as a [`bool`].
    pub fn get_bool(self) -> Result<bool, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        let res = self.source.get_bool()?;
        *self.done_flag = true;
        Ok(res)
    }

    /// Interprets this value as an [`i8`].
    pub fn get_i8(self) -> Result<i8, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        let res = self.source.get_i8()?;
        *self.done_flag = true;
        Ok(res)
    }

    /// Interprets this value as an [`i16`].
    pub fn get_i16(self) -> Result<i16, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        let res = self.source.get_i16()?;
        *self.done_flag = true;
        Ok(res)
    }

    /// Interprets this value as an [`i32`].
    pub fn get_i32(self) -> Result<i32, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        let res = self.source.get_i32()?;
        *self.done_flag = true;
        Ok(res)
    }

    /// Interprets this value as an [`i64`].
    pub fn get_i64(self) -> Result<i64, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        let res = self.source.get_i64()?;
        *self.done_flag = true;
        Ok(res)
    }

    /// Interprets this value as a [`u8`].
    pub fn get_u8(self) -> Result<u8, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        let res = self.source.get_u8()?;
        *self.done_flag = true;
        Ok(res)
    }

    /// Interprets this value as a [`u16`].
    pub fn get_u16(self) -> Result<u16, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        let res = self.source.get_u16()?;
        *self.done_flag = true;
        Ok(res)
    }

    /// Interprets this value as a [`u32`].
    pub fn get_u32(self) -> Result<u32, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        let res = self.source.get_u32()?;
        *self.done_flag = true;
        Ok(res)
    }

    /// Interprets this value as a [`u64`].
    pub fn get_u64(self) -> Result<u64, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        let res = self.source.get_u64()?;
        *self.done_flag = true;
        Ok(res)
    }

    /// Interprets this value as a [`f32`].
    pub fn get_f32(self) -> Result<f32, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        let res = self.source.get_f32()?;
        *self.done_flag = true;
        Ok(res)
    }

    /// Interprets this value as a [`f64`].
    pub fn get_f64(self) -> Result<f64, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        let res = self.source.get_f64()?;
        *self.done_flag = true;
        Ok(res)
    }

    /// Interprets this value as a [`char`].
    pub fn get_char(self) -> Result<char, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        let res = self.source.get_char()?;
        *self.done_flag = true;
        Ok(res)
    }

    /// Interprets this value as a string.
    pub fn get_str(self) -> Result<Cow<'a, str>, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        let res = self.source.read_str()?;
        *self.done_flag = true;
        Ok(res)
    }

    /// Interprets this value as an enum tag. The names of the possible tags (or a subset of them)
    /// are provided by a given [`NameMap`]. Depending on the underlying serialization format, this
    /// may accept a string, an integer index, or both.
    pub fn get_tag(
        self,
        max_index: usize,
        names: &'static NameMap<usize>,
    ) -> Result<usize, D::Error> {
        let res = self.source.get_tag(max_index, names)?;
        *self.done_flag = true;
        Ok(res)
    }

    /// Asserts that this value is a list.
    pub fn into_list(self) -> Result<List<'a, D>, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        let len = self.source.open_list()?;
        Ok(List::new(self.source, self.done_flag, len))
    }

    /// Uses the given closure to deserialize from this [`Value`], allowing a custom error to
    /// be returned and encoded as a `D::Error`. This is typically used when validation is
    /// performed on the deserialized data. If deserializer errors contain position information,
    /// the custom error will be tagged with this value's location.
    pub fn validate_with<R, E: std::error::Error + Send + Sync + 'static>(
        self,
        f: impl FnOnce(Value<D>) -> Result<Result<R, E>, D::Error>,
    ) -> Result<R, D::Error> {
        assert!(!*self.done_flag, "{}", INVALID_STATE_ERROR);
        let res = f(Value::new(self.source, self.done_flag))?;
        assert!(*self.done_flag, "{}", INVALID_STATE_ERROR);
        match res {
            Ok(res) => Ok(res),
            Err(err) => Err(self.source.error(Box::new(err))),
        }
    }
}

/// The error message for a panic that occurs due to improper use of a wrapper.
pub const INVALID_STATE_ERROR: &str = "wrapper is in an invalid state";

/// A wrapper over an [`Outliner`] which has an opened struct at the top of its stack.
/// 
/// This is a "helper" wrapper intended to provide a convenient interface and enforce correct usage
/// of the API.
#[must_use]
pub struct Struct<'a, O: Outliner + ?Sized> {
    source: &'a mut O,
    done_flag: &'a mut bool,
    ready_flag: bool,
}

impl<'a, O: Outliner + ?Sized> Struct<'a, O> {
    /// Constructs a new [`Struct`] wrapper over the given [`Outliner`], asserting that it has an
    /// opened struct at the top of its stack. `done_flag` will be set to true when the struct is
    /// popped.
    pub fn new(source: &'a mut O, done_flag: &'a mut bool) -> Self {
        Self {
            source,
            done_flag,
            ready_flag: true,
        }
    }

    /// Constructs a temporary [`Struct`] wrapper over the given [`Outliner`], asserting that it
    /// has an opened struct on top of its stack.
    pub fn with<R>(
        source: &mut O,
        f: impl FnOnce(Struct<O>) -> Result<R, O::Error>,
    ) -> Result<R, O::Error> {
        let mut done = false;
        let res = f(Struct::new(source, &mut done))?;
        assert!(!done, "{}", INVALID_STATE_ERROR);
        Ok(res)
    }

    /// Gets the value of a named field in the struct. Note that fields must be accessed in the
    /// order they are defined.
    pub fn field(&mut self, name: &'static str) -> Result<Value<O>, O::Error> {
        assert!(self.ready_flag, "{}", INVALID_STATE_ERROR);
        self.ready_flag = false;
        self.source.push_field(name)?;
        Ok(Value::new(self.source, &mut self.ready_flag))
    }

    /// Asserts that there are no more fields in the struct and closes it.
    pub fn close(self) -> Result<(), O::Error> {
        assert!(self.ready_flag, "{}", INVALID_STATE_ERROR);
        self.source.close_struct()?;
        *self.done_flag = true;
        Ok(())
    }
}

impl<'a, D: Deserializer + ?Sized> Struct<'a, D> {
    /// Gets the value for inlined/flattened struct within this struct.
    pub fn inline_get<T: DeserializeStruct<D>>(&mut self) -> Result<T, D::Error> {
        T::deserialize_content(self, &mut ())
    }

    /// Gets the value for inlined/flattened struct within this struct.
    pub fn inline_get_using<T: DeserializeStruct<D, Ctx>, Ctx: ?Sized>(
        &mut self,
        context: &mut Ctx,
    ) -> Result<T, D::Error> {
        T::deserialize_content(self, context)
    }
}

impl<'a, S: Serializer + ?Sized> Struct<'a, S> {
    /// Gets the value for inlined/flattened struct within this struct.
    pub fn inline_put<T: SerializeStruct<S>>(&mut self, value: &T) -> Result<(), S::Error> {
        T::serialize_content(value, self, &mut ())
    }

    /// Gets the value for inlined/flattened struct within this struct.
    pub fn inline_put_using<T: SerializeStruct<S, Ctx>, Ctx: ?Sized>(
        &mut self,
        value: &T,
        context: &mut Ctx,
    ) -> Result<(), S::Error> {
        T::serialize_content(value, self, context)
    }
}

/// A wrapper over an [`Outliner`] which has an opened tuple at the top of its stack.
/// 
/// This is a "helper" wrapper intended to provide a convenient interface and enforce correct usage
/// of the API.
#[must_use]
pub struct Tuple<'a, O: Outliner + ?Sized> {
    source: &'a mut O,
    done_flag: &'a mut bool,
    ready_flag: bool,
}

impl<'a, O: Outliner + ?Sized> Tuple<'a, O> {
    /// Constructs a new [`Tuple`] wrapper over the given [`Outliner`], asserting that it has an
    /// opened tuple at the top of its stack. `done_flag` will be set to true when the tuple is
    /// popped.
    pub fn new(source: &'a mut O, done_flag: &'a mut bool) -> Self {
        Self {
            source,
            done_flag,
            ready_flag: true,
        }
    }

    /// Constructs a temporary [`Tuple`] wrapper over the given [`Outliner`], asserting that it
    /// has an opened tuple on top of its stack.
    pub fn with<R>(
        source: &mut O,
        f: impl FnOnce(Tuple<O>) -> Result<R, O::Error>,
    ) -> Result<R, O::Error> {
        let mut done = false;
        let res = f(Tuple::new(source, &mut done))?;
        assert!(!done, "{}", INVALID_STATE_ERROR);
        Ok(res)
    }

    /// Gets the [`Value`] for the next element in the tuple, asserting that one exists.
    pub fn element(&mut self) -> Result<Value<O>, O::Error> {
        assert!(self.ready_flag, "{}", INVALID_STATE_ERROR);
        self.ready_flag = false;
        self.source.push_element()?;
        Ok(Value::new(self.source, &mut self.ready_flag))
    }

    /// Asserts that there are no more elements in the tuple and closes it.
    pub fn close(self) -> Result<(), O::Error> {
        assert!(self.ready_flag, "{}", INVALID_STATE_ERROR);
        self.source.close_tuple()?;
        *self.done_flag = true;
        Ok(())
    }
}

/// The error message for a panic that occurs due to an attempt to push an item to a list when
/// its remaining length is zero.
pub const LIST_OVERFLOW_ERROR: &str = "list has/expects no more items";

/// The error message for a panic that occurs due to an attempt to close a list before all items
/// have been read or written.
pub const LIST_UNDERFLOW_ERROR: &str = "list has/expects more items and may not be closed yet";

/// A wrapper over an [`Outliner`] which has an opened list at the top of its stack.
/// 
/// This is a "helper" wrapper intended to provide a convenient interface and enforce correct usage
/// of the API.
#[must_use]
pub struct List<'a, O: Outliner + ?Sized> {
    source: &'a mut O,
    done_flag: &'a mut bool,
    ready_flag: bool,
    is_len_known: bool,
    rem_len: usize,
}

impl<'a, O: Outliner + ?Sized> List<'a, O> {
    /// Constructs a new [`List`] wrapper over the given [`Outliner`], asserting that it
    /// has an opened list at the top of its stack. `done_flag` will be set to
    /// true when the list is popped.
    pub fn new(source: &'a mut O, done_flag: &'a mut bool, rem_len: Option<usize>) -> Self {
        Self {
            source,
            done_flag,
            ready_flag: true,
            is_len_known: rem_len.is_some(),
            rem_len: rem_len.unwrap_or(0),
        }
    }

    /// Gets the number of items remaining in the list, if known.
    pub fn rem_len(&self) -> Option<usize> {
        if self.is_len_known {
            Some(self.rem_len)
        } else {
            None
        }
    }

    /// Asserts that there is another item in the list, returning its value.
    pub fn push(&mut self) -> Result<Value<O>, O::Error> {
        assert!(!self.is_len_known || self.rem_len > 0, "{}", LIST_OVERFLOW_ERROR);
        assert!(self.ready_flag, "{}", INVALID_STATE_ERROR);
        self.ready_flag = false;
        self.rem_len = self.rem_len.wrapping_sub(1);
        self.source.push_item()?;
        Ok(Value::new(self.source, &mut self.ready_flag))
    }

    /// Asserts that there are no more items in the list and closes it.
    pub fn close(self) -> Result<(), O::Error> {
        assert!(self.ready_flag, "{}", INVALID_STATE_ERROR);
        assert!(!self.is_len_known || self.rem_len == 0, "{}", LIST_UNDERFLOW_ERROR);
        self.source.close_list()?;
        *self.done_flag = true;
        Ok(())
    }
}

impl<'a, D: Deserializer + ?Sized> List<'a, D> {
    /// Attempts to get the next value from the list, returning `Ok(None)` if the end of the list
    /// has been reached.
    #[allow(clippy::should_implement_trait)] // Requires lending iterator
    pub fn next(&mut self) -> Result<Option<Value<D>>, D::Error> {
        assert!(self.ready_flag, "{}", INVALID_STATE_ERROR);
        self.ready_flag = false;
        Ok(if self.source.next_item()? {
            Some(Value::new(self.source, &mut self.ready_flag))
        } else {
            *self.done_flag = true;
            None
        })
    }
}
