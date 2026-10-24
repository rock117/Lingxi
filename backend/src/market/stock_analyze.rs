//! 个股 AI 分析：近 5 日 K 线 + 当日（或最近交易日）成交明细。

use chrono::{NaiveDate, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, FromQueryResult, Statement,
};
use serde::Serialize;
use serde_json::{json, Value};
use tracing::{info, warn};

use super::data_source;
use super::eastmoney::{self, EastmoneyClient};
use super::session;
use crate::ai::AiClient;

/// 东财单次拉取最近笔数（负数）；`-50000` 尽量覆盖全天 Level-1 明细。
const DETAIL_POS: i32 = -50000;

#[derive(Debug, Clone, Serialize)]
pub struct StockAnalyzeBar {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub change_pct: f64,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockAnalyzeOut {
    pub symbol: String,
    pub name: String,
    /// 成交明细所用交易日（非开市日则为最近开市日）
    pub trade_date: String,
    /// `live` = 东财拉取并已写入库；`db` = 拉取失败后用库内快照
    pub detail_source: String,
    /// 明细最新一笔时间，如 `2026-10-24 14:59:57`；无则 null
    pub detail_last_time: Option<String>,
    pub detail_tick_count: i64,
    pub detail_updated_at: Option<String>,
    pub bars: Vec<StockAnalyzeBar>,
    /// AI 分析正文；AI 未启用或失败时为错误说明
    pub analysis: String,
    pub ai_enabled: bool,
    pub ai_model: Option<String>,
}

#[derive(Debug, FromQueryResult)]
struct DateRow {
    trade_date: NaiveDate,
}

#[derive(Debug, FromQueryResult)]
struct DetailRow {
    trade_date: NaiveDate,
    detail: Value,
    updated_at: String,
}

/// 解析最近开市日（≤ 上海今日）。
pub async fn latest_open_trade_date(db: &DatabaseConnection) -> Result<NaiveDate, DbErr> {
    let as_of = session::shanghai_today_naive();
    let sql = r#"
SELECT trade_date
FROM sc_trade_calendar
WHERE is_open = TRUE AND trade_date <= $1
ORDER BY trade_date DESC
LIMIT 1
"#;
    let row = DateRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        sql,
        [as_of.into()],
    ))
    .one(db)
    .await?;
    Ok(row.map(|r| r.trade_date).unwrap_or(as_of))
}

async fn upsert_trade_detail(
    db: &DatabaseConnection,
    symbol: &str,
    trade_date: NaiveDate,
    details: &[String],
) -> Result<(), DbErr> {
    let detail_json = Value::Array(details.iter().cloned().map(Value::String).collect());
    let detail_str = serde_json::to_string(&detail_json).unwrap_or_else(|_| "[]".into());
    let sql = r#"
INSERT INTO sc_stock_trade_detail (symbol, trade_date, detail, updated_at)
VALUES ($1, $2, $3::jsonb, NOW())
ON CONFLICT (symbol, trade_date) DO UPDATE SET
    detail = EXCLUDED.detail,
    updated_at = NOW()
"#;
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        sql,
        [symbol.into(), trade_date.into(), detail_str.into()],
    ))
    .await?;
    Ok(())
}

async fn load_trade_detail(
    db: &DatabaseConnection,
    symbol: &str,
    trade_date: NaiveDate,
) -> Result<Option<DetailRow>, DbErr> {
    let sql = r#"
SELECT trade_date, detail, updated_at::text AS updated_at
FROM sc_stock_trade_detail
WHERE symbol = $1 AND trade_date = $2
LIMIT 1
"#;
    DetailRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        sql,
        [symbol.into(), trade_date.into()],
    ))
    .one(db)
    .await
}

/// 若指定日无库数据，取该票最近一日明细。
async fn load_latest_trade_detail(
    db: &DatabaseConnection,
    symbol: &str,
) -> Result<Option<DetailRow>, DbErr> {
    let sql = r#"
SELECT trade_date, detail, updated_at::text AS updated_at
FROM sc_stock_trade_detail
WHERE symbol = $1
ORDER BY trade_date DESC
LIMIT 1
"#;
    DetailRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        sql,
        [symbol.into()],
    ))
    .one(db)
    .await
}

