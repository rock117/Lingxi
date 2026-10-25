//! 个股分析：近 5 日 K + 成交明细事实；AI 按维懒加载（见 `analyze_dim`）。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use sea_orm::{DatabaseBackend, DatabaseConnection, DbErr, FromQueryResult, Statement};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};

use super::data_source;
use super::eastmoney::{self, EastmoneyClient};
use super::fundamentals::{self, BalancePeriodRow, CashflowPeriodRow, IncomePeriodRow};
use super::session;
use super::tencent::{self, TencentClient};
use crate::ai::client::strip_code_fence;
use crate::ai::AiClient;

const FACTS_TTL: Duration = Duration::from_secs(180);
const AI_TTL: Duration = Duration::from_secs(1800);

/// 单维 AI 结构化结论（模型输出 JSON；失败时 status=error）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimAi {
    /// `pending` | `ready` | `disabled` | `error`
    pub status: String,
    pub text: String,
    /// `bullish` | `neutral` | `bearish` | `unknown`
    pub bias: String,
    /// `high` | `medium` | `low` | `unknown`
    pub confidence: String,
    pub flags: Vec<String>,
}

impl DimAi {
    fn pending() -> Self {
        Self {
            status: "pending".into(),
            text: String::new(),
            bias: "unknown".into(),
            confidence: "unknown".into(),
            flags: Vec::new(),
        }
    }

    fn disabled(msg: &str) -> Self {
        Self {
            status: "disabled".into(),
            text: msg.to_string(),
            bias: "unknown".into(),
            confidence: "unknown".into(),
            flags: Vec::new(),
        }
    }

    fn error(msg: String) -> Self {
        Self {
            status: "error".into(),
            text: msg,
            bias: "unknown".into(),
            confidence: "unknown".into(),
            flags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyzeDimKind {
    Tape,
    ShortTerm,
    Fundamentals,
    Statements,
}

impl AnalyzeDimKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "tape" | "盘面" => Some(Self::Tape),
            "short_term" | "short" | "短线" | "短线走势" => Some(Self::ShortTerm),
            "fundamentals" | "fund" | "基本面" => Some(Self::Fundamentals),
            "statements" | "stmt" | "财报" | "财务报表" => Some(Self::Statements),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Tape => "tape",
            Self::ShortTerm => "short_term",
            Self::Fundamentals => "fundamentals",
            Self::Statements => "statements",
        }
    }
}

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
pub struct ShortTermDim {
    pub bars: Vec<StockAnalyzeBar>,
    pub change_5d_pct: Option<f64>,
    pub change_10d_pct: Option<f64>,
    pub change_20d_pct: Option<f64>,
    pub ma5: Option<f64>,
    pub ma10: Option<f64>,
    pub ma20: Option<f64>,
    /// 最新成交额 / 近5日均额
    pub amount_vs_5d_avg: Option<f64>,
    /// 现价相对 MA 的偏离 %
    pub price_vs_ma5_pct: Option<f64>,
    pub price_vs_ma10_pct: Option<f64>,
    pub price_vs_ma20_pct: Option<f64>,
    /// `bull` | `bear` | `mixed` | `unknown`
    pub ma_alignment: String,
    pub near_high_20d: Option<f64>,
    pub near_low_20d: Option<f64>,
    pub dist_to_high_pct: Option<f64>,
    pub dist_to_low_pct: Option<f64>,
    /// `放量` | `缩量` | `平量` | `未知`
    pub volume_label: String,
    pub labels: Vec<String>,
    pub ai: DimAi,
}

