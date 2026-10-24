//! 个股 AI 分析：近 5 日 K 线 + 当日（或最近交易日）成交明细。

use chrono::NaiveDate;
use sea_orm::{DatabaseBackend, DatabaseConnection, DbErr, FromQueryResult, Statement};
use serde::Serialize;
use serde_json::{json, Value};
use tracing::{info, warn};

use super::data_source;
use super::eastmoney::{self, EastmoneyClient};
use super::session;
use super::tencent::{self, TencentClient};
use crate::ai::AiClient;

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

/// 当日（或最近开市日）行情摘要，供 App 报价区展示。
#[derive(Debug, Clone, Serialize)]
pub struct StockAnalyzeQuote {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    /// 现价：有成交明细用最新一笔，否则用收盘
    pub current: f64,
    pub pre_close: f64,
    pub change_pct: f64,
    pub change_amount: f64,
    pub amount: f64,
    /// 成交量（股）
    pub volume: f64,
    pub amplitude: f64,
    /// 换手率 %；库内无流通股本时为 null
    pub turnover_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StockAnalyzeOut {
    pub symbol: String,
    pub name: String,
    /// 成交明细所用交易日（非开市日则为最近开市日）
    pub trade_date: String,
    /// `eastmoney` / `tencent` = 实时拉取（不入库）；`db` = 库内快照
    pub detail_source: String,
    /// 明细最新一笔时间，如 `2026-10-24 14:59:57`；无则 null
    pub detail_last_time: Option<String>,
    pub detail_tick_count: i64,
    pub detail_updated_at: Option<String>,
    /// 当日价量摘要；无 K 线时为 null
    pub quote: Option<StockAnalyzeQuote>,
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
    tencent: &TencentClient,
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

    // 1) 东财 → 2) 腾讯（仅当日沪深）→ 3) 库
    let mut live_ok = false;
    if let Some(secid) = eastmoney::symbol_to_secid(symbol) {
        match eastmoney.fetch_trade_details(&secid).await {
            Ok(rows) if !rows.is_empty() => {
                details = rows;
                detail_source = "eastmoney".into();
                live_ok = true;
            }
            Ok(_) => {
                warn!(%symbol, secid, "东财成交明细为空，尝试腾讯");
            }
            Err(e) => {
                warn!(
                    %symbol,
                    secid,
                    error = %e,
                    error_debug = ?e,
                    "东财成交明细拉取失败，尝试腾讯"
                );
            }
        }
    } else {
        warn!(%symbol, "无法映射东财 secid，尝试腾讯");
    }

    if !live_ok {
        if let Some(tc) = tencent::symbol_to_tencent_code(symbol) {
            match tencent.fetch_trade_details_today(&tc).await {
                Ok(rows) if !rows.is_empty() => {
                    details = rows;
                    detail_source = "tencent".into();
                    live_ok = true;
                }
                Ok(_) => {
                    warn!(%symbol, tencent_code = %tc, "腾讯成交明细为空，尝试读库");
                }
                Err(e) => {
                    warn!(
                        %symbol,
                        tencent_code = %tc,
                        error = %e,
                        error_debug = ?e,
                        "腾讯成交明细拉取失败，尝试读库"
                    );
                }
            }
        } else {
            warn!(%symbol, "无法映射腾讯代码（北交所等不支持），尝试读库");
        }
    }

    if live_ok {
        info!(
            %symbol,
            %trade_date_s,
            n = details.len(),
            source = %detail_source,
            "成交明细实时拉取成功（不入库）"
        );
    } else if let Some(row) = load_trade_detail(db, symbol, trade_date).await? {
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

    let day_quote = data_source::load_stock_latest_quote(db, symbol).await?;
    let tx_snap = if let Some(tc) = tencent::symbol_to_tencent_code(symbol) {
        match tencent.fetch_quote_snap(&tc).await {
            Ok(s) => Some(s),
            Err(e) => {
                warn!(%symbol, error = %e, "腾讯实时报价失败，仅用日K/明细");
                None
            }
        }
    } else {
        None
    };

    let quote = day_quote.map(|q| {
        let current = tx_snap
            .as_ref()
            .and_then(|s| s.current)
            .or(summary.last_price.filter(|p| p.is_finite() && *p > 0.0))
            .unwrap_or(q.close);
        let open = tx_snap.as_ref().and_then(|s| s.open).unwrap_or(q.open);
        let pre_close = tx_snap
            .as_ref()
            .and_then(|s| s.pre_close)
            .filter(|p| *p > 1e-10)
            .unwrap_or(q.pre_close);
        let high = tx_snap
            .as_ref()
            .and_then(|s| s.high)
            .or(summary.high)
            .map(|h| h.max(q.high))
            .unwrap_or(q.high);
        let low = tx_snap
            .as_ref()
            .and_then(|s| s.low)
            .or(summary.low)
            .map(|l| l.min(q.low))
            .unwrap_or(q.low);
        let (change_pct, change_amount) = match (
            tx_snap.as_ref().and_then(|s| s.change_pct),
            tx_snap.as_ref().and_then(|s| s.change_amount),
        ) {
            (Some(p), Some(a)) => (p, a),
            _ if pre_close > 1e-10 => (
                (current / pre_close - 1.0) * 100.0,
                current - pre_close,
            ),
            _ => (q.change_pct, q.change_amount),
        };
        let amount = tx_snap
            .as_ref()
            .and_then(|s| s.amount)
            .filter(|a| *a > 0.0)
            .unwrap_or(q.amount);
        // 日K volume 为股；腾讯为手 → 股
        let volume = tx_snap
            .as_ref()
            .and_then(|s| s.volume_hand)
            .map(|h| h * 100.0)
            .filter(|v| *v > 0.0)
            .unwrap_or(q.volume);
        let amplitude = tx_snap
            .as_ref()
            .and_then(|s| s.amplitude)
            .unwrap_or_else(|| {
                if pre_close > 1e-10 {
                    (high - low) / pre_close * 100.0
                } else {
                    q.amplitude
                }
            });
        let turnover_rate = tx_snap.as_ref().and_then(|s| s.turnover_rate);
        StockAnalyzeQuote {
            date: q.date,
            open,
            high,
            low,
            close: q.close,
            current,
            pre_close,
            change_pct,
            change_amount,
            amount,
            volume,
            amplitude,
            turnover_rate,
        }
    });

    let payload = json!({
        "symbol": symbol,
        "name": name,
        "trade_date": used_trade_date.format("%Y-%m-%d").to_string(),
        "detail_source": detail_source,
        "quote": quote,
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
        quote,
        bars,
        analysis,
        ai_enabled,
        ai_model,
    })
}
