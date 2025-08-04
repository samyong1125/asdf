use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// 권한 계층 구조를 관리하는 구조체
/// "가장 강한 권한 승리" 원칙을 구현
#[derive(Debug, Clone)]
pub struct PermissionHierarchy {
    /// 권한 이름 -> 레벨 매핑
    levels: HashMap<String, u8>,
    /// 권한 상속 관계 (하위 권한 -> 상위 권한들)
    inheritance: HashMap<String, Vec<String>>,
}

impl Default for PermissionHierarchy {
    fn default() -> Self {
        Self::new()
    }
}

impl PermissionHierarchy {
    /// 기본 권한 계층 구조 생성
    pub fn new() -> Self {
        let mut levels = HashMap::new();
        
        // 권한 레벨 정의 (높을수록 강한 권한)
        levels.insert("viewer".to_string(), 1);
        levels.insert("commenter".to_string(), 2);
        levels.insert("editor".to_string(), 3);
        levels.insert("admin".to_string(), 4);
        levels.insert("owner".to_string(), 5);
        
        // 권한 상속 관계 정의
        let mut inheritance = HashMap::new();
        inheritance.insert("owner".to_string(), vec!["admin".to_string(), "editor".to_string(), "commenter".to_string(), "viewer".to_string()]);
        inheritance.insert("admin".to_string(), vec!["editor".to_string(), "commenter".to_string(), "viewer".to_string()]);
        inheritance.insert("editor".to_string(), vec!["commenter".to_string(), "viewer".to_string()]);
        inheritance.insert("commenter".to_string(), vec!["viewer".to_string()]);
        inheritance.insert("viewer".to_string(), vec![]);
        
        Self { levels, inheritance }
    }
    
    /// 권한 레벨 조회
    pub fn get_level(&self, permission: &str) -> u8 {
        self.levels.get(permission).copied().unwrap_or(0)
    }
    
    /// 요청된 권한에 대해 사용자 권한이 충분한지 확인
    /// user_permission이 required_permission 이상인지 검사
    pub fn can_access(&self, user_permission: &str, required_permission: &str) -> bool {
        let user_level = self.get_level(user_permission);
        let required_level = self.get_level(required_permission);
        
        user_level >= required_level
    }
    
    /// 여러 권한 중 가장 높은 레벨 반환
    pub fn get_max_permission_level(&self, permissions: &[String]) -> u8 {
        permissions
            .iter()
            .map(|p| self.get_level(p))
            .max()
            .unwrap_or(0)
    }
    
    /// 특정 권한이 다른 권한을 포함하는지 확인 (상속 관계)
    pub fn includes(&self, higher_permission: &str, lower_permission: &str) -> bool {
        if higher_permission == lower_permission {
            return true;
        }
        
        if let Some(inherited) = self.inheritance.get(higher_permission) {
            inherited.contains(&lower_permission.to_string())
        } else {
            false
        }
    }
    
    /// 특정 권한이 상속하는 모든 권한 목록 반환
    pub fn get_inherited_permissions(&self, permission: &str) -> Vec<String> {
        let mut result = vec![permission.to_string()];
        
        if let Some(inherited) = self.inheritance.get(permission) {
            result.extend(inherited.clone());
        }
        
        result
    }
    
    /// 권한 목록을 레벨 순으로 정렬 (높은 권한부터)
    pub fn sort_by_level(&self, permissions: &mut Vec<String>) {
        permissions.sort_by(|a, b| {
            let level_a = self.get_level(a);
            let level_b = self.get_level(b);
            level_b.cmp(&level_a) // 내림차순 정렬
        });
    }
    
    /// 유효한 권한인지 확인
    pub fn is_valid_permission(&self, permission: &str) -> bool {
        self.levels.contains_key(permission)
    }
    
    /// 모든 권한 목록 반환 (레벨 순)
    pub fn get_all_permissions(&self) -> Vec<String> {
        let mut permissions: Vec<String> = self.levels.keys().cloned().collect();
        self.sort_by_level(&mut permissions);
        permissions
    }
}

