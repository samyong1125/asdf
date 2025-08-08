package com.asdf.clients

import io.ktor.client.*
import io.ktor.client.call.*
import io.ktor.client.engine.cio.*
import io.ktor.client.plugins.contentnegotiation.*
import io.ktor.client.request.*
import io.ktor.http.*
import io.ktor.serialization.kotlinx.json.*
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.datetime.Clock

@Serializable
data class SentinelTuple(
    val namespace: String,
    val object_id: String,
    val relation: String,
    val user_type: String,
    val user_id: String,
    val created_at: String
)

@Serializable
data class SentinelTupleUpdate(
    val operation: String, // "Insert" or "Delete"
    val tuple: SentinelTuple
)

@Serializable
data class SentinelWriteRequest(
    val updates: List<SentinelTupleUpdate>
)

@Serializable
data class SentinelWriteResponse(
    val zookie: String
)

@Serializable
data class SentinelCheckRequest(
    val namespace: String,
    val object_id: String,
    val relation: String,
    val user_id: String,
    val user_type: String? = null
)

@Serializable
data class SentinelCheckResponse(
    val allowed: Boolean,
    val zookie: String
)

@Serializable
data class SentinelBatchCheckRequest(
    val checks: List<SentinelCheckRequest>
)

@Serializable
data class SentinelBatchCheckItem(
    val request_index: Int,
    val allowed: Boolean,
    val request_info: String
)

@Serializable
data class SentinelBatchCheckResponse(
    val results: List<SentinelBatchCheckItem>,
    val total_requests: Int,
    val allowed_count: Int,
    val denied_count: Int,
    val zookie: String
)

class SentinelClient(private val baseUrl: String) {
    private val client = HttpClient(CIO) {
        install(ContentNegotiation) {
            json(Json {
                ignoreUnknownKeys = true
                isLenient = true
            })
        }
    }
    
    /**
     * 팀 오너 추가 (팀 생성자용)
     */
    suspend fun addTeamOwner(teamId: String, userId: Int): Boolean {
        return try {
            val request = SentinelWriteRequest(
                updates = listOf(
                    SentinelTupleUpdate(
                        operation = "Insert",
                        tuple = SentinelTuple(
                            namespace = "teams",
                            object_id = teamId,
                            relation = "owner",
                            user_type = "user",
                            user_id = userId.toString(),
                            created_at = Clock.System.now().toString()
                        )
                    )
                )
            )
            
            val response = client.post("$baseUrl/api/v1/write") {
                contentType(ContentType.Application.Json)
                setBody(request)
            }
            
            val success = response.status.isSuccess()
            if (!success) {
                println("Sentinel addTeamOwner 실패: ${response.status} - teamId: $teamId, userId: $userId")
            }
            success
        } catch (e: Exception) {
            println("Sentinel addTeamOwner 호출 실패: ${e.message} - teamId: $teamId, userId: $userId")
            false
        }
    }
    
    /**
     * 팀 멤버 추가 - 단일 멤버
     */
    suspend fun addTeamMember(teamId: String, userId: Int): Boolean {
        return try {
            val request = SentinelWriteRequest(
                updates = listOf(
                    SentinelTupleUpdate(
                        operation = "Insert",
                        tuple = SentinelTuple(
                            namespace = "teams",
                            object_id = teamId,
                            relation = "member",
                            user_type = "user",
                            user_id = userId.toString(),
                            created_at = Clock.System.now().toString()
                        )
                    )
                )
            )
            
            val response = client.post("$baseUrl/api/v1/write") {
                contentType(ContentType.Application.Json)
                setBody(request)
            }
            
            val success = response.status.isSuccess()
            if (!success) {
                println("Sentinel addTeamMember 실패: ${response.status} - teamId: $teamId, userId: $userId")
            }
            success
        } catch (e: Exception) {
            println("Sentinel addTeamMember 호출 실패: ${e.message} - teamId: $teamId, userId: $userId")
            false // 실패해도 팀 멤버 추가는 계속 진행
        }
    }
    
