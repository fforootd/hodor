package cli

import (
	"strings"

	"github.com/pterm/pterm"
)

// PrintLogo prints the Zitadel ASCII logo to the terminal.
func PrintLogo() {
	logo := `
  ______ _ _            _      _ 
 |___  /(_) |          | |    | |
    / / |_| |_ __ _  __| | ___| | 
   / /  | | __/ _' |/ _' |/ _ \ | 
  / /__ | | || (_| | (_| |  __/ | 
 /_____||_|\__\__,_|\__,_|\___|_| 
`
	// Replace single quotes with backticks for the standard font style
	logo = strings.ReplaceAll(logo, "'", "`")

	// Print the logo left-aligned
	pterm.Println(pterm.FgCyan.Sprint(logo))
}
