use super::{JsonDeserializer, ValueType, JsonOutliner, JsonSerializer};
use crate::deserialize::DeserializeStruct;
use crate::{List, Struct, Value, INVALID_STATE_ERROR};
use std::borrow::Cow;


/// Contains JSON-specific extension methods for [`Value`].
pub trait ValueExt<'a, O: JsonOutliner + ?Sized> {
    /// Asserts that this value is the `null` literal.
    fn into_null(self) -> Result<(), O::Error>;

    /// Asserts that this value is a JSON [`Object`].
    fn into_object(self) -> Result<Object<'a, O>, O::Error>;
}

impl<'a, O: JsonOutliner + ?Sized> ValueExt<'a, O> for Value<'a, O> {
    fn into_null(self) -> Result<(), O::Error> {
        let (source, done_flag) = self.into_raw();
        source.pop_null()?;
        *done_flag = true;
        Ok(())
    }

    fn into_object(self) -> Result<Object<'a, O>, O::Error> {
        let (source, done_flag) = self.into_raw();
        source.open_object()?;
        Ok(Object::new(source, done_flag))
    }
}

/// Contains extension methods for [`Value`] specific to [`JsonSerializer`]s.
pub trait ValueSerialierExt<'a, S: JsonSerializer + ?Sized> {
    /// Asserts that this value is a list of unspecified length.
    fn into_list_streaming(self) -> Result<List<'a, S>, S::Error>;
}

impl<'a, S: JsonSerializer + ?Sized> ValueSerialierExt<'a, S> for Value<'a, S> {
    fn into_list_streaming(self) -> Result<List<'a, S>, S::Error> {
        let (source, done_flag) = self.into_raw();
        source.open_list_streaming()?;
        Ok(List::new(source, done_flag, None))
    }
}

/// Contains extension methods for [`Value`] specific to [`JsonDeserializer`]s.
pub trait ValueDeserializerExt<'a, D: JsonDeserializer + ?Sized> {
    /// Gets the JSON [`ValueType`] of this value.
    fn ty(&self) -> ValueType;

    /// Consumes this value without deserializing it.
    fn skip(self) -> Result<(), D::Error>;

    /// Asserts that this value is a collection (object or array).
    fn into_collection(self) -> Result<Collection<'a, D>, D::Error>;
}

impl<'a, D: JsonDeserializer + ?Sized> ValueDeserializerExt<'a, D> for Value<'a, D> {
    fn ty(&self) -> ValueType {
        self.as_raw().peek_value_type()
    }

    fn skip(self) -> Result<(), D::Error> {
        let (source, done_flag) = self.into_raw();
        source.skip_value()?;
        *done_flag = true;
        Ok(())
    }

    fn into_collection(self) -> Result<Collection<'a, D>, D::Error> {
        let (source, done_flag) = self.into_raw();
        if source.peek_value_type() == ValueType::Object {
            source.open_object()?;
            Ok(Object::new(source, done_flag).into())
        } else {
            let len = source.open_list()?;
            Ok(List::new(source, done_flag, len).into())
        }
    }
}

/// A wrapper over a [`JsonOutliner`] which has an object at the top of its stack.
/// 
/// This is a "helper" intended to provide a convenient interface and enforce correct usage of the
/// API.
#[must_use]
pub struct Object<'a, O: JsonOutliner + ?Sized> {
    source: &'a mut O,
    done_flag: &'a mut bool,
    ready_flag: bool,
}

impl<'a, O: JsonOutliner + ?Sized> Object<'a, O> {
    /// Constructs a new [`Object`] wrapper over the given [`JsonOutliner`], asserting that it has
    /// an opened object at the top of its stack. `done_flag` will be set to true when the object
    /// is closed.
    pub fn new(source: &'a mut O, done_flag: &'a mut bool) -> Self {
        Self {
            source,
            done_flag,
            ready_flag: true,
        }
    }

    /// Asserts that the object has an entry with the specified key, returning its value.
    pub fn entry(&mut self, key: &str) -> Result<Value<O>, O::Error> {
        assert!(self.ready_flag, "{}", INVALID_STATE_ERROR);
        self.ready_flag = false;
        self.source.push_entry(key)?;
        Ok(Value::new(self.source, &mut self.ready_flag))
    }

    /// Asserts that there are no more entries in the object and closes it.
    pub fn close(self) -> Result<(), O::Error> {
        assert!(self.ready_flag, "{}", INVALID_STATE_ERROR);
        self.source.close_object()?;
        *self.done_flag = true;
        Ok(())
    }
}

