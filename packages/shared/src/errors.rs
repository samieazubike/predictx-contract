use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    InvalidArgument = 4,
    PollNotFound = 5,
    PollNotOpen = 6,
    PollLocked = 7,
}
