//! Wrapper around `arrayvec::ArrayString`.
//!
//! Adds `from_truncate` and `from_unchecked` on top of
//! `arrayvec::ArrayString`.

use std::fmt;
use std::ops::Deref;

use crate::util;
use crate::{error::Error, Result};

use arrayvec::Array;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::hash::{Hash, Hasher};

#[derive(Debug, Default, Clone, Copy)]
pub struct ArrayString<A>(arrayvec::ArrayString<A>)
where
    A: Array<Item = u8> + Copy;

impl<A> ArrayString<A>
where
    A: Array<Item = u8> + Copy,
{
    pub fn from_truncate(s: &str) -> Self {
        ArrayString(arrayvec::ArrayString::from(util::truncate_str(s, A::CAPACITY as u8)).unwrap())
    }

    pub fn from_unchecked(s: &str) -> Self {
        let mut a = arrayvec::ArrayString::new();
        a.push_str(s);
        ArrayString(a)
    }

    pub fn from(s: &str) -> Result<Self> {
        Ok(ArrayString(
            arrayvec::ArrayString::from(s).map_err(|e| Error::Other(e.to_string()))?,
        ))
    }

    pub fn new() -> Self {
        ArrayString(arrayvec::ArrayString::new())
    }

    pub fn as_str(&self) -> &str {
        self.as_ref()
    }
}

impl<A> fmt::Display for ArrayString<A>
where
    A: Array<Item = u8> + Copy,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<A> AsRef<str> for ArrayString<A>
where
    A: Array<Item = u8> + Copy,
{
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<A> Deref for ArrayString<A>
where
    A: Array<Item = u8> + Copy,
{
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        &self.0
    }
}

impl<A> PartialEq for ArrayString<A>
where
    A: Array<Item = u8> + Copy,
{
    fn eq(&self, rhs: &Self) -> bool {
        **self == **rhs
    }
}

impl<A> PartialEq<str> for ArrayString<A>
where
    A: Array<Item = u8> + Copy,
{
    fn eq(&self, rhs: &str) -> bool {
        &**self == rhs
    }
}

impl<A> PartialEq<ArrayString<A>> for str
where
    A: Array<Item = u8> + Copy,
{
    fn eq(&self, rhs: &ArrayString<A>) -> bool {
        self == &**rhs
    }
}

impl<A> Eq for ArrayString<A> where A: Array<Item = u8> + Copy {}

impl<A> Hash for ArrayString<A>
where
    A: Array<Item = u8> + Copy,
{
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.hash(h)
    }
}

// #[cfg(feature="serde")]
/// Requires crate feature `"serde"`
impl<A> Serialize for ArrayString<A>
where
    A: Array<Item = u8> + Copy,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&*self)
    }
}

// #[cfg(feature = "serde")]
/// Requires crate feature `"serde"`
impl<'de, A> Deserialize<'de> for ArrayString<A>
where
    A: Array<Item = u8> + Copy,
{
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{self, Visitor};
        use std::marker::PhantomData;

        struct ArrayStringVisitor<A: Array<Item = u8>>(PhantomData<A>);

        impl<'de, A: Copy + Array<Item = u8>> Visitor<'de> for ArrayStringVisitor<A> {
            type Value = ArrayString<A>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    formatter,
                    "a string no more than {} bytes long",
                    A::CAPACITY
                )
            }

            fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                ArrayString::from(v).map_err(|_| E::invalid_length(v.len(), &self))
            }

            fn visit_bytes<E>(self, v: &[u8]) -> std::result::Result<Self::Value, E>
            where
                E: de::Error,
            {
                let s = std::str::from_utf8(v)
                    .map_err(|_| E::invalid_value(de::Unexpected::Bytes(v), &self))?;

                ArrayString::from(s).map_err(|_| E::invalid_length(s.len(), &self))
            }
        }

        deserializer.deserialize_str(ArrayStringVisitor::<A>(PhantomData))
    }
}
