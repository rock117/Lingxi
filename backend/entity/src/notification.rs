use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "notification")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub condition_id: i64,
    pub title: String,
    pub body: String,
    pub payload: Option<String>,
    pub read: bool,
    pub created_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::condition::Entity",
        from = "Column::ConditionId",
        to = "super::condition::Column::Id"
    )]
    Condition,
}

impl Related<super::condition::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Condition.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
