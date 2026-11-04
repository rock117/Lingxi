//! 助手工具白名单：进程内复用 market service，不经 HTTP 绕圈。

use anyhow::{anyhow, Context, Result};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{json, Value};

use super::client::{AiClient, ToolFnSpec, ToolSpec};
use crate::market::calculator;
use crate::market::data_source;
use crate::market::domain::MarketDataInput;
use crate::market::eastmoney::EastmoneyClient;
use crate::market::fund_stats as fund_stats_svc;
use crate::market::fundamentals;
use crate::market::intraday_push;
use crate::market::mid_term;
use crate::market::miana::MianaClient;
use crate::market::realtime_cache::RealtimeDayCache;
use crate::market::screen;
use crate::market::sector as sector_svc;
use crate::market::session;
use crate::market::stock_analyze as stock_analyze_svc;
use crate::market::tencent::TencentClient;
use entity::notification;

pub const TOOL_RESULT_MAX_CHARS: usize = 4000;
const MTT_LOOKBACK_DAYS: usize = 70;

pub struct ToolContext<'a> {
    pub db: &'a DatabaseConnection,
    pub miana: &'a MianaClient,
    pub cache: &'a RealtimeDayCache,
    pub eastmoney: &'a EastmoneyClient,
    pub tencent: &'a TencentClient,
    pub ai: &'a AiClient,
}

pub fn tool_specs() -> Vec<ToolSpec> {
    vec![
        tool(
            "market_sentiment",
            "查询 A 股全市场短线情绪指数 MSI（今日/近几日、涨跌分布、涨跌停等）",
            json!({
                "type": "object",
                "properties": {
                    "days": { "type": "integer", "minimum": 3, "maximum": 30, "description": "回看交易日，默认 5" }
                }
            }),
        ),
        tool(
            "mid_term_trend",
            "查询中期趋势 MTT（多头/空头/中性家数与状态）",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "intraday_alerts",
            "查询日内上涨趋势 Top 与超跌反弹 Top（含暴跌放量日）",
            json!({
                "type": "object",
                "properties": {
                    "kind": {
                        "type": "string",
                        "enum": ["trend", "oversold", "both"],
                        "description": "默认 both"
                    }
                }
            }),
        ),
        tool(
            "fund_stats",
            "全市场财报统计：赚钱面、利润/营收增长面、经营现金流，及同比环比",
            json!({
                "type": "object",
                "properties": {
                    "year": { "type": "integer" },
                    "period": { "type": "string", "description": "Q1|H1|Q3|FY" }
                }
            }),
        ),
        tool(
            "stock_quote",
            "按代码查个股近 10 日行情与涨跌摘要",
            json!({
                "type": "object",
                "properties": {
                    "symbol": { "type": "string", "description": "如 600519.SH" }
                },
                "required": ["symbol"]
            }),
        ),
        tool(
            "stock_fundamentals",
            "个股基本面与财报事实摘要（不含分维 AI 长文）",
            json!({
                "type": "object",
                "properties": {
                    "symbol": { "type": "string" }
                },
                "required": ["symbol"]
            }),
        ),
        tool(
            "stock_search",
            "按代码/名称/拼音搜索股票",
            json!({
                "type": "object",
                "properties": {
                    "q": { "type": "string" }
                },
                "required": ["q"]
            }),
        ),
        tool(
            "screen_preview",
            "按选股 expression JSON 立即筛选（不落库）",
            json!({
                "type": "object",
                "properties": {
                    "expression": { "type": "object" },
                    "top_n": { "type": "integer", "minimum": 1, "maximum": 50 }
                },
                "required": ["expression"]
            }),
        ),
        tool(
            "draft_screen",
            "把自然语言选股意图转成可执行 expression 草稿（需再 screen_preview 才出名单）",
            json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string" }
                },
                "required": ["prompt"]
            }),
        ),
        tool(
            "sector_movers",
            "板块涨跌幅领涨/领跌",
            json!({
                "type": "object",
                "properties": {
                    "top_n": { "type": "integer" },
                    "types": { "type": "string", "description": "如 INDUSTRY,CONCEPT" }
                }
            }),
        ),
        tool(
            "sector_detail",
            "板块详情（含 5/10 日与成分股）",
            json!({
                "type": "object",
                "properties": {
                    "symbol": { "type": "string", "description": "如 BK0145.EM" }
                },
                "required": ["symbol"]
            }),
        ),
        tool(
            "notification_recent",
            "最近推送通知摘要（趋势/超跌等）",
            json!({
                "type": "object",
                "properties": {
                    "unread_only": { "type": "boolean" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 20 }
                }
            }),
        ),
    ]
}

fn tool(name: &'static str, description: &'static str, parameters: Value) -> ToolSpec {
    ToolSpec {
        tool_type: "function",
        function: ToolFnSpec {
            name,
            description,
            parameters,
        },
    }
}

