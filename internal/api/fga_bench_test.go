package api_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
	"testing"
)

const fgaSeedSize = 200

// ──────────────────────────────────────────────────────────────
// FGA benchmarks: measure authorization check overhead
// ──────────────────────────────────────────────────────────────

// BenchmarkFGACheck measures the latency of a single /v1/fga/check call.
// This is the hot path — every authenticated API request runs an FGA check.
func BenchmarkFGACheck(b *testing.B) {
	bs := newBenchServer(b, 10)

	checkBody := map[string]any{
		"user":     "user:admin",
		"relation": "can_manage_orgs",
		"object":   "instance:default",
	}

	// Warm up: ensure we can reach the check endpoint.
	status, body := benchDoJSON(b, bs, "POST", "/v1/fga/check", checkBody)
	if status != http.StatusOK {
		b.Fatalf("warmup check: status=%d body=%v", status, body)
	}

	b.ResetTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		status, _ := benchDoJSON(b, bs, "POST", "/v1/fga/check", checkBody)
		if status != http.StatusOK {
			b.Fatalf("check: got %d", status)
		}
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "checks/sec")
}

// BenchmarkFGAWriteTuple measures the throughput of tuple writes.
func BenchmarkFGAWriteTuple(b *testing.B) {
	bs := newBenchServer(b, 0)

	b.ResetTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		status, _ := benchDoJSON(b, bs, "POST", "/v1/fga/tuples", map[string]any{
			"tuples": []map[string]string{
				{
					"user":     fmt.Sprintf("user:bench-%d", i),
					"relation": "member",
					"object":   "org:1",
				},
			},
		})
		if status != http.StatusOK {
			b.Fatalf("write tuple: got %d", status)
		}
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "writes/sec")
}

// BenchmarkFGAReadTuples measures the throughput of reading all tuples.
func BenchmarkFGAReadTuples(b *testing.B) {
	bs := newBenchServer(b, 0)

	// Seed some tuples to read.
	for i := 0; i < 50; i++ {
		benchDoJSON(b, bs, "POST", "/v1/fga/tuples", map[string]any{
			"tuples": []map[string]string{
				{"user": fmt.Sprintf("user:read-bench-%d", i), "relation": "member", "object": "org:1"},
			},
		})
	}

	b.ResetTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		status, _ := benchDoJSON(b, bs, "GET", "/v1/fga/tuples", nil)
		if status != http.StatusOK {
			b.Fatalf("read tuples: got %d", status)
		}
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "reads/sec")
}

// BenchmarkFGAModelGraph measures the overhead of computing the model graph.
func BenchmarkFGAModelGraph(b *testing.B) {
	bs := newBenchServer(b, 0)

	b.ResetTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		status, _ := benchDoJSON(b, bs, "GET", "/v1/fga/model/graph", nil)
		if status != http.StatusOK {
			b.Fatalf("model graph: got %d", status)
		}
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "graphs/sec")
}

// BenchmarkFGAParallelChecks measures concurrent check throughput.
// This simulates a multi-user system where every request triggers an FGA check.
func BenchmarkFGAParallelChecks(b *testing.B) {
	bs := newBenchServer(b, 0)

	checkBody := map[string]any{
		"user":     "user:admin",
		"relation": "can_manage_orgs",
		"object":   "instance:default",
	}

	// Verification: make sure check works before parallel run.
	status, _ := benchDoJSON(b, bs, "POST", "/v1/fga/check", checkBody)
	if status != http.StatusOK {
		b.Fatalf("warmup check: got %d", status)
	}

	b.ResetTimer()
	b.ReportAllocs()

	b.RunParallel(func(pb *testing.PB) {
		body, _ := json.Marshal(checkBody)
		for pb.Next() {
			req, _ := http.NewRequest("POST",
				bs.ts.URL+"/v1/fga/check", bytes.NewReader(body))
			req.Header.Set("Content-Type", "application/json")
			req.Header.Set("Authorization", "Bearer "+bs.token)
			resp, err := http.DefaultClient.Do(req)
			if err != nil {
				b.Fatalf("parallel check: %v", err)
			}
			resp.Body.Close()
		}
	})
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "checks/sec")
}
