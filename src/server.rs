// biffjezos asked Copilot (Windows 11) to port /services/server.js
// example: node.js /api/health to rust /api/v2/health

use axum::{
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::time::timeout;
use reqwest::Client;

// ============================================================
// CONFIG
// ============================================================

#[derive(Clone)]
struct Cfg {
    port: u16,
    auth: String,
    users: String,
    loc: String,
    msg: String,
    fav: String,
    tiers: String,
}

impl Cfg {
    fn from_env() -> Self {
        Self {
            port: std::env::var("PORT").unwrap_or("3000".into()).parse().unwrap(),
            auth: std::env::var("AUTH_SERVICE_URL").unwrap_or("http://auth".into()),
            users: std::env::var("USER_SERVICE_URL").unwrap_or("http://usr".into()),
            loc: std::env::var("LOC_SERVICE_URL").unwrap_or("http://loc".into()),
            msg: std::env::var("MSG_SERVICE_URL").unwrap_or("http://msg".into()),
            fav: std::env::var("FAV_SERVICE_URL").unwrap_or("http://fav".into()),
            tiers: std::env::var("TIERS_SERVICE_URL").unwrap_or("http://tiers".into()),
        }
    }
}

// ============================================================
// CACHE
// ============================================================

struct HealthCache {
    body: Value,
    status: u16,
    expires_at: Instant,
}

struct CacheState {
    inner: Mutex<Option<HealthCache>>,
}

impl CacheState {
    fn new() -> Self {
        Self { inner: Mutex::new(None) }
    }

    fn get(&self) -> Option<HealthCache> {
        let guard = self.inner.lock().unwrap();
        guard.clone()
    }

    fn set(&self, body: Value, status: u16, ttl_ms: u64) {
        let mut guard = self.inner.lock().unwrap();
        *guard = Some(HealthCache {
            body,
            status,
            expires_at: Instant::now() + Duration::from_millis(ttl_ms),
        });
    }
}

// ============================================================
// HEALTH HANDLER
// ============================================================

async fn health_handler(
    cfg: Arc<Cfg>,
    cache: Arc<CacheState>,
    client: Client,
) -> (axum::http::StatusCode, Json<Value>) {
    const TTL_MS: u64 = 30_000;

    // --- Cache check ---------------------------------------------------------
    if let Some(c) = cache.get() {
        if Instant::now() < c.expires_at {
            return (axum::http::StatusCode::from_u16(c.status).unwrap(), Json(c.body));
        }
    }

    // --- Build service list --------------------------------------------------
    let services = vec![
        ("auth", format!("{}/health", cfg.auth)),
        ("users", format!("{}/health", cfg.users)),
        ("location", format!("{}/health", cfg.loc)),
        ("messages", format!("{}/health", cfg.msg)),
        ("favourites", format!("{}/health", cfg.fav)),
        ("tiers", format!("{}/health", cfg.tiers)),
    ];

    // --- Parallel health checks ---------------------------------------------
    let mut results = HashMap::new();

    for (name, url) in services {
        let fut = client.get(url).send();
        let res = timeout(Duration::from_secs(3), fut).await;

        let status = match res {
            Ok(Ok(r)) if r.status().is_success() => "ok",
            Ok(Ok(_)) => "degraded",
            _ => "down",
        };

        results.insert(name.to_string(), json!(status));
    }

    let all_ok = results.values().all(|v| v == "ok");
    let ts = chrono::Utc::now().timestamp_millis();

    let body = json!({
        "ok": all_ok,
        "services": results,
        "ts": ts
    });

    let status = if all_ok { 200 } else { 503 };

    // Cache only successful results
    if all_ok {
        cache.set(body.clone(), status, TTL_MS);
    }

    (axum::http::StatusCode::from_u16(status).unwrap(), Json(body))
}

// ============================================================
// MAIN
// ============================================================

#[tokio::main]
async fn main() {
    let cfg = Arc::new(Cfg::from_env());
    let cache = Arc::new(CacheState::new());
    let client = Client::new();

    let app = Router::new()
        .route(
            "/api/v2/health",
            get({
                let cfg = cfg.clone();
                let cache = cache.clone();
                let client = client.clone();
                move || health_handler(cfg, cache, client)
            }),
        );

    let addr = format!("0.0.0.0:{}", cfg.port);
    println!("health-service running on {}", addr);

    axum::Server::bind(&addr.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