pub fn tool_label(name: &str) -> &'static str {
    match name {
        "market_sentiment" => "市场情绪",
        "mid_term_trend" => "中期结构",
        "intraday_alerts" => "日内榜单",
        "fund_stats" => "财报统计",
        "stock_quote" => "个股行情",
        "stock_fundamentals" => "个股基本面",
        "stock_search" => "搜股票",
        "screen_preview" => "条件选股",
        "draft_screen" => "选股草稿",
        "sector_movers" => "板块强弱",
        "sector_detail" => "板块详情",
        "notification_recent" => "最近通知",
        _ => "数据查询",
    }
}

pub async fn run_tool(ctx: &ToolContext<'_>, name: &str, args: &Value) -> Result<(Value, String)> {
    let value = match name {
        "market_sentiment" => market_sentiment(ctx, args).await?,
        "mid_term_trend" => mid_term_trend(ctx).await?,
        "intraday_alerts" => intraday_alerts(ctx, args).await?,
        "fund_stats" => fund_stats(ctx, args).await?,
        "stock_quote" => stock_quote(ctx, args).await?,
        "stock_fundamentals" => stock_fundamentals(ctx, args).await?,
        "stock_search" => stock_search(ctx, args).await?,
        "screen_preview" => screen_preview(ctx, args).await?,
        "draft_screen" => draft_screen(ctx, args).await?,
        "sector_movers" => sector_movers(ctx, args).await?,
        "sector_detail" => sector_detail(ctx, args).await?,
        "notification_recent" => notification_recent(ctx, args).await?,
        _ => return Err(anyhow!("未知工具: {name}")),
    };
    let summary = summarize_tool(name, &value);
    Ok((truncate_value(value), summary))
}

fn arg_i64(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|v| v.as_i64())
}

fn arg_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn arg_bool(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|v| v.as_bool())
}

async fn ensure_market(
    db: &DatabaseConnection,
    days: usize,
    miana: Option<(&MianaClient, &RealtimeDayCache)>,
) -> Result<MarketDataInput> {
    let input = if let Some((miana, cache)) = miana {
        data_source::load_market_window_hybrid(db, days, miana, cache).await?
    } else {
        data_source::load_market_window(db, days).await?
    };
    if input.snapshots.is_empty() {
        return Err(anyhow!("行情窗口为空"));
    }
    Ok(input)
}

async fn market_sentiment(ctx: &ToolContext<'_>, args: &Value) -> Result<Value> {
    let days = arg_i64(args, "days").unwrap_or(5).clamp(3, 30) as usize;
    let input = ensure_market(ctx.db, days, Some((ctx.miana, ctx.cache))).await?;
    let result = calculator::calculate_msi(&input);
    Ok(serde_json::to_value(result)?)
}

async fn mid_term_trend(ctx: &ToolContext<'_>) -> Result<Value> {
    let input = ensure_market(ctx.db, MTT_LOOKBACK_DAYS, None).await?;
    let result = mid_term::calculate_mtt(&input);
    Ok(serde_json::to_value(result)?)
}

async fn intraday_alerts(ctx: &ToolContext<'_>, args: &Value) -> Result<Value> {
    let kind = arg_str(args, "kind").unwrap_or_else(|| "both".into());
    let bundle = intraday_push::compute_now(ctx.db, ctx.miana, ctx.cache).await?;
    let v = match kind.as_str() {
        "trend" => json!({ "trend": bundle.trend }),
        "oversold" => json!({ "oversold": bundle.oversold }),
        _ => serde_json::to_value(bundle)?,
    };
    Ok(v)
}

async fn fund_stats(ctx: &ToolContext<'_>, args: &Value) -> Result<Value> {
    let year = arg_i64(args, "year").map(|y| y as i32);
    let period = arg_str(args, "period");
    let result =
        fund_stats_svc::compute_stats(ctx.db, year, period.as_deref()).await?;
    Ok(serde_json::to_value(result)?)
}