#[derive(Debug, Clone, Serialize)]
pub struct FundamentalsDim {
    pub report_date: Option<String>,
    pub fiscal_period: Option<String>,
    pub pe: Option<f64>,
    pub pb: Option<f64>,
    /// 流通/总市值（亿元，腾讯口径）
    pub float_mv: Option<f64>,
    pub total_mv: Option<f64>,
    pub revenue: Option<f64>,
    pub revenue_yoy: Option<f64>,
    pub parent_np: Option<f64>,
    pub parent_np_yoy: Option<f64>,
    pub deduct_np: Option<f64>,
    pub deduct_np_yoy: Option<f64>,
    pub operating_cf: Option<f64>,
    pub operating_cf_yoy: Option<f64>,
    pub receivable: Option<f64>,
    pub receivable_yoy: Option<f64>,
    pub total_assets: Option<f64>,
    pub total_liabilities: Option<f64>,
    pub total_equity: Option<f64>,
    /// 经营现金流 / 归母净利（有利润时）
    pub ocf_vs_np: Option<f64>,
    pub liability_ratio: Option<f64>,
    pub quality_flags: Vec<String>,
    pub ai: DimAi,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatementsDim {
    pub income: Vec<IncomePeriodRow>,
    pub cashflow: Vec<CashflowPeriodRow>,
    pub balance: Vec<BalancePeriodRow>,
    pub trend_flags: Vec<String>,
    pub ai: DimAi,
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
    /// `pending` = 未拉实时明细（等 `/analyze/ticks`）；`ready` = 已有摘要；`empty` = 已拉但无数据
    pub detail_status: String,
    /// 当日价量摘要；无 K 线时为 null
    pub quote: Option<StockAnalyzeQuote>,
    /// 近 5 日 K（盘面 Tab）
    pub bars: Vec<StockAnalyzeBar>,
    /// 盘面成交摘要（含分时段/量价标签，供 UI 与 AI）
    pub detail_summary: DetailSummary,
    /// 盘面 AI 正文（与 `ai.text` 同步，兼容旧客户端）
    pub analysis: String,
    pub ai: DimAi,
    pub short_term: ShortTermDim,
    pub fundamentals: FundamentalsDim,
    pub statements: StatementsDim,
    pub ai_enabled: bool,
    pub ai_model: Option<String>,
}

/// 单维 AI 响应。
#[derive(Debug, Clone, Serialize)]
pub struct StockAnalyzeAiOut {
    pub symbol: String,
    pub trade_date: String,
    pub dim: String,
    pub ai: DimAi,
    pub ai_enabled: bool,
    pub ai_model: Option<String>,
    /// 是否命中进程内缓存
    pub cached: bool,
}

/// 异步补全成交明细（东财/腾讯），并回写事实缓存供盘面 AI。
#[derive(Debug, Clone, Serialize)]
pub struct StockAnalyzeTicksOut {
    pub symbol: String,
    pub trade_date: String,
    pub detail_source: String,
    pub detail_last_time: Option<String>,
    pub detail_tick_count: i64,
    pub detail_updated_at: Option<String>,
    pub detail_status: String,
    pub detail_summary: DetailSummary,
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

#[derive(Debug, Clone, Serialize)]
pub struct DetailSummary {
    pub tick_count: usize,
    pub first_time: Option<String>,
    pub last_time: Option<String>,
    pub buy_lots: f64,
    pub sell_lots: f64,
    pub neutral_lots: f64,
    pub buy_amount_est: f64,
    pub sell_amount_est: f64,
    pub buy_ratio: Option<f64>,
    pub sell_ratio: Option<f64>,
    pub vwap_est: Option<f64>,
    pub price_vs_vwap_pct: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub last_price: Option<f64>,
    pub large_buy_lots: f64,
    pub large_sell_lots: f64,
    pub large_net_lots: f64,
    /// 开盘 09:30–10:00
    pub open_buy_lots: f64,
    pub open_sell_lots: f64,
    /// 尾盘 14:30–15:00
    pub close_buy_lots: f64,
    pub close_sell_lots: f64,
    /// 大买后约 5 笔的均价变化 %（样本过少为 null）
    pub large_buy_follow_pct: Option<f64>,
    pub large_sell_follow_pct: Option<f64>,
    pub labels: Vec<String>,
    pub sample_tail: Vec<String>,
}

fn time_to_hhmm(time: &str) -> Option<u32> {
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() < 2 {
        return None;
    }
    let h: u32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    Some(h * 100 + m)
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
    let mut open_buy = 0.0;
    let mut open_sell = 0.0;
    let mut close_buy = 0.0;
    let mut close_sell = 0.0;
    const LARGE: f64 = 200.0; // 手

    let parsed: Vec<(String, f64, f64, i32)> = details.iter().filter_map(|l| parse_tick(l)).collect();

    for (time, price, lots, side) in &parsed {
        if first_time.is_none() {
            first_time = Some(time.clone());
        }
        last_time = Some(time.clone());
        last_price = Some(*price);
        high = Some(high.map_or(*price, |h| h.max(*price)));
        low = Some(low.map_or(*price, |l| l.min(*price)));
        let amt = price * lots * 100.0;
        sum_px_lots += price * lots;
        sum_lots += lots;
        let bucket = time_to_hhmm(time);
        match *side {
            1 => {
                buy_lots += lots;
                buy_amt += amt;
                if *lots >= LARGE {
                    large_buy += lots;
                }
                if matches!(bucket, Some(t) if (930..1000).contains(&t)) {
                    open_buy += lots;
                }
                if matches!(bucket, Some(t) if (1430..=1500).contains(&t)) {
                    close_buy += lots;
                }
            }
            2 => {
                sell_lots += lots;
                sell_amt += amt;
                if *lots >= LARGE {
                    large_sell += lots;
                }
                if matches!(bucket, Some(t) if (930..1000).contains(&t)) {
                    open_sell += lots;
                }
                if matches!(bucket, Some(t) if (1430..=1500).contains(&t)) {
                    close_sell += lots;
                }
            }
            _ => {
                neutral_lots += lots;
            }
        }
    }

    let (large_buy_follow_pct, large_sell_follow_pct) = large_print_follow(&parsed, LARGE);

    let vwap = if sum_lots > 0.0 {
        Some(sum_px_lots / sum_lots)
    } else {
        None
    };
    let price_vs_vwap_pct = match (last_price, vwap) {
        (Some(p), Some(v)) if v > 1e-10 => Some((p / v - 1.0) * 100.0),
        _ => None,
    };
    let directed = buy_lots + sell_lots;
    let buy_ratio = if directed > 1e-6 {
        Some(buy_lots / directed)
    } else {
        None
    };
    let sell_ratio = if directed > 1e-6 {
        Some(sell_lots / directed)
    } else {
        None
    };

    let mut labels = Vec::new();
    if let Some(br) = buy_ratio {
        if br >= 0.58 {
            labels.push("买盘占优".into());
        } else if br <= 0.42 {
            labels.push("卖盘占优".into());
        } else {
            labels.push("买卖均衡".into());
        }
    }
    if let Some(pv) = price_vs_vwap_pct {
        if pv >= 0.15 {
            labels.push("价在VWAP上".into());
        } else if pv <= -0.15 {
            labels.push("价在VWAP下".into());
        }
    }
    let large_net = large_buy - large_sell;
    if large_buy + large_sell > 1e-6 {
        if large_net > large_sell * 0.25 {
            labels.push("大单净买".into());
        } else if large_net < -large_buy * 0.25 {
            labels.push("大单净卖".into());
        }
    }
    let open_dir = open_buy + open_sell;
    let close_dir = close_buy + close_sell;
    if open_dir > 1e-6 && close_dir > 1e-6 {
        let open_br = open_buy / open_dir;
        let close_br = close_buy / close_dir;
        if open_br >= 0.55 && close_br <= 0.45 {
            labels.push("尾盘转弱".into());
        } else if open_br <= 0.45 && close_br >= 0.55 {
            labels.push("尾盘转强".into());
        }
    }
    if let Some(f) = large_buy_follow_pct {
        if f > 0.05 {
            labels.push("大买后跟涨".into());
        } else if f < -0.05 {
            labels.push("大买后滞涨".into());
        }
    }

    let tail: Vec<String> = details
        .iter()
        .rev()
        .take(20)
        .cloned()
        .collect::<Vec<_>>()
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
        buy_ratio,
        sell_ratio,
        vwap_est: vwap,
        price_vs_vwap_pct,
        high,
        low,
        last_price,
        large_buy_lots: large_buy,
        large_sell_lots: large_sell,
        large_net_lots: large_net,
        open_buy_lots: open_buy,
        open_sell_lots: open_sell,
        close_buy_lots: close_buy,
        close_sell_lots: close_sell,
        large_buy_follow_pct,
        large_sell_follow_pct,
        labels,
        sample_tail: tail,
    }
}

/// 大单后约 5 笔价格相对大单价的变化 % 均值。
fn large_print_follow(parsed: &[(String, f64, f64, i32)], large: f64) -> (Option<f64>, Option<f64>) {
    let mut buy_moves = Vec::new();
    let mut sell_moves = Vec::new();
    for i in 0..parsed.len() {
        let (_t, price, lots, side) = &parsed[i];
        if *lots < large {
            continue;
        }
        let end = (i + 5).min(parsed.len().saturating_sub(1));
        if end <= i || *price <= 1e-10 {
            continue;
        }
        let later = parsed[end].1;
        let move_pct = (later / *price - 1.0) * 100.0;
        match *side {
            1 => buy_moves.push(move_pct),
            2 => sell_moves.push(move_pct),
            _ => {}
        }
    }
    let avg = |v: &[f64]| {
        if v.len() >= 3 {
            Some(v.iter().sum::<f64>() / v.len() as f64)
        } else {
            None
        }
    };
    (avg(&buy_moves), avg(&sell_moves))
}

struct LoadedTicks {
    details: Vec<String>,
    source: String,
    updated_at: Option<String>,
    used_trade_date: NaiveDate,
    /// pending | ready | empty
    status: String,
}

/// `live=false`：只读库（快）；`live=true`：东财→腾讯→库。
async fn load_ticks(
    db: &DatabaseConnection,
    eastmoney: &EastmoneyClient,
    tencent: &TencentClient,
    symbol: &str,
    trade_date: NaiveDate,
    live: bool,
) -> anyhow::Result<LoadedTicks> {
    let trade_date_s = trade_date.format("%Y-%m-%d").to_string();
    let mut details: Vec<String> = Vec::new();
    let mut detail_source = "db".to_string();
    let mut detail_updated_at: Option<String> = None;
    let mut used_trade_date = trade_date;
    let mut live_ok = false;

    if live {
        if let Some(secid) = eastmoney::symbol_to_secid(symbol) {
            match eastmoney.fetch_trade_details(&secid).await {
                Ok(rows) if !rows.is_empty() => {
                    details = rows;
                    detail_source = "eastmoney".into();
                    live_ok = true;
                }
                Ok(_) => warn!(%symbol, secid, "东财成交明细为空，尝试腾讯"),
                Err(e) => warn!(
                    %symbol,
                    secid,
                    error = %e,
                    error_debug = ?e,
                    "东财成交明细拉取失败，尝试腾讯"
                ),
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
                    Ok(_) => warn!(%symbol, tencent_code = %tc, "腾讯成交明细为空，尝试读库"),
                    Err(e) => warn!(
                        %symbol,
                        tencent_code = %tc,
                        error = %e,
                        error_debug = ?e,
                        "腾讯成交明细拉取失败，尝试读库"
                    ),
                }
            } else {
                warn!(%symbol, "无法映射腾讯代码（北交所等不支持），尝试读库");
            }
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
        return Ok(LoadedTicks {
            details,
            source: detail_source,
            updated_at: None,
            used_trade_date,
            status: "ready".into(),
        });
    }

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

    let status = if !details.is_empty() {
        "ready"
    } else if live {
        "empty"
    } else {
        // 主分析默认不拉实时，留给 /analyze/ticks
        "pending"
    }
    .to_string();

    Ok(LoadedTicks {
        details,
        source: detail_source,
        updated_at: detail_updated_at,
        used_trade_date,
        status,
    })
}

/// 个股事实分析。`live_ticks=false`（默认）跳过东财/腾讯翻页，秒级返回；明细另调 `refresh_analyze_ticks`。
pub async fn analyze_stock(
    db: &DatabaseConnection,
    eastmoney: &EastmoneyClient,
    tencent: &TencentClient,
    ai: &AiClient,
    symbol: &str,
    live_ticks: bool,
) -> anyhow::Result<StockAnalyzeOut> {
    let symbol = symbol.trim();
    if symbol.is_empty() || symbol.len() > 32 {
        anyhow::bail!("symbol 无效");
    }

    let trade_date = latest_open_trade_date(db).await?;
    let ticks = load_ticks(db, eastmoney, tencent, symbol, trade_date, live_ticks).await?;
    let details = ticks.details;
    let detail_source = ticks.source;
    let detail_updated_at = ticks.updated_at;
    let used_trade_date = ticks.used_trade_date;
    let detail_status = ticks.status;

    let bars_raw = data_source::load_stock_recent_klines(db, symbol, 30).await?;
    let all_bars: Vec<StockAnalyzeBar> = bars_raw
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
    let bars: Vec<StockAnalyzeBar> = all_bars
        .iter()
        .rev()
        .take(5)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
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

    let last_price = quote.as_ref().map(|q| q.current).or(summary.last_price);
    let mut short_term = build_short_term(&all_bars, last_price);
    let fund_snap = fundamentals::load_symbol_fund_snapshot(db, symbol)
        .await
        .unwrap_or_default();
    let income_periods = fundamentals::load_symbol_income_periods(db, symbol, 6)
        .await
        .unwrap_or_default();
    let cashflow_periods = fundamentals::load_symbol_cashflow_periods(db, symbol, 6)
        .await
        .unwrap_or_default();
    let balance_periods = fundamentals::load_symbol_balance_periods(db, symbol, 6)
        .await
        .unwrap_or_default();

    let (ocf_vs_np, liability_ratio, quality_flags) = fund_quality_metrics(&fund_snap);
    let trend_flags = statement_trend_flags(&income_periods, &cashflow_periods, &balance_periods);

    let ai_enabled = ai.enabled();
    let pending_or_off = if ai_enabled {
        DimAi::pending()
    } else {
        DimAi::disabled("AI 未启用：请配置 AI_API_KEY。已返回各维事实数据。")
    };
    short_term.ai = pending_or_off.clone();

    let fundamentals_dim = FundamentalsDim {
        report_date: fund_snap.report_date.clone(),
        fiscal_period: fund_snap.fiscal_period.clone(),
        pe: tx_snap.as_ref().and_then(|s| s.pe),
        pb: tx_snap.as_ref().and_then(|s| s.pb),
        float_mv: tx_snap.as_ref().and_then(|s| s.float_mv),
        total_mv: tx_snap.as_ref().and_then(|s| s.total_mv),
        revenue: fund_snap.revenue,
        revenue_yoy: fund_snap.revenue_yoy,
        parent_np: fund_snap.parent_np,
        parent_np_yoy: fund_snap.parent_np_yoy,
        deduct_np: fund_snap.deduct_np,
        deduct_np_yoy: fund_snap.deduct_np_yoy,
        operating_cf: fund_snap.operating_cf,
        operating_cf_yoy: fund_snap.operating_cf_yoy,
        receivable: fund_snap.receivable,
        receivable_yoy: fund_snap.receivable_yoy,
        total_assets: fund_snap.total_assets,
        total_liabilities: fund_snap.total_liabilities,
        total_equity: fund_snap.total_equity,
        ocf_vs_np,
        liability_ratio,
        quality_flags,
        ai: pending_or_off.clone(),
    };

    let statements_dim = StatementsDim {
        income: income_periods,
        cashflow: cashflow_periods,
        balance: balance_periods,
        trend_flags,
        ai: pending_or_off.clone(),
    };

    let trade_date_out = used_trade_date.format("%Y-%m-%d").to_string();
    let meta = json!({
        "symbol": symbol,
        "name": name,
        "trade_date": trade_date_out,
    });
    let tape_payload = json!({
        "meta": meta,
        "quote": quote,
        "bars_5d": bars,
        "detail_source": detail_source,
        "detail_summary": summary,
    });
    let short_payload = json!({
        "meta": meta,
        "quote": quote,
        "short_term": &short_term,
    });
    let fund_payload = json!({
        "meta": meta,
        "quote": quote,
        "fundamentals": &fundamentals_dim,
    });
    let stmt_payload = json!({
        "meta": meta,
        "statements": &statements_dim,
    });

    let out = StockAnalyzeOut {
        symbol: symbol.to_string(),
        name,
        trade_date: trade_date_out.clone(),
        detail_source,
        detail_last_time,
        detail_tick_count: details.len() as i64,
        detail_updated_at,
        detail_status,
        quote,
        bars,
        detail_summary: summary,
        analysis: pending_or_off.text.clone(),
        ai: pending_or_off.clone(),
        short_term,
        fundamentals: fundamentals_dim,
        statements: statements_dim,
        ai_enabled,
        ai_model: if ai_enabled {
            Some(ai.model_name().to_string())
        } else {
            None
        },
    };

    cache_put_facts(
        symbol,
        CachedFacts {
            out: out.clone(),
            tape_payload,
            short_payload,
            fund_payload,
            stmt_payload,
        },
    );

    Ok(out)
}

/// 按维调用 AI（复用事实缓存；结果进程内缓存）。`force` 跳过 AI 缓存。
/// 补拉实时成交明细，并合并进事实缓存（供盘面摘要 / tape AI）。
pub async fn refresh_analyze_ticks(
    db: &DatabaseConnection,
    eastmoney: &EastmoneyClient,
    tencent: &TencentClient,
    ai: &AiClient,
    symbol: &str,
) -> anyhow::Result<StockAnalyzeTicksOut> {
    let symbol = symbol.trim();
    if symbol.is_empty() || symbol.len() > 32 {
        anyhow::bail!("symbol 无效");
    }

    // 确保有事实缓存骨架（不含实时明细）
    if cache_get_facts(symbol).is_none() {
        let _ = analyze_stock(db, eastmoney, tencent, ai, symbol, false).await?;
    }

    let trade_date = latest_open_trade_date(db).await?;
    let ticks = load_ticks(db, eastmoney, tencent, symbol, trade_date, true).await?;
    let summary = summarize_details(&ticks.details);
    let detail_last_time = summary.last_time.as_ref().map(|t| {
        format!("{} {}", ticks.used_trade_date.format("%Y-%m-%d"), t)
    });
    let trade_date_out = ticks.used_trade_date.format("%Y-%m-%d").to_string();

    // 回写缓存：更新 out + tape_payload
    if let Some(mut facts) = cache_get_facts(symbol) {
        facts.out.detail_source = ticks.source.clone();
        facts.out.detail_last_time = detail_last_time.clone();
        facts.out.detail_tick_count = ticks.details.len() as i64;
        facts.out.detail_updated_at = ticks.updated_at.clone();
        facts.out.detail_status = ticks.status.clone();
        facts.out.detail_summary = summary.clone();
        facts.out.trade_date = trade_date_out.clone();
        let meta = json!({
            "symbol": facts.out.symbol,
            "name": facts.out.name,
            "trade_date": trade_date_out,
        });
        facts.tape_payload = json!({
            "meta": meta,
            "quote": facts.out.quote,
            "bars_5d": facts.out.bars,
            "detail_source": ticks.source,
            "detail_summary": summary,
        });
        // 明细更新后旧 tape AI 失效
        cache_invalidate_ai(symbol, &trade_date_out, AnalyzeDimKind::Tape.as_str());
        cache_put_facts(symbol, facts);
    }

    Ok(StockAnalyzeTicksOut {
        symbol: symbol.to_string(),
        trade_date: trade_date_out,
        detail_source: ticks.source,
        detail_last_time,
        detail_tick_count: ticks.details.len() as i64,
        detail_updated_at: ticks.updated_at,
        detail_status: ticks.status,
        detail_summary: summary,
    })
}

pub async fn analyze_dim(
    db: &DatabaseConnection,
    eastmoney: &EastmoneyClient,
    tencent: &TencentClient,
    ai: &AiClient,
    symbol: &str,
    dim: AnalyzeDimKind,
    force: bool,
) -> anyhow::Result<StockAnalyzeAiOut> {
    let symbol = symbol.trim();
    if symbol.is_empty() || symbol.len() > 32 {
        anyhow::bail!("symbol 无效");
    }

    if !ai.enabled() {
        return Ok(StockAnalyzeAiOut {
            symbol: symbol.to_string(),
            trade_date: String::new(),
            dim: dim.as_str().into(),
            ai: DimAi::disabled("AI 未启用：请配置 AI_API_KEY。"),
            ai_enabled: false,
            ai_model: None,
            cached: false,
        });
    }

    let mut facts = match cache_get_facts(symbol) {
        Some(f) => f,
        None => {
            let _ = analyze_stock(db, eastmoney, tencent, ai, symbol, false).await?;
            cache_get_facts(symbol).ok_or_else(|| anyhow::anyhow!("事实缓存写入失败"))?
        }
    };

    // 盘面 AI 需要实时明细时先补拉
    if dim == AnalyzeDimKind::Tape && facts.out.detail_status == "pending" {
        let _ = refresh_analyze_ticks(db, eastmoney, tencent, ai, symbol).await?;
        facts = cache_get_facts(symbol).ok_or_else(|| anyhow::anyhow!("明细回写缓存失败"))?;
    }

    let trade_date = facts.out.trade_date.clone();
    if !force {
        if let Some(hit) = cache_get_ai(symbol, &trade_date, dim.as_str()) {
            return Ok(StockAnalyzeAiOut {
                symbol: symbol.to_string(),
                trade_date,
                dim: dim.as_str().into(),
                ai: hit,
                ai_enabled: true,
                ai_model: Some(ai.model_name().to_string()),
                cached: true,
            });
        }
    }

    let (system, user_prefix, payload) = match dim {
        AnalyzeDimKind::Tape => (
            crate::ai::prompts::stock_analyze_tape_system(),
            "请解读盘面，只输出 JSON：",
            &facts.tape_payload,
        ),
        AnalyzeDimKind::ShortTerm => (
            crate::ai::prompts::stock_analyze_short_term_system(),
            "请解读短线走势，只输出 JSON：",
            &facts.short_payload,
        ),
        AnalyzeDimKind::Fundamentals => (
            crate::ai::prompts::stock_analyze_fundamentals_system(),
            "请解读基本面，只输出 JSON：",
            &facts.fund_payload,
        ),
        AnalyzeDimKind::Statements => (
            crate::ai::prompts::stock_analyze_statements_system(),
            "请解读财务报表，只输出 JSON：",
            &facts.stmt_payload,
        ),
    };
    let user = format!(
        "{user_prefix}\n{}",
        serde_json::to_string_pretty(payload).unwrap_or_default()
    );

    let insight = match ai.chat(&system, &user).await {
        Ok(raw) => parse_dim_ai_json(&raw),
        Err(e) => {
            warn!(error = %e, dim = dim.as_str(), "个股 AI 分维失败");
            DimAi::error(format!("AI 分析失败：{e}。下方仍可查看事实数据。"))
        }
    };

    if insight.status == "ready" {
        cache_put_ai(symbol, &trade_date, dim.as_str(), insight.clone());
    }

    Ok(StockAnalyzeAiOut {
        symbol: symbol.to_string(),
        trade_date,
        dim: dim.as_str().into(),
        ai: insight,
        ai_enabled: true,
        ai_model: Some(ai.model_name().to_string()),
        cached: false,
    })
}

struct CachedFacts {
    out: StockAnalyzeOut,
    tape_payload: Value,
    short_payload: Value,
    fund_payload: Value,
    stmt_payload: Value,
}

struct AnalyzeMemCache {
    facts: HashMap<String, (Instant, CachedFacts)>,
    ai: HashMap<String, (Instant, DimAi)>,
}

fn mem_cache() -> &'static Mutex<AnalyzeMemCache> {
    static C: OnceLock<Mutex<AnalyzeMemCache>> = OnceLock::new();
    C.get_or_init(|| {
        Mutex::new(AnalyzeMemCache {
            facts: HashMap::new(),
            ai: HashMap::new(),
        })
    })
}

