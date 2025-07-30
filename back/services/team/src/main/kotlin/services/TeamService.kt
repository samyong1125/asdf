package com.asdf.services

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
        
        val result = teamsCollection.deleteOne(Filters.eq("_id", objectId))
        return result.deletedCount > 0
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
        return result.modifiedCount > 0
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
        return result.modifiedCount > 0
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