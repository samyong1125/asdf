from fastapi import APIRouter, Depends, HTTPException, Header
from sqlalchemy.orm import Session
from typing import Optional
from . import crud, schemas
from .database import get_db

router = APIRouter(prefix="/api/v1/users", tags=["users"])

def get_current_user_id(x_user_id: Optional[str] = Header(None)) -> int:
    """Envoy에서 전달된 X-User-ID 헤더에서 사용자 ID 추출"""
    if not x_user_id:
        raise HTTPException(status_code=401, detail="인증이 필요합니다")
    
    try:
        return int(x_user_id)
    except ValueError:
        raise HTTPException(status_code=400, detail="잘못된 사용자 ID입니다")

@router.get("/me", response_model=schemas.UserResponse)
async def get_current_user_info(
    current_user_id: int = Depends(get_current_user_id),
    db: Session = Depends(get_db)
):
    """현재 로그인한 사용자 정보 조회"""
    user = crud.get_user_by_id(db, current_user_id)
    if not user:
        raise HTTPException(status_code=404, detail="사용자를 찾을 수 없습니다")
    
    return user

@router.put("/me", response_model=schemas.UserResponse)
async def update_current_user_info(
    user_update: schemas.UserUpdateRequest,
    current_user_id: int = Depends(get_current_user_id),
    db: Session = Depends(get_db)
):
    """현재 로그인한 사용자 정보 수정"""
    # 이메일 중복 체크
    if user_update.email:
        existing_user = crud.get_user_by_email(db, user_update.email)
        if existing_user and existing_user.id != current_user_id:
            raise HTTPException(status_code=400, detail="이미 사용 중인 이메일입니다")
    
    updated_user = crud.update_user(db, current_user_id, user_update)
    if not updated_user:
        raise HTTPException(status_code=404, detail="사용자를 찾을 수 없습니다")
    
    return updated_user

@router.get("/{user_id}", response_model=schemas.UserResponse)
async def get_user_info(
    user_id: int,
    db: Session = Depends(get_db)
):
    """특정 사용자 정보 조회"""
    user = crud.get_user_by_id(db, user_id)
    if not user:
        raise HTTPException(status_code=404, detail="사용자를 찾을 수 없습니다")
    
    return user