impl<'a, D: JsonDeserializer + ?Sized> Object<'a, D> {
    /// Attempts to get the value for a particular entry in the object, returning `Ok(None)` if
    /// there isn't an entry with the specified key.
    pub fn try_entry(&mut self, key: &str) -> Result<Option<Value<D>>, D::Error> {
        assert!(self.ready_flag, "{}", INVALID_STATE_ERROR);
        self.ready_flag = false;
        if self.source.try_push_entry(key)? {
            Ok(Some(Value::new(self.source, &mut self.ready_flag)))
        } else {
            self.ready_flag = true;
            Ok(None)
        }
    }

    /// Attempts to get the next unread entry for the object, returning `Ok(None)` if there are
    /// no entries left. In that case, the object will be automatically closed and should not
    /// be used again.
    pub fn next_entry(&mut self) -> Result<Option<Entry<D>>, D::Error> {
        assert!(self.ready_flag, "{}", INVALID_STATE_ERROR);
        self.ready_flag = false;
        if self.source.next_entry()? {
            Ok(Some(Entry::new(self.source, &mut self.ready_flag)))
        } else {
            *self.done_flag = true;
            Ok(None)
        }
    }

    /// Gets the value for inlined/flattened struct within this object.
    pub fn inline<T: DeserializeStruct<D>>(&mut self) -> Result<T, D::Error> {
        self.inline_using(&mut ())
    }

    /// Gets the value for inlined/flattened struct within this object.
    pub fn inline_using<T: DeserializeStruct<D, Ctx>, Ctx: ?Sized>(
        &mut self,
        context: &mut Ctx,
    ) -> Result<T, D::Error> {
        assert!(self.ready_flag, "{}", INVALID_STATE_ERROR);
        let mut done = false;
        let mut st = Struct::new(self.source, &mut done);
        // TODO: Verify `st.ready_flag`, somehow
        T::deserialize_content(&mut st, context)
    }
}

/// A wrapper over a [`JsonDeserializer`] which allows the user to deserialize an [`Object`] entry
/// (consisting of a key and a value).
/// 
/// This is a "helper" wrapper intended to provide a convenientninterface and enforce correct
/// usage of the deserializer.
#[must_use]
pub struct Entry<'a, D: JsonDeserializer + ?Sized> {
    source: &'a mut D,
    done_flag: &'a mut bool,
    key_read_flag: bool,
}

impl<'a, D: JsonDeserializer + ?Sized> Entry<'a, D> {
    /// Constructs a new [`Object`] wrapper over the given [`JsonDeserializer`], asserting that it
    /// has an opened string and value at the top of its deserialization stack, in that order.
    /// `done_flag` will be set to true when the entry is fully and successfully read.
    pub fn new(source: &'a mut D, done_flag: &'a mut bool) -> Self {
        Self {
            source,
            done_flag,
            key_read_flag: false,
        }
    }

    /// Gets the key string for this [`Entry`]. The key may only be retrieved from the entry once.
    pub fn key(&mut self) -> Result<Cow<str>, D::Error> {
        assert!(!self.key_read_flag, "{}", INVALID_STATE_ERROR);
        self.key_read_flag = true;
        self.source.flush_str()
    }

    /// Get the value for this [`Entry`]. If the key has not been read, it will be skipped.
    pub fn value(self) -> Result<Value<'a, D>, D::Error> {
        if !self.key_read_flag {
            self.source.skip_str()?;
        }
        Ok(Value::new(self.source, self.done_flag))
    }
}

/// A wrapper over a [`JsonDeserializer`] which allows the user to deserialize an [`Object`] or
/// a [`List`] uniformly.
/// 
/// This is a "helper" wrapper intended to provide a convenient interface and enforce correct usage
/// of the deserializer.
pub enum Collection<'a, D: JsonDeserializer + ?Sized> {
    Object(Object<'a, D>),
    List(List<'a, D>),
}

impl<'a, D: JsonDeserializer + ?Sized> Collection<'a, D> {
    /// Attempts to get the next unread value from the collection: entry values for objects
    /// and list items for lists. Returns `Ok(None)` if there are no entries remaining. In that
    /// case, the collection will be automatically closed and should not be used again.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<Option<Value<D>>, D::Error> {
        match self {
            Collection::Object(obj) => obj.next_entry()?.map(|entry| entry.value()).transpose(),
            Collection::List(list) => list.next(),
        }
    }
}

impl<'a, D: JsonDeserializer + ?Sized> From<Object<'a, D>> for Collection<'a, D> {
    fn from(value: Object<'a, D>) -> Self {
        Self::Object(value)
    }
}

impl<'a, D: JsonDeserializer + ?Sized> From<List<'a, D>> for Collection<'a, D> {
    fn from(value: List<'a, D>) -> Self {
        Self::List(value)
    }
}
