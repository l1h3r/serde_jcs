//! JSON Canonicalization Scheme ([RFC 8785]) for [Serde].
//!
//! ```
//! use serde_jcs::to_string;
//! use serde_json::json;
//!
//! let value = json!({ "b": [1.50, "\u{20ac}"], "a": null });
//! assert_eq!(to_string(&value).unwrap(), r#"{"a":null,"b":[1.5,"€"]}"#);
//! ```
//!
//! [RFC 8785]: https://www.rfc-editor.org/rfc/rfc8785
//! [Serde]: ::serde

use serde::Serialize;
use serde::ser::Error as _;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde::ser::SerializeStruct;
use serde::ser::SerializeStructVariant;
use serde::ser::SerializeTuple;
use serde::ser::SerializeTupleStruct;
use serde::ser::SerializeTupleVariant;
use serde::ser::Serializer;
use serde_json::Error;
use serde_json::Serializer as JsonSerializer;
use serde_json::Value as JsonValue;
use serde_json::from_str;
use serde_json::ser::CharEscape;
use serde_json::ser::CompactFormatter;
use serde_json::ser::Formatter as JsonFormatter;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::io;
use std::io::Write;
use std::mem::take;
use std::str::from_utf8;

/// The largest integer magnitude that can be serialized as a JSON number,
/// `2^53 - 1`.
///
/// Equal to ECMAScript's `Number.MAX_SAFE_INTEGER`.
pub const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

/// Serializes `value` as a canonical JSON [`String`].
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` fails, or if
/// `value` contains a non-finite float, a duplicate object key, or an integer
/// value outside ±[`MAX_SAFE_INTEGER`].
#[inline]
pub fn to_string<T>(value: &T) -> Result<String, Error>
where
  T: Serialize + ?Sized,
{
  let data: Vec<u8> = to_vec(value)?;

  // SAFETY: We only emit valid UTF-8.
  let data: String = unsafe { String::from_utf8_unchecked(data) };

  Ok(data)
}

/// Serializes `value` as canonical JSON bytes.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` fails, or if
/// `value` contains a non-finite float, a duplicate object key, or an integer
/// value outside ±[`MAX_SAFE_INTEGER`].
#[inline]
pub fn to_vec<T>(value: &T) -> Result<Vec<u8>, Error>
where
  T: Serialize + ?Sized,
{
  let mut data: Vec<u8> = Vec::with_capacity(128);

  to_writer(&mut data, value)?;

  Ok(data)
}

/// Serializes `value` as canonical JSON into `writer`.
///
/// # Errors
///
/// Serialization can fail if `T`'s implementation of `Serialize` fails, or if
/// `value` contains a non-finite float, a duplicate object key, or an integer
/// value outside ±[`MAX_SAFE_INTEGER`].
#[inline]
pub fn to_writer<W, T>(writer: W, value: &T) -> Result<(), Error>
where
  W: Write,
  T: Serialize + ?Sized,
{
  let mut serializer: JsonSerializer<W, JcsFormatter> =
    JsonSerializer::with_formatter(writer, JcsFormatter::new());

  JcsValue(value).serialize(&mut serializer)
}

// -----------------------------------------------------------------------------
// Errors
// -----------------------------------------------------------------------------

#[inline]
fn duplicate_key() -> io::Error {
  io::Error::other("duplicate object key")
}

#[inline]
fn invalid_float() -> io::Error {
  io::Error::other("invalid float value")
}

#[inline]
fn invalid_key() -> io::Error {
  io::Error::other("invalid UTF-8 key")
}

#[inline]
fn missing_object(event: &str) -> io::Error {
  io::Error::other(format!("{event} called before begin_object"))
}

#[inline]
fn unsafe_integer() -> io::Error {
  io::Error::other("integer outside safe range")
}

// -----------------------------------------------------------------------------
// JCS Formatter
// -----------------------------------------------------------------------------

macro_rules! forward_scope {
  ($($name:ident($($arg:ident: $ty:ty),*);)*) => {
    $(
      #[inline]
      fn $name<W>(&mut self, writer: &mut W, $($arg: $ty),*) -> io::Result<()>
      where
        W: Write + ?Sized,
      {
        CompactFormatter.$name(&mut self.scope(writer), $($arg),*)
      }
    )*
  };
}

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
  fn scope<'a, W>(&'a mut self, writer: &'a mut W) -> Scope<'a, W>
  where
    W: Write + ?Sized,
  {
    match self.entry_mut() {
      Some(entry) if entry.complete => Scope::Buffer(&mut entry.next_val),
      Some(entry) => Scope::Buffer(&mut entry.next_key),
      None => Scope::Writer(writer),
    }
  }

  #[inline]
  fn entry_mut(&mut self) -> Option<&mut Entry> {
    self.entries.last_mut()
  }

  #[inline]
  fn in_key(&self) -> bool {
    self.entries.last().is_some_and(|entry| !entry.complete)
  }

  #[inline]
  fn check_integer(&self, magnitude: u128) -> io::Result<()> {
    if self.in_key() || magnitude <= u128::from(MAX_SAFE_INTEGER) {
      Ok(())
    } else {
      Err(unsafe_integer())
    }
  }
}