fn cache_put_facts(symbol: &str, facts: CachedFacts) {
    if let Ok(mut g) = mem_cache().lock() {
        g.facts.insert(symbol.to_string(), (Instant::now(), facts));
    }
}

fn cache_get_facts(symbol: &str) -> Option<CachedFacts> {
    let mut g = mem_cache().lock().ok()?;
    let (t, f) = g.facts.get(symbol)?;
    if t.elapsed() > FACTS_TTL {
        g.facts.remove(symbol);
        return None;
    }
    // Clone payloads + out
    Some(CachedFacts {
        out: f.out.clone(),
        tape_payload: f.tape_payload.clone(),
        short_payload: f.short_payload.clone(),
        fund_payload: f.fund_payload.clone(),
        stmt_payload: f.stmt_payload.clone(),
    })
}

fn ai_cache_key(symbol: &str, trade_date: &str, dim: &str) -> String {
    format!("{symbol}|{trade_date}|{dim}")
}

fn cache_put_ai(symbol: &str, trade_date: &str, dim: &str, ai: DimAi) {
    if let Ok(mut g) = mem_cache().lock() {
        g.ai
            .insert(ai_cache_key(symbol, trade_date, dim), (Instant::now(), ai));
    }
}

fn cache_get_ai(symbol: &str, trade_date: &str, dim: &str) -> Option<DimAi> {
    let mut g = mem_cache().lock().ok()?;
    let key = ai_cache_key(symbol, trade_date, dim);
    let (t, v) = g.ai.get(&key)?;
    if t.elapsed() > AI_TTL {
        g.ai.remove(&key);
        return None;
    }
    Some(v.clone())
}

