use std::sync::Arc;
use redis::{Client as RedisClient, AsyncCommands};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use crate::errors::{SentinelError, SentinelResult};
use crate::models::{CheckRequest, CheckResponse};

/// 캐시 추상화 trait
/// 권한 체크 결과와 관련 메타데이터를 캐싱
#[async_trait::async_trait]
pub trait Cache: Send + Sync {
    /// 캐시에서 값 조회
    async fn get(&self, key: &str) -> SentinelResult<Option<String>>;
    
    /// 캐시에 값 저장 (TTL 포함)
    async fn set(&self, key: &str, value: &str, ttl_seconds: u64) -> SentinelResult<()>;
    
    /// 캐시에서 키 삭제
    async fn delete(&self, key: &str) -> SentinelResult<()>;
    
    /// 패턴에 일치하는 키들 일괄 삭제
    async fn delete_pattern(&self, pattern: &str) -> SentinelResult<()>;
    
    /// 캐시 연결 상태 확인
    async fn ping(&self) -> SentinelResult<()>;
}

/// Redis 기반 캐시 구현체
pub struct RedisCache {
    client: Arc<RedisClient>,
}

impl RedisCache {
    /// 새로운 RedisCache 생성
    pub fn new(client: Arc<RedisClient>) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl Cache for RedisCache {
    /// 캐시에서 값 조회
    async fn get(&self, key: &str) -> SentinelResult<Option<String>> {
        match self.client.get_multiplexed_async_connection().await {
            Ok(mut conn) => {
                match conn.get::<&str, Option<String>>(key).await {
                    Ok(value) => {
                        if value.is_some() {
                            info!("Cache hit for key: {}", key);
                        }
                        Ok(value)
                    }
                    Err(e) => {
                        warn!("Cache get failed for key {}: {}", key, e);
                        Err(SentinelError::from_redis_error(e, "Cache get failed"))
                    }
                }
            }
            Err(e) => {
                error!("Redis connection failed: {}", e);
                Err(SentinelError::from_redis_error(e, "Redis connection failed"))
            }
        }
    }
    
    /// 캐시에 값 저장 (TTL 포함)
    async fn set(&self, key: &str, value: &str, ttl_seconds: u64) -> SentinelResult<()> {
        match self.client.get_multiplexed_async_connection().await {
            Ok(mut conn) => {
                match conn.set_ex::<&str, &str, ()>(key, value, ttl_seconds).await {
                    Ok(_) => {
                        info!("Cache set for key: {} (TTL: {}s)", key, ttl_seconds);
                        Ok(())
                    }
                    Err(e) => {
                        warn!("Cache set failed for key {}: {}", key, e);
                        Err(SentinelError::from_redis_error(e, "Cache set failed"))
                    }
                }
            }
            Err(e) => {
                error!("Redis connection failed: {}", e);
                Err(SentinelError::from_redis_error(e, "Redis connection failed"))
            }
        }
    }
    
    /// 캐시에서 키 삭제
    async fn delete(&self, key: &str) -> SentinelResult<()> {
        match self.client.get_multiplexed_async_connection().await {
            Ok(mut conn) => {
                match conn.del::<&str, u64>(key).await {
                    Ok(deleted_count) => {
                        info!("Cache delete for key: {} (deleted: {})", key, deleted_count);
                        Ok(())
                    }
                    Err(e) => {
                        warn!("Cache delete failed for key {}: {}", key, e);
                        Err(SentinelError::from_redis_error(e, "Cache delete failed"))
                    }
                }
            }
            Err(e) => {
                error!("Redis connection failed: {}", e);
                Err(SentinelError::from_redis_error(e, "Redis connection failed"))
            }
        }
    }
    
    /// 패턴에 일치하는 키들 일괄 삭제
    async fn delete_pattern(&self, pattern: &str) -> SentinelResult<()> {
        match self.client.get_multiplexed_async_connection().await {
            Ok(mut conn) => {
                // KEYS 명령으로 패턴에 일치하는 키들 찾기
                match conn.keys::<&str, Vec<String>>(pattern).await {
                    Ok(keys) => {
                        if keys.is_empty() {
                            info!("No keys found for pattern: {}", pattern);
                            return Ok(());
                        }
                        
                        // 찾은 키들 일괄 삭제
                        match conn.del::<Vec<String>, u64>(keys.clone()).await {
                            Ok(deleted_count) => {
                                info!("Cache delete pattern: {} (deleted: {} keys)", pattern, deleted_count);
                                Ok(())
                            }
                            Err(e) => {
                                warn!("Cache pattern delete failed for pattern {}: {}", pattern, e);
                                Err(SentinelError::from_redis_error(e, "Cache pattern delete failed"))
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Cache keys lookup failed for pattern {}: {}", pattern, e);
                        Err(SentinelError::from_redis_error(e, "Cache keys lookup failed"))
                    }
                }
            }
            Err(e) => {
                error!("Redis connection failed: {}", e);
                Err(SentinelError::from_redis_error(e, "Redis connection failed"))
            }
        }
    }
    
    /// 캐시 연결 상태 확인
    async fn ping(&self) -> SentinelResult<()> {
        match self.client.get_multiplexed_async_connection().await {
            Ok(mut conn) => {
                match conn.ping::<String>().await {
                    Ok(_) => {
                        info!("Cache ping successful");
                        Ok(())
                    }
                    Err(e) => {
                        error!("Cache ping failed: {}", e);
                        Err(SentinelError::from_redis_error(e, "Cache ping failed"))
                    }
                }
            }
            Err(e) => {
                error!("Redis connection failed: {}", e);
                Err(SentinelError::from_redis_error(e, "Redis connection failed"))
            }
        }
    }
}

/// 권한 체크 결과를 캐싱하기 위한 구조체
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedCheckResult {
    /// 권한 허용 여부
    pub allowed: bool,
    /// 캐시된 시간의 타임스탬프
    pub cached_at: i64,
    /// 원본 zookie (일관성 보장용)
    pub original_zookie: String,
}

impl CachedCheckResult {
    /// CheckResponse로부터 CachedCheckResult 생성
    pub fn from_check_response(response: &CheckResponse) -> Self {
        Self {
            allowed: response.allowed,
            cached_at: chrono::Utc::now().timestamp_millis(),
            original_zookie: response.zookie.clone(),
        }
    }
    
    /// CachedCheckResult를 CheckResponse로 변환
    pub fn to_check_response(&self, current_zookie: &str) -> CheckResponse {
        CheckResponse {
            allowed: self.allowed,
            // 현재 요청의 zookie 사용 (일관성 보장)
            zookie: current_zookie.to_string(),
        }
    }
    
    /// JSON 문자열로 직렬화
    pub fn to_json(&self) -> SentinelResult<String> {
        serde_json::to_string(self)
            .map_err(|e| SentinelError::internal_error(format!("Failed to serialize cached result: {}", e)))
    }
    
    /// JSON 문자열에서 역직렬화
    pub fn from_json(json: &str) -> SentinelResult<Self> {
        serde_json::from_str(json)
            .map_err(|e| SentinelError::internal_error(format!("Failed to deserialize cached result: {}", e)))
    }
}

/// 캐시 키 생성 유틸리티
pub struct CacheKeyBuilder;

impl CacheKeyBuilder {
    /// 권한 체크 캐시 키 생성
    /// 형식: "check:{namespace}:{object_id}#{relation}@{user_type}:{user_id}"
    pub fn check_permission_key(request: &CheckRequest) -> String {
        let user_type = request.user_type.as_deref().unwrap_or("user");
        format!(
            "check:{}:{}#{}@{}:{}",
            request.namespace, 
            request.object_id, 
            request.relation, 
            user_type, 
            request.user_id
        )
    }
    
    /// 사용자의 모든 권한 캐시 무효화를 위한 패턴
    /// 형식: "check:*@user:{user_id}"
    pub fn user_permission_pattern(user_id: &str) -> String {
        format!("check:*@user:{}", user_id)
    }
    
    /// 객체의 모든 권한 캐시 무효화를 위한 패턴
    /// 형식: "check:{namespace}:{object_id}*"
    pub fn object_permission_pattern(namespace: &str, object_id: &str) -> String {
        format!("check:{}:{}*", namespace, object_id)
    }
    
    /// 네임스페이스의 모든 권한 캐시 무효화를 위한 패턴
    /// 형식: "check:{namespace}:*"
    pub fn namespace_permission_pattern(namespace: &str) -> String {
        format!("check:{}:*", namespace)
    }
}

/// 캐시 TTL 상수
pub struct CacheTTL;

impl CacheTTL {
    /// 권한 체크 결과 캐시 TTL (5분)
    pub const PERMISSION_CHECK: u64 = 5 * 60; // 300초
    
    /// 사용자 권한 목록 캐시 TTL (10분)
    pub const USER_PERMISSIONS: u64 = 10 * 60; // 600초
    
    /// 객체 권한 목록 캐시 TTL (10분)
    pub const OBJECT_PERMISSIONS: u64 = 10 * 60; // 600초
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cache_key_generation() {
        let request = CheckRequest {
            namespace: "documents".to_string(),
            object_id: "doc123".to_string(),
            relation: "viewer".to_string(),
            user_id: "alice".to_string(),
            user_type: Some("user".to_string()),
            zookie: None,
        };
        
        let key = CacheKeyBuilder::check_permission_key(&request);
        assert_eq!(key, "check:documents:doc123#viewer@user:alice");
        
        let user_pattern = CacheKeyBuilder::user_permission_pattern("alice");
        assert_eq!(user_pattern, "check:*@user:alice");
        
        let object_pattern = CacheKeyBuilder::object_permission_pattern("documents", "doc123");
        assert_eq!(object_pattern, "check:documents:doc123*");
    }
    
    #[test]
    fn test_cached_check_result_serialization() {
        let response = CheckResponse {
            allowed: true,
            zookie: "1234567890".to_string(),
        };
        
        let cached = CachedCheckResult::from_check_response(&response);
        let json = cached.to_json().unwrap();
        let deserialized = CachedCheckResult::from_json(&json).unwrap();
        
        assert_eq!(cached.allowed, deserialized.allowed);
        assert_eq!(cached.original_zookie, deserialized.original_zookie);
    }
}