fn details_from_json(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// 东财 details 行：`时间,价格,手数,买卖方向(1买/2卖/4中性),第几笔`（见 eastmoney-apis §1.4）。
fn parse_tick(line: &str) -> Option<(String, f64, f64, i32)> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 4 {
        return None;
    }
    let time = parts[0].trim().to_string();
    let price = parts[1].trim().parse::<f64>().ok()?;
    let lots = parts[2].trim().parse::<f64>().ok()?;
    let side = parts[3].trim().parse::<i32>().ok()?;
    Some((time, price, lots, side))
}

#[derive(Debug, Serialize)]
struct DetailSummary {
    tick_count: usize,
    first_time: Option<String>,
    last_time: Option<String>,
    buy_lots: f64,
    sell_lots: f64,
    neutral_lots: f64,
    buy_amount_est: f64,
    sell_amount_est: f64,
    vwap_est: Option<f64>,
    high: Option<f64>,
    low: Option<f64>,
    last_price: Option<f64>,
    large_buy_lots: f64,
    large_sell_lots: f64,
    sample_tail: Vec<String>,
}

fn summarize_details(details: &[String]) -> DetailSummary {
    let mut buy_lots = 0.0;
    let mut sell_lots = 0.0;
    let mut neutral_lots = 0.0;
    let mut buy_amt = 0.0;
    let mut sell_amt = 0.0;
    let mut sum_px_lots = 0.0;
    let mut sum_lots = 0.0;
    let mut high = None::<f64>;
    let mut low = None::<f64>;
    let mut first_time = None::<String>;
    let mut last_time = None::<String>;
    let mut last_price = None::<f64>;
    let mut large_buy = 0.0;
    let mut large_sell = 0.0;
    const LARGE: f64 = 200.0; // 手

    for line in details {
        let Some((time, price, lots, side)) = parse_tick(line) else {
            continue;
        };
        if first_time.is_none() {
            first_time = Some(time.clone());
        }
        last_time = Some(time);
        last_price = Some(price);
        high = Some(high.map_or(price, |h| h.max(price)));
        low = Some(low.map_or(price, |l| l.min(price)));
        let amt = price * lots * 100.0; // 手→股粗算金额
        sum_px_lots += price * lots;
        sum_lots += lots;
        // 1=买 2=卖；4=中性（集合竞价等偶见 0，一并计入中性）
        match side {
            1 => {
                buy_lots += lots;
                buy_amt += amt;
                if lots >= LARGE {
                    large_buy += lots;
                }
            }
            2 => {
                sell_lots += lots;
                sell_amt += amt;
                if lots >= LARGE {
                    large_sell += lots;
                }
            }
            _ => {
                // 4 / 0 / 其它
                neutral_lots += lots;
            }
        }
    }

    let tail: Vec<String> = details.iter().rev().take(20).cloned().collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    DetailSummary {
        tick_count: details.len(),
        first_time,
        last_time,
        buy_lots,
        sell_lots,
        neutral_lots,
        buy_amount_est: buy_amt,
        sell_amount_est: sell_amt,
        vwap_est: if sum_lots > 0.0 {
            Some(sum_px_lots / sum_lots)
        } else {
            None
        },
        high,
        low,
        last_price,
        large_buy_lots: large_buy,
        large_sell_lots: large_sell,
        sample_tail: tail,
    }
}

