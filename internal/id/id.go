// Package id provides distributed ID generation using Sonyflake.
// IDs are 64-bit integers (time-ordered, machine-scoped) suitable
// for use as database primary keys stored as BIGINT.
package id

import (
	"fmt"
	"sync"

	"github.com/sony/sonyflake"
)

var (
	sf   *sonyflake.Sonyflake
	once sync.Once
)

// Init initializes the global Sonyflake generator.
// Call once at startup. If machineID is nil, the default
// (lower 16 bits of private IP) is used.
func Init(machineID func() (uint16, error)) {
	once.Do(func() {
		var settings sonyflake.Settings
		if machineID != nil {
			settings.MachineID = machineID
		}
		sf = sonyflake.NewSonyflake(settings)
	})
}

// New generates a new unique, time-ordered 64-bit ID.
func New() (int64, error) {
	if sf == nil {
		Init(nil)
	}
	id, err := sf.NextID()
	if err != nil {
		return 0, fmt.Errorf("generate id: %w", err)
	}
	return int64(id), nil
}

// MustNew generates a new ID or panics. Use in tests or init paths.
func MustNew() int64 {
	id, err := New()
	if err != nil {
		panic(fmt.Sprintf("id.MustNew: %v", err))
	}
	return id
}
