# Sentinel - Zanzibar 권한 관리 시스템 명세서

## 1. 개요

**Sentinel**은 Google Zanzibar 논문을 기반으로 구현한 관계 기반 권한 관리 시스템입니다. 마이크로서비스 아키텍처에서 중앙집중식 권한 관리를 제공하며, 복잡한 권한 상속과 팀 기반 간접 권한을 지원합니다.

### 1.1 핵심 특징
- **Google Zanzibar 호환**: 관계 튜플 기반 권한 표현
- **확장 가능한 아키텍처**: ScyllaDB + Redis 기반 고성능 처리
- **권한 상속**: 계층적 권한 구조 (owner → admin → editor → commenter → viewer)
- **Userset 지원**: 팀 멤버십을 통한 간접 권한 부여
- **외부 일관성**: Zookie 토큰을 통한 일관된 읽기 보장
- **독립적 서비스**: API Gateway를 거치지 않는 직접 호출

### 1.2 기술 스택
- **언어**: Rust (Edition 2024)
- **웹 프레임워크**: Actix Web 4.11.0
- **주 데이터베이스**: ScyllaDB (권한 튜플 저장)
- **캐시**: Redis (권한 검증 결과 캐싱)
- **포트**: 15004

## 2. 아키텍처

### 2.1 시스템 구성도

```
┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│   User Service   │    │   Team Service   │    │   Other Service  │
│   (Python)       │    │   (Kotlin)       │    │   (Various)      │
└─────────┬────────┘    └─────────┬────────┘    └─────────┬────────┘
          │                       │                       │
          └───────────────────────┼───────────────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │      Sentinel API         │
                    │    (Rust/Actix Web)       │
                    │       Port: 15004         │
                    └─────────────┬─────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │    권한 검증 엔진           │
                    │  - 직접 권한 확인           │
                    │  - 권한 상속 처리           │
                    │  - Userset 재귀 확인       │
                    └─────┬───────────────┬─────┘
                          │               │
              ┌───────────▼─────────┐    ┌▼─────────┐
              │     ScyllaDB        │    │  Redis   │
              │  (권한 튜플 저장)     │    │ (캐싱)    │
              │  Port: 50004-50005  │    │Port:50006│
              └─────────────────────┘    └──────────┘
```

### 2.2 데이터 플로우

1. **권한 체크 요청**: 다른 서비스 → Sentinel API
2. **권한 검증**: 직접 권한 → 상속 권한 → Userset 권한 순차 확인
3. **데이터베이스 조회**: ScyllaDB에서 관련 튜플 검색
4. **결과 캐싱**: Redis에 검증 결과 저장 (TTL: 5분)
5. **응답 반환**: 권한 허용 여부 + Zookie 토큰

## 3. 데이터 모델

### 3.1 관계 튜플 (RelationTuple)

권한은 `<namespace:object_id#relation@user_type:user_id>` 형태의 튜플로 표현됩니다.

```rust
pub struct RelationTuple {
    pub namespace: String,    // 네임스페이스 (예: "document", "team")
    pub object_id: String,    // 객체 ID (예: "doc123", "backend-team")
    pub relation: String,     // 관계 (예: "owner", "member")
    pub user_type: String,    // 사용자 타입 (예: "user", "team")
    pub user_id: String,      // 사용자 ID (예: "alice", "backend-team")
    pub created_at: CqlTimestamp,
}
```

#### 튜플 예시

```
# 직접 사용자 권한
document:project-spec#owner@user:alice
document:api-docs#viewer@user:bob

# 팀 멤버십
team:backend-team#member@user:alice
team:backend-team#member@user:bob

# Userset 권한 (팀이 문서에 대한 권한)
document:team-handbook#editor@team:backend-team

# 팀 관리자 권한
team:frontend-team#admin@user:charlie
```

### 3.2 권한 계층구조

```
owner (레벨 5)     ──→ 최고 권한
  ├── admin (레벨 4)   ──→ 관리자 권한
  │   ├── editor (레벨 3) ──→ 편집 권한
  │   │   ├── commenter (레벨 2) ──→ 댓글 권한
  │   │   │   └── viewer (레벨 1) ──→ 보기 권한
```

**상속 규칙**: 상위 권한을 가진 사용자는 모든 하위 권한을 자동으로 획득

### 3.3 ScyllaDB 테이블 스키마

