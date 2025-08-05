package com.asdf.services

import com.asdf.clients.SentinelClient
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
    
    suspend fun updateTeam(teamId: String, request: UpdateTeamRequest): Boolean {
        val objectId = try {
            ObjectId(teamId)
        } catch (e: IllegalArgumentException) {
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
    
    suspend fun deleteTeam(teamId: String): Boolean {
        val objectId = try {
            ObjectId(teamId)
        } catch (e: IllegalArgumentException) {
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
    
    suspend fun addMember(teamId: String, userId: Int): Boolean {
        val objectId = try {
            ObjectId(teamId)
        } catch (e: IllegalArgumentException) {
            return false
        }
        
        // 이미 멤버인지 확인
        val existing = teamsCollection.find(
            Filters.and(
                Filters.eq("_id", objectId),
                Filters.eq("members.userId", userId)
            )
        ).firstOrNull()
        
        if (existing != null) {
            return false // 이미 멤버임
        }
        
        val nowDate = Date.from(Clock.System.now().toJavaInstant())
        val newMember = Document()
            .append("userId", userId)
            .append("joinedAt", nowDate)
        
        val update = Updates.combine(
            Updates.push("members", newMember),
            Updates.set("updatedAt", nowDate)
        )
        
        val result = teamsCollection.updateOne(Filters.eq("_id", objectId), update)
        val success = result.modifiedCount > 0
        
        // ✨ MongoDB 업데이트 성공 시 Sentinel에 멤버십 권한 추가
        if (success) {
            val sentinelSuccess = sentinelClient.addTeamMember(teamId, userId)
            if (!sentinelSuccess) {
                println("⚠️ 멤버 추가는 성공했지만 Sentinel 권한 동기화 실패 - teamId: $teamId, userId: $userId")
            }
        }
        
        return success
    }
    
    suspend fun removeMember(teamId: String, userId: Int): Boolean {
        val objectId = try {
            ObjectId(teamId)
        } catch (e: IllegalArgumentException) {
            return false
        }
        
        val nowDate = Date.from(Clock.System.now().toJavaInstant())
        val update = Updates.combine(
            Updates.pull("members", Document("userId", userId)),
            Updates.set("updatedAt", nowDate)
        )
        
        val result = teamsCollection.updateOne(Filters.eq("_id", objectId), update)
        val success = result.modifiedCount > 0
        
        // ✨ MongoDB 업데이트 성공 시 Sentinel에서 멤버십 권한 제거
        if (success) {
            val sentinelSuccess = sentinelClient.removeTeamMember(teamId, userId)
            if (!sentinelSuccess) {
                println("⚠️ 멤버 제거는 성공했지만 Sentinel 권한 동기화 실패 - teamId: $teamId, userId: $userId")
            }
        }
        
        return success
    }
    
    suspend fun getTeamMembers(teamId: String): List<TeamMember>? {
        val team = getTeamById(teamId)
        return team?.members
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