use actix_web::{web, HttpResponse, Result};
use std::sync::Arc;
use tracing::{info, error};
use chrono::Utc;

use crate::models::{
    CheckRequest, WriteRequest, WriteResponse, ReadRequest, ReadResponse,
    RelationTuple, Operation, BatchCheckRequest
};
use crate::zookie::Zookie;
use crate::permission_checker::PermissionChecker;
use crate::tuple_store::{TupleStore, ScyllaTupleStore};
use crate::cache::{Cache, RedisCache};
use crate::AppState;

/// Zanzibar Check API - 권한 검증 (캐싱 포함)
/// POST /api/v1/check
pub async fn check_permission(
    data: web::Data<AppState>,
    req: web::Json<CheckRequest>,
) -> Result<HttpResponse> {
    info!("Permission check request: {}:{}#{} for user:{}", 
        req.namespace, req.object_id, req.relation, req.user_id);

    let tuple_store = Arc::new(ScyllaTupleStore::new(data.session.clone()));
    let checker = PermissionChecker::new(tuple_store, data.cache.clone(), data.zookie_manager.clone());

    match checker.check_permission(&req).await {
        Ok(response) => {
            info!("Permission check result: allowed={}", response.allowed);
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            error!("Permission check failed: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Permission check failed",
                "message": e.to_string()
            })))
        }
    }
}

/// Zanzibar Write API - 권한 튜플 생성/삭제 (캐시 무효화 포함)
/// POST /api/v1/write
pub async fn write_permissions(
    data: web::Data<AppState>,
    req: web::Json<WriteRequest>,
) -> Result<HttpResponse> {
    info!("Write request with {} tuple updates", req.updates.len());

    let tuple_store = Arc::new(ScyllaTupleStore::new(data.session.clone()));
    let checker = PermissionChecker::new(tuple_store.clone(), data.cache.clone(), data.zookie_manager.clone());

    let mut success_count = 0;
    let mut errors = Vec::new();
    let mut affected_objects = std::collections::HashSet::new();
    let mut affected_users = std::collections::HashSet::new();

    for update in &req.updates {
        let tuple = RelationTuple {
            namespace: update.tuple.namespace.clone(),
            object_id: update.tuple.object_id.clone(),
            relation: update.tuple.relation.clone(),
            user_type: update.tuple.user_type.clone(),
            user_id: update.tuple.user_id.clone(),
            created_at: scylla::value::CqlTimestamp(Utc::now().timestamp_millis()),
        };

        let result = match update.operation {
            Operation::Insert => {
                info!("Inserting tuple: {}:{}#{}@{}:{}", 
                    tuple.namespace, tuple.object_id, tuple.relation, tuple.user_type, tuple.user_id);
                tuple_store.insert_tuple(&tuple).await
            }
            Operation::Delete => {
                info!("Deleting tuple: {}:{}#{}@{}:{}", 
                    tuple.namespace, tuple.object_id, tuple.relation, tuple.user_type, tuple.user_id);
                tuple_store.delete_tuple(&tuple).await
            }
        };

        match result {
            Ok(_) => {
                success_count += 1;
                // 캐시 무효화를 위해 영향받은 객체와 사용자 추적
                affected_objects.insert((tuple.namespace.clone(), tuple.object_id.clone()));
                if tuple.user_type == "user" {
                    affected_users.insert(tuple.user_id.clone());
                }
            }
            Err(e) => {
                error!("Tuple operation failed: {}", e);
                errors.push(e.to_string());
            }
        }
    }

    // 성공한 작업이 있으면 관련 캐시 무효화
    if success_count > 0 {
        // 객체별 캐시 무효화
        for (namespace, object_id) in affected_objects {
            if let Err(e) = checker.invalidate_object_cache(&namespace, &object_id).await {
                error!("Failed to invalidate object cache for {}:{}: {}", namespace, object_id, e);
            }
        }
        
        // 사용자별 캐시 무효화
        for user_id in affected_users {
            if let Err(e) = checker.invalidate_user_cache(&user_id).await {
                error!("Failed to invalidate user cache for {}: {}", user_id, e);
            }
        }
    }

    // 새로운 쓰기 Zookie 생성
    let write_zookie = data.zookie_manager.generate_zookie().await.map_err(|e| {
        error!("Failed to generate write zookie: {}", e);
        e
    })?;
    
    let response = WriteResponse {
        zookie: write_zookie.to_string().unwrap_or_else(|_| format!("{}", Utc::now().timestamp_millis())),
    };

    if errors.is_empty() {
        info!("Write request completed: {} operations successful", success_count);
        Ok(HttpResponse::Ok().json(response))
    } else {
        error!("Write request partially failed: {} errors", errors.len());
        Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Some operations failed",
            "successful_count": success_count,
            "errors": errors,
            "response": response
        })))
    }
}

