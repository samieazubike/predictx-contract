#![no_std]

pub mod constants;
pub mod errors;
pub mod math;
pub mod security;
pub mod storage;
pub mod types;

pub use constants::*;
pub use errors::PredictXError;
pub use math::*;
pub use security::*;
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

    #[test]
    fn security_constants_are_reasonable() {
        // Lock buffer should be at least 1 minute to prevent frontrunning
        assert!(LOCK_TIME_BUFFER_SECS >= 60);
        // Max stake should be greater than min stake
        assert!(MAX_STAKE_AMOUNT > MIN_STAKE_AMOUNT);
    }

    #[test]
    fn safe_math_functions_work() {
        assert_eq!(super::safe_add(1, 2).unwrap(), 3);
        assert_eq!(super::safe_sub(5, 3).unwrap(), 2);
        assert_eq!(super::safe_mul(3, 4).unwrap(), 12);
        assert_eq!(super::safe_div(10, 2).unwrap(), 5);
    }

    #[test]
    fn validate_stake_amount_works() {
        assert!(validate_stake_amount(100, MIN_STAKE_AMOUNT, MAX_STAKE_AMOUNT).is_ok());
        assert_eq!(
            validate_stake_amount(0, MIN_STAKE_AMOUNT, MAX_STAKE_AMOUNT),
            Err(PredictXError::StakeAmountZero)
        );
        assert_eq!(
            validate_stake_amount(-1, MIN_STAKE_AMOUNT, MAX_STAKE_AMOUNT),
            Err(PredictXError::StakeAmountZero)
        );
        assert_eq!(
            validate_stake_amount(5, MIN_STAKE_AMOUNT, MAX_STAKE_AMOUNT),
            Err(PredictXError::StakeBelowMinimum)
        );
        assert_eq!(
            validate_stake_amount(MAX_STAKE_AMOUNT + 1, MIN_STAKE_AMOUNT, MAX_STAKE_AMOUNT),
            Err(PredictXError::StakeExceedsLimit)
        );
    }
}