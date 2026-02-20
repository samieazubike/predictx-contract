#![no_std]

pub mod constants;
pub mod errors;
pub mod types;

pub use constants::*;
pub use errors::Error;
pub use types::*;

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn platform_fee_constant_is_set() {
        assert_eq!(PLATFORM_FEE_BPS, 500);
    }
}
