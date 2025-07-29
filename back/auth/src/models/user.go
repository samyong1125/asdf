package models

import (
	"auth-server/src/config"
	"time"
)

type User struct {
	ID        int       `json:"id"`
	Email     string    `json:"email"`
	Password  string    `json:"-"` // 응답에서 제외
	CreatedAt time.Time `json:"created_at"`
	UpdatedAt time.Time `json:"updated_at"`
}

func InitUserTable() error {
	query := `
	CREATE TABLE IF NOT EXISTS users (
		id SERIAL PRIMARY KEY,
		email VARCHAR(255) UNIQUE NOT NULL,
		password VARCHAR(255) NOT NULL,
		created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
		updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
	)`

	_, err := config.DB.Exec(query)
	return err
}

func CreateUser(email, hashedPassword string) (*User, error) {
	query := `
	INSERT INTO users (email, password) 
	VALUES ($1, $2) 
	RETURNING id, email, created_at, updated_at`

	user := &User{}
	err := config.DB.QueryRow(query, email, hashedPassword).Scan(
		&user.ID, &user.Email, &user.CreatedAt, &user.UpdatedAt,
	)
	if err != nil {
		return nil, err
	}

	return user, nil
}

func GetUserByEmail(email string) (*User, error) {
	query := `SELECT id, email, password, created_at, updated_at FROM users WHERE email = $1`

	user := &User{}
	err := config.DB.QueryRow(query, email).Scan(
		&user.ID, &user.Email, &user.Password, &user.CreatedAt, &user.UpdatedAt,
	)
	if err != nil {
		return nil, err
	}

	return user, nil
}

func GetUserByID(id int) (*User, error) {
	query := `SELECT id, email, created_at, updated_at FROM users WHERE id = $1`

	user := &User{}
	err := config.DB.QueryRow(query, id).Scan(
		&user.ID, &user.Email, &user.CreatedAt, &user.UpdatedAt,
	)
	if err != nil {
		return nil, err
	}

	return user, nil
}