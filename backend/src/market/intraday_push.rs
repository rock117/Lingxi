//! 开市日竞价窗内每 5 分钟计算「上涨趋势 / 超跌反弹」两类 Top10，分别推送并落库；
//! 结果写入 `market_result_cache`；非交易时段不推送。

use chrono::{Timelike, Utc};
use entity::{condition, notification};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set};
use tokio::time::{Duration, Instant, MissedTickBehavior};
use tracing::{info, warn};

use super::data_source;
use super::intraday_alert::{
    calculate_alert_bundle, AlertItem, IntradayAlertBundle, IntradayAlertResult,
};
use super::miana::MianaClient;
use super::realtime_cache::RealtimeDayCache;
use super::result_cache;
use super::session;
use crate::ws::hub::{Hub, PushMessage};

const INTERVAL_SECS: u64 = 300;

const KIND_TREND: &str = "intraday_alert";
const NAME_TREND: &str = "上涨趋势股";
const KIND_OVERSOLD: &str = "intraday_oversold";
const NAME_OVERSOLD: &str = "超跌反弹股";

/// 启动后台任务：每 5 分钟一轮。
pub fn spawn(
    db: DatabaseConnection,
    hub: Hub,
    miana: MianaClient,
    cache: RealtimeDayCache,
) {
    tokio::spawn(async move {
        sleep_until_next_slot().await;
        let mut ticker = tokio::time::interval(Duration::from_secs(INTERVAL_SECS));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            match run_once(&db, &hub, &miana, &cache).await {
                Ok((t, o)) => {
                    if t.is_none() && o.is_none() {
                        info!("日内推送：本轮两类均无新推送");
                    }
                }
                Err(e) => warn!("日内推送失败: {e:#}"),
            }
        }
    });
    info!(
        "日内 TopN 推送已启动：间隔 {}s（趋势 + 超跌，仅开市竞价）",
        INTERVAL_SECS
    );
}

async fn sleep_until_next_slot() {
    let sh = Utc::now().with_timezone(&session::shanghai_offset());
    let mins = sh.hour() as u32 * 60 + sh.minute() as u32;
    let next_slot = ((mins / 5) + 1) * 5;
    let wait_mins = next_slot.saturating_sub(mins);
    let wait_secs = wait_mins as u64 * 60;
    let wait = wait_secs.saturating_sub(sh.second() as u64).max(1);
    tokio::time::sleep_until(Instant::now() + Duration::from_secs(wait.min(300))).await;
}

async fn run_once(
    db: &DatabaseConnection,
    hub: &Hub,
    miana: &MianaClient,
    cache: &RealtimeDayCache,
) -> anyhow::Result<(Option<()>, Option<()>)> {
    if !data_source::is_open_auction_session(db).await {
        tracing::debug!("非交易时段：跳过日内 TopN 推送");
        return Ok((None, None));
    }
    if !miana.token_configured() {
        warn!("跳过日内推送：开市竞价时段未配置 MIANA_TOKEN");
        return Ok((None, None));
    }

    let bundle = compute_bundle(db, miana, cache).await?;
    let now = session::shanghai_now_rfc3339();
    let trade_date = session::shanghai_today();

    result_cache::store_intraday(db, &trade_date, &bundle.trend, &now)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    result_cache::store_intraday_oversold(db, &trade_date, &bundle.oversold, &now)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let pushed_trend = if bundle.trend.items.is_empty() {
        false
    } else {
        push_notification(
            db,
            hub,
            KIND_TREND,
            NAME_TREND,
            "上涨趋势",
            &bundle.trend,
            false,
        )
        .await?
    };
    let pushed_os = if bundle.oversold.items.is_empty() {
        false
    } else {
        push_notification(
            db,
            hub,
            KIND_OVERSOLD,
            NAME_OVERSOLD,
            "超跌反弹",
            &bundle.oversold,
            true,
        )
        .await?
    };

    if pushed_trend {
        info!(
            date = %bundle.trend.date,
            n = bundle.trend.items.len(),
            "上涨趋势 Top 已推送"
        );
    }
    if pushed_os {
        info!(
            date = %bundle.oversold.date,
            n = bundle.oversold.items.len(),
            "超跌反弹 Top 已推送"
        );
    }

    Ok((
        pushed_trend.then_some(()),
        pushed_os.then_some(()),
    ))
}

async fn compute_bundle(
    db: &DatabaseConnection,
    miana: &MianaClient,
    cache: &RealtimeDayCache,
) -> anyhow::Result<IntradayAlertBundle> {
    // 11 = 近 10 个已收盘日 + 当日（超跌回看 10 日；趋势 B 仍用其中近 5 日）
    let input = data_source::load_market_window_hybrid(db, 11, miana, cache).await?;
    let mut bundle = calculate_alert_bundle(&input);
    enrich_names(db, &mut bundle.trend).await?;
    enrich_names(db, &mut bundle.oversold).await?;
    Ok(bundle)
}

