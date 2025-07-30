from sqlalchemy.orm import Session
from sqlalchemy import text
from . import models, schemas
from typing import Optional

def get_user_by_id(db: Session, user_id: int) -> Optional[models.User]:
    """사용자 ID로 사용자 조회"""
    return db.query(models.User).filter(models.User.id == user_id).first()

def get_user_by_email(db: Session, email: str) -> Optional[models.User]:
    """이메일로 사용자 조회"""
    return db.query(models.User).filter(models.User.email == email).first()

def update_user(db: Session, user_id: int, user_update: schemas.UserUpdateRequest) -> Optional[models.User]:
    """사용자 정보 업데이트"""
    db_user = db.query(models.User).filter(models.User.id == user_id).first()
    if not db_user:
        return None
    
    update_data = user_update.model_dump(exclude_unset=True)
    for field, value in update_data.items():
        setattr(db_user, field, value)
    
    # updated_at은 SQLAlchemy의 onupdate로 자동 처리됨
    db.commit()
    db.refresh(db_user)
    return db_user

def test_db_connection(db: Session) -> bool:
    """DB 연결 테스트"""
    try:
        db.execute(text("SELECT 1"))
        return True
    except Exception as e:
        print(f"DB 연결 실패: {e}")
        return False