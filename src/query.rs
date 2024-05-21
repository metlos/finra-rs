use std::{fmt::Display, ops::Range};

use serde::{
    ser::{SerializeMap, SerializeSeq},
    Serialize,
};
use time::Date;

const MAX_RESULTS_PER_PAGE: u64 = 1000;

pub(crate) trait Query: Serialize {
    fn limit(&self) -> u64;
    fn offset(&self) -> u64;
    fn move_cursor(self, by: u64) -> Self;
}

/// This enum is used to limit which fields are included in the query results.
#[derive(Debug, Clone, Copy)]
pub enum ConsolidatedShortInterestField {
    StockSplitFlag,
    PreviousShortPositionQuantity,
    AverageDailyVolumeQuantity,
    IssueName,
    CurrentShortPositionQuantity,
    ChangePreviousNumber,
    AccountingYearMonthNumber,
    SettlementDate,
    MarketClassCode,
    SymbolCode,
    DaysToCoverQuantity,
    IssuerServicesGroupExchangeCode,
    RevisionFlag,
    ChangePercent,
}

/// Represents the query to limit the number of results. This does not correspond to the generic
/// nature of the queries supported by FINRA but supports the common usecases.
#[derive(Debug)]
pub struct ConsolidatedShortInterestQuery {
    /// If `None`, all fields are included.
    pub fields: Option<Vec<ConsolidatedShortInterestField>>,
    /// If `None`, the full available history is included.
    pub date_range: Option<Range<Date>>,
    // If `None` the data for all symbols is included.
    pub symbol: Option<String>,

    // These are internally used for paging...
    limit: u64,
    offset: u64,
}

struct AsSeq<T: Serialize>(T);
struct ConsolidatedShortInterestQueryDateRange(Range<Date>);
struct ConsolidatedShortInterestQuerySymbolFilter<'a>(&'a str);

impl ConsolidatedShortInterestField {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::StockSplitFlag => "stockSplitFlag",
            Self::PreviousShortPositionQuantity => "previousShortPositionQuantity",
            Self::AverageDailyVolumeQuantity => "averageDailyVolumeQuantity",
            Self::IssueName => "issueName",
            Self::CurrentShortPositionQuantity => "currentShortPositionQuantity",
            Self::ChangePreviousNumber => "changePreviousNumber",
            Self::AccountingYearMonthNumber => "accountingYearMonthNumber",
            Self::SettlementDate => "settlementDate",
            Self::MarketClassCode => "marketClassCode",
            Self::SymbolCode => "symbolCode",
            Self::DaysToCoverQuantity => "daysToCoverQuantity",
            Self::IssuerServicesGroupExchangeCode => "issuerServicesGroupExchangeCode",
            Self::RevisionFlag => "revisionFlag",
            Self::ChangePercent => "changePercent",
        }
    }
}

impl AsRef<str> for ConsolidatedShortInterestField {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Display for ConsolidatedShortInterestField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ConsolidatedShortInterestField {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl ConsolidatedShortInterestQuery {
    pub fn new(
        fields: Option<Vec<ConsolidatedShortInterestField>>,
        date_range: Option<Range<Date>>,
        symbol: Option<String>,
    ) -> Self {
        Self {
            fields,
            date_range,
            symbol,
            limit: MAX_RESULTS_PER_PAGE,
            offset: 0,
        }
    }
}

impl Query for ConsolidatedShortInterestQuery {
    fn limit(&self) -> u64 {
        self.limit
    }

    fn offset(&self) -> u64 {
        self.offset
    }

    fn move_cursor(self, by: u64) -> Self {
        Self {
            fields: self.fields,
            date_range: self.date_range,
            symbol: self.symbol,
            limit: self.limit,
            offset: self.offset + by,
        }
    }
}

impl Serialize for ConsolidatedShortInterestQuery {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let len = 2
            + self.fields.iter().count()
            + self.date_range.iter().count()
            + self.date_range.iter().count();

        let mut map = serializer.serialize_map(Some(len))?;

        if let Some(ref fields) = self.fields {
            map.serialize_entry("fields", fields)?;
        }
        if let Some(ref date_range) = self.date_range {
            map.serialize_entry(
                "dateRangeFilters",
                &AsSeq(ConsolidatedShortInterestQueryDateRange(date_range.clone())),
            )?;
        }

        if let Some(ref symbol) = self.symbol {
            map.serialize_entry(
                "compareFilters",
                &AsSeq(ConsolidatedShortInterestQuerySymbolFilter(symbol)),
            )?;
        }

        map.serialize_entry("limit", &self.limit)?;
        map.serialize_entry("offset", &self.offset)?;

        map.end()
    }
}

impl<T> Serialize for AsSeq<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(1))?;

        seq.serialize_element(&self.0)?;

        seq.end()
    }
}

impl Serialize for ConsolidatedShortInterestQueryDateRange {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(3))?;

        map.serialize_entry("fieldName", "settlementDate")?;

        let start = self.0.start;
        let end = self.0.end;
        map.serialize_entry(
            "startDate",
            &format!(
                "{}-{:02}-{:02}",
                start.year(),
                start.month() as u8,
                start.day()
            ),
        )?;
        map.serialize_entry(
            "endDate",
            &format!("{}-{:02}-{:02}", end.year(), end.month() as u8, end.day()),
        )?;

        map.end()
    }
}

impl<'a> Serialize for ConsolidatedShortInterestQuerySymbolFilter<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("fieldName", "symbolCode")?;
        map.serialize_entry("fieldValue", self.0)?;
        map.serialize_entry("compareType", "EQUAL")?;

        map.end()
    }
}