async fn stock_quote(ctx: &ToolContext<'_>, args: &Value) -> Result<Value> {
    let symbol = arg_str(args, "symbol").ok_or_else(|| anyhow!("缺少 symbol"))?;
    let bars = data_source::load_stock_recent_klines(ctx.db, &symbol, 15)
        .await
        .context("读 K 线失败")?;
    if bars.is_empty() {
        return Err(anyhow!("未找到 {symbol} 行情"));
    }
    let names = data_source::load_stock_names(ctx.db, &[symbol.clone()])
        .await
        .unwrap_or_default();
    let name = names.get(&symbol).cloned().unwrap_or_default();
    let last = bars.last().unwrap();
    let n = bars.len();
    let change_nd = |lookback: usize| -> Option<f64> {
        if n <= lookback {
            return None;
        }
        let old = bars[n - 1 - lookback].close_price;
        let cur = last.close_price;
        if old > 1e-10 {
            Some((cur / old - 1.0) * 100.0)
        } else {
            None
        }
    };
    Ok(json!({
        "symbol": symbol,
        "name": name,
        "date": last.date,
        "close": last.close_price,
        "change_pct": last.change_pct,
        "amount": last.amount,
        "change_5d_pct": change_nd(5),
        "change_10d_pct": change_nd(10),
        "bars": bars.iter().rev().take(10).collect::<Vec<_>>().into_iter().rev().map(|b| json!({
            "date": b.date,
            "open": b.open,
            "high": b.high,
            "low": b.low,
            "close": b.close_price,
            "change_pct": b.change_pct,
            "amount": b.amount,
        })).collect::<Vec<_>>(),
    }))
}

async fn stock_fundamentals(ctx: &ToolContext<'_>, args: &Value) -> Result<Value> {
    let symbol = arg_str(args, "symbol").ok_or_else(|| anyhow!("缺少 symbol"))?;
    let out = stock_analyze_svc::analyze_stock(
        ctx.db,
        ctx.eastmoney,
        ctx.tencent,
        ctx.ai,
        &symbol,
        false,
    )
    .await?;
    // 去掉分维 AI 长文，控体积
    Ok(json!({
        "symbol": out.symbol,
        "name": out.name,
        "trade_date": out.trade_date,
        "quote": out.quote,
        "bars": out.bars,
        "detail_status": out.detail_status,
        "detail_summary": out.detail_summary,
        "short_term": {
            "labels": out.short_term.labels,
            "ma_alignment": out.short_term.ma_alignment,
            "volume_label": out.short_term.volume_label,
            "rsi14": out.short_term.rsi14,
            "rsi_zone": out.short_term.rsi_zone,
        },
        "fundamentals": {
            "quality_flags": out.fundamentals.quality_flags,
            "ocf_vs_np": out.fundamentals.ocf_vs_np,
            "liability_ratio": out.fundamentals.liability_ratio,
            "pe": out.fundamentals.pe,
            "pb": out.fundamentals.pb,
            "revenue_yoy": out.fundamentals.revenue_yoy,
            "parent_np_yoy": out.fundamentals.parent_np_yoy,
        },
        "statements": {
            "trend_flags": out.statements.trend_flags,
            "income_n": out.statements.income.len(),
            "cashflow_n": out.statements.cashflow.len(),
            "balance_n": out.statements.balance.len(),
            "income_latest": out.statements.income.first(),
            "cashflow_latest": out.statements.cashflow.first(),
        }
    }))
}

async fn stock_search(ctx: &ToolContext<'_>, args: &Value) -> Result<Value> {
    let q = arg_str(args, "q").ok_or_else(|| anyhow!("缺少 q"))?;
    let items = data_source::search_stocks(ctx.db, &q, 20).await?;
    Ok(json!({ "total": items.len(), "items": items }))
}

async fn screen_preview(ctx: &ToolContext<'_>, args: &Value) -> Result<Value> {
    let expr_v = args
        .get("expression")
        .cloned()
        .ok_or_else(|| anyhow!("缺少 expression"))?;
    let top_n = arg_i64(args, "top_n").unwrap_or(20).clamp(1, 50) as usize;
    let expr_str = if let Some(s) = expr_v.as_str() {
        s.to_string()
    } else {
        serde_json::to_string(&expr_v)?
    };
    let expr = screen::parse_expression(&expr_str).map_err(|e| anyhow!(e))?;
    let days = screen::required_lookback(&expr).clamp(10, 120);
    let input = ensure_market(ctx.db, days, None).await?;
    let income = if fundamentals::expression_needs_income(&expr.rules) {
        fundamentals::load_income_snapshots(ctx.db).await.ok()
    } else {
        None
    };
    let cashflow = if fundamentals::expression_needs_cashflow(&expr.rules) {
        fundamentals::load_cashflow_snapshots(ctx.db).await.ok()
    } else {
        None
    };
    let balance = if fundamentals::expression_needs_balance(&expr.rules) {
        fundamentals::load_balance_snapshots(ctx.db).await.ok()
    } else {
        None
    };
    let mut result = screen::run_screen_with_fundamentals(
        &input,
        &expr,
        top_n,
        screen::FundMaps {
            income: income.as_ref(),
            cashflow: cashflow.as_ref(),
            balance: balance.as_ref(),
        },
    );
    let symbols: Vec<String> = result.items.iter().map(|i| i.symbol.clone()).collect();
    if let Ok(names) = data_source::load_stock_names(ctx.db, &symbols).await {
        for item in &mut result.items {
            if let Some(n) = names.get(&item.symbol) {
                item.name = n.clone();
            }
        }
    }
    Ok(json!({
        "date": result.date,
        "sample_size": result.sample_size,
        "match_count": result.match_count,
        "items": result.items,
    }))
}

