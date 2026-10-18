#![no_std]

mod storage;
mod voting;

use predictx_shared::{PollStatus, PredictXError, VoteChoice, VoteTally, VOTING_WINDOW_SECS};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Vec};

/// Maximum number of admins that may be registered at once.
///
/// Keeps `list_admins` bounded so it cannot grow without limit.
pub const MAX_ADMINS: u32 = 10;
/// Maximum voters retained per poll; keeping this low bounds full-vector reads.
pub const MAX_VOTERS: u32 = 64;

#[contract]
pub struct VotingOracle;

#[contracttype]
#[derive(Clone)]
struct StoredPollStatus {
    status: PollStatus,
    updated_at: u64,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    /// Registered admins `Vec<Address>`. (Instance)
    AdminList,
    PollStatus(u64),
    /// `poll_id` → vote tally. (Temporary — only needed during the voting window)
    VoteTally(u64),
    /// `poll_id` → automatically resolved outcome.
    PollOutcome(u64),
    /// `poll_id` → persistent roster of voters who cast a vote.
    Voters(u64),
    /// `(poll_id, voter)` → `bool` — has this voter cast a vote? (Temporary)
    HasVoted(u64, Address),
    /// `(poll_id, voter)` → the choice the voter recorded. (Persistent)
    VoterChoice(u64, Address),
    /// `poll_id` → voter reward reserve (unclaimed incentive pool). (Persistent)
    RewardPool(u64),
    /// `(poll_id, voter)` → `i128` reward paid to an eligible voter. (Persistent)
    VoterReward(u64, Address),
    /// `(poll_id, voter)` → `bool` — has the voter claimed their reward? (Persistent)
    RewardClaimed(u64, Address),
}

fn get_admin(env: &Env) -> Result<Address, PredictXError> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(PredictXError::NotInitialized)
}

pub(crate) fn read_poll_status(env: &Env, poll_id: u64) -> PollStatus {
    let stored: Option<StoredPollStatus> = env
        .storage()
        .persistent()
        .get(&DataKey::PollStatus(poll_id));

    stored.map(|s| s.status).unwrap_or(PollStatus::Active)
}

pub(crate) fn read_poll_status_updated_at(env: &Env, poll_id: u64) -> u64 {
    env.storage()
        .persistent()
        .get::<DataKey, StoredPollStatus>(&DataKey::PollStatus(poll_id))
        .map(|stored| stored.updated_at)
        .unwrap_or(0)
}

