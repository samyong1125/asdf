import os
from sqlalchemy import create_engine, text
from sqlalchemy.ext.declarative import declarative_base
from sqlalchemy.orm import sessionmaker

# 환경 변수에서 DB 설정 가져오기
DB_HOST = os.getenv("DB_HOST", "localhost")
DB_PORT = os.getenv("DB_PORT", "50001")
DB_USER = os.getenv("DB_USER", "asdf")
DB_PASSWORD = os.getenv("DB_PASSWORD", "asdf")
DB_NAME = os.getenv("DB_NAME", "userdb")

# PostgreSQL 연결 URL 생성
DATABASE_URL = f"postgresql://{DB_USER}:{DB_PASSWORD}@{DB_HOST}:{DB_PORT}/{DB_NAME}"

# SQLAlchemy 엔진 생성
engine = create_engine(DATABASE_URL)

# 세션 팩토리 생성
SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)

# Base 클래스 생성
Base = declarative_base()

def get_db():
    """DB 세션 의존성 함수"""
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()

def test_db_connection():
    """DB 연결 테스트 함수"""
    try:
        with engine.connect() as connection:
            result = connection.execute(text("SELECT 1"))
            return True
    except Exception as e:
        print(f"DB 연결 실패: {e}")
        return False