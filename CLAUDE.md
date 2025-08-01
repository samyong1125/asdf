# CLAUDE.md

이 파일은 Claude Code (claude.ai/code)가 이 저장소에서 작업할 때 필요한 가이드를 제공합니다.

**중요**: Claude는 항상 한국어로 응답해야 합니다. 모든 설명, 오류 메시지, 코멘트는 한국어로 작성하세요.

## 프로젝트 개요

**asdf**는 다양한 기술 스택을 활용한 포트폴리오 프로젝트입니다. 마이크로서비스 아키텍처를 기반으로 여러 서비스가 통합된 플랫폼을 구축하는 것이 목표입니다.

### 프로젝트 목적
- **포트폴리오**: 다양한 기술 스택과 아키텍처 설계 능력을 보여주기 위한 프로젝트
- **확장성**: 추후 다양한 서비스를 추가할 수 있도록 설계된 플랫폼
- **기술 과시**: 최신 기술과 복잡한 시스템 구현을 통한 기술력 증명

### 계획된 서비스들
- **인증 시스템** (구현됨): JWT 기반 사용자 인증 및 권한 관리
- **User 서비스** (구현 중): 사용자 관리 및 프로필 기능
- **Team 서비스** (구현 중): 팀 관리
- **Sentinel** (구현 중): Google Zanzibar 스타일 권한 관리 시스템
- **개인 블로그**: 개인 블로그 플랫폼
- **클라우드 서비스**: 개인 클라우드 스토리지 및 관리
- **추가 서비스들**: 지속적인 확장 예정

### 기술적 특징
- **마이크로서비스 아키텍처**: 각 서비스별 독립적 개발 및 배포
- **API Gateway 패턴**: Envoy Proxy를 통한 중앙집중식 라우팅 및 인증
- **다양한 기술 스택**: 포트폴리오 목적에 맞춰 여러 기술 혼용 예정
- **복잡한 권한 관리**: Zanzibar 모델 기반의 정교한 권한 시스템

## 아키텍처 개요

**Envoy Proxy**를 API Gateway로 사용하는 JWT 기반 인증 마이크로서비스 시스템입니다. 시스템 구성:

- **API Gateway**: Envoy Proxy (포트 15000) - 요청 라우팅 및 JWT 인증 처리
- **Auth Server**: Go/Gin 서비스 (포트 15001) - 사용자 인증 및 JWT 검증 처리
- **User Service**: Python/FastAPI (포트 15002) - 사용자 프로필 관리
- **Team Service**: Kotlin/Ktor (포트 15003) - 팀 관리
- **Sentinel**: Rust/Actix Web (포트 15004) - Zanzibar 스타일 권한 관리
- **PostgreSQL**: (포트 50001) - Auth/User 서비스 데이터 저장
- **Redis**: (포트 50002) - Auth 서비스 리프레시 토큰 저장
- **MongoDB**: (포트 50003) - Team 서비스 데이터 저장
- **ScyllaDB**: (포트 50004-50005) - Sentinel 권한 튜플 저장
- **Sentinel Redis**: (포트 50006) - Sentinel 권한 캐싱

### 주요 아키텍처 개념

1. **JWT 인증 플로우**: 클라이언트 → 게이트웨이 → 인증 검증 → 백엔드 서비스
2. **Envoy ext_authz 필터**: 모든 요청(헬스체크 제외)은 `/api/v1/verify` 엔드포인트를 통해 검증
3. **사용자 컨텍스트 전파**: 검증된 요청에는 `X-User-ID`, `X-User-Email` 헤더 포함
4. **포트 할당**: 데이터베이스 레이어(50000번대), 애플리케이션 레이어(15000번대)

## 개발 명령어

### 시스템 시작
```bash
docker-compose up -d --build    # 모든 서비스 시작
docker-compose logs -f         # 로그 확인
docker-compose down            # 모든 서비스 중지
```

### 개별 서비스 관리
```bash
# Auth 서버만 실행
docker-compose up auth-server -d --build

# Gateway만 실행
docker-compose up gateway -d --build

# 데이터베이스 서비스만 실행
docker-compose up postgres redis -d
```

### Go 서비스 빌드
```bash
# Auth 서버 빌드
cd back/auth
go build -o auth-server ./src

# 로컬 실행 (환경변수 필요)
go run ./src
```

## 새로운 백엔드 서비스 추가

아키텍처에 새 서비스를 추가할 때:

