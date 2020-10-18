//! JSON Canonicalization Scheme (JCS)
//!
//! ### References
//!
//! [RFC 8785](https://tools.ietf.org/html/rfc8785)
//!
mod buffer;
mod entry;
mod ser;

pub use self::ser::to_string;
pub use self::ser::to_vec;
pub use self::ser::to_writer;
