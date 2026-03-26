import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory('/console'),
  routes: [
    { path: '/', name: 'dashboard', component: () => import('./views/DashboardView.vue') },
    { path: '/identities', name: 'identities', component: () => import('./views/IdentityListView.vue') },
    { path: '/identities/new', name: 'identity-create', component: () => import('./views/IdentityCreateView.vue') },
    { path: '/identities/:id', name: 'identity-detail', component: () => import('./views/IdentityDetailView.vue') },
    { path: '/schemas', name: 'schemas', component: () => import('./views/SchemaListView.vue') },
    { path: '/sessions', name: 'sessions', component: () => import('./views/SessionListView.vue') },
    { path: '/events', name: 'events', component: () => import('./views/EventListView.vue') },
    { path: '/jobs', name: 'jobs', component: () => import('./views/JobsView.vue') },
  ],
})

export default router