fn cache_invalidate_ai(symbol: &str, trade_date: &str, dim: &str) {
    if let Ok(mut g) = mem_cache().lock() {
        g.ai.remove(&ai_cache_key(symbol, trade_date, dim));
    }
}

#[derive(Debug, Deserialize)]
struct DimAiModelOut {
    text: Option<String>,
    bias: Option<String>,
    confidence: Option<String>,
    flags: Option<Vec<String>>,
}

fn parse_dim_ai_json(raw: &str) -> DimAi {
    let cleaned = strip_code_fence(raw);
    match serde_json::from_str::<DimAiModelOut>(&cleaned) {
        Ok(m) => {
            let bias = normalize_bias(m.bias.as_deref().unwrap_or(""));
            let confidence = normalize_confidence(m.confidence.as_deref().unwrap_or(""));
            let mut flags = m.flags.unwrap_or_default();
            flags.truncate(4);
            flags.retain(|f| !f.trim().is_empty());
            let text = m.text.unwrap_or_default().trim().to_string();
            if text.is_empty() {
                DimAi::error("模型返回空 text。".into())
            } else {
                DimAi {
                    status: "ready".into(),
                    text,
                    bias,
                    confidence,
                    flags,
                }
            }
        }
        Err(_) => {
            // 兼容偶发纯文本
            let text = cleaned.trim().to_string();
            if text.is_empty() {
                DimAi::error("模型输出无法解析。".into())
            } else {
                DimAi {
                    status: "ready".into(),
                    text,
                    bias: "unknown".into(),
                    confidence: "low".into(),
                    flags: Vec::new(),
                }
            }
        }
    }
}