#### relation_tuples (주 테이블)
```cql
CREATE TABLE relation_tuples (
    namespace text,
    object_id text,
    relation text,
    user_type text,
    user_id text,
    created_at timestamp,
    PRIMARY KEY ((namespace, object_id), relation, user_type, user_id)
)
```

#### user_memberships (사용자 권한 인덱스)
```cql
CREATE TABLE user_memberships (
    user_id text,
    user_type text,
    namespace text,
    object_id text,
    relation text,
    created_at timestamp,
    PRIMARY KEY ((user_id, user_type), namespace, object_id, relation)
)
```

#### object_permissions (객체 권한 인덱스)
```cql
CREATE TABLE object_permissions (
    namespace text,
    object_id text,
    relation text,
    user_type text,
    user_id text,
    created_at timestamp,
    PRIMARY KEY ((namespace, object_id), relation, user_type, user_id)
)
```

#### relation_index (관계별 인덱스)
```cql
CREATE TABLE relation_index (
    namespace text,
    relation text,
    object_id text,
    user_type text,
    user_id text,
    created_at timestamp,
    PRIMARY KEY ((namespace, relation), object_id, user_type, user_id)
)
```

## 4. API 명세

### 4.1 기본 정보
- **Base URL**: `http://localhost:15004`
- **Content-Type**: `application/json`
- **인증**: 없음 (내부 서비스 간 통신)

### 4.2 헬스체크

#### GET /health
서비스 상태 확인

**응답:**
```json
{
  "status": "ok",
  "timestamp": "2025-08-04T04:00:00Z"
}
```

### 4.3 데이터베이스 연결 테스트

#### GET /db-test
모든 데이터베이스 연결 상태 확인

#### GET /redis-test
Redis 연결 상태 확인

#### GET /scylla-test
ScyllaDB 연결 상태 확인

### 4.4 권한 검증 API

#### POST /api/v1/check
권한 검증 요청 (Zanzibar Check API)

**요청:**
```json
{
  "namespace": "document",
  "object_id": "project-spec",
  "relation": "viewer",
  "user_id": "alice",
  "user_type": "user",  // 선택적, 기본값: "user"
  "zookie": "1754283000000"  // 선택적, 일관성 토큰
}
```

**응답:**
```json
{
  "allowed": true,
  "zookie": "1754283001000"
}
```

### 4.5 권한 관리 API

#### POST /api/v1/write
권한 튜플 생성/삭제 (Zanzibar Write API)

**요청:**
```json
{
  "updates": [
    {
      "operation": "Insert",  // "Insert" 또는 "Delete"
      "tuple": {
        "namespace": "document",
        "object_id": "project-spec",
        "relation": "owner",
        "user_type": "user",
        "user_id": "alice",
        "created_at": "2025-08-04T04:00:00Z"
      }
    }
  ],
  "preconditions": []  // 선택적, 선행 조건
}
```

**응답:**
```json
{
  "zookie": "1754283001000"
}
```

#### POST /api/v1/read
권한 튜플 조회 (Zanzibar Read API)

**요청:**
```json
{
  "tuple_filter": {
    "namespace": "document",  // 선택적
    "object_id": "project-spec",  // 선택적
    "relation": "owner",  // 선택적
    "user_type": "user",  // 선택적
    "user_id": "alice"  // 선택적
  },
  "zookie": "1754283000000",  // 선택적
  "page_size": 100,  // 선택적
  "page_token": "next_page_token"  // 선택적
}
```

**응답:**
```json
{
  "tuples": [
    {
      "namespace": "document",
      "object_id": "project-spec",
      "relation": "owner",
      "user_type": "user",
      "user_id": "alice",
      "created_at": "2025-08-04T04:00:00Z"
    }
  ],
  "next_page_token": null,
  "zookie": "1754283001000"
}
```

## 5. 권한 검증 로직

### 5.1 검증 순서

권한 검증은 다음 순서로 진행됩니다:

1. **직접 권한 확인**: 요청된 권한이 정확히 일치하는 튜플이 있는가?
2. **권한 상속 확인**: 상위 권한을 통해 요청된 권한을 획득할 수 있는가?
3. **Userset 권한 확인**: 팀 멤버십을 통해 간접적으로 권한을 획득할 수 있는가?

### 5.2 직접 권한 확인

