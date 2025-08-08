use std::sync::Arc;
use std::collections::HashSet;
use async_recursion::async_recursion;
use tracing::{info, warn};
use crate::models::{RelationTuple, CheckRequest, CheckResponse, BatchCheckRequest, BatchCheckResponse, BatchCheckItem};
use crate::tuple_store::{TupleStore, ScyllaTupleStore};
use crate::permission_hierarchy::{PermissionHierarchy, PermissionCheckResult};
use crate::cache::{Cache, CachedCheckResult, CacheKeyBuilder, CacheTTL};
use crate::zookie::{Zookie, ZookieManager};
use crate::errors::SentinelResult;

/// Zanzibar 권한 검증 엔진
/// 직접 권한, userset 재귀 확인, 권한 상속을 처리
pub struct PermissionChecker<C: Cache> {
    tuple_store: Arc<ScyllaTupleStore>,
    hierarchy: PermissionHierarchy,
    cache: Arc<C>,
    zookie_manager: Arc<ZookieManager<C>>,
}

impl<C: Cache> PermissionChecker<C> {
    /// 새로운 PermissionChecker 생성 (캐시 포함)
    pub fn new(tuple_store: Arc<ScyllaTupleStore>, cache: Arc<C>, zookie_manager: Arc<ZookieManager<C>>) -> Self {
        Self {
            tuple_store,
            hierarchy: PermissionHierarchy::new(),
            cache,
            zookie_manager,
        }
    }

    /// 권한 검증 메인 함수 (캐싱 포함)
    pub async fn check_permission(&self, request: &CheckRequest) -> SentinelResult<CheckResponse> {
        // 1. Zookie 검증 및 스냅샷 읽기 시간 결정
        let snapshot_zookie = self.zookie_manager
            .validate_and_get_snapshot_time(request.zookie.as_deref())
            .await?;
            
        // 2. 캐시에서 먼저 확인
        let cache_key = CacheKeyBuilder::check_permission_key(request);
        
        match self.cache.get(&cache_key).await {
            Ok(Some(cached_json)) => {
                match CachedCheckResult::from_json(&cached_json) {
                    Ok(cached_result) => {
                        info!("Cache hit for permission check: {}", cache_key);
                        return Ok(cached_result.to_check_response(&snapshot_zookie.to_string()?));
                    }
                    Err(e) => {
                        warn!("Failed to deserialize cached result: {}, proceeding without cache", e);
                    }
                }
            }
            Ok(None) => {
                info!("Cache miss for permission check: {}", cache_key);
            }
            Err(e) => {
                warn!("Cache lookup failed: {}, proceeding without cache", e);
            }
        }
        
        // 3. 캐시 미스 또는 에러 시 실제 권한 검증 수행
        let response = self.check_permission_uncached(request, &snapshot_zookie).await?;
        
        // 3. 결과를 캐시에 저장 (비동기, 실패해도 응답에는 영향 없음)
        let cached_result = CachedCheckResult::from_check_response(&response);
        if let Ok(cached_json) = cached_result.to_json() {
            if let Err(e) = self.cache.set(&cache_key, &cached_json, CacheTTL::PERMISSION_CHECK).await {
                warn!("Failed to cache permission result: {}", e);
            }
        }
        
        Ok(response)
    }

