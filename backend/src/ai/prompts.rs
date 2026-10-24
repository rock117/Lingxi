//! AI 系统提示模板。
//! 个股四维提示词默认读 `backend/config/ai_prompts.toml`（可用 `AI_PROMPTS_PATH` 覆盖）。
//! 编译期内嵌同文件，作为找不到配置时的回退。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Deserialize;
use tracing::{info, warn};

const EMBEDDED_AI_PROMPTS: &str = include_str!("../../config/ai_prompts.toml");

#[derive(Debug, Clone, Deserialize)]
struct StockAnalyzePromptsFile {
    stock_analyze: StockAnalyzeSection,
}

#[derive(Debug, Clone, Deserialize)]
struct StockAnalyzeSection {
    #[serde(default)]
    common_rules: String,
    tape: String,
    short_term: String,
    fundamentals: String,
    statements: String,
}

#[derive(Debug, Clone)]
struct StockAnalyzePrompts {
    tape: String,
    short_term: String,
    fundamentals: String,
    statements: String,
}

fn apply_common(template: &str, common: &str) -> String {
    template
        .replace("{common_rules}", common.trim())
        .trim()
        .to_string()
}

fn from_toml_str(raw: &str) -> anyhow::Result<StockAnalyzePrompts> {
    let file: StockAnalyzePromptsFile = toml::from_str(raw)?;
    let s = file.stock_analyze;
    Ok(StockAnalyzePrompts {
        tape: apply_common(&s.tape, &s.common_rules),
        short_term: apply_common(&s.short_term, &s.common_rules),
        fundamentals: apply_common(&s.fundamentals, &s.common_rules),
        statements: apply_common(&s.statements, &s.common_rules),
    })
}

fn builtin_stock_analyze() -> StockAnalyzePrompts {
    from_toml_str(EMBEDDED_AI_PROMPTS).unwrap_or_else(|e| {
        warn!(error = %e, "内嵌 ai_prompts.toml 解析失败，使用极简回退");
        let common = "只根据用户 JSON 事实写作；简体中文短文；禁止买卖指令；仅供参考，非投资建议。";
        StockAnalyzePrompts {
            tape: format!("你是 A 股盘面分析助手。\n{common}"),
            short_term: format!("你是 A 股短线分析助手。\n{common}"),
            fundamentals: format!("你是 A 股基本面分析助手。\n{common}"),
            statements: format!("你是 A 股财报分析助手。\n{common}"),
        }
    })
}

fn candidate_prompt_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(p) = std::env::var("AI_PROMPTS_PATH") {
        let p = p.trim();
        if !p.is_empty() {
            paths.push(PathBuf::from(p));
        }
    }
    paths.push(PathBuf::from("config/ai_prompts.toml"));
    paths.push(PathBuf::from("backend/config/ai_prompts.toml"));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.join("config/ai_prompts.toml"));
        }
    }
    paths
}

fn load_from_path(path: &Path) -> anyhow::Result<StockAnalyzePrompts> {
    let raw = std::fs::read_to_string(path)?;
    from_toml_str(&raw)
}

fn stock_analyze_prompts() -> &'static StockAnalyzePrompts {
    static PROMPTS: OnceLock<StockAnalyzePrompts> = OnceLock::new();
    PROMPTS.get_or_init(|| {
        for path in candidate_prompt_paths() {
            if !path.is_file() {
                continue;
            }
            match load_from_path(&path) {
                Ok(p) => {
                    info!(path = %path.display(), "已加载 AI 提示词配置");
                    return p;
                }
                Err(e) => {
                    warn!(
                        path = %path.display(),
                        error = %e,
                        "读取 AI 提示词失败，继续尝试下一路径"
                    );
                }
            }
        }
        warn!("未找到可用的 ai_prompts.toml，使用编译期内嵌提示词");
        builtin_stock_analyze()
    })
}

pub fn explain_system(kind: &str) -> String {
    format!(
        r#"你是 Lingxi 行情助手，负责用简体中文解释已经计算好的市场/财报结果。
规则：
1. 只根据用户消息里提供的 JSON 事实写作，禁止编造数字或股票名单。
2. 输出 3～5 句短文，口语清晰，不要用 Markdown 标题，不要代码块。
3. 若 JSON 缺字段或样本为 0，如实说明数据不足。
4. 结果类型 kind = {kind}。
口径提示：赚钱面=归母净利>0 占比；营收增长面=营收同比>0；经营现金流为正=经营净额>0。"#
    )
}

pub fn stock_analyze_tape_system() -> String {
    stock_analyze_prompts().tape.clone()
}

pub fn stock_analyze_short_term_system() -> String {
    stock_analyze_prompts().short_term.clone()
}

pub fn stock_analyze_fundamentals_system() -> String {
    stock_analyze_prompts().fundamentals.clone()
}

pub fn stock_analyze_statements_system() -> String {
    stock_analyze_prompts().statements.clone()
}

pub fn draft_screen_system(catalog_json: &str) -> String {
    format!(
        r#"你是 Lingxi 条件选股助手。根据用户自然语言，生成可执行的选股 expression JSON。
只能使用下列策略目录中的 type_id 与参数（不要发明新 type）：
{catalog_json}

输出要求：
1. 只输出一个 JSON 对象，不要 Markdown，不要其它说明文字。
2. JSON 形状：
{{
  "expression": {{ "op": "and", "rules": [ /* rule 对象，含 type 与参数 */ ] }},
  "explanation": "一两句中文说明如何映射用户意图"
}}
3. 数值阈值用数字；枚举必须是目录 options 中的 value。
4. 金额「当期值」单位为亿元；同比/环比为百分数。
5. 不确定的条件宁可省略，不要猜测。"#
    )
}