```rust
// 예시: user:alice가 document:project-spec#viewer 권한을 가지는가?
let tuple = RelationTuple {
    namespace: "document",
    object_id: "project-spec", 
    relation: "viewer",
    user_type: "user",
    user_id: "alice",
    // ...
};

let found = tuple_store.find_direct_tuple(&tuple).await?;
// 결과: 해당 튜플이 존재하면 true
```

### 5.3 권한 상속 확인

```rust
// viewer 권한 요청 시 확인할 상위 권한들
let inherited_permissions = hierarchy.get_inherited_permissions("viewer");
// 결과: ["owner", "admin", "editor", "commenter"]

// 각 상위 권한에 대해 재귀적으로 확인
for permission in inherited_permissions {
    if check_permission_recursive(namespace, object_id, &permission, user_type, user_id).await? {
        return Ok(true);
    }
}
```

### 5.4 Userset 권한 확인

```rust
// document:team-handbook#editor 권한을 가진 모든 주체 조회
let tuples = tuple_store.find_tuples_by_object_relation("document", "team-handbook", "editor").await?;

for tuple in tuples {
    if tuple.user_type == "team" {
        // user:alice가 team:backend-team의 멤버인가?
        if check_team_membership("backend-team", "alice").await? {
            return Ok(true);
        }
    }
}
```

### 5.5 팀 멤버십 확인

```rust
// user:alice가 team:backend-team#member 권한을 가지는가?
let membership_tuple = RelationTuple {
    namespace: "team",
    object_id: "backend-team",
    relation: "member", 
    user_type: "user",
    user_id: "alice",
    // ...
};

let found = tuple_store.find_direct_tuple(&membership_tuple).await?;
```

## 6. 사용 예시

### 6.1 기본 권한 시나리오

```bash
# 1. Alice에게 문서 소유 권한 부여
curl -X POST http://localhost:15004/api/v1/write \
  -H "Content-Type: application/json" \
  -d '{
    "updates": [{
      "operation": "Insert",
      "tuple": {
        "namespace": "document",
        "object_id": "project-spec",
        "relation": "owner",
        "user_type": "user", 
        "user_id": "alice",
        "created_at": "2025-08-04T04:00:00Z"
      }
    }]
  }'

# 2. Alice의 viewer 권한 확인 (상속을 통해 true가 되어야 함)
curl -X POST http://localhost:15004/api/v1/check \
  -H "Content-Type: application/json" \
  -d '{
    "namespace": "document",
    "object_id": "project-spec", 
    "relation": "viewer",
    "user_id": "alice"
  }'
# 응답: {"allowed": true, "zookie": "..."}
```

### 6.2 팀 기반 권한 시나리오

```bash
# 1. 팀 생성 및 멤버 추가
curl -X POST http://localhost:15004/api/v1/write \
  -H "Content-Type: application/json" \
  -d '{
    "updates": [
      {
        "operation": "Insert",
        "tuple": {
          "namespace": "team",
          "object_id": "backend-team",
          "relation": "member", 
          "user_type": "user",
          "user_id": "alice",
          "created_at": "2025-08-04T04:00:00Z"
        }
      },
      {
        "operation": "Insert",
        "tuple": {
          "namespace": "document",
          "object_id": "team-handbook",
          "relation": "editor",
          "user_type": "team",
          "user_id": "backend-team", 
          "created_at": "2025-08-04T04:00:00Z"
        }
      }
    ]
  }'

# 2. Alice의 간접 권한 확인 (팀 멤버십을 통해)
curl -X POST http://localhost:15004/api/v1/check \
  -H "Content-Type: application/json" \
  -d '{
    "namespace": "document",
    "object_id": "team-handbook",
    "relation": "editor",
    "user_id": "alice"
  }'
# 응답: {"allowed": true, "zookie": "..."}
```

### 6.3 권한 조회

```bash
# 특정 문서의 모든 권한 조회
curl -X POST http://localhost:15004/api/v1/read \
  -H "Content-Type: application/json" \
  -d '{
    "tuple_filter": {
      "namespace": "document",
      "object_id": "project-spec"
    }
  }'
```

## 7. Team Service 연동 통합 테스트

### 7.1 완전한 E2E 테스트 시나리오

다음은 Team Service와 Sentinel이 완전히 연동된 상태에서 실제로 실행하여 검증된 테스트 명령어들입니다.

#### 환경 준비
```bash
# 모든 서비스 시작
docker-compose up -d

# 서비스 상태 확인
curl -s http://localhost:15003/health  # Team Service 
curl -s http://localhost:15004/health  # Sentinel
```

