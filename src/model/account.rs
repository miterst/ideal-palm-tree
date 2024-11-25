use rust_decimal::Decimal;
use serde::Serialize;

use crate::processor::ProcessingError;

use super::ClientId;

#[derive(Debug, Default)]
pub struct Account {
    pub available: Decimal,
    pub held: Decimal,
    pub locked: bool,
    pub error: Option<ProcessingError>,
}

#[derive(Debug, Serialize)]
pub struct AccountSummary {
    pub client: ClientId,
    pub available: Decimal,
    pub held: Decimal,
    pub total: Decimal,
    pub locked: bool,
}
