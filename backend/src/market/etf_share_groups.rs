//! 宽基 ETF 按组份额 / 估算市值。文档：`docs/etf_share_groups.md`。

use chrono::{Datelike, Duration, NaiveDate};
use sea_orm::{DatabaseBackend, DatabaseConnection, FromQueryResult, Statement};
use serde::Serialize;
use std::collections::BTreeMap;

const GROUPS: &[(&str, &str)] = &[
    ("hs300", "沪深300"),
    ("sz50", "上证50"),
    ("kc50", "科创50"),
    ("cyb", "创业板指"),
];

#[derive(Debug, Clone, Serialize)]
pub struct EtfSharePoint {
    pub date: String,
    pub share: f64,
    pub mkt: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EtfShareMember {
    pub symbol: String,
    pub name: String,
    pub share: f64,
    pub mkt: Option<f64>,
    pub delta_share: Option<f64>,
    pub delta_share_pct: Option<f64>,
    pub delta_mkt: Option<f64>,
    pub delta_mkt_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EtfShareGroup {
    pub id: String,
    pub label: String,
    pub latest_share: f64,
    pub latest_mkt: Option<f64>,
    pub delta_share: Option<f64>,
    pub delta_share_pct: Option<f64>,
    pub delta_mkt: Option<f64>,
    pub delta_mkt_pct: Option<f64>,
    pub series: Vec<EtfSharePoint>,
    pub members: Vec<EtfShareMember>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EtfShareGroupsResult {
    pub as_of: String,
    pub range: String,
    pub granularity: String,
    pub start_date: String,
    pub note: String,
    pub groups: Vec<EtfShareGroup>,
}

#[derive(Debug, FromQueryResult)]
struct AsOfRow {
    d: Option<NaiveDate>,
}

#[derive(Debug, FromQueryResult)]
struct RawRow {
    symbol: String,
    name: String,
    index_group: String,
    trade_date: NaiveDate,
    share_units: f64,
    close: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Granularity {
    Day,
    Week,
    Month,
}

impl Granularity {
    fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "week" | "w" | "周" => Self::Week,
            "month" | "m" | "月" => Self::Month,
            _ => Self::Day,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
        }
    }
}

fn parse_range(range: &str) -> &'static str {
    match range.trim().to_ascii_lowercase().as_str() {
        "1m" => "1m",
        "6m" => "6m",
        "ytd" => "ytd",
        "1y" => "1y",
        _ => "3m",
    }
}

fn range_start(as_of: NaiveDate, range: &str) -> NaiveDate {
    match range {
        "1m" => as_of - Duration::days(31),
        "6m" => as_of - Duration::days(183),
        "1y" => as_of - Duration::days(366),
        "ytd" => NaiveDate::from_ymd_opt(as_of.year(), 1, 1).unwrap_or(as_of),
        _ => as_of - Duration::days(92), // 3m
    }
}

fn bucket_key(d: NaiveDate, g: Granularity) -> (i32, u32) {
    match g {
        Granularity::Day => (d.year(), d.ordinal()),
        Granularity::Week => {
            let iso = d.iso_week();
            (iso.year(), iso.week() as u32)
        }
        Granularity::Month => (d.year(), d.month()),
    }
}

fn pct_delta(first: f64, last: f64) -> Option<f64> {
    if first.abs() < f64::EPSILON {
        None
    } else {
        Some((last - first) / first * 100.0)
    }
}

fn abs_delta(first: Option<f64>, last: Option<f64>) -> Option<f64> {
    match (first, last) {
        (Some(a), Some(b)) => Some(b - a),
        _ => None,
    }
}

async fn load_as_of(db: &DatabaseConnection) -> Result<Option<NaiveDate>, sea_orm::DbErr> {
    let row = AsOfRow::find_by_statement(Statement::from_string(
        DatabaseBackend::Postgres,
        String::from("SELECT MAX(trade_date) AS d FROM sc_etf_share_d1"),
    ))
    .one(db)
    .await?;
    Ok(row.and_then(|r| r.d))
}

async fn load_rows(
    db: &DatabaseConnection,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<RawRow>, sea_orm::DbErr> {
    let sql = format!(
        r#"
SELECT
  s.symbol,
  s.name,
  s.index_group,
  s.trade_date,
  s.share_units::float8 AS share_units,
  k.close::float8 AS close
FROM sc_etf_share_d1 s
LEFT JOIN sc_fund_kline_d1_bfq k
  ON k.symbol = s.symbol AND k.trade_date = s.trade_date
WHERE s.trade_date >= DATE '{start}'
  AND s.trade_date <= DATE '{end}'
  AND s.index_group IN ('hs300', 'sz50', 'kc50', 'cyb')
ORDER BY s.trade_date ASC, s.index_group ASC, s.symbol ASC
"#
    );
    RawRow::find_by_statement(Statement::from_string(
        DatabaseBackend::Postgres,
        sql,
    ))
    .all(db)
    .await
}

/// `granularity`: day|week|month；`range`: 1m|3m|6m|ytd|1y
pub async fn compute(
    db: &DatabaseConnection,
    granularity: Option<&str>,
    range: Option<&str>,
) -> Result<EtfShareGroupsResult, sea_orm::DbErr> {
    let gran = Granularity::parse(granularity.unwrap_or("day"));
    let range_id = parse_range(range.unwrap_or("3m"));

    let Some(as_of) = load_as_of(db).await? else {
        return Ok(EtfShareGroupsResult {
            as_of: String::new(),
            range: range_id.into(),
            granularity: gran.as_str().into(),
            start_date: String::new(),
            note: "暂无 sc_etf_share_d1 数据，请先在 SecuritiesCollector 同步 etf_share。".into(),
            groups: GROUPS
                .iter()
                .map(|(id, label)| EtfShareGroup {
                    id: (*id).into(),
                    label: (*label).into(),
                    latest_share: 0.0,
                    latest_mkt: None,
                    delta_share: None,
                    delta_share_pct: None,
                    delta_mkt: None,
                    delta_mkt_pct: None,
                    series: vec![],
                    members: vec![],
                })
                .collect(),
        });
    };

    let start = range_start(as_of, range_id);
    let rows = load_rows(db, start, as_of).await?;

    // date -> group -> (share_sum, mkt_sum)
    let mut daily: BTreeMap<NaiveDate, BTreeMap<String, (f64, f64)>> = BTreeMap::new();
    // symbol meta + first/last snapshots for members
    #[derive(Default)]
    struct SymTrack {
        name: String,
        group: String,
        first_share: Option<f64>,
        first_mkt: Option<f64>,
        last_share: f64,
        last_mkt: Option<f64>,
        last_date: Option<NaiveDate>,
    }
    let mut syms: BTreeMap<String, SymTrack> = BTreeMap::new();

    // Forward-fill last known close so early share days without kline still get mkt.
    let mut last_close: BTreeMap<String, f64> = BTreeMap::new();
    for r in &rows {
        if let Some(c) = r.close {
            if c.is_finite() && c > 0.0 {
                last_close.insert(r.symbol.clone(), c);
            }
        }
        let close = r
            .close
            .filter(|c| c.is_finite() && *c > 0.0)
            .or_else(|| last_close.get(&r.symbol).copied());
        let mkt = close.map(|c| r.share_units * c);
        let gmap = daily.entry(r.trade_date).or_default();
        let e = gmap.entry(r.index_group.clone()).or_insert((0.0, 0.0));
        e.0 += r.share_units;
        if let Some(mv) = mkt {
            e.1 += mv;
        }

        let t = syms.entry(r.symbol.clone()).or_default();
        if t.name.is_empty() {
            t.name = r.name.clone();
            t.group = r.index_group.clone();
        }
        if t.first_share.is_none() {
            t.first_share = Some(r.share_units);
            t.first_mkt = mkt;
        }
        t.last_share = r.share_units;
        t.last_mkt = mkt;
        t.last_date = Some(r.trade_date);
    }

    // Aggregate daily -> granularity (keep last day in each bucket per group)
    let dates: Vec<NaiveDate> = daily.keys().copied().collect();
    let series_dates: Vec<NaiveDate> = if gran == Granularity::Day {
        dates
    } else {
        let mut out = Vec::new();
        for (i, d) in dates.iter().enumerate() {
            let b = bucket_key(*d, gran);
            let is_last = i + 1 == dates.len() || bucket_key(dates[i + 1], gran) != b;
            if is_last {
                out.push(*d);
            }
        }
        out
    };

    let mut groups_out = Vec::with_capacity(GROUPS.len());
    for (gid, label) in GROUPS {
        // Only emit days this group actually has share data (no zero-fill).
        let mut series = Vec::with_capacity(series_dates.len());
        for d in &series_dates {
            let Some((share, mkt)) = daily.get(d).and_then(|m| m.get(*gid).copied()) else {
                continue;
            };
            if share <= 0.0 {
                continue;
            }
            series.push(EtfSharePoint {
                date: d.format("%Y-%m-%d").to_string(),
                share,
                mkt,
            });
        }

        let (latest_share, latest_mkt_raw) = series
            .last()
            .map(|p| (p.share, p.mkt))
            .unwrap_or((0.0, 0.0));
        let first_share_pt = series.iter().find(|p| p.share > 0.0);
        let first_mkt_pt = series.iter().find(|p| p.mkt > 0.0);
        let delta_share = first_share_pt.map(|p| latest_share - p.share);
        let delta_share_pct = first_share_pt.and_then(|p| pct_delta(p.share, latest_share));
        let latest_mkt = if latest_mkt_raw > 0.0 {
            Some(latest_mkt_raw)
        } else {
            None
        };
        let first_mkt = first_mkt_pt.map(|p| p.mkt);
        let delta_mkt = abs_delta(first_mkt, latest_mkt);
        let delta_mkt_pct = match (first_mkt, latest_mkt) {
            (Some(a), Some(b)) => pct_delta(a, b),
            _ => None,
        };

        let mut members: Vec<EtfShareMember> = syms
            .iter()
            .filter(|(_, t)| t.group == *gid && t.last_date == Some(as_of))
            .map(|(symbol, t)| {
                let delta_share = t.first_share.map(|f| t.last_share - f);
                let delta_share_pct = t
                    .first_share
                    .and_then(|f| pct_delta(f, t.last_share));
                let delta_mkt = abs_delta(t.first_mkt, t.last_mkt);
                let delta_mkt_pct = match (t.first_mkt, t.last_mkt) {
                    (Some(a), Some(b)) => pct_delta(a, b),
                    _ => None,
                };
                EtfShareMember {
                    symbol: symbol.clone(),
                    name: t.name.clone(),
                    share: t.last_share,
                    mkt: t.last_mkt,
                    delta_share,
                    delta_share_pct,
                    delta_mkt,
                    delta_mkt_pct,
                }
            })
            .collect();
        members.sort_by(|a, b| {
            b.mkt
                .unwrap_or(0.0)
                .partial_cmp(&a.mkt.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    b.share
                        .partial_cmp(&a.share)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });

        groups_out.push(EtfShareGroup {
            id: (*gid).into(),
            label: (*label).into(),
            latest_share,
            latest_mkt,
            delta_share,
            delta_share_pct,
            delta_mkt,
            delta_mkt_pct,
            series,
            members,
        });
    }

    Ok(EtfShareGroupsResult {
        as_of: as_of.format("%Y-%m-%d").to_string(),
        range: range_id.into(),
        granularity: gran.as_str().into(),
        start_date: start.format("%Y-%m-%d").to_string(),
        note: "市值=份额×收盘估算（缺当日K线时沿用最近收盘）；份额为交易所场内份额，多为T+1披露。".into(),
        groups: groups_out,
    })
}
