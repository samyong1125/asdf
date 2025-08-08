use std::fmt;
use std::error::Error as StdError;
use actix_web::{HttpResponse, ResponseError};

/// Sentinel 시스템의 주요 에러 타입들
#[derive(Debug)]
pub enum SentinelError {
    /// 데이터베이스 관련 에러 (ScyllaDB, Redis)
    DatabaseError {
        message: String,
        source: Option<Box<dyn StdError + Send + Sync>>,
    },
    /// 권한 튜플 검증 에러
    ValidationError {
        message: String,
    },
    /// 권한 체크 관련 에러
    PermissionError {
        message: String,
    },
    /// 직렬화/역직렬화 에러
    SerializationError {
        message: String,
        source: Option<Box<dyn StdError + Send + Sync>>,
    },
    /// 캐시 관련 에러
    CacheError {
        message: String,
        source: Option<Box<dyn StdError + Send + Sync>>,
    },
    /// 내부 서버 에러
    InternalError {
        message: String,
    },
}

impl fmt::Display for SentinelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SentinelError::DatabaseError { message, .. } => {
                write!(f, "database error: {}", message)
            }
            SentinelError::ValidationError { message } => {
                write!(f, "validation error: {}", message)
            }
            SentinelError::PermissionError { message } => {
                write!(f, "permission error: {}", message)
            }
            SentinelError::SerializationError { message, .. } => {
                write!(f, "serialization error: {}", message)
            }
            SentinelError::CacheError { message, .. } => {
                write!(f, "cache error: {}", message)
            }
            SentinelError::InternalError { message } => {
                write!(f, "internal error: {}", message)
            }
        }
    }
}

impl StdError for SentinelError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            SentinelError::DatabaseError { source, .. } => {
                source.as_ref().map(|e| e.as_ref() as &(dyn StdError + 'static))
            }
            SentinelError::SerializationError { source, .. } => {
                source.as_ref().map(|e| e.as_ref() as &(dyn StdError + 'static))
            }
            SentinelError::CacheError { source, .. } => {
                source.as_ref().map(|e| e.as_ref() as &(dyn StdError + 'static))
            }
            _ => None,
        }
    }
}

impl SentinelError {
    /// ScyllaDB 에러를 Sentinel 에러로 변환
    pub fn from_scylla_error(err: scylla::errors::ExecutionError, context: &str) -> Self {
        SentinelError::DatabaseError {
            message: format!("{}: {}", context, err),
            source: Some(Box::new(err)),
        }
    }
    
    /// ScyllaDB RowsError를 Sentinel 에러로 변환
    pub fn from_rows_error(err: scylla::response::query_result::RowsError, context: &str) -> Self {
        SentinelError::DatabaseError {
            message: format!("{}: {}", context, err),
            source: Some(Box::new(err)),
        }
    }

    /// Redis 에러를 Sentinel 에러로 변환
    pub fn from_redis_error(err: redis::RedisError, context: &str) -> Self {
        SentinelError::CacheError {
            message: format!("{}: {}", context, err),
            source: Some(Box::new(err)),
        }
    }

    /// 검증 에러 생성
    pub fn validation_error(message: impl Into<String>) -> Self {
        SentinelError::ValidationError {
            message: message.into(),
        }
    }

    /// 권한 에러 생성
    pub fn permission_error(message: impl Into<String>) -> Self {
        SentinelError::PermissionError {
            message: message.into(),
        }
    }

    /// 내부 에러 생성
    pub fn internal_error(message: impl Into<String>) -> Self {
        SentinelError::InternalError {
            message: message.into(),
        }
    }
}

/// Sentinel 결과 타입 별칭
pub type SentinelResult<T> = Result<T, SentinelError>;

impl ResponseError for SentinelError {
    fn error_response(&self) -> HttpResponse {
        match self {
            SentinelError::ValidationError { message } => {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Validation error",
                    "message": message
                }))
            }
            SentinelError::PermissionError { message } => {
                HttpResponse::Forbidden().json(serde_json::json!({
                    "error": "Permission error",
                    "message": message
                }))
            }
            SentinelError::DatabaseError { message, .. } => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Database error",
                    "message": message
                }))
            }
            SentinelError::CacheError { message, .. } => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Cache error", 
                    "message": message
                }))
            }
            SentinelError::SerializationError { message, .. } => {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Serialization error",
                    "message": message
                }))
            }
            SentinelError::InternalError { message } => {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Internal error",
                    "message": message
                }))
            }
        }
    }
}