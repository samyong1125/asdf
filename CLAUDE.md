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
- **인증 시스템** (현재 구현됨): JWT 기반 사용자 인증 및 권한 관리
- **User 서비스** (구현 중): 사용자 관리 및 프로필 기능
- **Team 서비스** (계획): 팀 관리
- **개인 블로그**: 개인 블로그 플랫폼
- **클라우드 서비스**: 개인 클라우드 스토리지 및 관리
- **Zanzibar 스타일 권한 관리**: Google Zanzibar를 모델로 한 관계 기반 권한 시스템 직접 구현
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
- **Database**: PostgreSQL (포트 50001) - 사용자 데이터 저장
- **Cache**: Redis (포트 50002) - 리프레시 토큰 저장

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

## 파일 구조

- `back/auth/src/` - Go 인증 서비스
- `back/gateway/envoy.yaml` - Envoy 프록시 설정
- `db/init/` - 데이터베이스 초기화 스크립트
- `docker-compose.yml` - 서비스 오케스트레이션