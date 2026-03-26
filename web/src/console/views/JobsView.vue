<template>
  <div class="jobs">
    <div class="card">
      <h4>Registered Jobs</h4>
      <div class="job-list">
        <div class="job-row" v-for="job in jobs" :key="job">
          <span class="job-name">{{ job }}</span>
          <span class="badge">scheduled</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
const jobs = ['lake_writer', 'session_gc', 'event_gc']
</script>

<style scoped>
.card { background: #fff; border: 1px solid #e5e7eb; border-radius: 10px; }
h4 { padding: 1.25rem; font-size: 0.8125rem; font-weight: 600; color: #6b7280; text-transform: uppercase; border-bottom: 1px solid #e5e7eb; }
.job-row {
  display: flex; justify-content: space-between; align-items: center;
  padding: 0.75rem 1.25rem; border-bottom: 1px solid #f3f4f6;
}
.job-row:last-child { border-bottom: none; }
.job-name { font-size: 0.875rem; font-weight: 500; color: #1a1a2e; }
.badge { font-size: 0.75rem; padding: 0.125rem 0.5rem; background: #ecfdf5; color: #059669; border-radius: 4px; }
</style>
