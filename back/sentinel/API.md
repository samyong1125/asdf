# Sentinel API Documentation

**Sentinel**은 Google Zanzibar 논문을 기반으로 구현된 관계 기반 권한 관리 시스템입니다. 모든 마이크로서비스가 직접 호출하여 중앙집중식 권한 검증을 제공합니다.

## 기본 정보

- **Base URL**: `http://localhost:15004`
- **API Version**: `v1`
- **Content-Type**: `application/json`
- **Architecture**: API Gateway를 거치지 않는 독립 서비스

## 인증

Sentinel은 내부 마이크로서비스 전용 API로 별도 인증이 필요하지 않습니다. 프로덕션 환경에서는 네트워크 레벨에서 접근 제어를 권장합니다.

## 권한 튜플 형식

Sentinel은 다음과 같은 형식의 권한 튜플을 사용합니다:

```
<namespace:object_id#relation@user_type:user_id>
```

**예시:**
- `documents:doc123#viewer@user:alice` - alice가 doc123 문서를 볼 수 있음
- `teams:backend#member@user:bob` - bob이 backend 팀의 멤버임
- `projects:webapp#editor@teams:backend#member` - backend 팀 멤버들이 webapp 프로젝트를 편집할 수 있음

## Zookie (일관성 토큰)

Sentinel은 Zanzibar의 Zookie를 구현하여 "new enemy problem"을 방지합니다:

- **스냅샷 읽기**: 특정 시점의 일관성 있는 데이터 읽기
- **외부 일관성**: 쓰기 후 읽기 작업의 일관성 보장
- **Base64 인코딩**: 타임스탬프와 메타데이터를 포함한 토큰

## API 엔드포인트

### 1. 권한 검증 (Check)

사용자가 특정 객체에 대해 권한을 가지고 있는지 확인합니다.

#### Request
```http
POST /api/v1/check
Content-Type: application/json

{
  "namespace": "documents",
  "object_id": "doc123",
  "relation": "viewer",
  "user_id": "alice",
  "user_type": "user",
  "zookie": "eyJ0aW1lc3RhbXBfbWljcm9zIjoxNjk..."
}
```

#### Parameters
| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| `namespace` | string | 예 | 네임스페이스 (예: "documents", "teams") |
| `object_id` | string | 예 | 객체 ID (예: "doc123") |
| `relation` | string | 예 | 권한 관계 (예: "viewer", "editor") |
| `user_id` | string | 예 | 사용자 ID |
| `user_type` | string | 아니오 | 사용자 타입 (기본값: "user") |
| `zookie` | string | 아니오 | 일관성 토큰 |

#### Response
```json
{
  "allowed": true,
  "zookie": "eyJ0aW1lc3RhbXBfbWljcm9zIjoxNjk..."
}
```

#### 권한 계층 구조
- `owner` (레벨 5) → `admin`, `editor`, `commenter`, `viewer`
- `admin` (레벨 4) → `editor`, `commenter`, `viewer`  
- `editor` (레벨 3) → `commenter`, `viewer`
- `commenter` (레벨 2) → `viewer`
- `viewer` (레벨 1)

### 2. 권한 튜플 작성 (Write)

권한 튜플을 생성하거나 삭제합니다.

#### Request
```http
POST /api/v1/write
Content-Type: application/json

{
  "updates": [
    {
      "operation": "Insert",
      "tuple": {
        "namespace": "documents",
        "object_id": "doc123",
        "relation": "viewer",
        "user_type": "user",
        "user_id": "alice",
        "created_at": "2024-01-01T00:00:00Z"
      }
    },
    {
      "operation": "Delete",
      "tuple": {
        "namespace": "documents",
        "object_id": "doc456",
        "relation": "editor",
        "user_type": "user", 
        "user_id": "bob",
        "created_at": "2024-01-01T00:00:00Z"
      }
    }
  ]
}
```

#### Parameters
| 필드 | 타입 | 설명 |
|------|------|------|
| `updates` | array | 수행할 작업 목록 |
| `updates[].operation` | string | "Insert" 또는 "Delete" |
| `updates[].tuple` | object | 권한 튜플 정보 |

