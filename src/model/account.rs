use rust_decimal::Decimal;
use serde::Serialize;

use super::ClientId;

#[derive(Debug, Default)]
pub struct Account {
    pub available: Decimal,
    pub held: Decimal,
    pub locked: bool,
}

#[derive(Debug, Serialize)]
pub struct AccountSummary {
    pub client: ClientId,
    pub available: Decimal,
    pub held: Decimal,
    pub total: Decimal,
    pub locked: bool,
}
