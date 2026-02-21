use soroban_sdk::{Address, Env, String, Symbol, Vec};

use crate::{
    errors::PredictXError,
    storage::DataKey,
    types::Match,
};

// ── Internal helper ───────────────────────────────────────────────────────────

pub fn require_admin(env: &Env, caller: &Address) -> Result<(), PredictXError> {
    caller.require_auth();
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(PredictXError::NotInitialized)?;
    if *caller != admin {
        return Err(PredictXError::Unauthorized);
    }
    Ok(())
}

// ── Match functions ───────────────────────────────────────────────────────────

pub fn create_match(
    env: &Env,
    admin: Address,
    home_team: String,
    away_team: String,
    league: String,
    venue: String,
    kickoff_time: u64,
) -> Result<u64, PredictXError> {
    require_admin(env, &admin)?;

    let now = env.ledger().timestamp();
    if kickoff_time <= now {
        return Err(PredictXError::InvalidLockTime);
    }

    let match_id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::NextMatchId)
        .unwrap_or(1);

    let new_match = Match {
        match_id,
        home_team,
        away_team,
        league,
        venue,
        kickoff_time,
        created_by: admin,
        is_finished: false,
    };

    env.storage()
        .persistent()
        .set(&DataKey::Match(match_id), &new_match);

    let empty: Vec<u64> = Vec::new(env);
    env.storage()
        .persistent()
        .set(&DataKey::MatchPolls(match_id), &empty);

    env.storage()
        .instance()
        .set(&DataKey::NextMatchId, &(match_id + 1));

    env.events().publish(
        (Symbol::new(env, "MatchCreated"), match_id),
        new_match,
    );

    Ok(match_id)
}

pub fn update_match(
    env: &Env,
    admin: Address,
    match_id: u64,
    home_team: Option<String>,
    away_team: Option<String>,
    league: Option<String>,
    venue: Option<String>,
    kickoff_time: Option<u64>,
) -> Result<Match, PredictXError> {
    require_admin(env, &admin)?;

    let mut m: Match = env
        .storage()
        .persistent()
        .get(&DataKey::Match(match_id))
        .ok_or(PredictXError::MatchNotFound)?;

    let now = env.ledger().timestamp();
    if now >= m.kickoff_time {
        return Err(PredictXError::MatchAlreadyStarted);
    }

    if let Some(v) = home_team  { m.home_team = v; }
    if let Some(v) = away_team  { m.away_team = v; }
    if let Some(v) = league     { m.league    = v; }
    if let Some(v) = venue      { m.venue     = v; }
    if let Some(kt) = kickoff_time {
        if kt <= now {
            return Err(PredictXError::InvalidLockTime);
        }
        m.kickoff_time = kt;
    }

    env.storage()
        .persistent()
        .set(&DataKey::Match(match_id), &m);

    env.events().publish(
        (Symbol::new(env, "MatchUpdated"), match_id),
        m.clone(),
    );

    Ok(m)
}

pub fn finish_match(
    env: &Env,
    admin: Address,
    match_id: u64,
) -> Result<(), PredictXError> {
    require_admin(env, &admin)?;

    let mut m: Match = env
        .storage()
        .persistent()
        .get(&DataKey::Match(match_id))
        .ok_or(PredictXError::MatchNotFound)?;

    m.is_finished = true;
    env.storage()
        .persistent()
        .set(&DataKey::Match(match_id), &m);

    env.events().publish(
        (Symbol::new(env, "MatchFinished"), match_id),
        (),
    );

    Ok(())
}

pub fn get_match(env: &Env, match_id: u64) -> Result<Match, PredictXError> {
    env.storage()
        .persistent()
        .get(&DataKey::Match(match_id))
        .ok_or(PredictXError::MatchNotFound)
}

pub fn get_match_polls(env: &Env, match_id: u64) -> Result<Vec<u64>, PredictXError> {
    if !env.storage().persistent().has(&DataKey::Match(match_id)) {
        return Err(PredictXError::MatchNotFound);
    }
    Ok(env
        .storage()
        .persistent()
        .get(&DataKey::MatchPolls(match_id))
        .unwrap_or(Vec::new(env)))
}

