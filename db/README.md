# Database Schema

## 초기화 스크립트

PostgreSQL 컨테이너가 시작될 때 `init/` 폴더의 스크립트가 자동으로 실행됩니다.

### 실행 순서:
1. `01_create_database.sql` - 테이블 및 인덱스 생성

## 테이블 구조

### users 테이블
```sql
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    email VARCHAR(255) UNIQUE NOT NULL,
    password VARCHAR(255) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

## 사용자 등록

사용자는 `/api/v1/register` 엔드포인트를 통해 등록하거나, 직접 데이터베이스에 추가할 수 있습니다.

## Redis 구조

Refresh Token은 Redis에 저장됩니다:
- Key: `refresh:{user_id}`
- Value: UUID 형식의 refresh token
- TTL: 180일 (15552000초)