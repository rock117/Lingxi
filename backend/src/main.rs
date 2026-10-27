mod access_log;
mod ai;
mod config;
mod db;
mod error;
mod logging;
mod market;
mod request_id;
mod routes;
mod ws;

use std::collections::HashSet;

use rocket::http::Method;
use rocket::routes;
use rocket_cors::{AllowedHeaders, AllowedOrigins, CorsOptions};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

use config::AppConfig;
use ws::hub::Hub;

fn init_logging(cfg: &AppConfig) {
    // sqlx 默认会打每条 SQL 的 INFO，压到 WARN 以免刷屏
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
        .add_directive("lingxi_backend=info".parse().expect("log directive"))
        .add_directive("sqlx=warn".parse().expect("log directive"))
        .add_directive("sqlx::query=warn".parse().expect("log directive"));

    let timer = logging::ShanghaiTimer;
    let stdout_layer = fmt::layer()
        .event_format(logging::RequestIdFormat::new(timer.clone(), true))
        .with_writer(std::io::stdout);

    match logging::DateSizeFileWriter::open(&cfg.log_dir, "lingxi", cfg.log_max_bytes) {
        Ok(file_writer) => {
            let file_hint = file_writer.current_path_hint();
            let file_layer = fmt::layer()
                .event_format(logging::RequestIdFormat::new(timer, false))
                .with_writer(file_writer);
            tracing_subscriber::registry()
                .with(filter)
                .with(stdout_layer)
                .with(file_layer)
                .init();
            tracing::info!(
                log_dir = %cfg.log_dir.display(),
                log_file = %file_hint,
                log_max_bytes = cfg.log_max_bytes,
                "文件日志已启用（当前 lingxi.log，归档带日期，东八区时间戳，单文件上限）"
            );
        }
        Err(e) => {
            eprintln!(
                "无法打开文件日志 dir={}: {}；仅输出到 stdout",
                cfg.log_dir.display(),
                e
            );
            tracing_subscriber::registry()
                .with(filter)
                .with(stdout_layer)
                .init();
        }
    }
}

#[rocket::launch]
async fn rocket() -> _ {
    let cfg = AppConfig::default();
    init_logging(&cfg);

    if cfg.miana_token.trim().is_empty() {
        tracing::warn!("MIANA_TOKEN 未配置：盘中实时行情与日内推送将不可用");
    }

    let ai = ai::AiClient::from_config(&cfg).expect("初始化 AI 客户端失败");
    if ai.enabled() {
        tracing::info!(model = %cfg.ai_model, base = %cfg.ai_api_base, "AI 已启用（仅 backend 内部调用）");
    } else {
        tracing::info!("AI_API_KEY 未配置：AiClient 关闭，调用方需自行降级");
    }

    let miana = market::miana::MianaClient::new(&cfg).expect("初始化 Miana 客户端失败");
    let eastmoney =
        market::eastmoney::EastmoneyClient::new().expect("初始化东财客户端失败");
    let tencent =
        market::tencent::TencentClient::new().expect("初始化腾讯财经客户端失败");
    let realtime_cache =
        market::realtime_cache::RealtimeDayCache::new(cfg.miana_realtime_cache_secs);

    // 初始化 PostgreSQL 并自动执行 migrations
    let db = db::init_db(&cfg.database_url)
        .await
        .expect("数据库初始化失败");

    match market::data_source::probe_market_tables(&db).await {
        Ok((n, last)) => tracing::info!(
            "行情源就绪: active_stocks={} last_kline_date={:?} realtime_session={}",
            n,
            last,
            market::session::is_realtime_session_now(),
        ),
        Err(e) => tracing::warn!("行情表探测失败（MSI/MTT/日内推送将无法使用真实行情）: {}", e),
    }

    // WebSocket Hub + 日内强势推送（每 5 分钟）
    let hub = Hub::new(256);
    market::intraday_push::spawn(db.clone(), hub.clone(), miana.clone(), realtime_cache.clone());

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
        .manage(ai)
        .manage(miana)
        .manage(eastmoney)
        .manage(tencent)
        .manage(realtime_cache)
        .manage(hub)
        .attach(request_id::RequestIdFairing)
        .attach(access_log::AccessLog)
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
                routes::notification::delete_one,
                routes::notification::delete_all,
                market::routes::sentiment,
                market::routes::mid_term_trend,
                market::routes::intraday_alerts,
                market::routes::strategies,
                market::routes::screen_preview,
                market::routes::screen_by_condition,
                market::routes::stock_detail,
                market::routes::stocks_search,
                market::routes::stock_analyze_query,
                market::routes::stock_analyze_ticks_query,
                market::routes::stock_analyze_ai_query,
                market::routes::sector_movers,
                market::routes::sector_detail,
                market::routes::fund_stats_meta,
                market::routes::fund_stats_query,
                ws::ws_handler,
            ],
        )
}