impl JsonFormatter for JcsFormatter {
  forward_scope! {
    write_null();
    write_bool(value: bool);
    write_char_escape(escape: CharEscape);
    write_string_fragment(fragment: &str);
    write_i8(value: i8);
    write_i16(value: i16);
    write_i32(value: i32);
    write_u8(value: u8);
    write_u16(value: u16);
    write_u32(value: u32);
    begin_string();
    end_string();
    begin_array();
    end_array();
    begin_array_value(first: bool);
  }

  #[inline]
  fn write_number_str<W>(&mut self, writer: &mut W, value: &str) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.write_f64(writer, value.parse().map_err(io::Error::other)?)
  }

  #[inline]
  fn write_raw_fragment<W>(&mut self, writer: &mut W, fragment: &str) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    let mut scope: Scope<'_, W> = self.scope(writer);
    let value: JsonValue = from_str(fragment)?;

    to_writer(&mut scope as &mut dyn Write, &value).map_err(Into::into)
  }

  #[inline]
  fn write_i64<W>(&mut self, writer: &mut W, value: i64) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.check_integer(value.unsigned_abs().into())?;
    CompactFormatter.write_i64(&mut self.scope(writer), value)
  }

  #[inline]
  fn write_i128<W>(&mut self, writer: &mut W, value: i128) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.check_integer(value.unsigned_abs())?;
    CompactFormatter.write_i128(&mut self.scope(writer), value)
  }

  #[inline]
  fn write_u64<W>(&mut self, writer: &mut W, value: u64) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.check_integer(value.into())?;
    CompactFormatter.write_u64(&mut self.scope(writer), value)
  }

  #[inline]
  fn write_u128<W>(&mut self, writer: &mut W, value: u128) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.check_integer(value)?;
    CompactFormatter.write_u128(&mut self.scope(writer), value)
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
    #[expect(
      clippy::cast_possible_truncation,
      clippy::cast_precision_loss,
      reason = "exact in safe range"
    )]
    if value.fract() == 0.0 && value.abs() <= MAX_SAFE_INTEGER as f64 {
      return CompactFormatter.write_i64(&mut self.scope(writer), value as i64);
    }

    if value.is_finite() {
      let mut buffer: ryu_js::Buffer = ryu_js::Buffer::new();

      self
        .scope(writer)
        .write_all(buffer.format_finite(value).as_bytes())
    } else {
      Err(invalid_float())
    }
  }

  #[inline]
  fn begin_object<W>(&mut self, _writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self.entries.push(Entry::new());

    Ok(())
  }

  #[inline]
  fn end_object<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    let entry: Entry = self
      .entries
      .pop()
      .ok_or_else(|| missing_object("end_object"))?;

    let mut scope: Scope<'_, W> = self.scope(writer);

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
      .ok_or_else(|| missing_object("begin_object_key"))
      .map(|entry| entry.complete(false))
  }

  #[inline]
  fn end_object_key<W>(&mut self, _writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    self
      .entry_mut()
      .ok_or_else(|| missing_object("end_object_key"))
      .map(|entry| entry.complete(true))
  }

  #[inline]
  fn begin_object_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    Ok(())
  }

  #[inline]
  fn end_object_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    let entry: &mut Entry = self
      .entry_mut()
      .ok_or_else(|| missing_object("end_object_value"))?;

    let key: Vec<u8> = take(&mut entry.next_key);
    let val: Vec<u8> = take(&mut entry.next_val);

    if entry.object.insert(EntryKey::new(key)?, val).is_some() {
      return Err(duplicate_key());
    }

    Ok(())
  }
}

// -----------------------------------------------------------------------------
// JCS Formatter - Scope
// -----------------------------------------------------------------------------

