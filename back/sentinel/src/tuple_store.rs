use std::sync::Arc;
use scylla::client::session::Session;
use scylla::value::CqlTimestamp;
use crate::models::{RelationTuple, ChangelogEntry, Operation};
use crate::errors::{SentinelError, SentinelResult};

/// ScyllaDB와의 상호작용을 위한 TupleStore trait
/// 권한 튜플의 CRUD 작업과 복잡한 쿼리를 담당
#[async_trait::async_trait]
pub trait TupleStore: Send + Sync {
    /// 권한 튜플 삽입
    async fn insert_tuple(&self, tuple: &RelationTuple) -> SentinelResult<()>;
    
    /// 권한 튜플 삭제  
    async fn delete_tuple(&self, tuple: &RelationTuple) -> SentinelResult<()>;
    
    /// 직접 권한 튜플 조회 (정확히 일치하는 튜플)
    async fn find_direct_tuple(&self, tuple: &RelationTuple) -> SentinelResult<Option<RelationTuple>>;
    
    /// 특정 객체에 대한 모든 권한 튜플 조회
    async fn find_tuples_by_object(
        &self,
        namespace: &str,
        object_id: &str,
    ) -> SentinelResult<Vec<RelationTuple>>;
    
    /// 특정 객체-관계에 대한 모든 권한 튜플 조회
    async fn find_tuples_by_object_relation(
        &self,
        namespace: &str,
        object_id: &str,
        relation: &str,
    ) -> SentinelResult<Vec<RelationTuple>>;
    
    /// 사용자의 그룹 멤버십 조회 (team:backend#member@user:alice 형태)
    async fn find_user_memberships(&self, user_id: &str) -> SentinelResult<Vec<RelationTuple>>;
    
    /// 특정 userset의 모든 멤버 조회 (team:backend#member에 속한 모든 사용자)
    async fn find_userset_members(
        &self,
        namespace: &str,
        object_id: &str,
        relation: &str,
    ) -> SentinelResult<Vec<RelationTuple>>;
    
    /// 변경 이력 기록
    async fn record_change(&self, entry: &ChangelogEntry) -> SentinelResult<()>;
}

/// ScyllaDB 기반 TupleStore 구현체
pub struct ScyllaTupleStore {
    session: Arc<Session>,
}

impl ScyllaTupleStore {
    pub fn new(session: Arc<Session>) -> Self {
        Self { session }
    }
}