async fn push_notification(
    db: &DatabaseConnection,
    hub: &Hub,
    kind: &str,
    cond_name: &str,
    title_prefix: &str,
    result: &IntradayAlertResult,
    oversold: bool,
) -> anyhow::Result<bool> {
    let cond_id = ensure_system_condition(db, kind, cond_name).await?;
    let now = session::shanghai_now_rfc3339();
    let title = format!("{title_prefix} Top{}", result.items.len());
    let body = format_body(result, oversold);
    let payload_val = serde_json::json!({
        "kind": kind,
        "date": result.date,
        "r_mkt": result.r_mkt,
        "sample_size": result.sample_size,
        "candidate_size": result.candidate_size,
        "items": result.items,
    });
    let payload_str = payload_val.to_string();

    if latest_payload_eq(db, cond_id, &payload_str).await? {
        info!(kind, date = %result.date, "与上一条相同，跳过推送");
        return Ok(false);
    }

    let notif = notification::ActiveModel {
        condition_id: Set(cond_id),
        title: Set(title.clone()),
        body: Set(body.clone()),
        payload: Set(Some(payload_str.clone())),
        read: Set(false),
        created_at: Set(now.clone()),
        ..Default::default()
    };
    let inserted = notification::Entity::insert(notif).exec(db).await?;
    let id = inserted.last_insert_id;

    let push_data = serde_json::json!({
        "id": id,
        "condition_id": cond_id,
        "title": title,
        "body": body,
        "payload": payload_str,
        "read": false,
        "created_at": now,
    });
    hub.broadcast(PushMessage::notification(push_data));
    Ok(true)
}

async fn latest_payload_eq(
    db: &DatabaseConnection,
    condition_id: i64,
    payload: &str,
) -> anyhow::Result<bool> {
    let last = notification::Entity::find()
        .filter(notification::Column::ConditionId.eq(condition_id))
        .order_by_desc(notification::Column::Id)
        .one(db)
        .await?;
    Ok(last
        .and_then(|n| n.payload)
        .as_deref()
        .is_some_and(|p| p == payload))
}

fn short_code(symbol: &str) -> &str {
    symbol.split('.').next().unwrap_or(symbol)
}

fn format_item_line(rank: usize, i: &AlertItem, oversold: bool) -> String {
    let code = short_code(&i.symbol);
    let name_part = if i.name.is_empty() {
        code.to_string()
    } else {
        format!("{}({})", i.name, code)
    };
    if oversold {
        format!(
            "{}. {} 得分{:.0} 回撤{:+.1}% 涨跌{:+.1}%",
            rank, name_part, i.score, i.drawdown_pct, i.change_pct
        )
    } else {
        format!(
            "{}. {} 得分{:.0} 相对{:+.1}%",
            rank, name_part, i.score, i.rs
        )
    }
}

fn format_body(r: &IntradayAlertResult, oversold: bool) -> String {
    let mut lines = vec![format!("大盘等权 {:+.2}%", r.r_mkt)];
    for (idx, item) in r.items.iter().take(5).enumerate() {
        lines.push(format_item_line(idx + 1, item, oversold));
    }
    if r.items.len() > 5 {
        lines.push(format!("…共 {} 只，打开 App 查看全部", r.items.len()));
    }
    lines.join("\n")
}

async fn enrich_names(
    db: &DatabaseConnection,
    result: &mut IntradayAlertResult,
) -> anyhow::Result<()> {
    let symbols: Vec<String> = result.items.iter().map(|i| i.symbol.clone()).collect();
    match data_source::load_stock_names(db, &symbols).await {
        Ok(map) => {
            for item in &mut result.items {
                if let Some(n) = map.get(&item.symbol) {
                    item.name = n.clone();
                }
            }
        }
        Err(e) => {
            warn!("加载股票简称失败（仍推送代码）: {e}");
        }
    }
    Ok(())
}

async fn ensure_system_condition(
    db: &DatabaseConnection,
    kind: &str,
    name: &str,
) -> anyhow::Result<i64> {
    if let Some(m) = condition::Entity::find()
        .filter(condition::Column::Kind.eq(kind))
        .one(db)
        .await?
    {
        if m.name != name {
            use sea_orm::ActiveModelTrait;
            let id = m.id;
            let mut active: condition::ActiveModel = m.into();
            active.name = Set(name.to_string());
            active.updated_at = Set(session::shanghai_now_rfc3339());
            active.update(db).await?;
            return Ok(id);
        }
        return Ok(m.id);
    }
    let now = session::shanghai_now_rfc3339();
    let active = condition::ActiveModel {
        name: Set(name.into()),
        kind: Set(kind.into()),
        symbol: Set(None),
        expression: Set("{}".into()),
        enabled: Set(true),
        created_at: Set(now.clone()),
        updated_at: Set(now),
        ..Default::default()
    };
    let inserted = condition::Entity::insert(active).exec(db).await?;
    Ok(inserted.last_insert_id)
}

/// 调试：立即计算两类（不推送）。
pub async fn compute_now(
    db: &DatabaseConnection,
    miana: &MianaClient,
    cache: &RealtimeDayCache,
) -> anyhow::Result<IntradayAlertBundle> {
    compute_bundle(db, miana, cache).await
}
