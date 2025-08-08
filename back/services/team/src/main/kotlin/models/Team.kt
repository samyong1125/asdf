package com.asdf.models

import kotlinx.serialization.Contextual
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import org.bson.types.ObjectId
import kotlinx.datetime.Instant

@Serializable
data class TeamMember(
    val userId: Int,
    @Contextual
    val joinedAt: Instant
)

@Serializable
data class TeamMemberWithRole(
    val userId: Int,
    @Contextual
    val joinedAt: Instant,
    val role: String // "owner", "admin", "member", "unknown"
)

@Serializable
data class Team(
    @Contextual
    @SerialName("_id")
    val id: ObjectId? = null,
    val name: String,
    val members: List<TeamMember> = emptyList(),
    @Contextual
    val createdAt: Instant,
    @Contextual
    val updatedAt: Instant
)

@Serializable
data class CreateTeamRequest(
    val name: String
)

@Serializable
data class UpdateTeamRequest(
    val name: String
)

@Serializable
data class AddMemberRequest(
    val userId: Int
)

@Serializable
data class TeamResponse(
    val id: String,
    val name: String,
    val members: List<TeamMember>,
    @Contextual
    val createdAt: Instant,
    @Contextual
    val updatedAt: Instant
) {
    companion object {
        fun fromTeam(team: Team): TeamResponse {
            return TeamResponse(
                id = team.id.toString(),
                name = team.name,
                members = team.members,
                createdAt = team.createdAt,
                updatedAt = team.updatedAt
            )
        }
    }
}