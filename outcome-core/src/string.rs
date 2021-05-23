//! Introduces additional functions for creating `arrayvec::ArrayString`s.

use arrayvec::Array;

use crate::error::{Error, Result};
use crate::util;

#[cfg(feature = "stack_stringid")]
pub fn new<A>(s: &str) -> Result<arrayvec::ArrayString<A>>
where
    A: Array<Item = u8> + Copy,
{
    arrayvec::ArrayString::from(s).map_err(|e| Error::Other(format!("{}", e)))
}
#[cfg(not(feature = "stack_stringid"))]
pub fn new(s: &str) -> Result<String> {
    Ok(String::from(s))
}

#[cfg(feature = "stack_stringid")]
pub fn new_truncate<A>(s: &str) -> arrayvec::ArrayString<A>
where
    A: Array<Item = u8> + Copy,
{
    arrayvec::ArrayString::from(util::truncate_str(s, A::CAPACITY as u8)).unwrap()
}
#[cfg(not(feature = "stack_stringid"))]
pub fn new_truncate(s: &str) -> String {
    String::from(s)
}
