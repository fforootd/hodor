# Zitadel Licensing

Zitadel is open-source software licensed under the
[GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0-only).

## Component Licenses

| Component | License | Notes |
|-----------|---------|-------|
| All Rust crates (`crates/`) | AGPL-3.0-only | Including `zitadel-cloud` |
| Web SPAs (`web/src/`) | AGPL-3.0-only | Console, Login, Account |
| Documentation (`docs/`) | Apache-2.0 | |
| `packages/console-kit/` | Apache-2.0 | *(planned)* Reusable Vue components |
| `packages/auth-kit/` | MIT | *(planned)* Login widgets, OIDC client helpers |
| `packages/types/` | Apache-2.0 | *(planned)* TypeScript API types |

## Cloud Features and License Key

The `zitadel-cloud` crate contains integrations for running Zitadel as a
managed cloud service (billing, infrastructure automation, support tooling,
staff administration, and usage metering).

This code is **fully included in the public repository** under the same
AGPL-3.0-only license as the rest of the project. It is source available
and auditable by anyone.

Cloud features require a **valid license key** to activate at runtime
(`cloud.license_key` in the configuration). License keys are issued by
Zitadel and encode the licensee's entitlements.

Removing or bypassing the license key check constitutes a modification
of the software. Under AGPL-3.0 Section 13, anyone who runs a modified
version as a network service must provide the complete corresponding
source code to all users of that service.

## Community Contributions

To maintain a clear licensing structure and facilitate community
contributions, all contributions must be licensed under the
[Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0) to be
accepted. By submitting a contribution, you agree to this licensing.

This approach avoids the need for a Contributor License Agreement (CLA)
while ensuring clarity regarding license terms. We will only accept
contributions licensed under Apache 2.0.

## Commercial Licensing

Organizations whose use of Zitadel triggers AGPL obligations they
cannot meet may contact us to discuss commercial licensing options.
