#[allow(unused_imports)]
use crate::{Deserializer, Serializer};

/// A streaming interface for describing an arbitrarily-complex data structure. This contains the
/// common functionality of [`Serializer`] and [`Deserializer`]: methods which describe the
/// "shape" of the data structure without the actual data. This is done using a stack-based API.
pub trait Outliner {
    /// An error that can occur during serialization or deserialization. For serialization, this is
    /// limited to errors in the downstream data sink. For deserialization, this may be due to
    /// malformed input, or an error in the upstream data source. Incorrect API usage should
    /// generally panic, rather than return an error.
    type Error: std::error::Error;

    /// Indicates whether the underlying serialization format supports `null` literals.
    fn supports_null(&self) -> bool;

    /// Assuming that the top item on the stack is a value, asserts that it is a `null` literal
    /// and pops it. `null` is a format-dependent literal representing either a default, or the
    /// absence of a "real" value. This method may only be called if [`Outliner::supports_null`]
    /// returns `true`.
    fn pop_null(&mut self) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is a value, asserts that it is a string, popping
    /// it and pushing an opened string onto the stack.
    fn open_str(&mut self) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is an opened string, asserts that the end of
    /// string has been reached, popping it from the stack.
    fn close_str(&mut self) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is a value, asserts that it is an ordered
    /// collection of named fields, popping it and pushing an opened struct onto the stack.
    fn open_struct(&mut self, type_name: Option<&'static str>) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is an opened struct, asserts that the next field
    /// exists and has the given name, pushing the value of the field onto the stack. Regardless
    /// of name, fields must be always considered in the struct-defined order.
    fn push_field(&mut self, name: &'static str) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is an opened struct, asserts that it has no
    /// remaining fields and pops it from the stack.
    fn close_struct(&mut self) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is a value, asserts that it is an ordered
    /// collection of unnamed elements, popping it and pushing an opened tuple onto the stack.
    fn open_tuple(&mut self, type_name: Option<&'static str>) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is an opened tuple, asserts that a next element
    /// exists, pushing the value of the element onto the stack.
    fn push_element(&mut self) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is an opened tuple, asserts that it has no
    /// remaining elements and pops it from the stack.
    fn close_tuple(&mut self) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is an opened list, asserts that it has at least
    /// one more item, pushing the value of the item onto the stack.
    fn push_item(&mut self) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is an opened list, asserts that it has no
    /// remaining items and pops it from the stack.
    fn close_list(&mut self) -> Result<(), Self::Error>;
}