#### 단계 1: 사용자 등록 및 인증
```bash
# Alice 사용자 등록
curl -X POST http://localhost:15001/api/v1/register \
  -H "Content-Type: application/json" \
  -d '{"email": "alice@test.com", "password": "password123"}'
# 응답: JWT 토큰 획득

# Bob 사용자 등록  
curl -X POST http://localhost:15001/api/v1/register \
  -H "Content-Type: application/json" \
  -d '{"email": "bob@test.com", "password": "password123"}'
# 응답: JWT 토큰 획득
```

#### 단계 2: 팀 생성 및 자동 권한 동기화 검증
```bash
# Alice가 API Gateway를 통해 새 팀 생성
curl -X POST http://localhost:15000/api/v1/teams \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1c2VyX2lkIjoxLCJlbWFpbCI6ImFsaWNlQHRlc3QuY29tIiwic3ViIjoiMSIsImV4cCI6MTc1NDI4NjI2MCwiaWF0IjoxNzU0Mjg1MzYwfQ.kbrDyt570606HuXi8zu0j3DXh9br84HF47HO4rfoJfI" \
  -H "Content-Type: application/json" \
  -d '{"name": "integration-test-team"}'
# 응답: {"id": "68904544d80f3741080d6276", "name": "integration-test-team", ...}

# ✅ 검증: Alice의 팀 멤버십 권한이 Sentinel에 자동 추가되었는지 확인
curl -X POST http://localhost:15004/api/v1/check \
  -H "Content-Type: application/json" \
  -d '{
    "namespace": "team",
    "object_id": "68904544d80f3741080d6276",
    "relation": "member",
    "user_id": "1"
  }'
# 예상 응답: {"allowed": true, "zookie": "..."}
```

#### 단계 3: 팀 멤버 추가 및 권한 동기화 검증
```bash
# Alice가 Bob을 팀에 추가
curl -X POST http://localhost:15000/api/v1/teams/68904544d80f3741080d6276/members \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1c2VyX2lkIjoxLCJlbWFpbCI6ImFsaWNlQHRlc3QuY29tIiwic3ViIjoiMSIsImV4cCI6MTc1NDI4NjI2MCwiaWF0IjoxNzU0Mjg1MzYwfQ.kbrDyt570606HuXi8zu0j3DXh9br84HF47HO4rfoJfI" \
  -H "Content-Type: application/json" \
  -d '{"userId": 2}'
# 응답: {"message": "멤버가 추가되었습니다"}

# ✅ 검증: Bob의 팀 멤버십 권한이 Sentinel에 자동 추가되었는지 확인
curl -X POST http://localhost:15004/api/v1/check \
  -H "Content-Type: application/json" \
  -d '{
    "namespace": "team",
    "object_id": "68904544d80f3741080d6276",
    "relation": "member",
    "user_id": "2"
  }'
# 예상 응답: {"allowed": true, "zookie": "..."}
```

#### 단계 4: Userset 기반 간접 권한 테스트
```bash
# 팀에 문서 편집 권한 부여 (Sentinel 직접 호출)
curl -X POST http://localhost:15004/api/v1/write \
  -H "Content-Type: application/json" \
  -d '{
    "updates": [{
      "operation": "Insert",
      "tuple": {
        "namespace": "document",
        "object_id": "team-project-doc",
        "relation": "editor",
        "user_type": "team",
        "user_id": "68904544d80f3741080d6276",
        "created_at": "2025-08-04T05:30:00Z"
      }
    }]
  }'
# 응답: {"zookie": "..."}

# ✅ 검증: Alice가 팀 멤버십을 통해 문서 편집 권한을 획득했는지 확인
curl -X POST http://localhost:15004/api/v1/check \
  -H "Content-Type: application/json" \
  -d '{
    "namespace": "document",
    "object_id": "team-project-doc",
    "relation": "editor",
    "user_id": "1"
  }'
# 예상 응답: {"allowed": true, "zookie": "..."}

# ✅ 검증: Bob도 팀 멤버십을 통해 문서 편집 권한을 획득했는지 확인
curl -X POST http://localhost:15004/api/v1/check \
  -H "Content-Type: application/json" \
  -d '{
    "namespace": "document",
    "object_id": "team-project-doc",
    "relation": "editor",
    "user_id": "2"
  }'
# 예상 응답: {"allowed": true, "zookie": "..."}
```

