use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "market_realtime_snapshot")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub symbol: String,
    /// Miana `timestamp`（毫秒）
    #[sea_orm(primary_key, auto_increment = false)]
    pub quote_ts: i64,
    pub local_date: String,
    pub trade_date: String,
    pub fetched_at: String,
    pub name: Option<String>,
    pub trade_status: Option<String>,
    pub price: f64,
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub pre_close: Option<f64>,
    pub volume: Option<f64>,
    pub amount: Option<f64>,
    pub change_amt: Option<f64>,
    pub change_rate: Option<f64>,
    pub high_limit: Option<f64>,
    pub low_limit: Option<f64>,
    pub turnover: Option<f64>,
    pub vol_ratio: Option<f64>,
    pub amplitude: Option<f64>,
    pub active_buy_vol: Option<f64>,
    pub active_sell_vol: Option<f64>,
    pub net_inflow: Option<f64>,
    pub mkt_cap: Option<f64>,
    pub float_cap: Option<f64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
