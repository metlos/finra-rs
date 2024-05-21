use crate::{pager, ConsolidatedShortInterestQuery, Error, Result};
use base64::Engine;
use futures::{stream, StreamExt, TryStream, TryStreamExt};
use reqwest::{
    header::{self, HeaderValue},
    Client, ClientBuilder, StatusCode,
};
use serde::Deserialize;
use time::{Duration, OffsetDateTime};

#[cfg(not(feature = "tokio"))]
use std::sync::Mutex;
use std::{mem, sync::Arc};

#[cfg(feature = "tokio")]
use tokio::sync::Mutex;

const OAUTH2_ENDPOINT: &str =
    "https://ews.fip.finra.org/fip/rest/ews/oauth2/access_token?grant_type=client_credentials";
const SHORT_INTEREST_ENDPOINT: &str =
    "https://api.finra.org/data/group/otcmarket/name/consolidatedShortInterest";
const MOCK_SHORT_INTEREST_ENDPOINT: &str =
    "https://api.finra.org/data/group/otcmarket/name/consolidatedShortInterestMock";

/// The main entry-point to access the Finra data.
pub struct Finra {
    use_mock_datasets: bool,
    client_getter: Mutex<ClientGetter>,
}

/// Represents the short interest data obtained from Finra for a single stock symbol.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ConsolidatedShortInterest {
    #[serde(rename = "stockSplitFlag")]
    pub stock_split_flag: Option<String>,

    #[serde(rename = "previousShortPositionQuantity")]
    pub previous_short_position_quantity: usize,

    #[serde(rename = "averageDailyVolumeQuantity")]
    pub average_daily_volume_quantity: usize,

    #[serde(rename = "issueName")]
    pub issue_name: String,

    #[serde(rename = "currentShortPositionQuantity")]
    pub current_short_position_quantity: usize,

    #[serde(rename = "changePreviousNumber")]
    pub change_previous_number: isize,

    #[serde(rename = "accountingYearMonthNumber")]
    pub accounting_year_month_number: usize,

    #[serde(rename = "settlementDate")]
    pub settlement_date: String,

    #[serde(rename = "marketClassCode")]
    pub market_class_code: String,

    #[serde(rename = "symbolCode")]
    pub symbol_code: String,

    #[serde(rename = "daysToCoverQuantity")]
    pub days_to_cover_quantity: f64,

    #[serde(rename = "issuerServicesGroupExchangeCode")]
    pub issuer_services_group_exchange_code: String,

    #[serde(rename = "revisionFlag")]
    pub revision_flag: Option<String>,

    #[serde(rename = "changePercent")]
    pub change_percent: f64,
}

#[derive(Clone)]
struct LoginData {
    client_builder: Arc<dyn Fn() -> ClientBuilder>,
    client_id: String,
    client_secret: String,
}

enum ClientGetter {
    Unauthenticated {
        login_data: LoginData,
    },
    Authenticated {
        login_data: LoginData,
        client: Client,
        valid_until: OffsetDateTime,
    },
}

impl Finra {
    /// Creates a new instance. `client_builder` is a function for obtaining new reqwest clients
    /// from builders. You can use this to set up a builder with a proxy or whatever other
    /// requirements you have. The Authorization header will be set based on the tokens obtained
    /// using the provided `client_id` and `client_secret`.
    pub fn new(
        client_builder: Arc<dyn Fn() -> ClientBuilder>,
        client_id: String,
        client_secret: String,
        use_mock_datasets: bool,
    ) -> Self {
        Self {
            client_getter: Mutex::new(ClientGetter::Unauthenticated {
                login_data: LoginData {
                    client_builder,
                    client_id,
                    client_secret,
                },
            }),
            use_mock_datasets,
        }
    }

    /// Queries the consolidated short interest from finra.org. Use the `query` parameter to limit
    /// the size of the data. The full dataset is humongous.
    pub async fn consolidated_short_interest(
        &self,
        query: ConsolidatedShortInterestQuery,
    ) -> Result<impl TryStream<Ok = ConsolidatedShortInterest, Error = Error>> {
        let endpoint = if self.use_mock_datasets {
            MOCK_SHORT_INTEREST_ENDPOINT
        } else {
            SHORT_INTEREST_ENDPOINT
        };

        let cl = self
            .get_client()
            .await?
            .ok_or(Error::CannotConstructHttpClient)?;

        Ok(
            pager::all_results::<ConsolidatedShortInterest, ConsolidatedShortInterestQuery>(
                cl, endpoint, query,
            )
            .await?
            .map_ok(|vs| stream::iter(vs).map(Ok::<ConsolidatedShortInterest, Error>))
            .try_flatten(),
        )
    }

