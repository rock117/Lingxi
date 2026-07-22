use rocket::get;
use rocket::serde::json::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct Health {
    status: String,
}

#[get("/api/health")]
pub fn health() -> Json<Health> {
    Json(Health {
        status: "ok".into(),
    })
}
