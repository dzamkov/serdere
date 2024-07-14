/// A number that can be serialized and deserialized to JSON. The default value should be 0.
pub trait Num: Sized {
    /// The [`NumBuilder`] for this type of number.
    type Builder: NumBuilder + Default;

    /// Tries getting a number of this type from the given [`NumBuilder`], possibly negating
    /// it and multiplying by a power of 10. This will return [`None`] if it is not possible to
    /// represent the desired number with this type.
    fn from_builder(builder: Self::Builder, negate: bool, exp_10: i32) -> Option<Self>;
}

/// An interface for "constructing" a number by iteratively appending digits.
pub trait NumBuilder {
    /// Attempts to "append" a digit to the number, returning `false` if this would result in
    /// overflow
    fn push_digit(&mut self, digit: u8) -> bool;
}

macro_rules! impl_unsigned {
    ($ty:ty) => {
        impl Num for $ty {
            type Builder = Self;
            fn from_builder(value: Self, negate: bool, exp_10: i32) -> Option<Self> {
                if negate && value != 0 {
                    return None;
                }
                let ten: Self = 10;
                if let Ok(exp_10) = u32::try_from(exp_10) {
                    let pow = ten.checked_pow(exp_10)?;
                    value.checked_mul(pow)
                } else {
                    let i_exp_10 = exp_10.unsigned_abs();
                    let pow = ten.checked_pow(i_exp_10)?;
                    if value.wrapping_rem(pow) != 0 {
                        return None;
                    }
                    Some(value.wrapping_div(pow))
                }
            }
        }

        impl NumBuilder for $ty {
            fn push_digit(&mut self, digit: u8) -> bool {
                if let Some(x) = self.checked_mul(10) {
                    if let Some(x) = x.checked_add(Self::from(digit)) {
                        *self = x;
                        return true;
                    }
                }
                false
            }
        }
    };
}

impl_unsigned!(u8);
impl_unsigned!(u16);
impl_unsigned!(u32);
impl_unsigned!(u64);
impl_unsigned!(u128);

macro_rules! impl_signed {
    ($ty:ty, $unsigned_ty:ty) => {
        impl Num for $ty {
            type Builder = $unsigned_ty;
            fn from_builder(mut value: Self::Builder, negate: bool, exp_10: i32) -> Option<Self> {
                let ten: Self::Builder = 10;
                if let Ok(exp_10) = u32::try_from(exp_10) {
                    let pow = ten.checked_pow(exp_10)?;
                    value = value.checked_mul(pow)?;
                } else {
                    let i_exp_10 = exp_10.unsigned_abs();
                    let pow = ten.checked_pow(i_exp_10)?;
                    if value.wrapping_rem(pow) != 0 {
                        return None;
                    }
                    value = value.wrapping_div(pow);
                };
                if negate {
                    Self::checked_sub_unsigned(0, value)
                } else {
                    Self::try_from(value).ok()
                }
            }
        }
    };
}

impl_signed!(i8, u8);
impl_signed!(i16, u16);
impl_signed!(i32, u32);
impl_signed!(i64, u64);
impl_signed!(i128, u128);

// TODO: Improve float parsing performance by bypassing std `parse`.

/// The [`NumBuilder`] for a floating-point number.
pub enum FloatBuilder {
    Small {
        buf: [u8; Self::SMALL_LEN],
        len: usize,
    },
    Large(Vec<u8>),
}

impl FloatBuilder {
    /// The amount of buffer space allocated for small numbers.
    const SMALL_LEN: usize = 32;

    /// Appends the given string to the builder.
    fn push_str(&mut self, s: &str) {
        match self {
            Self::Small { buf, len } => {
                let s_len = s.len();
                if *len + s_len <= Self::SMALL_LEN {
                    buf[*len..(*len + s_len)].copy_from_slice(s.as_bytes());
                    *len += s_len;
                } else {
                    let mut vec = Vec::with_capacity(*len + s_len);
                    vec.extend_from_slice(&buf[..*len]);
                    vec.extend_from_slice(s.as_bytes());
                    *self = Self::Large(vec);
                }
            }
            Self::Large(vec) => {
                vec.extend_from_slice(s.as_bytes());
            }
        }
    }

    /// Appends the given integer to the builder.
    fn push_i32(&mut self, value: i32) {
        let mut buf = itoa::Buffer::new();
        self.push_str(buf.format(value));
    }

    /// Gets the string representation of the number.
    fn as_str(&self) -> &str {
        let buf = match self {
            Self::Small { buf, len } => &buf[..*len],
            Self::Large(vec) => vec,
        };
        unsafe { std::str::from_utf8_unchecked(buf) }
    }
}

impl Default for FloatBuilder {
    fn default() -> Self {
        Self::Small {
            buf: Default::default(),
            len: 0,
        }
    }
}

impl NumBuilder for FloatBuilder {
    fn push_digit(&mut self, digit: u8) -> bool {
        let digit_utf = digit + b'0';
        self.push_str(unsafe { std::str::from_utf8_unchecked(std::slice::from_ref(&digit_utf)) });
        true
    }
}

macro_rules! impl_float {
    ($ty:ty) => {
        impl Num for $ty {
            type Builder = FloatBuilder;
            fn from_builder(mut builder: FloatBuilder, negate: bool, exp_10: i32) -> Option<Self> {
                let mut res = if matches!(builder, FloatBuilder::Small { len: 0, .. }) {
                    0.0
                } else {
                    builder.push_str("e");
                    builder.push_i32(exp_10);
                    builder.as_str().parse::<Self>().unwrap()
                };
                if negate {
                    res = -res;
                }
                Some(res)
            }
        }
    };
}

impl_float!(f32);
impl_float!(f64);