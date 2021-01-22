use core::panic;
use std::{ffi::CStr, os::raw::c_uint};

use aflpp_custom_mutator_sys::afl_state;

use crate::{CustomMutator, FuzzResult};

#[allow(unused_variables)]
/// A custom mutator that can fail. This mirrors [`CustomMutator`], but all methods return a [`Result<T, E>`] instead of `T`.
/// This trait can be implemented as an alternative to [`CustomMutator`], when it is more convenient (specifically: methods of your mutator can use `?` for ergonomic error handling).
/// [`FallibleCustomMutator::handle_err`] will be called in case any method returns an [`Result::Err`].
pub trait FallibleCustomMutator {
    /// The error type. All methods must return the same error type.
    type TErr;

    /// The method which handles errors. It is convenient to log the error here.
    /// This method is *expected to [panic!]*.
    fn handle_err(err: Self::TErr);

    fn init(afl: &'static afl_state, seed: c_uint) -> Result<Self, Self::TErr>
    where
        Self: Sized;

    fn fuzz_count(&mut self, buffer: &[u8]) -> Result<u32, Self::TErr> {
        Ok(1)
    }

    fn fuzz(
        &mut self,
        buffer: &mut [u8],
        add_buff: Option<&[u8]>,
        max_size: usize,
    ) -> Result<FuzzResult, Self::TErr>;

    fn queue_new_entry(
        &mut self,
        filename_new_queue: &CStr,
        filename_orig_queue: Option<&CStr>,
    ) -> Result<(), Self::TErr> {
        Ok(())
    }

    fn queue_get(&mut self, filename: &CStr) -> Result<bool, Self::TErr> {
        Ok(true)
    }

    fn describe(&mut self, max_description: usize) -> Result<Option<&CStr>, Self::TErr> {
        Ok(None)
    }

    fn introspection(&mut self) -> Result<Option<&CStr>, Self::TErr> {
        Ok(None)
    }
}

impl<M> CustomMutator for M
where
    M: FallibleCustomMutator,
    M::TErr: core::fmt::Debug,
{
    fn init(afl: &'static afl_state, seed: c_uint) -> Self
    where
        Self: Sized,
    {
        match Self::init(afl, seed) {
            Ok(r) => r,
            Err(e) => {
                Self::handle_err(e);
                panic!("Error handler did not panic")
            }
        }
    }

    fn fuzz_count(&mut self, buffer: &[u8]) -> u32 {
        match self.fuzz_count(buffer) {
            Ok(r) => r,
            Err(e) => {
                Self::handle_err(e);
                panic!("Error handler did not panic")
            }
        }
    }

    fn fuzz<'r>(
        &'r mut self,
        buffer: &mut [u8],
        add_buff: Option<&[u8]>,
        max_size: usize,
    ) -> FuzzResult<'r> {
        match self.fuzz(buffer, add_buff, max_size) {
            Ok(r) => r,
            Err(e) => {
                Self::handle_err(e);
                panic!("Error handler did not panic")
            }
        }
    }

    fn queue_new_entry(&mut self, filename_new_queue: &CStr, filename_orig_queue: Option<&CStr>) {
        match self.queue_new_entry(filename_new_queue, filename_orig_queue) {
            Ok(r) => r,
            Err(e) => {
                Self::handle_err(e);
                panic!("Error handler did not panic")
            }
        }
    }

    fn queue_get(&mut self, filename: &CStr) -> bool {
        match self.queue_get(filename) {
            Ok(r) => r,
            Err(e) => {
                Self::handle_err(e);
                panic!("Error handler did not panic")
            }
        }
    }

    fn describe(&mut self, max_description: usize) -> Option<&CStr> {
        match self.describe(max_description) {
            Ok(r) => r,
            Err(e) => {
                Self::handle_err(e);
                panic!("Error handler did not panic")
            }
        }
    }

    fn introspection(&mut self) -> Option<&CStr> {
        match self.introspection() {
            Ok(r) => r,
            Err(e) => {
                Self::handle_err(e);
                panic!("Error handler did not panic")
            }
        }
    }
}
