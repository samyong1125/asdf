package com.asdf.services

import com.asdf.clients.SentinelClient
import com.asdf.clients.SentinelCheckRequest
import com.asdf.config.DatabaseConfig
import com.asdf.models.*
import com.mongodb.kotlin.client.coroutine.MongoCollection
import kotlinx.coroutines.flow.firstOrNull
import kotlinx.coroutines.flow.toList
import kotlinx.datetime.Clock
import kotlinx.datetime.Instant
import org.bson.types.ObjectId
import com.mongodb.client.model.Filters
import com.mongodb.client.model.Updates
import org.bson.Document
import java.time.ZoneOffset
import java.util.*

class TeamService {
    private val database = DatabaseConfig.getDatabase()
    private val teamsCollection: MongoCollection<Document> = database.getCollection("teams")
    private val sentinelClient = SentinelClient(
        System.getenv("SENTINEL_URL") ?: "http://localhost:15004"
    )
    
    suspend fun createTeam(request: CreateTeamRequest, creatorUserId: Int): TeamResponse {
        val now = Clock.System.now()
        val nowDate = Date.from(now.toJavaInstant())
        
        val memberDoc = Document()
            .append("userId", creatorUserId)
            .append("joinedAt", nowDate)
        
        val teamDoc = Document()
            .append("name", request.name)
            .append("members", listOf(memberDoc))
            .append("createdAt", nowDate)
            .append("updatedAt", nowDate)
        
        val result = teamsCollection.insertOne(teamDoc)
        val insertedId = result.insertedId?.asObjectId()?.value
        
        // ✨ MongoDB 저장 성공 시 Sentinel에 팀 생성자 owner 권한 추가
        if (insertedId != null) {
            val sentinelSuccess = sentinelClient.addTeamOwner(insertedId.toString(), creatorUserId)
            if (!sentinelSuccess) {
                println("⚠️ 팀 생성은 성공했지만 Sentinel owner 권한 동기화 실패 - teamId: $insertedId, creatorId: $creatorUserId")
            }
        }
        
        return TeamResponse(
            id = insertedId.toString(),
            name = request.name,
            members = listOf(
                TeamMember(
                    userId = creatorUserId,
                    joinedAt = now
                )
            ),
            createdAt = now,
            updatedAt = now
        )
    }
    
    suspend fun getTeamById(teamId: String): Team? {
        val objectId = try {
            ObjectId(teamId)
        } catch (e: IllegalArgumentException) {
            return null
        }
        
        val doc = teamsCollection.find(Filters.eq("_id", objectId)).firstOrNull()
        return doc?.let { documentToTeam(it) }
    }
    
    suspend fun getTeamsByUserId(userId: Int): List<Team> {
        val docs = teamsCollection.find(Filters.eq("members.userId", userId)).toList()
        return docs.map { documentToTeam(it) }
    }
    
    suspend fun updateTeam(teamId: String, request: UpdateTeamRequest, userId: Int): Boolean {
        val objectId = try {
            ObjectId(teamId)
        } catch (e: IllegalArgumentException) {
            return false
        }
        
        // 팀 관리 권한 체크 (owner 또는 admin 필요)
        val hasManagePermission = sentinelClient.checkTeamManagePermission(teamId, userId)
        if (!hasManagePermission) {
            println("⚠️ 팀 수정 권한 없음 - teamId: $teamId, userId: $userId")
            return false
        }
        
        val nowDate = Date.from(Clock.System.now().toJavaInstant())
        val update = Updates.combine(
            Updates.set("name", request.name),
            Updates.set("updatedAt", nowDate)
        )
        
        val result = teamsCollection.updateOne(Filters.eq("_id", objectId), update)
        return result.modifiedCount > 0
    }
    
