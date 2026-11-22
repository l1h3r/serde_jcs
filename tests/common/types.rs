use serde::Serialize;
use serde::Serializer;
use serde::ser::Error as _;
use serde::ser::SerializeMap as _;
use serde::ser::SerializeSeq as _;
use serde::ser::SerializeStruct as _;
use serde::ser::SerializeStructVariant as _;
use serde::ser::SerializeTupleStruct as _;
use serde::ser::SerializeTupleVariant as _;
use std::io;
use std::io::Write;

pub(crate) const WRITE_FAILED: &str = "write failed";

type Field<T> = (&'static str, T);

pub(crate) enum Shape<T> {
  UnitStruct,
  NewtypeStruct(T),
  TupleStruct(Vec<T>),
  Struct(Vec<Field<T>>),
  UnitVariant,
  NewtypeVariant(T),
  TupleVariant(Vec<T>),
  StructVariant(Vec<Field<T>>),
}

impl<T> Shape<T>
where
  T: Serialize,
{
  fn tuple_struct<S>(serializer: S, values: &[T]) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut state: S::SerializeTupleStruct =
      serializer.serialize_tuple_struct("Shape", values.len())?;

    for value in values {
      state.serialize_field(value)?;
    }

    state.end()
  }

  fn tuple_variant<S>(serializer: S, values: &[T]) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut state: S::SerializeTupleVariant =
      serializer.serialize_tuple_variant("Shape", 2, "Tuple", values.len())?;

    for value in values {
      state.serialize_field(value)?;
    }

    state.end()
  }

  fn struct_fields<S>(serializer: S, fields: &[Field<T>]) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut state: S::SerializeStruct = serializer.serialize_struct("Shape", fields.len())?;

    for (key, value) in fields {
      state.serialize_field(key, value)?;
    }

    state.end()
  }

  fn struct_variant<S>(serializer: S, fields: &[Field<T>]) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut state: S::SerializeStructVariant =
      serializer.serialize_struct_variant("Shape", 3, "Struct", fields.len())?;

    for (key, value) in fields {
      state.serialize_field(key, value)?;
    }

    state.end()
  }
}

impl<T> Serialize for Shape<T>
where
  T: Serialize,
{
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    match self {
      Self::UnitStruct => serializer.serialize_unit_struct("Shape"),
      Self::NewtypeStruct(value) => serializer.serialize_newtype_struct("Shape", value),
      Self::TupleStruct(values) => Self::tuple_struct(serializer, values),
      Self::Struct(fields) => Self::struct_fields(serializer, fields),
      Self::UnitVariant => serializer.serialize_unit_variant("Shape", 0, "Unit"),
      Self::NewtypeVariant(value) => {
        serializer.serialize_newtype_variant("Shape", 1, "Newtype", value)
      }
      Self::TupleVariant(values) => Self::tuple_variant(serializer, values),
      Self::StructVariant(fields) => Self::struct_variant(serializer, fields),
    }
  }
}

pub(crate) struct Entries<K, V, const N: usize>(pub(crate) [(K, V); N]);

impl<K, V, const N: usize> Serialize for Entries<K, V, N>
where
  K: Serialize,
  V: Serialize,
{
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut map: S::SerializeMap = serializer.serialize_map(None)?;

    for (key, value) in &self.0 {
      map.serialize_entry(key, value)?;
    }

    map.end()
  }
}

pub(crate) struct IgnoredError {
  pub(crate) next_key: bool,
}

impl Serialize for IgnoredError {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut map: S::SerializeMap = serializer.serialize_map(None)?;
    let _ignored: Result<(), S::Error> = map.serialize_entry("k", &f64::NAN);

    if self.next_key {
      map.serialize_entry("z", &1)?;
    }

    map.end()
  }
}

pub(crate) struct IgnoredElement;

impl Serialize for IgnoredElement {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut seq: S::SerializeSeq = serializer.serialize_seq(None)?;

    seq.serialize_element(&1)?;

    let _ignored: Result<(), S::Error> = seq.serialize_element(&f64::NAN);

    seq.serialize_element(&2)?;
    seq.end()
  }
}

pub(crate) struct IgnoredNested;

impl Serialize for IgnoredNested {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut map: S::SerializeMap = serializer.serialize_map(None)?;
    let _ignored: Result<(), S::Error> = map.serialize_entry("k", &Unfinished);

    map.end()
  }
}

pub(crate) struct Unfinished;

impl Serialize for Unfinished {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut map: S::SerializeMap = serializer.serialize_map(None)?;

    map.serialize_entry("x", &1)?;

    Err(S::Error::custom("unfinished"))
  }
}

pub(crate) struct FailWriter(pub(crate) usize);

impl Write for FailWriter {
  fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
    if self.0 == 0 {
      return Err(io::Error::other(WRITE_FAILED));
    }

    let written: usize = buf.len().min(self.0);

    self.0 -= written;

    Ok(written)
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}
