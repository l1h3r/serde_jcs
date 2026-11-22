use serde::Serialize;
use serde::Serializer;
use serde::ser::Error as _;
use serde::ser::SerializeMap as _;
use serde::ser::SerializeSeq as _;
use serde_jcs::MAX_SAFE_INTEGER;
use serde_jcs::to_string;
use serde_jcs::to_vec;
use serde_jcs::to_writer;
use serde_json::Error;
use serde_json::Value;
use serde_json::from_str;
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use std::io;
use std::io::Write;

const DUPLICATE_KEY: &str = "duplicate object key";
const IGNORED_ERROR: &str = "an error was ignored";
const INCOMPLETE_ENTRY: &str = "object entry left incomplete";
const INVALID_FLOAT: &str = "invalid float value";
const NON_STRING_KEY: &str = "key must be a string";
const UNSAFE_INTEGER: &str = "integer outside safe range";
const UNFINISHED: &str = "unfinished";
const WRITE_FAILED: &str = "write failed";

#[derive(Serialize)]
struct Meters(u32);

#[derive(Serialize)]
struct Field {
  x: f64,
}

#[derive(Serialize)]
struct Flatten {
  a: u8,
  #[serde(flatten)]
  rest: BTreeMap<&'static str, u8>,
}

#[derive(Serialize)]
struct Renamed {
  #[serde(rename = "a")]
  z: u8,
  #[serde(skip_serializing_if = "Option::is_none")]
  none: Option<u8>,
  b: u8,
}

#[derive(Serialize)]
struct Raw {
  b: Box<RawValue>,
  a: u8,
}

#[derive(Serialize)]
enum External {
  Unit,
  Newtype(f64),
  Tuple(u8, f64),
  Struct { b: u8, a: f64 },
}

#[derive(Serialize)]
#[serde(tag = "t")]
enum Internal {
  Struct { b: u8, a: f64 },
}

#[derive(Serialize)]
#[serde(tag = "t", content = "c")]
enum Adjacent {
  Newtype(f64),
}

#[derive(Serialize)]
#[serde(untagged)]
enum Untagged {
  Newtype(f64),
}

struct Entry<K>(K);

impl<K> Serialize for Entry<K>
where
  K: Serialize,
{
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut map: S::SerializeMap = serializer.serialize_map(Some(1))?;
    map.serialize_entry(&self.0, &1)?;
    map.end()
  }
}

struct IgnoredError {
  next_key: bool,
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

struct IgnoredElement;

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

struct IgnoredNested;

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

struct Unfinished;

impl Serialize for Unfinished {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut map: S::SerializeMap = serializer.serialize_map(None)?;

    map.serialize_entry("x", &1)?;

    Err(S::Error::custom(UNFINISHED))
  }
}

struct FailWriter(usize);

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

fn map<K, V, const N: usize>(entries: [(K, V); N]) -> BTreeMap<K, V>
where
  K: Ord,
{
  BTreeMap::from(entries)
}

