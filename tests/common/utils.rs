#![allow(dead_code)]

use serde::Serialize;
use serde_jcs::to_string;
use serde_json::Error;
use serde_json::Value;
use serde_json::from_str;
use serde_json::value::RawValue;

pub(crate) const DUPLICATE_KEY: &str = "duplicate object key";
pub(crate) const IGNORED_ERROR: &str = "an error was ignored";
pub(crate) const INCOMPLETE_ENTRY: &str = "object entry left incomplete";
pub(crate) const INVALID_FLOAT: &str = "invalid float value";
pub(crate) const NON_STRING_KEY: &str = "key must be a string";
pub(crate) const UNSAFE_INTEGER: &str = "integer outside safe range";

pub(crate) fn value(input: &str) -> Value {
  from_str(input).unwrap()
}

pub(crate) fn raw(input: &str) -> Box<RawValue> {
  RawValue::from_string(input.to_owned()).unwrap()
}

pub(crate) fn hex(input: &str) -> Vec<u8> {
  input
    .split_whitespace()
    .map(|byte| u8::from_str_radix(byte, 16).unwrap())
    .collect()
}

#[track_caller]
pub(crate) fn assert_json<T>(input: T, expected: &str)
where
  T: Serialize,
{
  let received: String = to_string(&input).unwrap();

  assert_eq!(received, expected);
}

#[track_caller]
pub(crate) fn assert_json_err<T>(input: T, message: &str)
where
  T: Serialize,
{
  assert_error(&to_string(&input).unwrap_err(), message);
}

#[track_caller]
pub(crate) fn assert_error(error: &Error, message: &str) {
  assert!(error.to_string().contains(message), "{error}");
}
