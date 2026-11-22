#[path = "common/types.rs"]
mod types;
#[path = "common/utils.rs"]
mod utils;

use serde_jcs::MAX_SAFE_INTEGER;
use serde_jcs::to_string;
use serde_jcs::to_vec;
use serde_jcs::to_writer;
use serde_json::Error;
use serde_json::Value;
use serde_json::error::Category;
use serde_json::value::RawValue;
use std::collections::BTreeMap;

use crate::types::Entries;
use crate::types::FailWriter;
use crate::types::IgnoredElement;
use crate::types::IgnoredError;
use crate::types::IgnoredNested;
use crate::types::Shape;
use crate::types::WRITE_FAILED;
use crate::utils::DUPLICATE_KEY;
use crate::utils::IGNORED_ERROR;
use crate::utils::INCOMPLETE_ENTRY;
use crate::utils::INVALID_FLOAT;
use crate::utils::NON_STRING_KEY;
use crate::utils::UNSAFE_INTEGER;
use crate::utils::assert_error;
use crate::utils::assert_json;
use crate::utils::assert_json_category;
use crate::utils::assert_json_err;
use crate::utils::raw;
use crate::utils::value;

const OUTPUT: &str = r#"{"b":[1,2.5,{"z":null,"y":"é"}],"a":true}"#;

fn map<K, V, const N: usize>(entries: [(K, V); N]) -> BTreeMap<K, V>
where
  K: Ord,
{
  BTreeMap::from(entries)
}

#[test]
fn test_integers() {
  assert_json(i8::MIN, "-128");
  assert_json(i16::MIN, "-32768");
  assert_json(i32::MIN, "-2147483648");
  assert_json(u8::MAX, "255");
  assert_json(u16::MAX, "65535");
  assert_json(u32::MAX, "4294967295");
}

#[test]
fn test_integers_safe() {
  let max: u64 = MAX_SAFE_INTEGER;
  let min: i64 = -9_007_199_254_740_991;
  let parsed: Value = value("[9007199254740991,-9007199254740991,-0]");

  assert_json(max, "9007199254740991");
  assert_json(min, "-9007199254740991");
  assert_json(i128::from(max), "9007199254740991");
  assert_json(i128::from(min), "-9007199254740991");
  assert_json(u128::from(max), "9007199254740991");
  assert_json(parsed, "[9007199254740991,-9007199254740991,0]");
}

#[test]
fn test_integers_same_digits() {
  let parsed: &str = "[9007199254740992,-9007199254740994,1152921504606847000]";

  assert_json(MAX_SAFE_INTEGER + 1, "9007199254740992");
  assert_json(-9_007_199_254_740_994_i64, "-9007199254740994");
  assert_json(1_000_000_000_000_000_000_u64, "1000000000000000000");
  assert_json(1_152_921_504_606_847_000_u64, "1152921504606847000");
  assert_json(295_147_905_179_352_830_000_u128, "295147905179352830000");
  assert_json(-295_147_905_179_352_830_000_i128, "-295147905179352830000");
  assert_json(value(parsed), parsed);
  assert_json(raw(parsed), parsed);
}