    /// 배치 권한 검증 (병렬 처리 + 캐시 최적화)
    pub async fn batch_check_permissions(&self, request: &BatchCheckRequest) -> SentinelResult<BatchCheckResponse> {
        // 1. Zookie 검증 및 스냅샷 읽기 시간 결정
        let snapshot_zookie = self.zookie_manager
            .validate_and_get_snapshot_time(request.zookie.as_deref())
            .await?;
        use futures::future::join_all;
        use std::collections::HashMap;
        
        info!("Starting batch permission check for {} requests", request.checks.len());
        
        // 중복 요청 제거 및 인덱스 매핑
        let mut unique_requests: HashMap<String, Vec<usize>> = HashMap::new();
        let mut request_details: Vec<String> = Vec::new();
        
        for (index, check_request) in request.checks.iter().enumerate() {
            let cache_key = CacheKeyBuilder::check_permission_key(check_request);
            let request_info = format!(
                "{}:{}#{}@{}", 
                check_request.namespace,
                check_request.object_id, 
                check_request.relation,
                check_request.user_id
            );
            
            request_details.push(request_info);
            unique_requests.entry(cache_key).or_insert_with(Vec::new).push(index);
        }
        
        info!("Deduplicated {} requests to {} unique requests", 
              request.checks.len(), unique_requests.len());
        
        // 유니크한 요청들만 병렬로 실행
        let check_futures = unique_requests.iter().map(|(cache_key, indices)| {
            let checker = self;
            let first_index = indices[0];
            let check_request = &request.checks[first_index];
            let request_info = request_details[first_index].clone();
            let indices = indices.clone();
            
            async move {
                let result = checker.check_permission(check_request).await;
                
                match result {
                    Ok(response) => {
                        // 동일한 요청의 모든 인덱스에 동일한 결과 적용
                        indices.into_iter().map(|index| BatchCheckItem {
                            request_index: index,
                            allowed: response.allowed,
                            request_info: request_info.clone(),
                        }).collect::<Vec<_>>()
                    },
                    Err(_) => {
                        // 에러 시 모든 관련 인덱스를 거부로 처리
                        indices.into_iter().map(|index| BatchCheckItem {
                            request_index: index,
                            allowed: false,
                            request_info: format!("{} (ERROR)", request_info),
                        }).collect::<Vec<_>>()
                    }
                }
            }
        });
        
        // 모든 Future를 병렬 실행
        let results_groups = join_all(check_futures).await;
        
        // 결과를 평면화하고 원래 순서대로 정렬
        let mut all_results: Vec<BatchCheckItem> = results_groups.into_iter().flatten().collect();
        all_results.sort_by_key(|item| item.request_index);
        
        let mut response = BatchCheckResponse::new(all_results);
        response.zookie = snapshot_zookie.to_string()?;
        
        info!(
            "Batch permission check completed: {}/{} allowed ({}% cache efficiency)", 
            response.allowed_count,
            response.total_requests,
            ((request.checks.len() - unique_requests.len()) as f32 / request.checks.len() as f32 * 100.0) as u32
        );
        
        Ok(response)
    }
    
    /// 캐시를 사용하지 않는 권한 검증 (내부용)
    pub async fn check_permission_uncached(&self, request: &CheckRequest, snapshot_zookie: &Zookie) -> SentinelResult<CheckResponse> {
        let mut visited = HashSet::new();
        let mut result = PermissionCheckResult::new(
            &request.relation,
            &self.hierarchy,
        );

        let user_type = request.user_type.as_deref().unwrap_or("user");
        let has_permission = self.check_permission_recursive(
            &request.namespace,
            &request.object_id,
            &request.relation,
            user_type,
            &request.user_id,
            &mut visited,
            &mut result,
        ).await?;

        Ok(CheckResponse {
            allowed: has_permission,
            zookie: snapshot_zookie.to_string()?,
        })
    }

    /// 재귀적 권한 검증 (순환 참조 방지)
    #[async_recursion]
    async fn check_permission_recursive(
        &self,
        namespace: &str,
        object_id: &str,
        relation: &str,
        user_type: &str,
        user_id: &str,
        visited: &mut HashSet<String>,
        result: &mut PermissionCheckResult,
    ) -> SentinelResult<bool> {
        // 순환 참조 방지
        let check_key = format!("{}:{}#{}@{}:{}", namespace, object_id, relation, user_type, user_id);
        if visited.contains(&check_key) {
            return Ok(false);
        }
        visited.insert(check_key);

        // 1. 직접 권한 확인
        if self.check_direct_permission(namespace, object_id, relation, user_type, user_id).await? {
            result.add_direct_permission(relation, &self.hierarchy);
            return Ok(true);
        }

        // 2. 권한 상속 확인 (editor -> viewer 등)
        if self.check_inherited_permissions(namespace, object_id, relation, user_type, user_id, visited, result).await? {
            return Ok(true);
        }

        // 3. Userset 권한 확인 (팀 멤버십 등)
        if self.check_userset_permissions(namespace, object_id, relation, user_type, user_id, visited, result).await? {
            return Ok(true);
        }

        Ok(false)
    }

