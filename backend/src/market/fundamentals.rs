//! 利润表 / 现金流量表 / 资产负债表基本面快照（值、同比、环比）。
//! 表：`sc_stock_income_statement`、`sc_stock_cashflow_statement`、`sc_stock_balance_sheet`。

use std::collections::HashMap;

use chrono::NaiveDate;
use sea_orm::{DatabaseBackend, DatabaseConnection, FromQueryResult, Statement};
use serde::Serialize;
use serde_json::Value;

/// 利润表：营收 / 归母 / 扣非 / 研发费用。
#[derive(Debug, Clone, Default)]
pub struct IncomeSnapshot {
    #[allow(dead_code)]
    pub report_date: String,
    #[allow(dead_code)]
    pub fiscal_period: Option<String>,
    pub revenue: Option<f64>,
    pub parent_np: Option<f64>,
    pub deduct_np: Option<f64>,
    pub rd: Option<f64>,
    pub revenue_yoy: Option<f64>,
    pub parent_np_yoy: Option<f64>,
    pub deduct_np_yoy: Option<f64>,
    pub rd_yoy: Option<f64>,
    pub revenue_qoq: Option<f64>,
    pub parent_np_qoq: Option<f64>,
    pub deduct_np_qoq: Option<f64>,
    pub rd_qoq: Option<f64>,
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

/// 资产负债表：应收账款等。
#[derive(Debug, Clone, Default)]
pub struct BalanceSnapshot {
    #[allow(dead_code)]
    pub report_date: String,
    #[allow(dead_code)]
    pub fiscal_period: Option<String>,
    pub receivable: Option<f64>,
    pub receivable_yoy: Option<f64>,
    pub receivable_qoq: Option<f64>,
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
    d: Option<f64>,
}

type ExtractFn = fn(&Value) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>);

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
                rd: latest.d,
                revenue_yoy: pct_change(latest.a, yoy.and_then(|p| p.a)),
                parent_np_yoy: pct_change(latest.b, yoy.and_then(|p| p.b)),
                deduct_np_yoy: pct_change(latest.c, yoy.and_then(|p| p.c)),
                rd_yoy: pct_change(latest.d, yoy.and_then(|p| p.d)),
                revenue_qoq: pct_change(latest.a, qoq.and_then(|p| p.a)),
                parent_np_qoq: pct_change(latest.b, qoq.and_then(|p| p.b)),
                deduct_np_qoq: pct_change(latest.c, qoq.and_then(|p| p.c)),
                rd_qoq: pct_change(latest.d, qoq.and_then(|p| p.d)),
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

pub async fn load_balance_snapshots(
    db: &DatabaseConnection,
) -> Result<HashMap<String, BalanceSnapshot>, sea_orm::DbErr> {
    let by_symbol = load_period_map(db, "sc_stock_balance_sheet", extract_balance).await?;
    let mut out = HashMap::with_capacity(by_symbol.len());
    for (symbol, periods) in by_symbol {
        let Some(latest) = periods.first() else {
            continue;
        };
        let yoy = find_yoy(latest, &periods);
        let qoq = periods.get(1);
        out.insert(
            symbol,
            BalanceSnapshot {
                report_date: latest.report_date.format("%Y-%m-%d").to_string(),
                fiscal_period: latest.fiscal_period.clone(),
                receivable: latest.a,
                receivable_yoy: pct_change(latest.a, yoy.and_then(|p| p.a)),
                receivable_qoq: pct_change(latest.a, qoq.and_then(|p| p.a)),
            },
        );
    }
    Ok(out)
}

async fn load_period_map(
    db: &DatabaseConnection,
    table: &str,
    extract: ExtractFn,
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
        let (a, b, c, d) = extract(&row.payload);
        by_symbol.entry(row.symbol).or_default().push(PeriodRef {
            report_date: row.report_date,
            fiscal_year: row.fiscal_year,
            fiscal_period: row.fiscal_period,
            a,
            b,
            c,
            d,
        });
    }
    Ok(by_symbol)
}

fn extract_income(payload: &Value) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
    (
        json_f64(payload, &["operatingRevenue", "operating_revenue"])
            .or_else(|| json_f64(payload, &["totalRevenue", "total_revenue"])),
        json_f64(payload, &["netProfitToParent", "net_profit_to_parent"]),
        json_f64(payload, &["netProfitCorrected", "net_profit_corrected"]),
        json_f64(
            payload,
            &["researchAndDevelopment", "research_and_development"],
        ),
    )
}

fn extract_cashflow(payload: &Value) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
    (
        json_f64(payload, &["netCashFromOperating", "net_cash_from_operating"]),
        json_f64(payload, &["netCashFromInvesting", "net_cash_from_investing"]),
        json_f64(payload, &["netCashFromFinancing", "net_cash_from_financing"]),
        None,
    )
}

