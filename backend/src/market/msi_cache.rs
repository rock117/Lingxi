//! MSI 结果缓存：进程内 + `msi_cache` 表（JSONB）。
//!
//! 按交易日 `date`（`yyyy-MM-dd`）存一条；开市日竞价窗内重算落库；
//! 非交易时段取 `date` 最近一条返回（不区分 `days`）。

use std::sync::{Mutex, OnceLock};

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, FromQueryResult, Statement};
use serde_json::Value as JsonValue;

use super::domain::MsiResult;
use crate::error::{AppError, AppResult};

fn memory() -> &'static Mutex<Option<MsiResult>> {
    static CACHE: OnceLock<Mutex<Option<MsiResult>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn put_memory(result: &MsiResult) {
    if let Ok(mut g) = memory().lock() {
        *g = Some(result.clone());
    }
}

fn get_memory() -> Option<MsiResult> {
    memory().lock().ok().and_then(|g| g.clone())
}

/// 库内 `date` 最大的一条；无行 → `None`。
pub async fn load_latest(db: &DatabaseConnection) -> Result<Option<(String, MsiResult)>, DbErr> {
    #[derive(Debug, FromQueryResult)]
    struct Row {
        date: String,
        payload: JsonValue,
    }

    let row = Row::find_by_statement(Statement::from_string(
        DbBackend::Postgres,
        "SELECT date, payload FROM msi_cache ORDER BY date DESC LIMIT 1".to_string(),
    ))
    .one(db)
    .await?;

    match row {
        Some(r) => {
            let parsed: MsiResult = serde_json::from_value(r.payload).map_err(|e| {
                DbErr::Custom(format!("msi_cache payload 反序列化失败: {e}"))
            })?;
            Ok(Some((r.date, parsed)))
        }
        None => Ok(None),
    }
}

/// Upsert：同一 `date` 覆盖。
pub async fn save_db(
    db: &DatabaseConnection,
    date: &str,
    result: &MsiResult,
) -> AppResult<()> {
    let payload = serde_json::to_value(result)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("msi_cache 序列化失败: {e}")))?;
    let updated_at = result.calculated_at.clone();

    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
INSERT INTO msi_cache (date, payload, updated_at)
VALUES ($1, $2, $3)
ON CONFLICT (date) DO UPDATE SET
    payload = EXCLUDED.payload,
    updated_at = EXCLUDED.updated_at
"#,
        [date.into(), payload.into(), updated_at.into()],
    ))
    .await
    .map_err(AppError::from)?;

    Ok(())
}

/// 内存优先，未命中再读库最近一条并回填内存。
pub async fn get_latest(db: &DatabaseConnection) -> AppResult<Option<MsiResult>> {
    if let Some(hit) = get_memory() {
        return Ok(Some(hit));
    }
    match load_latest(db).await {
        Ok(Some((_date, hit))) => {
            put_memory(&hit);
            Ok(Some(hit))
        }
        Ok(None) => Ok(None),
        Err(e) => Err(AppError::from(e)),
    }
}

/// 按当日 `date` 写内存并落库。
pub async fn store(
    db: &DatabaseConnection,
    date: &str,
    result: &MsiResult,
) -> AppResult<()> {
    put_memory(result);
    save_db(db, date, result).await
}
