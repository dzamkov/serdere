use std::cmp::Ordering;

/// An immutable lookup table which associates `&static str`'s to values of type `T`.
#[derive(Debug)]
#[repr(transparent)]
pub struct NameMap<T>([(&'static str, T)]);

impl<T> NameMap<T> {
    /// Begins a lookup into this [`NameMap`].
    pub fn lookup(&self) -> NameMapLookup<T> {
        NameMapLookup {
            cands: &self.0,
            input_len: 0,
        }
    }

    /// Gets the value corresponding to the given name in this [`NameMap`], or returns [`None`]
    /// if no such entry exists.
    pub fn get(&self, name: &str) -> Option<&T> {
        let mut lookup = self.lookup();
        lookup.write_str(name);
        lookup.result()
    }

    /// Gets the number of entries in this [`NameMap`].
    pub fn size(&self) -> usize {
        self.0.len()
    }

    /// Gets an iterator over the entries in this [`NameMap`].
    pub fn entries(&self) -> impl Iterator<Item = (&'static str, &T)> {
        self.0.iter().map(|(name, value)| (*name, value))
    }
}

/// A [`NameMap`] with a fixed amount of entries.
pub struct FixedNameMap<T, const N: usize>([(&'static str, T); N]);

impl<T: Copy, const N: usize> FixedNameMap<T, N> {
    /// Constructs a new [`FixedNameMap`] with the given entries.
    pub const fn new(mut entries: [(&'static str, T); N]) -> Self {
        entries = sort_entries(entries, 0, N);
        Self(entries)
    }

    /// Converts this into a [`NameMap`] reference.
    pub const fn unfix(&self) -> &NameMap<T> {
        let inner: &[_] = &self.0;
        // SAFETY: Transmuting to `repr(transparent)` wrapper.
        unsafe { std::mem::transmute(inner) }
    }
}

/// An interface for looking up a name in a [`NameMap`] from a string that is incrementally
/// written into it. This is done without actually storing the string, avoiding unnecessary
/// allocations.
#[derive(Debug)]
pub struct NameMapLookup<'a, T> {
    cands: &'a [(&'static str, T)],
    input_len: usize,
}

impl<'a, T> NameMapLookup<'a, T> {
    /// Adds an additional charater to the lookup string.
    pub fn write_char(&mut self, ch: char) {
        self.write_str(ch.encode_utf8(&mut [0; 4]))
    }

    /// Extends the lookup string.
    pub fn write_str(&mut self, str: &str) {
        self.write_bytes(str.as_bytes())
    }

    /// Extends the lookup string with the given UTF-8 encoded byte data. If the data is not
    /// valid UTF-8, the lookup will return an indeterminate result.
    pub fn write_bytes(&mut self, data: &[u8]) {
        // Binary search to find some candidate that can still match the lookup string.
        let mut lo = 0;
        let mut hi = self.cands.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let mid_data = truncate_slice(self.cands[mid].0.as_bytes(), self.input_len, data.len());
            match cmp_bytes(mid_data, data) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => {
                    let mut lo_hi = mid;
                    let mut hi_lo = mid + 1;

                    // Binary search to extend the lower bound to include all matching candidates
                    while lo < lo_hi {
                        let mid = lo + (lo_hi - lo) / 2;
                        let mid_data = truncate_slice(
                            self.cands[mid].0.as_bytes(),
                            self.input_len,
                            data.len(),
                        );
                        match cmp_bytes(mid_data, data) {
                            Ordering::Less => lo = mid + 1,
                            Ordering::Greater => unreachable!(),
                            Ordering::Equal => lo_hi = mid,
                        }
                    }

                    // Binary search to extend the upper bound to include all matching candidates
                    while hi_lo < hi {
                        let mid = hi_lo + (hi - hi_lo) / 2;
                        let mid_data = truncate_slice(
                            self.cands[mid].0.as_bytes(),
                            self.input_len,
                            data.len(),
                        );
                        match cmp_bytes(mid_data, data) {
                            Ordering::Less => unreachable!(),
                            Ordering::Greater => hi = mid,
                            Ordering::Equal => hi_lo = mid + 1,
                        }
                    }
                    break;
                }
            }
        }
        self.cands = &self.cands[lo..hi];
        self.input_len += data.len();
    }

    /// Gets the value corresponding to the lookup string written to this [`NameMapLookup`], or
    /// [`None`] if no such entry exists.
    pub fn result(&self) -> Option<&'a T> {
        let (key, value) = self.cands.first()?;
        if key.len() != self.input_len {
            return None;
        }
        Some(value)
    }
}

/// Gets a slice of the given array, starting at the given index and having up to the given
/// length.
fn truncate_slice<T>(data: &[T], start: usize, len: usize) -> &[T] {
    let data = &data[start..];
    if data.len() > len {
        &data[..len]
    } else {
        data
    }
}

/// Sorts an array of entries.
const fn sort_entries<T: Copy, const N: usize>(
    mut values: [(&'static str, T); N],
    mut lo: usize,
    mut hi: usize,
) -> [(&'static str, T); N] {
    while lo + 1 < hi {
        let mut le = lo;
        let mut ge = hi;
        let p = values[lo + (hi - lo) / 2].0;
        loop {
            while let Ordering::Less = cmp_str(values[le].0, p) {
                le += 1;
            }
            ge -= 1;
            while let Ordering::Greater = cmp_str(values[ge].0, p) {
                ge -= 1;
            }
            if le >= ge {
                ge = le;
                break;
            }
            let temp = values[le];
            values[le] = values[ge];
            values[ge] = temp;
            le += 1;
            if le >= ge {
                break;
            }
        }
        if ge - lo < hi - le {
            values = sort_entries(values, lo, ge);
            lo = le;
        } else {
            values = sort_entries(values, le, hi);
            hi = ge;
        }
    }
    values
}

/// Compares two strings. Useable in a constant context.
const fn cmp_str(left: &str, right: &str) -> Ordering {
    cmp_bytes(left.as_bytes(), right.as_bytes())
}

/// Compares two byte arrays. Useable in a constant context.
const fn cmp_bytes(left: &[u8], right: &[u8]) -> Ordering {
    let mut i = 0;
    loop {
        if i >= left.len() {
            if i >= right.len() {
                return Ordering::Equal;
            } else {
                return Ordering::Less;
            }
        } else if i >= right.len() {
            return Ordering::Greater;
        } else if left[i] < right[i] {
            return Ordering::Less;
        } else if left[i] > right[i] {
            return Ordering::Greater;
        }
        i += 1
    }
}

#[test]
fn test_sorted() {
    let map = FixedNameMap::new([
        ("pot", 0),
        ("boy", 1),
        ("rice", 2),
        ("film", 3),
        ("taxi", 4),
        ("debt", 5),
        ("fat", 6),
        ("firm", 7),
        ("run", 8),
        ("bat", 9),
        ("fi", 10),
    ]);
    let entries = &map.0;
    for i in 1..entries.len() {
        assert!(entries[i - 1] < entries[i]);
    }
}

#[test]
fn test_lookup() {
    const NAMES: &NameMap<u32> = FixedNameMap::new([
        ("pot", 0),
        ("boy", 1),
        ("rice", 2),
        ("film", 3),
        ("taxi", 4),
        ("debt", 5),
        ("fat", 6),
        ("firm", 7),
        ("run", 8),
        ("bat", 9),
        ("fi", 10),
    ])
    .unfix();
    for (name, value) in NAMES.0.iter() {
        assert_eq!(NAMES.get(name).unwrap(), value);
    }
    assert_eq!(NAMES.get("dolphin"), None);
    assert_eq!(NAMES.get(""), None);
    assert_eq!(NAMES.get("fir"), None);
    assert_eq!(NAMES.get("d"), None);
}
