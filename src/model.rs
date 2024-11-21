mod account;
mod transaction;

use std::fmt::Display;

pub use account::{Account, AccountSummary};
use serde::{Deserialize, Serialize};
pub use transaction::{Chargeback, Deposit, Dispute, Resolve, Transaction, Withdrawal};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[repr(transparent)]
pub struct ClientId(u16);

impl From<u16> for ClientId {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl Display for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Deserialize)]
#[repr(transparent)]
pub struct TransactionId(u32);

impl From<u32> for TransactionId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl Display for TransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
