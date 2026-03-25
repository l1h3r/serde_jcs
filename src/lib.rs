//! JSON Canonicalization Scheme (JCS)
//!
//! ### References
//!
//! [RFC 8785](https://tools.ietf.org/html/rfc8785)
//!

use ryu_js::Buffer;
use serde::Serialize;
use serde_json::Result;
use serde_json::Serializer;
use serde_json::Value;
use serde_json::from_slice;
use serde_json::from_str;
use serde_json::ser::CharEscape;
use serde_json::ser::Formatter;
use std::collections::BTreeMap;
use std::io;
use std::io::Write;
use std::mem::take;
use std::num::FpCategory;

/// Serialize the given value as a String of JSON.
///
/// Serialization is performed as specified in [RFC 8785](https://tools.ietf.org/html/rfc8785).
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` fails.
#[inline]
pub fn to_string<T>(value: &T) -> Result<String>
where
  T: Serialize + ?Sized,
{
  let data: Vec<u8> = to_vec(value)?;

  // SAFETY: We only emit valid UTF-8.
  let data: String = unsafe { String::from_utf8_unchecked(data) };

  Ok(data)
}

/// Serialize the given value as a JSON byte vector.
///
/// Serialization is performed as specified in [RFC 8785](https://tools.ietf.org/html/rfc8785).
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` fails.
#[inline]
pub fn to_vec<T>(value: &T) -> Result<Vec<u8>>
where
  T: Serialize + ?Sized,
{
  let mut data: Vec<u8> = Vec::with_capacity(128);

  to_writer(&mut data, value)?;

  Ok(data)
}

