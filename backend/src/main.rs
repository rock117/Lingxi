mod config;
mod db;
mod engine;
mod error;
mod market;
mod routes;
mod ws;

use std::collections::HashSet;

use rocket::http::Method;
use rocket::routes;
use rocket_cors::{AllowedHeaders, AllowedOrigins, CorsOptions};
use tracing_subscriber::EnvFilter;

use config::AppConfig;
use ws::hub::Hub;

#[rocket::launch]
async fn rocket() -> _ {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("lingxi_backend=info".parse().unwrap()),
        )
        .init();

    let cfg = AppConfig::default();
    if cfg.market_use_mock {
        tracing::warn!("MARKET_USE_MOCK=true：MSI/MTT 将使用本地 Mock 行情");
    }
    if !cfg.market_use_mock && cfg.miana_token.trim().is_empty() {
        tracing::warn!("MIANA_TOKEN 未配置：盘中实时行情窗口将无法覆盖最新交易日");
    }

    let miana = market::miana::MianaClient::new(&cfg).expect("初始化 Miana 客户端失败");
    let realtime_cache =
        market::realtime_cache::RealtimeDayCache::new(cfg.miana_realtime_cache_secs);

    // 初始化 PostgreSQL 并自动执行 migrations
    let db = db::init_db(&cfg.database_url)
        .await
        .expect("数据库初始化失败");

    if !cfg.market_use_mock {
        match market::data_source::probe_market_tables(&db).await {
            Ok((n, last)) => tracing::info!(
                "行情源就绪: active_stocks={} last_kline_date={:?} realtime_session={}",
                n,
                last,
                market::session::is_realtime_session_now(),
            ),
            Err(e) => tracing::warn!(
                "行情表探测失败（MSI/MTT 将无法使用真实行情）: {}",
                e
            ),
        }
    }

    // WebSocket Hub
    let hub = Hub::new(256);

    // 启动 Mock 引擎
    engine::mock::spawn(db.clone(), hub.clone(), 10);

    // CORS：允许所有来源（个人自用，方便 App 调试）
    let mut methods = HashSet::new();
    methods.insert(Method::Get.into());
    methods.insert(Method::Post.into());
    methods.insert(Method::Put.into());
    methods.insert(Method::Delete.into());
    methods.insert(Method::Options.into());

    let cors = CorsOptions {
        allowed_origins: AllowedOrigins::all(),
        allowed_methods: methods,
        allowed_headers: AllowedHeaders::some(&["Authorization", "Accept", "Content-Type"]),
        allow_credentials: true,
        ..Default::default()
    }
    .to_cors()
    .expect("CORS 配置失败");

    // 构建 Rocket
    rocket::build()
        .configure(rocket::Config {
            port: cfg.port,
            address: cfg.host.parse().expect("无效的 HOST"),
            ..rocket::Config::default()
        })
        .manage(db)
        .manage(cfg)
        .manage(miana)
        .manage(realtime_cache)
        .manage(hub)
        .attach(cors)
        .mount(
            "/",
            routes![
                routes::health::health,
                routes::condition::create,
                routes::condition::list,
                routes::condition::get_one,
                routes::condition::update,
                routes::condition::delete,
                routes::notification::list,
                routes::notification::mark_read,
                market::routes::sentiment,
                market::routes::mid_term_trend,
                ws::ws_handler,
            ],
        )
}
