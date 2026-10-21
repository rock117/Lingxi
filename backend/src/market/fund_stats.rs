//! A 股全市场财报统计：赚钱占比、营收增长面、经营现金流为正占比。
//! 按 `fiscal_year` + 期次（Q1/H1/Q3/FY）同口径聚合。

use chrono::Datelike;
use sea_orm::{DatabaseBackend, DatabaseConnection, FromQueryResult, Statement};
use serde::Serialize;

/// 用户可选期次。
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
}

#[derive(Debug, Clone, Serialize)]
pub struct FundStatsResult {
    pub year: i32,
    pub period: String,
    pub period_label: String,
    /// 该期次下有数据的年份（降序）
    pub available_years: Vec<i32>,
    /// 归母净利 > 0
    pub net_profit: RatioStat,
    /// 营收同比 > 0（需上年同口径）
    pub revenue_yoy: RatioStat,
    /// 经营现金流净额 > 0
    pub operating_cf: RatioStat,
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

fn ratio(sample: i64, positive: i64) -> RatioStat {
    let non_positive = (sample - positive).max(0);
    let positive_pct = if sample > 0 {
        Some((positive as f64) / (sample as f64) * 100.0)
    } else {
        None
    };
    RatioStat {
        sample,
        positive,
        non_positive,
        positive_pct,
    }
}

#[derive(Debug, FromQueryResult)]
struct YearRow {
    y: i32,
}

#[derive(Debug, FromQueryResult)]
struct IncomeAggRow {
    np_sample: i64,
    np_pos: i64,
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
    let aliases = period_sql_in_list(&period);
    let in_list = aliases
        .iter()
        .map(|a| format!("'{}'", a.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",");

    let income_sql = format!(
        r#"
WITH cur AS (
    SELECT DISTINCT ON (symbol)
        symbol,
        CASE
            WHEN coalesce(payload->>'netProfitToParent', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN (payload->>'netProfitToParent')::float8
            WHEN coalesce(payload->>'net_profit_to_parent', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN (payload->>'net_profit_to_parent')::float8
            ELSE NULL
        END AS parent_np,
        CASE
            WHEN coalesce(payload->>'operatingRevenue', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN (payload->>'operatingRevenue')::float8
            WHEN coalesce(payload->>'operating_revenue', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN (payload->>'operating_revenue')::float8
            WHEN coalesce(payload->>'totalRevenue', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN (payload->>'totalRevenue')::float8
            WHEN coalesce(payload->>'total_revenue', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN (payload->>'total_revenue')::float8
            ELSE NULL
        END AS revenue
    FROM sc_stock_income_statement
    WHERE fiscal_year = {year}
      AND upper(replace(trim(coalesce(fiscal_period, '')), ' ', '')) IN ({in_list})
    ORDER BY symbol, report_date DESC
),
prev AS (
    SELECT DISTINCT ON (symbol)
        symbol,
        CASE
            WHEN coalesce(payload->>'operatingRevenue', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN (payload->>'operatingRevenue')::float8
            WHEN coalesce(payload->>'operating_revenue', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN (payload->>'operating_revenue')::float8
            WHEN coalesce(payload->>'totalRevenue', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN (payload->>'totalRevenue')::float8
            WHEN coalesce(payload->>'total_revenue', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN (payload->>'total_revenue')::float8
            ELSE NULL
        END AS revenue
    FROM sc_stock_income_statement
    WHERE fiscal_year = {prev_year}
      AND upper(replace(trim(coalesce(fiscal_period, '')), ' ', '')) IN ({in_list})
    ORDER BY symbol, report_date DESC
)
SELECT
    (SELECT count(*) FROM cur WHERE parent_np IS NOT NULL)::bigint AS np_sample,
    (SELECT count(*) FROM cur WHERE parent_np IS NOT NULL AND parent_np > 0)::bigint AS np_pos,
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
        year = year,
        prev_year = year - 1,
        in_list = in_list,
    );

    let cf_sql = format!(
        r#"
WITH cur AS (
    SELECT DISTINCT ON (symbol)
        symbol,
        CASE
            WHEN coalesce(payload->>'netCashFromOperating', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN (payload->>'netCashFromOperating')::float8
            WHEN coalesce(payload->>'net_cash_from_operating', '') ~ '^-?[0-9]+(\.[0-9]+)?$'
                THEN (payload->>'net_cash_from_operating')::float8
            ELSE NULL
        END AS op_cf
    FROM sc_stock_cashflow_statement
    WHERE fiscal_year = {year}
      AND upper(replace(trim(coalesce(fiscal_period, '')), ' ', '')) IN ({in_list})
    ORDER BY symbol, report_date DESC
)
SELECT
    (SELECT count(*) FROM cur WHERE op_cf IS NOT NULL)::bigint AS cf_sample,
    (SELECT count(*) FROM cur WHERE op_cf IS NOT NULL AND op_cf > 0)::bigint AS cf_pos
"#,
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

    let (np_sample, np_pos, rev_sample, rev_pos) = income_row
        .map(|r| (r.np_sample, r.np_pos, r.rev_sample, r.rev_pos))
        .unwrap_or((0, 0, 0, 0));
    let (cf_sample, cf_pos) = cf_row
        .map(|r| (r.cf_sample, r.cf_pos))
        .unwrap_or((0, 0));

    Ok(FundStatsResult {
        year,
        period: period.clone(),
        period_label: period_label(&period),
        available_years: years,
        net_profit: ratio(np_sample, np_pos),
        revenue_yoy: ratio(rev_sample, rev_pos),
        operating_cf: ratio(cf_sample, cf_pos),
        note: "口径：归母净利>0 为赚钱；营收为同比>0（需上年同口径）；经营现金流净额>0。分母均为该期有有效字段的公司数。".into(),
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
        assert_eq!(ratio(100, 80).positive_pct.unwrap(), 80.0);
    }
}