/// Zanzibar Read API - 권한 튜플 조회
/// POST /api/v1/read
pub async fn read_permissions(
    data: web::Data<AppState>,
    req: web::Json<ReadRequest>,
) -> Result<HttpResponse> {
    info!("Read request for filter: {:?}", req.tuple_filter);

    let tuple_store = Arc::new(ScyllaTupleStore::new(data.session.clone()));

    // 필터에 따른 조회 로직
    let tuples_result = if let (Some(namespace), Some(object_id)) = (
        &req.tuple_filter.namespace,
        &req.tuple_filter.object_id,
    ) {
        if let Some(relation) = &req.tuple_filter.relation {
            // 특정 객체-관계에 대한 튜플 조회
            tuple_store.find_tuples_by_object_relation(namespace, object_id, relation).await
        } else {
            // 특정 객체에 대한 모든 튜플 조회
            tuple_store.find_tuples_by_object(namespace, object_id).await
        }
    } else if let Some(user_id) = &req.tuple_filter.user_id {
        // 특정 사용자의 모든 권한 조회
        tuple_store.find_user_memberships(user_id).await
    } else {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid filter",
            "message": "Must specify either (namespace, object_id) or user_id"
        })));
    };

    match tuples_result {
        Ok(tuples) => {
            info!("Read request completed: {} tuples found", tuples.len());
            
            let api_tuples = tuples.iter().map(|t| t.to_api_tuple()).collect::<Vec<_>>();
            
            // 읽기 Zookie 생성
            let read_zookie = data.zookie_manager.generate_zookie().await.unwrap_or_else(|_| Zookie::new());
            
            let response = ReadResponse {
                tuples: api_tuples,
                next_page_token: None, // TODO: 페이징 구현
                zookie: read_zookie.to_string().unwrap_or_else(|_| format!("{}", Utc::now().timestamp_millis())),
            };
            
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            error!("Read request failed: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Read failed",
                "message": e.to_string()
            })))
        }
    }
}

/// 사용자 권한 조회 (디버깅용)
/// GET /api/v1/users/{user_id}/permissions
pub async fn get_user_permissions(
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let user_id = path.into_inner();
    info!("Getting permissions for user: {}", user_id);

    let tuple_store = Arc::new(ScyllaTupleStore::new(data.session.clone()));
    let checker = PermissionChecker::new(tuple_store, data.cache.clone(), data.zookie_manager.clone());

    match checker.get_user_permissions(&user_id).await {
        Ok(permissions) => {
            info!("Found {} permissions for user {}", permissions.len(), user_id);
            
            let api_permissions = permissions.iter().map(|p| p.to_api_tuple()).collect::<Vec<_>>();
            
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "user_id": user_id,
                "permissions": api_permissions,
                "count": api_permissions.len()
            })))
        }
        Err(e) => {
            error!("Failed to get user permissions: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get user permissions",
                "message": e.to_string()
            })))
        }
    }
}

/// 객체 권한 조회 (디버깅용)
/// GET /api/v1/objects/{namespace}/{object_id}/permissions
pub async fn get_object_permissions(
    data: web::Data<AppState>,
    path: web::Path<(String, String)>,
) -> Result<HttpResponse> {
    let (namespace, object_id) = path.into_inner();
    info!("Getting permissions for object: {}:{}", namespace, object_id);

    let tuple_store = Arc::new(ScyllaTupleStore::new(data.session.clone()));
    let checker = PermissionChecker::new(tuple_store, data.cache.clone(), data.zookie_manager.clone());

    match checker.get_object_permissions(&namespace, &object_id).await {
        Ok(permissions) => {
            info!("Found {} permissions for object {}:{}", permissions.len(), namespace, object_id);
            
            let api_permissions = permissions.iter().map(|p| p.to_api_tuple()).collect::<Vec<_>>();
            
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "namespace": namespace,
                "object_id": object_id,
                "permissions": api_permissions,
                "count": api_permissions.len()
            })))
        }
        Err(e) => {
            error!("Failed to get object permissions: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get object permissions",
                "message": e.to_string()
            })))
        }
    }
}

/// Zanzibar 배치 권한 체크 API - 여러 권한을 한 번에 검증 (병렬 처리)
/// POST /api/v1/batch_check
pub async fn batch_check_permissions(
    data: web::Data<AppState>,
    req: web::Json<BatchCheckRequest>,
) -> Result<HttpResponse> {
    info!("Batch permission check request with {} items", req.checks.len());

    let tuple_store = Arc::new(ScyllaTupleStore::new(data.session.clone()));
    let checker = PermissionChecker::new(tuple_store, data.cache.clone(), data.zookie_manager.clone());

    match checker.batch_check_permissions(&req).await {
        Ok(response) => {
            info!(
                "Batch permission check result: {}/{} allowed", 
                response.allowed_count,
                response.total_requests
            );
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            error!("Batch permission check failed: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Batch permission check failed",
                "message": e.to_string()
            })))
        }
    }
}