package com.asdf

import com.asdf.config.DatabaseConfig
import com.asdf.controllers.TeamController
import com.asdf.services.TeamService
import io.ktor.http.*
import io.ktor.server.application.*
import io.ktor.server.response.*
import io.ktor.server.routing.*

fun Application.configureRouting() {
    val teamService = TeamService()
    val teamController = TeamController(teamService)
    
    routing {
        // 헬스체크 엔드포인트
        get("/health") {
            call.respond(HttpStatusCode.OK, mapOf("status" to "Team 서비스가 실행 중입니다"))
        }
        
        // DB 연결 테스트 엔드포인트
        get("/health/db") {
            val isConnected = DatabaseConfig.testConnection()
            if (isConnected) {
                call.respond(HttpStatusCode.OK, mapOf(
                    "status" to "DB 연결 성공",
                    "database" to "MongoDB"
                ))
            } else {
                call.respond(HttpStatusCode.ServiceUnavailable, mapOf(
                    "status" to "DB 연결 실패",
                    "database" to "MongoDB"
                ))
            }
        }
        
        // Team API 라우트 추가
        with(teamController) {
            teamRoutes()
        }
    }
}
