use std::collections::HashMap;

use rust_decimal::Decimal;
use thiserror::Error;

use crate::model::{Account, AccountSummary, ClientId, Transaction, TransactionId};

#[derive(Debug, Error)]
pub enum ProcessingError {
    #[error("Cannot execute transactions with negative amount. client={client} tx={tx}")]
    NegativeAmount { client: ClientId, tx: TransactionId },
    #[error("Cannot execute transactions on a locked account. client={client} tx={tx}")]
    LockedAccount { client: ClientId, tx: TransactionId },
    #[error("Not sufficient funds for withdrawal client={client} tx={tx}")]
    NotSufficientFundsForWithdrawal { client: ClientId, tx: TransactionId },
    #[error("Dispute references transaction that already disputed. client={client} tx={tx}")]
    DisputeReferencesAlreadyDisputedTx { client: ClientId, tx: TransactionId },
    #[error("Dispute transaction cannot be handled. client={client} tx={tx}")]
    NotSufficientFundsForDispute { client: ClientId, tx: TransactionId },
    #[error("Cannot resolve transaction not under dispute. client={client} tx={tx}")]
    CannotResolveWhenTxNotUnderDispute { client: ClientId, tx: TransactionId },
    #[error("Cannot chargeback transaction not under dispute. client={client} tx={tx}")]
    ChargebackWhenNotUnderDispute { client: ClientId, tx: TransactionId },
}

#[derive(Default)]
pub struct TransactionProcessor {
    accounts: HashMap<ClientId, Account>,
    transactions: HashMap<TransactionId, State>,
}

struct State {
    amount: Decimal,
    is_under_dispute: bool,
    is_deposit: bool,
}

impl TransactionProcessor {
    pub fn handle(&mut self, tx: Transaction) -> Result<(), ProcessingError> {
        let account = self.accounts.entry(tx.client_id()).or_default();

        if account.locked {
            return Ok(());
        }

        match &tx {
            Transaction::Deposit(deposit) => {
                if deposit.amount < Decimal::ZERO {
                    return Err(ProcessingError::NegativeAmount {
                        client: deposit.client,
                        tx: deposit.transaction_id,
                    });
                }

                account.available += deposit.amount;
            }
            Transaction::Withdrawal(withdrawal) => {
                if withdrawal.amount < Decimal::ZERO {
                    return Err(ProcessingError::NegativeAmount {
                        client: withdrawal.client,
                        tx: withdrawal.transaction_id,
                    });
                }

                if withdrawal.amount > account.available {
                    return Err(ProcessingError::NotSufficientFundsForWithdrawal {
                        client: withdrawal.client,
                        tx: withdrawal.transaction_id,
                    });
                }

                account.available -= withdrawal.amount;
            }
            Transaction::Dispute(dispute) => {
                let Some(tx_state) = self.transactions.get_mut(&dispute.transaction_id) else {
                    return Ok(());
                };

                if tx_state.is_under_dispute {
                    return Err(ProcessingError::DisputeReferencesAlreadyDisputedTx {
                        client: dispute.client,
                        tx: dispute.transaction_id,
                    });
                }

                if tx_state.is_deposit {
                    if tx_state.amount > account.available {
                        return Err(ProcessingError::NotSufficientFundsForDispute {
                            client: dispute.client,
                            tx: dispute.transaction_id,
                        });
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
                    return Ok(());
                };

                if !tx_state.is_under_dispute {
                    return Err(ProcessingError::CannotResolveWhenTxNotUnderDispute {
                        client: resolve.client,
                        tx: resolve.transaction_id,
                    });
                }

                account.available += tx_state.amount;
                account.held -= tx_state.amount;

                tx_state.is_under_dispute = false;
            }
            Transaction::Chargeback(chargeback) => {
                let Some(tx_state) = self.transactions.get_mut(&chargeback.transaction_id) else {
                    return Ok(());
                };

                if !tx_state.is_under_dispute {
                    return Err(ProcessingError::ChargebackWhenNotUnderDispute {
                        client: chargeback.client,
                        tx: chargeback.transaction_id,
                    });
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

        Ok(())
    }

    pub fn summary(self) -> impl Iterator<Item = AccountSummary> {
        self.accounts.into_iter().map(|(client, account)| {
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
            Transaction::Deposit(deposit) => State {
                amount: deposit.amount,
                is_under_dispute: false,
                is_deposit: true,
            },
            Transaction::Withdrawal(withdrawal) => State {
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
    fn test_chargeback_locks_account() {
        let mut processor = TransactionProcessor::default();

        for tx in [
            deposit(1.into(), 2.into(), Decimal::new(15, 1)),
            dispute(1.into(), 2.into()),
            chargeback(1.into(), 2.into()),
        ] {
            processor.handle(tx).unwrap()
        }

        let summary = processor.summary().next().unwrap();

        assert_eq!(summary.client, 1.into());
        assert_eq!(summary.locked, true);
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
            processor.handle(tx).unwrap()
        }

        let summary = processor.summary().next().unwrap();

        assert_eq!(summary.client, 1.into());
        assert_eq!(summary.locked, false);
        assert_eq!(summary.available, Decimal::new(105, 1));
        assert_eq!(summary.held, Decimal::new(50, 1));
    }

    #[test]
    fn test_resolve_fails_if_transaction_not_under_dispute() {
        let mut processor = TransactionProcessor::default();

        processor
            .handle(deposit(1.into(), 2.into(), Decimal::new(15, 1)))
            .unwrap();
        let res = processor.handle(resolve(1.into(), 2.into()));

        assert2::let_assert!(Err(ProcessingError::CannotResolveWhenTxNotUnderDispute { .. }) = res);
    }

    #[test]
    fn test_dispute_fails_if_not_sufficient_funds() {
        let mut processor = TransactionProcessor::default();

        for tx in [
            deposit(1.into(), 2.into(), Decimal::new(15, 1)),
            withdraw(1.into(), 3.into(), Decimal::new(5, 1)),
        ] {
            processor.handle(tx).unwrap()
        }
        let res = processor.handle(dispute(1.into(), 2.into()));

        assert2::let_assert!(Err(ProcessingError::NotSufficientFundsForDispute { .. }) = res);
    }

    #[test]
    fn test_returns_error_on_transactions_with_negative_amounts() {
        let mut processor = TransactionProcessor::default();

        for tx in [
            deposit(1.into(), 2.into(), Decimal::new(-10, 1)),
            withdraw(1.into(), 3.into(), Decimal::new(-5, 1)),
        ] {
            let res = processor.handle(tx);
            assert2::let_assert!(Err(ProcessingError::NegativeAmount { .. }) = res);
        }
    }

    #[test]
    fn test_returns_error_on_withdrawal_with_with_insufficient_funds() {
        let mut processor = TransactionProcessor::default();
        let tx = withdraw(1.into(), 2.into(), Decimal::new(20, 1));

        let res = processor.handle(tx);

        assert2::let_assert!(Err(ProcessingError::NotSufficientFundsForWithdrawal { .. }) = res);
    }

    #[test]
    fn test_dispute_fails_when_transaction_already_disputed() {
        let mut processor = TransactionProcessor::default();
        for tx in [
            deposit(1.into(), 2.into(), Decimal::new(20, 1)),
            dispute(1.into(), 2.into()),
        ] {
            processor.handle(tx).unwrap();
        }

        let res = processor.handle(dispute(1.into(), 2.into()));

        assert2::let_assert!(Err(ProcessingError::DisputeReferencesAlreadyDisputedTx { .. }) = res);
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
}
