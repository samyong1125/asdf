use serde::{Deserialize, Serialize};
use scylla::{DeserializeRow, SerializeRow};
use scylla::value::CqlTimestamp;
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Zanzibar 권한 튜플을 나타내는 구조체 (데이터베이스 저장용)
/// 스키마: relation_tuples (namespace, object_id, relation, user_type, user_id, created_at)
#[derive(Debug, Clone, PartialEq, Eq, SerializeRow, DeserializeRow)]
pub struct RelationTuple {
    /// 네임스페이스 (예: "document", "team", "project")
    pub namespace: String,
    /// 객체 ID (예: "doc:123", "team:backend")
    pub object_id: String,
    /// 관계 (예: "owner", "viewer", "member")
    pub relation: String,
    /// 사용자 타입 (예: "user", "team")
    pub user_type: String,
    /// 사용자 ID (예: "alice", "team:backend")
    pub user_id: String,
    /// 생성 시간
    pub created_at: CqlTimestamp,
}

/// API 요청/응답에서 사용하는 권한 튜플 구조체
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiRelationTuple {
    /// 네임스페이스 (예: "document", "team", "project")
    pub namespace: String,
    /// 객체 ID (예: "doc:123", "team:backend")
    pub object_id: String,
    /// 관계 (예: "owner", "viewer", "member")
    pub relation: String,
    /// 사용자 타입 (예: "user", "team")
    pub user_type: String,
    /// 사용자 ID (예: "alice", "team:backend")
    pub user_id: String,
    /// 생성 시간
    pub created_at: DateTime<Utc>,
}

impl RelationTuple {
    /// 새로운 RelationTuple 생성
    pub fn new(
        namespace: impl Into<String>,
        object_id: impl Into<String>,
        relation: impl Into<String>,
        user_type: impl Into<String>,
        user_id: impl Into<String>,
    ) -> Self {
        Self {
            namespace: namespace.into(),
            object_id: object_id.into(),
            relation: relation.into(),
            user_type: user_type.into(),
            user_id: user_id.into(),
            created_at: CqlTimestamp(chrono::Utc::now().timestamp_millis()),
        }
    }

    /// ApiRelationTuple로 변환
    pub fn to_api_tuple(&self) -> ApiRelationTuple {
        ApiRelationTuple {
            namespace: self.namespace.clone(),
            object_id: self.object_id.clone(),
            relation: self.relation.clone(),
            user_type: self.user_type.clone(),
            user_id: self.user_id.clone(),
            created_at: DateTime::from_timestamp_millis(self.created_at.0)
                .unwrap_or_else(|| chrono::Utc::now()),
        }
    }

    /// 튜플을 문자열로 표현 (디버깅용)
    pub fn to_string_representation(&self) -> String {
        format!(
            "{}:{}#{}@{}:{}",
            self.namespace, self.object_id, self.relation, self.user_type, self.user_id
        )
    }

    /// 직접 사용자 권한인지 확인 (userset이 아닌)
    pub fn is_direct_user(&self) -> bool {
        self.user_type == "user"
    }

    /// userset 권한인지 확인
    pub fn is_userset(&self) -> bool {
        self.user_type != "user"
    }
}

impl ApiRelationTuple {
    /// RelationTuple로 변환 (데이터베이스 저장용)
    pub fn to_db_tuple(&self) -> RelationTuple {
        RelationTuple {
            namespace: self.namespace.clone(),
            object_id: self.object_id.clone(),
            relation: self.relation.clone(),
            user_type: self.user_type.clone(),
            user_id: self.user_id.clone(),
            created_at: CqlTimestamp(self.created_at.timestamp_millis()),
        }
    }
}

/// 권한 체크 요청
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRequest {
    /// 네임스페이스
    pub namespace: String,
    /// 객체 ID
    pub object_id: String,
    /// 관계
    pub relation: String,
    /// 사용자 ID
    pub user_id: String,
    /// 사용자 타입 (선택적, 기본값: "user")
    pub user_type: Option<String>,
    /// 일관성 토큰 (선택적)
    pub zookie: Option<String>,
}

/// 권한 체크 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResponse {
    /// 권한 허용 여부
    pub allowed: bool,
    /// 응답 시간의 일관성 토큰
    pub zookie: String,
}

/// 권한 튜플 쓰기 작업 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// 튜플 추가
    Insert,
    /// 튜플 삭제
    Delete,
}

/// 권한 튜플 쓰기 요청
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteRequest {
    /// 수행할 작업들
    pub updates: Vec<TupleUpdate>,
    /// 선행 조건 (선택적)
    pub preconditions: Option<Vec<Precondition>>,
}