enum Scope<'a, W>
where
  W: Write + ?Sized,
{
  Buffer(&'a mut Vec<u8>),
  Writer(&'a mut W),
}

impl<W> Write for Scope<'_, W>
where
  W: Write + ?Sized,
{
  #[inline]
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    match self {
      Self::Buffer(buffer) => buffer.write(buf),
      Self::Writer(writer) => writer.write(buf),
    }
  }

  #[inline]
  fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
    match self {
      Self::Buffer(buffer) => buffer.write_all(buf),
      Self::Writer(writer) => writer.write_all(buf),
    }
  }

  #[inline]
  fn flush(&mut self) -> io::Result<()> {
    match self {
      Self::Buffer(buffer) => buffer.flush(),
      Self::Writer(writer) => writer.flush(),
    }
  }
}

// -----------------------------------------------------------------------------
// JCS Formatter - Entry
// -----------------------------------------------------------------------------

struct Entry {
  object: BTreeMap<EntryKey, Vec<u8>>,
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
// JCS Formatter - Entry Key
// -----------------------------------------------------------------------------

struct EntryKey {
  tag: Vec<u16>,
  key: Vec<u8>,
}

impl EntryKey {
  fn new(key: Vec<u8>) -> io::Result<Self> {
    #[inline]
    fn unescape(text: &str) -> Option<(u16, &str)> {
      let escape: u8 = *text.as_bytes().first()?;
      let text: &str = text.get(1..)?;

      match escape {
        b'"' | b'\\' | b'/' => Some((u16::from(escape), text)),
        b'b' => Some((0x08, text)),
        b'f' => Some((0x0C, text)),
        b'n' => Some((0x0A, text)),
        b'r' => Some((0x0D, text)),
        b't' => Some((0x09, text)),
        b'u' => {
          let (code, text): (&str, &str) = text.split_at_checked(4)?;
          let unit: u16 = u16::from_str_radix(code, 16).ok()?;

          Some((unit, text))
        }
        _ => None,
      }
    }

    let mut text: &str = from_utf8(&key)
      .ok()
      .and_then(|text| text.strip_prefix('"'))
      .and_then(|text| text.strip_suffix('"'))
      .ok_or_else(invalid_key)?;

    let mut tag: Vec<u16> = Vec::with_capacity(text.len());

    while let Some((head, tail)) = text.split_once('\\') {
      let (unit, tail): (u16, &str) = unescape(tail).ok_or_else(invalid_key)?;

      tag.extend(head.encode_utf16());
      tag.push(unit);
      text = tail;
    }

    tag.extend(text.encode_utf16());

    Ok(Self { tag, key })
  }
}

impl PartialEq for EntryKey {
  #[inline]
  fn eq(&self, other: &Self) -> bool {
    self.tag.eq(&other.tag)
  }
}

impl Eq for EntryKey {}

impl PartialOrd for EntryKey {
  #[inline]
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for EntryKey {
  #[inline]
  fn cmp(&self, other: &Self) -> Ordering {
    self.tag.cmp(&other.tag)
  }
}

// -----------------------------------------------------------------------------
// JCS Value
// -----------------------------------------------------------------------------

struct JcsValue<'a, T>(&'a T)
where
  T: ?Sized;

impl<T> Serialize for JcsValue<'_, T>
where
  T: Serialize + ?Sized,
{
  #[inline]
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    self.0.serialize(JcsSerializer(serializer))
  }
}

// -----------------------------------------------------------------------------
// JCS Serializer
// -----------------------------------------------------------------------------

macro_rules! forward {
  ($($name:ident($($arg:ident: $ty:ty),*);)*) => {
    $(
      #[inline]
      fn $name(self, $($arg: $ty),*) -> Result<Self::Ok, Self::Error> {
        self.0.$name($($arg),*)
      }
    )*
  };
}

macro_rules! forward_float {
  ($($name:ident($ty:ty);)*) => {
    $(
      #[inline]
      fn $name(self, value: $ty) -> Result<Self::Ok, Self::Error> {
        if value.is_finite() {
          self.0.$name(value)
        } else {
          Err(Self::Error::custom($crate::invalid_float()))
        }
      }
    )*
  };
}

macro_rules! forward_compound {
  ($($name:ident($($arg:ident: $ty:ty),*) -> $assoc:ident;)*) => {
    $(
      #[inline]
      fn $name(self, $($arg: $ty),*) -> Result<Self::$assoc, Self::Error> {
        self.0.$name($($arg),*).map($crate::JcsSerializer)
      }
    )*
  };
}