fn extract_balance(payload: &Value) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
    (
        json_f64(payload, &["accountsReceivable", "accounts_receivable"]),
        None,
        None,
        None,
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

/// 单票最近若干期利润表关键指标（报告期降序；同比相对同 fiscal_period 上一期）。
#[derive(Debug, Clone, Serialize)]
pub struct IncomePeriodRow {
    pub report_date: String,
    pub fiscal_period: Option<String>,
    pub revenue: Option<f64>,
    pub revenue_yoy: Option<f64>,
    pub parent_np: Option<f64>,
    pub parent_np_yoy: Option<f64>,
    pub deduct_np: Option<f64>,
    pub deduct_np_yoy: Option<f64>,
    pub rd: Option<f64>,
    pub rd_yoy: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CashflowPeriodRow {
    pub report_date: String,
    pub fiscal_period: Option<String>,
    pub operating: Option<f64>,
    pub operating_yoy: Option<f64>,
    pub investing: Option<f64>,
    pub investing_yoy: Option<f64>,
    pub financing: Option<f64>,
    pub financing_yoy: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BalancePeriodRow {
    pub report_date: String,
    pub fiscal_period: Option<String>,
    pub total_assets: Option<f64>,
    pub total_assets_yoy: Option<f64>,
    pub total_liabilities: Option<f64>,
    pub total_liabilities_yoy: Option<f64>,
    pub total_equity: Option<f64>,
    pub total_equity_yoy: Option<f64>,
    pub receivable: Option<f64>,
    pub receivable_yoy: Option<f64>,
}

/// 单票最新一期基本面快照（含同比）。
#[derive(Debug, Clone, Default, Serialize)]
pub struct SymbolFundSnapshot {
    pub report_date: Option<String>,
    pub fiscal_period: Option<String>,
    pub revenue: Option<f64>,
    pub revenue_yoy: Option<f64>,
    pub parent_np: Option<f64>,
    pub parent_np_yoy: Option<f64>,
    pub deduct_np: Option<f64>,
    pub deduct_np_yoy: Option<f64>,
    pub rd: Option<f64>,
    pub rd_yoy: Option<f64>,
    pub operating_cf: Option<f64>,
    pub operating_cf_yoy: Option<f64>,
    pub receivable: Option<f64>,
    pub receivable_yoy: Option<f64>,
    pub total_assets: Option<f64>,
    pub total_liabilities: Option<f64>,
    pub total_equity: Option<f64>,
}

pub async fn load_symbol_fund_snapshot(
    db: &DatabaseConnection,
    symbol: &str,
) -> Result<SymbolFundSnapshot, sea_orm::DbErr> {
    let income = load_symbol_income_periods(db, symbol, 8).await?;
    let cashflow = load_symbol_cashflow_periods(db, symbol, 8).await?;
    let balance = load_symbol_balance_periods(db, symbol, 8).await?;

    let mut out = SymbolFundSnapshot::default();
    if let Some(cur) = income.first() {
        out.report_date = Some(cur.report_date.clone());
        out.fiscal_period = cur.fiscal_period.clone();
        out.revenue = cur.revenue;
        out.parent_np = cur.parent_np;
        out.deduct_np = cur.deduct_np;
        out.rd = cur.rd;
        if let Some(prev) = income.get(1).filter(|p| {
            // 优先同报告期类型；否则取上一期作粗同比
            normalize_period(p.fiscal_period.as_deref())
                == normalize_period(cur.fiscal_period.as_deref())
                || income.len() >= 2
        }) {
            // 找真正同比：同 fiscal_period
            let yoy = income.iter().find(|p| {
                p.report_date != cur.report_date
                    && normalize_period(p.fiscal_period.as_deref())
                        == normalize_period(cur.fiscal_period.as_deref())
            });
            let y = yoy.unwrap_or(prev);
            out.revenue_yoy = pct_change(cur.revenue, y.revenue);
            out.parent_np_yoy = pct_change(cur.parent_np, y.parent_np);
            out.deduct_np_yoy = pct_change(cur.deduct_np, y.deduct_np);
            out.rd_yoy = pct_change(cur.rd, y.rd);
        }
    }
    if let Some(cur) = cashflow.first() {
        if out.report_date.is_none() {
            out.report_date = Some(cur.report_date.clone());
            out.fiscal_period = cur.fiscal_period.clone();
        }
        out.operating_cf = cur.operating;
        let yoy = cashflow.iter().find(|p| {
            p.report_date != cur.report_date
                && normalize_period(p.fiscal_period.as_deref())
                    == normalize_period(cur.fiscal_period.as_deref())
        });
        if let Some(y) = yoy.or(cashflow.get(1)) {
            out.operating_cf_yoy = pct_change(cur.operating, y.operating);
        }
    }
    if let Some(cur) = balance.first() {
        if out.report_date.is_none() {
            out.report_date = Some(cur.report_date.clone());
            out.fiscal_period = cur.fiscal_period.clone();
        }
        out.receivable = cur.receivable;
        out.total_assets = cur.total_assets;
        out.total_liabilities = cur.total_liabilities;
        out.total_equity = cur.total_equity;
        let yoy = balance.iter().find(|p| {
            p.report_date != cur.report_date
                && normalize_period(p.fiscal_period.as_deref())
                    == normalize_period(cur.fiscal_period.as_deref())
        });
        if let Some(y) = yoy.or(balance.get(1)) {
            out.receivable_yoy = pct_change(cur.receivable, y.receivable);
        }
    }
    Ok(out)
}

pub async fn load_symbol_income_periods(
    db: &DatabaseConnection,
    symbol: &str,
    limit: u32,
) -> Result<Vec<IncomePeriodRow>, sea_orm::DbErr> {
    // 多取几期以便算同比（同 fiscal_period 上年）
    let fetch = limit.saturating_add(4).clamp(1, 16);
    let rows = load_symbol_fund_rows(db, "sc_stock_income_statement", symbol, fetch).await?;
    let mut out: Vec<IncomePeriodRow> = rows
        .into_iter()
        .map(|r| {
            let (rev, np, ded, rd) = extract_income(&r.payload);
            IncomePeriodRow {
                report_date: r.report_date.format("%Y-%m-%d").to_string(),
                fiscal_period: r.fiscal_period,
                revenue: rev,
                revenue_yoy: None,
                parent_np: np,
                parent_np_yoy: None,
                deduct_np: ded,
                deduct_np_yoy: None,
                rd,
                rd_yoy: None,
            }
        })
        .collect();
    fill_income_yoy(&mut out);
    out.truncate(limit.clamp(1, 16) as usize);
    Ok(out)
}

pub async fn load_symbol_cashflow_periods(
    db: &DatabaseConnection,
    symbol: &str,
    limit: u32,
) -> Result<Vec<CashflowPeriodRow>, sea_orm::DbErr> {
    let fetch = limit.saturating_add(4).clamp(1, 16);
    let rows = load_symbol_fund_rows(db, "sc_stock_cashflow_statement", symbol, fetch).await?;
    let mut out: Vec<CashflowPeriodRow> = rows
        .into_iter()
        .map(|r| {
            let (op, inv, fin, _) = extract_cashflow(&r.payload);
            CashflowPeriodRow {
                report_date: r.report_date.format("%Y-%m-%d").to_string(),
                fiscal_period: r.fiscal_period,
                operating: op,
                operating_yoy: None,
                investing: inv,
                investing_yoy: None,
                financing: fin,
                financing_yoy: None,
            }
        })
        .collect();
    fill_cashflow_yoy(&mut out);
    out.truncate(limit.clamp(1, 16) as usize);
    Ok(out)
}

pub async fn load_symbol_balance_periods(
    db: &DatabaseConnection,
    symbol: &str,
    limit: u32,
) -> Result<Vec<BalancePeriodRow>, sea_orm::DbErr> {
    let fetch = limit.saturating_add(4).clamp(1, 16);
    let rows = load_symbol_fund_rows(db, "sc_stock_balance_sheet", symbol, fetch).await?;
    let mut out: Vec<BalancePeriodRow> = rows
        .into_iter()
        .map(|r| {
            let (ar, _, _, _) = extract_balance(&r.payload);
            BalancePeriodRow {
                report_date: r.report_date.format("%Y-%m-%d").to_string(),
                fiscal_period: r.fiscal_period,
                total_assets: json_f64(&r.payload, &["totalAssets", "total_assets"]),
                total_assets_yoy: None,
                total_liabilities: json_f64(&r.payload, &["totalLiabilities", "total_liabilities"]),
                total_liabilities_yoy: None,
                total_equity: json_f64(&r.payload, &["totalEquity", "total_equity"]).or_else(|| {
                    json_f64(
                        &r.payload,
                        &["totalShareholdersEquity", "total_shareholders_equity"],
                    )
                }),
                total_equity_yoy: None,
                receivable: ar,
                receivable_yoy: None,
            }
        })
        .collect();
    fill_balance_yoy(&mut out);
    out.truncate(limit.clamp(1, 16) as usize);
    Ok(out)
}

fn find_prior_same_period<'a, T>(
    rows: &'a [T],
    idx: usize,
    period_of: impl Fn(&T) -> Option<&str>,
) -> Option<&'a T> {
    let cur_p = normalize_period(period_of(&rows[idx]));
    rows.iter().skip(idx + 1).find(|p| normalize_period(period_of(p)) == cur_p)
}

fn fill_income_yoy(rows: &mut [IncomePeriodRow]) {
    for i in 0..rows.len() {
        let prior = find_prior_same_period(rows, i, |r| r.fiscal_period.as_deref())
            .map(|p| (p.revenue, p.parent_np, p.deduct_np, p.rd));
        let Some((pr, pn, pd, prd)) = prior else {
            continue;
        };
        rows[i].revenue_yoy = pct_change(rows[i].revenue, pr);
        rows[i].parent_np_yoy = pct_change(rows[i].parent_np, pn);
        rows[i].deduct_np_yoy = pct_change(rows[i].deduct_np, pd);
        rows[i].rd_yoy = pct_change(rows[i].rd, prd);
    }
}

fn fill_cashflow_yoy(rows: &mut [CashflowPeriodRow]) {
    for i in 0..rows.len() {
        let prior = find_prior_same_period(rows, i, |r| r.fiscal_period.as_deref())
            .map(|p| (p.operating, p.investing, p.financing));
        let Some((po, pi, pf)) = prior else {
            continue;
        };
        rows[i].operating_yoy = pct_change(rows[i].operating, po);
        rows[i].investing_yoy = pct_change(rows[i].investing, pi);
        rows[i].financing_yoy = pct_change(rows[i].financing, pf);
    }
}

fn fill_balance_yoy(rows: &mut [BalancePeriodRow]) {
    for i in 0..rows.len() {
        let prior = find_prior_same_period(rows, i, |r| r.fiscal_period.as_deref()).map(|p| {
            (
                p.total_assets,
                p.total_liabilities,
                p.total_equity,
                p.receivable,
            )
        });
        let Some((pa, pl, pe, pr)) = prior else {
            continue;
        };
        rows[i].total_assets_yoy = pct_change(rows[i].total_assets, pa);
        rows[i].total_liabilities_yoy = pct_change(rows[i].total_liabilities, pl);
        rows[i].total_equity_yoy = pct_change(rows[i].total_equity, pe);
        rows[i].receivable_yoy = pct_change(rows[i].receivable, pr);
    }
}

async fn load_symbol_fund_rows(
    db: &DatabaseConnection,
    table: &str,
    symbol: &str,
    limit: u32,
) -> Result<Vec<FundRow>, sea_orm::DbErr> {
    let limit = limit.clamp(1, 16) as i64;
    let sql = format!(
        r#"
SELECT symbol, report_date, fiscal_year, fiscal_period, payload
FROM {table}
WHERE symbol = $1
ORDER BY report_date DESC
LIMIT $2
"#
    );
    FundRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        sql,
        [symbol.into(), limit.into()],
    ))
    .all(db)
    .await
}