async fn draft_screen(ctx: &ToolContext<'_>, args: &Value) -> Result<Value> {
    let prompt = arg_str(args, "prompt").ok_or_else(|| anyhow!("缺少 prompt"))?;
    let draft = ctx.ai.draft_screen(&prompt).await?;
    Ok(json!({
        "expression": draft.expression,
        "explanation": draft.explanation,
        "warnings": draft.warnings,
    }))
}

async fn sector_movers(ctx: &ToolContext<'_>, args: &Value) -> Result<Value> {
    let top_n = arg_i64(args, "top_n").map(|n| n as usize);
    let types = arg_str(args, "types");
    let out = sector_svc::load_movers(ctx.db, ctx.miana, top_n, types.as_deref()).await?;
    Ok(serde_json::to_value(out)?)
}

async fn sector_detail(ctx: &ToolContext<'_>, args: &Value) -> Result<Value> {
    let symbol = arg_str(args, "symbol").ok_or_else(|| anyhow!("缺少 symbol"))?;
    let out = sector_svc::load_detail(ctx.db, ctx.miana, &symbol)
        .await?
        .ok_or_else(|| anyhow!("未找到板块 {symbol}"))?;
    Ok(serde_json::to_value(out)?)
}

async fn notification_recent(ctx: &ToolContext<'_>, args: &Value) -> Result<Value> {
    let limit = arg_i64(args, "limit").unwrap_or(10).clamp(1, 20) as u64;
    let unread_only = arg_bool(args, "unread_only").unwrap_or(false);
    let mut q = notification::Entity::find();
    if unread_only {
        q = q.filter(notification::Column::Read.eq(false));
    }
    let rows = q
        .order_by_desc(notification::Column::CreatedAt)
        .limit(limit)
        .all(ctx.db)
        .await?;
    let items: Vec<Value> = rows
        .into_iter()
        .map(|m| {
            json!({
                "id": m.id,
                "title": m.title,
                "body": m.body.chars().take(160).collect::<String>(),
                "read": m.read,
                "created_at": m.created_at,
            })
        })
        .collect();
    Ok(json!({ "items": items }))
}

fn truncate_value(v: Value) -> Value {
    let s = serde_json::to_string(&v).unwrap_or_default();
    if s.chars().count() <= TOOL_RESULT_MAX_CHARS {
        return v;
    }
    let cut: String = s.chars().take(TOOL_RESULT_MAX_CHARS).collect();
    json!({
        "truncated": true,
        "preview": cut,
        "note": "结果过长已截断，请基于 preview 作答或缩小查询范围"
    })
}

fn summarize_tool(name: &str, v: &Value) -> String {
    match name {
        "market_sentiment" => {
            let today = v.pointer("/today/value").and_then(|x| x.as_f64());
            let label = v
                .pointer("/today/signal/label")
                .and_then(|x| x.as_str())
                .unwrap_or("-");
            match today {
                Some(t) => format!("今日 MSI {t:.1}（{label}）"),
                None => "已获取情绪数据".into(),
            }
        }
        "mid_term_trend" => {
            let val = v.get("value").and_then(|x| x.as_f64());
            match val {
                Some(t) => format!("MTT {t:.1}"),
                None => "已获取中期趋势".into(),
            }
        }
        "fund_stats" => {
            let pct = v
                .pointer("/net_profit/positive_pct")
                .and_then(|x| x.as_f64());
            let year = v.get("year").and_then(|x| x.as_i64()).unwrap_or(0);
            let period = v
                .get("period_label")
                .and_then(|x| x.as_str())
                .unwrap_or("");
            match pct {
                Some(p) => format!("{year}{period} 赚钱面 {p:.1}%"),
                None => "已获取财报统计".into(),
            }
        }
        "stock_quote" | "stock_fundamentals" => {
            let sym = v.get("symbol").and_then(|x| x.as_str()).unwrap_or("?");
            let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("");
            format!("已获取 {name}{sym}")
        }
        "stock_search" => {
            let n = v.get("total").and_then(|x| x.as_u64()).unwrap_or(0);
            format!("搜到 {n} 只")
        }
        "screen_preview" => {
            let n = v.get("match_count").and_then(|x| x.as_u64()).unwrap_or(0);
            format!("命中 {n} 只")
        }
        "intraday_alerts" => "已获取日内榜单".into(),
        "sector_movers" => "已获取板块涨跌".into(),
        "sector_detail" => "已获取板块详情".into(),
        "notification_recent" => "已获取通知".into(),
        "draft_screen" => "已生成选股草稿".into(),
        _ => session::shanghai_today(),
    }
}
