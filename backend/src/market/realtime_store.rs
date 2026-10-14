//! 将 Miana 实时截面写入 `market_realtime_snapshot`。

use entity::market_realtime_snapshot::{ActiveModel, Column, Entity};
use sea_orm::{sea_query::OnConflict, DatabaseConnection, EntityTrait, Set};
use tracing::{info, warn};

use super::miana::RealtimeSnapshot;
use crate::error::{AppError, AppResult};

pub async fn insert_snapshots(
    db: &DatabaseConnection,
    snaps: &[RealtimeSnapshot],
) -> AppResult<usize> {
    if snaps.is_empty() {
        return Ok(0);
    }

    let models: Vec<ActiveModel> = snaps
        .iter()
        .map(|s| ActiveModel {
            symbol: Set(s.symbol.clone()),
            quote_ts: Set(s.quote_ts),
            local_date: Set(s.local_date.clone()),
            trade_date: Set(s.trade_date.clone()),
            fetched_at: Set(s.fetched_at.clone()),
            name: Set(s.name.clone()),
            trade_status: Set(s.trade_status.clone()),
            price: Set(s.price),
            open: Set(s.open),
            high: Set(s.high),
            low: Set(s.low),
            pre_close: Set(s.pre_close),
            volume: Set(s.volume),
            amount: Set(s.amount),
            change_amt: Set(s.change_amt),
            change_rate: Set(s.change_rate),
            high_limit: Set(s.high_limit),
            low_limit: Set(s.low_limit),
            turnover: Set(s.turnover),
            vol_ratio: Set(s.vol_ratio),
            amplitude: Set(s.amplitude),
            active_buy_vol: Set(s.active_buy_vol),
            active_sell_vol: Set(s.active_sell_vol),
            net_inflow: Set(s.net_inflow),
            mkt_cap: Set(s.mkt_cap),
            float_cap: Set(s.float_cap),
        })
        .collect();

    // 分批插入，避免单次 SQL 过大
    const CHUNK: usize = 500;
    let mut batches_ok = 0usize;
    for chunk in models.chunks(CHUNK) {
        match Entity::insert_many(chunk.to_vec())
            .on_conflict(
                OnConflict::columns([Column::Symbol, Column::QuoteTs])
                    .do_nothing()
                    .to_owned(),
            )
            .exec(db)
            .await
        {
            Ok(_) => batches_ok += chunk.len(),
            Err(e) => {
                warn!(error = %e, n = chunk.len(), "实时行情落库失败（本批）");
                return Err(AppError::from(e));
            }
        }
    }

    info!(
        n = snaps.len(),
        wrote = batches_ok,
        "实时行情已写入 market_realtime_snapshot"
    );
    Ok(batches_ok)
}
