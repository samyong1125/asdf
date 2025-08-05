use std::sync::Arc;
use std::collections::HashSet;
use async_recursion::async_recursion;
use crate::models::{RelationTuple, CheckRequest, CheckResponse};
use crate::tuple_store::{TupleStore, ScyllaTupleStore};
use crate::permission_hierarchy::{PermissionHierarchy, PermissionCheckResult};
use crate::errors::SentinelResult;

/// Zanzibar 권한 검증 엔진
/// 직접 권한, userset 재귀 확인, 권한 상속을 처리
pub struct PermissionChecker {
    tuple_store: Arc<ScyllaTupleStore>,
    hierarchy: PermissionHierarchy,
}

impl PermissionChecker {
    /// 새로운 PermissionChecker 생성
    pub fn new(tuple_store: Arc<ScyllaTupleStore>) -> Self {
        Self {
            tuple_store,
            hierarchy: PermissionHierarchy::new(),
        }
    }

    /// 권한 검증 메인 함수 (Zanzibar Check API)
    pub async fn check_permission(&self, request: &CheckRequest) -> SentinelResult<CheckResponse> {
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
            zookie: format!("{}", chrono::Utc::now().timestamp_millis()),
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
}