#[async_trait::async_trait]
impl TupleStore for ScyllaTupleStore {
    /// 권한 튜플 삽입 (인덱스 테이블들에 동시 삽입)
    async fn insert_tuple(&self, tuple: &RelationTuple) -> SentinelResult<()> {
        // 메인 테이블에 삽입
        let main_query = "
            INSERT INTO sentinel.relation_tuples 
            (namespace, object_id, relation, user_type, user_id, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
        ";
        
        self.session
            .query_unpaged(main_query, tuple)
            .await
            .map_err(|e| SentinelError::from_scylla_error(e, "Failed to insert tuple"))?;
        
        // 인덱스 테이블들에도 삽입
        let user_membership_query = "
            INSERT INTO sentinel.user_memberships 
            (user_id, user_type, namespace, object_id, relation, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
        ";
        
        let user_membership_values = (
            &tuple.user_id, &tuple.user_type, &tuple.namespace,
            &tuple.object_id, &tuple.relation, &tuple.created_at
        );
        
        self.session
            .query_unpaged(user_membership_query, user_membership_values)
            .await
            .map_err(|e| SentinelError::from_scylla_error(e, "Failed to insert user membership"))?;
        
        let object_permission_query = "
            INSERT INTO sentinel.object_permissions 
            (namespace, object_id, relation, user_type, user_id, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
        ";
        
        self.session
            .query_unpaged(object_permission_query, tuple)
            .await
            .map_err(|e| SentinelError::from_scylla_error(e, "Failed to insert object permission"))?;
        
        let relation_index_query = "
            INSERT INTO sentinel.relation_index 
            (namespace, relation, object_id, user_type, user_id, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
        ";
        
        let relation_index_values = (
            &tuple.namespace, &tuple.relation, &tuple.object_id,
            &tuple.user_type, &tuple.user_id, &tuple.created_at
        );
        
        self.session
            .query_unpaged(relation_index_query, relation_index_values)
            .await
            .map_err(|e| SentinelError::from_scylla_error(e, "Failed to insert relation index"))?;
            
        // 변경 이력 기록
        let changelog = ChangelogEntry::new(tuple, &Operation::Insert);
        self.record_change(&changelog).await?;
        
        Ok(())
    }
    
    /// 권한 튜플 삭제 (모든 인덱스 테이블에서 삭제)
    async fn delete_tuple(&self, tuple: &RelationTuple) -> SentinelResult<()> {
        let tuple_values = (
            &tuple.namespace, &tuple.object_id, &tuple.relation,
            &tuple.user_type, &tuple.user_id,
        );
        
        // 메인 테이블에서 삭제
        let main_delete = "
            DELETE FROM sentinel.relation_tuples 
            WHERE namespace = ? AND object_id = ? 
            AND relation = ? AND user_type = ? AND user_id = ?
        ";
        
        self.session
            .query_unpaged(main_delete, tuple_values)
            .await
            .map_err(|e| SentinelError::from_scylla_error(e, "Failed to delete tuple"))?;
        
        // 인덱스 테이블들에서도 삭제
        let user_membership_delete = "
            DELETE FROM sentinel.user_memberships 
            WHERE user_id = ? AND user_type = ? 
            AND namespace = ? AND object_id = ? AND relation = ?
        ";
        
        let user_membership_values = (
            &tuple.user_id, &tuple.user_type, &tuple.namespace,
            &tuple.object_id, &tuple.relation,
        );
        
        self.session
            .query_unpaged(user_membership_delete, user_membership_values)
            .await
            .map_err(|e| SentinelError::from_scylla_error(e, "Failed to delete user membership"))?;
        
        let object_permission_delete = "
            DELETE FROM sentinel.object_permissions 
            WHERE namespace = ? AND object_id = ? 
            AND relation = ? AND user_type = ? AND user_id = ?
        ";
        
        self.session
            .query_unpaged(object_permission_delete, tuple_values)
            .await
            .map_err(|e| SentinelError::from_scylla_error(e, "Failed to delete object permission"))?;
        
        let relation_index_delete = "
            DELETE FROM sentinel.relation_index 
            WHERE namespace = ? AND relation = ? 
            AND object_id = ? AND user_type = ? AND user_id = ?
        ";
        
        let relation_index_values = (
            &tuple.namespace, &tuple.relation, &tuple.object_id,
            &tuple.user_type, &tuple.user_id,
        );
        
        self.session
            .query_unpaged(relation_index_delete, relation_index_values)
            .await
            .map_err(|e| SentinelError::from_scylla_error(e, "Failed to delete relation index"))?;
            
        // 변경 이력 기록
        let changelog = ChangelogEntry::new(tuple, &Operation::Delete);
        self.record_change(&changelog).await?;
        
        Ok(())
    }
    
    /// 직접 권한 튜플 조회
    async fn find_direct_tuple(&self, tuple: &RelationTuple) -> SentinelResult<Option<RelationTuple>> {
        let query = "
            SELECT namespace, object_id, relation, user_type, user_id, created_at
            FROM sentinel.relation_tuples 
            WHERE namespace = ? AND object_id = ? 
            AND relation = ? AND user_type = ? AND user_id = ?
            LIMIT 1
        ";
        
        let values = (
            &tuple.namespace,
            &tuple.object_id,
            &tuple.relation, 
            &tuple.user_type,
            &tuple.user_id,
        );
        
        let result = self.session
            .query_unpaged(query, values)
            .await
            .map_err(|e| SentinelError::from_scylla_error(e, "Failed to find direct tuple"))?;
            
        let rows = result.into_rows_result()
            .map_err(|e| SentinelError::internal_error(format!("Query result error: {}", e)))?;
            
        if let Some(row) = rows.rows()
            .map_err(|e| SentinelError::from_rows_error(e, "Failed to access rows"))?.next() {
            let tuple: RelationTuple = row
                .map_err(|e| SentinelError::internal_error(format!("Row parsing error: {}", e)))?;
            Ok(Some(tuple))
        } else {
            Ok(None)
        }
    }
    
    /// 특정 객체에 대한 모든 권한 튜플 조회
    async fn find_tuples_by_object(
        &self,
        namespace: &str,
        object_id: &str,
    ) -> SentinelResult<Vec<RelationTuple>> {
        let query = "
            SELECT namespace, object_id, relation, user_type, user_id, created_at
            FROM sentinel.relation_tuples 
            WHERE namespace = ? AND object_id = ?
        ";
        
        let values = (namespace, object_id);
        
        let result = self.session
            .query_unpaged(query, values)
            .await
            .map_err(|e| SentinelError::from_scylla_error(e, "Failed to find tuples by object"))?;
            
        let rows = result.into_rows_result()
            .map_err(|e| SentinelError::internal_error(format!("Query result error: {}", e)))?;
            
        let mut tuples = Vec::new();
        for row in rows.rows()
            .map_err(|e| SentinelError::from_rows_error(e, "Failed to access rows"))? {
            let tuple: RelationTuple = row
                .map_err(|e| SentinelError::internal_error(format!("Row parsing error: {}", e)))?;
            tuples.push(tuple);
        }
        
        Ok(tuples)
    }
    
    /// 특정 객체-관계에 대한 모든 권한 튜플 조회
    async fn find_tuples_by_object_relation(
        &self,
        namespace: &str,
        object_id: &str,
        relation: &str,
    ) -> SentinelResult<Vec<RelationTuple>> {
        let query = "
            SELECT namespace, object_id, relation, user_type, user_id, created_at
            FROM sentinel.relation_tuples 
            WHERE namespace = ? AND object_id = ? AND relation = ?
        ";
        
        let values = (namespace, object_id, relation);
        
        let result = self.session
            .query_unpaged(query, values)
            .await
            .map_err(|e| SentinelError::from_scylla_error(e, "Failed to find tuples by object-relation"))?;
            
        let rows = result.into_rows_result()
            .map_err(|e| SentinelError::internal_error(format!("Query result error: {}", e)))?;
            
        let mut tuples = Vec::new();
        for row in rows.rows()
            .map_err(|e| SentinelError::from_rows_error(e, "Failed to access rows"))? {
            let tuple: RelationTuple = row
                .map_err(|e| SentinelError::internal_error(format!("Row parsing error: {}", e)))?;
            tuples.push(tuple);
        }
        
        Ok(tuples)
    }
    
    /// 사용자의 그룹 멤버십 조회 (최적화된 인덱스 테이블 사용)
    async fn find_user_memberships(&self, user_id: &str) -> SentinelResult<Vec<RelationTuple>> {
        let query = "
            SELECT user_id, user_type, namespace, object_id, relation, created_at
            FROM sentinel.user_memberships 
            WHERE user_id = ? AND user_type = 'user'
        ";
        
        let values = (user_id,);
        
        let result = self.session
            .query_unpaged(query, values)
            .await
            .map_err(|e| SentinelError::from_scylla_error(e, "Failed to find user memberships"))?;
            
        let rows = result.into_rows_result()
            .map_err(|e| SentinelError::internal_error(format!("Query result error: {}", e)))?;
            
        let mut tuples = Vec::new();
        for row in rows.rows()
            .map_err(|e| SentinelError::from_rows_error(e, "Failed to access rows"))? {
            // user_memberships 테이블의 컬럼 순서에 맞춰 RelationTuple 생성
            let (user_id, user_type, namespace, object_id, relation, created_at): (String, String, String, String, String, CqlTimestamp) = row
                .map_err(|e| SentinelError::internal_error(format!("Row parsing error: {}", e)))?;
            
            let tuple = RelationTuple {
                namespace,
                object_id,
                relation,
                user_type,
                user_id,
                created_at,
            };
            tuples.push(tuple);
        }
        
        Ok(tuples)
    }
    
    /// 특정 userset의 모든 멤버 조회
    async fn find_userset_members(
        &self,
        namespace: &str,
        object_id: &str,
        relation: &str,
    ) -> SentinelResult<Vec<RelationTuple>> {
        let query = "
            SELECT namespace, object_id, relation, user_type, user_id, created_at
            FROM sentinel.relation_tuples 
            WHERE namespace = ? AND object_id = ? AND relation = ?
        ";
        
        let values = (namespace, object_id, relation);
        
        let result = self.session
            .query_unpaged(query, values)
            .await
            .map_err(|e| SentinelError::from_scylla_error(e, "Failed to find userset members"))?;
            
        let rows = result.into_rows_result()
            .map_err(|e| SentinelError::internal_error(format!("Query result error: {}", e)))?;
            
        let mut tuples = Vec::new();
        for row in rows.rows()
            .map_err(|e| SentinelError::from_rows_error(e, "Failed to access rows"))? {
            let tuple: RelationTuple = row
                .map_err(|e| SentinelError::internal_error(format!("Row parsing error: {}", e)))?;
            tuples.push(tuple);
        }
        
        Ok(tuples)
    }
    
    /// 변경 이력 기록
    async fn record_change(&self, entry: &ChangelogEntry) -> SentinelResult<()> {
        let query = "
            INSERT INTO sentinel.changelog 
            (id, namespace, object_id, relation, user_type, user_id, operation, timestamp)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ";
        
        self.session
            .query_unpaged(query, entry)
            .await
            .map_err(|e| SentinelError::from_scylla_error(e, "Failed to record changelog"))?;
            
        Ok(())
    }
}