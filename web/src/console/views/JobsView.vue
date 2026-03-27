<template>
  <div class="space-y-6">
    <div>
      <h1 class="text-2xl font-bold tracking-tight">Background Jobs</h1>
      <p class="text-muted-foreground">Registered recurring jobs and their status.</p>
    </div>

    <Card>
      <CardHeader>
        <CardTitle class="text-sm font-medium">Registered Jobs</CardTitle>
        <CardDescription>{{ jobs.length }} jobs configured</CardDescription>
      </CardHeader>
      <CardContent class="p-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Job Name</TableHead>
              <TableHead>Status</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="job in jobs" :key="job">
              <TableCell class="font-medium font-mono text-sm">{{ job }}</TableCell>
              <TableCell>
                <Badge variant="outline" class="border-green-200 bg-green-50 text-green-700">
                  scheduled
                </Badge>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Badge } from '@/components/ui/badge'

const jobs = ['lake_writer', 'session_gc', 'event_gc']
</script>