    suspend fun deleteTeam(teamId: String, userId: Int): Boolean {
        val objectId = try {
            ObjectId(teamId)
        } catch (e: IllegalArgumentException) {
            return false
        }
        
        // 팀 소유자 권한 체크 (삭제는 owner만 가능)
        val hasOwnerPermission = sentinelClient.checkPermission(teamId, "owner", userId)
        if (!hasOwnerPermission) {
            println("⚠️ 팀 삭제 권한 없음 (owner 권한 필요) - teamId: $teamId, userId: $userId")
            return false
        }
        
        // ✨ 삭제 전에 팀 멤버 목록 조회 (Sentinel 권한 제거용)
        val team = getTeamById(teamId)
        
        val result = teamsCollection.deleteOne(Filters.eq("_id", objectId))
        val success = result.deletedCount > 0
        
        // ✨ MongoDB 삭제 성공 시 Sentinel에서 모든 관련 권한 제거 (owner, member 등)
        if (success && team != null) {
            val userIds = team.members.map { it.userId }
            println("🔥 팀 삭제 - Sentinel 권한 제거 시작: teamId=$teamId, userIds=$userIds")
            val sentinelSuccess = sentinelClient.removeAllTeamPermissions(teamId, userIds)
            println("🔥 팀 삭제 - Sentinel 권한 제거 결과: $sentinelSuccess")
            if (!sentinelSuccess) {
                println("⚠️ 팀 삭제는 성공했지만 Sentinel 권한 동기화 실패 - teamId: $teamId, memberIds: $userIds")
            }
        } else {
            println("🔥 팀 삭제 - Sentinel 호출 안됨: success=$success, team=$team")
        }
        
        return success
    }
    
    suspend fun addMember(teamId: String, targetUserId: Int, requesterId: Int): Boolean {
        val objectId = try {
            ObjectId(teamId)
        } catch (e: IllegalArgumentException) {
            return false
        }
        
        // 팀 관리 권한 체크 (owner 또는 admin 필요)
        val hasManagePermission = sentinelClient.checkTeamManagePermission(teamId, requesterId)
        if (!hasManagePermission) {
            println("⚠️ 멤버 추가 권한 없음 - teamId: $teamId, requesterId: $requesterId")
            return false
        }
        
        // 이미 멤버인지 확인
        val existing = teamsCollection.find(
            Filters.and(
                Filters.eq("_id", objectId),
                Filters.eq("members.userId", targetUserId)
            )
        ).firstOrNull()
        
        if (existing != null) {
            return false // 이미 멤버임
        }
        
        val nowDate = Date.from(Clock.System.now().toJavaInstant())
        val newMember = Document()
            .append("userId", targetUserId)
            .append("joinedAt", nowDate)
        
        val update = Updates.combine(
            Updates.push("members", newMember),
            Updates.set("updatedAt", nowDate)
        )
        
        val result = teamsCollection.updateOne(Filters.eq("_id", objectId), update)
        val success = result.modifiedCount > 0
        
        // ✨ MongoDB 업데이트 성공 시 Sentinel에 멤버십 권한 추가
        if (success) {
            val sentinelSuccess = sentinelClient.addTeamMember(teamId, targetUserId)
            if (!sentinelSuccess) {
                println("⚠️ 멤버 추가는 성공했지만 Sentinel 권한 동기화 실패 - teamId: $teamId, userId: $targetUserId")
            }
        }
        
        return success
    }
    
    suspend fun removeMember(teamId: String, targetUserId: Int, requesterId: Int): Boolean {
        val objectId = try {
            ObjectId(teamId)
        } catch (e: IllegalArgumentException) {
            return false
        }
        
        // 팀 관리 권한 체크 또는 본인 탈퇴
        val hasManagePermission = sentinelClient.checkTeamManagePermission(teamId, requesterId)
        val isSelfRemoval = (requesterId == targetUserId)
        
        if (!hasManagePermission && !isSelfRemoval) {
            println("⚠️ 멤버 제거 권한 없음 - teamId: $teamId, requesterId: $requesterId, targetUserId: $targetUserId")
            return false
        }
        
        val nowDate = Date.from(Clock.System.now().toJavaInstant())
        val update = Updates.combine(
            Updates.pull("members", Document("userId", targetUserId)),
            Updates.set("updatedAt", nowDate)
        )
        
        val result = teamsCollection.updateOne(Filters.eq("_id", objectId), update)
        val success = result.modifiedCount > 0
        
        // ✨ MongoDB 업데이트 성공 시 Sentinel에서 멤버십 권한 제거
        if (success) {
            val sentinelSuccess = sentinelClient.removeTeamMember(teamId, targetUserId)
            if (!sentinelSuccess) {
                println("⚠️ 멤버 제거는 성공했지만 Sentinel 권한 동기화 실패 - teamId: $teamId, userId: $targetUserId")
            }
        }
        
        return success
    }
    
