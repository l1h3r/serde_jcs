use core::str::from_utf8_unchecked;
use ryu::Float;

/// A hack to fix ryu-js buffer overflow after ES6 changes.
///
/// TODO: Fix/Fork/Alert ryu-js dev(s)
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct Buffer([u8; Self::SIZE]);

impl Buffer {
  pub const SIZE: usize = 25;

  pub const fn new() -> Self {
    Self([0; Self::SIZE])
  }

  pub fn format_finite(&mut self, float: impl Float) -> &str {
    let bytes: usize = unsafe { float.write_to_ryu_buffer(self.0.as_mut_ptr() as *mut u8) };

    debug_assert!(bytes <= self.0.len());

    unsafe { from_utf8_unchecked(&self.0[..bytes]) }
  }
}
