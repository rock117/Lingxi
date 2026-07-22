use sea_orm::{Database, DatabaseConnection};
use tracing::info;

/// 初始化数据库连接（不执行 migration，migration 由 sea-orm-cli 执行）
pub async fn init_db(database_url: &str) -> anyhow::Result<DatabaseConnection> {
    let db = Database::connect(database_url).await?;
    info!("数据库已连接: {}", database_url);
    Ok(db)
}
