use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::panic;
use std::ptr;

use crate::methods::{
    transform_buffer, transform_buffer_incremental, IncrementalBuffer as RustIncrementalBuffer,
    TELEX, VNI,
};
use crate::processor::AccentStyle;
use crate::Definition;

// Helper to convert a Definition map (like TELEX or VNI) to a &'static Definition
// This is a bit of a simplification; real custom definitions from C would be more complex.
fn get_method_definition(method_type_str: &str) -> Option<&'static Definition> {
    match method_type_str.to_lowercase().as_str() {
        "vni" => Some(&VNI),
        "telex" => Some(&TELEX),
        _ => None,
    }
}

/// Represents the style of accent placement.
#[repr(C)]
pub enum ViAccentStyle {
    New = 0, // hoà
    Old = 1, // hòa
}

impl From<ViAccentStyle> for AccentStyle {
    fn from(style: ViAccentStyle) -> Self {
        match style {
            ViAccentStyle::New => AccentStyle::New,
            ViAccentStyle::Old => AccentStyle::Old,
        }
    }
}

/// Opaque struct representing an incremental buffer.
/// The actual Rust struct `RustIncrementalBuffer` will be boxed and its pointer cast to this.
#[repr(C)]
pub struct ViIncrementalBuffer {
    // Actual implementation will be a Box<RustIncrementalBuffer<'static>>
    // We use a dummy field to make it an inhabited type for C.
    // C code should only ever deal with pointers to this struct.
    _private: [u8; 0],
}

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
    let method_str = unsafe {
        if method_type.is_null() {
            eprintln!("Error: Method type string is null");
            return 1;
        }
        match CStr::from_ptr(method_type).to_str() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to convert method type to &str: {}", e);
                return 1;
            }
        }
    };

    let input_str = unsafe {
        if input_buffer.is_null() {
            eprintln!("Error: Input buffer string is null");
            return 1;
        }
        match CStr::from_ptr(input_buffer).to_str() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to convert input buffer to &str: {}", e);
                return 1;
            }
        }
    };

    let method_definition = match get_method_definition(method_str) {
        Some(def) => def,
        None => {
            eprintln!("Unsupported method type: {}", method_str);
            return 1;
        }
    };

    // Using a panic catcher to make FFI boundary safer
    let result_string_or_panic = panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut result_string_inner = String::new();
        transform_buffer(
            method_definition,
            input_str.chars(),
            &mut result_string_inner,
        );
        result_string_inner
    }));

    let result_string = match result_string_or_panic {
        Ok(s) => s,
        Err(_) => {
            eprintln!("Panic occurred in transform_buffer");
            return 2; // Indicate panic
        }
    };

    if result_string.len() >= output_buffer_size {
        eprintln!(
            "Output buffer too small. Needed {}, got {}",
            result_string.len() + 1,
            output_buffer_size
        );
        return 1;
    }

    match CString::new(result_string) {
        Ok(c_output_str) => unsafe {
            ptr::copy_nonoverlapping(
                c_output_str.as_ptr(),
                output_buffer,
                c_output_str.as_bytes_with_nul().len(),
            );
        },
        Err(e) => {
            eprintln!("Failed to create CString from result: {}", e);
            return 1;
        }
    }
    0 // Success
}

// --- Incremental Buffer Functions ---

/// Creates an incremental processing buffer for a given input method.
///
/// # Safety
/// - `method_type` must be a pointer to a valid null-terminated C string.
/// - The caller is responsible for freeing the returned buffer using `vi_incremental_buffer_free`.
#[no_mangle]
pub unsafe extern "C" fn vi_create_incremental_buffer(
    method_type: *const c_char,
) -> *mut ViIncrementalBuffer {
    if method_type.is_null() {
        eprintln!("Error: Method type string is null for vi_create_incremental_buffer");
        return ptr::null_mut();
    }

    let method_str = match CStr::from_ptr(method_type).to_str() {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "Failed to convert method type to &str in vi_create_incremental_buffer: {}",
                e
            );
            return ptr::null_mut();
        }
    };

    let method_definition = match get_method_definition(method_str) {
        Some(def) => def,
        None => {
            eprintln!(
                "Unsupported method type in vi_create_incremental_buffer: {}",
                method_str
            );
            return ptr::null_mut();
        }
    };

    let buffer = transform_buffer_incremental(method_definition); // This returns methods::IncrementalBuffer
    Box::into_raw(Box::new(buffer)) as *mut ViIncrementalBuffer // Cast to opaque C struct
}

