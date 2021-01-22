//! Somewhat safe and somewhat ergonomic bindings for creating [AFL++](https://github.com/AFLplusplus/AFLplusplus) [custom mutators](https://github.com/AFLplusplus/AFLplusplus/blob/stable/docs/custom_mutators.md) in Rust.
//! 
//! # Usage
//! AFL++ custom mutators are expected to be dynamic libraries which expose a set of symbols.
//! Check out [`CustomMutator`] to see which functions of the API are supported.
//! Then use [`export_mutator`] to export the correct symbols for your mutator.
//! In order to use the mutator, your crate needs to be a library crate and have a `crate-type` of `cdylib`.
//! Putting 
//! ```yaml
//! [lib]
//! crate-type = ["cdylib"]
//! ```
//! into your `Cargo.toml` should do the trick.
//! The final executable can be found in `target/(debug|release)/libyour_crate_name.so`.
//! # Example
//! See [`export_mutator`] for an example.
//! 
pub mod fallible;

use std::{ffi::CStr, os::raw::c_uint};

#[doc(hidden)]
pub use aflpp_custom_mutator_sys::afl_state;


/// The result of a call to [`CustomMutator::fuzz`].
#[derive(Debug)]
pub enum FuzzResult<'l> {
    /// Returned when the given buffer was modified in place. Modifying the buffer in place to be of a different length is not supported currently.
    InPlace,
    /// Returned when a buffer that is managed by the custom mutator should be returned.
    NewBuffer(&'l [u8]),
    /// Returned when the custom mutator fails. This will make AFL++ fail fast (ie crash) and corresponds to returning a NULL buffer and 0 length.
    Fail,
}

#[allow(unused_variables)]
/// Implement this trait for the mutator and export it using [`export_mutator`] to generate a custom mutator.
/// For documentation refer to the AFL++ sources.
pub trait CustomMutator {
    fn init(afl: &'static afl_state, seed: c_uint) -> Self
    where
        Self: Sized;

    fn fuzz(&mut self, buffer: &mut [u8], add_buff: Option<&[u8]>, max_size: usize) -> FuzzResult;

    fn fuzz_count(&mut self, buffer: &[u8]) -> u32 {
        1
    }

    fn queue_new_entry(&mut self, filename_new_queue: &CStr, _filename_orig_queue: Option<&CStr>) {}

    fn queue_get(&mut self, filename: &CStr) -> bool {
        true
    }

    fn describe(&mut self, max_description: usize) -> Option<&CStr> {
        None
    }

    fn introspection(&mut self) -> Option<&CStr> {
        None
    }

    /*fn post_process(&self, buffer: &[u8], unsigned char **out_buf)-> usize;
    int afl_custom_init_trim(&self, buffer: &[u8]);
    size_t afl_custom_trim(&self, unsigned char **out_buf);
    int afl_custom_post_trim(&self, unsigned char success);
    size_t afl_custom_havoc_mutation(&self, buffer: &[u8], unsigned char **out_buf, size_t max_size);
    unsigned char afl_custom_havoc_mutation_probability(&self);*/
}

/// Wrappers for the custom mutator which provide the bridging between the C API and CustomMutator.
/// These wrappers are not intended to be used directly, rather export_mutator will use them to publish the custom mutator C API.
#[doc(hidden)]
pub mod wrappers {
    use aflpp_custom_mutator_sys::afl_state;
    use core::slice;
    use std::{
        convert::TryInto,
        ffi::{c_void, CStr},
        mem::ManuallyDrop,
        os::raw::{c_char, c_uint},
        ptr::null,
    };

    use crate::{CustomMutator, FuzzResult};

    /// A structure to be used as the data pointer for our custom mutator. This was used as additional storage and is kept for now in case its needed later.
    /// Also has some convenience functions for FFI conversions (from and to ptr) and tries to make misuse hard (see [`FFIContext::from`]).
    struct FFIContext<M: CustomMutator> {
        mutator: M,
    }

    impl<M: CustomMutator> FFIContext<M> {
        fn from(ptr: *mut c_void) -> ManuallyDrop<Box<Self>> {
            assert!(!ptr.is_null());
            ManuallyDrop::new(unsafe { Box::from_raw(ptr as *mut Self) })
        }

        fn into_ptr(self: Box<Self>) -> *const c_void {
            Box::into_raw(self) as *const c_void
        }

        fn new(afl: &'static afl_state, seed: c_uint) -> Box<Self> {
            Box::new(Self {
                mutator: M::init(afl, seed),
            })
        }
    }

