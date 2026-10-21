//! 利润表 / 现金流量表基本面快照（值、同比、环比）。
//! 表：`sc_stock_income_statement`、`sc_stock_cashflow_statement`（payload 对齐 Miana）。

use std::collections::HashMap;

use chrono::NaiveDate;
use sea_orm::{DatabaseBackend, DatabaseConnection, FromQueryResult, Statement};
use serde_json::Value;

/// 利润表：营收 / 归母 / 扣非。
#[derive(Debug, Clone, Default)]
pub struct IncomeSnapshot {
    #[allow(dead_code)]
    pub report_date: String,
    #[allow(dead_code)]
    pub fiscal_period: Option<String>,
    pub revenue: Option<f64>,
    pub parent_np: Option<f64>,
    pub deduct_np: Option<f64>,
    pub revenue_yoy: Option<f64>,
    pub parent_np_yoy: Option<f64>,
    pub deduct_np_yoy: Option<f64>,
    pub revenue_qoq: Option<f64>,
    pub parent_np_qoq: Option<f64>,
    pub deduct_np_qoq: Option<f64>,
}

/// 现金流量表：经营 / 投资 / 筹资净额。
#[derive(Debug, Clone, Default)]
pub struct CashflowSnapshot {
    #[allow(dead_code)]
    pub report_date: String,
    #[allow(dead_code)]
    pub fiscal_period: Option<String>,
    pub operating: Option<f64>,
    pub investing: Option<f64>,
    pub financing: Option<f64>,
    pub operating_yoy: Option<f64>,
    pub investing_yoy: Option<f64>,
    pub financing_yoy: Option<f64>,
    pub operating_qoq: Option<f64>,
    pub investing_qoq: Option<f64>,
    pub financing_qoq: Option<f64>,
}

#[derive(Debug, FromQueryResult)]
struct FundRow {
    symbol: String,
    report_date: NaiveDate,
    fiscal_year: Option<i32>,
    fiscal_period: Option<String>,
    payload: Value,
}

struct PeriodRef {
    report_date: NaiveDate,
    fiscal_year: Option<i32>,
    fiscal_period: Option<String>,
    a: Option<f64>,
    b: Option<f64>,
    c: Option<f64>,
}

/// 每只股票取最近若干报告期，计算最新一期的值 / 同比 / 环比。
pub async fn load_income_snapshots(
    db: &DatabaseConnection,
) -> Result<HashMap<String, IncomeSnapshot>, sea_orm::DbErr> {
    let by_symbol = load_period_map(db, "sc_stock_income_statement", extract_income).await?;
    let mut out = HashMap::with_capacity(by_symbol.len());
    for (symbol, periods) in by_symbol {
        let Some(latest) = periods.first() else {
            continue;
        };
        let yoy = find_yoy(latest, &periods);
        let qoq = periods.get(1);
        out.insert(
            symbol,
            IncomeSnapshot {
                report_date: latest.report_date.format("%Y-%m-%d").to_string(),
                fiscal_period: latest.fiscal_period.clone(),
                revenue: latest.a,
                parent_np: latest.b,
                deduct_np: latest.c,
                revenue_yoy: pct_change(latest.a, yoy.and_then(|p| p.a)),
                parent_np_yoy: pct_change(latest.b, yoy.and_then(|p| p.b)),
                deduct_np_yoy: pct_change(latest.c, yoy.and_then(|p| p.c)),
                revenue_qoq: pct_change(latest.a, qoq.and_then(|p| p.a)),
                parent_np_qoq: pct_change(latest.b, qoq.and_then(|p| p.b)),
                deduct_np_qoq: pct_change(latest.c, qoq.and_then(|p| p.c)),
            },
        );
    }
    Ok(out)
}

pub async fn load_cashflow_snapshots(
    db: &DatabaseConnection,
) -> Result<HashMap<String, CashflowSnapshot>, sea_orm::DbErr> {
    let by_symbol = load_period_map(db, "sc_stock_cashflow_statement", extract_cashflow).await?;
    let mut out = HashMap::with_capacity(by_symbol.len());
    for (symbol, periods) in by_symbol {
        let Some(latest) = periods.first() else {
            continue;
        };
        let yoy = find_yoy(latest, &periods);
        let qoq = periods.get(1);
        out.insert(
            symbol,
            CashflowSnapshot {
                report_date: latest.report_date.format("%Y-%m-%d").to_string(),
                fiscal_period: latest.fiscal_period.clone(),
                operating: latest.a,
                investing: latest.b,
                financing: latest.c,
                operating_yoy: pct_change(latest.a, yoy.and_then(|p| p.a)),
                investing_yoy: pct_change(latest.b, yoy.and_then(|p| p.b)),
                financing_yoy: pct_change(latest.c, yoy.and_then(|p| p.c)),
                operating_qoq: pct_change(latest.a, qoq.and_then(|p| p.a)),
                investing_qoq: pct_change(latest.b, qoq.and_then(|p| p.b)),
                financing_qoq: pct_change(latest.c, qoq.and_then(|p| p.c)),
            },
        );
    }
    Ok(out)
}

