use soroban_sdk::{contracttype, Address, String};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PollStatus {
    Open = 0,
    Locked = 1,
    Resolved = 2,
    Disputed = 3,
    Cancelled = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Poll {
    pub id: u64,
    pub creator: Address,
    pub question: String,
    pub status: PollStatus,
    pub lock_timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stake {
    pub poll_id: u64,
    pub staker: Address,
    pub amount: i128,
    /// `true` = Yes, `false` = No.
    pub side: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vote {
    pub poll_id: u64,
    pub voter: Address,
    /// `true` = Yes, `false` = No.
    pub outcome: bool,
}