#### 단계 5: 권한 상속 + Userset 복합 테스트
```bash
# ✅ 검증: Alice가 editor -> viewer 권한 상속을 통해 viewer 권한도 획득했는지 확인
curl -X POST http://localhost:15004/api/v1/check \
  -H "Content-Type: application/json" \
  -d '{
    "namespace": "document",
    "object_id": "team-project-doc",
    "relation": "viewer",
    "user_id": "1"
  }'
# 예상 응답: {"allowed": true, "zookie": "..."}

# ✅ 검증: Bob도 editor -> viewer 권한 상속을 통해 viewer 권한을 획득했는지 확인
curl -X POST http://localhost:15004/api/v1/check \
  -H "Content-Type: application/json" \
  -d '{
    "namespace": "document",
    "object_id": "team-project-doc",
    "relation": "viewer",
    "user_id": "2"
  }'
# 예상 응답: {"allowed": true, "zookie": "..."}
```

#### 단계 6: 멤버 제거 및 권한 자동 삭제 검증
```bash
# Alice가 Bob을 팀에서 제거
curl -X DELETE http://localhost:15000/api/v1/teams/68904544d80f3741080d6276/members/2 \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1c2VyX2lkIjoxLCJlbWFpbCI6ImFsaWNlQHRlc3QuY29tIiwic3ViIjoiMSIsImV4cCI6MTc1NDI4NjI2MCwiaWF0IjoxNzU0Mjg1MzYwfQ.kbrDyt570606HuXi8zu0j3DXh9br84HF47HO4rfoJfI"
# 응답: {"message": "멤버가 제거되었습니다"}

# ✅ 검증: Bob의 팀 멤버십 권한이 자동으로 제거되었는지 확인
curl -X POST http://localhost:15004/api/v1/check \
  -H "Content-Type: application/json" \
  -d '{
    "namespace": "team",
    "object_id": "68904544d80f3741080d6276",
    "relation": "member",
    "user_id": "2"
  }'
# 예상 응답: {"allowed": false, "zookie": "..."}

# ✅ 검증: Bob의 문서 편집 권한도 자동으로 제거되었는지 확인
curl -X POST http://localhost:15004/api/v1/check \
  -H "Content-Type: application/json" \
  -d '{
    "namespace": "document",
    "object_id": "team-project-doc",
    "relation": "editor",
    "user_id": "2"
  }'
# 예상 응답: {"allowed": false, "zookie": "..."}

# ✅ 검증: Alice는 여전히 권한을 보유하고 있는지 확인
curl -X POST http://localhost:15004/api/v1/check \
  -H "Content-Type: application/json" \
  -d '{
    "namespace": "document",
    "object_id": "team-project-doc",
    "relation": "editor",
    "user_id": "1"
  }'
# 예상 응답: {"allowed": true, "zookie": "..."}
```

#### 단계 7: 팀 삭제 및 모든 권한 일괄 삭제 검증
```bash
# Alice가 팀 삭제
curl -X DELETE http://localhost:15000/api/v1/teams/68904544d80f3741080d6276 \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJ1c2VyX2lkIjoxLCJlbWFpbCI6ImFsaWNlQHRlc3QuY29tIiwic3ViIjoiMSIsImV4cCI6MTc1NDI4NjI2MCwiaWF0IjoxNzU0Mjg1MzYwfQ.kbrDyt570606HuXi8zu0j3DXh9br84HF47HO4rfoJfI"
# 응답: {"message": "팀이 삭제되었습니다"}

# ✅ 검증: Alice의 팀 멤버십 권한이 자동으로 제거되었는지 확인
curl -X POST http://localhost:15004/api/v1/check \
  -H "Content-Type: application/json" \
  -d '{
    "namespace": "team",
    "object_id": "68904544d80f3741080d6276",
    "relation": "member",
    "user_id": "1"
  }'
# 예상 응답: {"allowed": false, "zookie": "..."}

# ✅ 검증: Alice의 문서 편집 권한도 자동으로 제거되었는지 확인
curl -X POST http://localhost:15004/api/v1/check \
  -H "Content-Type: application/json" \
  -d '{
    "namespace": "document",
    "object_id": "team-project-doc",
    "relation": "editor",
    "user_id": "1"
  }'
# 예상 응답: {"allowed": false, "zookie": "..."}
```

### 7.2 테스트 결과 요약