    /// Internal function used in the macro
    pub fn afl_custom_init_<M: CustomMutator>(
        afl: Option<&'static afl_state>,
        seed: c_uint,
    ) -> *const c_void {
        let afl = afl.expect("mutator func called with NULL afl");
        FFIContext::<M>::new(afl, seed).into_ptr()
    }

    /// Internal function used in the macro
    pub unsafe fn afl_custom_fuzz_<M: CustomMutator>(
        data: *mut c_void,
        buf: *mut u8,
        buf_size: usize,
        out_buf: *mut *const u8,
        add_buf: *mut u8,
        add_buf_size: usize,
        max_size: usize,
    ) -> usize {
        let mut context = FFIContext::<M>::from(data);
        if buf.is_null() {
            panic!("null buf passed to afl_custom_fuzz")
        }
        if out_buf.is_null() {
            panic!("null out_buf passed to afl_custom_fuzz")
        }
        let buff_slice = slice::from_raw_parts_mut(buf, buf_size);
        let add_buff_slice = if add_buf.is_null() {
            None
        } else {
            Some(slice::from_raw_parts(add_buf, add_buf_size))
        };
        match context
            .mutator
            .fuzz(buff_slice, add_buff_slice, max_size.try_into().unwrap())
        {
            FuzzResult::InPlace => {
                *out_buf = buff_slice.as_ptr();
                buff_slice.len().try_into().unwrap()
            }
            FuzzResult::NewBuffer(b) => {
                *out_buf = b.as_ptr();
                b.len().try_into().unwrap()
            }
            FuzzResult::Fail => {
                *out_buf = null();
                0
            }
        }
    }

    /// Internal function used in the macro
    pub unsafe fn afl_custom_fuzz_count_<M: CustomMutator>(
        data: *mut c_void,
        buf: *const u8,
        buf_size: usize,
    ) -> u32 {
        let mut context = FFIContext::<M>::from(data);
        if buf.is_null() {
            panic!("null buf passed to afl_custom_fuzz")
        }
        let buf_slice = slice::from_raw_parts(buf, buf_size);
        // see https://doc.rust-lang.org/nomicon/borrow-splitting.html
        let ctx = &mut **context;
        let mutator = &mut ctx.mutator;
        mutator.fuzz_count(buf_slice)
    }

    /// Internal function used in the macro
    pub fn afl_custom_queue_new_entry_<M: CustomMutator>(
        data: *mut c_void,
        filename_new_queue: *const c_char,
        filename_orig_queue: *const c_char,
    ) {
        let mut context = FFIContext::<M>::from(data);
        if filename_new_queue.is_null() {
            panic!("received null filename_new_queue in afl_custom_queue_new_entry");
        }
        let filename_new_queue = unsafe { CStr::from_ptr(filename_new_queue) };
        let filename_orig_queue = if !filename_orig_queue.is_null() {
            Some(unsafe { CStr::from_ptr(filename_orig_queue) })
        } else {
            None
        };
        context
            .mutator
            .queue_new_entry(filename_new_queue, filename_orig_queue);
    }

    /// Internal function used in the macro
    pub unsafe fn afl_custom_deinit_<M: CustomMutator>(data: *mut c_void) {
        // drop the context
        ManuallyDrop::into_inner(FFIContext::<M>::from(data));
    }

    /// Internal function used in the macro
    pub fn afl_custom_introspection_<M: CustomMutator>(data: *mut c_void) -> *const c_char {
        let mut context = FFIContext::<M>::from(data);
        if let Some(res) = context.mutator.introspection() {
            res.as_ptr()
        } else {
            null()
        }
    }

    /// Internal function used in the macro
    pub fn afl_custom_describe_<M: CustomMutator>(
        data: *mut c_void,
        max_description_len: usize,
    ) -> *const c_char {
        let mut context = FFIContext::<M>::from(data);
        if let Some(res) = context.mutator.describe(max_description_len) {
            res.as_ptr()
        } else {
            null()
        }
    }

