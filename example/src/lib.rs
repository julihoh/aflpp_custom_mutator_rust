use aflpp_custom_mutator::{CustomMutator, FuzzResult, afl_state, export_mutator};

struct ExampleMutator;

impl CustomMutator for ExampleMutator {
    fn init(afl: &'static afl_state, seed: std::os::raw::c_uint) -> Self
    where
        Self: Sized {
            Self
    }

    fn fuzz(&mut self, buffer: &mut [u8], add_buff: Option<&[u8]>, max_size: usize) -> FuzzResult {
        buffer.reverse();
        FuzzResult::InPlace
    }
}

export_mutator!(ExampleMutator);