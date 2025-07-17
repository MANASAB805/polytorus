// Legacy CLI command removed - use modular architecture
use actix_web::{post, web, HttpResponse, Responder};
use serde::Deserialize;

#[derive(Deserialize)]
struct StartNodeRequest {
    host: String,
    port: String,
    bootstrap: Option<String>,
}

#[post("/start-node")]
pub async fn start_node(req: web::Json<StartNodeRequest>) -> impl Responder {
    // Log the request details even though we don't implement it
    eprintln!("Legacy node request received for {}:{}", req.host, req.port);
    if let Some(ref bootstrap) = req.bootstrap {
        eprintln!("Bootstrap node specified: {}", bootstrap);
    }

    HttpResponse::NotImplemented()
        .body("Legacy node has been removed. Use 'polytorus modular start' commands instead.")
}
