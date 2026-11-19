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

use ryu_js::Buffer;
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
use serde_json::Value;
use serde_json::from_slice;
use serde_json::from_str;
use serde_json::ser::CharEscape;
use serde_json::ser::Formatter;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::io;
use std::io::Write;
use std::mem::take;

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
          Err(Self::Error::custom(invalid_float()))
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
        self.0.$name($($arg),*).map(JcsSerializer)
      }
    )*
  };
}

macro_rules! forward_element {
  ($($trait:ident::$method:ident($($arg:ident: $ty:ty),*);)*) => {
    $(
      impl<S> $trait for JcsSerializer<S>
      where
        S: $trait,
      {
        type Ok = S::Ok;
        type Error = S::Error;

        #[inline]
        fn $method<T>(&mut self, $($arg: $ty,)* value: &T) -> Result<(), Self::Error>
        where
          T: Serialize + ?Sized,
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
/// `value` contains a non-finite float or an integer outside
/// ±[`MAX_SAFE_INTEGER`].
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
/// `value` contains a non-finite float or an integer outside
/// ±[`MAX_SAFE_INTEGER`].
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
/// `value` contains a non-finite float or an integer outside
/// ±[`MAX_SAFE_INTEGER`].
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
      .ok_or_else(invalid_key)?
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
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for Utf16Key {
  #[inline]
  fn cmp(&self, other: &Self) -> Ordering {
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
    if value.unsigned_abs() > MAX_SAFE_INTEGER {
      return Err(unsafe_integer());
    }

    self.write_f64(writer, value as f64)
  }

  #[expect(clippy::cast_precision_loss)]
  #[inline]
  fn write_i128<W>(&mut self, writer: &mut W, value: i128) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    if value.unsigned_abs() > u128::from(MAX_SAFE_INTEGER) {
      return Err(unsafe_integer());
    }

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
    if value > MAX_SAFE_INTEGER {
      return Err(unsafe_integer());
    }

    self.write_f64(writer, value as f64)
  }

  #[expect(clippy::cast_precision_loss)]
  #[inline]
  fn write_u128<W>(&mut self, writer: &mut W, value: u128) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    if value > u128::from(MAX_SAFE_INTEGER) {
      return Err(unsafe_integer());
    }

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
    if value.is_finite() {
      let mut buffer: Buffer = Buffer::new();
      let mut writer: Box<dyn Write> = self.scope(writer);

      writer.write_all(buffer.format_finite(value).as_bytes())
    } else {
      Err(invalid_float())
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

  #[inline]
  fn end_object<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: Write + ?Sized,
  {
    let entry: Entry = self
      .entries
      .pop()
      .ok_or_else(|| missing_object("end_object"))?;

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

    if entry.object.insert(Utf16Key::new(key)?, val).is_some() {
      return Err(duplicate_key());
    }

    Ok(())
  }
}

// -----------------------------------------------------------------------------
// JSON Value
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
// JSON Serializer
// -----------------------------------------------------------------------------

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
