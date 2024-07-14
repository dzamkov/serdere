use serdere::Outliner;

/// Extends [`Outliner`] to be able to describe JSON structures.
pub trait JsonOutliner: Outliner {
    /// Assuming that the top item on the stack is a value, asserts that it is a JSON object,
    /// popping it and pushing an opened object onto the stack.
    fn open_object(&mut self) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is an opened object, asserts that it has a
    /// remaining entry with the given key, pushing the corresponding value onto the stack.
    fn push_entry(&mut self, key: &str) -> Result<(), Self::Error>;

    /// Assuming that the top item on the stack is an opened JSON object, asserts that it has no
    /// remaining entries and pops it from the stack.
    fn close_object(&mut self) -> Result<(), Self::Error>;
}