use std::collections::HashMap;

use rust_decimal::Decimal;
use thiserror::Error;

use crate::model::{Account, AccountSummary, ClientId, Transaction, TransactionId};

#[derive(Debug, Error)]
#[cfg_attr(test, derive(PartialEq))]
pub enum ProcessingErrorKind {
    #[error("Cannot execute transactions with negative amount")]
    NegativeAmount,
    #[error("Not sufficient funds for executing transaction")]
    NotSufficientFunds,
    #[error("Dispute references transaction that already disputed")]
    DisputeReferencesAlreadyDisputedTx,
    #[error("Dispute transaction cannot be handled")]
    NotSufficientFundsForDispute,
    #[error("Cannot resolve transaction when not under dispute")]
    ResolveWhenTxNotUnderDispute,
    #[error("Cannot chargeback transaction when not under dispute")]
    ChargebackWhenTxNotUnderDispute,
}

#[derive(Debug, Error)]
#[error("client={client} tx={tx}. Error: {kind}")]
pub struct ProcessingError {
    client: ClientId,
    tx: TransactionId,
    kind: ProcessingErrorKind,
}

#[derive(Default)]
pub struct TransactionProcessor {
    accounts: HashMap<ClientId, Account>,
    transactions: HashMap<TransactionId, TransactionState>,
}

#[derive(Debug)]
struct TransactionState {
    amount: Decimal,
    is_under_dispute: bool,
    is_deposit: bool,
}

impl TransactionProcessor {
    pub fn handle(&mut self, tx: Transaction) {
        let account = self.accounts.entry(tx.client_id()).or_default();

        // we skip processing an account that has been locked or if a transaction resulted in an error
        if account.locked || account.error.is_some() {
            return;
        }

        match &tx {
            Transaction::Deposit(deposit) => {
                if deposit.amount < Decimal::ZERO {
                    account.error = Some(ProcessingError {
                        client: deposit.client,
                        tx: deposit.transaction_id,
                        kind: ProcessingErrorKind::NegativeAmount,
                    });

                    return;
                }

                account.available += deposit.amount;
            }
            Transaction::Withdrawal(withdrawal) => {
                if withdrawal.amount < Decimal::ZERO {
                    account.error = Some(ProcessingError {
                        client: withdrawal.client,
                        tx: withdrawal.transaction_id,
                        kind: ProcessingErrorKind::NegativeAmount,
                    });

                    return;
                }

                if withdrawal.amount > account.available {
                    account.error = Some(ProcessingError {
                        client: withdrawal.client,
                        tx: withdrawal.transaction_id,
                        kind: ProcessingErrorKind::NotSufficientFunds,
                    });

                    return;
                }

                account.available -= withdrawal.amount;
            }
            Transaction::Dispute(dispute) => {
                let Some(tx_state) = self.transactions.get_mut(&dispute.transaction_id) else {
                    return;
                };

                if tx_state.is_under_dispute {
                    account.error = Some(ProcessingError {
                        client: dispute.client,
                        tx: dispute.transaction_id,
                        kind: ProcessingErrorKind::DisputeReferencesAlreadyDisputedTx,
                    });

                    return;
                }

                if tx_state.is_deposit {
                    if tx_state.amount > account.available {
                        account.error = Some(ProcessingError {
                            client: dispute.client,
                            tx: dispute.transaction_id,
                            kind: ProcessingErrorKind::NotSufficientFundsForDispute,
                        });

                        return;
                    }

                    account.available -= tx_state.amount;
                    account.held += tx_state.amount;
                } else {
                    account.held += tx_state.amount;
                }

                tx_state.is_under_dispute = true;
            }
            Transaction::Resolve(resolve) => {
                let Some(tx_state) = self.transactions.get_mut(&resolve.transaction_id) else {
                    return;
                };

                if !tx_state.is_under_dispute {
                    account.error = Some(ProcessingError {
                        client: resolve.client,
                        tx: resolve.transaction_id,
                        kind: ProcessingErrorKind::ResolveWhenTxNotUnderDispute,
                    });

                    return;
                }

                account.available += tx_state.amount;
                account.held -= tx_state.amount;

                tx_state.is_under_dispute = false;
            }
            Transaction::Chargeback(chargeback) => {
                let Some(tx_state) = self.transactions.get_mut(&chargeback.transaction_id) else {
                    return;
                };

                if !tx_state.is_under_dispute {
                    account.error = Some(ProcessingError {
                        client: chargeback.client,
                        tx: chargeback.transaction_id,
                        kind: ProcessingErrorKind::ChargebackWhenTxNotUnderDispute,
                    });

                    return;
                }

                if tx_state.is_deposit {
                    account.held -= tx_state.amount;
                } else {
                    account.available += tx_state.amount;
                    account.held -= tx_state.amount;
                }

                account.locked = true;
                tx_state.is_under_dispute = false;
            }
        }

        self.add_transaction(tx);
    }