    /**
     * 팀 멤버 제거 - 단일 멤버
     */
    suspend fun removeTeamMember(teamId: String, userId: Int): Boolean {
        return try {
            val request = SentinelWriteRequest(
                updates = listOf(
                    SentinelTupleUpdate(
                        operation = "Delete",
                        tuple = SentinelTuple(
                            namespace = "teams",
                            object_id = teamId,
                            relation = "member",
                            user_type = "user",
                            user_id = userId.toString(),
                            created_at = Clock.System.now().toString()
                        )
                    )
                )
            )
            
            val response = client.post("$baseUrl/api/v1/write") {
                contentType(ContentType.Application.Json)
                setBody(request)
            }
            
            val success = response.status.isSuccess()
            if (!success) {
                println("Sentinel removeTeamMember 실패: ${response.status} - teamId: $teamId, userId: $userId")
            }
            success
        } catch (e: Exception) {
            println("Sentinel removeTeamMember 호출 실패: ${e.message} - teamId: $teamId, userId: $userId")
            false
        }
    }
    
    /**
     * 팀 멤버 일괄 추가 (팀 생성 시 사용)
     */
    suspend fun addTeamMembers(teamId: String, userIds: List<Int>): Boolean {
        if (userIds.isEmpty()) return true
        
        return try {
            val updates = userIds.map { userId ->
                SentinelTupleUpdate(
                    operation = "Insert",
                    tuple = SentinelTuple(
                        namespace = "teams",
                        object_id = teamId,
                        relation = "member",
                        user_type = "user",
                        user_id = userId.toString(),
                        created_at = Clock.System.now().toString()
                    )
                )
            }
            
            val request = SentinelWriteRequest(updates = updates)
            
            val response = client.post("$baseUrl/api/v1/write") {
                contentType(ContentType.Application.Json)
                setBody(request)
            }
            
            val success = response.status.isSuccess()
            if (!success) {
                println("Sentinel addTeamMembers 실패: ${response.status} - teamId: $teamId, userIds: $userIds")
            }
            success
        } catch (e: Exception) {
            println("Sentinel addTeamMembers 호출 실패: ${e.message} - teamId: $teamId, userIds: $userIds")
            false
        }
    }
    
    /**
     * 팀 멤버 일괄 제거 (팀 삭제 시 사용)
     */
    suspend fun removeTeamMembers(teamId: String, userIds: List<Int>): Boolean {
        if (userIds.isEmpty()) return true
        
        return try {
            val updates = userIds.map { userId ->
                SentinelTupleUpdate(
                    operation = "Delete",
                    tuple = SentinelTuple(
                        namespace = "teams",
                        object_id = teamId,
                        relation = "member",
                        user_type = "user",
                        user_id = userId.toString(),
                        created_at = Clock.System.now().toString()
                    )
                )
            }
            
            val request = SentinelWriteRequest(updates = updates)
            
            val response = client.post("$baseUrl/api/v1/write") {
                contentType(ContentType.Application.Json)
                setBody(request)
            }
            
            val success = response.status.isSuccess()
            if (!success) {
                println("Sentinel removeTeamMembers 실패: ${response.status} - teamId: $teamId, userIds: $userIds")
            }
            success
        } catch (e: Exception) {
            println("Sentinel removeTeamMembers 호출 실패: ${e.message} - teamId: $teamId, userIds: $userIds")
            false
        }
    }
    
    /**
     * 팀 삭제 시 모든 관련 권한 제거 (owner, member 등 모든 관계)
     */
    suspend fun removeAllTeamPermissions(teamId: String, userIds: List<Int>): Boolean {
        println("🔥 SentinelClient.removeAllTeamPermissions 호출: teamId=$teamId, userIds=$userIds")
        if (userIds.isEmpty()) return true
        
        return try {
            val updates = mutableListOf<SentinelTupleUpdate>()
            
            // 각 사용자에 대해 owner, member 권한 모두 삭제
            for (userId in userIds) {
                // owner 권한 삭제
                updates.add(SentinelTupleUpdate(
                    operation = "Delete",
                    tuple = SentinelTuple(
                        namespace = "teams",
                        object_id = teamId,
                        relation = "owner",
                        user_type = "user",
                        user_id = userId.toString(),
                        created_at = Clock.System.now().toString()
                    )
                ))
                
                // member 권한 삭제 (혹시 있을 수 있으므로)
                updates.add(SentinelTupleUpdate(
                    operation = "Delete",
                    tuple = SentinelTuple(
                        namespace = "teams",
                        object_id = teamId,
                        relation = "member",
                        user_type = "user",
                        user_id = userId.toString(),
                        created_at = Clock.System.now().toString()
                    )
                ))
            }
            
            val request = SentinelWriteRequest(updates = updates)
            
            val response = client.post("$baseUrl/api/v1/write") {
                contentType(ContentType.Application.Json)
                setBody(request)
            }
            
            val success = response.status.isSuccess()
            if (!success) {
                println("Sentinel removeAllTeamPermissions 실패: ${response.status} - teamId: $teamId, userIds: $userIds")
            }
            success
        } catch (e: Exception) {
            println("Sentinel removeAllTeamPermissions 호출 실패: ${e.message} - teamId: $teamId, userIds: $userIds")
            false
        }
    }
    
