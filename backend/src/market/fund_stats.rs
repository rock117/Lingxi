//! A 股全市场财报统计：水平面（赚钱/现金流）+ 增长面（利润/营收同比）+
//! 各「面」相对上年同期 / 上一报告期的百分点升降汇总。

use chrono::Datelike;
use sea_orm::{DatabaseBackend, DatabaseConnection, FromQueryResult, Statement};
use serde::Serialize;

/// 用户可选期次（报告期顺序：Q1 → H1 → Q3 → FY）。
pub const PERIODS: &[(&str, &str)] = &[
    ("Q1", "一季报"),
    ("H1", "中报"),
    ("Q3", "三季报"),
    ("FY", "年报"),
];

#[derive(Debug, Clone, Serialize)]
pub struct RatioStat {
    /// 有有效字段的公司数（分母）
    pub sample: i64,
    /// 满足条件的公司数（分子）
    pub positive: i64,
    /// sample - positive（亏损 / 未增长 / CF≤0）
    pub non_positive: i64,
    /// positive / sample * 100；sample=0 时为 null
    pub positive_pct: Option<f64>,
    /// 相对上年同口径「面」的百分点差（本期 − 上年同期）
    pub yoy_delta_pct: Option<f64>,
    /// 相对上一报告期「面」的百分点差（本期 − 上期）
    pub qoq_delta_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeriodRef {
    pub year: i32,
    pub period: String,
    pub period_label: String,
}

/// 四项「面」同比/环比升降家数汇总（有有效 delta 才计入）。
#[derive(Debug, Clone, Serialize)]
pub struct FaceChangeSummary {
    pub yoy_up: i32,
    pub yoy_down: i32,
    pub yoy_flat: i32,
    pub qoq_up: i32,
    pub qoq_down: i32,
    pub qoq_flat: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct FundStatsResult {
    pub year: i32,
    pub period: String,
    pub period_label: String,
    /// 该期次下有数据的年份（降序）
    pub available_years: Vec<i32>,
    /// 同比对比基期（上年同口径）
    pub yoy_base: Option<PeriodRef>,
    /// 环比对比基期（上一报告期）
    pub qoq_base: Option<PeriodRef>,
    /// 归母净利 > 0（水平面）
    pub net_profit: RatioStat,
    /// 归母净利同比 > 0（增长面；需上年同口径）
    pub net_profit_yoy: RatioStat,
    /// 营收同比 > 0（增长面；需上年同口径）
    pub revenue_yoy: RatioStat,
    /// 经营现金流净额 > 0（水平面）
    pub operating_cf: RatioStat,
    /// 四项面的同比/环比升降汇总
    pub summary: FaceChangeSummary,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FundStatsMeta {
    pub periods: Vec<PeriodOption>,
    pub default_year: Option<i32>,
    pub default_period: String,
    pub available_years: Vec<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeriodOption {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Default)]
struct RawRatios {
    net_profit: (i64, i64),
    net_profit_yoy: (i64, i64),
    revenue_yoy: (i64, i64),
    operating_cf: (i64, i64),
}

fn period_label(period: &str) -> String {
    PERIODS
        .iter()
        .find(|(id, _)| id.eq_ignore_ascii_case(period))
        .map(|(_, l)| (*l).to_string())
        .unwrap_or_else(|| period.to_string())
}

fn normalize_period_param(raw: Option<&str>) -> String {
    let p = raw.unwrap_or("FY").trim().to_uppercase();
    match p.as_str() {
        "Q1" | "1" => "Q1".into(),
        "H1" | "Q2" | "HY" | "SEMI" | "中报" => "H1".into(),
        "Q3" | "3" => "Q3".into(),
        "FY" | "Q4" | "A" | "ANNUAL" | "年报" | "Y" => "FY".into(),
        _ => "FY".into(),
    }
}

/// 库中 fiscal_period 可能写法 → 规范期次。
fn period_sql_in_list(period: &str) -> &'static [&'static str] {
    match period {
        "Q1" => &["Q1", "1"],
        "H1" => &["H1", "Q2", "HY", "SEMI", "S1"],
        "Q3" => &["Q3", "3"],
        _ => &["FY", "Q4", "A", "ANNUAL", "Y", "YEAR"],
    }
}

/// 上一报告期：Q1←上年 FY，H1←Q1，Q3←H1，FY←Q3。
fn previous_report_period(year: i32, period: &str) -> (i32, String) {
    match period {
        "Q1" => (year - 1, "FY".into()),
        "H1" => (year, "Q1".into()),
        "Q3" => (year, "H1".into()),
        _ => (year, "Q3".into()),
    }
}

fn ratio_pct(sample: i64, positive: i64) -> Option<f64> {
    if sample > 0 {
        Some((positive as f64) / (sample as f64) * 100.0)
    } else {
        None
    }
}

fn delta_pct(cur: Option<f64>, base: Option<f64>) -> Option<f64> {
    match (cur, base) {
        (Some(c), Some(b)) => Some(c - b),
        _ => None,
    }
}

fn ratio_with_deltas(
    sample: i64,
    positive: i64,
    yoy_base: Option<(i64, i64)>,
    qoq_base: Option<(i64, i64)>,
) -> RatioStat {
    let non_positive = (sample - positive).max(0);
    let positive_pct = ratio_pct(sample, positive);
    let yoy_delta_pct = yoy_base.and_then(|(s, p)| delta_pct(positive_pct, ratio_pct(s, p)));
    let qoq_delta_pct = qoq_base.and_then(|(s, p)| delta_pct(positive_pct, ratio_pct(s, p)));
    RatioStat {
        sample,
        positive,
        non_positive,
        positive_pct,
        yoy_delta_pct,
        qoq_delta_pct,
    }
}

#[derive(Clone, Copy)]
enum DeltaClass {
    Up,
    Down,
    Flat,
}

fn classify_delta(d: Option<f64>) -> Option<DeltaClass> {
    let v = d?;
    Some(if v > 0.05 {
        DeltaClass::Up
    } else if v < -0.05 {
        DeltaClass::Down
    } else {
        DeltaClass::Flat
    })
}

fn bump_summary(s: &mut FaceChangeSummary, yoy: bool, class: DeltaClass) {
    match (yoy, class) {
        (true, DeltaClass::Up) => s.yoy_up += 1,
        (true, DeltaClass::Down) => s.yoy_down += 1,
        (true, DeltaClass::Flat) => s.yoy_flat += 1,
        (false, DeltaClass::Up) => s.qoq_up += 1,
        (false, DeltaClass::Down) => s.qoq_down += 1,
        (false, DeltaClass::Flat) => s.qoq_flat += 1,
    }
}

fn build_summary(faces: &[&RatioStat]) -> FaceChangeSummary {
    let mut s = FaceChangeSummary {
        yoy_up: 0,
        yoy_down: 0,
        yoy_flat: 0,
        qoq_up: 0,
        qoq_down: 0,
        qoq_flat: 0,
    };
    for f in faces {
        if let Some(c) = classify_delta(f.yoy_delta_pct) {
            bump_summary(&mut s, true, c);
        }
        if let Some(c) = classify_delta(f.qoq_delta_pct) {
            bump_summary(&mut s, false, c);
        }
    }
    s
}

#[derive(Debug, FromQueryResult)]
struct YearRow {
    y: i32,
}

#[derive(Debug, FromQueryResult)]
struct IncomeAggRow {
    np_sample: i64,
    np_pos: i64,
    np_yoy_sample: i64,
    np_yoy_pos: i64,
    rev_sample: i64,
    rev_pos: i64,
}

#[derive(Debug, FromQueryResult)]
struct CfAggRow {
    cf_sample: i64,
    cf_pos: i64,
}

async fn available_years(db: &DatabaseConnection, period: &str) -> Result<Vec<i32>, sea_orm::DbErr> {
    let aliases = period_sql_in_list(period);
    let in_list = aliases
        .iter()
        .map(|a| format!("'{}'", a.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        r#"
SELECT DISTINCT fiscal_year AS y
FROM sc_stock_income_statement
WHERE fiscal_year IS NOT NULL
  AND upper(replace(trim(coalesce(fiscal_period, '')), ' ', '')) IN ({in_list})
ORDER BY y DESC
"#
    );
    let rows = YearRow::find_by_statement(Statement::from_string(
        DatabaseBackend::Postgres,
        sql,
    ))
    .all(db)
    .await?;
    Ok(rows.into_iter().map(|r| r.y).collect())
}

/// 元数据：期次列表 + 默认年/期。
pub async fn load_meta(db: &DatabaseConnection) -> Result<FundStatsMeta, sea_orm::DbErr> {
    let default_period = "FY".to_string();
    let years = available_years(db, &default_period).await?;
    // 若年报尚无，尝试三季报
    let (default_period, years) = if years.is_empty() {
        let y = available_years(db, "Q3").await?;
        if y.is_empty() {
            let y2 = available_years(db, "H1").await?;
            if y2.is_empty() {
                ("Q1".into(), available_years(db, "Q1").await?)
            } else {
                ("H1".into(), y2)
            }
        } else {
            ("Q3".into(), y)
        }
    } else {
        (default_period, years)
    };
    Ok(FundStatsMeta {
        periods: PERIODS
            .iter()
            .map(|(id, label)| PeriodOption {
                id: (*id).into(),
                label: (*label).into(),
            })
            .collect(),
        default_year: years.first().copied(),
        default_period,
        available_years: years,
    })
}

fn parent_np_expr(alias: &str) -> String {
    format!(
        r#"CASE
            WHEN coalesce({a}.payload->>'netProfitToParent', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN ({a}.payload->>'netProfitToParent')::float8
            WHEN coalesce({a}.payload->>'net_profit_to_parent', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN ({a}.payload->>'net_profit_to_parent')::float8
            ELSE NULL
        END"#,
        a = alias
    )
}

fn revenue_expr(alias: &str) -> String {
    format!(
        r#"CASE
            WHEN coalesce({a}.payload->>'operatingRevenue', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN ({a}.payload->>'operatingRevenue')::float8
            WHEN coalesce({a}.payload->>'operating_revenue', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN ({a}.payload->>'operating_revenue')::float8
            WHEN coalesce({a}.payload->>'totalRevenue', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN ({a}.payload->>'totalRevenue')::float8
            WHEN coalesce({a}.payload->>'total_revenue', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN ({a}.payload->>'total_revenue')::float8
            ELSE NULL
        END"#,
        a = alias
    )
}

fn op_cf_expr(alias: &str) -> String {
    format!(
        r#"CASE
            WHEN coalesce({a}.payload->>'netCashFromOperating', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN ({a}.payload->>'netCashFromOperating')::float8
            WHEN coalesce({a}.payload->>'net_cash_from_operating', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN ({a}.payload->>'net_cash_from_operating')::float8
            ELSE NULL
        END"#,
        a = alias
    )
}

async fn load_raw_ratios(
    db: &DatabaseConnection,
    year: i32,
    period: &str,
) -> Result<RawRatios, sea_orm::DbErr> {
    let aliases = period_sql_in_list(period);
    let in_list = aliases
        .iter()
        .map(|a| format!("'{}'", a.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",");
    let np = parent_np_expr("t");
    let rev = revenue_expr("t");
    let prev_year = year - 1;

    let income_sql = format!(
        r#"
WITH cur AS (
    SELECT DISTINCT ON (symbol)
        symbol,
        {np} AS parent_np,
        {rev} AS revenue
    FROM sc_stock_income_statement t
    WHERE fiscal_year = {year}
      AND upper(replace(trim(coalesce(fiscal_period, '')), ' ', '')) IN ({in_list})
    ORDER BY symbol, report_date DESC
),
prev AS (
    SELECT DISTINCT ON (symbol)
        symbol,
        {np} AS parent_np,
        {rev} AS revenue
    FROM sc_stock_income_statement t
    WHERE fiscal_year = {prev_year}
      AND upper(replace(trim(coalesce(fiscal_period, '')), ' ', '')) IN ({in_list})
    ORDER BY symbol, report_date DESC
)
SELECT
    (SELECT count(*) FROM cur WHERE parent_np IS NOT NULL)::bigint AS np_sample,
    (SELECT count(*) FROM cur WHERE parent_np IS NOT NULL AND parent_np > 0)::bigint AS np_pos,
    (SELECT count(*) FROM cur c
        INNER JOIN prev p ON p.symbol = c.symbol
        WHERE c.parent_np IS NOT NULL AND p.parent_np IS NOT NULL AND abs(p.parent_np) > 1e-6
    )::bigint AS np_yoy_sample,
    (SELECT count(*) FROM cur c
        INNER JOIN prev p ON p.symbol = c.symbol
        WHERE c.parent_np IS NOT NULL AND p.parent_np IS NOT NULL AND abs(p.parent_np) > 1e-6
          AND (c.parent_np - p.parent_np) / abs(p.parent_np) > 0
    )::bigint AS np_yoy_pos,
    (SELECT count(*) FROM cur c
        INNER JOIN prev p ON p.symbol = c.symbol
        WHERE c.revenue IS NOT NULL AND p.revenue IS NOT NULL AND abs(p.revenue) > 1e-6
    )::bigint AS rev_sample,
    (SELECT count(*) FROM cur c
        INNER JOIN prev p ON p.symbol = c.symbol
        WHERE c.revenue IS NOT NULL AND p.revenue IS NOT NULL AND abs(p.revenue) > 1e-6
          AND (c.revenue - p.revenue) / abs(p.revenue) > 0
    )::bigint AS rev_pos
"#,
        np = np,
        rev = rev,
        year = year,
        prev_year = prev_year,
        in_list = in_list,
    );

    let cf = op_cf_expr("t");
    let cf_sql = format!(
        r#"
WITH cur AS (
    SELECT DISTINCT ON (symbol)
        symbol,
        {cf} AS op_cf
    FROM sc_stock_cashflow_statement t
    WHERE fiscal_year = {year}
      AND upper(replace(trim(coalesce(fiscal_period, '')), ' ', '')) IN ({in_list})
    ORDER BY symbol, report_date DESC
)
SELECT
    (SELECT count(*) FROM cur WHERE op_cf IS NOT NULL)::bigint AS cf_sample,
    (SELECT count(*) FROM cur WHERE op_cf IS NOT NULL AND op_cf > 0)::bigint AS cf_pos
"#,
        cf = cf,
        year = year,
        in_list = in_list,
    );

    let income_row = IncomeAggRow::find_by_statement(Statement::from_string(
        DatabaseBackend::Postgres,
        income_sql,
    ))
    .one(db)
    .await?;
    let cf_row = CfAggRow::find_by_statement(Statement::from_string(
        DatabaseBackend::Postgres,
        cf_sql,
    ))
    .one(db)
    .await?;

    let mut raw = RawRatios::default();
    if let Some(r) = income_row {
        raw.net_profit = (r.np_sample, r.np_pos);
        raw.net_profit_yoy = (r.np_yoy_sample, r.np_yoy_pos);
        raw.revenue_yoy = (r.rev_sample, r.rev_pos);
    }
    if let Some(r) = cf_row {
        raw.operating_cf = (r.cf_sample, r.cf_pos);
    }
    Ok(raw)
}

pub async fn compute_stats(
    db: &DatabaseConnection,
    year: Option<i32>,
    period: Option<&str>,
) -> Result<FundStatsResult, sea_orm::DbErr> {
    let period = normalize_period_param(period);
    let years = available_years(db, &period).await?;
    let year = year
        .or_else(|| years.first().copied())
        .unwrap_or_else(|| chrono::Local::now().date_naive().year());

    let (qoq_year, qoq_period) = previous_report_period(year, &period);
    let yoy_year = year - 1;

    let cur = load_raw_ratios(db, year, &period).await?;
    let yoy = load_raw_ratios(db, yoy_year, &period).await?;
    let qoq = load_raw_ratios(db, qoq_year, &qoq_period).await?;

    let net_profit = ratio_with_deltas(
        cur.net_profit.0,
        cur.net_profit.1,
        Some(yoy.net_profit),
        Some(qoq.net_profit),
    );
    let net_profit_yoy = ratio_with_deltas(
        cur.net_profit_yoy.0,
        cur.net_profit_yoy.1,
        Some(yoy.net_profit_yoy),
        Some(qoq.net_profit_yoy),
    );
    let revenue_yoy = ratio_with_deltas(
        cur.revenue_yoy.0,
        cur.revenue_yoy.1,
        Some(yoy.revenue_yoy),
        Some(qoq.revenue_yoy),
    );
    let operating_cf = ratio_with_deltas(
        cur.operating_cf.0,
        cur.operating_cf.1,
        Some(yoy.operating_cf),
        Some(qoq.operating_cf),
    );

    // 基期无样本则不展示对应 PeriodRef / delta（ratio_with_deltas 已用 sample=0 → None pct）
    let yoy_base = if yoy.net_profit.0 > 0
        || yoy.net_profit_yoy.0 > 0
        || yoy.revenue_yoy.0 > 0
        || yoy.operating_cf.0 > 0
    {
        Some(PeriodRef {
            year: yoy_year,
            period: period.clone(),
            period_label: period_label(&period),
        })
    } else {
        None
    };
    let qoq_base = if qoq.net_profit.0 > 0
        || qoq.net_profit_yoy.0 > 0
        || qoq.revenue_yoy.0 > 0
        || qoq.operating_cf.0 > 0
    {
        Some(PeriodRef {
            year: qoq_year,
            period: qoq_period.clone(),
            period_label: period_label(&qoq_period),
        })
    } else {
        None
    };

    let summary = build_summary(&[&net_profit, &net_profit_yoy, &revenue_yoy, &operating_cf]);

    Ok(FundStatsResult {
        year,
        period: period.clone(),
        period_label: period_label(&period),
        available_years: years,
        yoy_base,
        qoq_base,
        net_profit,
        net_profit_yoy,
        revenue_yoy,
        operating_cf,
        summary,
        note: "水平面：归母净利>0、经营现金流净额>0。增长面：归母净利/营收相对上年同口径增速>0。同比=较上年同期该「面」百分点差；环比=较上一报告期（Q1←上年年报，H1←Q1，Q3←H1，FY←Q3）。分母均为该期有有效字段的公司数。".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_periods() {
        assert_eq!(normalize_period_param(Some("年报")), "FY");
        assert_eq!(normalize_period_param(Some("q1")), "Q1");
        assert_eq!(normalize_period_param(Some("中报")), "H1");
    }

    #[test]
    fn previous_periods() {
        assert_eq!(previous_report_period(2024, "FY"), (2024, "Q3".into()));
        assert_eq!(previous_report_period(2024, "Q3"), (2024, "H1".into()));
        assert_eq!(previous_report_period(2024, "H1"), (2024, "Q1".into()));
        assert_eq!(previous_report_period(2024, "Q1"), (2023, "FY".into()));
    }

    #[test]
    fn deltas_and_summary() {
        let r = ratio_with_deltas(100, 80, Some((100, 70)), Some((100, 85)));
        assert!((r.positive_pct.unwrap() - 80.0).abs() < 1e-9);
        assert!((r.yoy_delta_pct.unwrap() - 10.0).abs() < 1e-9);
        assert!((r.qoq_delta_pct.unwrap() - (-5.0)).abs() < 1e-9);
        let s = build_summary(&[&r]);
        assert_eq!(s.yoy_up, 1);
        assert_eq!(s.qoq_down, 1);
    }
}