/// 권한 검증 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionCheckResult {
    /// 권한 허용 여부
    pub allowed: bool,
    /// 사용자가 가진 최고 권한
    pub user_max_permission: Option<String>,
    /// 사용자가 가진 최고 권한 레벨
    pub user_max_level: u8,
    /// 요청된 권한
    pub required_permission: String,
    /// 요청된 권한 레벨
    pub required_level: u8,
    /// 권한 소스 (직접 권한인지 팀 권한인지 등)
    pub permission_sources: Vec<PermissionSource>,
}

/// 권한의 출처 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionSource {
    /// 권한 이름
    pub permission: String,
    /// 권한 레벨
    pub level: u8,
    /// 소스 타입 (direct, team, group 등)
    pub source_type: String,
    /// 소스 식별자 (예: team:backend)
    pub source_identifier: Option<String>,
}

impl PermissionCheckResult {
    /// 새로운 권한 검증 결과 생성
    pub fn new(
        required_permission: &str,
        hierarchy: &PermissionHierarchy,
    ) -> Self {
        let required_level = hierarchy.get_level(required_permission);
        
        Self {
            allowed: false,
            user_max_permission: None,
            user_max_level: 0,
            required_permission: required_permission.to_string(),
            required_level,
            permission_sources: Vec::new(),
        }
    }
    
    /// 권한 소스 추가
    pub fn add_permission_source(&mut self, source: PermissionSource) {
        // 최고 권한 업데이트
        if source.level > self.user_max_level {
            self.user_max_level = source.level;
            self.user_max_permission = Some(source.permission.clone());
        }
        
        self.permission_sources.push(source);
        
        // 권한 허용 여부 업데이트
        self.allowed = self.user_max_level >= self.required_level;
    }
    
    /// 직접 권한 추가
    pub fn add_direct_permission(&mut self, permission: &str, hierarchy: &PermissionHierarchy) {
        let level = hierarchy.get_level(permission);
        let source = PermissionSource {
            permission: permission.to_string(),
            level,
            source_type: "direct".to_string(),
            source_identifier: None,
        };
        self.add_permission_source(source);
    }
    
    /// 팀/그룹 권한 추가
    pub fn add_team_permission(
        &mut self,
        permission: &str,
        team_identifier: &str,
        hierarchy: &PermissionHierarchy,
    ) {
        let level = hierarchy.get_level(permission);
        let source = PermissionSource {
            permission: permission.to_string(),
            level,
            source_type: "team".to_string(),
            source_identifier: Some(team_identifier.to_string()),
        };
        self.add_permission_source(source);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_permission_hierarchy() {
        let hierarchy = PermissionHierarchy::new();
        
        // 레벨 테스트
        assert_eq!(hierarchy.get_level("viewer"), 1);
        assert_eq!(hierarchy.get_level("editor"), 3);
        assert_eq!(hierarchy.get_level("owner"), 5);
        assert_eq!(hierarchy.get_level("nonexistent"), 0);
        
        // 접근 권한 테스트
        assert!(hierarchy.can_access("editor", "viewer")); // editor는 viewer 가능
        assert!(hierarchy.can_access("owner", "editor"));  // owner는 editor 가능
        assert!(!hierarchy.can_access("viewer", "editor")); // viewer는 editor 불가
        
        // 상속 관계 테스트
        assert!(hierarchy.includes("owner", "viewer"));
        assert!(hierarchy.includes("editor", "viewer"));
        assert!(!hierarchy.includes("viewer", "editor"));
    }
    
    #[test]
    fn test_permission_check_result() {
        let hierarchy = PermissionHierarchy::new();
        let mut result = PermissionCheckResult::new("editor", &hierarchy);
        
        // 직접 권한 추가 - viewer (부족함)
        result.add_direct_permission("viewer", &hierarchy);
        assert!(!result.allowed);
        assert_eq!(result.user_max_level, 1);
        
        // 팀 권한 추가 - admin (충분함)
        result.add_team_permission("admin", "team:backend", &hierarchy);
        assert!(result.allowed);
        assert_eq!(result.user_max_level, 4);
        assert_eq!(result.user_max_permission, Some("admin".to_string()));
    }
}