위의 모든 테스트 명령어는 **실제로 실행되어 성공**한 것들입니다:

- ✅ **팀 생성 → 자동 멤버십 권한 추가**
- ✅ **멤버 추가 → 자동 멤버십 권한 추가**  
- ✅ **Userset 기반 간접 권한** (팀 → 문서 권한 전파)
- ✅ **권한 상속** (editor → viewer 자동 상속)
- ✅ **멤버 제거 → 자동 권한 삭제**
- ✅ **팀 삭제 → 모든 멤버십 권한 일괄 삭제**

이 테스트를 통해 Google Zanzibar 기반의 완전한 권한 관리 시스템이 Team Service와 완벽하게 연동되어 작동함을 검증했습니다.

## 7. 성능 최적화

### 7.1 데이터베이스 최적화

#### 인덱스 전략
- **주 테이블**: `(namespace, object_id)` 파티션 키로 객체별 권한 빠른 조회
- **사용자 인덱스**: `(user_id, user_type)` 파티션 키로 사용자별 권한 빠른 조회
- **관계 인덱스**: `(namespace, relation)` 파티션 키로 관계별 권한 빠른 조회

#### 쿼리 최적화
- **ALLOW FILTERING 제거**: 모든 쿼리가 파티션 키를 사용하도록 설계
- **배치 삽입**: 다중 테이블 동기화를 통한 일관성 보장
- **페이징 지원**: 대량 데이터 조회 시 메모리 효율성

### 7.2 캐싱 전략

#### Redis 캐싱
- **캐시 키**: `permission:${namespace}:${object_id}:${relation}:${user_type}:${user_id}`
- **TTL**: 5분 (권한 변경의 실시간성과 성능의 균형)
- **캐시 무효화**: 권한 변경 시 관련 캐시 자동 삭제

#### 메모리 캐싱
- **권한 계층구조**: 메모리에 상주하여 빠른 상속 관계 확인
- **방문 기록**: 순환 참조 방지를 위한 HashSet 사용

### 7.3 비동기 처리

#### 동시성 최적화
- **비동기 I/O**: tokio 기반 논블로킹 데이터베이스 접근
- **병렬 권한 확인**: futures::future::join_all로 다중 권한 동시 처리
- **연결 풀링**: ScyllaDB와 Redis 연결 풀 관리

## 8. 확장성 및 제한사항

### 8.1 확장성

#### 수평 확장
- **ScyllaDB 클러스터**: 노드 추가를 통한 용량 및 성능 확장
- **Redis 클러스터**: 캐시 용량 확장 가능
- **서비스 인스턴스**: 로드밸런서를 통한 다중 인스턴스 운영

#### 수직 확장
- **메모리 확장**: 더 많은 캐시 데이터 보관
- **CPU 확장**: 더 많은 동시 요청 처리

### 8.2 현재 제한사항

#### 단일 서버 환경 최적화
- **Request Hedging 미지원**: 동일 서버 내에서 불필요
- **다중 지역 복제 미지원**: 포트폴리오 규모에서는 과도한 복잡성
- **Leopard 인덱스 미적용**: 중첩 그룹 최적화 생략

#### 팀 서비스 연동
- **임시 구현**: 현재는 데이터베이스 직접 조회
- **향후 개선**: Team Service API 호출로 변경 예정

## 9. 보안 고려사항

### 9.1 접근 제어
- **내부 네트워크**: 외부 인터넷에서 직접 접근 불가
- **서비스 간 인증**: 현재는 신뢰된 내부 네트워크 가정

### 9.2 데이터 보안
- **권한 데이터 암호화**: ScyllaDB와 Redis에서 저장 시 암호화
- **로깅 보안**: 민감한 사용자 정보 로그 출력 방지

### 9.3 일관성 보장
- **Zookie 토큰**: 외부 일관성을 통한 "new enemy problem" 방지
- **원자적 연산**: 권한 변경 시 모든 인덱스 테이블 동기 업데이트

## 10. 모니터링 및 운영

### 10.1 로깅
```rust
// 권한 체크 로깅
info!("Permission check request: {}:{}#{} for user:{}", 
    namespace, object_id, relation, user_id);
info!("Permission check result: allowed={}", result.allowed);

// 권한 변경 로깅  
info!("Write request with {} tuple updates", updates.len());
info!("Inserted tuple: {}:{}#{} for {}:{}", 
    namespace, object_id, relation, user_type, user_id);
```