    /// 직접 권한 확인 (정확히 일치하는 튜플)
    async fn check_direct_permission(
        &self,
        namespace: &str,
        object_id: &str,
        relation: &str,
        user_type: &str,
        user_id: &str,
    ) -> SentinelResult<bool> {
        let tuple = RelationTuple {
            namespace: namespace.to_string(),
            object_id: object_id.to_string(),
            relation: relation.to_string(),
            user_type: user_type.to_string(),
            user_id: user_id.to_string(),
            created_at: scylla::value::CqlTimestamp(0),
        };

        let found = self.tuple_store.find_direct_tuple(&tuple).await?;
        Ok(found.is_some())
    }

    /// 권한 상속 확인 (owner -> admin -> editor -> viewer)
    #[async_recursion]
    async fn check_inherited_permissions(
        &self,
        namespace: &str,
        object_id: &str,
        relation: &str,
        user_type: &str,
        user_id: &str,
        visited: &mut HashSet<String>,
        result: &mut PermissionCheckResult,
    ) -> SentinelResult<bool> {
        let inherited_permissions = self.hierarchy.get_inherited_permissions(relation);
        
        for higher_permission in inherited_permissions {
            if self.check_permission_recursive(
                namespace,
                object_id,
                &higher_permission,
                user_type,
                user_id,
                visited,
                result,
            ).await? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Userset 권한 확인 (팀 멤버십 기반 간접 권한)
    async fn check_userset_permissions(
        &self,
        namespace: &str,
        object_id: &str,
        relation: &str,
        user_type: &str,
        user_id: &str,
        visited: &mut HashSet<String>,
        result: &mut PermissionCheckResult,
    ) -> SentinelResult<bool> {
        // 해당 객체-관계에 대한 모든 권한 튜플 조회
        let all_tuples = self.tuple_store.find_tuples_by_object_relation(
            namespace,
            object_id,
            relation,
        ).await?;

        for tuple in all_tuples {
            // userset 형태인지 확인 (user_type이 'userset')
            if tuple.user_type == "userset" {
                // userset_id 파싱: "teams:backend#member" -> (teams, backend, member)
                if let Some((userset_namespace, userset_object_relation)) = tuple.user_id.split_once(':') {
                    if let Some((userset_object, userset_relation)) = userset_object_relation.split_once('#') {
                        // 사용자가 해당 userset에 속하는지 확인
                        if self.check_permission_recursive(
                            userset_namespace,
                            userset_object,
                            userset_relation,
                            user_type,
                            user_id,
                            visited,
                            result,
                        ).await? {
                            result.add_team_permission(userset_namespace, &tuple.user_id, &self.hierarchy);
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    /// Userset 멤버십 확인 (예: user:alice가 team:backend#member에 속하는가?)
    #[async_recursion]
    async fn check_userset_membership(
        &self,
        userset_type: &str,
        userset_id: &str,
        user_type: &str,
        user_id: &str,
        visited: &mut HashSet<String>,
    ) -> SentinelResult<bool> {
        // userset이 팀인 경우 멤버십 확인
        if userset_type == "team" {
            return self.check_team_membership(userset_id, user_id).await;
        }

        // 다른 userset 타입들에 대한 재귀적 확인
        // 예: group:editors#member -> team:backend#member
        let userset_tuples = self.tuple_store.find_tuples_by_object(userset_type, userset_id).await?;
        
        for tuple in userset_tuples {
            if tuple.user_type == user_type && tuple.user_id == user_id {
                return Ok(true);
            }
            
            // 중첩된 userset 확인 (재귀)
            if tuple.user_type != "user" {
                let membership_key = format!("membership:{}:{}@{}:{}", 
                    userset_type, userset_id, user_type, user_id);
                if !visited.contains(&membership_key) {
                    visited.insert(membership_key);
                    
                    if self.check_userset_membership(
                        &tuple.user_type,
                        &tuple.user_id,
                        user_type,
                        user_id,
                        visited,
                    ).await? {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    /// 팀 멤버십 확인 (Team Service와 연동)
    async fn check_team_membership(&self, team_id: &str, user_id: &str) -> SentinelResult<bool> {
        // TODO: Team Service API 호출로 실제 팀 멤버십 확인
        // 지금은 데이터베이스에서 직접 확인
        
        let membership_tuple = RelationTuple {
            namespace: "team".to_string(),
            object_id: team_id.to_string(),
            relation: "member".to_string(),
            user_type: "user".to_string(),
            user_id: user_id.to_string(),
            created_at: scylla::value::CqlTimestamp(0),
        };

        let found = self.tuple_store.find_direct_tuple(&membership_tuple).await?;
        Ok(found.is_some())
    }

    /// 사용자의 모든 권한 조회 (디버깅 및 권한 확인용)
    pub async fn get_user_permissions(&self, user_id: &str) -> SentinelResult<Vec<RelationTuple>> {
        self.tuple_store.find_user_memberships(user_id).await
    }

    /// 객체에 대한 모든 권한 조회
    pub async fn get_object_permissions(&self, namespace: &str, object_id: &str) -> SentinelResult<Vec<RelationTuple>> {
        self.tuple_store.find_tuples_by_object(namespace, object_id).await
    }
    
    /// 사용자와 관련된 모든 권한 캐시 무효화
    pub async fn invalidate_user_cache(&self, user_id: &str) -> SentinelResult<()> {
        let pattern = CacheKeyBuilder::user_permission_pattern(user_id);
        match self.cache.delete_pattern(&pattern).await {
            Ok(_) => {
                info!("Invalidated cache for user: {}", user_id);
                Ok(())
            }
            Err(e) => {
                warn!("Failed to invalidate user cache for {}: {}", user_id, e);
                Err(e)
            }
        }
    }
    
    /// 객체와 관련된 모든 권한 캐시 무효화
    pub async fn invalidate_object_cache(&self, namespace: &str, object_id: &str) -> SentinelResult<()> {
        let pattern = CacheKeyBuilder::object_permission_pattern(namespace, object_id);
        match self.cache.delete_pattern(&pattern).await {
            Ok(_) => {
                info!("Invalidated cache for object: {}:{}", namespace, object_id);
                Ok(())
            }
            Err(e) => {
                warn!("Failed to invalidate object cache for {}:{}: {}", namespace, object_id, e);
                Err(e)
            }
        }
    }
    
    /// 네임스페이스와 관련된 모든 권한 캐시 무효화
    pub async fn invalidate_namespace_cache(&self, namespace: &str) -> SentinelResult<()> {
        let pattern = CacheKeyBuilder::namespace_permission_pattern(namespace);
        match self.cache.delete_pattern(&pattern).await {
            Ok(_) => {
                info!("Invalidated cache for namespace: {}", namespace);
                Ok(())
            }
            Err(e) => {
                warn!("Failed to invalidate namespace cache for {}: {}", namespace, e);
                Err(e)
            }
        }
    }
    
    /// 특정 권한 체크 캐시만 무효화
    pub async fn invalidate_specific_cache(&self, request: &CheckRequest) -> SentinelResult<()> {
        let cache_key = CacheKeyBuilder::check_permission_key(request);
        match self.cache.delete(&cache_key).await {
            Ok(_) => {
                info!("Invalidated specific cache: {}", cache_key);
                Ok(())
            }
            Err(e) => {
                warn!("Failed to invalidate specific cache {}: {}", cache_key, e);
                Err(e)
            }
        }
    }
}