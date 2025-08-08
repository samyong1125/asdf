use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use crate::errors::{SentinelError, SentinelResult};
use crate::cache::Cache;

/// Zookie는 Zanzibar의 일관성 토큰으로 "new enemy problem"을 방지합니다.
/// 단순화된 구현: Google Spanner TrueTime 대신 단조 증가 타임스탬프 사용
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zookie {
    /// 단조 증가하는 마이크로초 타임스탬프
    pub timestamp_micros: i64,
    /// 추가 메타데이터 (선택적)
    pub metadata: Option<ZookieMetadata>,
}

/// Zookie 메타데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZookieMetadata {
    /// 생성된 노드 ID (분산 환경에서 사용)
    pub node_id: Option<String>,
    /// 트랜잭션 ID (선택적)
    pub transaction_id: Option<String>,
}

impl Zookie {
    /// 새로운 Zookie 생성 (현재 시간 기준)
    pub fn new() -> Self {
        Self {
            timestamp_micros: Utc::now().timestamp_micros(),
            metadata: None,
        }
    }
    
    /// 메타데이터와 함께 Zookie 생성
    pub fn with_metadata(metadata: ZookieMetadata) -> Self {
        Self {
            timestamp_micros: Utc::now().timestamp_micros(),
            metadata: Some(metadata),
        }
    }
    
    /// 특정 타임스탬프로 Zookie 생성
    pub fn from_timestamp(timestamp_micros: i64) -> Self {
        Self {
            timestamp_micros,
            metadata: None,
        }
    }
    
    /// Zookie를 문자열로 직렬화 (Base64 인코딩)
    pub fn to_string(&self) -> SentinelResult<String> {
        let json = serde_json::to_string(self)
            .map_err(|e| SentinelError::internal_error(format!("Failed to serialize zookie: {}", e)))?;
        
        Ok(BASE64_STANDARD.encode(json))
    }
    
    /// 문자열에서 Zookie 파싱 (Base64 디코딩)
    pub fn from_string(encoded: &str) -> SentinelResult<Self> {
        let json = BASE64_STANDARD.decode(encoded)
            .map_err(|e| SentinelError::validation_error(format!("Invalid zookie encoding: {}", e)))?;
        
        let json_str = String::from_utf8(json)
            .map_err(|e| SentinelError::validation_error(format!("Invalid zookie UTF-8: {}", e)))?;
        
        serde_json::from_str(&json_str)
            .map_err(|e| SentinelError::validation_error(format!("Invalid zookie format: {}", e)))
    }
    
    /// 두 Zookie의 시간 순서 비교
    /// Returns: -1 (this < other), 0 (this == other), 1 (this > other)
    pub fn compare_timestamp(&self, other: &Zookie) -> i8 {
        if self.timestamp_micros < other.timestamp_micros {
            -1
        } else if self.timestamp_micros > other.timestamp_micros {
            1
        } else {
            0
        }
    }
    
    /// 이 Zookie가 다른 Zookie보다 최신인지 확인
    pub fn is_newer_than(&self, other: &Zookie) -> bool {
        self.timestamp_micros > other.timestamp_micros
    }
    
    /// 이 Zookie가 다른 Zookie와 같거나 최신인지 확인  
    pub fn is_at_least(&self, other: &Zookie) -> bool {
        self.timestamp_micros >= other.timestamp_micros
    }
    
    /// DateTime으로 변환
    pub fn to_datetime(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_micros(self.timestamp_micros)
            .unwrap_or_else(|| Utc::now())
    }
}

impl Default for Zookie {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for Zookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.to_string() {
            Ok(encoded) => write!(f, "{}", encoded),
            Err(_) => write!(f, "zookie[{}]", self.timestamp_micros),
        }
    }
}

/// Zookie 관리자 - 전역 일관성 토큰 생성 및 검증
pub struct ZookieManager<C: Cache> {
    /// 단조 증가하는 타임스탬프 카운터
    last_timestamp: Arc<AtomicI64>,
    /// 캐시 (최근 Zookie 저장용)
    cache: Arc<C>,
    /// 노드 ID (분산 환경에서 사용)
    node_id: String,
}