    /**
     * 단일 권한 체크
     */
    suspend fun checkPermission(teamId: String, relation: String, userId: Int): Boolean {
        return try {
            val request = SentinelCheckRequest(
                namespace = "teams",
                object_id = teamId,
                relation = relation,
                user_id = userId.toString(),
                user_type = "user"
            )
            
            val response = client.post("$baseUrl/api/v1/check") {
                contentType(ContentType.Application.Json)
                setBody(request)
            }
            
            if (response.status.isSuccess()) {
                val checkResponse: SentinelCheckResponse = response.body()
                checkResponse.allowed
            } else {
                println("Sentinel checkPermission 실패: ${response.status} - teamId: $teamId, relation: $relation, userId: $userId")
                false
            }
        } catch (e: Exception) {
            println("Sentinel checkPermission 호출 실패: ${e.message} - teamId: $teamId, relation: $relation, userId: $userId")
            false
        }
    }
    
    /**
     * 배치 권한 체크 - 여러 권한을 한번에 검증
     */
    suspend fun batchCheckPermissions(checks: List<SentinelCheckRequest>): List<Boolean> {
        if (checks.isEmpty()) return emptyList()
        
        return try {
            val request = SentinelBatchCheckRequest(checks = checks)
            
            val response = client.post("$baseUrl/api/v1/batch_check") {
                contentType(ContentType.Application.Json)
                setBody(request)
            }
            
            if (response.status.isSuccess()) {
                val batchResponse: SentinelBatchCheckResponse = response.body()
                // 결과를 원래 순서대로 정렬하여 반환
                batchResponse.results
                    .sortedBy { it.request_index }
                    .map { it.allowed }
            } else {
                println("Sentinel batchCheckPermissions 실패: ${response.status} - ${checks.size}개 요청")
                List(checks.size) { false } // 모든 권한을 거부로 처리
            }
        } catch (e: Exception) {
            println("Sentinel batchCheckPermissions 호출 실패: ${e.message} - ${checks.size}개 요청")
            List(checks.size) { false } // 모든 권한을 거부로 처리
        }
    }
    
    /**
     * 팀 관리 권한 체크 (owner 또는 admin)
     */
    suspend fun checkTeamManagePermission(teamId: String, userId: Int): Boolean {
        val checks = listOf(
            SentinelCheckRequest("teams", teamId, "owner", userId.toString(), "user"),
            SentinelCheckRequest("teams", teamId, "admin", userId.toString(), "user")
        )
        
        val results = batchCheckPermissions(checks)
        return results.any { it } // owner 또는 admin 중 하나라도 있으면 true
    }
    
    /**
     * 팀 멤버십 체크 (member, admin, owner 중 하나)
     */
    suspend fun checkTeamMembership(teamId: String, userId: Int): Boolean {
        val checks = listOf(
            SentinelCheckRequest("teams", teamId, "member", userId.toString(), "user"),
            SentinelCheckRequest("teams", teamId, "admin", userId.toString(), "user"),
            SentinelCheckRequest("teams", teamId, "owner", userId.toString(), "user")
        )
        
        val results = batchCheckPermissions(checks)
        return results.any { it } // member, admin, owner 중 하나라도 있으면 true
    }
    
    /**
     * 연결 종료
     */
    fun close() {
        client.close()
    }
}