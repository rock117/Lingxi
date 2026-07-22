use rocket::http::Status;
use rocket::request::Request;
use rocket::response::{Responder, Response};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(#[from] sea_orm::DbErr),
    #[error("未找到资源")]
    NotFound,
    #[error("参数错误: {0}")]
    BadRequest(String),
    #[error("内部错误: {0}")]
    Internal(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    message: String,
}

impl<'r> Responder<'r, 'static> for AppError {
    fn respond_to(self, _: &'r Request<'_>) -> Result<Response<'static>, Status> {
        let (status, body) = match &self {
            AppError::NotFound => (
                Status::NotFound,
                ErrorResponse {
                    error: "not_found".into(),
                    message: self.to_string(),
                },
            ),
            AppError::BadRequest(_) => (
                Status::BadRequest,
                ErrorResponse {
                    error: "bad_request".into(),
                    message: self.to_string(),
                },
            ),
            _ => (
                Status::InternalServerError,
                ErrorResponse {
                    error: "internal".into(),
                    message: self.to_string(),
                },
            ),
        };

        let body = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
        let response = Response::build()
            .status(status)
            .header(rocket::http::ContentType::JSON)
            .sized_body(body.len(), std::io::Cursor::new(body))
            .finalize();
        Ok(response)
    }
}

pub type AppResult<T> = Result<T, AppError>;
