package database

import (
	"database/sql"
	"fmt"
	"time"

	_ "github.com/jackc/pgx/v5/stdlib" // Postgres driver via pgx.
)

func openPostgres(connStr string) (*DB, error) {
	return openPostgresWithPool(connStr, 25, 5, time.Hour)
}

// openPostgresWithPool connects to Postgres with configurable pool settings.
func openPostgresWithPool(connStr string, maxOpen, maxIdle int, connMaxLifetime time.Duration) (*DB, error) {
	sqlDB, err := sql.Open("pgx", connStr)
	if err != nil {
		return nil, fmt.Errorf("open postgres: %w", err)
	}

	if maxOpen > 0 {
		sqlDB.SetMaxOpenConns(maxOpen)
	}
	if maxIdle > 0 {
		sqlDB.SetMaxIdleConns(maxIdle)
	}
	if connMaxLifetime > 0 {
		sqlDB.SetConnMaxLifetime(connMaxLifetime)
	}

	if err := sqlDB.Ping(); err != nil {
		sqlDB.Close()
		return nil, fmt.Errorf("ping postgres: %w", err)
	}

	return &DB{sql: sqlDB, dialect: "postgres"}, nil
}
