// Legacy CLI command import removed in Phase 4 - using modular architecture
// use crate::command::cil_startminer::cmd_start_miner_from_api;
use actix_web::{post, web, HttpResponse, Responder};
use serde::Deserialize;

#[derive(Deserialize)]
struct StartMinerRequest {
    host: String,
    port: String,
    bootstrap: Option<String>,
    mining_address: String,
}

#[post("/start-miner")]
pub async fn start_miner(req: web::Json<StartMinerRequest>) -> impl Responder {
    // Log the request details even though we don't implement it
    eprintln!(
        "Legacy miner request received for {}:{} with mining address: {}",
        req.host, req.port, req.mining_address
    );
    if let Some(ref bootstrap) = req.bootstrap {
        eprintln!("Bootstrap node specified: {}", bootstrap);
    }

    HttpResponse::NotImplemented()
        .body("Legacy miner has been removed. Use 'polytorus modular mine' commands instead.")
}
