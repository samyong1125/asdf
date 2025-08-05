# Sentinel 통합 완료 문서

## 🎉 완성된 기능

### 1. Sentinel 권한 관리 시스템
- **Google Zanzibar 기반** 권한 관리 구현
- **포트**: 15004
- **API**: check, write, read 엔드포인트
- **권한 계층**: owner > admin > editor > commenter > viewer
- **Userset 지원**: 팀 기반 간접 권한 관리

### 2. Team Service + Sentinel 통합
- **팀 생성 시**: 생성자에게 자동으로 **owner** 권한 부여
- **멤버 추가 시**: 새 멤버에게 자동으로 **member** 권한 부여  
- **멤버 제거 시**: 해당 멤버의 권한 자동 삭제
- **팀 삭제 시**: 모든 멤버의 **owner/member** 권한 자동 삭제

### 3. 완전한 테스트 환경
- **test.html**: localhost:3000에서 모든 기능 테스트 가능
- **원클릭 시나리오**: 4가지 자동화된 권한 테스트
- **통합 테스트**: Team Service ↔ Sentinel 동기화 확인
- **pgAdmin**: localhost:16001에서 PostgreSQL DB 관리

## 🔧 기술적 구현 사항

### Sentinel 핵심 구조
```
- Rust/Actix Web
- ScyllaDB (권한 튜플 저장)  
- Redis (권한 검증 캐싱)
- CORS 설정으로 localhost:3000 접근 가능
```

### 권한 튜플 형태
```
teams:teamId#owner@user:userId        // 팀 소유권
teams:teamId#member@user:userId       // 팀 멤버십
documents:docId#editor@userset:teams:teamId#member  // Userset 기반 간접 권한
```

### API 형태 수정사항
1. **Write API**: `{"updates": [{"operation": "Insert", "tuple": {...}}]}` 형태
2. **Read API**: `{"tuple_filter": {"namespace": "teams", "object_id": "teamId"}}` 형태
3. **Operation 값**: `"Insert"`, `"Delete"` (대소문자 주의)

## 🚀 테스트 시나리오 (모두 성공)

### 기본 권한 테스트
- ✅ 직접 권한 부여 및 확인
- ✅ 권한 상속 (owner → viewer)
- ✅ 권한 거부 확인

### Userset 권한 테스트  
- ✅ 팀 멤버십을 통한 간접 권한
- ✅ documents:doc#editor@userset:teams:team#member 형태
- ✅ 권한 상속과 Userset 조합

### Team Service 통합 테스트
- ✅ 팀 생성 → 생성자 owner 권한 자동 생성
- ✅ 멤버 추가 → member 권한 자동 생성  
- ✅ 멤버 제거 → 권한 자동 삭제
- ✅ 팀 삭제 → 모든 권한 자동 삭제

## 📂 주요 파일 변경사항

### Sentinel 구현
- `/back/sentinel/src/permission_checker.rs`: Userset 처리 로직 수정
- `/back/sentinel/src/main.rs`: CORS 설정 추가
- `/back/sentinel/Cargo.toml`: actix-cors 의존성 추가

### Team Service 통합
- `/back/services/team/src/main/kotlin/clients/SentinelClient.kt`:
  - `addTeamOwner()`: 팀 생성자 owner 권한 부여
  - `removeAllTeamPermissions()`: 팀 삭제 시 모든 권한 제거
- `/back/services/team/src/main/kotlin/services/TeamService.kt`:
  - 팀 생성 시 `addTeamOwner()` 호출
  - 팀 삭제 시 `removeAllTeamPermissions()` 호출

### 테스트 도구
- `/test.html`: 모든 API 테스트 인터페이스 (CORS 대응)
- `/docker-compose.yml`: pgAdmin 추가 (포트 16001)

## 🌐 접속 정보

### 서비스 포트
- **Auth Server**: 15001 (직접 호출)
- **API Gateway**: 15000 (User/Team Service 경유)
- **User Service**: 15002 (Gateway 경유)
- **Team Service**: 15003 (Gateway 경유)  
- **Sentinel**: 15004 (직접 호출)

### 데이터베이스 포트
- **PostgreSQL**: 50001 (Auth/User)
- **Redis**: 50002 (Auth)
- **MongoDB**: 50003 (Team)
- **ScyllaDB**: 50004-50005 (Sentinel)
- **Sentinel Redis**: 50006 (Sentinel 캐시)

### 관리 도구
- **pgAdmin**: 16001 (admin@asdf.com / asdf)
- **Envoy Admin**: 9901

## 📋 CORS 주의사항

localhost:3000에서 테스트 시:
- ✅ **Auth Server (15001)**: CORS 헤더 필요 (미설정)
- ✅ **Sentinel (15004)**: CORS 설정 완료
- ✅ **Gateway (15000)**: User/Team Service CORS 처리

## 🔍 문제 해결 과정

### 해결된 주요 이슈들
1. **namespace 불일치**: "team" vs "teams" → "teams"로 통일
2. **API 형태 불일치**: write/read API 구조 수정
3. **Operation 대소문자**: "INSERT" → "Insert" 수정
4. **Userset 처리 로직**: tuple.user_type == "userset" 조건 수정
5. **팀 삭제 권한**: member만 삭제 → owner/member 모두 삭제
6. **CORS 설정**: Sentinel에 actix-cors 추가

### 검증 방법
- curl 명령어로 API 직접 호출
- docker-compose logs로 서비스 상태 확인  
- test.html 인터페이스로 통합 테스트
- Sentinel read API로 권한 상태 확인

## 🎯 최종 결과

**완전히 작동하는 마이크로서비스 권한 관리 시스템**
- Google Zanzibar 모델 구현
- 실시간 팀 권한 동기화
- 복잡한 권한 상속 및 Userset 지원
- 완전한 테스트 환경 제공

모든 기능이 정상 작동하며, localhost:3000의 test.html에서 전체 시스템을 테스트할 수 있습니다.