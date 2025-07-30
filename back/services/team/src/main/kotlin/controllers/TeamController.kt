package com.asdf.controllers

import com.asdf.models.*
import com.asdf.services.TeamService
import io.ktor.http.*
import io.ktor.server.application.*
import io.ktor.server.request.*
import io.ktor.server.response.*
import io.ktor.server.routing.*

class TeamController(private val teamService: TeamService) {
    
    fun Route.teamRoutes() {
        route("/api/v1/teams") {
            // 팀 생성
            post {
                val userId = getUserIdFromHeader(call) ?: run {
                    call.respond(HttpStatusCode.Unauthorized, mapOf("error" to "인증이 필요합니다"))
                    return@post
                }
                
                try {
                    val request = call.receive<CreateTeamRequest>()
                    val team = teamService.createTeam(request, userId)
                    call.respond(HttpStatusCode.Created, team)
                } catch (e: Exception) {
                    call.respond(HttpStatusCode.BadRequest, mapOf("error" to "잘못된 요청입니다"))
                }
            }
            
            // 사용자가 속한 팀 목록 조회
            get {
                val userId = getUserIdFromHeader(call) ?: run {
                    call.respond(HttpStatusCode.Unauthorized, mapOf("error" to "인증이 필요합니다"))
                    return@get
                }
                
                val teams = teamService.getTeamsByUserId(userId)
                val teamResponses = teams.map { TeamResponse.fromTeam(it) }
                call.respond(HttpStatusCode.OK, teamResponses)
            }
            
            // 특정 팀 정보 조회
            get("/{teamId}") {
                val teamId = call.parameters["teamId"] ?: run {
                    call.respond(HttpStatusCode.BadRequest, mapOf("error" to "팀 ID가 필요합니다"))
                    return@get
                }
                
                val team = teamService.getTeamById(teamId)
                if (team == null) {
                    call.respond(HttpStatusCode.NotFound, mapOf("error" to "팀을 찾을 수 없습니다"))
                } else {
                    call.respond(HttpStatusCode.OK, TeamResponse.fromTeam(team))
                }
            }
            
            // 팀 정보 수정
            put("/{teamId}") {
                val userId = getUserIdFromHeader(call) ?: run {
                    call.respond(HttpStatusCode.Unauthorized, mapOf("error" to "인증이 필요합니다"))
                    return@put
                }
                
                val teamId = call.parameters["teamId"] ?: run {
                    call.respond(HttpStatusCode.BadRequest, mapOf("error" to "팀 ID가 필요합니다"))
                    return@put
                }
                
                try {
                    val request = call.receive<UpdateTeamRequest>()
                    val success = teamService.updateTeam(teamId, request)
                    
                    if (success) {
                        call.respond(HttpStatusCode.OK, mapOf("message" to "팀 정보가 수정되었습니다"))
                    } else {
                        call.respond(HttpStatusCode.NotFound, mapOf("error" to "팀을 찾을 수 없습니다"))
                    }
                } catch (e: Exception) {
                    call.respond(HttpStatusCode.BadRequest, mapOf("error" to "잘못된 요청입니다"))
                }
            }
            
            // 팀 삭제
            delete("/{teamId}") {
                val userId = getUserIdFromHeader(call) ?: run {
                    call.respond(HttpStatusCode.Unauthorized, mapOf("error" to "인증이 필요합니다"))
                    return@delete
                }
                
                val teamId = call.parameters["teamId"] ?: run {
                    call.respond(HttpStatusCode.BadRequest, mapOf("error" to "팀 ID가 필요합니다"))
                    return@delete
                }
                
                val success = teamService.deleteTeam(teamId)
                if (success) {
                    call.respond(HttpStatusCode.OK, mapOf("message" to "팀이 삭제되었습니다"))
                } else {
                    call.respond(HttpStatusCode.NotFound, mapOf("error" to "팀을 찾을 수 없습니다"))
                }
            }
            
            // 팀 멤버 추가
            post("/{teamId}/members") {
                val userId = getUserIdFromHeader(call) ?: run {
                    call.respond(HttpStatusCode.Unauthorized, mapOf("error" to "인증이 필요합니다"))
                    return@post
                }
                
                val teamId = call.parameters["teamId"] ?: run {
                    call.respond(HttpStatusCode.BadRequest, mapOf("error" to "팀 ID가 필요합니다"))
                    return@post
                }
                
                try {
                    val request = call.receive<AddMemberRequest>()
                    val success = teamService.addMember(teamId, request.userId)
                    
                    if (success) {
                        call.respond(HttpStatusCode.OK, mapOf("message" to "멤버가 추가되었습니다"))
                    } else {
                        call.respond(HttpStatusCode.BadRequest, mapOf("error" to "멤버 추가에 실패했습니다"))
                    }
                } catch (e: Exception) {
                    call.respond(HttpStatusCode.BadRequest, mapOf("error" to "잘못된 요청입니다"))
                }
            }
            
            // 팀 멤버 목록 조회
            get("/{teamId}/members") {
                val teamId = call.parameters["teamId"] ?: run {
                    call.respond(HttpStatusCode.BadRequest, mapOf("error" to "팀 ID가 필요합니다"))
                    return@get
                }
                
                val members = teamService.getTeamMembers(teamId)
                if (members == null) {
                    call.respond(HttpStatusCode.NotFound, mapOf("error" to "팀을 찾을 수 없습니다"))
                } else {
                    call.respond(HttpStatusCode.OK, members)
                }
            }
            
            // 팀 멤버 제거
            delete("/{teamId}/members/{userId}") {
                val currentUserId = getUserIdFromHeader(call) ?: run {
                    call.respond(HttpStatusCode.Unauthorized, mapOf("error" to "인증이 필요합니다"))
                    return@delete
                }
                
                val teamId = call.parameters["teamId"] ?: run {
                    call.respond(HttpStatusCode.BadRequest, mapOf("error" to "팀 ID가 필요합니다"))
                    return@delete
                }
                
                val targetUserId = call.parameters["userId"]?.toIntOrNull() ?: run {
                    call.respond(HttpStatusCode.BadRequest, mapOf("error" to "유효한 사용자 ID가 필요합니다"))
                    return@delete
                }
                
                val success = teamService.removeMember(teamId, targetUserId)
                if (success) {
                    call.respond(HttpStatusCode.OK, mapOf("message" to "멤버가 제거되었습니다"))
                } else {
                    call.respond(HttpStatusCode.BadRequest, mapOf("error" to "멤버 제거에 실패했습니다"))
                }
            }
        }
        
        // 특정 사용자가 속한 팀 목록 조회
        route("/api/v1/users") {
            get("/{userId}/teams") {
                val userId = call.parameters["userId"]?.toIntOrNull() ?: run {
                    call.respond(HttpStatusCode.BadRequest, mapOf("error" to "유효한 사용자 ID가 필요합니다"))
                    return@get
                }
                
                val teams = teamService.getTeamsByUserId(userId)
                val teamResponses = teams.map { TeamResponse.fromTeam(it) }
                call.respond(HttpStatusCode.OK, teamResponses)
            }
        }
    }
}

// X-User-ID 헤더에서 사용자 ID 추출
private fun getUserIdFromHeader(call: ApplicationCall): Int? {
    val userIdHeader = call.request.headers["X-User-ID"]
    return userIdHeader?.toIntOrNull()
}