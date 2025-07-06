use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::panic;
use std::ptr;

use crate::methods::{transform_buffer, TELEX, VNI};

/// Transforms an input string using the specified Vietnamese input method.
///
/// # Safety
///
/// - `method_type` must be a pointer to a valid null-terminated C string.
/// - `input_buffer` must be a pointer to a valid null-terminated C string.
/// - `output_buffer` must be a pointer to a mutable character buffer of at least `output_buffer_size` bytes.
/// - `output_buffer_size` must be large enough to hold the transformed string, including the null terminator.
///   It is recommended to allocate a buffer of the same size as the input buffer, plus one for the null terminator,
///   as the transformed string can sometimes be larger than the input.
#[no_mangle]
pub unsafe extern "C" fn transform_buffer_c(
    method_type: *const c_char,
    input_buffer: *const c_char,
    output_buffer: *mut c_char,
    output_buffer_size: usize,
) -> c_int {
    let result = panic::catch_unwind(|| {
        let method_str = unsafe {
            if method_type.is_null() {
                return Err("Method type string is null".to_string());
            }
            CStr::from_ptr(method_type)
        }
        .to_str()
        .map_err(|e| format!("Failed to convert method type to &str: {}", e))?;

        let input_str = unsafe {
            if input_buffer.is_null() {
                return Err("Input buffer string is null".to_string());
            }
            CStr::from_ptr(input_buffer)
        }
        .to_str()
        .map_err(|e| format!("Failed to convert input buffer to &str: {}", e))?;

        let mut result = String::new();
        let method = match method_str.to_lowercase().as_str() {
            "vni" => &VNI,
            "telex" => &TELEX,
            _ => return Err(format!("Unsupported method type: {}", method_str)),
        };

        transform_buffer(method, input_str.chars(), &mut result);

        if result.len() >= output_buffer_size {
            return Err(format!(
                "Output buffer too small. Needed {}, got {}",
                result.len() + 1,
                output_buffer_size
            ));
        }

        let c_output_str = CString::new(result)
            .map_err(|e| format!("Failed to create CString from result: {}", e))?;
        unsafe {
            ptr::copy_nonoverlapping(
                c_output_str.as_ptr(),
                output_buffer,
                c_output_str.as_bytes_with_nul().len(),
            );
        }
        Ok(())
    });

    match result {
        Ok(Ok(())) => 0, // Success
        Ok(Err(e)) => {
            eprintln!("Error: {}", e);
            1 // Indicate error
        }
        Err(_) => {
            eprintln!("Panic occurred in transform_buffer_c");
            2 // Indicate panic
        }
    }
}