### 10.2 메트릭스
- **응답 시간**: 권한 체크 평균/P95/P99 응답 시간
- **처리량**: 초당 권한 체크 요청 수 (QPS)
- **캐시 적중률**: Redis 캐시 효율성
- **데이터베이스 연결**: ScyllaDB 연결 풀 상태

### 10.3 헬스체크
```bash
# 서비스 상태 확인
curl http://localhost:15004/health

# 데이터베이스 연결 확인
curl http://localhost:15004/db-test
curl http://localhost:15004/redis-test  
curl http://localhost:15004/scylla-test
```

## 11. 개발 가이드

### 11.1 로컬 개발 환경

#### 전체 서비스 시작
```bash
docker-compose up -d --build
```

#### Sentinel만 개발
```bash
# 의존성 서비스만 시작
docker-compose up -d scylladb sentinel-redis

# Sentinel 빌드 및 시작
cd back/sentinel
cargo build --release
SCYLLA_HOST=localhost REDIS_HOST=localhost PORT=15004 ./target/release/sentinel
```

### 11.2 테스트

#### 단위 테스트
```bash
cd back/sentinel
cargo test
```

#### 통합 테스트
```bash
# 기본 권한 테스트
curl -X POST http://localhost:15004/api/v1/check \
  -H "Content-Type: application/json" \
  -d '{"namespace": "document", "object_id": "test", "relation": "viewer", "user_id": "alice"}'

# 권한 생성 테스트  
curl -X POST http://localhost:15004/api/v1/write \
  -H "Content-Type: application/json" \
  -d '{
    "updates": [{
      "operation": "Insert",
      "tuple": {
        "namespace": "document", "object_id": "test", "relation": "owner",
        "user_type": "user", "user_id": "alice", "created_at": "2025-08-04T04:00:00Z"
      }
    }]
  }'
```

### 11.3 디버깅

#### 로그 확인
```bash
docker-compose logs sentinel
docker-compose logs sentinel -f  # 실시간 로그
```

#### 데이터베이스 직접 접근
```bash
# ScyllaDB 콘솔
docker exec -it sentinel-scylladb cqlsh

# 데이터 조회
USE sentinel;
SELECT * FROM relation_tuples;
SELECT * FROM user_memberships WHERE user_id = 'alice' AND user_type = 'user';
```

#### Redis 직접 접근
```bash
# Redis 콘솔
docker exec -it sentinel-redis redis-cli

# 캐시 데이터 확인
KEYS permission:*
GET permission:document:test:viewer:user:alice
```

## 12. 향후 개선 계획

### 12.1 Stage 4: 캐싱 레이어 고도화
- **분산 캐시**: Redis Cluster 지원
- **캐시 워밍**: 자주 사용되는 권한 미리 로드
- **캐시 계층화**: L1(메모리) + L2(Redis) 다단계 캐싱

### 12.2 Stage 5: Zookie 일관성 개선
- **벡터 클록**: 더 정교한 일관성 보장
- **스냅샷 격리**: 읽기 트랜잭션 지원
- **변경 추적**: 권한 변경 이력 완전 추적

### 12.3 Stage 6: 배치 처리 최적화
- **배치 체크**: 단일 요청으로 다중 권한 검증
- **병렬 처리**: 독립적 권한 검증 동시 실행
- **결과 집계**: 권한 검증 결과 통합 응답

### 12.4 Team Service 연동
- **HTTP 클라이언트**: Team Service REST API 호출
- **회로 차단기**: 서비스 장애 대응
- **폴백 메커니즘**: Team Service 다운 시 데이터베이스 직접 조회

### 12.5 추가 기능
- **권한 위임**: 일시적 권한 위임 기능
- **조건부 권한**: 시간/위치 기반 조건부 권한
- **감사 로그**: 모든 권한 변경 추적 및 감사
- **GraphQL API**: REST API 외 GraphQL 지원

---

## 참고 자료

- [Google Zanzibar Paper](https://research.google/pubs/pub48190/)
- [ScyllaDB Documentation](https://docs.scylladb.com/)
- [Actix Web Guide](https://actix.rs/docs/)
- [Rust Async Book](https://rust-lang.github.io/async-book/)

---

**Sentinel v1.0.0**  
*Google Zanzibar 기반 권한 관리 시스템*  
*ASDF 마이크로서비스 플랫폼의 핵심 구성요소*