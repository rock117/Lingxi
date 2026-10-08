use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbErr};
use tracing::info;

/// 启动时执行的 SQL migration（与 `migrations/*.sql` 保持同步）。
/// 全部使用 `IF NOT EXISTS`，可重复执行。
const INIT_SQL: &str = include_str!("../migrations/20260721000001_init.sql");

/// 初始化数据库连接，并确保 schema 已就绪。
pub async fn init_db(database_url: &str) -> anyhow::Result<DatabaseConnection> {
    let db = Database::connect(database_url).await?;
    info!("数据库已连接: {}", database_url);
    run_migrations(&db).await?;
    Ok(db)
}

async fn run_migrations(db: &DatabaseConnection) -> Result<(), DbErr> {
    for stmt in split_sql_statements(INIT_SQL) {
        db.execute_unprepared(&stmt).await?;
    }
    info!("数据库 schema 已就绪");
    Ok(())
}

fn split_sql_statements(sql: &str) -> Vec<String> {
    let cleaned = sql
        .lines()
        .filter(|line| {
            let t = line.trim();
            !t.is_empty() && !t.starts_with("--")
        })
        .collect::<Vec<_>>()
        .join("\n");

    cleaned
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}
