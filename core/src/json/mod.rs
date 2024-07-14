pub mod deserialize;
pub mod serialize;
mod helper;
mod outliner;
pub use deserialize::*;
pub use serialize::*;
pub use helper::*;
pub use outliner::*;

/// Identifies a type of JSON value.
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Copy)]
pub enum ValueType {
    String,
    Number,
    Object,
    Array,
    Bool,
    Null,
}

/// Identifies a type of JSON collection (either object or array).
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Copy)]
pub enum CollectionType {
    Object,
    Array,
}
