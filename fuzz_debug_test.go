package api_test

import (
"os"
"testing"
)

func TestFuzzTempDir(t *testing.T) {
	dir := t.TempDir()
	files, _ := os.ReadDir(dir)
	t.Logf("dir: %s, files: %v", dir, files)
}
