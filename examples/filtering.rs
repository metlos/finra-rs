use std::{env, sync::Arc};

use finra_rs::{
    ConsolidatedShortInterest, ConsolidatedShortInterestField, ConsolidatedShortInterestQuery,
    Finra, Result,
};
use futures::{StreamExt, TryStreamExt};
use reqwest::ClientBuilder;
use time::macros::date;

#[tokio::main]
async fn main() {
    let finra = Finra::new(
        Arc::new(ClientBuilder::new),
        env::var("CLIENT_ID").unwrap(),
        env::var("CLIENT_SECRET").unwrap(),
        true,
    );

    let stream = match finra
        .consolidated_short_interest(ConsolidatedShortInterestQuery::new(
            // limit the size of the returned data by specifying only the fields that are needed
            Some(vec![
                ConsolidatedShortInterestField::SymbolCode,
                ConsolidatedShortInterestField::SettlementDate,
                ConsolidatedShortInterestField::ChangePercent,
            ]),
            // limit the date range for the data
            Some(date!(2024 - 01 - 01)..date!(2024 - 02 - 01)),
            // limit for which symbol to fetch the data
            Some("BDRBF".to_string()),
        ))
        .await
    {
        Ok(s) => s,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    let _ = stream
        .into_stream()
        .collect::<Vec<Result<ConsolidatedShortInterest>>>()
        .await;
}