/// Pushes a character to the incremental buffer and returns the current transformed string.
///
/// # Safety
/// - `buffer_ptr` must be a valid pointer to a `ViIncrementalBuffer` created by `vi_create_incremental_buffer`.
/// - The returned C string is owned by the buffer. It is valid until the next call to
///   `vi_incremental_buffer_push`, `vi_incremental_buffer_clear`, or `vi_incremental_buffer_free` on the same buffer.
///   The caller must not free this string.
#[no_mangle]
pub unsafe extern "C" fn vi_incremental_buffer_push(
    buffer_ptr: *mut ViIncrementalBuffer,
    ch: c_char,
) -> *const c_char {
    if buffer_ptr.is_null() {
        eprintln!("Error: Buffer pointer is null for vi_incremental_buffer_push");
        return ptr::null();
    }
    let buffer = &mut *(buffer_ptr as *mut RustIncrementalBuffer);
    let _update_result = buffer.push(ch as u8 as char); // Assuming ASCII or compatible char

    match CString::new(buffer.view()) {
        Ok(c_str) => {
            let ptr = c_str.as_ptr();
            std::mem::forget(c_str);
            ptr
        }
        Err(_) => ptr::null(),
    }
}

/// Returns the current transformed string from the incremental buffer.
///
/// # Safety
/// - `buffer_ptr` must be a valid pointer to a `ViIncrementalBuffer` created by `vi_create_incremental_buffer`.
/// - The returned C string is owned by the buffer. It is valid until the next call to
///   `vi_incremental_buffer_push`, `vi_incremental_buffer_clear`, or `vi_incremental_buffer_free` on the same buffer.
///   The caller must not free this string.
#[no_mangle]
pub unsafe extern "C" fn vi_incremental_buffer_view(
    buffer_ptr: *mut ViIncrementalBuffer,
) -> *const c_char {
    if buffer_ptr.is_null() {
        eprintln!("Error: Buffer pointer is null for vi_incremental_buffer_view");
        return ptr::null();
    }
    let buffer = &*(buffer_ptr as *mut RustIncrementalBuffer);
    match CString::new(buffer.view()) {
        Ok(c_str) => {
            let ptr = c_str.as_ptr();
            std::mem::forget(c_str); // Leak for now
            ptr
        }
        Err(_) => ptr::null(),
    }
}

/// Clears the content and input of the incremental buffer.
///
/// # Safety
/// - `buffer_ptr` must be a valid pointer to a `ViIncrementalBuffer` created by `vi_create_incremental_buffer`.
#[no_mangle]
pub unsafe extern "C" fn vi_incremental_buffer_clear(buffer_ptr: *mut ViIncrementalBuffer) {
    if buffer_ptr.is_null() {
        eprintln!("Error: Buffer pointer is null for vi_incremental_buffer_clear");
        return;
    }
    let buffer = &mut *(buffer_ptr as *mut RustIncrementalBuffer);
    buffer.clear();
}

/// Returns the current sequence of input characters in the incremental buffer.
///
/// # Safety
/// - `buffer_ptr` must be a valid pointer to a `ViIncrementalBuffer` created by `vi_create_incremental_buffer`.
/// - The returned C string is owned by the buffer. It is valid until the next call to
///   `vi_incremental_buffer_push`, `vi_incremental_buffer_clear`, or `vi_incremental_buffer_free` on the same buffer.
///   The caller must not free this string.
#[no_mangle]
pub unsafe extern "C" fn vi_incremental_buffer_get_input(
    buffer_ptr: *mut ViIncrementalBuffer,
) -> *const c_char {
    if buffer_ptr.is_null() {
        eprintln!("Error: Buffer pointer is null for vi_incremental_buffer_get_input");
        return ptr::null();
    }
    let buffer = &*(buffer_ptr as *mut RustIncrementalBuffer);
    let input_string: String = buffer.input().iter().collect();
    match CString::new(input_string) {
        Ok(c_str) => {
            let ptr = c_str.as_ptr();
            std::mem::forget(c_str); // Leak for now
            ptr
        }
        Err(_) => ptr::null(),
    }
}

