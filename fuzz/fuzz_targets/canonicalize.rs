#![no_main]

use libfuzzer_sys::fuzz_target;
use serde_jcs::to_string;
use serde_json::Error;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;
use serde_json::from_str;
use serde_json::value::RawValue;
use serde_json::value::to_raw_value;
use std::collections::BTreeMap;
use std::str;

type Object = Map<String, Value>;

fuzz_target!(|data: &[u8]| {
  let Ok(text) = str::from_utf8(data) else {
    return;
  };

  let Ok(raw) = from_str::<&RawValue>(text) else {
    return;
  };

  let jcs: Result<String, Error> = to_string(raw);
  let map: Result<String, Error> = to_string(&BTreeMap::from([("", raw)]));

  match (&jcs, map) {
    (Ok(jcs), Ok(map)) => assert_eq!(map, format!(r#"{{"":{jcs}}}"#)),
    (Err(_), Err(_)) => {}
    (_, _) => panic!("nested RawValue differs"),
  }

  let Ok(value) = from_str::<Value>(text) else {
    assert!(jcs.is_err(), "RawValue accepted text that Value rejects");
    return;
  };

  let Ok(output) = to_string(&value) else {
    assert!(jcs.is_err(), "RawValue accepted a rejected Value");
    return;
  };

  match jcs {
    Ok(jcs) => assert_eq!(jcs, output),
    Err(_) => assert_eq!(to_string(&to_raw_value(&value).unwrap()).unwrap(), output),
  }

  let parsed: Value = from_str(&output).unwrap();

  assert_eq!(to_string(&parsed).unwrap(), output);
  assert!(eq(&value, &parsed), "output changed the value");
  assert!(sorted(&parsed), "output is not sorted: {output}");
});

// -----------------------------------------------------------------------------
// Sort Helpers
// -----------------------------------------------------------------------------

fn sorted(value: &Value) -> bool {
  match value {
    Value::Array(value) => sorted_array(value),
    Value::Object(value) => sorted_object(value),
    _ => true,
  }
}

fn sorted_array(value: &[Value]) -> bool {
  value.iter().all(sorted)
}

fn sorted_object(value: &Object) -> bool {
  let ordered: bool = value
    .keys()
    .is_sorted_by(|lhs, rhs| lhs.encode_utf16().lt(rhs.encode_utf16()));

  ordered && value.values().all(sorted)
}

// -----------------------------------------------------------------------------
// Eq Helpers
// -----------------------------------------------------------------------------

fn eq(lhs: &Value, rhs: &Value) -> bool {
  match (lhs, rhs) {
    (Value::Number(lhs), Value::Number(rhs)) => eq_number(lhs, rhs),
    (Value::Array(lhs), Value::Array(rhs)) => eq_array(lhs, rhs),
    (Value::Object(lhs), Value::Object(rhs)) => eq_object(lhs, rhs),
    (lhs, rhs) => lhs == rhs,
  }
}

fn eq_number(lhs: &Number, rhs: &Number) -> bool {
  lhs.as_f64() == rhs.as_f64()
}

fn eq_array(lhs: &[Value], rhs: &[Value]) -> bool {
  if lhs.len() != rhs.len() {
    return false;
  }

  lhs.iter().zip(rhs).all(|(lhs, rhs)| eq(lhs, rhs))
}

fn eq_object(lhs: &Object, rhs: &Object) -> bool {
  if lhs.len() != rhs.len() {
    return false;
  }

  lhs
    .iter()
    .all(|(key, lhs)| rhs.get(key).is_some_and(|rhs| eq(lhs, rhs)))
}
