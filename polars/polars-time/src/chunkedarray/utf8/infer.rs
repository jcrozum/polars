use super::patterns;
#[cfg(feature = "dtype-date")]
use crate::chunkedarray::date::naive_date_to_date;
use crate::chunkedarray::utf8::patterns::Pattern;
use chrono::{NaiveDate, NaiveDateTime};
use polars_arrow::export::arrow::array::{ArrayRef, PrimitiveArray};
use polars_core::prelude::*;
use polars_core::utils::arrow::types::NativeType;

#[derive(Clone)]
pub struct DatetimeInfer<T> {
    patterns: &'static [&'static str],
    latest: &'static str,
    transform: fn(&str, &str) -> Option<T>,
    logical_type: DataType,
}

impl TryFrom<Pattern> for DatetimeInfer<i64> {
    type Error = PolarsError;

    fn try_from(value: Pattern) -> Result<Self> {
        match value {
            Pattern::DatetimeDMY => Ok(DatetimeInfer {
                patterns: patterns::DATETIME_D_M_Y,
                latest: patterns::DATETIME_D_M_Y[0],
                transform: transform_datetime,
                logical_type: DataType::Datetime(TimeUnit::Microseconds, None),
            }),
            Pattern::DatetimeYMD => Ok(DatetimeInfer {
                patterns: patterns::DATETIME_Y_M_D,
                latest: patterns::DATETIME_Y_M_D[0],
                transform: transform_datetime,
                logical_type: DataType::Datetime(TimeUnit::Microseconds, None),
            }),
            _ => Err(PolarsError::ComputeError(
                "could not convert pattern".into(),
            )),
        }
    }
}

#[cfg(feature = "dtype-date")]
impl TryFrom<Pattern> for DatetimeInfer<i32> {
    type Error = PolarsError;

    fn try_from(value: Pattern) -> Result<Self> {
        match value {
            Pattern::DateDMY => Ok(DatetimeInfer {
                patterns: patterns::DATE_D_M_Y,
                latest: patterns::DATE_D_M_Y[0],
                transform: transform_date,
                logical_type: DataType::Date,
            }),
            Pattern::DateYMD => Ok(DatetimeInfer {
                patterns: patterns::DATE_Y_M_D,
                latest: patterns::DATE_Y_M_D[0],
                transform: transform_date,
                logical_type: DataType::Date,
            }),
            _ => Err(PolarsError::ComputeError(
                "could not convert pattern".into(),
            )),
        }
    }
}

impl<T: NativeType> DatetimeInfer<T> {
    pub fn parse(&mut self, val: &str) -> Option<T> {
        match (self.transform)(val, self.latest) {
            Some(parsed) => Some(parsed),
            // try other patterns
            None => {
                for fmt in self.patterns {
                    if let Some(parsed) = (self.transform)(val, fmt) {
                        self.latest = fmt;
                        return Some(parsed);
                    }
                }
                None
            }
        }
    }

    pub fn coerce_utf8(&mut self, ca: &Utf8Chunked) -> Series {
        let chunks = ca
            .downcast_iter()
            .into_iter()
            .map(|array| {
                let iter = array
                    .into_iter()
                    .map(|opt_val| opt_val.and_then(|val| self.parse(val)));
                Arc::new(PrimitiveArray::from_trusted_len_iter(iter)) as ArrayRef
            })
            .collect();
        match self.logical_type {
            DataType::Date => Int32Chunked::from_chunks(ca.name(), chunks)
                .into_series()
                .cast(&self.logical_type)
                .unwrap(),
            DataType::Datetime(_, _) => Int64Chunked::from_chunks(ca.name(), chunks)
                .into_series()
                .cast(&self.logical_type)
                .unwrap(),
            _ => unreachable!(),
        }
    }
}

#[cfg(feature = "dtype-date")]
fn transform_date(val: &str, fmt: &str) -> Option<i32> {
    NaiveDate::parse_from_str(val, fmt)
        .ok()
        .map(naive_date_to_date)
}

fn transform_datetime(val: &str, fmt: &str) -> Option<i64> {
    NaiveDateTime::parse_from_str(val, fmt)
        .ok()
        .map(datetime_to_timestamp_us)
}

pub fn compile_single(val: &str) -> Option<Pattern> {
    if patterns::DATE_D_M_Y
        .iter()
        .any(|fmt| NaiveDate::parse_from_str(val, fmt).is_ok())
    {
        Some(Pattern::DateDMY)
    } else if patterns::DATE_Y_M_D
        .iter()
        .any(|fmt| NaiveDate::parse_from_str(val, fmt).is_ok())
    {
        Some(Pattern::DateYMD)
    } else if patterns::DATETIME_D_M_Y
        .iter()
        .any(|fmt| NaiveDateTime::parse_from_str(val, fmt).is_ok())
    {
        Some(Pattern::DatetimeDMY)
    } else if patterns::DATETIME_Y_M_D
        .iter()
        .any(|fmt| NaiveDateTime::parse_from_str(val, fmt).is_ok())
    {
        Some(Pattern::DatetimeYMD)
    } else {
        None
    }
}