/// Serialize the given value as JSON into the IO stream.
///
/// Serialization is performed as specified in [RFC 8785](https://tools.ietf.org/html/rfc8785).
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` fails.
#[inline]
pub fn to_writer<W, T>(writer: W, value: &T) -> Result<()>
where
  W: Write,
  T: Serialize + ?Sized,
{
  value.serialize(&mut Serializer::with_formatter(writer, JcsFormatter::new()))
}

// -----------------------------------------------------------------------------
// UTF-16 Key
// -----------------------------------------------------------------------------

struct Utf16Key {
  tag: Vec<u16>,
  key: Vec<u8>,
}

impl Utf16Key {
  fn new(key: Vec<u8>) -> io::Result<Self> {
    let tag: Vec<u16> = from_slice::<Value>(&key)?
      .as_str()
      .ok_or_else(|| io::Error::other("invalid UTF-8 key"))?
      .encode_utf16()
      .collect();

    Ok(Self { tag, key })
  }
}

impl PartialEq for Utf16Key {
  #[inline]
  fn eq(&self, other: &Self) -> bool {
    self.tag.eq(&other.tag)
  }
}

impl Eq for Utf16Key {}

impl PartialOrd for Utf16Key {
  #[inline]
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for Utf16Key {
  #[inline]
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.tag.cmp(&other.tag)
  }
}

// -----------------------------------------------------------------------------
// Formatter Entry
// -----------------------------------------------------------------------------

struct Entry {
  object: BTreeMap<Utf16Key, Vec<u8>>,
  next_key: Vec<u8>,
  next_val: Vec<u8>,
  complete: bool,
}

impl Entry {
  #[inline]
  const fn new() -> Self {
    Self {
      object: BTreeMap::new(),
      next_key: Vec::new(),
      next_val: Vec::new(),
      complete: false,
    }
  }

  #[inline]
  const fn complete(&mut self, value: bool) {
    self.complete = value;
  }
}

// -----------------------------------------------------------------------------
// JSON Formatter
// -----------------------------------------------------------------------------

struct JcsFormatter {
  entries: Vec<Entry>,
}

impl JcsFormatter {
  #[inline]
  const fn new() -> Self {
    Self {
      entries: Vec::new(),
    }
  }

  #[inline]
  fn scope<'a, W>(&'a mut self, writer: &'a mut W) -> Box<dyn Write + 'a>
  where
    W: Write + ?Sized,
  {
    match self.entry_mut() {
      Some(entry) if entry.complete => Box::new(&mut entry.next_val),
      Some(entry) => Box::new(&mut entry.next_key),
      None => Box::new(writer),
    }
  }

  #[inline]
  fn entry_mut(&mut self) -> Option<&mut Entry> {
    self.entries.last_mut()
  }
}

impl Formatter for JcsFormatter {
  #[inline]
  fn write_null<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.scope(writer).write_all(b"null")
  }

  #[inline]
  fn write_bool<W>(&mut self, writer: &mut W, value: bool) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    if value {
      self.scope(writer).write_all(b"true")
    } else {
      self.scope(writer).write_all(b"false")
    }
  }

  #[inline]
  fn write_char_escape<W>(&mut self, writer: &mut W, escape: CharEscape) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    #[inline]
    fn serialize(control: u8) -> [u8; 6] {
      static HEX: [u8; 16] = *b"0123456789abcdef";
      [
        b'\\',
        b'u',
        b'0',
        b'0',
        HEX[(control >> 4) as usize],
        HEX[(control & 0xF) as usize],
      ]
    }

    match escape {
      CharEscape::Quote => self.scope(writer).write_all(b"\\\""),
      CharEscape::ReverseSolidus => self.scope(writer).write_all(b"\\\\"),
      CharEscape::Solidus => self.scope(writer).write_all(b"/"),
      CharEscape::Backspace => self.scope(writer).write_all(b"\\b"),
      CharEscape::FormFeed => self.scope(writer).write_all(b"\\f"),
      CharEscape::LineFeed => self.scope(writer).write_all(b"\\n"),
      CharEscape::CarriageReturn => self.scope(writer).write_all(b"\\r"),
      CharEscape::Tab => self.scope(writer).write_all(b"\\t"),
      CharEscape::AsciiControl(control) => self.scope(writer).write_all(&serialize(control)),
    }
  }

  #[inline]
  fn write_number_str<W>(&mut self, writer: &mut W, value: &str) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.write_f64(writer, value.parse().map_err(io::Error::other)?)
  }

  #[inline]
  fn write_string_fragment<W>(&mut self, writer: &mut W, fragment: &str) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.scope(writer).write_all(fragment.as_bytes())
  }

  #[inline]
  fn write_raw_fragment<W>(&mut self, writer: &mut W, fragment: &str) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    let scope: Box<dyn Write> = self.scope(writer);
    let value: Value = from_str(fragment)?;

    to_writer(scope, &value).map_err(Into::into)
  }

  #[inline]
  fn write_i8<W>(&mut self, writer: &mut W, value: i8) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.write_f64(writer, f64::from(value))
  }

  #[inline]
  fn write_i16<W>(&mut self, writer: &mut W, value: i16) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.write_f64(writer, f64::from(value))
  }

  #[inline]
  fn write_i32<W>(&mut self, writer: &mut W, value: i32) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.write_f64(writer, f64::from(value))
  }

  #[expect(clippy::cast_precision_loss)]
  #[inline]
  fn write_i64<W>(&mut self, writer: &mut W, value: i64) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.write_f64(writer, value as f64)
  }

  #[expect(clippy::cast_precision_loss)]
  #[inline]
  fn write_i128<W>(&mut self, writer: &mut W, value: i128) -> io::Result<()>
  where
    W: ?Sized + Write,
  {
    self.write_f64(writer, value as f64)
  }

  #[inline]
  fn write_u8<W>(&mut self, writer: &mut W, value: u8) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.write_f64(writer, f64::from(value))
  }

  #[inline]
  fn write_u16<W>(&mut self, writer: &mut W, value: u16) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.write_f64(writer, f64::from(value))
  }

  #[inline]
  fn write_u32<W>(&mut self, writer: &mut W, value: u32) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.write_f64(writer, f64::from(value))
  }

  #[expect(clippy::cast_precision_loss)]
  #[inline]
  fn write_u64<W>(&mut self, writer: &mut W, value: u64) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.write_f64(writer, value as f64)
  }

  #[expect(clippy::cast_precision_loss)]
  #[inline]
  fn write_u128<W>(&mut self, writer: &mut W, value: u128) -> io::Result<()>
  where
    W: ?Sized + Write,
  {
    self.write_f64(writer, value as f64)
  }

  #[inline]
  fn write_f32<W>(&mut self, writer: &mut W, value: f32) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.write_f64(writer, f64::from(value))
  }

  #[inline]
  fn write_f64<W>(&mut self, writer: &mut W, value: f64) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    match value.classify() {
      FpCategory::Nan | FpCategory::Infinite => Err(io::Error::other("oh no")),
      FpCategory::Zero => self.scope(writer).write_all(b"0"),
      FpCategory::Normal | FpCategory::Subnormal => self
        .scope(writer)
        .write_all(Buffer::new().format_finite(value).as_bytes()),
    }
  }

  #[inline]
  fn begin_string<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.scope(writer).write_all(b"\"")
  }

  #[inline]
  fn end_string<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.scope(writer).write_all(b"\"")
  }

  #[inline]
  fn begin_array<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.scope(writer).write_all(b"[")
  }

  #[inline]
  fn end_array<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.scope(writer).write_all(b"]")
  }

  #[inline]
  fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    if first {
      Ok(())
    } else {
      self.scope(writer).write_all(b",")
    }
  }

  #[inline]
  fn end_array_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    Ok(())
  }

  #[inline]
  fn begin_object<W>(&mut self, _writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.entries.push(Entry::new());

    Ok(())
  }

  fn end_object<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    let entry: Entry = self
      .entries
      .pop()
      .ok_or_else(|| io::Error::other("end_object called before begin_object"))?;

    let mut scope: Box<dyn Write> = self.scope(writer);

    scope.write_all(b"{")?;

    for (index, (key, val)) in entry.object.into_iter().enumerate() {
      if index != 0 {
        scope.write_all(b",")?;
      }

      scope.write_all(&key.key)?;
      scope.write_all(b":")?;
      scope.write_all(&val)?;
    }

    scope.write_all(b"}")?;

    Ok(())
  }

  #[inline]
  fn begin_object_key<W>(&mut self, _writer: &mut W, _first: bool) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self
      .entry_mut()
      .ok_or_else(|| io::Error::other("begin_object_key called before begin_object"))
      .map(|entry| entry.complete(false))
  }

  #[inline]
  fn end_object_key<W>(&mut self, _writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self
      .entry_mut()
      .ok_or_else(|| io::Error::other("end_object_key called before begin_object"))
      .map(|entry| entry.complete(true))
  }

  #[inline]
  fn begin_object_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    Ok(())
  }

  fn end_object_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    let entry: &mut Entry = self
      .entry_mut()
      .ok_or_else(|| io::Error::other("end_object_value called before begin_object"))?;

    let key: Vec<u8> = take(&mut entry.next_key);
    let val: Vec<u8> = take(&mut entry.next_val);

    entry.object.insert(Utf16Key::new(key)?, val);

    Ok(())
  }
}