fn normalize_bias(s: &str) -> String {
    match s.trim().to_ascii_lowercase().as_str() {
        "bullish" | "bull" | "偏多" | "多" => "bullish".into(),
        "bearish" | "bear" | "偏空" | "空" => "bearish".into(),
        "neutral" | "中性" | "平" => "neutral".into(),
        _ => "unknown".into(),
    }
}

fn normalize_confidence(s: &str) -> String {
    match s.trim().to_ascii_lowercase().as_str() {
        "high" | "高" => "high".into(),
        "medium" | "med" | "中" => "medium".into(),
        "low" | "低" => "low".into(),
        _ => "unknown".into(),
    }
}

fn fund_quality_metrics(
    snap: &fundamentals::SymbolFundSnapshot,
) -> (Option<f64>, Option<f64>, Vec<String>) {
    let ocf_vs_np = match (snap.operating_cf, snap.parent_np) {
        (Some(ocf), Some(np)) if np.abs() > 1e-6 => Some(ocf / np),
        _ => None,
    };
    let liability_ratio = match (snap.total_liabilities, snap.total_assets) {
        (Some(l), Some(a)) if a > 1e-6 => Some(l / a),
        _ => None,
    };
    let mut flags = Vec::new();
    if let (Some(ry), Some(ny)) = (snap.revenue_yoy, snap.parent_np_yoy) {
        if ry > 5.0 && ny < -5.0 {
            flags.push("增收不增利".into());
        } else if ry < -5.0 && ny > 5.0 {
            flags.push("利润改善营收仍弱".into());
        }
    }
    if let Some(r) = ocf_vs_np {
        if r < 0.5 && snap.parent_np.unwrap_or(0.0) > 0.0 {
            flags.push("现金流弱于利润".into());
        } else if r > 1.2 {
            flags.push("现金流覆盖利润".into());
        }
    }
    if let (Some(p), Some(d)) = (snap.parent_np, snap.deduct_np) {
        if p.abs() > 1e-6 && (p - d).abs() / p.abs() > 0.35 {
            flags.push("扣非与归母差距大".into());
        }
    }
    if let Some(rr) = snap.receivable_yoy {
        if let Some(ry) = snap.revenue_yoy {
            if rr > ry + 15.0 && rr > 20.0 {
                flags.push("应收增速快于营收".into());
            }
        }
    }
    if let Some(lr) = liability_ratio {
        if lr >= 0.7 {
            flags.push("资产负债率偏高".into());
        }
    }
    (ocf_vs_np, liability_ratio, flags)
}

