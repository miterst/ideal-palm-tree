# tp - toy transaction processor 

This implements a toy transaction processor that processes transactions from a CSV file, updates client accounts, and outputs summary of the final state of the accounts.

## Problem Description

The processor supports the following types of transactions:

1. **Deposit**: Adds funds to a client's account.
2. **Withdrawal**: Deducts funds from a client's account if sufficient funds are available.
3. **Dispute**: Temporarily freezes funds for a transaction under dispute.
4. **Resolve**: Resolves a dispute, unfreezing the associated funds.
5. **Chargeback**: Finalizes a dispute by withdrawing the disputed funds and locking the account.


---

## Features

- Handles deposits, withdrawals, disputes, resolves, and chargebacks.
- Processes transactions sequentially for each client to ensure correctness.
- Outputs final account balances with:
  - **Available funds**: Funds available for use.
  - **Held funds**: Funds frozen due to disputes.
  - **Total funds**: Sum of available and held funds.
  - **Locked status**: Whether the account is locked due to a chargeback.

---

## Usage

### Running and building

- Building the repository:
   ```sh
    cargo build
   ```
- Running the processor:
    ```sh
    $ cargo run -- <csv-file> # outputs the summary to stdout
  
    ```
- Running the test:
    ```sh
    $ cargo test
    ```
    
### Input Format

The input CSV should have the following columns:
-   **type**: Transaction type (deposit, withdrawal, dispute, resolve, or chargeback).
-	**client**: Client ID (u16).
-	**tx**: Transaction ID (u32).
-	**amount**: Transaction amount (optional for disputes/resolves/chargebacks).

Example input:
```csv
type,client,tx,amount
deposit,1,1,1.0
deposit,2,2,2.0
withdrawal,1,3,1.5
dispute,1,1,
resolve,1,1,
chargeback,2,2,
```

### Output Format

The output CSV contains the following columns:
- **client**: Client ID.
- **available**: Available funds (up to 4 decimal places).
- **held**: Held funds.
- **total**: Total funds.
- **locked**: Whether the account is locked (true or false).

Example output:
```csv
client,available,held,total,locked
1,0.5,0.0,0.5,false
2,0.0,0.0,0.0,true
``` 

### Assumptions

- Each client has a single asset account.
- Continues processing when encountering an error(skipping accounts for which there was an error).
- Transaction IDs (tx) are unique but may appear in any order.
- Transactions are processed in the order they appear in the file.
- Invalid transactions (e.g., referencing non-existent transactions) are ignored.

### Limitations

- The engine does not persist state. It operates entirely in memory and processes a single CSV at a time.
- Sequential processing, can be only parallelized to transactions across different clients. 

### Future Improvements

- Persistence: Add database support for long-term storage of transactions and account states.
- Streaming Output: Write output incrementally for extremely large datasets.
- API Integration: Expose the engine as a REST or gRPC service.
- Parallelize using actors
- Try to implement it with [differential dataflow](https://crates.io/crates/differential-dataflow)