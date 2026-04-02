package httptestutil

import (
	"net"
	"net/http"
	"net/http/httptest"
	"testing"
)

// NewServer starts an HTTP test server or skips when the environment blocks
// loopback listeners, which happens in some sandboxed test runs.
func NewServer(t testing.TB, handler http.Handler) *httptest.Server {
	t.Helper()

	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Skipf("skipping test that requires a local listener: %v", err)
	}

	ts := httptest.NewUnstartedServer(handler)
	ts.Listener = ln
	ts.Start()
	return ts
}
