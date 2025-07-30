package main

import (
	"auth-server/src/config"
	"auth-server/src/handlers"
	"log"
	"os"

	"github.com/gin-gonic/gin"
)

func main() {
	// DB 초기화
	config.InitDB()
	defer config.DB.Close()

	// Redis 초기화
	config.InitRedis()
	defer config.Redis.Close()

	log.Println("Database and Redis connections established")

	// Gin 라우터 설정
	r := gin.Default()

	// CORS 미들웨어
	r.Use(func(c *gin.Context) {
		c.Header("Access-Control-Allow-Origin", "*")
		c.Header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS")
		c.Header("Access-Control-Allow-Headers", "Content-Type, Authorization")
		
		if c.Request.Method == "OPTIONS" {
			c.AbortWithStatus(204)
			return
		}
		c.Next()
	})

	// API 라우트
	api := r.Group("/api/v1")
	{
		// 인증 관련 엔드포인트
		api.POST("/register", handlers.Register)
		api.POST("/login", handlers.Login)
		api.POST("/refresh", handlers.Refresh)
		api.POST("/logout", handlers.Logout)
		api.GET("/verify", handlers.Verify) // Envoy가 호출
		api.POST("/verify", handlers.Verify) // POST 요청 지원
		api.PUT("/verify", handlers.Verify) // PUT 요청 지원
		api.DELETE("/verify", handlers.Verify) // DELETE 요청 지원
		api.PATCH("/verify", handlers.Verify) // PATCH 요청 지원
		api.GET("/verify/*path", handlers.Verify) // Envoy가 path_prefix로 호출하는 경우
		api.POST("/verify/*path", handlers.Verify) // POST path_prefix 지원
		api.PUT("/verify/*path", handlers.Verify) // PUT path_prefix 지원
		api.DELETE("/verify/*path", handlers.Verify) // DELETE path_prefix 지원
		api.PATCH("/verify/*path", handlers.Verify) // PATCH path_prefix 지원
	}

	// 헬스체크 엔드포인트
	r.GET("/health", func(c *gin.Context) {
		c.JSON(200, gin.H{"status": "ok"})
	})

	// 서버 시작
	port := os.Getenv("PORT")
	if port == "" {
		port = "15001"
	}

	log.Printf("Auth Server starting on port %s", port)
	log.Println("Available endpoints:")
	log.Println("  POST /api/v1/register - User registration")
	log.Println("  POST /api/v1/login - User login")
	log.Println("  POST /api/v1/refresh - Refresh access token")
	log.Println("  POST /api/v1/logout - User logout")
	log.Println("  GET  /api/v1/verify - Token verification (for Envoy)")
	log.Println("  GET  /health - Health check")
	
	r.Run(":" + port)
}