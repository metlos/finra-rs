use std::io::BufReader;

use crate::{error::Result, Error, Query};
use futures::{stream, TryStream};
use reqwest::{header, Client, IntoUrl, StatusCode, Url};
use serde::de::DeserializeOwned;

struct PagerState<Q: Query> {
    client: Client,
    url: Url,
    query: Q,
    end: bool,
}

/// Gets all the results of the query as a stream. The pagination query parameters are
/// automatically added.
pub async fn all_results<T, Q>(
    client: Client,
    url: impl IntoUrl,
    query: Q,
) -> Result<impl TryStream<Ok = Vec<T>, Error = Error>>
where
    T: DeserializeOwned,
    Q: Query,
{
    Ok(stream::try_unfold(
        PagerState {
            client,
            url: url.into_url()?,
            query,
            end: false,
        },
        |state| {
            Box::pin(async move {
                if state.end {
                    return Ok(None);
                }

                let response = state
                    .client
                    .post(state.url.clone())
                    .header(header::ACCEPT, "text/plain")
                    .header(header::CONTENT_TYPE, "application/json")
                    .json(&state.query)
                    .send()
                    .await?
                    .error_for_status()?;

                if response.status() != StatusCode::OK {
                    // this includes 204 - no content
                    return Ok(None);
                }

                let total: u64 = response
                    .headers()
                    .get("Record-Total")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);

                let body = response.text().await?;
                let mut rdr =
                    csv::ReaderBuilder::new().from_reader(BufReader::new(body.as_bytes()));
                let items: Vec<T> = rdr.deserialize().flatten().collect();

                let new_query = state.query.move_cursor(items.len() as u64);

                let end = total <= new_query.offset();

                Ok(Some((
                    items,
                    PagerState {
                        client: state.client,
                        url: state.url,
                        query: new_query,
                        end,
                    },
                )))
            })
        },
    ))
}