    pub fn summary(self) -> impl Iterator<Item = AccountSummary> {
        self.accounts
            .into_iter()
            .filter(|(_, client)| client.error.is_none())
            .map(|(client, account)| {
                let available = account.available;
                let held = account.held;

                AccountSummary {
                    client,
                    available,
                    held,
                    total: available + held,
                    locked: account.locked,
                }
            })
    }

    fn add_transaction(&mut self, tx: Transaction) {
        let tx_id = tx.tx_id();

        let state = match tx {
            Transaction::Deposit(deposit) => TransactionState {
                amount: deposit.amount,
                is_under_dispute: false,
                is_deposit: true,
            },
            Transaction::Withdrawal(withdrawal) => TransactionState {
                amount: withdrawal.amount,
                is_under_dispute: false,
                is_deposit: false,
            },
            Transaction::Dispute(_) | Transaction::Resolve(_) | Transaction::Chargeback(_) => {
                return
            }
        };

        self.transactions.insert(tx_id, state);
    }
}

#[cfg(test)]
mod test {
    use rust_decimal::Decimal;

    use crate::{
        model::{
            Chargeback, ClientId, Deposit, Dispute, Resolve, Transaction, TransactionId, Withdrawal,
        },
        processor::TransactionProcessor,
    };

    use super::*;

    #[test]
    fn test_trying_to_revert_withdrawn_funds_locks_account() {
        let mut processor = TransactionProcessor::default();

        for tx in [
            deposit(1.into(), 1.into(), Decimal::new(15, 1)),
            deposit(1.into(), 2.into(), Decimal::new(15, 1)),
            withdraw(1.into(), 3.into(), Decimal::new(15, 1)),
            dispute(1.into(), 1.into()),
            chargeback(1.into(), 1.into()),
        ] {
            processor.handle(tx)
        }

        dbg!(&processor.accounts[&ClientId::from(1)]);

        let summary = processor.summary().next().unwrap();

        assert_eq!(summary.client, 1.into());
        assert!(summary.locked);
        assert_eq!(summary.total, Decimal::ZERO);
    }

    #[test]
    fn test_chargeback_locks_account() {
        let mut processor = TransactionProcessor::default();

        for tx in [
            deposit(1.into(), 2.into(), Decimal::new(15, 1)),
            dispute(1.into(), 2.into()),
            chargeback(1.into(), 2.into()),
        ] {
            processor.handle(tx)
        }

        assert!(processor.accounts[&ClientId::from(1)].error.is_none());

        let summary = processor.summary().next().unwrap();

        assert_eq!(summary.client, 1.into());
        assert!(summary.locked);
        assert_eq!(summary.total, Decimal::ZERO);
    }

    #[test]
    fn test_dispute_withdrawal() {
        let mut processor = TransactionProcessor::default();

        for tx in [
            deposit(1.into(), 2.into(), Decimal::new(155, 1)),
            withdraw(1.into(), 3.into(), Decimal::new(50, 1)),
            dispute(1.into(), 3.into()),
        ] {
            processor.handle(tx);
        }

        assert!(processor.accounts[&ClientId::from(1)].error.is_none());

        let summary = processor.summary().next().unwrap();

        assert_eq!(summary.client, 1.into());
        assert!(!summary.locked);
        assert_eq!(summary.available, Decimal::new(105, 1));
        assert_eq!(summary.held, Decimal::new(50, 1));
    }