macro_rules! forward_element {
  ($($trait:ident::$method:ident($($arg:ident: $ty:ty),*);)*) => {
    $(
      impl<S> $trait for $crate::JcsSerializer<S>
      where
        S: $trait,
      {
        type Ok = S::Ok;
        type Error = S::Error;

        #[inline]
        fn $method<T>(&mut self, $($arg: $ty,)* value: &T) -> Result<(), Self::Error>
        where
          T: ::serde::Serialize + ?Sized,
        {
          self.0.$method($($arg,)* &JcsValue(value))
        }

        #[inline]
        fn end(self) -> Result<Self::Ok, Self::Error> {
          self.0.end()
        }
      }
    )*
  };
}

struct JcsSerializer<S>(S);

impl<S> Serializer for JcsSerializer<S>
where
  S: Serializer,
{
  type Ok = S::Ok;
  type Error = S::Error;
  type SerializeSeq = JcsSerializer<S::SerializeSeq>;
  type SerializeTuple = JcsSerializer<S::SerializeTuple>;
  type SerializeTupleStruct = JcsSerializer<S::SerializeTupleStruct>;
  type SerializeTupleVariant = JcsSerializer<S::SerializeTupleVariant>;
  type SerializeMap = JcsSerializer<S::SerializeMap>;
  type SerializeStruct = JcsSerializer<S::SerializeStruct>;
  type SerializeStructVariant = JcsSerializer<S::SerializeStructVariant>;

  forward! {
    serialize_bool(value: bool);
    serialize_i8(value: i8);
    serialize_i16(value: i16);
    serialize_i32(value: i32);
    serialize_i64(value: i64);
    serialize_i128(value: i128);
    serialize_u8(value: u8);
    serialize_u16(value: u16);
    serialize_u32(value: u32);
    serialize_u64(value: u64);
    serialize_u128(value: u128);
    serialize_char(value: char);
    serialize_str(value: &str);
    serialize_bytes(value: &[u8]);
    serialize_none();
    serialize_unit();
    serialize_unit_struct(name: &'static str);
    serialize_unit_variant(name: &'static str, index: u32, variant: &'static str);
  }

  forward_float! {
    serialize_f32(f32);
    serialize_f64(f64);
  }

  forward_compound! {
    serialize_seq(len: Option<usize>) -> SerializeSeq;
    serialize_tuple(len: usize) -> SerializeTuple;
    serialize_tuple_struct(name: &'static str, len: usize) -> SerializeTupleStruct;
    serialize_tuple_variant(
      name: &'static str,
      index: u32,
      variant: &'static str,
      len: usize
    ) -> SerializeTupleVariant;
    serialize_map(len: Option<usize>) -> SerializeMap;
    serialize_struct(name: &'static str, len: usize) -> SerializeStruct;
    serialize_struct_variant(
      name: &'static str,
      index: u32,
      variant: &'static str,
      len: usize
    ) -> SerializeStructVariant;
  }

  #[inline]
  fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
  where
    T: Serialize + ?Sized,
  {
    self.0.serialize_some(&JcsValue(value))
  }

  #[inline]
  fn serialize_newtype_struct<T>(
    self,
    name: &'static str,
    value: &T,
  ) -> Result<Self::Ok, Self::Error>
  where
    T: Serialize + ?Sized,
  {
    self.0.serialize_newtype_struct(name, &JcsValue(value))
  }

  #[inline]
  fn serialize_newtype_variant<T>(
    self,
    name: &'static str,
    index: u32,
    variant: &'static str,
    value: &T,
  ) -> Result<Self::Ok, Self::Error>
  where
    T: Serialize + ?Sized,
  {
    self
      .0
      .serialize_newtype_variant(name, index, variant, &JcsValue(value))
  }

  #[inline]
  fn is_human_readable(&self) -> bool {
    self.0.is_human_readable()
  }
}

forward_element! {
  SerializeSeq::serialize_element();
  SerializeTuple::serialize_element();
  SerializeTupleStruct::serialize_field();
  SerializeTupleVariant::serialize_field();
  SerializeStruct::serialize_field(key: &'static str);
  SerializeStructVariant::serialize_field(key: &'static str);
}

impl<S> SerializeMap for JcsSerializer<S>
where
  S: SerializeMap,
{
  type Ok = S::Ok;
  type Error = S::Error;

  #[inline]
  fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
  where
    T: Serialize + ?Sized,
  {
    self.0.serialize_key(&JcsValue(key))
  }

  #[inline]
  fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
  where
    T: Serialize + ?Sized,
  {
    self.0.serialize_value(&JcsValue(value))
  }

  #[inline]
  fn end(self) -> Result<Self::Ok, Self::Error> {
    self.0.end()
  }
}