    /// Internal function used in the macro
    pub fn afl_custom_queue_get_<M: CustomMutator>(
        data: *mut c_void,
        filename: *const c_char,
    ) -> u8 {
        let mut context = FFIContext::<M>::from(data);
        assert!(!filename.is_null());

        context
            .mutator
            .queue_get(unsafe { CStr::from_ptr(filename) }) as u8
    }
}

/// exports the given Mutator as a custom mutator as the C interface that AFL++ expects.
/// It is not possible to call this macro multiple times, because it would define the custom mutator symbols multiple times.
/// # Example
/// ```
/// # #[macro_use] extern crate aflpp_custom_mutator;
/// # use aflpp_custom_mutator::{CustomMutator, FuzzResult, afl_state};
/// # use std::os::raw::c_uint;
/// struct MyMutator;
/// impl CustomMutator for MyMutator {
///     /// ...
/// #  fn init(_afl: &afl_state, _seed: c_uint) -> Self {unimplemented!()}
/// #  fn fuzz(&mut self, _buffer: &mut [u8], _add_buff: Option<&[u8]>, _max_size: usize) -> FuzzResult {unimplemented!()}
/// }
/// export_mutator!(MyMutator);
/// ```
#[macro_export]
macro_rules! export_mutator {
    ($mutator_type:ty) => {
        #[no_mangle]
        pub extern "C" fn afl_custom_init(
            afl: ::std::option::Option<&'static $crate::afl_state>,
            seed: ::std::os::raw::c_uint,
        ) -> *const ::std::os::raw::c_void {
            $crate::wrappers::afl_custom_init_::<$mutator_type>(afl, seed)
        }

        #[no_mangle]
        pub extern "C" fn afl_custom_fuzz_count(
            data: *mut ::std::os::raw::c_void,
            buf: *const u8,
            buf_size: usize,
        ) -> u32 {
            unsafe {
                $crate::wrappers::afl_custom_fuzz_count_::<$mutator_type>(data, buf, buf_size)
            }
        }

        #[no_mangle]
        pub extern "C" fn afl_custom_fuzz(
            data: *mut ::std::os::raw::c_void,
            buf: *mut u8,
            buf_size: usize,
            out_buf: *mut *const u8,
            add_buf: *mut u8,
            add_buf_size: usize,
            max_size: usize,
        ) -> usize {
            unsafe {
                $crate::wrappers::afl_custom_fuzz_::<$mutator_type>(
                    data,
                    buf,
                    buf_size,
                    out_buf,
                    add_buf,
                    add_buf_size,
                    max_size,
                )
            }
        }

        // void afl_custom_queue_new_entry(&self, const unsigned char *filename_new_queue, const unsigned int *filename_orig_queue);
        #[no_mangle]
        pub extern "C" fn afl_custom_queue_new_entry(
            data: *mut ::std::os::raw::c_void,
            filename_new_queue: *const ::std::os::raw::c_char,
            filename_orig_queue: *const ::std::os::raw::c_char,
        ) {
            $crate::wrappers::afl_custom_queue_new_entry_::<$mutator_type>(
                data,
                filename_new_queue,
                filename_orig_queue,
            )
        }

        #[no_mangle]
        pub extern "C" fn afl_custom_queue_get(
            data: *mut ::std::os::raw::c_void,
            filename: *const ::std::os::raw::c_char,
        ) -> u8 {
            $crate::wrappers::afl_custom_queue_get_::<$mutator_type>(data, filename)
        }

        #[no_mangle]
        pub extern "C" fn afl_custom_introspection(
            data: *mut ::std::os::raw::c_void,
        ) -> *const ::std::os::raw::c_char {
            $crate::wrappers::afl_custom_introspection_::<$mutator_type>(data)
        }

        #[no_mangle]
        pub extern "C" fn afl_custom_describe(
            data: *mut ::std::os::raw::c_void,
            max_description_len: usize,
        ) -> *const ::std::os::raw::c_char {
            $crate::wrappers::afl_custom_describe_::<$mutator_type>(data, max_description_len)
        }

        #[no_mangle]
        pub extern "C" fn afl_custom_deinit(data: *mut ::std::os::raw::c_void) {
            unsafe { $crate::wrappers::afl_custom_deinit_::<$mutator_type>(data) }
        }
    };
}

#[cfg(test)]
/// this sanity test is supposed to just find out whether an empty mutator being exported by the macro compiles
mod sanity_test {
    use std::os::raw::c_uint;

    use crate::FuzzResult;

    use super::{afl_state, export_mutator, CustomMutator};

    struct ExampleMutator;

    impl CustomMutator for ExampleMutator {
        fn init(_afl: &afl_state, _seed: c_uint) -> Self {
            unimplemented!()
        }

        fn fuzz(
            &mut self,
            _buffer: &mut [u8],
            _add_buff: Option<&[u8]>,
            _max_size: usize,
        ) -> FuzzResult {
            unimplemented!()
        }
    }

    export_mutator!(ExampleMutator);
}