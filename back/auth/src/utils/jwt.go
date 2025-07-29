package utils

import (
	"context"
	"fmt"
	"os"
	"strconv"
	"time"

	"auth-server/src/config"

	"github.com/golang-jwt/jwt/v5"
	"github.com/google/uuid"
)

type Claims struct {
	UserID int    `json:"user_id"`
	Email  string `json:"email"`
	jwt.RegisteredClaims
}

func GenerateTokens(userID int, email string) (string, string, error) {
	secret := os.Getenv("JWT_SECRET")
	if secret == "" {
		return "", "", fmt.Errorf("JWT_SECRET not set")
	}

	// Access Token (15분)
	accessClaims := Claims{
		UserID: userID,
		Email:  email,
		RegisteredClaims: jwt.RegisteredClaims{
			ExpiresAt: jwt.NewNumericDate(time.Now().Add(15 * time.Minute)),
			IssuedAt:  jwt.NewNumericDate(time.Now()),
			Subject:   strconv.Itoa(userID),
		},
	}

	accessToken := jwt.NewWithClaims(jwt.SigningMethodHS256, accessClaims)
	accessTokenString, err := accessToken.SignedString([]byte(secret))
	if err != nil {
		return "", "", err
	}

	// Refresh Token (180일) - Redis에 저장할 UUID
	refreshToken := uuid.New().String()
	refreshKey := fmt.Sprintf("refresh:%d", userID)

	ctx := context.Background()
	err = config.Redis.Set(ctx, refreshKey, refreshToken, 180*24*time.Hour).Err()
	if err != nil {
		return "", "", err
	}

	return accessTokenString, refreshToken, nil
}

func VerifyToken(tokenString string) (*Claims, error) {
	secret := os.Getenv("JWT_SECRET")
	if secret == "" {
		return nil, fmt.Errorf("JWT_SECRET not set")
	}

	token, err := jwt.ParseWithClaims(tokenString, &Claims{}, func(token *jwt.Token) (interface{}, error) {
		if _, ok := token.Method.(*jwt.SigningMethodHMAC); !ok {
			return nil, fmt.Errorf("unexpected signing method: %v", token.Header["alg"])
		}
		return []byte(secret), nil
	})

	if err != nil {
		return nil, err
	}

	if claims, ok := token.Claims.(*Claims); ok && token.Valid {
		return claims, nil
	}

	return nil, fmt.Errorf("invalid token")
}

func RefreshAccessToken(userID int, refreshToken string) (string, error) {
	ctx := context.Background()
	refreshKey := fmt.Sprintf("refresh:%d", userID)

	// Redis에서 저장된 refresh token 확인
	storedToken, err := config.Redis.Get(ctx, refreshKey).Result()
	if err != nil {
		return "", fmt.Errorf("refresh token not found")
	}

	if storedToken != refreshToken {
		return "", fmt.Errorf("invalid refresh token")
	}

	// 새로운 access token 생성 (사용자 정보 조회 필요)
	secret := os.Getenv("JWT_SECRET")
	if secret == "" {
		return "", fmt.Errorf("JWT_SECRET not set")
	}

	// 간단히 userID만으로 새 토큰 생성 (실제로는 DB에서 사용자 정보 조회)
	claims := Claims{
		UserID: userID,
		RegisteredClaims: jwt.RegisteredClaims{
			ExpiresAt: jwt.NewNumericDate(time.Now().Add(15 * time.Minute)),
			IssuedAt:  jwt.NewNumericDate(time.Now()),
			Subject:   strconv.Itoa(userID),
		},
	}

	token := jwt.NewWithClaims(jwt.SigningMethodHS256, claims)
	return token.SignedString([]byte(secret))
}

func RevokeRefreshToken(userID int) error {
	ctx := context.Background()
	refreshKey := fmt.Sprintf("refresh:%d", userID)
	return config.Redis.Del(ctx, refreshKey).Err()
}