/// Frees the memory associated with the incremental buffer.
///
/// # Safety
/// - `buffer_ptr` must be a valid pointer to a `ViIncrementalBuffer` created by `vi_create_incremental_buffer`.
/// - After calling this function, `buffer_ptr` becomes invalid and must not be used again.
#[no_mangle]
pub unsafe extern "C" fn vi_incremental_buffer_free(buffer_ptr: *mut ViIncrementalBuffer) {
    if !buffer_ptr.is_null() {
        drop(Box::from_raw(buffer_ptr as *mut RustIncrementalBuffer));
    }
}

// Placeholder for proper CString management in ViIncrementalBuffer
// This is a common challenge in Rust FFI. One good way is to have the C client pass in buffers.
// Another is to manage a CString within the Rust struct that is exposed via a raw pointer.
// For now, the above functions leak CStrings for `view`, `push`, and `get_input` calls,
// which is not production-ready but allows testing the API structure.
// A real implementation would need to fix these leaks, possibly by:
// 1. Requiring the C caller to provide output buffers for string data.
// 2. Storing a CString in the Rust struct, updating it, and returning its .as_ptr(). This CString
//    would be dropped when the main struct is freed. This means the CString pointer is only valid
//    until the next mutable operation or free.

// --- Transformation with Style ---

/// Transforms an input string using the specified Vietnamese input method and accent style.
///
/// # Safety
/// - `method_type` must be a pointer to a valid null-terminated C string.
/// - `input_buffer` must be a pointer to a valid null-terminated C string.
/// - `output_buffer` must be a pointer to a mutable character buffer of at least `output_buffer_size` bytes.
/// - `output_buffer_size` must be large enough to hold the transformed string, including the null terminator.
#[no_mangle]
pub unsafe extern "C" fn vi_transform_buffer_with_style(
    method_type: *const c_char,
    style: ViAccentStyle,
    input_buffer: *const c_char,
    output_buffer: *mut c_char,
    output_buffer_size: usize,
) -> c_int {
    let method_str = unsafe {
        if method_type.is_null() {
            eprintln!("Error: Method type string is null");
            return 1;
        }
        match CStr::from_ptr(method_type).to_str() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to convert method type to &str: {}", e);
                return 1;
            }
        }
    };

    let input_str = unsafe {
        if input_buffer.is_null() {
            eprintln!("Error: Input buffer string is null");
            return 1;
        }
        match CStr::from_ptr(input_buffer).to_str() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to convert input buffer to &str: {}", e);
                return 1;
            }
        }
    };

    let method_definition = match get_method_definition(method_str) {
        Some(def) => def,
        None => {
            eprintln!("Unsupported method type: {}", method_str);
            return 1;
        }
    };

    let accent_style: AccentStyle = style.into();

    let result_string_or_panic = panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut result_string_inner = String::new();
        crate::methods::transform_buffer_with_style(
            method_definition,
            accent_style,
            input_str.chars(),
            &mut result_string_inner,
        );
        result_string_inner
    }));

    let result_string = match result_string_or_panic {
        Ok(s) => s,
        Err(_) => {
            eprintln!("Panic occurred in transform_buffer_with_style");
            return 2; // Indicate panic
        }
    };
    if result_string.len() >= output_buffer_size {
        eprintln!(
            "Output buffer too small for transform_buffer_with_style. Needed {}, got {}",
            result_string.len() + 1,
            output_buffer_size
        );
        return 1;
    }

    match CString::new(result_string) {
        Ok(c_output_str) => unsafe {
            ptr::copy_nonoverlapping(
                c_output_str.as_ptr(),
                output_buffer,
                c_output_str.as_bytes_with_nul().len(),
            );
        },
        Err(e) => {
            eprintln!(
                "Failed to create CString from result for transform_buffer_with_style: {}",
                e
            );
            return 1;
        }
    }
    0 // Success
}
