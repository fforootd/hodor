//go:build devweb

package server

import "embed"

// webAssets is an empty filesystem in dev mode.
// The Vite dev server on :5173 serves frontend assets instead.
// All ReadFile/Sub calls in server.go handle errors gracefully (404 or skipped mount).
var webAssets embed.FS
