# 빌드 스테이지
FROM golang:1.24-alpine AS builder

WORKDIR /app

# 의존성 복사 및 다운로드
COPY go.mod go.sum ./
RUN go mod download

# 소스 코드 복사 및 빌드
COPY . .
RUN go mod tidy
RUN CGO_ENABLED=0 GOOS=linux go build -a -installsuffix cgo -o auth-server ./src

# 실행 스테이지
FROM alpine:latest

# 빌드된 바이너리 복사
COPY --from=builder /app/auth-server .

# 포트 노출
EXPOSE 15001

# 실행
CMD ["./auth-server"]