/// 튜플 업데이트 작업
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TupleUpdate {
    /// 작업 타입
    pub operation: Operation,
    /// 대상 튜플
    pub tuple: ApiRelationTuple,
}

/// 쓰기 작업의 선행 조건
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Precondition {
    /// 작업 타입
    pub operation: Operation,
    /// 필터 조건
    pub filter: RelationTupleFilter,
}

/// 튜플 필터 조건
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationTupleFilter {
    /// 네임스페이스 (선택적)
    pub namespace: Option<String>,
    /// 객체 ID (선택적)  
    pub object_id: Option<String>,
    /// 관계 (선택적)
    pub relation: Option<String>,
    /// 사용자 타입 (선택적)
    pub user_type: Option<String>,
    /// 사용자 ID (선택적)
    pub user_id: Option<String>,
}

/// 권한 튜플 쓰기 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteResponse {
    /// 응답 시간의 일관성 토큰
    pub zookie: String,
}

/// 권한 튜플 읽기 요청
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadRequest {
    /// 읽기 필터
    pub tuple_filter: RelationTupleFilter,
    /// 일관성 토큰 (선택적)
    pub zookie: Option<String>,
    /// 페이지 크기 (선택적)
    pub page_size: Option<u32>,
    /// 페이지 토큰 (선택적)
    pub page_token: Option<String>,
}

/// 권한 튜플 읽기 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadResponse {
    /// 조회된 튜플들
    pub tuples: Vec<ApiRelationTuple>,
    /// 다음 페이지 토큰 (선택적)
    pub next_page_token: Option<String>,
    /// 응답 시간의 일관성 토큰
    pub zookie: String,
}

/// 변경 이력 기록용 구조체 (데이터베이스 저장용)
/// 스키마: changelog (id, namespace, object_id, relation, user_type, user_id, operation, timestamp)
#[derive(Debug, Clone, SerializeRow, DeserializeRow)]
pub struct ChangelogEntry {
    /// 고유 ID
    pub id: Uuid,
    /// 네임스페이스
    pub namespace: String,
    /// 객체 ID
    pub object_id: String,
    /// 관계
    pub relation: String,
    /// 사용자 타입
    pub user_type: String,
    /// 사용자 ID
    pub user_id: String,
    /// 작업 타입 ("INSERT" 또는 "DELETE")
    pub operation: String,
    /// 작업 시간
    pub timestamp: CqlTimestamp,
}

impl ChangelogEntry {
    /// 새로운 변경 이력 생성
    pub fn new(tuple: &RelationTuple, operation: &Operation) -> Self {
        Self {
            id: Uuid::new_v4(),
            namespace: tuple.namespace.clone(),
            object_id: tuple.object_id.clone(),
            relation: tuple.relation.clone(),  
            user_type: tuple.user_type.clone(),
            user_id: tuple.user_id.clone(),
            operation: match operation {
                Operation::Insert => "INSERT".to_string(),
                Operation::Delete => "DELETE".to_string(),
            },
            timestamp: CqlTimestamp(chrono::Utc::now().timestamp_millis()),
        }
    }
}

/// 배치 권한 체크 요청
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCheckRequest {
    /// 체크할 권한들의 목록
    pub checks: Vec<CheckRequest>,
    /// 일관성 토큰 (선택적)
    pub zookie: Option<String>,
}

/// 개별 권한 체크 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCheckItem {
    /// 원본 요청 인덱스
    pub request_index: usize,
    /// 권한 허용 여부
    pub allowed: bool,
    /// 요청 정보 (디버깅용)
    pub request_info: String,
}

/// 배치 권한 체크 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCheckResponse {
    /// 각 권한 체크 결과
    pub results: Vec<BatchCheckItem>,
    /// 전체 처리된 요청 수
    pub total_requests: usize,
    /// 허용된 요청 수
    pub allowed_count: usize,
    /// 거부된 요청 수
    pub denied_count: usize,
    /// 응답 시간의 일관성 토큰
    pub zookie: String,
}

impl BatchCheckResponse {
    /// 새로운 배치 응답 생성
    pub fn new(results: Vec<BatchCheckItem>) -> Self {
        let total_requests = results.len();
        let allowed_count = results.iter().filter(|r| r.allowed).count();
        let denied_count = total_requests - allowed_count;
        
        Self {
            results,
            total_requests,
            allowed_count,
            denied_count,
            zookie: format!("{}", chrono::Utc::now().timestamp_millis()),
        }
    }
}