fn flatten<const N: usize>(rest: [(&'static str, u8); N]) -> Flatten {
  Flatten {
    a: 1,
    rest: map(rest),
  }
}

fn raw(input: &str) -> Box<RawValue> {
  RawValue::from_string(input.to_owned()).unwrap()
}

#[track_caller]
fn assert_json<T>(input: T, expected: &str)
where
  T: Serialize,
{
  let received: String = to_string(&input).unwrap();

  assert_eq!(received, expected);
}

#[track_caller]
fn assert_json_err<T>(input: T, message: &str)
where
  T: Serialize,
{
  let received: Error = to_string(&input).unwrap_err();

  assert!(received.to_string().contains(message), "{received}");
}

#[test]
fn test_integers() {
  assert_json(i8::MIN, "-128");
  assert_json(i16::MIN, "-32768");
  assert_json(i32::MIN, "-2147483648");
  assert_json(u8::MAX, "255");
  assert_json(u16::MAX, "65535");
  assert_json(u32::MAX, "4294967295");
  assert_json(Meters(7), "7");
}

#[test]
fn test_integers_safe() {
  let max: u64 = MAX_SAFE_INTEGER;
  let min: i64 = -9_007_199_254_740_991;
  let parsed: Value = from_str("[9007199254740991,-9007199254740991,-0]").unwrap();

  assert_json(max, "9007199254740991");
  assert_json(min, "-9007199254740991");
  assert_json(i128::from(max), "9007199254740991");
  assert_json(i128::from(min), "-9007199254740991");
  assert_json(u128::from(max), "9007199254740991");
  assert_json(parsed, "[9007199254740991,-9007199254740991,0]");
}

#[test]
fn test_integers_same_digits() {
  let text: &str = "[9007199254740992,-9007199254740994,1152921504606847000]";
  let parsed: Value = from_str(text).unwrap();

  assert_json(MAX_SAFE_INTEGER + 1, "9007199254740992");
  assert_json(-9_007_199_254_740_994_i64, "-9007199254740994");
  assert_json(1_000_000_000_000_000_000_u64, "1000000000000000000");
  assert_json(1_152_921_504_606_847_000_u64, "1152921504606847000");
  assert_json(295_147_905_179_352_830_000_u128, "295147905179352830000");
  assert_json(-295_147_905_179_352_830_000_i128, "-295147905179352830000");
  assert_json(parsed, text);
  assert_json(raw(text), text);
}

#[test]
fn test_integers_unsafe() {
  let max: u64 = MAX_SAFE_INTEGER + 2;
  let min: i64 = -9_007_199_254_740_993;
  let rounded: Value = from_str("9007199254740993").unwrap();
  let negative: Value = from_str("-9007199254740993").unwrap();
  let exact: Value = from_str("1152921504606846976").unwrap();

  assert_json_err(max, UNSAFE_INTEGER);
  assert_json_err(min, UNSAFE_INTEGER);
  assert_json_err(i128::from(max), UNSAFE_INTEGER);
  assert_json_err(i128::from(min), UNSAFE_INTEGER);
  assert_json_err(u128::from(max), UNSAFE_INTEGER);
  assert_json_err(1_u64 << 60, UNSAFE_INTEGER);
  assert_json_err(-(1_i64 << 60), UNSAFE_INTEGER);
  assert_json_err(1_000_000_000_000_000_000_000_u128, UNSAFE_INTEGER);
  assert_json_err(i64::MIN, UNSAFE_INTEGER);
  assert_json_err(u64::MAX, UNSAFE_INTEGER);
  assert_json_err(i128::MIN, UNSAFE_INTEGER);
  assert_json_err(u128::MAX, UNSAFE_INTEGER);
  assert_json_err(rounded, UNSAFE_INTEGER);
  assert_json_err(negative, UNSAFE_INTEGER);
  assert_json_err(exact, UNSAFE_INTEGER);
  assert_json_err([1, i64::MIN], UNSAFE_INTEGER);
  assert_json_err(Some(u64::MAX), UNSAFE_INTEGER);
}

#[test]
fn test_floats_f32() {
  assert_json(0.1_f32, "0.10000000149011612");
  assert_json(1.5_f32, "1.5");
  assert_json(f32::MAX, "3.4028234663852886e+38");
}

#[test]
fn test_floats_non_finite() {
  let field: Field = Field {
    x: f64::NEG_INFINITY,
  };

  assert_json_err(f64::NAN, INVALID_FLOAT);
  assert_json_err(f32::NAN, INVALID_FLOAT);
  assert_json_err(Some(f64::NAN), INVALID_FLOAT);
  assert_json_err((1_u8, f64::NAN), INVALID_FLOAT);
  assert_json_err(vec![f64::NAN], INVALID_FLOAT);
  assert_json_err(vec![f32::INFINITY], INVALID_FLOAT);
  assert_json_err(vec![vec![f64::NAN]], INVALID_FLOAT);
  assert_json_err(map([("a", f64::NAN)]), INVALID_FLOAT);
  assert_json_err(field, INVALID_FLOAT);
}

#[test]
fn test_strings_escapes() {
  const INPUT: &str = "\u{0}\u{8}\t\n\u{b}\u{c}\r\u{1f}\u{7f}/\"\\";
  const ESCAPED: &str = "\"\\u0000\\b\\t\\n\\u000b\\f\\r\\u001f\u{7f}/\\\"\\\\\"";

  let object: String = format!("{{{ESCAPED}:1}}");

  assert_json(INPUT, ESCAPED);
  assert_json(map([(INPUT, 1)]), &object);
}

#[test]
fn test_nulls() {
  assert_json((), "null");
  assert_json(None::<u8>, "null");
  assert_json([None, Some(1)], "[null,1]");
}

#[test]
fn test_keys_integer() {
  let i128_min: &str = r#"{"-170141183460469231731687303715884105728":1}"#;
  let u128_max: &str = r#"{"340282366920938463463374607431768211455":1}"#;

  assert_json(map([(10, 1), (9, 2)]), r#"{"10":1,"9":2}"#);
  assert_json(map([(i64::MIN, 1)]), r#"{"-9223372036854775808":1}"#);
  assert_json(map([(u64::MAX, 1)]), r#"{"18446744073709551615":1}"#);
  assert_json(map([(i128::MIN, 1)]), i128_min);
  assert_json(map([(u128::MAX, 1)]), u128_max);
  assert_json_err(map([("a", u64::MAX)]), UNSAFE_INTEGER);
}

#[test]
fn test_keys_float() {
  assert_json(Entry(1.5_f64), r#"{"1.5":1}"#);
  assert_json(Entry(1e21_f64), r#"{"1e+21":1}"#);
  assert_json(Entry(1.1_f32), r#"{"1.100000023841858":1}"#);
  assert!(to_string(&Entry(f64::NAN)).is_err());
}

#[test]
fn test_keys_other() {
  assert_json(map([(true, 1)]), r#"{"true":1}"#);
  assert_json(map([('x', 1)]), r#"{"x":1}"#);
  assert_json_err(map([(vec![1_u8], 1)]), NON_STRING_KEY);
  assert_json_err(Entry(()), NON_STRING_KEY);
}

#[test]
fn test_keys_utf16_order() {
  let input: BTreeMap<&str, u8> = map([("\u{ffff}", 1), ("\u{10000}", 2)]);
  let expected: &str = "{\"\u{10000}\":2,\"\u{ffff}\":1}";

  assert_json(input, expected);
}

#[test]
fn test_keys_escaped() {
  let input: BTreeMap<&str, u8> = map([("\u{c}", 1), ("\t", 2), ("\u{8}", 3)]);

  assert_json(input, r#"{"\b":3,"\t":2,"\f":1}"#);
}

#[test]
fn test_keys_duplicate() {
  assert_json(flatten([("c", 3), ("b", 2)]), r#"{"a":1,"b":2,"c":3}"#);
  assert_json_err(flatten([("a", 2)]), DUPLICATE_KEY);
  assert_json_err([flatten([("a", 2)])], DUPLICATE_KEY);
  assert_json_err(map([("x", flatten([("a", 2)]))]), DUPLICATE_KEY);
}

#[test]
fn test_objects_incomplete_entry() {
  let nested: BTreeMap<&str, IgnoredError> = map([("x", IgnoredError { next_key: false })]);

  assert_json_err(IgnoredError { next_key: true }, INCOMPLETE_ENTRY);
  assert_json_err(IgnoredError { next_key: false }, INCOMPLETE_ENTRY);
  assert_json_err(nested, INCOMPLETE_ENTRY);
}

#[test]
fn test_ignored_errors() {
  assert_json_err(IgnoredElement, IGNORED_ERROR);
  assert_json_err(IgnoredNested, IGNORED_ERROR);
  assert_json_err(vec![IgnoredNested], IGNORED_ERROR);
}

#[test]
fn test_structs_renamed() {
  let input: Renamed = Renamed {
    z: 1,
    none: None,
    b: 2,
  };

  assert_json(input, r#"{"a":1,"b":2}"#);
}

#[test]
fn test_enums_external() {
  let input: External = External::Struct { b: 1, a: 1.5 };

  assert_json(External::Unit, r#""Unit""#);
  assert_json(External::Newtype(1.5), r#"{"Newtype":1.5}"#);
  assert_json(External::Tuple(1, 1.5), r#"{"Tuple":[1,1.5]}"#);
  assert_json(input, r#"{"Struct":{"a":1.5,"b":1}}"#);
}

#[test]
fn test_enums_tagged() {
  let internal: Internal = Internal::Struct { b: 1, a: 1.5 };

  assert_json(internal, r#"{"a":1.5,"b":1,"t":"Struct"}"#);
  assert_json(Adjacent::Newtype(1.5), r#"{"c":1.5,"t":"Newtype"}"#);
  assert_json(Untagged::Newtype(1.5), "1.5");
}

#[test]
fn test_enums_non_finite() {
  let external: External = External::Struct { b: 1, a: f64::NAN };
  let internal: Internal = Internal::Struct { b: 1, a: f64::NAN };

  assert_json_err(External::Newtype(f64::NAN), INVALID_FLOAT);
  assert_json_err(External::Tuple(1, f64::NAN), INVALID_FLOAT);
  assert_json_err(external, INVALID_FLOAT);
  assert_json_err(internal, INVALID_FLOAT);
  assert_json_err(Adjacent::Newtype(f64::NAN), INVALID_FLOAT);
  assert_json_err(Untagged::Newtype(f64::NAN), INVALID_FLOAT);
}

#[test]
fn test_raw_value() {
  let value: Box<RawValue> = raw(r#"{"b":[2.50,1e3],"a":"€"}"#);
  let field: Raw = Raw {
    b: raw(r#"{"y":1e3,"x":2.50}"#),
    a: 1,
  };

  assert_json(value, r#"{"a":"€","b":[2.5,1000]}"#);
  assert_json(field, r#"{"a":1,"b":{"x":2.5,"y":1000}}"#);
}

// Under serde_json/arbitrary_precision, this covers serde_jcs's NUMBER_TOKEN.
#[test]
fn test_raw_value_numbers() {
  let value: Box<RawValue> = raw(r#"[1e3,0.1,-0,9007199254740991,{"n":2.50}]"#);

  assert_json(value, r#"[1000,0.1,0,9007199254740991,{"n":2.5}]"#);
  assert_json_err(raw("9007199254740993"), UNSAFE_INTEGER);
  assert_json_err(raw(r#"{"n":[-9007199254740993]}"#), UNSAFE_INTEGER);
}

#[test]
fn test_raw_value_duplicate_key() {
  let field: Raw = Raw {
    b: raw(r#"{"a":1,"a":2}"#),
    a: 1,
  };

  assert_json_err(raw(r#"{"a":1,"a":2}"#), DUPLICATE_KEY);
  assert_json_err(raw(r#"[{"x":{"a":1,"a":2}}]"#), DUPLICATE_KEY);
  assert_json_err(field, DUPLICATE_KEY);
}

#[test]
fn test_output_apis() {
  let input: Value = from_str(r#"{"b":[1,2.5,{"z":null,"y":"é"}],"a":true}"#).unwrap();
  let string: String = to_string(&input).unwrap();
  let bytes: Vec<u8> = to_vec(&input).unwrap();
  let mut written: Vec<u8> = Vec::new();

  to_writer(&mut written, &input).unwrap();

  assert_eq!(string.as_bytes(), bytes);
  assert_eq!(written, bytes);
}

#[test]
fn test_output_partial() {
  let mut written: Vec<u8> = Vec::new();
  let received: Result<(), Error> = to_writer(&mut written, &[1.0, f64::NAN]);

  assert!(received.is_err());
  assert_eq!(written, b"[1,");
}

#[test]
fn test_output_writer_error() {
  let input: Value = from_str(r#"{"b":[1,{"z":null,"y":"é"}],"a":true}"#).unwrap();
  let length: usize = to_vec(&input).unwrap().len();

  for limit in 0..length {
    let received: Error = to_writer(FailWriter(limit), &input).unwrap_err();

    assert!(received.to_string().contains(WRITE_FAILED), "{received}");
  }

  assert!(to_writer(FailWriter(length), &input).is_ok());
}
