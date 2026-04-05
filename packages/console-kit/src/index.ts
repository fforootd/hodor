// @zitadel/console-kit — shared Vue component library
//
// This package contains all reusable views, composables, UI components,
// and the configurable API client used by both the standalone console
// (self-hosted) and the cloud portal.

// API client
export { configureApi, type ApiClientConfig } from './api/client'
export { api, requestJSON, requestText, ApiError, type ApiErrorKind } from './api/client'
export { getApiBaseUrl, getCurrentOrgHeader, getCurrentInstanceHeader } from './api/client'
export { resetTraceContext, credentialsMode, parseApiErrorPayload } from './api/client'

// Composables
export { useOrgContext } from './console/composables/useOrgContext'
export { useResourceList } from './console/composables/useResourceList'
export { useResourceDetail } from './console/composables/useResourceDetail'
export { useResourceCreate } from './console/composables/useResourceCreate'

// Bootstrap
export { useAppBootstrap } from './bootstrap/app-bootstrap'
