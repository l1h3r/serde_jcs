use serde_jcs::to_string;
use serde_json::Value;
use serde_json::from_str;

#[track_caller]
fn assert_json(input: &str, expected: &str) {
  let json_val: Value = from_str(input).unwrap();
  let received: String = to_string(&json_val).unwrap();

  assert_eq!(received.trim(), expected.trim());
}

#[track_caller]
fn assert_json_number(input: u64, expected: &str) {
  let json_val: f64 = f64::from_bits(input);
  let received: String = to_string(&json_val).unwrap();

  assert_eq!(received, expected);
}

#[test]
fn test_rfc_appendix_b() {
  assert_json_number(0x0000000000000000, "0"); // Zero
  assert_json_number(0x8000000000000000, "0"); // Minus zero
  assert_json_number(0x0000000000000001, "5e-324"); // Min pos number
  assert_json_number(0x8000000000000001, "-5e-324"); // Min neg number
  assert_json_number(0x7fefffffffffffff, "1.7976931348623157e+308"); // Max pos number
  assert_json_number(0xffefffffffffffff, "-1.7976931348623157e+308"); // Max neg number
  assert_json_number(0x4340000000000000, "9007199254740992"); // Max pos int
  assert_json_number(0xc340000000000000, "-9007199254740992"); // Max neg int
  assert_json_number(0x4430000000000000, "295147905179352830000"); // ~2**68

  assert_json_number(0x44b52d02c7e14af5, "9.999999999999997e+22");
  assert_json_number(0x44b52d02c7e14af6, "1e+23");
  assert_json_number(0x44b52d02c7e14af7, "1.0000000000000001e+23");
  assert_json_number(0x444b1ae4d6e2ef4e, "999999999999999700000");
  assert_json_number(0x444b1ae4d6e2ef4f, "999999999999999900000");
  assert_json_number(0x444b1ae4d6e2ef50, "1e+21");
  assert_json_number(0x3eb0c6f7a0b5ed8c, "9.999999999999997e-7");
  assert_json_number(0x3eb0c6f7a0b5ed8d, "0.000001");
  assert_json_number(0x41b3de4355555553, "333333333.3333332");
  assert_json_number(0x41b3de4355555554, "333333333.33333325");
  assert_json_number(0x41b3de4355555555, "333333333.3333333");
  assert_json_number(0x41b3de4355555556, "333333333.3333334");
  assert_json_number(0x41b3de4355555557, "333333333.33333343");
  assert_json_number(0xbecbf647612f3696, "-0.0000033333333333333333");
  assert_json_number(0x43143ff3c1cb0959, "1424953923781206.2"); // Round to even
}

#[test]
fn test_rfc_appendix_c() {
  assert_json(
    include_str!("fixtures/input/appendix_c.json"),
    include_str!("fixtures/output/appendix_c.json"),
  );
}

#[test]
fn test_rfc_appendix_e() {
  assert_json(
    include_str!("fixtures/input/appendix_e.json"),
    include_str!("fixtures/output/appendix_e.json"),
  );
}

#[test]
fn test_rfc_appendix_i_arrays() {
  assert_json(
    include_str!("testdata/input/arrays.json"),
    include_str!("testdata/output/arrays.json"),
  );
}

#[test]
fn test_rfc_appendix_i_french() {
  assert_json(
    include_str!("testdata/input/french.json"),
    include_str!("testdata/output/french.json"),
  );
}

#[test]
fn test_rfc_appendix_i_structures() {
  assert_json(
    include_str!("testdata/input/structures.json"),
    include_str!("testdata/output/structures.json"),
  );
}

#[test]
fn test_rfc_appendix_i_unicode() {
  assert_json(
    include_str!("testdata/input/unicode.json"),
    include_str!("testdata/output/unicode.json"),
  );
}

#[test]
fn test_rfc_appendix_i_values() {
  assert_json(
    include_str!("testdata/input/values.json"),
    include_str!("testdata/output/values.json"),
  );
}

#[test]
fn test_rfc_appendix_i_weird() {
  assert_json(
    include_str!("testdata/input/weird.json"),
    include_str!("testdata/output/weird.json"),
  );
}