fn statement_trend_flags(
    income: &[IncomePeriodRow],
    cashflow: &[CashflowPeriodRow],
    balance: &[BalancePeriodRow],
) -> Vec<String> {
    let mut flags = Vec::new();
    if income.len() >= 2 {
        let a = &income[0];
        let b = &income[1];
        match (a.revenue, b.revenue) {
            (Some(x), Some(y)) if y > 1e-6 => {
                if x > y * 1.05 {
                    flags.push("营收环比改善".into());
                } else if x < y * 0.95 {
                    flags.push("营收环比走弱".into());
                }
            }
            _ => {}
        }
        match (a.parent_np, b.parent_np) {
            (Some(x), Some(y)) => {
                if x > y && (y <= 0.0 || x > y * 1.05) {
                    flags.push("归母净利改善".into());
                } else if x < y && (x <= 0.0 || x < y * 0.95) {
                    flags.push("归母净利走弱".into());
                }
            }
            _ => {}
        }
    }
    if !income.is_empty() && !cashflow.is_empty() {
        if let (Some(np), Some(ocf)) = (income[0].parent_np, cashflow[0].operating) {
            if np > 0.0 && ocf < np * 0.5 {
                flags.push("近一期利润现金含量低".into());
            }
        }
    }
    if balance.len() >= 2 {
        if let (Some(r0), Some(r1), Some(rev0), Some(rev1)) = (
            balance[0].receivable,
            balance[1].receivable,
            income.first().and_then(|i| i.revenue),
            income.get(1).and_then(|i| i.revenue),
        ) {
            if r1 > 1e-6 && rev1 > 1e-6 {
                let r_g = r0 / r1 - 1.0;
                let v_g = rev0 / rev1 - 1.0;
                if r_g > v_g + 0.1 && r_g > 0.1 {
                    flags.push("应收扩张快于营收".into());
                }
            }
        }
    }
    flags.truncate(6);
    flags
}

