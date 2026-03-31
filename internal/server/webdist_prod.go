//go:build !devweb

package server

import "embed"

//go:embed all:webdist
var webAssets embed.FS