1. **docker-compose.yml에 추가** - 15002+ 포트 범위 사용
2. **Envoy 설정 업데이트** (`back/gateway/envoy.yaml`):
   - `routes` 섹션에 적절한 경로 prefix로 라우트 추가
   - 서비스 엔드포인트로 클러스터 정의 추가
   - `typed_per_filter_config`를 사용하여 인증 요구사항 설정
3. **게이트웨이 의존성 업데이트** - 새 서비스 포함

### 인증 요구사항

- **인증 건너뛰기**: `ext_authz` 필터 설정에서 `disabled: true` 설정
- **인증 필수**: `disabled: false` 설정하거나 생략
- **인증된 요청**은 자동으로 `X-User-ID`, `X-User-Email` 헤더 수신

## 데이터베이스 스키마

- **PostgreSQL**: 이메일/비밀번호 인증을 통한 사용자 관리
- **Redis**: TTL(180일) 설정된 리프레시 토큰 저장
- **초기화**: `db/init/`의 SQL 스크립트가 컨테이너 시작 시 자동 실행

## 환경 설정

Auth 서버의 주요 환경변수:
- 데이터베이스: `DB_HOST`, `DB_PORT`, `DB_USER`, `DB_PASSWORD`, `DB_NAME`
- Redis: `REDIS_HOST`, `REDIS_PORT`  
- JWT: `JWT_SECRET`
- 서버: `PORT` (기본값: 15001)

## API 엔드포인트

### Auth Server (`/api/v1/`)
- `POST /register` - 사용자 등록
- `POST /login` - 사용자 로그인
- `POST /refresh` - 액세스 토큰 갱신
- `POST /logout` - 사용자 로그아웃
- `GET /verify` - 토큰 검증 (Envoy에서 사용)

### Gateway
- `GET /health` - 헬스체크 (인증 불필요)
- 기타 모든 경로는 JWT Bearer 토큰을 통한 인증 필요

## 보안 고려사항

- JWT 액세스 토큰은 15분 후 만료
- 리프레시 토큰은 Redis에 180일 TTL로 저장
- 브라우저 접근을 위한 CORS 설정
- 모든 인증 플로우에서 bcrypt를 사용한 비밀번호 해싱
- Envoy에서 속도 제한 및 요청 검증 처리

## 디버깅

- **Envoy Admin**: http://localhost:9901 에서 통계 및 설정 확인
- **Auth Server 로그**: `docker-compose logs auth-server`
- **Gateway 로그**: `docker-compose logs gateway`
- **데이터베이스 접근**: localhost:50001에서 PostgreSQL 연결

## Sentinel - Zanzibar 권한 관리 시스템

### 개요
**Sentinel**은 Google Zanzibar 논문을 기반으로 구현한 관계 기반 권한 관리 시스템입니다. 
API Gateway를 거치지 않는 독립적인 서비스로, 모든 마이크로서비스가 직접 호출하여 권한을 검증합니다.

### 핵심 역할
- **중앙집중식 권한 관리**: 모든 객체(사용자, 팀, 문서, 프로젝트 등)의 권한 관계 관리
- **관계 기반 권한**: `<object#relation@user>` 형태의 튜플로 권한 표현
- **Userset 지원**: 팀 멤버십을 활용한 간접 권한 부여
- **권한 상속**: 복잡한 권한 계층 구조 지원

### 기술 스택
- **언어**: Rust
- **웹 프레임워크**: Actix Web
- **주 데이터베이스**: ScyllaDB (권한 튜플 저장)
- **캐시**: Redis (권한 검증 결과 캐싱)
- **포트**: 15004

### 권한 튜플 예시
```rust
// 사용자 소유 권한
doc:my-notes#owner@user:alice
doc:my-notes#viewer@user:bob

// 팀 기반 권한 (Userset)
team:backend#member@user:alice
doc:api-spec#editor@team:backend#member

// 권한 상속
project:mobile-app#viewer@team:engineering#member
```

### API 엔드포인트 (구현 예정)
- `POST /api/v1/check` - 권한 검증
- `POST /api/v1/write` - 권한 튜플 생성/삭제
- `GET /api/v1/read` - 권한 튜플 조회
- `POST /api/v1/expand` - 권한 트리 조회
- `POST /api/v1/batch_check` - 다중 권한 검증

### 상세 구현 계획

#### 1단계: 기본 데이터 구조 및 모델 정의
- RelationTuple, CheckRequest 등 핵심 Rust 구조체 정의
- Serde 직렬화/역직렬화 지원
- Operation enum (INSERT, DELETE) 정의
- 각종 Request/Response 구조체 구현

