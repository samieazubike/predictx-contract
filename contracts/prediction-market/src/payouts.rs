use predictx_shared::{Poll, PredictXError, BPS_DENOMINATOR};

pub fn winning_pool(poll: &Poll) -> Result<i128, PredictXError> {
    match poll.outcome {
        Some(true) => Ok(poll.yes_pool),
        Some(false) => Ok(poll.no_pool),
        None => Err(PredictXError::InvalidOutcome),
    }
}

pub fn losing_pool(poll: &Poll) -> Result<i128, PredictXError> {
    match poll.outcome {
        Some(true) => Ok(poll.no_pool),
        Some(false) => Ok(poll.yes_pool),
        None => Err(PredictXError::InvalidOutcome),
    }
}

pub fn fee_amount(total: i128, fee_bps: i128) -> Result<i128, PredictXError> {
    let bps = BPS_DENOMINATOR as i128;
    Ok(total * fee_bps / bps)
}

pub fn payout_share(
    user_stake: i128,
    winning_pool: i128,
    distributable: i128,
) -> Result<i128, PredictXError> {
    if winning_pool == 0 {
        return Err(PredictXError::InvalidOutcome);
    }
    Ok(user_stake * distributable / winning_pool)
}

#[cfg(test)]
mod test {
    extern crate std;

    use super::*;
    use predictx_shared::{Poll, PollCategory, PollStatus};
    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    fn make_poll(yes_pool: i128, no_pool: i128, outcome: Option<bool>) -> Poll {
        let env = Env::default();
        Poll {
            poll_id: 1,
            match_id: 1,
            creator: Address::generate(&env),
            question: String::from_str(&env, "Q"),
            category: PollCategory::Other,
            lock_time: 0,
            yes_pool,
            no_pool,
            yes_count: 0,
            no_count: 0,
            status: PollStatus::Resolved,
            outcome,
            resolution_time: 0,
            created_at: 0,
        }
    }

    #[test]
    fn winning_and_losing_pool_select_correct_sides() {
        let poll_yes = make_poll(450, 300, Some(true));
        assert_eq!(winning_pool(&poll_yes), Ok(450));
        assert_eq!(losing_pool(&poll_yes), Ok(300));

        let poll_no = make_poll(450, 300, Some(false));
        assert_eq!(winning_pool(&poll_no), Ok(300));
        assert_eq!(losing_pool(&poll_no), Ok(450));

        let poll_unresolved = make_poll(450, 300, None);
        assert_eq!(winning_pool(&poll_unresolved), Err(PredictXError::InvalidOutcome));
        assert_eq!(losing_pool(&poll_unresolved), Err(PredictXError::InvalidOutcome));
    }

    #[test]
    fn fee_amount_with_five_percent_on_seven_fifty_total() {
        let total: i128 = 450 + 300;
        let fee_bps: i128 = 500;
        let fee = fee_amount(total, fee_bps).unwrap();
        let expected = 750i128 * 500 / 10_000;
        assert_eq!(fee, expected);
        assert_eq!(fee, 37);
    }

    #[test]
    fn payout_share_worked_example_four_fifty_three_hundred_pools() {
        let yes_pool: i128 = 450;
        let no_pool: i128 = 300;
        let total = yes_pool + no_pool;
        let fee_bps: i128 = 500;
        let fee = fee_amount(total, fee_bps).unwrap();
        let distributable = total - fee;
        assert_eq!(distributable, 750 - 37);

        let user_stake: i128 = 100;
        let share = payout_share(user_stake, yes_pool, distributable).unwrap();
        let expected = 100i128 * 713 / 450;
        assert_eq!(share, expected);
        assert_eq!(share, 158);
    }

    #[test]
    fn payout_share_rejects_zero_denominator() {
        let err = payout_share(100, 0, 500).expect_err("should reject zero winning_pool");
        assert_eq!(err, PredictXError::InvalidOutcome);
    }
}