#[test]
fn test_integers_unsafe() {
  let max: u64 = MAX_SAFE_INTEGER + 2;
  let min: i64 = -9_007_199_254_740_993;

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
  assert_json_err(value("9007199254740993"), UNSAFE_INTEGER);
  assert_json_err(value("-9007199254740993"), UNSAFE_INTEGER);
  assert_json_err(value("1152921504606846976"), UNSAFE_INTEGER);
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
  assert_json_err(f64::NAN, INVALID_FLOAT);
  assert_json_err(f32::NAN, INVALID_FLOAT);
  assert_json_err(Some(f64::NAN), INVALID_FLOAT);
  assert_json_err((1_u8, f64::NAN), INVALID_FLOAT);
  assert_json_err(vec![f64::NAN], INVALID_FLOAT);
  assert_json_err(vec![f32::INFINITY], INVALID_FLOAT);
  assert_json_err(vec![vec![f64::NAN]], INVALID_FLOAT);
  assert_json_err(map([("a", f64::NAN)]), INVALID_FLOAT);
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
  assert_json(Entries([(1.5_f64, 1)]), r#"{"1.5":1}"#);
  assert_json(Entries([(1e21_f64, 1)]), r#"{"1e+21":1}"#);
  assert_json(Entries([(1.1_f32, 1)]), r#"{"1.100000023841858":1}"#);
  assert_json_err(Entries([(f64::NAN, 1)]), INVALID_FLOAT);
}

#[test]
fn test_keys_other() {
  assert_json(map([(true, 1)]), r#"{"true":1}"#);
  assert_json(map([('x', 1)]), r#"{"x":1}"#);
  assert_json_err(map([(vec![1_u8], 1)]), NON_STRING_KEY);
  assert_json_err(Entries([((), 1)]), NON_STRING_KEY);
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
  let unique: Entries<&str, u8, 3> = Entries([("a", 1), ("c", 3), ("b", 2)]);
  let duplicate: Entries<&str, u8, 2> = Entries([("a", 1), ("a", 2)]);

  assert_json(unique, r#"{"a":1,"b":2,"c":3}"#);
  assert_json_err(&duplicate, DUPLICATE_KEY);
  assert_json_err([&duplicate], DUPLICATE_KEY);
  assert_json_err(map([("x", &duplicate)]), DUPLICATE_KEY);
}

#[test]
fn test_objects_incomplete_entry() {
  let inner: IgnoredError = IgnoredError { next_key: false };
  let nested: BTreeMap<&str, IgnoredError> = map([("x", inner)]);

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
fn test_structs() {
  let object: Shape<f64> = Shape::Struct(vec![("b", 1.0), ("a", 1.5)]);

  assert_json(Shape::<u8>::UnitStruct, "null");
  assert_json(Shape::NewtypeStruct(7), "7");
  assert_json(Shape::TupleStruct(vec![2.5, 1.0]), "[2.5,1]");
  assert_json(object, r#"{"a":1.5,"b":1}"#);
}

#[test]
fn test_structs_non_finite() {
  assert_json_err(Shape::NewtypeStruct(f64::NAN), INVALID_FLOAT);
  assert_json_err(Shape::TupleStruct(vec![1.0, f64::NAN]), INVALID_FLOAT);
  assert_json_err(Shape::Struct(vec![("x", f64::NEG_INFINITY)]), INVALID_FLOAT);
}

#[test]
fn test_enums() {
  let object: Shape<f64> = Shape::StructVariant(vec![("b", 1.0), ("a", 1.5)]);

  assert_json(Shape::<u8>::UnitVariant, r#""Unit""#);
  assert_json(Shape::NewtypeVariant(1.5), r#"{"Newtype":1.5}"#);
  assert_json(Shape::TupleVariant(vec![1.0, 1.5]), r#"{"Tuple":[1,1.5]}"#);
  assert_json(object, r#"{"Struct":{"a":1.5,"b":1}}"#);
}

#[test]
fn test_enums_non_finite() {
  let object: Shape<f64> = Shape::StructVariant(vec![("b", 1.0), ("a", f64::NAN)]);

  assert_json_err(Shape::NewtypeVariant(f64::NAN), INVALID_FLOAT);
  assert_json_err(Shape::TupleVariant(vec![1.0, f64::NAN]), INVALID_FLOAT);
  assert_json_err(object, INVALID_FLOAT);
}

#[test]
fn test_raw_value() {
  let value: Box<RawValue> = raw(r#"{"b":[2.50,1e3],"a":"€"}"#);
  let object: Box<RawValue> = raw(r#"{"y":1e3,"x":2.50}"#);
  let field: Shape<Box<RawValue>> = Shape::Struct(vec![("b", object), ("a", raw("1"))]);

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
  let object: Box<RawValue> = raw(r#"{"a":1,"a":2}"#);
  let field: Shape<Box<RawValue>> = Shape::Struct(vec![("b", object), ("a", raw("1"))]);

  assert_json_err(raw(r#"{"a":1,"a":2}"#), DUPLICATE_KEY);
  assert_json_err(raw(r#"[{"x":{"a":1,"a":2}}]"#), DUPLICATE_KEY);
  assert_json_err(field, DUPLICATE_KEY);
}

#[test]
fn test_output_apis() {
  let input: Value = value(OUTPUT);
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
  let received: Error = to_writer(&mut written, &[1.0, f64::NAN]).unwrap_err();

  assert_error(&received, INVALID_FLOAT);
  assert_eq!(written, b"[1,");
}

#[test]
fn test_output_writer_error() {
  let input: Value = value(OUTPUT);
  let length: usize = to_vec(&input).unwrap().len();

  for limit in 0..length {
    let received: Error = to_writer(FailWriter(limit), &input).unwrap_err();

    assert_error(&received, WRITE_FAILED);
    assert_eq!(received.classify(), Category::Io);
  }

  assert!(to_writer(FailWriter(length), &input).is_ok());
}

#[test]
fn test_error_categories() {
  let duplicate: Entries<&str, u8, 2> = Entries([("a", 1), ("a", 2)]);

  assert_json_category(f64::NAN, Category::Data);
  assert_json_category(u64::MAX, Category::Data);
  assert_json_category(duplicate, Category::Data);
  assert_json_category(IgnoredElement, Category::Data);
  assert_json_category(IgnoredError { next_key: true }, Category::Data);
  assert_json_category(raw("[1e400]"), Category::Data);
  assert_json_category(raw("[9007199254740993]"), Category::Data);
  assert_json_category(raw(r#"{"a":1,"a":2}"#), Category::Data);
}