#### 2단계: ScyllaDB 연동 레이어  
- TupleStore 구현 (insert_tuple, delete_tuple, find_direct_tuple 등)
- 비동기 데이터베이스 접근 최적화
- 복잡한 쿼리 함수들 (find_tuples_by_object, find_user_memberships)
- 인덱싱 전략 및 성능 최적화

#### 3단계: 핵심 Zanzibar API
- PermissionChecker: 권한 검증 엔진 (재귀적 userset 지원)
- API 핸들러: check, write, read, expand 엔드포인트
- 직접 튜플 확인 + userset 재귀 확인 + 권한 상속 로직
- 그룹 멤버십 확인 (Team Service와의 연동)

#### 4단계: 캐싱 레이어
- Redis 기반 권한 체크 결과 캐싱
- 캐시 키 전략 및 TTL 관리 (5분 TTL)
- Cache trait 정의 및 RedisCache 구현
- 중복 제거 및 성능 최적화

#### 5단계: Zookie 일관성 보장
- 타임스탬프 기반 단순화된 zookie 구현 (Google Spanner TrueTime 대신)
- 스냅샷 읽기 지원으로 "new enemy problem" 방지
- ZookieManager 구조체 및 generate_zookie/parse_zookie 함수
- 외부 일관성 보장 메커니즘

#### 6단계: 배치 처리 및 최적화
- 다중 권한 체크 API (검색 최적화용)
- 병렬 처리로 성능 향상 (futures::future::join_all 활용)
- BatchCheckRequest 구조체 및 핸들러
- 단일 서버 환경에 최적화 (Request Hedging 제외)

#### 7단계: 테스트 및 검증
- 기본 권한 플로우 테스트 (소유권, 직접 권한)
- Userset 상속 테스트 (팀 멤버십 기반 권한)
- 성능 벤치마크 및 부하 테스트
- 통합 테스트 시나리오 작성

### 현재 구현 상태
✅ **1단계 완료**: 
- Redis 인프라 구축 (포트 50006)
- ScyllaDB 연결 및 스키마 초기화
- 기본 서버 구조 (Actix Web)
- 연결 테스트 엔드포인트들

🎯 **다음 작업**: 2단계 기본 데이터 구조 정의부터 시작

### Google Zanzibar Leopard 인덱스 (참고용)

**Leopard**는 Zanzibar의 중첩된 그룹과 간접 관계를 빠르게 평가하기 위한 특화된 인덱스 시스템입니다.

#### 사용 사례
- 그룹 안에 또 다른 그룹이 들어있는 중첩 그룹
- 간접적인 멤버십 확인이 필요한 경우  
- 성능 병목이 생길 수 있는 권한 그래프 평가

#### 인덱스 구조
Leopard는 (T, s, e) 튜플들을 저장:

| 요소 | 의미 |
|------|------|
| `T` | 튜플 타입 (GROUP→GROUP, MEMBER→GROUP) |
| `s` | source ID (예: 그룹 ID, 사용자 ID 등) |
| `e` | element ID (예: 자식 그룹, 부모 그룹 등) |

**예시 튜플**:
- `GROUP→GROUP(g1, g2)`: 그룹 g1은 g2의 상위 그룹 (g2 ∈ g1)
- `MEMBER→GROUP(u1, g1)`: 사용자 u1은 그룹 g1의 직접 멤버

#### 쿼리 평가 방식
예시: user:alice가 group:dev에 속하는가?
```
MEMBER→GROUP(alice) ∩ GROUP→GROUP(dev) ≠ ∅
```
- alice가 속한 그룹들 (직접)
- dev의 하위 그룹들 (직·간접)  
- 교집합이 존재하면 권한 있음

그래프의 reachability 문제를 집합의 교집합 문제로 단순화.

#### 구성 요소
**오프라인 인덱서**:
- 전체 relation tuple 데이터를 읽어 인덱스 생성
- Userset rewrite 규칙을 따라 ACL 그래프를 재귀적으로 전개
- 전 세계적으로 샤드로 분산 저장 및 복제

**인크리멘털 업데이트**:
- 실시간 Watch API로 변경 감지
- (T, s, e, t, d) 형태로 업데이트 튜플 수신 (t: timestamp, d: 삭제 여부)
- 오프라인 인덱스 + 인크리멘털 인덱스를 머지하여 최신 상태 유지

**저장 구조**:
- Skip list 기반의 리스트로 저장
- 교집합, 합집합 등 set 연산이 매우 빠름 (O(min(|A|, |B|)))

#### 성능 지표 (논문 기준)
- 평균 1.56M QPS, 99% 지연 1ms 이하
- 인크리멘털 레이어는 초당 수천 개 업데이트 처리
- 전체 인덱스는 메모리 기반 skip list로 고속 처리

