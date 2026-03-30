import type {
  AppBootstrapErrorDetail,
  AppBootstrapErrorKind,
  AppBootstrapState,
} from '@/bootstrap/app-bootstrap'
import {
  nextAppBootstrapRetryDelay,
  shouldRetryAppBootstrap,
  toAppBootstrapErrorDetail,
} from '@/bootstrap/app-bootstrap'

export type LoginInitState = AppBootstrapState
export type LoginErrorKind = AppBootstrapErrorKind
export type LoginErrorDetail = AppBootstrapErrorDetail

export const toLoginErrorDetail = toAppBootstrapErrorDetail
export const nextLoginInitRetryDelay = nextAppBootstrapRetryDelay
export const shouldRetryLoginInit = shouldRetryAppBootstrap