#[contractimpl]
impl VotingOracle {
    pub fn initialize(env: Env, admin: Address) -> Result<(), PredictXError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(PredictXError::AlreadyInitialized);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);

        // Seed the multi-admin registry with the initial admin.
        let mut admins: Vec<Address> = Vec::new(&env);
        admins.push_back(admin);
        env.storage().instance().set(&DataKey::AdminList, &admins);

        Ok(())
    }

    pub fn admin(env: Env) -> Result<Address, PredictXError> {
        get_admin(&env)
    }

    /// Register `new_admin` in the multi-admin registry.
    ///
    /// Only an existing admin may call this. Returns `AdminAlreadyRegistered`
    /// if the address is already registered.
    pub fn add_admin(env: Env, caller: Address, new_admin: Address) -> Result<(), PredictXError> {
        storage::require_admin(&env, &caller)?;
        caller.require_auth();

        let mut admins = storage::read_admins(&env);
        if admins.contains(new_admin.clone()) {
            return Err(PredictXError::AdminAlreadyRegistered);
        }
        if admins.len() >= MAX_ADMINS {
            return Err(PredictXError::AdminAlreadyRegistered);
        }

        admins.push_back(new_admin);
        storage::write_admins(&env, &admins);
        Ok(())
    }

    /// Remove `admin` from the multi-admin registry.
    ///
    /// Only an existing admin may call this. The last remaining admin cannot
    /// be removed.
    pub fn remove_admin(env: Env, caller: Address, admin: Address) -> Result<(), PredictXError> {
        storage::require_admin(&env, &caller)?;
        caller.require_auth();

        let admins = storage::read_admins(&env);
        if admins.len() <= 1 {
            return Err(PredictXError::Unauthorized);
        }

        let mut found = false;
        let mut updated: Vec<Address> = Vec::new(&env);
        for i in 0..admins.len() {
            let a = admins.get(i).unwrap();
            if a == admin {
                found = true;
            } else {
                updated.push_back(a);
            }
        }

        if !found {
            return Err(PredictXError::Unauthorized);
        }

        storage::write_admins(&env, &updated);
        Ok(())
    }

    /// Returns `true` if `addr` is a registered admin.
    pub fn is_admin(env: Env, addr: Address) -> bool {
        storage::read_admins(&env).contains(addr)
    }

    /// Returns all registered admins.
    pub fn list_admins(env: Env) -> Vec<Address> {
        storage::read_admins(&env)
    }

    /// Placeholder oracle state setter.
    ///
    /// This exists only to validate cross-contract invocation patterns during
    /// Phase 1 scaffolding.
    pub fn set_poll_status(
        env: Env,
        poll_id: u64,
        status: PollStatus,
    ) -> Result<(), PredictXError> {
        let admin = get_admin(&env)?;
        admin.require_auth();

        let stored = StoredPollStatus {
            status,
            updated_at: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::PollStatus(poll_id), &stored);
        Ok(())
    }

    /// Placeholder oracle query used by `PredictionMarket`.
    pub fn get_poll_status(env: Env, poll_id: u64) -> PollStatus {
        read_poll_status(&env, poll_id)
    }

    pub fn get_poll_status_updated_at(env: Env, poll_id: u64) -> u64 {
        read_poll_status_updated_at(&env, poll_id)
    }

    /// Return the voters who have cast a vote on `poll_id`.
    pub fn get_voters(env: Env, poll_id: u64) -> Vec<Address> {
        storage::read_voters(&env, poll_id)
    }

    /// Returns whether `voter` has already voted on a known poll.
    pub fn has_voted(env: Env, poll_id: u64, voter: Address) -> bool {
        if !env
            .storage()
            .persistent()
            .has(&DataKey::PollStatus(poll_id))
        {
            return false;
        }

        storage::has_voted(&env, poll_id, &voter)
            || storage::read_voters(&env, poll_id).contains(voter)
    }

    /// Returns whether `voter` can cast a vote on `poll_id` right now.
    ///
    /// Unknown polls, polls outside the voting window, repeat voters, and polls
    /// at the voter limit are ineligible. Staker exclusion is handled separately.
    pub fn can_vote(env: Env, poll_id: u64, voter: Address) -> bool {
        if !env
            .storage()
            .persistent()
            .has(&DataKey::PollStatus(poll_id))
            || read_poll_status(&env, poll_id) != PollStatus::Voting
        {
            return false;
        }

        let voting_end_time = read_poll_status_updated_at(&env, poll_id)
            .checked_add(VOTING_WINDOW_SECS)
            .unwrap_or(0);
        if env.ledger().timestamp() >= voting_end_time
            || Self::has_voted(env.clone(), poll_id, voter)
            || storage::read_voters(&env, poll_id).len() >= MAX_VOTERS
        {
            return false;
        }

        true
    }

    /// Record a voter's choice on a poll.
    pub fn cast_vote(
        env: Env,
        voter: Address,
        poll_id: u64,
        choice: VoteChoice,
    ) -> Result<VoteTally, PredictXError> {
        voting::cast_vote(&env, voter, poll_id, choice)
    }

    pub fn auto_resolve(env: Env, poll_id: u64) -> Result<VoteChoice, PredictXError> {
        voting::auto_resolve(&env, poll_id)
    }

    pub fn get_poll_outcome(env: Env, poll_id: u64) -> Result<VoteChoice, PredictXError> {
        env.storage()
            .persistent()
            .get(&DataKey::PollOutcome(poll_id))
            .ok_or(PredictXError::PollNotFound)
    }

    /// Set (fund) the voter reward reserve for `poll_id`. Admin only.
    ///
    /// The policy for how large the reserve should be is deliberately out of
    /// scope here; this only records the amount that `claim_reward` divides
    /// among the eligible (winning) voters.
    pub fn set_reward_pool(
        env: Env,
        caller: Address,
        poll_id: u64,
        amount: i128,
    ) -> Result<(), PredictXError> {
        voting::set_reward_pool(&env, caller, poll_id, amount)
    }

    /// Claim the caller's voter reward for `poll_id`.
    ///
    /// Only voters who backed the resolved winning outcome may claim; the pool
    /// is split evenly across those eligible voters.
    pub fn claim_reward(env: Env, voter: Address, poll_id: u64) -> Result<i128, PredictXError> {
        voting::claim_reward(&env, voter, poll_id)
    }

    /// The choice `voter` recorded on `poll_id`, if they voted.
    pub fn get_voter_choice(env: Env, poll_id: u64, voter: Address) -> Option<VoteChoice> {
        storage::read_vote_choice(&env, poll_id, &voter)
    }

    /// The voter reward reserve set for `poll_id` (0 when unset).
    pub fn get_reward_pool(env: Env, poll_id: u64) -> i128 {
        storage::read_reward_pool(&env, poll_id)
    }

    /// Whether `voter` has already claimed their `poll_id` reward.
    pub fn has_claimed_reward(env: Env, poll_id: u64, voter: Address) -> bool {
        storage::has_claimed_reward(&env, poll_id, &voter)
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};

    #[test]
    fn set_and_get_status() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        client.set_poll_status(&42_u64, &PollStatus::Resolved);
        assert_eq!(client.get_poll_status(&42_u64), PollStatus::Resolved);
    }

    #[test]
    fn voting_views_return_false_for_unknown_poll() {
        let (env, _admin, client) = setup();
        let voter = Address::generate(&env);

        assert!(!client.has_voted(&99_u64, &voter));
        assert!(!client.can_vote(&99_u64, &voter));
    }

    #[test]
    fn voting_views_track_vote_and_duplicate_eligibility() {
        let (env, _admin, client) = setup();
        let voter = Address::generate(&env);
        client.set_poll_status(&1_u64, &PollStatus::Voting);

        assert!(!client.has_voted(&1_u64, &voter));
        assert!(client.can_vote(&1_u64, &voter));

        client.cast_vote(&voter, &1_u64, &VoteChoice::Yes);

        assert!(client.has_voted(&1_u64, &voter));
        assert!(!client.can_vote(&1_u64, &voter));
    }

    #[test]
    fn can_vote_rejects_unopened_and_expired_polls() {
        let (env, _admin, client) = setup();
        let voter = Address::generate(&env);

        client.set_poll_status(&2_u64, &PollStatus::Active);
        assert!(!client.can_vote(&2_u64, &voter));

        client.set_poll_status(&3_u64, &PollStatus::Voting);
        env.ledger()
            .with_mut(|ledger| ledger.timestamp += VOTING_WINDOW_SECS);
        assert!(!client.can_vote(&3_u64, &voter));
    }

    fn setup() -> (Env, Address, VotingOracleClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VotingOracle, ());
        let client = VotingOracleClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        (env, admin, client)
    }

    #[test]
    fn initialize_seeds_admin_registry() {
        let (env, admin, client) = setup();

        assert!(client.is_admin(&admin));

        let mut expected: Vec<Address> = Vec::new(&env);
        expected.push_back(admin);
        assert_eq!(client.list_admins(), expected);
    }

    #[test]
    fn add_admin_registers_new_admin() {
        let (env, admin, client) = setup();
        let new_admin = Address::generate(&env);

        client.add_admin(&admin, &new_admin);

        assert!(client.is_admin(&new_admin));
        assert_eq!(client.list_admins().len(), 2);
    }

    #[test]
    fn add_admin_rejects_existing_admin() {
        let (_env, admin, client) = setup();

        let err = client
            .try_add_admin(&admin, &admin)
            .expect_err("re-adding an existing admin must fail");

        assert_eq!(err, Ok(PredictXError::AdminAlreadyRegistered));
    }

    #[test]
    fn add_admin_rejects_non_admin_caller() {
        let (env, _admin, client) = setup();
        let stranger = Address::generate(&env);
        let new_admin = Address::generate(&env);

        let err = client
            .try_add_admin(&stranger, &new_admin)
            .expect_err("non-admin caller must be rejected");

        assert_eq!(err, Ok(PredictXError::Unauthorized));
        assert!(!client.is_admin(&new_admin));
    }

    #[test]
    fn remove_admin_removes_registered_admin() {
        let (env, admin, client) = setup();
        let second = Address::generate(&env);
        client.add_admin(&admin, &second);

        client.remove_admin(&admin, &second);

        assert!(!client.is_admin(&second));
        assert_eq!(client.list_admins().len(), 1);
    }

    #[test]
    fn remove_admin_rejects_last_remaining_admin() {
        let (_env, admin, client) = setup();

        let err = client
            .try_remove_admin(&admin, &admin)
            .expect_err("the last remaining admin cannot be removed");

        assert_eq!(err, Ok(PredictXError::Unauthorized));
        assert!(client.is_admin(&admin));
    }
}
