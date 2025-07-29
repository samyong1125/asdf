# Envoy API Gateway 설정 가이드

## 개요
이 프로젝트는 Envoy Proxy를 API Gateway로 사용하여 JWT 기반 인증을 구현한 시스템입니다.

## 아키텍처
```
Client → Envoy Gateway (15000) → Auth Server (15001)
                               → Backend Services
```

## 포트 구성
- **Database Layer**: 50000번대
  - PostgreSQL: 50001
  - Redis: 50002
- **Application Layer**: 15000번대
  - API Gateway: 15000 (Envoy)
  - Auth Server: 15001 (Go)
  - Backend Services: 15002~ (추가 서비스용)
- **Admin**: 
  - Envoy Admin: 9901

## 인증 플로우

### 1. 사용자 인증
```
1. Client → POST /api/v1/login → Auth Server (15001)
2. Auth Server → JWT 토큰 생성 및 반환
3. Client → Authorization: Bearer <token> → Gateway (15000)
4. Gateway → GET /api/v1/verify → Auth Server (15001)
5. Auth Server → 토큰 검증 후 사용자 헤더 추가 (X-User-ID, X-User-Email)
6. Gateway → Backend Service (인증된 요청 전달)
```

### 2. JWT 토큰 구조
- **Access Token**: 15분 유효, JWT 형태
- **Refresh Token**: 180일 유효, Redis에 저장

## Envoy 설정 주요 포인트

### 1. HTTP Filters 순서 (중요!)
```yaml
http_filters:
  - name: envoy.filters.http.cors      # CORS 처리
  - name: envoy.filters.http.ext_authz # 인증 처리
  - name: envoy.filters.http.router    # 라우팅 처리
```

### 2. 인증 필터 설정
```yaml
- name: envoy.filters.http.ext_authz
  typed_config:
    "@type": type.googleapis.com/envoy.extensions.filters.http.ext_authz.v3.ExtAuthz
    transport_api_version: V3
    http_service:
      server_uri:
        uri: http://auth-server:15001
        cluster: auth_service
        timeout: 2s
      path_prefix: /api/v1/verify  # 인증 서버의 검증 엔드포인트
      authorization_request:
        allowed_headers:
          patterns:
          - exact: authorization
          - exact: content-type
      authorization_response:
        allowed_upstream_headers:
          patterns:
          - exact: x-user-id
          - exact: x-user-email
    failure_mode_allow: false
```

### 3. 라우팅 설정

#### 인증 제외 경로 (Health Check)
```yaml
- match:
    path: "/health"
  direct_response:
    status: 200
    body:
      inline_string: "Gateway is running"
  typed_per_filter_config:
    envoy.filters.http.ext_authz:
      "@type": type.googleapis.com/envoy.extensions.filters.http.ext_authz.v3.ExtAuthzPerRoute
      disabled: true  # 이 경로는 인증 건너뛰기
```

#### 인증 필요 경로
```yaml
- match:
    prefix: "/api/protected/"
  route:
    cluster: backend_service
  typed_per_filter_config:
    envoy.filters.http.ext_authz:
      "@type": type.googleapis.com/envoy.extensions.filters.http.ext_authz.v3.ExtAuthzPerRoute
      disabled: false  # 이 경로는 인증 필수
```

### 4. 클러스터 설정
```yaml
clusters:
- name: auth_service
  connect_timeout: 2s
  type: LOGICAL_DNS
  dns_lookup_family: V4_ONLY
  lb_policy: ROUND_ROBIN
  load_assignment:
    cluster_name: auth_service
    endpoints:
    - lb_endpoints:
      - endpoint:
          address:
            socket_address:
              address: auth-server  # Docker Compose 서비스명
              port_value: 15001
```

## 새로운 백엔드 서비스 추가하기

### 1. Docker Compose에 서비스 추가
```yaml
  new-service:
    build:
      context: ./back/new-service
      dockerfile: Dockerfile
    container_name: new-service
    ports:
      - "15003:15003"
    environment:
      - PORT=15003
    networks:
      - asdf
```

### 2. Envoy에 라우팅 추가
```yaml
# envoy.yaml의 routes 섹션에 추가
- match:
    prefix: "/api/new-service/"
  route:
    cluster: new_service_cluster
  response_headers_to_add:
  - header:
      key: "Access-Control-Allow-Origin"
      value: "*"
  # 인증이 필요한 경우:
  typed_per_filter_config:
    envoy.filters.http.ext_authz:
      "@type": type.googleapis.com/envoy.extensions.filters.http.ext_authz.v3.ExtAuthzPerRoute
      disabled: false
```

### 3. Envoy에 클러스터 추가
```yaml
# envoy.yaml의 clusters 섹션에 추가
- name: new_service_cluster
  connect_timeout: 2s
  type: LOGICAL_DNS
  dns_lookup_family: V4_ONLY
  lb_policy: ROUND_ROBIN
  load_assignment:
    cluster_name: new_service_cluster
    endpoints:
    - lb_endpoints:
      - endpoint:
          address:
            socket_address:
              address: new-service
              port_value: 15003
```

### 4. Gateway 의존성 업데이트
```yaml
# docker-compose.yml의 gateway 서비스
depends_on:
  - auth-server
  - new-service  # 추가
```

## 주의사항 및 트러블슈팅

### 1. 인증 서버 설정
- 인증 서버는 `/api/v1/verify` 엔드포인트에서 **정확히 `/api/v1/verify`와 `/api/v1/verify/*path` 모두** 처리해야 함
- Envoy가 `path_prefix`를 사용하면 원본 경로가 붙어서 전달됨
- 성공시 **반드시 200 OK**와 사용자 헤더(`X-User-ID`, `X-User-Email`) 반환
- 실패시 401 반환 (Envoy가 자동으로 403으로 변환)

### 2. CORS 설정
- 브라우저에서 접근하려면 CORS 헤더가 필요함
- Virtual Host 레벨과 Route 레벨에서 모두 설정
- OPTIONS 요청도 처리해야 함

### 3. 네트워킹
- 모든 서비스는 같은 Docker 네트워크(`asdf`)에 있어야 함
- 서비스 간 통신은 컨테이너명 사용 (예: `auth-server:15001`)
- 외부 포트와 내부 포트를 일치시키는 것이 혼란 방지에 도움됨

### 4. 디버깅
- Envoy Admin 포트(9901)에서 통계 확인: `curl localhost:9901/stats | grep ext_authz`
- 로그 확인: `docker-compose logs gateway`
- 인증 서버 로그: `docker-compose logs auth-server`

### 5. 일반적인 에러들

#### "UAEX" 에러
- Unauthorized External Service
- 인증 실패를 의미
- 인증 서버의 `/verify` 엔드포인트 확인

#### 404 Not Found
- 라우팅 설정 문제
- `match` 조건이 요청 경로와 맞지 않음

#### Connection refused
- 서비스가 실행되지 않거나 포트가 틀림
- Docker Compose의 서비스명과 포트 확인

## 설정 파일 위치
- **Envoy 설정**: `back/gateway/envoy.yaml`
- **Docker 설정**: `docker-compose.yml`
- **인증 서버**: `back/auth/src/`

## 개발 워크플로우
1. 새 서비스 개발
2. Docker Compose에 서비스 추가
3. Envoy에 라우팅/클러스터 추가
4. 의존성 업데이트
5. `docker-compose up -d --build` 로 재시작
6. 테스트

## 보안 고려사항
- JWT Secret은 환경변수로 관리
- HTTPS 사용 권장 (프로덕션)
- Rate limiting 고려
- 로그에서 민감한 정보 제거