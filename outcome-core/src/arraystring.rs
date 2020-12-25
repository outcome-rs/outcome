//! Wrapper around `arrayvec::ArrayString`.
//!
//! Adds `from_truncate` and `from_unchecked` on top of
//! `arrayvec::ArrayString`.

use arrayvec::Array;

use crate::error::{Error, Result};
use crate::util;

pub fn new<A>(s: &str) -> Result<arrayvec::ArrayString<A>>
where
    A: Array<Item = u8> + Copy,
{
    arrayvec::ArrayString::from(s).map_err(|e| Error::Other(format!("{}", e)))
}

pub fn new_truncate<A>(s: &str) -> arrayvec::ArrayString<A>
where
    A: Array<Item = u8> + Copy,
{
    arrayvec::ArrayString::from(util::truncate_str(s, A::CAPACITY as u8)).unwrap()
}

pub fn new_unchecked<A>(s: &str) -> arrayvec::ArrayString<A>
where
    A: Array<Item = u8> + Copy,
{
    arrayvec::ArrayString::from(s).unwrap()
}