#### Response
```json
{
  "zookie": "eyJ0aW1lc3RhbXBfbWljcm9zIjoxNjk..."
}
```

### 3. 권한 튜플 조회 (Read)

저장된 권한 튜플을 조회합니다.

#### Request
```http
POST /api/v1/read
Content-Type: application/json

{
  "tuple_filter": {
    "namespace": "documents",
    "object_id": "doc123"
  },
  "zookie": "eyJ0aW1lc3RhbXBfbWljcm9zIjoxNjk...",
  "page_size": 100
}
```

#### Filter Options
| 필드 | 타입 | 설명 | 예시 |
|------|------|------|------|
| `namespace` + `object_id` | string | 특정 객체의 모든 권한 | `{"namespace": "documents", "object_id": "doc123"}` |
| `namespace` + `object_id` + `relation` | string | 특정 객체-관계의 모든 권한 | `{"namespace": "teams", "object_id": "backend", "relation": "member"}` |
| `user_id` | string | 특정 사용자의 모든 권한 | `{"user_id": "alice"}` |

#### Response
```json
{
  "tuples": [
    {
      "namespace": "documents",
      "object_id": "doc123",
      "relation": "viewer",
      "user_type": "user",
      "user_id": "alice",
      "created_at": "2024-01-01T00:00:00Z"
    }
  ],
  "next_page_token": null,
  "zookie": "eyJ0aW1lc3RhbXBfbWljcm9zIjoxNjk..."
}
```

### 4. 배치 권한 검증 (Batch Check)

여러 권한을 한 번에 검증합니다. 병렬 처리와 중복 제거를 통해 성능을 최적화합니다.

#### Request
```http
POST /api/v1/batch_check
Content-Type: application/json

{
  "checks": [
    {
      "namespace": "documents",
      "object_id": "doc123",
      "relation": "viewer",
      "user_id": "alice"
    },
    {
      "namespace": "documents", 
      "object_id": "doc456",
      "relation": "editor",
      "user_id": "alice"
    }
  ],
  "zookie": "eyJ0aW1lc3RhbXBfbWljcm9zIjoxNjk..."
}
```

#### Response
```json
{
  "results": [
    {
      "request_index": 0,
      "allowed": true,
      "request_info": "documents:doc123#viewer@alice"
    },
    {
      "request_index": 1,
      "allowed": false,
      "request_info": "documents:doc456#editor@alice"
    }
  ],
  "total_requests": 2,
  "allowed_count": 1,
  "denied_count": 1,
  "zookie": "eyJ0aW1lc3RhbXBfbWljcm9zIjoxNjk..."
}
```

## 디버그 API

### 1. 사용자 권한 조회

특정 사용자가 가진 모든 권한을 조회합니다.

```http
GET /api/v1/users/{user_id}/permissions
```

#### Response
```json
{
  "user_id": "alice",
  "permissions": [
    {
      "namespace": "documents",
      "object_id": "doc123", 
      "relation": "viewer",
      "user_type": "user",
      "user_id": "alice",
      "created_at": "2024-01-01T00:00:00Z"
    }
  ],
  "count": 1
}
```

### 2. 객체 권한 조회

특정 객체에 설정된 모든 권한을 조회합니다.

```http
GET /api/v1/objects/{namespace}/{object_id}/permissions
```

#### Response
```json
{
  "namespace": "documents",
  "object_id": "doc123",
  "permissions": [
    {
      "namespace": "documents",
      "object_id": "doc123",
      "relation": "owner",
      "user_type": "user", 
      "user_id": "bob",
      "created_at": "2024-01-01T00:00:00Z"
    }
  ],
  "count": 1
}
```

## 헬스체크

### 서비스 상태 확인
```http
GET /health
```

### 데이터베이스 연결 테스트
```http
GET /db-test        # 모든 DB 연결 테스트
GET /scylla-test    # ScyllaDB 연결 테스트
GET /redis-test     # Redis 연결 테스트
GET /cache-test     # 캐시 연결 테스트
```

## 사용 예시

### 1. 문서 소유자 설정
```bash
curl -X POST http://localhost:15004/api/v1/write \
  -H "Content-Type: application/json" \
  -d '{
    "updates": [
      {
        "operation": "Insert",
        "tuple": {
          "namespace": "documents",
          "object_id": "doc123",
          "relation": "owner", 
          "user_type": "user",
          "user_id": "alice",
          "created_at": "2024-01-01T00:00:00Z"
        }
      }
    ]
  }'
```

