//! 市场计算结果缓存：进程内 + `market_result_cache`（JSONB）。
//!
//! 按 `(kind, date)` 存一条；开市日竞价窗内重算落库；
//! 非交易时段取该 `kind` 下 `date` 最近一条。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, FromQueryResult, Statement};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value as JsonValue;

use super::domain::MsiResult;
use super::intraday_alert::IntradayAlertResult;
use crate::error::{AppError, AppResult};

/// 短线情绪 MSI。
pub const KIND_MSI: &str = "msi";
/// 上涨趋势 TopN。
pub const KIND_INTRADAY_ALERT: &str = "intraday_alert";
/// 超跌反弹 TopN。
pub const KIND_INTRADAY_OVERSOLD: &str = "intraday_oversold";

fn memory() -> &'static Mutex<HashMap<&'static str, JsonValue>> {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, JsonValue>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn put_memory(kind: &'static str, payload: &JsonValue) {
    if let Ok(mut g) = memory().lock() {
        g.insert(kind, payload.clone());
    }
}

fn get_memory(kind: &str) -> Option<JsonValue> {
    memory()
        .lock()
        .ok()
        .and_then(|g| g.get(kind).cloned())
}

async fn load_latest_raw(
    db: &DatabaseConnection,
    kind: &str,
) -> Result<Option<(String, JsonValue)>, DbErr> {
    #[derive(Debug, FromQueryResult)]
    struct Row {
        date: String,
        payload: JsonValue,
    }

    let row = Row::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT date, payload FROM market_result_cache WHERE kind = $1 ORDER BY date DESC LIMIT 1",
        [kind.into()],
    ))
    .one(db)
    .await?;

    Ok(row.map(|r| (r.date, r.payload)))
}

async fn save_raw(
    db: &DatabaseConnection,
    kind: &str,
    date: &str,
    payload: &JsonValue,
    updated_at: &str,
) -> AppResult<()> {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
INSERT INTO market_result_cache (kind, date, payload, updated_at)
VALUES ($1, $2, $3, $4)
ON CONFLICT (kind, date) DO UPDATE SET
    payload = EXCLUDED.payload,
    updated_at = EXCLUDED.updated_at
"#,
        [
            kind.into(),
            date.into(),
            payload.clone().into(),
            updated_at.into(),
        ],
    ))
    .await
    .map_err(AppError::from)?;
    Ok(())
}

fn parse_payload<T: DeserializeOwned>(kind: &str, payload: JsonValue) -> Result<T, DbErr> {
    serde_json::from_value(payload)
        .map_err(|e| DbErr::Custom(format!("market_result_cache[{kind}] 反序列化失败: {e}")))
}

async fn get_latest_typed<T: DeserializeOwned>(
    db: &DatabaseConnection,
    kind: &'static str,
) -> AppResult<Option<T>> {
    if let Some(payload) = get_memory(kind) {
        return Ok(Some(parse_payload(kind, payload).map_err(AppError::from)?));
    }
    match load_latest_raw(db, kind).await {
        Ok(Some((_date, payload))) => {
            put_memory(kind, &payload);
            Ok(Some(parse_payload(kind, payload).map_err(AppError::from)?))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(AppError::from(e)),
    }
}

async fn store_typed<T: Serialize>(
    db: &DatabaseConnection,
    kind: &'static str,
    date: &str,
    result: &T,
    updated_at: &str,
) -> AppResult<()> {
    let payload = serde_json::to_value(result)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("market_result_cache 序列化失败: {e}")))?;
    put_memory(kind, &payload);
    save_raw(db, kind, date, &payload, updated_at).await
}

pub async fn get_latest_msi(db: &DatabaseConnection) -> AppResult<Option<MsiResult>> {
    get_latest_typed(db, KIND_MSI).await
}

pub async fn store_msi(
    db: &DatabaseConnection,
    date: &str,
    result: &MsiResult,
) -> AppResult<()> {
    store_typed(db, KIND_MSI, date, result, &result.calculated_at).await
}

/// 读最近一条日内强势缓存（盘中写入；休市推送已关闭，供排查/后续接口使用）。
#[allow(dead_code)]
pub async fn get_latest_intraday(
    db: &DatabaseConnection,
) -> AppResult<Option<IntradayAlertResult>> {
    get_latest_typed(db, KIND_INTRADAY_ALERT).await
}

pub async fn store_intraday(
    db: &DatabaseConnection,
    date: &str,
    result: &IntradayAlertResult,
    updated_at: &str,
) -> AppResult<()> {
    store_typed(db, KIND_INTRADAY_ALERT, date, result, updated_at).await
}

pub async fn store_intraday_oversold(
    db: &DatabaseConnection,
    date: &str,
    result: &IntradayAlertResult,
    updated_at: &str,
) -> AppResult<()> {
    store_typed(db, KIND_INTRADAY_OVERSOLD, date, result, updated_at).await
}
