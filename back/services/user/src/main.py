import os
from fastapi import FastAPI, Depends
from dotenv import load_dotenv
from sqlalchemy.orm import Session
from .database import get_db
from .routers import router as user_router
from . import crud

# 환경 변수 로드
load_dotenv()

app = FastAPI(
    title="User Service API",
    description="ASDF 프로젝트의 사용자 관리 서비스",
    version="1.0.0"
)

# 라우터 등록
app.include_router(user_router)

@app.on_event("startup")
async def startup_event():
    """애플리케이션 시작 시 실행되는 이벤트"""
    print("User Service 시작 중...")
    print("✅ User Service 준비 완료")

@app.get("/health")
async def health_check():
    """헬스체크 엔드포인트"""
    return {
        "status": "ok",
        "service": "user-service",
        "version": "1.0.0"
    }

@app.get("/")
async def root():
    """루트 엔드포인트"""
    return {"message": "User Service is running"}

@app.get("/db-test")
async def db_test(db: Session = Depends(get_db)):
    """DB 연결 테스트 엔드포인트"""
    if crud.test_db_connection(db):
        return {"status": "ok", "message": "Database connection successful"}
    else:
        return {"status": "error", "message": "Database connection failed"}

if __name__ == "__main__":
    import uvicorn
    port = int(os.getenv("PORT", 15002))
    uvicorn.run(app, host="0.0.0.0", port=port)