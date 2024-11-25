use std::io;

use anyhow::Context;
use csv::{ReaderBuilder, Trim, WriterBuilder};

use tp::model::Transaction;
use tp::processor::TransactionProcessor;

fn main() -> anyhow::Result<()> {
    let filename = std::env::args()
        .nth(1)
        .context("Missing path to csv file.\nTry running `cargo run -- filename.csv`")?;

    let mut reader = ReaderBuilder::new()
        .trim(Trim::All)
        .from_path(filename)
        .unwrap();

    let mut handler = TransactionProcessor::default();

    for record in reader.deserialize() {
        let transaction: Transaction = record.context("Failed parsing file")?;

        handler.handle(transaction);
    }

    let stdout = io::stdout().lock();
    let mut writer = WriterBuilder::new().from_writer(stdout);

    for record in handler.summary() {
        writer
            .serialize(record)
            .context("Failed producing output")?;
    }

    writer.flush().unwrap();

    Ok(())
}