#### 장점과 한계
**장점**:
- 중첩된 그룹이나 상속 구조에서도 낮은 지연
- 복잡한 userset rewrite를 미리 계산해 저장
- 다수 사용자 또는 그룹 대상 쿼리에 특화됨

**한계**:
- 오프라인 인덱스는 최신 변경사항을 반영하지 못함 (인크리멘털 레이어로 보완)
- 인덱스 크기가 클 경우 메모리 압박 발생 가능

### 제외된 기능들 (단일 서버 환경)
- **Request Hedging** (동일 서버 내 중복 요청으로 인한 리소스 낭비)
- **다중 지역 복제** (Google의 30+ 데이터센터 환경 불필요)
- **Leopard 인덱스** (포트폴리오 규모에서는 과도한 복잡성, 하지만 참고용으로 위에 기록)

### 서비스 간 통신 패턴
```
User Service → "user:alice can edit user:bob?" → Sentinel
Team Service → "user:alice can admin team:backend?" → Sentinel  
Blog Service → "user:alice can write blog:post123?" → Sentinel
```

### 데이터베이스 스키마
**ScyllaDB Tables**:
- `relation_tuples`: Zanzibar 권한 튜플 저장
- `namespaces`: 네임스페이스 설정 저장
- `changelog`: 권한 변경 추적

**Redis Cache**: 권한 검증 결과 캐싱 (TTL 설정)

### 중요한 기술적 결정사항

#### Zanzibar 핵심 개념 적용
- **관계 튜플**: `<object#relation@user>` 형태로 모든 권한 표현
- **Userset**: `@team:backend#member` 형태로 간접 권한 부여
- **권한 상속**: userset_rewrite 규칙으로 editor → viewer 자동 포함
- **외부 일관성**: zookie 토큰으로 "new enemy problem" 방지

#### 아키텍처 특징
- **독립적 서비스**: API Gateway 우회, 다른 서비스들이 직접 호출
- **Team Service 연동**: 팀 멤버십을 userset으로 활용
- **다중 DB 전략**: 각 서비스별 최적화된 데이터베이스 사용
- **캐싱 전략**: Redis 기반 권한 검증 결과 캐싱 (5분 TTL)

#### 성능 최적화 방향
- **배치 처리**: 검색 결과 등에서 다중 권한 동시 검증
- **병렬 처리**: futures::future::join_all로 동시 권한 체크
- **캐시 활용**: 중복 권한 검증 요청 최소화
- **인덱싱**: ScyllaDB 복합 인덱스로 쿼리 최적화

#### 일관성 보장 전략
- **스냅샷 읽기**: zookie 기반 특정 시점 데이터 읽기
- **타임스탬프 관리**: chrono::Utc 기반 microsecond 정밀도
- **변경 추적**: changelog 테이블로 모든 권한 변경사항 기록
- **캐시 무효화**: 권한 변경 시 관련 캐시 자동 삭제

### 개발 명령어
```bash
# Sentinel 서비스만 실행
docker-compose up -d scylladb sentinel-redis sentinel-server

# 연결 테스트
curl http://localhost:15004/health        # 헬스체크
curl http://localhost:15004/db-test       # 모든 DB 연결 테스트
curl http://localhost:15004/redis-test    # Redis 연결 테스트
curl http://localhost:15004/scylla-test   # ScyllaDB 연결 테스트

# 로그 확인
docker-compose logs sentinel-server

# 빌드 및 재시작
docker-compose build sentinel-server && docker-compose up -d sentinel-server
```

### 개발 참고사항
- **Rust Edition**: 2024 사용
- **ScyllaDB Driver**: scylla 1.3.1, query_unpaged() 메서드 사용
- **Redis Driver**: redis 0.32.4, tokio-comp 기능 활성화
- **로깅**: tracing-subscriber 단독 사용 (env_logger와 충돌 방지)
- **환경변수**: SCYLLA_HOST, REDIS_HOST, PORT 등 docker-compose에서 주입

## 파일 구조

- `back/auth/src/` - Go 인증 서비스
- `back/services/user/src/` - Python 사용자 서비스
- `back/services/team/src/` - Kotlin 팀 서비스
- `back/sentinel/src/` - Rust Sentinel 권한 관리 서비스
- `back/gateway/envoy.yaml` - Envoy 프록시 설정
- `db/init/` - 데이터베이스 초기화 스크립트
- `docker-compose.yml` - 서비스 오케스트레이션