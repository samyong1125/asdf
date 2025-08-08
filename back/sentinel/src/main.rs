use actix_web::{web, App, HttpResponse, HttpServer, Result};
use actix_cors::Cors;
use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;
use redis::Client as RedisClient;
use std::env;
use std::sync::Arc;
use tracing::{info, error};
use cache::Cache;
use zookie::ZookieManager;

mod database;
mod errors;
mod models;
mod tuple_store;
mod permission_hierarchy;
mod permission_checker;
mod api_handlers;
mod cache;
mod zookie;

// App State to hold database connections
#[derive(Clone)]
pub struct AppState {
    pub session: Arc<Session>,
    pub redis: Arc<RedisClient>,
    pub cache: Arc<cache::RedisCache>,
    pub zookie_manager: Arc<ZookieManager<cache::RedisCache>>,
}

// Health check endpoint
async fn health() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "sentinel",
        "version": "0.1.0"
    })))
}

// ScyllaDB connection test endpoint
async fn scylla_test(data: web::Data<AppState>) -> Result<HttpResponse> {
    match database::test_scylla_connection(&data.session).await {
        Ok(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "message": "ScyllaDB connection successful"
        }))),
        Err(e) => {
            error!("ScyllaDB connection failed: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "message": format!("ScyllaDB connection failed: {}", e)
            })))
        }
    }
}

// Redis connection test endpoint
async fn redis_test(data: web::Data<AppState>) -> Result<HttpResponse> {
    match database::test_redis_connection(&data.redis).await {
        Ok(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "message": "Redis connection successful"
        }))),
        Err(e) => {
            error!("Redis connection failed: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "message": format!("Redis connection failed: {}", e)
            })))
        }
    }
}

// Cache test endpoint
async fn cache_test(data: web::Data<AppState>) -> Result<HttpResponse> {
    match data.cache.ping().await {
        Ok(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "message": "Cache connection successful"
        }))),
        Err(e) => {
            error!("Cache connection failed: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error", 
                "message": format!("Cache connection failed: {}", e)
            })))
        }
    }
}

// All databases connection test endpoint
async fn db_test(data: web::Data<AppState>) -> Result<HttpResponse> {
    let scylla_result = database::test_scylla_connection(&data.session).await;
    let redis_result = database::test_redis_connection(&data.redis).await;
    let cache_result = data.cache.ping().await;
    
    match (scylla_result, redis_result, cache_result) {
        (Ok(_), Ok(_), Ok(_)) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "status": "ok",
            "message": "All database connections successful",
            "scylla": "ok",
            "redis": "ok",
            "cache": "ok"
        }))),
        (scylla_err, redis_err, cache_err) => {
            let mut errors = Vec::new();
            if let Err(e) = scylla_err {
                errors.push(format!("ScyllaDB: {}", e));
            }
            if let Err(e) = redis_err {
                errors.push(format!("Redis: {}", e));
            }
            if let Err(e) = cache_err {
                errors.push(format!("Cache: {}", e));
            }
            
            error!("Database connection errors: {:?}", errors);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "message": "Some database connections failed",
                "errors": errors
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
    let redis_host = env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".to_string());
    let redis_port = env::var("REDIS_PORT")
        .unwrap_or_else(|_| "50006".to_string())
        .parse::<u16>()
        .expect("Invalid REDIS_PORT");
    let port = env::var("PORT")
        .unwrap_or_else(|_| "15004".to_string())
        .parse::<u16>()
        .expect("Invalid PORT");

    info!("Connecting to ScyllaDB at {}:{}", scylla_host, scylla_port);
    info!("Connecting to Redis at {}:{}", redis_host, redis_port);

    // Initialize ScyllaDB connection
    let session = SessionBuilder::new()
        .known_node(format!("{}:{}", scylla_host, scylla_port))
        .build()
        .await
        .expect("Failed to connect to ScyllaDB");
    let session = Arc::new(session);

    // Initialize Redis connection
    let redis = database::init_redis(&redis_host, redis_port)
        .await
        .expect("Failed to connect to Redis");
    let redis = Arc::new(redis);

    // Initialize database schema
    if let Err(e) = database::init_schema(&session).await {
        error!("Failed to initialize database schema: {}", e);
        std::process::exit(1);
    }

    info!("Database schema initialized successfully");

    // Initialize cache
    let cache = Arc::new(cache::RedisCache::new(redis.clone()));
    
    // Initialize Zookie manager
    let node_id = env::var("NODE_ID").ok();
    let zookie_manager = Arc::new(ZookieManager::new(cache.clone(), node_id));
    
    let app_state = AppState {
        session: session.clone(),
        redis: redis.clone(),
        cache: cache.clone(),
        zookie_manager,
    };

    info!("Starting Sentinel server on port {}", port);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(app_state.clone()))
            .route("/health", web::get().to(health))
            .route("/db-test", web::get().to(db_test))
            .route("/scylla-test", web::get().to(scylla_test))
            .route("/redis-test", web::get().to(redis_test))
            .route("/cache-test", web::get().to(cache_test))
            .service(
                web::scope("/api/v1")
                    // Zanzibar Core API
                    .route("/check", web::post().to(api_handlers::check_permission))
                    .route("/write", web::post().to(api_handlers::write_permissions))
                    .route("/read", web::post().to(api_handlers::read_permissions))
                    .route("/batch_check", web::post().to(api_handlers::batch_check_permissions))
                    
                    // Debug/Utility APIs
                    .route("/users/{user_id}/permissions", web::get().to(api_handlers::get_user_permissions))
                    .route("/objects/{namespace}/{object_id}/permissions", web::get().to(api_handlers::get_object_permissions))
            )
    })
    .bind(format!("0.0.0.0:{}", port))?
    .run()
    .await
}
