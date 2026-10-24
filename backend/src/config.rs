use std::env;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    /// Miana Open API 根地址。
    pub miana_base_url: String,
    /// Miana token（盘中实时行情必填）。
    pub miana_token: String,
    /// 每分钟最多请求次数（官方上限 500，默认略低留余量）。
    pub miana_rate_limit_per_min: u32,
    /// 盘中最新日实时快照内存缓存秒数。
    pub miana_realtime_cache_secs: u64,
    /// 文件日志目录（默认 `log`；远端部署为 `/home/rock/apps/lingxi/log`）。
    pub log_dir: PathBuf,
    /// 单个日志文件最大字节数（默认 10MiB）。
    pub log_max_bytes: u64,
    /// OpenAI 兼容 API 根路径（如 `https://api.openai.com/v1`）。
    pub ai_api_base: String,
    /// 大模型 API Key；为空则 AI 关闭。
    pub ai_api_key: String,
    /// 模型名。
    pub ai_model: String,
    /// AI HTTP 超时秒数。
    pub ai_timeout_secs: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://lingxi:lingxi@127.0.0.1:5432/lingxi".to_string()
            }),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8000),
            miana_base_url: env::var("MIANA_BASE_URL")
                .unwrap_or_else(|_| "https://miana.com.cn".to_string()),
            miana_token: env::var("MIANA_TOKEN").unwrap_or_default(),
            miana_rate_limit_per_min: env::var("MIANA_RATE_LIMIT_PER_MIN")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(480),
            miana_realtime_cache_secs: env::var("MIANA_REALTIME_CACHE_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(120),
            log_dir: env::var("LOG_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("log")),
            log_max_bytes: env::var("LOG_MAX_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10 * 1024 * 1024),
            // 默认对接 DeepSeek（OpenAI 兼容）；见 https://api-docs.deepseek.com/
            ai_api_base: env::var("AI_API_BASE")
                .unwrap_or_else(|_| "https://api.deepseek.com".to_string()),
            ai_api_key: env::var("AI_API_KEY").unwrap_or_default(),
            ai_model: env::var("AI_MODEL").unwrap_or_else(|_| "deepseek-flash".to_string()),
            ai_timeout_secs: env::var("AI_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
        }
    }
}
