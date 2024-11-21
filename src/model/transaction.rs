use rust_decimal::Decimal;
use serde::de::{self, Error, Visitor};
use serde::Deserialize;

use super::{ClientId, TransactionId};

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum Transaction {
    Deposit(Deposit),
    Withdrawal(Withdrawal),
    Dispute(Dispute),
    Resolve(Resolve),
    Chargeback(Chargeback),
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Deposit {
    pub client: ClientId,
    #[serde(rename = "tx")]
    pub transaction_id: TransactionId,
    pub amount: Decimal,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Withdrawal {
    pub client: ClientId,
    #[serde(rename = "tx")]
    pub transaction_id: TransactionId,
    pub amount: Decimal,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Dispute {
    pub client: ClientId,
    #[serde(rename = "tx")]
    pub transaction_id: TransactionId,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Resolve {
    pub client: ClientId,
    #[serde(rename = "tx")]
    pub transaction_id: TransactionId,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(PartialEq))]

pub struct Chargeback {
    pub client: ClientId,
    #[serde(rename = "tx")]
    pub transaction_id: TransactionId,
}

impl Transaction {
    pub fn client_id(&self) -> ClientId {
        match self {
            Transaction::Deposit(t) => t.client,
            Transaction::Withdrawal(t) => t.client,
            Transaction::Dispute(t) => t.client,
            Transaction::Resolve(t) => t.client,
            Transaction::Chargeback(t) => t.client,
        }
    }

    pub fn tx_id(&self) -> TransactionId {
        match self {
            Transaction::Deposit(t) => t.transaction_id,
            Transaction::Withdrawal(t) => t.transaction_id,
            Transaction::Dispute(t) => t.transaction_id,
            Transaction::Resolve(t) => t.transaction_id,
            Transaction::Chargeback(t) => t.transaction_id,
        }
    }
}

impl<'de> Deserialize<'de> for Transaction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(TransactionVisitor)
    }
}

struct TransactionVisitor;

impl<'de> Visitor<'de> for TransactionVisitor {
    type Value = Transaction;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("Transaction")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let tag: &'de str = seq
            .next_element()?
            .ok_or_else(|| A::Error::missing_field("Missing enum variant tag"))?;

        let variant = de::value::SeqAccessDeserializer::new(seq);

        match tag {
            "deposit" => Deposit::deserialize(variant).map(Transaction::Deposit),
            "withdrawal" => Withdrawal::deserialize(variant).map(Transaction::Withdrawal),
            "dispute" => Dispute::deserialize(variant).map(Transaction::Dispute),
            "resolve" => Resolve::deserialize(variant).map(Transaction::Resolve),
            "chargeback" => Chargeback::deserialize(variant).map(Transaction::Chargeback),
            other => Err(A::Error::unknown_variant(
                other,
                &["deposit", "withdrawal", "dispute", "resolve", "chargeback"],
            )),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use csv::{ReaderBuilder, Trim};

    #[test]
    fn test_csv_transaction_deserialization() {
        let csv = indoc::indoc! {"
            type, client, tx, amount
            deposit, 1, 1, 1.0
            withdrawal, 2, 2, 2.0
            dispute, 1, 1,
            resolve, 1, 1,
            chargeback, 2, 2,
        "};

        let expected = [
            Transaction::Deposit(Deposit {
                client: 1.into(),
                transaction_id: 1.into(),
                amount: Decimal::new(10, 1),
            }),
            Transaction::Withdrawal(Withdrawal {
                client: 2.into(),
                transaction_id: 2.into(),
                amount: Decimal::new(20, 1),
            }),
            Transaction::Dispute(Dispute {
                client: 1.into(),
                transaction_id: 1.into(),
            }),
            Transaction::Resolve(Resolve {
                client: 1.into(),
                transaction_id: 1.into(),
            }),
            Transaction::Chargeback(Chargeback {
                client: 2.into(),
                transaction_id: 2.into(),
            }),
        ];

        let mut reader = ReaderBuilder::new()
            .trim(Trim::All)
            .from_reader(csv.as_bytes());

        let iter = reader.deserialize();

        for (record, expected) in iter.zip(expected) {
            assert_eq!(expected, record.unwrap());
        }
    }
}
