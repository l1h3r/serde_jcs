use core::mem::replace;
use core::num::FpCategory;
use serde::Serialize;
use serde_json::from_str;
use serde_json::ser::CharEscape;
use serde_json::ser::CompactFormatter;
use serde_json::ser::Formatter;
use serde_json::Result;
use serde_json::Serializer;
use serde_json::Value;

use std::io;
use std::io::Write;

use crate::entry::Entry;

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

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct JcsFormatter(Vec<Entry>);

impl JcsFormatter {
  pub const fn new() -> Self {
    Self(Vec::new())
  }

  pub fn scope<'a, W>(&'a mut self, writer: &'a mut W) -> Box<dyn Write + 'a>
  where
    W: Write + ?Sized,
  {
    match self.0.last_mut() {
      Some(entry) if entry.complete => Box::new(&mut entry.next_val),
      Some(entry) => Box::new(&mut entry.next_key),
      None => Box::new(writer),
    }
  }

  pub fn entry_mut(&mut self) -> io::Result<&mut Entry> {
    self
      .0
      .last_mut()
      .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "oh no"))
  }

  fn write_float<W, F>(&mut self, writer: &mut W, category: FpCategory, value: F) -> io::Result<()>
  where
    W: Write + ?Sized,
    F: ryu_js::Float,
  {
    match category {
      FpCategory::Nan | FpCategory::Infinite => Err(io::Error::new(io::ErrorKind::Other, "oh no")),
      FpCategory::Zero => self.scope(writer).write_all(b"0"),
      FpCategory::Normal | FpCategory::Subnormal => self
        .scope(writer)
        .write_all(ryu_js::Buffer::new().format_finite(value).as_bytes()),
    }
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
      CharEscape::Solidus => self.scope(writer).write_all(b"\\/"),
      CharEscape::Backspace => self.scope(writer).write_all(b"\\b"),
      CharEscape::FormFeed => self.scope(writer).write_all(b"\\f"),
      CharEscape::LineFeed => self.scope(writer).write_all(b"\\n"),
      CharEscape::CarriageReturn => self.scope(writer).write_all(b"\\r"),
      CharEscape::Tab => self.scope(writer).write_all(b"\\t"),
      CharEscape::AsciiControl(control) => self.scope(writer).write_all(&serialize(control)),
    }
  }

  #[inline]
  fn write_number_str<W>(&mut self, _writer: &mut W, _value: &str) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    todo!("Handle number str (u128/i128)")
  }

  #[inline]
  fn write_string_fragment<W>(&mut self, writer: &mut W, fragment: &str) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    // TOOD: Check
    self.scope(writer).write_all(fragment.as_bytes())
  }

  #[inline]
  fn write_raw_fragment<W>(&mut self, writer: &mut W, fragment: &str) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    // TOOD: Check
    from_str::<Value>(fragment)?
      .serialize(&mut Serializer::with_formatter(
        self.scope(writer),
        Self::new(),
      ))
      .map_err(Into::into)
  }

  #[inline]
  fn write_i8<W>(&mut self, writer: &mut W, value: i8) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    CompactFormatter.write_i8(&mut self.scope(writer), value)
  }

  #[inline]
  fn write_i16<W>(&mut self, writer: &mut W, value: i16) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    CompactFormatter.write_i16(&mut self.scope(writer), value)
  }

  #[inline]
  fn write_i32<W>(&mut self, writer: &mut W, value: i32) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    CompactFormatter.write_i32(&mut self.scope(writer), value)
  }

  #[inline]
  fn write_i64<W>(&mut self, writer: &mut W, value: i64) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    CompactFormatter.write_i64(&mut self.scope(writer), value)
  }

  #[inline]
  fn write_u8<W>(&mut self, writer: &mut W, value: u8) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    CompactFormatter.write_u8(&mut self.scope(writer), value)
  }

  #[inline]
  fn write_u16<W>(&mut self, writer: &mut W, value: u16) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    CompactFormatter.write_u16(&mut self.scope(writer), value)
  }

  #[inline]
  fn write_u32<W>(&mut self, writer: &mut W, value: u32) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    CompactFormatter.write_u32(&mut self.scope(writer), value)
  }

  #[inline]
  fn write_u64<W>(&mut self, writer: &mut W, value: u64) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    CompactFormatter.write_u64(&mut self.scope(writer), value)
  }

  #[inline]
  fn write_f32<W>(&mut self, writer: &mut W, value: f32) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.write_float(writer, value.classify(), value)
  }

  #[inline]
  fn write_f64<W>(&mut self, writer: &mut W, value: f64) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.write_float(writer, value.classify(), value)
  }

  #[inline]
  fn begin_string<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    CompactFormatter.begin_string(&mut self.scope(writer))
  }

  #[inline]
  fn end_string<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    CompactFormatter.end_string(&mut self.scope(writer))
  }

  #[inline]
  fn begin_array<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    CompactFormatter.begin_array(&mut self.scope(writer))
  }

  #[inline]
  fn end_array<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    CompactFormatter.end_array(&mut self.scope(writer))
  }

  #[inline]
  fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    CompactFormatter.begin_array_value(&mut self.scope(writer), first)
  }

  #[inline]
  fn end_array_value<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    CompactFormatter.end_array_value(&mut self.scope(writer))
  }

  #[inline]
  fn begin_object<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    CompactFormatter.begin_object(&mut self.scope(writer))?;

    self.0.push(Entry::new());

    Ok(())
  }

  fn end_object<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    let entry: Entry = self
      .0
      .pop()
      .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "oh no"))?;

    let mut scope = self.scope(writer);
    let mut first = true;

    for (key, val) in entry.object {
      CompactFormatter.begin_object_key(&mut scope, first)?;
      scope.write_all(&key)?;
      CompactFormatter.end_object_key(&mut scope)?;

      CompactFormatter.begin_object_value(&mut scope)?;
      scope.write_all(&val)?;
      CompactFormatter.end_object_value(&mut scope)?;

      first = false;
    }

    CompactFormatter.end_object(&mut scope)
  }

  #[inline]
  fn begin_object_key<W>(&mut self, _writer: &mut W, _first: bool) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.entry_mut().map(|entry| {
      entry.complete = false;
    })
  }

  #[inline]
  fn end_object_key<W>(&mut self, _writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.entry_mut().map(|entry| {
      entry.complete = true;
    })
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
    let entry: &mut Entry = self.entry_mut()?;

    let key: Vec<u8> = replace(&mut entry.next_key, Vec::new());
    let val: Vec<u8> = replace(&mut entry.next_val, Vec::new());

    entry.object.insert(key, val);

    Ok(())
  }
}