    async fn get_client(&self) -> Result<Option<Client>> {
        #[cfg(feature = "tokio")]
        let mut clg = self.client_getter.lock().await;

        #[cfg(not(feature = "tokio"))]
        let mut clg = self.client_getter.lock().unwrap();

        clg.ensure_authenticated().await?;

        Ok(clg.get_client())
    }
}

impl ClientGetter {
    async fn ensure_authenticated(&mut self) -> Result<()> {
        match self {
            Self::Unauthenticated { login_data } => {
                let ld = login_data.clone();
                self._authenticated_self(ld).await?;
                Ok(())
            }
            Self::Authenticated {
                login_data,
                client: _,
                valid_until,
            } => {
                if time::OffsetDateTime::now_utc() < *valid_until {
                    Ok(())
                } else {
                    let ld = login_data.clone();
                    self._authenticated_self(ld).await?;
                    Ok(())
                }
            }
        }
    }

    fn get_client(&self) -> Option<Client> {
        match self {
            Self::Authenticated {
                client,
                login_data: _,
                valid_until: _,
            } => Some(client.clone()),
            _ => None,
        }
    }

    async fn _authenticated_self(&mut self, login_data: LoginData) -> Result<()> {
        let (cl, validity) = Self::_authenticate_client(login_data.clone()).await?;

        let valid_until = time::OffsetDateTime::now_utc() + validity;
        let login_data = login_data.clone();
        mem::swap(
            self,
            &mut Self::Authenticated {
                login_data,
                client: cl,
                valid_until,
            },
        );

        Ok(())
    }

    async fn _authenticate_client(login_data: LoginData) -> Result<(Client, time::Duration)> {
        let auth_header = "Basic ".to_string()
            + &base64::prelude::BASE64_STANDARD
                .encode(login_data.client_id + ":" + &login_data.client_secret);

        let login_client = (login_data.client_builder)().build()?;
        let login_req = login_client.post(OAUTH2_ENDPOINT);
        let login_req = login_req.header(header::AUTHORIZATION, auth_header);

        let login_response = login_req.send().await?;
        let login_status = login_response.status();
        if login_status != StatusCode::OK {
            return Err(Error::CannotLogin(format!(
                "login attempt failed with status code {}",
                login_status
            )));
        }

        let login_json: serde_json::Value = login_response.json().await?;

        let valid_until = login_json
            .get("expires_in")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                Error::CannotLogin(
                    "the login response didn't contain the expiry of the token".to_string(),
                )
            })?;

        let valid_until = valid_until.parse::<i64>().map_err(|e| {
            Error::CannotLogin(format!(
                "could not parse the token expiry as a number: {}",
                e
            ))
        })?;

        let bearer_header = "Bearer ".to_string()
            + login_json
                .get("access_token")
                .ok_or_else(|| {
                    Error::CannotLogin("access_token not present in the login response".to_string())
                })?
                .as_str()
                .ok_or_else(|| {
                    Error::CannotLogin(
                        "access_token is not a string in the login response".to_string(),
                    )
                })?;

        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_str(&bearer_header)?,
        );

        let client = (login_data.client_builder)()
            .default_headers(headers)
            .build()?;

        Ok((client, Duration::new(valid_until, 0)))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use dotenv::dotenv;
    use reqwest::ClientBuilder;

    use crate::{ConsolidatedShortInterestField, Finra};

    use time::macros::date;

    #[tokio::test]
    async fn consolidated_short_interest() {
        dotenv().ok();

        let finra = Finra::new(
            Arc::new(ClientBuilder::new),
            dotenv::var("CLIENT_ID").unwrap(),
            dotenv::var("CLIENT_SECRET").unwrap(),
            true,
        );

        let stream = match finra
            .consolidated_short_interest(ConsolidatedShortInterestQuery::new(
                Some(vec![
                    ConsolidatedShortInterestField::SymbolCode,
                    ConsolidatedShortInterestField::SettlementDate,
                    ConsolidatedShortInterestField::ChangePercent,
                ]),
                Some(date!(2024 - 01 - 01)..date!(2024 - 02 - 01)),
                Some("BDRBF".to_string()),
            ))
            .await
        {
            Ok(s) => s,
            Err(e) => {
                println!("{}", e);
                panic!()
            }
        };

        let data = stream
            .into_stream()
            .collect::<Vec<Result<ConsolidatedShortInterest>>>()
            .await;

        assert_eq!(2, data.len());
    }
}
