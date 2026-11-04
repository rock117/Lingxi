use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ai_chat_message")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub session_id: i64,
    /// user | assistant | tool
    pub role: String,
    pub content: String,
    pub tool_name: Option<String>,
    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub tool_payload: Option<Json>,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::ai_chat_session::Entity",
        from = "Column::SessionId",
        to = "super::ai_chat_session::Column::Id"
    )]
    Session,
}

impl Related<super::ai_chat_session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Session.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
