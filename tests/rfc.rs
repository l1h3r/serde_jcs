#[path = "common/utils.rs"]
mod utils;

use serde_jcs::to_string;
use serde_jcs::to_vec;
use serde_json::Value;

use crate::utils::INVALID_FLOAT;
use crate::utils::assert_json;
use crate::utils::assert_json_err;
use crate::utils::hex;
use crate::utils::raw;
use crate::utils::value;

macro_rules! vectors {
  ($($dir:literal => { $($test:ident: $name:literal,)* })*) => {
    $($(
      #[test]
      fn $test() {
        let input: &str = include_str!(concat!($dir, "/input/", $name, ".json"));
        let expected: &str = include_str!(concat!($dir, "/output/", $name, ".json"));

        assert_json(value(input), expected);
      }
    )*)*
  };
}

vectors! {
  "testdata/rfc8785" => {
    test_rfc_appendix_c: "appendix_c",
    test_rfc_appendix_e: "appendix_e",
    test_rfc_section_3_2_3: "section_3_2_3",
    test_rfc_section_3_2_3_order: "section_3_2_3_order",
  }
  "testdata/cyberphone" => {
    test_rfc_appendix_i_arrays: "arrays",
    test_rfc_appendix_i_french: "french",
    test_rfc_appendix_i_structures: "structures",
    test_rfc_appendix_i_unicode: "unicode",
    test_rfc_appendix_i_values: "values",
    test_rfc_appendix_i_weird: "weird",
  }
}

#[track_caller]
fn assert_json_number(input: u64, expected: &str) {
  assert_json(f64::from_bits(input), expected);
}

#[test]
fn test_rfc_appendix_b() {
  assert_json_number(0x0000_0000_0000_0000, "0"); // Zero
  assert_json_number(0x8000_0000_0000_0000, "0"); // Minus zero
  assert_json_number(0x0000_0000_0000_0001, "5e-324"); // Min pos number
  assert_json_number(0x8000_0000_0000_0001, "-5e-324"); // Min neg number
  assert_json_number(0x7fef_ffff_ffff_ffff, "1.7976931348623157e+308"); // Max pos number
  assert_json_number(0xffef_ffff_ffff_ffff, "-1.7976931348623157e+308"); // Max neg number
  assert_json_number(0x4340_0000_0000_0000, "9007199254740992"); // Max pos int
  assert_json_number(0xc340_0000_0000_0000, "-9007199254740992"); // Max neg int
  assert_json_number(0x4430_0000_0000_0000, "295147905179352830000"); // ~2**68

  assert_json_err(f64::from_bits(0x7fff_ffff_ffff_ffff), INVALID_FLOAT); // NaN
  assert_json_err(f64::from_bits(0x7ff0_0000_0000_0000), INVALID_FLOAT); // Infinity

  assert_json_number(0x44b5_2d02_c7e1_4af5, "9.999999999999997e+22");
  assert_json_number(0x44b5_2d02_c7e1_4af6, "1e+23");
  assert_json_number(0x44b5_2d02_c7e1_4af7, "1.0000000000000001e+23");
  assert_json_number(0x444b_1ae4_d6e2_ef4e, "999999999999999700000");
  assert_json_number(0x444b_1ae4_d6e2_ef4f, "999999999999999900000");
  assert_json_number(0x444b_1ae4_d6e2_ef50, "1e+21");
  assert_json_number(0x3eb0_c6f7_a0b5_ed8c, "9.999999999999997e-7");
  assert_json_number(0x3eb0_c6f7_a0b5_ed8d, "0.000001");
  assert_json_number(0x41b3_de43_5555_5553, "333333333.3333332");
  assert_json_number(0x41b3_de43_5555_5554, "333333333.33333325");
  assert_json_number(0x41b3_de43_5555_5555, "333333333.3333333");
  assert_json_number(0x41b3_de43_5555_5556, "333333333.3333334");
  assert_json_number(0x41b3_de43_5555_5557, "333333333.33333343");
  assert_json_number(0xbecb_f647_612f_3696, "-0.0000033333333333333333");
  assert_json_number(0x4314_3ff3_c1cb_0959, "1424953923781206.2"); // Round to even
}

#[test]
fn test_rfc_es6_numbers() {
  for case in include_str!("testdata/cyberphone/numbers.jsonl").lines() {
    let case: Value = value(case);
    let id: &str = case["id"].as_str().unwrap();
    let bits: u64 = u64::from_str_radix(case["bits"].as_str().unwrap(), 16).unwrap();
    let expected: &str = case["expected"].as_str().unwrap();
    let received: String = to_string(&f64::from_bits(bits)).unwrap();

    assert_eq!(received, expected, "{id}");
  }
}

#[test]
fn test_rfc_section_3_2_4() {
  let input: &str = include_str!("testdata/cyberphone/input/values.json");
  let bytes: &str = include_str!("testdata/rfc8785/output/section_3_2_4.txt");

  assert_eq!(to_vec(&value(input)).unwrap(), hex(bytes));
}

#[test]
fn test_rfc_lone_surrogates() {
  assert_json_err(raw(r#""\udead""#), "lone leading surrogate");
  assert_json_err(raw(r#"{"\udead":1}"#), "lone leading surrogate");
  assert_json_err(raw(r#"["\ud800"]"#), "unexpected end of hex escape");
}