### 2. 팀 기반 권한 설정
```bash
# 1. 팀 멤버십 설정
curl -X POST http://localhost:15004/api/v1/write \
  -H "Content-Type: application/json" \
  -d '{
    "updates": [
      {
        "operation": "Insert",
        "tuple": {
          "namespace": "teams",
          "object_id": "backend",
          "relation": "member",
          "user_type": "user",
          "user_id": "bob",
          "created_at": "2024-01-01T00:00:00Z"
        }
      }
    ]
  }'

# 2. 팀에 문서 편집 권한 부여
curl -X POST http://localhost:15004/api/v1/write \
  -H "Content-Type: application/json" \
  -d '{
    "updates": [
      {
        "operation": "Insert", 
        "tuple": {
          "namespace": "documents",
          "object_id": "api-spec",
          "relation": "editor",
          "user_type": "userset",
          "user_id": "teams:backend#member",
          "created_at": "2024-01-01T00:00:00Z"
        }
      }
    ]
  }'
```

### 3. 권한 확인
```bash
curl -X POST http://localhost:15004/api/v1/check \
  -H "Content-Type: application/json" \
  -d '{
    "namespace": "documents",
    "object_id": "api-spec",
    "relation": "editor", 
    "user_id": "bob"
  }'
```

## 성능 및 캐싱

### 캐시 전략
- **권한 체크 결과**: 5분 TTL로 Redis 캐싱
- **사용자 권한 목록**: 10분 TTL
- **객체 권한 목록**: 10분 TTL

### 성능 최적화
- **배치 처리**: 중복 요청 자동 제거
- **병렬 처리**: futures::join_all로 동시 실행
- **인덱스 테이블**: 4개 최적화된 ScyllaDB 테이블

### 캐시 무효화
권한 변경 시 관련 캐시가 자동으로 무효화됩니다:
- 사용자별 캐시: `check:*@user:{user_id}`
- 객체별 캐시: `check:{namespace}:{object_id}*`
- 네임스페이스별 캐시: `check:{namespace}:*`

## 오류 처리

### HTTP 상태 코드
- `200` - 성공
- `400` - 잘못된 요청 (검증 오류, 직렬화 오류)
- `403` - 권한 오류
- `500` - 내부 서버 오류 (데이터베이스, 캐시 오류)

### 오류 응답 형식
```json
{
  "error": "Validation error",
  "message": "Invalid zookie encoding: Invalid character"
}
```

## 일관성 보장

### Zookie 작동 원리
1. **쓰기 작업** 후 새로운 Zookie 반환
2. **읽기 작업** 시 해당 Zookie 사용으로 일관성 보장
3. **스냅샷 읽기**로 특정 시점 데이터 접근

### New Enemy Problem 방지
```
1. 사용자 A가 문서 X에 사용자 B를 editor로 추가 → Zookie Z1 반환
2. 사용자 B가 Zookie Z1과 함께 문서 X 편집 권한 확인 → 일관성 보장됨
3. Zookie 없이 확인 시 → 캐시로 인해 권한이 아직 반영되지 않을 수 있음
```

## 제한사항

### 현재 구현에서 제외된 기능
- **Request Hedging**: 단일 서버 환경으로 불필요
- **다중 지역 복제**: 포트폴리오 규모에서 과도한 복잡성
- **Leopard 인덱스**: 중첩 그룹 최적화 (향후 구현 예정)

### 권장사항
- **페이징**: 대량 데이터 조회 시 `page_size` 사용
- **배치 처리**: 다중 권한 확인 시 `/batch_check` 사용
- **Zookie 사용**: 일관성이 중요한 작업에서 필수
- **캐시 고려**: 권한 변경 후 캐시 무효화 시간(최대 5분) 고려

---

**참고**: 이 API는 Google Zanzibar 논문의 핵심 개념을 구현한 포트폴리오 프로젝트입니다. 실제 프로덕션 환경에서는 추가적인 보안 및 확장성 고려사항이 필요할 수 있습니다.