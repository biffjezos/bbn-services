use axum::{ routing::get, Router };
use std::fmt;

struct Health { gateway: String }

impl fmt::Display for Health {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{ gateway: {} }}", self.gateway)
    }
}

#[tokio::main]
async fn main() {
    let healthy = Health { gateway: "ok".to_string() };
    let app = Router::new().route("/api/v2/health", get(|| async { healthy }));
}
