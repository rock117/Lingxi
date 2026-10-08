use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, DbErr};
use tracing::info;

const INIT_SQL: &str = include_str!("../migrations/20260721000001_init.sql");

/// 初始化 PostgreSQL 连接，并确保 schema 已就绪。
pub async fn init_db(database_url: &str) -> anyhow::Result<DatabaseConnection> {
    let db = Database::connect(database_url).await?;
    if db.get_database_backend() != DatabaseBackend::Postgres {
        anyhow::bail!("仅支持 PostgreSQL，当前 DATABASE_URL={}", redact_url(database_url));
    }
    info!("数据库已连接: {}", redact_url(database_url));
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

fn redact_url(url: &str) -> String {
    if let Some(scheme_end) = url.find("://") {
        let rest = &url[scheme_end + 3..];
        if let Some(at) = rest.find('@') {
            let creds = &rest[..at];
            if let Some(colon) = creds.find(':') {
                let user = &creds[..colon];
                return format!("{}://{}:***@{}", &url[..scheme_end], user, &rest[at + 1..]);
            }
        }
    }
    url.to_string()
}