async fn load_period_map(
    db: &DatabaseConnection,
    table: &str,
    extract: fn(&Value) -> (Option<f64>, Option<f64>, Option<f64>),
) -> Result<HashMap<String, Vec<PeriodRef>>, sea_orm::DbErr> {
    let sql = format!(
        r#"
SELECT symbol, report_date, fiscal_year, fiscal_period, payload
FROM (
    SELECT
        symbol,
        report_date,
        fiscal_year,
        fiscal_period,
        payload,
        ROW_NUMBER() OVER (PARTITION BY symbol ORDER BY report_date DESC) AS rn
    FROM {table}
) t
WHERE rn <= 8
ORDER BY symbol, report_date DESC
"#
    );

    let rows = FundRow::find_by_statement(Statement::from_string(
        DatabaseBackend::Postgres,
        sql,
    ))
    .all(db)
    .await?;

    let mut by_symbol: HashMap<String, Vec<PeriodRef>> = HashMap::new();
    for row in rows {
        let (a, b, c) = extract(&row.payload);
        by_symbol.entry(row.symbol).or_default().push(PeriodRef {
            report_date: row.report_date,
            fiscal_year: row.fiscal_year,
            fiscal_period: row.fiscal_period,
            a,
            b,
            c,
        });
    }
    Ok(by_symbol)
}

fn extract_income(payload: &Value) -> (Option<f64>, Option<f64>, Option<f64>) {
    (
        json_f64(payload, &["operatingRevenue", "operating_revenue"])
            .or_else(|| json_f64(payload, &["totalRevenue", "total_revenue"])),
        json_f64(payload, &["netProfitToParent", "net_profit_to_parent"]),
        json_f64(payload, &["netProfitCorrected", "net_profit_corrected"]),
    )
}

fn extract_cashflow(payload: &Value) -> (Option<f64>, Option<f64>, Option<f64>) {
    (
        json_f64(payload, &["netCashFromOperating", "net_cash_from_operating"]),
        json_f64(payload, &["netCashFromInvesting", "net_cash_from_investing"]),
        json_f64(payload, &["netCashFromFinancing", "net_cash_from_financing"]),
    )
}

fn find_yoy<'a>(latest: &PeriodRef, periods: &'a [PeriodRef]) -> Option<&'a PeriodRef> {
    let period = normalize_period(latest.fiscal_period.as_deref());
    if let Some(year) = latest.fiscal_year {
        if let Some(p) = periods.iter().find(|p| {
            p.report_date != latest.report_date
                && p.fiscal_year == Some(year - 1)
                && normalize_period(p.fiscal_period.as_deref()) == period
        }) {
            return Some(p);
        }
    }
    periods.iter().find(|p| {
        p.report_date != latest.report_date
            && normalize_period(p.fiscal_period.as_deref()) == period
            && (latest.report_date - p.report_date).num_days() >= 300
            && (latest.report_date - p.report_date).num_days() <= 430
    })
}

fn normalize_period(p: Option<&str>) -> String {
    p.unwrap_or("")
        .trim()
        .to_uppercase()
        .replace(' ', "")
}

fn json_f64(payload: &Value, keys: &[&str]) -> Option<f64> {
    let obj = payload.as_object()?;
    for k in keys {
        if let Some(v) = obj.get(*k) {
            if let Some(n) = value_to_f64(v) {
                return Some(n);
            }
        }
    }
    None
}

fn value_to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let s = s.trim().replace(',', "");
            if s.is_empty() || s == "--" || s == "-" || s.eq_ignore_ascii_case("null") {
                return None;
            }
            s.parse::<f64>().ok()
        }
        Value::Null => None,
        _ => None,
    }
}

fn pct_change(curr: Option<f64>, prev: Option<f64>) -> Option<f64> {
    let (c, p) = (curr?, prev?);
    if !c.is_finite() || !p.is_finite() || p.abs() < 1e-6 {
        return None;
    }
    Some((c - p) / p.abs() * 100.0)
}

pub fn expression_needs_income(expr_rules: &[Value]) -> bool {
    expr_rules.iter().any(|r| {
        r.get("type").and_then(|t| t.as_str()).is_some_and(|t| {
            matches!(
                t,
                "income_revenue" | "income_parent_np" | "income_deduct_np" | "fundamental_income"
            )
        })
    })
}

pub fn expression_needs_cashflow(expr_rules: &[Value]) -> bool {
    expr_rules.iter().any(|r| {
        r.get("type").and_then(|t| t.as_str()).is_some_and(|t| {
            matches!(t, "cf_operating" | "cf_investing" | "cf_financing")
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_income_and_cashflow() {
        let income = json!({
            "operatingRevenue": "1000000000.00",
            "netProfitToParent": "100000000",
            "netProfitCorrected": "80000000"
        });
        let (rev, _, _) = extract_income(&income);
        assert!((rev.unwrap() - 1e9).abs() < 1.0);

        let cf = json!({
            "netCashFromOperating": "500000000",
            "netCashFromInvesting": "-200000000",
            "netCashFromFinancing": "-100000000"
        });
        let (op, inv, fin) = extract_cashflow(&cf);
        assert!((op.unwrap() - 5e8).abs() < 1.0);
        assert!((inv.unwrap() + 2e8).abs() < 1.0);
        assert!((fin.unwrap() + 1e8).abs() < 1.0);
        assert!((pct_change(Some(110.0), Some(100.0)).unwrap() - 10.0).abs() < 1e-9);
    }
}