pub async fn analyze_stock(
    db: &DatabaseConnection,
    eastmoney: &EastmoneyClient,
    ai: &AiClient,
    symbol: &str,
) -> anyhow::Result<StockAnalyzeOut> {
    let symbol = symbol.trim();
    if symbol.is_empty() || symbol.len() > 32 {
        anyhow::bail!("symbol 无效");
    }

    let trade_date = latest_open_trade_date(db).await?;
    let trade_date_s = trade_date.format("%Y-%m-%d").to_string();

    let mut detail_source = "db".to_string();
    let mut details: Vec<String> = Vec::new();
    let mut detail_updated_at: Option<String> = None;
    let mut used_trade_date = trade_date;

    if let Some(secid) = eastmoney::symbol_to_secid(symbol) {
        match eastmoney.fetch_trade_details(&secid, DETAIL_POS).await {
            Ok(rows) if !rows.is_empty() => {
                if let Err(e) = upsert_trade_detail(db, symbol, trade_date, &rows).await {
                    warn!(error = %e, %symbol, "写入 sc_stock_trade_detail 失败");
                } else {
                    info!(%symbol, %trade_date_s, n = rows.len(), "成交明细已写入库");
                }
                details = rows;
                detail_source = "live".into();
                detail_updated_at = Some(Utc::now().to_rfc3339());
            }
            Ok(_) => {
                warn!(%symbol, secid, "东财成交明细为空，尝试读库");
            }
            Err(e) => {
                warn!(
                    %symbol,
                    secid,
                    error = %e,
                    error_debug = ?e,
                    "东财成交明细拉取失败，尝试读库"
                );
            }
        }
    } else {
        warn!(%symbol, "无法映射东财 secid");
    }

    if details.is_empty() {
        if let Some(row) = load_trade_detail(db, symbol, trade_date).await? {
            details = details_from_json(&row.detail);
            used_trade_date = row.trade_date;
            detail_updated_at = Some(row.updated_at);
            detail_source = "db".into();
        } else if let Some(row) = load_latest_trade_detail(db, symbol).await? {
            details = details_from_json(&row.detail);
            used_trade_date = row.trade_date;
            detail_updated_at = Some(row.updated_at);
            detail_source = "db".into();
            warn!(
                %symbol,
                wanted = %trade_date_s,
                got = %row.trade_date,
                "指定交易日无明细，改用库内最近一日"
            );
        }
    }

    let bars_raw = data_source::load_stock_recent_klines(db, symbol, 5).await?;
    let bars: Vec<StockAnalyzeBar> = bars_raw
        .iter()
        .map(|b| StockAnalyzeBar {
            date: b.date.clone(),
            open: b.open,
            high: b.high,
            low: b.low,
            close: b.close_price,
            change_pct: b.change_pct,
            amount: b.amount,
        })
        .collect();

    let names = data_source::load_stock_names(db, &[symbol.to_string()])
        .await
        .unwrap_or_default();
    let name = names.get(symbol).cloned().unwrap_or_default();

    let summary = summarize_details(&details);
    let detail_last_time = summary.last_time.as_ref().map(|t| {
        format!("{} {}", used_trade_date.format("%Y-%m-%d"), t)
    });

    let payload = json!({
        "symbol": symbol,
        "name": name,
        "trade_date": used_trade_date.format("%Y-%m-%d").to_string(),
        "detail_source": detail_source,
        "bars_5d": bars,
        "detail_summary": summary,
    });

    let (analysis, ai_model, ai_enabled) = if !ai.enabled() {
        (
            "AI 未启用：请配置 AI_API_KEY。已返回近5日K线与成交明细摘要。".to_string(),
            None,
            false,
        )
    } else {
        let system = crate::ai::prompts::stock_analyze_system();
        let user = format!(
            "请根据以下 JSON 分析该股短线强弱与成交细节（买卖力量、放量/缩量、尾盘特征等）：\n{}",
            serde_json::to_string_pretty(&payload).unwrap_or_default()
        );
        match ai.chat(&system, &user).await {
            Ok(text) => (text, Some(ai.model_name().to_string()), true),
            Err(e) => {
                warn!(error = %e, "个股 AI 分析失败");
                (
                    format!("AI 分析失败：{e}。下方仍可查看行情与明细摘要。"),
                    None,
                    true,
                )
            }
        }
    };

    Ok(StockAnalyzeOut {
        symbol: symbol.to_string(),
        name,
        trade_date: used_trade_date.format("%Y-%m-%d").to_string(),
        detail_source,
        detail_last_time,
        detail_tick_count: details.len() as i64,
        detail_updated_at,
        bars,
        analysis,
        ai_enabled,
        ai_model,
    })
}
