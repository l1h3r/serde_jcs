use serde::Serialize;
use serde_jcs::MAX_SAFE_INTEGER;
use serde_jcs::to_string;
use serde_jcs::to_vec;
use serde_jcs::to_writer;
use serde_json::Error;
use serde_json::Value;
use serde_json::from_str;
use std::collections::BTreeMap;
use std::io;
use std::io::Write;

const DUPLICATE_KEY: &str = "duplicate object key";
const INVALID_FLOAT: &str = "invalid float value";
const NON_STRING_KEY: &str = "key must be a string";
const UNSAFE_INTEGER: &str = "integer outside safe range";

#[derive(Serialize)]
struct Field {
  x: f64,
}

#[derive(Serialize)]
enum Variant {
  Newtype(f64),
}

#[derive(Serialize)]
struct Flatten {
  a: u8,
  #[serde(flatten)]
  rest: BTreeMap<&'static str, u8>,
}

struct FailWriter;

impl Write for FailWriter {
  fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
    Err(io::Error::other("write failed"))
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
  assert_json(MAX_SAFE_INTEGER, "9007199254740991");
  assert_json(9_007_199_254_740_991_i64, "9007199254740991");
  assert_json(-9_007_199_254_740_991_i64, "-9007199254740991");
  assert_json(9_007_199_254_740_991_i128, "9007199254740991");
  assert_json(-9_007_199_254_740_991_i128, "-9007199254740991");
  assert_json(9_007_199_254_740_991_u128, "9007199254740991");
  assert_json(-5_i128, "-5");
}

#[test]
fn test_integers_unsafe() {
  assert_json_err(9_007_199_254_740_992_i64, UNSAFE_INTEGER);
  assert_json_err(-9_007_199_254_740_992_i64, UNSAFE_INTEGER);
  assert_json_err(9_007_199_254_740_992_u64, UNSAFE_INTEGER);
  assert_json_err(9_007_199_254_740_992_i128, UNSAFE_INTEGER);
  assert_json_err(-9_007_199_254_740_992_i128, UNSAFE_INTEGER);
  assert_json_err(9_007_199_254_740_992_u128, UNSAFE_INTEGER);
  assert_json_err(i64::MIN, UNSAFE_INTEGER);
  assert_json_err(u64::MAX, UNSAFE_INTEGER);
  assert_json_err(i128::MIN, UNSAFE_INTEGER);
  assert_json_err(u128::MAX, UNSAFE_INTEGER);
  assert_json_err(
    from_str::<Value>("9007199254740992").unwrap(),
    UNSAFE_INTEGER,
  );
}

#[test]
fn test_floats_f32() {
  assert_json(0.1_f32, "0.10000000149011612");
  assert_json(1.5_f32, "1.5");
  assert_json(f32::MAX, "3.4028234663852886e+38");
}

#[test]
fn test_floats_non_finite() {
  assert_json_err(f64::NAN, INVALID_FLOAT);
  assert_json_err(f32::NAN, INVALID_FLOAT);
  assert_json_err(Some(f64::NAN), INVALID_FLOAT);
  assert_json_err((1_u8, f64::NAN), INVALID_FLOAT);
  assert_json_err(vec![f64::NAN], INVALID_FLOAT);
  assert_json_err(vec![f32::INFINITY], INVALID_FLOAT);
  assert_json_err(vec![vec![f64::NAN]], INVALID_FLOAT);
  assert_json_err(map([("a", f64::NAN)]), INVALID_FLOAT);
  assert_json_err(
    Field {
      x: f64::NEG_INFINITY,
    },
    INVALID_FLOAT,
  );
  assert_json_err(Variant::Newtype(f64::INFINITY), INVALID_FLOAT);
}

#[test]
fn test_string_escapes() {
  const INPUT: &str = "\u{0}\u{8}\t\n\u{b}\u{c}\r\u{1f}\u{7f}/\"\\";
  const ESCAPED: &str = "\"\\u0000\\b\\t\\n\\u000b\\f\\r\\u001f\u{7f}/\\\"\\\\\"";

  assert_json(INPUT, ESCAPED);
  assert_json(map([(INPUT, 1)]), &format!("{{{ESCAPED}:1}}"));
}

#[test]
fn test_keys_other() {
  assert_json(map([(true, 1)]), r#"{"true":1}"#);
  assert_json(map([('x', 1)]), r#"{"x":1}"#);
  assert_json_err(map([(vec![1_u8], 1)]), NON_STRING_KEY);
}

#[test]
fn test_keys_duplicate() {
  let sorted: Flatten = Flatten {
    a: 1,
    rest: map([("c", 3), ("b", 2)]),
  };

  let duplicate: Flatten = Flatten {
    a: 1,
    rest: map([("a", 2)]),
  };

  assert_json(sorted, r#"{"a":1,"b":2,"c":3}"#);
  assert_json_err(duplicate, DUPLICATE_KEY);
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
fn test_output_writer_error() {
  let received: Error = to_writer(FailWriter, &true).unwrap_err();

  assert!(received.to_string().contains("write failed"), "{received}");
}
