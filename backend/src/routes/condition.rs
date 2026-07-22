use chrono::Utc;
use entity::condition;
use rocket::serde::json::Json;
use rocket::{delete, get, post, put};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Deserialize)]
pub struct CreateCondition {
    pub name: String,
    pub kind: String,
    pub symbol: Option<String>,
    pub expression: String,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCondition {
    pub name: Option<String>,
    pub kind: Option<String>,
    pub symbol: Option<Option<String>>,
    pub expression: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ConditionOut {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub symbol: Option<String>,
    pub expression: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<condition::Model> for ConditionOut {
    fn from(m: condition::Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            kind: m.kind,
            symbol: m.symbol,
            expression: m.expression,
            enabled: m.enabled,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

fn validate_kind(kind: &str) -> AppResult<()> {
    match kind {
        "trade" | "news" => Ok(()),
        _ => Err(AppError::BadRequest(format!(
            "kind 必须是 'trade' 或 'news'，收到: {}",
            kind
        ))),
    }
}

#[post("/api/conditions", data = "<input>")]
pub async fn create(
    db: &rocket::State<sea_orm::DatabaseConnection>,
    input: Json<CreateCondition>,
) -> AppResult<Json<ConditionOut>> {
    validate_kind(&input.kind)?;
    let now = Utc::now().to_rfc3339();

    let model = condition::ActiveModel {
        name: Set(input.name.clone()),
        kind: Set(input.kind.clone()),
        symbol: Set(input.symbol.clone()),
        expression: Set(input.expression.clone()),
        enabled: Set(input.enabled.unwrap_or(true)),
        created_at: Set(now.clone()),
        updated_at: Set(now),
        ..Default::default()
    };
    let saved = model.insert(db.inner()).await?;
    Ok(Json(saved.into()))
}

#[get("/api/conditions")]
pub async fn list(
    db: &rocket::State<sea_orm::DatabaseConnection>,
) -> AppResult<Json<Vec<ConditionOut>>> {
    let items = condition::Entity::find().all(db.inner()).await?;
    Ok(Json(items.into_iter().map(Into::into).collect()))
}

#[get("/api/conditions/<id>")]
pub async fn get_one(
    db: &rocket::State<sea_orm::DatabaseConnection>,
    id: i64,
) -> AppResult<Json<ConditionOut>> {
    let m = condition::Entity::find_by_id(id)
        .one(db.inner())
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(m.into()))
}

#[put("/api/conditions/<id>", data = "<input>")]
pub async fn update(
    db: &rocket::State<sea_orm::DatabaseConnection>,
    id: i64,
    input: Json<UpdateCondition>,
) -> AppResult<Json<ConditionOut>> {
    let m = condition::Entity::find_by_id(id)
        .one(db.inner())
        .await?
        .ok_or(AppError::NotFound)?;

    if let Some(kind) = &input.kind {
        validate_kind(kind)?;
    }

    let mut active: condition::ActiveModel = m.into();
    if let Some(v) = &input.name {
        active.name = Set(v.clone());
    }
    if let Some(v) = &input.kind {
        active.kind = Set(v.clone());
    }
    if let Some(v) = &input.symbol {
        active.symbol = Set(v.clone());
    }
    if let Some(v) = &input.expression {
        active.expression = Set(v.clone());
    }
    if let Some(v) = input.enabled {
        active.enabled = Set(v);
    }
    active.updated_at = Set(Utc::now().to_rfc3339());

    let saved = active.update(db.inner()).await?;
    Ok(Json(saved.into()))
}

#[delete("/api/conditions/<id>")]
pub async fn delete(
    db: &rocket::State<sea_orm::DatabaseConnection>,
    id: i64,
) -> AppResult<rocket::http::Status> {
    let m = condition::Entity::find_by_id(id)
        .one(db.inner())
        .await?
        .ok_or(AppError::NotFound)?;
    m.delete(db.inner()).await?;
    Ok(rocket::http::Status::NoContent)
}

/// 删除某条件的所有通知（内部使用，条件删除时由外键 CASCADE 处理）
#[allow(dead_code)]
pub async fn delete_notifications_of(
    db: &sea_orm::DatabaseConnection,
    condition_id: i64,
) -> AppResult<()> {
    use entity::notification;
    notification::Entity::delete_many()
        .filter(notification::Column::ConditionId.eq(condition_id))
        .exec(db)
        .await?;
    Ok(())
}