impl<C: Cache> ZookieManager<C> {
    /// 새로운 ZookieManager 생성
    pub fn new(cache: Arc<C>, node_id: Option<String>) -> Self {
        let node_id = node_id.unwrap_or_else(|| {
            // 기본 노드 ID 생성 (단일 서버 환경)
            format!("sentinel-{}", 
                std::process::id())
        });
        
        Self {
            last_timestamp: Arc::new(AtomicI64::new(Utc::now().timestamp_micros())),
            cache,
            node_id,
        }
    }
    
    /// 새로운 Zookie 생성 (단조 증가 보장)
    pub async fn generate_zookie(&self) -> SentinelResult<Zookie> {
        let now_micros = Utc::now().timestamp_micros();
        
        // 단조 증가 보장을 위해 이전 타임스탬프와 비교
        let new_timestamp = self.last_timestamp.fetch_max(now_micros, Ordering::Relaxed).max(now_micros);
        
        // 동일한 마이크로초에 여러 요청이 오면 1씩 증가
        let final_timestamp = if new_timestamp == now_micros {
            now_micros
        } else {
            self.last_timestamp.fetch_add(1, Ordering::Relaxed) + 1
        };
        
        let zookie = Zookie::with_metadata(ZookieMetadata {
            node_id: Some(self.node_id.clone()),
            transaction_id: Some(uuid::Uuid::new_v4().to_string()),
        });
        
        // 최신 Zookie를 캐시에 저장 (스냅샷 읽기용)
        self.cache_latest_zookie(&zookie).await?;
        
        info!("Generated new zookie: {}", final_timestamp);
        Ok(zookie)
    }
    
    /// 요청의 Zookie 검증 및 스냅샷 읽기 시간 결정
    pub async fn validate_and_get_snapshot_time(
        &self, 
        request_zookie: Option<&str>
    ) -> SentinelResult<Zookie> {
        match request_zookie {
            Some(zookie_str) => {
                // 클라이언트가 제공한 Zookie 파싱
                let requested_zookie = Zookie::from_string(zookie_str)?;
                
                // 현재 시간과 비교하여 미래의 Zookie인지 확인
                let now = Utc::now().timestamp_micros();
                if requested_zookie.timestamp_micros > now {
                    warn!("Received future zookie: {} > {}", requested_zookie.timestamp_micros, now);
                    return Err(SentinelError::validation_error("Future zookie not allowed"));
                }
                
                // 너무 오래된 Zookie인지 확인 (예: 1시간 이상)
                let max_age_micros = 60 * 60 * 1_000_000; // 1시간
                if now - requested_zookie.timestamp_micros > max_age_micros {
                    warn!("Received stale zookie: age = {} seconds", 
                        (now - requested_zookie.timestamp_micros) / 1_000_000);
                    return Err(SentinelError::validation_error("Stale zookie"));
                }
                
                info!("Using client zookie for snapshot read: {}", requested_zookie.timestamp_micros);
                Ok(requested_zookie)
            }
            None => {
                // Zookie가 없으면 현재 시간으로 새로 생성
                info!("No client zookie, using current time for snapshot");
                self.generate_zookie().await
            }
        }
    }
    
    /// 최신 Zookie를 캐시에 저장
    async fn cache_latest_zookie(&self, zookie: &Zookie) -> SentinelResult<()> {
        let cache_key = "zookie:latest";
        let zookie_str = zookie.to_string()?;
        
        // 캐시 TTL: 1시간
        self.cache.set(cache_key, &zookie_str, 60 * 60).await?;
        
        Ok(())
    }
    
    /// 캐시에서 최신 Zookie 조회
    pub async fn get_latest_cached_zookie(&self) -> SentinelResult<Option<Zookie>> {
        let cache_key = "zookie:latest";
        
        match self.cache.get(cache_key).await? {
            Some(zookie_str) => {
                match Zookie::from_string(&zookie_str) {
                    Ok(zookie) => Ok(Some(zookie)),
                    Err(e) => {
                        warn!("Failed to parse cached zookie: {}", e);
                        Ok(None)
                    }
                }
            }
            None => Ok(None),
        }
    }
    
    /// 특정 시점의 스냅샷 읽기를 위한 Zookie 생성
    pub fn create_snapshot_zookie(&self, timestamp_micros: i64) -> Zookie {
        Zookie::from_timestamp(timestamp_micros)
    }
    
