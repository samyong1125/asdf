use actix_web::{web, App, HttpResponse, HttpServer, Result};
use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;
use std::env;
use std::sync::Arc;
use tracing::{info, error};

mod database;

// App State to hold database connection
#[derive(Clone)]
pub struct AppState {
    pub session: Arc<Session>,
}

// Health check endpoint
async fn health() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "sentinel",
        "version": "0.1.0"
    })))
}

// Database connection test endpoint
async fn db_test(data: web::Data<AppState>) -> Result<HttpResponse> {
    match database::test_connection(&data.session).await {
        Ok(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "message": "ScyllaDB connection successful"
        }))),
        Err(e) => {
            error!("Database connection failed: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "message": format!("Database connection failed: {}", e)
            })))
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Get environment variables
    let scylla_host = env::var("SCYLLA_HOST").unwrap_or_else(|_| "localhost".to_string());
    let scylla_port = env::var("SCYLLA_PORT")
        .unwrap_or_else(|_| "9042".to_string())
        .parse::<u16>()
        .expect("Invalid SCYLLA_PORT");
    let port = env::var("PORT")
        .unwrap_or_else(|_| "15004".to_string())
        .parse::<u16>()
        .expect("Invalid PORT");

    info!("Connecting to ScyllaDB at {}:{}", scylla_host, scylla_port);

    // Initialize ScyllaDB connection
    let session = SessionBuilder::new()
        .known_node(format!("{}:{}", scylla_host, scylla_port))
        .build()
        .await
        .expect("Failed to connect to ScyllaDB");

    let session = Arc::new(session);

    // Initialize database schema
    if let Err(e) = database::init_schema(&session).await {
        error!("Failed to initialize database schema: {}", e);
        std::process::exit(1);
    }

    info!("Database schema initialized successfully");

    let app_state = AppState {
        session: session.clone(),
    };

    info!("Starting Sentinel server on port {}", port);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .route("/health", web::get().to(health))
            .route("/db-test", web::get().to(db_test))
            .service(
                web::scope("/api/v1")
                    // TODO: Add Zanzibar API endpoints
            )
    })
    .bind(format!("0.0.0.0:{}", port))?
    .run()
    .await
}
