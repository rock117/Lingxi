use entity::notification;
use rocket::serde::json::Json;
use rocket::{get, post};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::Serialize;

use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize)]
pub struct NotificationOut {
    pub id: i64,
    pub condition_id: i64,
    pub title: String,
    pub body: String,
    pub payload: Option<serde_json::Value>,
    pub read: bool,
    pub created_at: String,
}

impl TryFrom<notification::Model> for NotificationOut {
    type Error = AppError;
    fn try_from(m: notification::Model) -> AppResult<Self> {
        let payload = match m.payload {
            Some(s) => Some(serde_json::from_str(&s).map_err(|e| {
                AppError::Internal(anyhow::anyhow!("payload JSON 解析失败: {e}"))
            })?),
            None => None,
        };
        Ok(Self {
            id: m.id,
            condition_id: m.condition_id,
            title: m.title,
            body: m.body,
            payload,
            read: m.read,
            created_at: m.created_at,
        })
    }
}

#[get("/api/notifications?<unread>")]
pub async fn list(
    db: &rocket::State<sea_orm::DatabaseConnection>,
    unread: Option<bool>,
) -> AppResult<Json<Vec<NotificationOut>>> {
    let mut query = notification::Entity::find();
    if let Some(true) = unread {
        query = query.filter(notification::Column::Read.eq(false));
    }
    let items = query
        .order_by_desc(notification::Column::CreatedAt)
        .all(db.inner())
        .await?;

    let out: Vec<NotificationOut> = items
        .into_iter()
        .map(TryFrom::try_from)
        .collect::<Result<_, _>>()?;
    Ok(Json(out))
}

#[post("/api/notifications/<id>/read")]
pub async fn mark_read(
    db: &rocket::State<sea_orm::DatabaseConnection>,
    id: i64,
) -> AppResult<Json<NotificationOut>> {
    let m = notification::Entity::find_by_id(id)
        .one(db.inner())
        .await?
        .ok_or(AppError::NotFound)?;

    let mut active: notification::ActiveModel = m.into();
    active.read = Set(true);
    let saved = active.update(db.inner()).await?;
    Ok(Json(saved.try_into()?))
}
