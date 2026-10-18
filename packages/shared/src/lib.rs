#![no_std]

pub mod constants;
pub mod errors;
pub mod storage;
pub mod types;

pub use constants::*;
pub use errors::PredictXError;
pub use storage::DataKey;
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

    #[test]
    fn bps_denominator_is_correct() {
        assert_eq!(BPS_DENOMINATOR, 10_000);
    }

    #[test]
    fn voting_window_is_two_hours() {
        assert_eq!(VOTING_WINDOW_SECS, 7_200);
    }

    #[test]
    fn error_codes_are_sequential() {
        // Spot-check a few to ensure no accidental gaps
        assert_eq!(PredictXError::NotInitialized as u32, 1);
        assert_eq!(PredictXError::AlreadyInitialized as u32, 2);
        assert_eq!(PredictXError::TransferFailed as u32, 32);
    }

    #[test]
    fn poll_status_discriminants_are_stable() {
        assert_eq!(PollStatus::Active as u32, 0);
        assert_eq!(PollStatus::Cancelled as u32, 6);
    }
}