    suspend fun getTeamMembers(teamId: String): List<TeamMember>? {
        val team = getTeamById(teamId)
        return team?.members
    }
    
    /**
     * 배치 처리를 활용한 팀 멤버 목록 조회 (권한 포함)
     */
    suspend fun getTeamMembersWithRoles(teamId: String, requesterId: Int): List<TeamMemberWithRole>? {
        val team = getTeamById(teamId) ?: return null
        
        // 요청자가 팀 멤버인지 체크
        val hasAccess = sentinelClient.checkTeamMembership(teamId, requesterId)
        if (!hasAccess) {
            println("⚠️ 팀 멤버 목록 조회 권한 없음 - teamId: $teamId, requesterId: $requesterId")
            return null
        }
        
        // 모든 멤버들의 권한을 배치로 체크
        val permissionChecks = mutableListOf<SentinelCheckRequest>()
        
        team.members.forEach { member ->
            // 각 멤버에 대해 owner, admin, member 권한 체크
            permissionChecks.addAll(listOf(
                SentinelCheckRequest("teams", teamId, "owner", member.userId.toString(), "user"),
                SentinelCheckRequest("teams", teamId, "admin", member.userId.toString(), "user"),
                SentinelCheckRequest("teams", teamId, "member", member.userId.toString(), "user")
            ))
        }
        
        val permissionResults = sentinelClient.batchCheckPermissions(permissionChecks)
        
        // 결과를 멤버별로 그룹핑하여 역할 결정
        return team.members.mapIndexed { index, member ->
            val baseIndex = index * 3 // 각 멤버당 3개 권한 체크
            val isOwner = permissionResults.getOrNull(baseIndex) ?: false
            val isAdmin = permissionResults.getOrNull(baseIndex + 1) ?: false
            val isMember = permissionResults.getOrNull(baseIndex + 2) ?: false
            
            val role = when {
                isOwner -> "owner"
                isAdmin -> "admin"  
                isMember -> "member"
                else -> "unknown"
            }
            
            TeamMemberWithRole(
                userId = member.userId,
                joinedAt = member.joinedAt,
                role = role
            )
        }
    }
    
    suspend fun isTeamMember(teamId: String, userId: Int): Boolean {
        val objectId = try {
            ObjectId(teamId)
        } catch (e: IllegalArgumentException) {
            return false
        }
        
        val team = teamsCollection.find(
            Filters.and(
                Filters.eq("_id", objectId),
                Filters.eq("members.userId", userId)
            )
        ).firstOrNull()
        
        return team != null
    }
    
    private fun documentToTeam(doc: Document): Team {
        val membersDocuments = doc.getList("members", Document::class.java) ?: emptyList()
        val members = membersDocuments.map { memberDoc ->
            TeamMember(
                userId = memberDoc.getInteger("userId"),
                joinedAt = Instant.fromEpochMilliseconds(memberDoc.getDate("joinedAt").time)
            )
        }
        
        return Team(
            id = doc.getObjectId("_id"),
            name = doc.getString("name"),
            members = members,
            createdAt = Instant.fromEpochMilliseconds(doc.getDate("createdAt").time),
            updatedAt = Instant.fromEpochMilliseconds(doc.getDate("updatedAt").time)
        )
    }
}

private fun Instant.toJavaInstant(): java.time.Instant {
    return java.time.Instant.ofEpochSecond(epochSeconds, nanosecondsOfSecond.toLong())
}