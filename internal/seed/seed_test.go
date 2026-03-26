package seed

import (
	"context"
	"os"
	"testing"
)

func TestSubstituteEnvVars_Basic(t *testing.T) {
	os.Setenv("TEST_CLIENT_ID", "my-client-123")
	defer os.Unsetenv("TEST_CLIENT_ID")

	input := []byte(`client_id: ${TEST_CLIENT_ID}`)
	result := substituteEnvVars(input)

	if string(result) != "client_id: my-client-123" {
		t.Errorf("got %q", string(result))
	}
}

func TestSubstituteEnvVars_Default(t *testing.T) {
	os.Unsetenv("MISSING_VAR")

	input := []byte(`value: ${MISSING_VAR:-fallback_value}`)
	result := substituteEnvVars(input)

	if string(result) != "value: fallback_value" {
		t.Errorf("got %q", string(result))
	}
}

func TestSubstituteEnvVars_EnvOverridesDefault(t *testing.T) {
	os.Setenv("PRESENT_VAR", "real-value")
	defer os.Unsetenv("PRESENT_VAR")

	input := []byte(`value: ${PRESENT_VAR:-fallback}`)
	result := substituteEnvVars(input)

	if string(result) != "value: real-value" {
		t.Errorf("got %q", string(result))
	}
}

func TestSubstituteEnvVars_NoVars(t *testing.T) {
	input := []byte(`plain: text without vars`)
	result := substituteEnvVars(input)

	if string(result) != "plain: text without vars" {
		t.Errorf("got %q", string(result))
	}
}

func TestSubstituteEnvVars_MultipleVars(t *testing.T) {
	os.Setenv("VAR_A", "alpha")
	os.Setenv("VAR_B", "beta")
	defer os.Unsetenv("VAR_A")
	defer os.Unsetenv("VAR_B")

	input := []byte(`a: ${VAR_A}, b: ${VAR_B}`)
	result := substituteEnvVars(input)

	if string(result) != "a: alpha, b: beta" {
		t.Errorf("got %q", string(result))
	}
}

func TestLoadAndApply_FileNotFound(t *testing.T) {
	err := LoadAndApply(context.TODO(), nil, "/nonexistent/file.yaml")
	if err == nil {
		t.Error("expected error for missing file")
	}
}