    #[test]
    fn test_resolve_fails_if_transaction_not_under_dispute() {
        let mut processor = TransactionProcessor::default();

        processor.handle(deposit(1.into(), 2.into(), Decimal::new(15, 1)));
        processor.handle(resolve(1.into(), 2.into()));

        check_error_kind(
            &processor.accounts[&ClientId::from(1)],
            ProcessingErrorKind::ResolveWhenTxNotUnderDispute,
        );
    }

    #[test]
    fn test_dispute_fails_if_not_sufficient_funds() {
        let mut processor = TransactionProcessor::default();

        for tx in [
            deposit(1.into(), 2.into(), Decimal::new(15, 1)),
            withdraw(1.into(), 3.into(), Decimal::new(5, 1)),
        ] {
            processor.handle(tx);
        }
        processor.handle(dispute(1.into(), 2.into()));

        check_error_kind(
            &processor.accounts[&ClientId::from(1)],
            ProcessingErrorKind::NotSufficientFundsForDispute,
        );
    }

    #[test]
    fn test_returns_error_on_transactions_with_negative_amounts() {
        let mut processor = TransactionProcessor::default();

        for tx in [
            deposit(1.into(), 2.into(), Decimal::new(-10, 1)),
            withdraw(1.into(), 3.into(), Decimal::new(-5, 1)),
        ] {
            processor.handle(tx);

            check_error_kind(
                &processor.accounts[&ClientId::from(1)],
                ProcessingErrorKind::NegativeAmount,
            );
        }
    }

    #[test]
    fn test_returns_error_on_withdrawal_with_with_insufficient_funds() {
        let mut processor = TransactionProcessor::default();
        let tx = withdraw(1.into(), 2.into(), Decimal::new(20, 1));

        processor.handle(tx);

        check_error_kind(
            &processor.accounts[&ClientId::from(1)],
            ProcessingErrorKind::NotSufficientFunds,
        );
    }

    #[test]
    fn test_dispute_fails_when_transaction_already_disputed() {
        let mut processor = TransactionProcessor::default();
        for tx in [
            deposit(1.into(), 2.into(), Decimal::new(20, 1)),
            dispute(1.into(), 2.into()),
        ] {
            processor.handle(tx);
        }

        processor.handle(dispute(1.into(), 2.into()));

        check_error_kind(
            &processor.accounts[&ClientId::from(1)],
            ProcessingErrorKind::DisputeReferencesAlreadyDisputedTx,
        );
    }

    fn deposit(client: ClientId, tx: TransactionId, amt: Decimal) -> Transaction {
        Transaction::Deposit(Deposit {
            client,
            transaction_id: tx,
            amount: amt,
        })
    }

    fn withdraw(client: ClientId, tx: TransactionId, amt: Decimal) -> Transaction {
        Transaction::Withdrawal(Withdrawal {
            client,
            transaction_id: tx,
            amount: amt,
        })
    }

    fn dispute(client: ClientId, tx: TransactionId) -> Transaction {
        Transaction::Dispute(Dispute {
            client,
            transaction_id: tx,
        })
    }

    fn resolve(client: ClientId, tx: TransactionId) -> Transaction {
        Transaction::Resolve(Resolve {
            client,
            transaction_id: tx,
        })
    }

    fn chargeback(client: ClientId, tx: TransactionId) -> Transaction {
        Transaction::Chargeback(Chargeback {
            client,
            transaction_id: tx,
        })
    }

    #[track_caller]
    fn check_error_kind(account: &Account, expected_error_kind: ProcessingErrorKind) {
        let error = account.error.as_ref().map(|e| &e.kind);

        assert_eq!(Some(&expected_error_kind), error);
    }
}
