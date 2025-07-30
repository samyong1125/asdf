from pydantic import BaseModel, EmailStr
from datetime import datetime
from typing import Optional

class UserBase(BaseModel):
    email: EmailStr

class UserResponse(UserBase):
    """사용자 정보 응답 스키마 (패스워드 제외)"""
    id: int
    created_at: datetime
    updated_at: datetime
    
    class Config:
        from_attributes = True

class UserUpdateRequest(BaseModel):
    """사용자 정보 수정 요청 스키마"""
    email: Optional[EmailStr] = None