fn change_nd(bars: &[StockAnalyzeBar], lookback: usize) -> Option<f64> {
    let n = bars.len();
    if n <= lookback {
        return None;
    }
    let old = bars[n - 1 - lookback].close;
    let cur = bars[n - 1].close;
    if old > 1e-10 {
        Some((cur / old - 1.0) * 100.0)
    } else {
        None
    }
}

fn sma_close(bars: &[StockAnalyzeBar], window: usize) -> Option<f64> {
    if bars.len() < window || window == 0 {
        return None;
    }
    let slice = &bars[bars.len() - window..];
    let sum: f64 = slice.iter().map(|b| b.close).sum();
    Some(sum / window as f64)
}

fn pct_vs(price: f64, ma: Option<f64>) -> Option<f64> {
    ma.filter(|m| *m > 1e-10).map(|m| (price / m - 1.0) * 100.0)
}

fn build_short_term(all_bars: &[StockAnalyzeBar], last_price: Option<f64>) -> ShortTermDim {
    let show: Vec<StockAnalyzeBar> = all_bars
        .iter()
        .rev()
        .take(20)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let last_amt = all_bars.last().map(|b| b.amount);
    let avg5 = if all_bars.len() >= 5 {
        let s: f64 = all_bars[all_bars.len() - 5..]
            .iter()
            .map(|b| b.amount)
            .sum();
        Some(s / 5.0)
    } else {
        None
    };
    let amount_vs_5d_avg = match (last_amt, avg5) {
        (Some(a), Some(avg)) if avg > 1e-6 => Some(a / avg),
        _ => None,
    };
    let ma5 = sma_close(all_bars, 5);
    let ma10 = sma_close(all_bars, 10);
    let ma20 = sma_close(all_bars, 20);
    let price = last_price
        .or_else(|| all_bars.last().map(|b| b.close))
        .unwrap_or(0.0);
    let price_vs_ma5_pct = pct_vs(price, ma5);
    let price_vs_ma10_pct = pct_vs(price, ma10);
    let price_vs_ma20_pct = pct_vs(price, ma20);

    let ma_alignment = match (ma5, ma10, ma20) {
        (Some(a), Some(b), Some(c)) if a >= b && b >= c => "bull",
        (Some(a), Some(b), Some(c)) if a <= b && b <= c => "bear",
        (Some(_), Some(_), Some(_)) => "mixed",
        _ => "unknown",
    }
    .to_string();

    let window = if show.len() >= 2 { &show[..] } else { &[][..] };
    let near_high_20d = window.iter().map(|b| b.high).fold(None, |acc: Option<f64>, h| {
        Some(acc.map_or(h, |a| a.max(h)))
    });
    let near_low_20d = window.iter().map(|b| b.low).fold(None, |acc: Option<f64>, l| {
        Some(acc.map_or(l, |a| a.min(l)))
    });
    let dist_to_high_pct = match (near_high_20d, price > 1e-10) {
        (Some(h), true) if h > 1e-10 => Some((price / h - 1.0) * 100.0),
        _ => None,
    };
    let dist_to_low_pct = match (near_low_20d, price > 1e-10) {
        (Some(l), true) if l > 1e-10 => Some((price / l - 1.0) * 100.0),
        _ => None,
    };

    let volume_label = match amount_vs_5d_avg {
        Some(x) if x >= 1.3 => "放量",
        Some(x) if x <= 0.7 => "缩量",
        Some(_) => "平量",
        None => "未知",
    }
    .to_string();

    let mut labels = Vec::new();
    match ma_alignment.as_str() {
        "bull" => labels.push("均线多头".into()),
        "bear" => labels.push("均线空头".into()),
        "mixed" => labels.push("均线纠缠".into()),
        _ => {}
    }
    if let Some(p) = price_vs_ma5_pct {
        if p.abs() <= 1.0 {
            labels.push("贴近MA5".into());
        } else if p > 3.0 {
            labels.push("站上MA5".into());
        } else if p < -3.0 {
            labels.push("跌破MA5".into());
        }
    }
    if volume_label != "未知" {
        labels.push(volume_label.clone());
    }
    if let Some(d) = dist_to_high_pct {
        if d >= -2.0 {
            labels.push("靠近近高".into());
        }
    }
    if let Some(d) = dist_to_low_pct {
        if d <= 2.0 {
            labels.push("靠近近低".into());
        }
    }

    ShortTermDim {
        bars: show,
        change_5d_pct: change_nd(all_bars, 5),
        change_10d_pct: change_nd(all_bars, 10),
        change_20d_pct: change_nd(all_bars, 20),
        ma5,
        ma10,
        ma20,
        amount_vs_5d_avg,
        price_vs_ma5_pct,
        price_vs_ma10_pct,
        price_vs_ma20_pct,
        ma_alignment,
        near_high_20d,
        near_low_20d,
        dist_to_high_pct,
        dist_to_low_pct,
        volume_label,
        labels,
        ai: DimAi::pending(),
    }
}