    /// "New Enemy Problem" 방지 검증
    /// 권한 변경 후 바로 권한 체크할 때 일관성 보장
    pub async fn ensure_consistency_after_write(
        &self,
        write_zookie: &Zookie,
        read_zookie: Option<&Zookie>,
    ) -> SentinelResult<bool> {
        match read_zookie {
            Some(read_zookie) => {
                // 읽기 Zookie가 쓰기 Zookie보다 최신이거나 같아야 함
                if read_zookie.is_at_least(write_zookie) {
                    info!("Consistency check passed: read_zookie >= write_zookie");
                    Ok(true)
                } else {
                    warn!(
                        "Consistency check failed: read_zookie ({}) < write_zookie ({})", 
                        read_zookie.timestamp_micros, 
                        write_zookie.timestamp_micros
                    );
                    Ok(false)
                }
            }
            None => {
                // 읽기 Zookie가 없으면 현재 시간이 쓰기 시간보다 최신인지 확인
                let now_micros = Utc::now().timestamp_micros();
                Ok(now_micros >= write_zookie.timestamp_micros)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;
    
    // 테스트용 간단한 캐시 구현
    struct MockCache {
        data: Arc<Mutex<HashMap<String, (String, i64)>>>,
    }
    
    impl MockCache {
        fn new() -> Self {
            Self {
                data: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }
    
    #[async_trait::async_trait]
    impl Cache for MockCache {
        async fn get(&self, key: &str) -> SentinelResult<Option<String>> {
            let data = self.data.lock().unwrap();
            if let Some((value, expiry)) = data.get(key) {
                if Utc::now().timestamp() < *expiry {
                    Ok(Some(value.clone()))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }
        
        async fn set(&self, key: &str, value: &str, ttl_seconds: u64) -> SentinelResult<()> {
            let expiry = Utc::now().timestamp() + ttl_seconds as i64;
            let mut data = self.data.lock().unwrap();
            data.insert(key.to_string(), (value.to_string(), expiry));
            Ok(())
        }
        
        async fn delete(&self, _key: &str) -> SentinelResult<()> {
            Ok(())
        }
        
        async fn delete_pattern(&self, _pattern: &str) -> SentinelResult<()> {
            Ok(())
        }
        
        async fn ping(&self) -> SentinelResult<()> {
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_zookie_serialization() {
        let zookie = Zookie::new();
        let encoded = zookie.to_string().unwrap();
        let decoded = Zookie::from_string(&encoded).unwrap();
        
        assert_eq!(zookie.timestamp_micros, decoded.timestamp_micros);
    }
    
    #[tokio::test]
    async fn test_zookie_comparison() {
        let zookie1 = Zookie::from_timestamp(1000);
        let zookie2 = Zookie::from_timestamp(2000);
        
        assert_eq!(zookie1.compare_timestamp(&zookie2), -1);
        assert_eq!(zookie2.compare_timestamp(&zookie1), 1);
        assert_eq!(zookie1.compare_timestamp(&zookie1), 0);
        
        assert!(!zookie1.is_newer_than(&zookie2));
        assert!(zookie2.is_newer_than(&zookie1));
        assert!(zookie2.is_at_least(&zookie1));
    }
    
    #[tokio::test]
    async fn test_zookie_manager() {
        let cache = Arc::new(MockCache::new());
        let manager = ZookieManager::new(cache, Some("test-node".to_string()));
        
        let zookie1 = manager.generate_zookie().await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        let zookie2 = manager.generate_zookie().await.unwrap();
        
        assert!(zookie2.is_newer_than(&zookie1));
        
        // 캐시된 최신 Zookie 확인
        let cached = manager.get_latest_cached_zookie().await.unwrap();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().timestamp_micros, zookie2.timestamp_micros);
    }
    
    #[tokio::test]
    async fn test_consistency_check() {
        let cache = Arc::new(MockCache::new());
        let manager = ZookieManager::new(cache, None);
        
        let write_zookie = Zookie::from_timestamp(1000);
        let read_zookie_old = Zookie::from_timestamp(500);
        let read_zookie_new = Zookie::from_timestamp(1500);
        
        // 읽기 Zookie가 쓰기 Zookie보다 오래된 경우 - 실패
        let result = manager.ensure_consistency_after_write(&write_zookie, Some(&read_zookie_old)).await.unwrap();
        assert!(!result);
        
        // 읽기 Zookie가 쓰기 Zookie보다 최신인 경우 - 성공
        let result = manager.ensure_consistency_after_write(&write_zookie, Some(&read_zookie_new)).await.unwrap();
        assert!(result);
    }
}