pub fn get_match_count(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::NextMatchId)
        .unwrap_or(1u64)
        .saturating_sub(1)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    extern crate std;

    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env, String,
    };

    use crate::{errors::PredictXError, PredictXContract, PredictXContractClient};

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn setup() -> (Env, Address, PredictXContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register(PredictXContract, ());
        let client = PredictXContractClient::new(&env, &cid);
        let admin = Address::generate(&env);
        // initialize() returns () directly — no .unwrap() needed
        client.initialize(&admin);
        env.ledger().with_mut(|l| l.timestamp = 1_000_000);
        (env, admin, client)
    }

    fn s(env: &Env, t: &str) -> String { String::from_str(env, t) }

    const KICKOFF: u64 = 1_003_600;

    fn default_match(env: &Env, client: &PredictXContractClient, admin: &Address) -> u64 {
        // create_match() returns u64 directly — no .unwrap() needed
        client.create_match(
            admin,
            &s(env, "Arsenal"), &s(env, "Chelsea"),
            &s(env, "Premier League"), &s(env, "Emirates"),
            &KICKOFF,
        )
    }

    // ── create_match ──────────────────────────────────────────────────────────

    #[test]
    fn test_create_match_returns_id() {
        let (env, admin, client) = setup();
        assert_eq!(default_match(&env, &client, &admin), 1);
    }

    #[test]
    fn test_create_match_stores_correct_data() {
        let (env, admin, client) = setup();
        let id = default_match(&env, &client, &admin);
        // get_match() returns Match directly — no .unwrap() needed
        let m = client.get_match(&id);
        assert_eq!(m.match_id, 1);
        assert_eq!(m.home_team, s(&env, "Arsenal"));
        assert!(!m.is_finished);
    }

    #[test]
    fn test_create_match_auto_increments() {
        let (env, admin, client) = setup();
        assert_eq!(default_match(&env, &client, &admin), 1);
        assert_eq!(default_match(&env, &client, &admin), 2);
        assert_eq!(client.get_match_count(), 2);
    }

    #[test]
    fn test_create_match_rejects_past_kickoff() {
        let (env, admin, client) = setup();
        let err = client.try_create_match(
            &admin,
            &s(&env, "A"), &s(&env, "B"),
            &s(&env, "L"), &s(&env, "V"),
            &999_999u64,
        ).unwrap_err().unwrap();
        assert_eq!(err, PredictXError::InvalidLockTime);
    }

    #[test]
    fn test_create_match_rejects_non_admin() {
        let (env, _, client) = setup();
        let err = client.try_create_match(
            &Address::generate(&env),
            &s(&env, "A"), &s(&env, "B"),
            &s(&env, "L"), &s(&env, "V"),
            &KICKOFF,
        ).unwrap_err().unwrap();
        assert_eq!(err, PredictXError::Unauthorized);
    }

    #[test]
    fn test_create_match_emits_event() {
        use soroban_sdk::{testutils::Events, Symbol, TryIntoVal};
        let (env, admin, client) = setup();
        default_match(&env, &client, &admin);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        let (_, topics, _) = events.get(0).unwrap();
        let name: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
        assert_eq!(name, Symbol::new(&env, "MatchCreated"));
    }

    // ── update_match ──────────────────────────────────────────────────────────

    #[test]
    fn test_update_match_partial() {
        let (env, admin, client) = setup();
        let id = default_match(&env, &client, &admin);
        // update_match() returns Match directly — no .unwrap() needed
        let updated = client.update_match(
            &admin, &id,
            &Some(s(&env, "Liverpool")), &None, &None, &None, &None,
        );
        assert_eq!(updated.home_team, s(&env, "Liverpool"));
        assert_eq!(updated.away_team, s(&env, "Chelsea"));
    }

    #[test]
    fn test_update_match_after_kickoff_fails() {
        let (env, admin, client) = setup();
        let id = default_match(&env, &client, &admin);
        env.ledger().with_mut(|l| l.timestamp = KICKOFF + 1);
        let err = client.try_update_match(
            &admin, &id,
            &Some(s(&env, "X")), &None, &None, &None, &None,
        ).unwrap_err().unwrap();
        assert_eq!(err, PredictXError::MatchAlreadyStarted);
    }

    #[test]
    fn test_update_nonexistent_match_fails() {
        let (env, admin, client) = setup();
        let err = client.try_update_match(
            &admin, &999u64,
            &None, &None, &None, &None, &None,
        ).unwrap_err().unwrap();
        assert_eq!(err, PredictXError::MatchNotFound);
    }

    #[test]
    fn test_update_match_rejects_non_admin() {
        let (env, admin, client) = setup();
        let id = default_match(&env, &client, &admin);
        let err = client.try_update_match(
            &Address::generate(&env), &id,
            &Some(s(&env, "X")), &None, &None, &None, &None,
        ).unwrap_err().unwrap();
        assert_eq!(err, PredictXError::Unauthorized);
    }

    // ── finish_match ──────────────────────────────────────────────────────────

    #[test]
    fn test_finish_match_sets_flag() {
        let (env, admin, client) = setup();
        let id = default_match(&env, &client, &admin);
        // finish_match() returns () directly — no .unwrap() needed
        client.finish_match(&admin, &id);
        assert!(client.get_match(&id).is_finished);
    }

    #[test]
    fn test_finish_match_emits_event() {
        use soroban_sdk::{testutils::Events, Symbol, TryIntoVal};
        let (env, admin, client) = setup();
        let id = default_match(&env, &client, &admin);
        client.finish_match(&admin, &id);
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        let (_, topics, _) = events.get(0).unwrap();
        let name: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
        assert_eq!(name, Symbol::new(&env, "MatchFinished"));
    }

    #[test]
    fn test_finish_nonexistent_match_fails() {
        let (_, admin, client) = setup();
        let err = client.try_finish_match(&admin, &999u64).unwrap_err().unwrap();
        assert_eq!(err, PredictXError::MatchNotFound);
    }

    #[test]
    fn test_finish_match_rejects_non_admin() {
        let (env, admin, client) = setup();
        let id = default_match(&env, &client, &admin);
        let err = client.try_finish_match(&Address::generate(&env), &id).unwrap_err().unwrap();
        assert_eq!(err, PredictXError::Unauthorized);
    }

    // ── View functions ────────────────────────────────────────────────────────

    #[test]
    fn test_get_match_not_found() {
        let (_, _, client) = setup();
        let err = client.try_get_match(&999u64).unwrap_err().unwrap();
        assert_eq!(err, PredictXError::MatchNotFound);
    }

    #[test]
    fn test_get_match_polls_empty_on_creation() {
        let (env, admin, client) = setup();
        let id = default_match(&env, &client, &admin);
        assert_eq!(client.get_match_polls(&id).len(), 0);
    }

    #[test]
    fn test_get_match_polls_nonexistent_fails() {
        let (_, _, client) = setup();
        let err = client.try_get_match_polls(&999u64).unwrap_err().unwrap();
        assert_eq!(err, PredictXError::MatchNotFound);
    }

    #[test]
    fn test_get_match_count_starts_zero() {
        let (_, _, client) = setup();
        assert_eq!(client.get_match_count(), 0);
    }
}