pub fn expression_needs_income(expr_rules: &[Value]) -> bool {
    expr_rules.iter().any(|r| {
        r.get("type").and_then(|t| t.as_str()).is_some_and(|t| {
            matches!(
                t,
                "income_revenue"
                    | "income_parent_np"
                    | "income_deduct_np"
                    | "income_rd"
                    | "fundamental_income"
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

pub fn expression_needs_balance(expr_rules: &[Value]) -> bool {
    expr_rules.iter().any(|r| {
        r.get("type")
            .and_then(|t| t.as_str())
            .is_some_and(|t| t == "bs_receivable")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_income_cashflow_balance() {
        let income = json!({
            "operatingRevenue": "1000000000.00",
            "netProfitToParent": "100000000",
            "netProfitCorrected": "80000000",
            "researchAndDevelopment": "50000000"
        });
        let (rev, _, _, rd) = extract_income(&income);
        assert!((rev.unwrap() - 1e9).abs() < 1.0);
        assert!((rd.unwrap() - 5e7).abs() < 1.0);

        let cf = json!({
            "netCashFromOperating": "500000000",
            "netCashFromInvesting": "-200000000",
            "netCashFromFinancing": "-100000000"
        });
        let (op, inv, fin, _) = extract_cashflow(&cf);
        assert!((op.unwrap() - 5e8).abs() < 1.0);
        assert!((inv.unwrap() + 2e8).abs() < 1.0);
        assert!((fin.unwrap() + 1e8).abs() < 1.0);

        let bs = json!({ "accountsReceivable": "123456789.0" });
        let (ar, _, _, _) = extract_balance(&bs);
        assert!((ar.unwrap() - 123456789.0).abs() < 1.0);
        assert!((pct_change(Some(110.0), Some(100.0)).unwrap() - 10.0).abs() < 1e-9);
    }
}
