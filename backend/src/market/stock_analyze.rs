//! 个股 AI 分析：近 5 日 K 线 + 当日（或最近交易日）成交明细。

use chrono::NaiveDate;
use sea_orm::{DatabaseBackend, DatabaseConnection, DbErr, FromQueryResult, Statement};
use serde::Serialize;
use serde_json::{json, Value};
use tracing::{info, warn};

use super::data_source;
use super::eastmoney::{self, EastmoneyClient};
use super::fundamentals::{self, BalancePeriodRow, CashflowPeriodRow, IncomePeriodRow};
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
    pub analysis: String,
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
    pub analysis: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatementsDim {
    pub income: Vec<IncomePeriodRow>,
    pub cashflow: Vec<CashflowPeriodRow>,
    pub balance: Vec<BalancePeriodRow>,
    pub analysis: String,
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
    /// 近 5 日 K（盘面 Tab）
    pub bars: Vec<StockAnalyzeBar>,
    /// 盘面 AI；兼容旧字段
    pub analysis: String,
    pub short_term: ShortTermDim,
    pub fundamentals: FundamentalsDim,
    pub statements: StatementsDim,
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

    let short_term = build_short_term(&all_bars);
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

    let mut fundamentals_dim = FundamentalsDim {
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
        analysis: String::new(),
    };

    let mut statements_dim = StatementsDim {
        income: income_periods,
        cashflow: cashflow_periods,
        balance: balance_periods,
        analysis: String::new(),
    };

    let meta = json!({
        "symbol": symbol,
        "name": name,
        "trade_date": used_trade_date.format("%Y-%m-%d").to_string(),
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
        "short_term": {
            "change_5d_pct": short_term.change_5d_pct,
            "change_10d_pct": short_term.change_10d_pct,
            "change_20d_pct": short_term.change_20d_pct,
            "ma5": short_term.ma5,
            "ma10": short_term.ma10,
            "ma20": short_term.ma20,
            "amount_vs_5d_avg": short_term.amount_vs_5d_avg,
            "bars": &short_term.bars,
        },
    });
    let fund_payload = json!({
        "meta": meta,
        "quote": quote,
        "fundamentals": {
            "report_date": &fundamentals_dim.report_date,
            "fiscal_period": &fundamentals_dim.fiscal_period,
            "pe": fundamentals_dim.pe,
            "pb": fundamentals_dim.pb,
            "float_mv": fundamentals_dim.float_mv,
            "total_mv": fundamentals_dim.total_mv,
            "revenue": fundamentals_dim.revenue,
            "revenue_yoy": fundamentals_dim.revenue_yoy,
            "parent_np": fundamentals_dim.parent_np,
            "parent_np_yoy": fundamentals_dim.parent_np_yoy,
            "deduct_np": fundamentals_dim.deduct_np,
            "deduct_np_yoy": fundamentals_dim.deduct_np_yoy,
            "operating_cf": fundamentals_dim.operating_cf,
            "operating_cf_yoy": fundamentals_dim.operating_cf_yoy,
            "receivable": fundamentals_dim.receivable,
            "total_assets": fundamentals_dim.total_assets,
            "total_liabilities": fundamentals_dim.total_liabilities,
            "total_equity": fundamentals_dim.total_equity,
        },
    });
    let stmt_payload = json!({
        "meta": meta,
        "statements": {
            "income": &statements_dim.income,
            "cashflow": &statements_dim.cashflow,
            "balance": &statements_dim.balance,
        },
    });

    let (mut analysis, mut short_analysis, mut fund_analysis, mut stmt_analysis, ai_model, ai_enabled) =
        if !ai.enabled() {
            let msg = "AI 未启用：请配置 AI_API_KEY。已返回各维事实数据。".to_string();
            (msg.clone(), msg.clone(), msg.clone(), msg, None, false)
        } else {
            // 四维各自提示词 + 并行请求，侧重点分离且总耗时接近单次上限
            let tape_sys = crate::ai::prompts::stock_analyze_tape_system();
            let short_sys = crate::ai::prompts::stock_analyze_short_term_system();
            let fund_sys = crate::ai::prompts::stock_analyze_fundamentals_system();
            let stmt_sys = crate::ai::prompts::stock_analyze_statements_system();
            let tape_user = format!(
                "请解读盘面：\n{}",
                serde_json::to_string_pretty(&tape_payload).unwrap_or_default()
            );
            let short_user = format!(
                "请解读短线走势：\n{}",
                serde_json::to_string_pretty(&short_payload).unwrap_or_default()
            );
            let fund_user = format!(
                "请解读基本面：\n{}",
                serde_json::to_string_pretty(&fund_payload).unwrap_or_default()
            );
            let stmt_user = format!(
                "请解读财务报表：\n{}",
                serde_json::to_string_pretty(&stmt_payload).unwrap_or_default()
            );
            let (tape_r, short_r, fund_r, stmt_r) = tokio::join!(
                ai.chat(&tape_sys, &tape_user),
                ai.chat(&short_sys, &short_user),
                ai.chat(&fund_sys, &fund_user),
                ai.chat(&stmt_sys, &stmt_user),
            );
            let map_err = |dim: &str, e: anyhow::Error| {
                warn!(error = %e, %dim, "个股 AI 分维分析失败");
                format!("AI 分析失败：{e}。下方仍可查看事实数据。")
            };
            (
                tape_r.unwrap_or_else(|e| map_err("tape", e)),
                short_r.unwrap_or_else(|e| map_err("short_term", e)),
                fund_r.unwrap_or_else(|e| map_err("fundamentals", e)),
                stmt_r.unwrap_or_else(|e| map_err("statements", e)),
                Some(ai.model_name().to_string()),
                true,
            )
        };

    if analysis.trim().is_empty() {
        analysis = "暂无盘面解读。".into();
    }
    if short_analysis.trim().is_empty() {
        short_analysis = "暂无短线解读。".into();
    }
    if fund_analysis.trim().is_empty() {
        fund_analysis = "暂无基本面解读。".into();
    }
    if stmt_analysis.trim().is_empty() {
        stmt_analysis = "暂无财报解读。".into();
    }

    let mut short_term = short_term;
    short_term.analysis = short_analysis;
    fundamentals_dim.analysis = fund_analysis;
    statements_dim.analysis = stmt_analysis;

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
        short_term,
        fundamentals: fundamentals_dim,
        statements: statements_dim,
        ai_enabled,
        ai_model,
    })
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

fn build_short_term(all_bars: &[StockAnalyzeBar]) -> ShortTermDim {
    let show: Vec<StockAnalyzeBar> = all_bars.iter().rev().take(20).cloned().collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let last_amt = all_bars.last().map(|b| b.amount);
    let avg5 = if all_bars.len() >= 5 {
        let s: f64 = all_bars[all_bars.len() - 5..].iter().map(|b| b.amount).sum();
        Some(s / 5.0)
    } else {
        None
    };
    let amount_vs_5d_avg = match (last_amt, avg5) {
        (Some(a), Some(avg)) if avg > 1e-6 => Some(a / avg),
        _ => None,
    };
    ShortTermDim {
        bars: show,
        change_5d_pct: change_nd(all_bars, 5),
        change_10d_pct: change_nd(all_bars, 10),
        change_20d_pct: change_nd(all_bars, 20),
        ma5: sma_close(all_bars, 5),
        ma10: sma_close(all_bars, 10),
        ma20: sma_close(all_bars, 20),
        amount_vs_5d_avg,
        analysis: String::new(),
    }
}

