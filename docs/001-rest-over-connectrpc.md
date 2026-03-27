# ADR-001: REST+JSON over ConnectRPC

**Status**: Accepted  
**Date**: 2026-03-26  
**Supersedes**: N/A (original architecture decision)

## Decision

Removed ConnectRPC (protobuf) entirely. All API endpoints now use plain REST+JSON over `net/http`.

## Rationale

1. **Schema-free data model doesn't benefit from proto.** The core product value is flexible identity schemas — customer-defined fields stored as JSON. Protobuf adds a compile-time type system where the payload is inherently dynamic. The result was `string data_json = 6;` — an opaque blob inside a typed message. Worst of both worlds.

2. **JSON Schema IS the contract.** Customers register schemas via API (`POST /v1/schemas`), and the OpenAPI spec dynamically reflects those schemas. Proto files can't express this dynamism.

3. **Dynamic OpenAPI > static .proto files.** `GET /openapi.json` returns a spec composed at runtime from the schema registry. Clients get a self-describing API that changes when schemas are updated — no codegen required.

4. **Simpler toolchain.** Removed `buf`, `protoc`, proto codegen. One fewer build step, fewer dependencies, faster CI.

5. **Browser-native.** No grpc-web adapter, no connect transport configuration. Just `fetch()`.

6. **Industry alignment.** Auth0, Stripe, Zitadel v1, OpenFGA — all use REST+JSON for resource CRUD. Proto/gRPC is typically reserved for internal service-to-service communication.

## What was removed

| Artifact | Purpose |
|---|---|
| `proto/` | Protocol buffer definitions |
| `gen/` | Generated Go code (protobuf + ConnectRPC) |
| `internal/service/` | ConnectRPC service handlers |
| `buf.yaml`, `buf.gen.yaml` | Buf build configuration |
| `connectrpc.com/connect` dep | ConnectRPC Go library |
| `google.golang.org/protobuf` dep | Protobuf runtime |

## What replaced it

| New file | Purpose |
|---|---|
| `internal/api/api.go` | REST identity + schema CRUD, helpers |
| `internal/api/session.go` | REST session CRUD + Internal exports for UI |
| `internal/api/event.go` | REST event list/aggregate + SSE streaming |
| `internal/api/openapi.go` | Dynamic OpenAPI 3.1 from schema registry |
