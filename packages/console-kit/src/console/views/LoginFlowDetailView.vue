<template>
  <div class="space-y-5 pb-8">
    <section class="rounded-2xl border bg-card shadow-sm">
      <div class="flex flex-col gap-4 p-4 lg:flex-row lg:items-start lg:justify-between">
        <div class="flex items-start gap-4">
          <Button
            variant="ghost"
            size="icon"
            class="mt-0.5 shrink-0"
            @click="$router.push('/login-flows')"
          >
            <ArrowLeft class="size-4" />
          </Button>
          <div class="space-y-2.5">
            <div class="flex flex-wrap items-center gap-2">
              <Badge v-if="flow?.is_default" variant="default">Default</Badge>
              <Badge :variant="stateVariant(flow?.state)" class="text-xs capitalize">{{
                flow?.state || 'draft'
              }}</Badge>
              <Badge v-if="templateSource" variant="secondary">Template-backed</Badge>
            </div>
            <div class="space-y-1">
              <h1 class="text-2xl font-semibold tracking-tight lg:text-[2rem]">
                {{ flow?.name || 'Login Flow' }}
              </h1>
              <p class="max-w-2xl text-sm text-muted-foreground">
                <template v-if="flow?.is_default">
                  Fallback experience for anyone who does not match a more specific flow.
                </template>
                <template v-else>
                  Configure how this flow starts, protects sign-in, and looks.
                </template>
              </p>
            </div>

            <div class="flex flex-wrap gap-2 text-xs text-muted-foreground">
              <div class="rounded-full border bg-muted/40 px-2.5 py-1">
                Strategy: <span class="font-medium text-foreground">{{ strategyLabel }}</span>
              </div>
              <div class="rounded-full border bg-muted/40 px-2.5 py-1">
                Layout: <span class="font-medium text-foreground">{{ currentLayoutLabel }}</span>
              </div>
              <div class="rounded-full border bg-muted/40 px-2.5 py-1">
                Priority: <span class="font-medium text-foreground">{{ form.priority }}</span>
              </div>
            </div>
          </div>
        </div>

        <div class="flex flex-col gap-2 sm:flex-row">
          <Button
            v-if="flow && flow.state !== 'active' && flow.state !== 'archived'"
            variant="outline"
            :disabled="promoting"
            @click="promoteFlow"
          >
            <Spinner v-if="promoting" class="mr-2 size-4" />
            <ArrowUp v-else class="mr-2 size-4" />
            Promote to {{ flow?.state === 'draft' ? 'Testing' : 'Active' }}
          </Button>
          <Button :disabled="saving" @click="saveFlow">
            <Spinner v-if="saving" class="mr-2 size-4" />
            Save changes
          </Button>
        </div>
      </div>
    </section>

    <div
      v-if="flow?.is_default"
      class="flex items-center gap-2 rounded-xl border bg-muted/30 px-4 py-3 text-sm text-muted-foreground"
    >
      <Shield class="size-4 shrink-0 text-primary" />
      <p>
        <span class="font-medium text-foreground">Fallback flow.</span> Applies when no targeted
        flow matches.
      </p>
    </div>

    <div class="grid items-start gap-5 xl:grid-cols-[minmax(0,1fr)_24rem]">
      <div class="min-w-0 space-y-5">
        <Tabs v-model="activePanel" class="space-y-3">
          <div
            class="flex flex-col gap-3 rounded-xl border bg-card p-3 shadow-sm sm:flex-row sm:items-center sm:justify-between"
          >
            <div>
              <p class="text-sm font-medium">Flow editor</p>
              <p class="text-xs text-muted-foreground">Experience and protection</p>
            </div>
            <TabsList class="grid w-full grid-cols-2 sm:w-[280px]">
              <TabsTrigger value="experience" class="gap-2">
                <Palette class="size-4" />
                Experience
              </TabsTrigger>
              <TabsTrigger value="protection" class="gap-2">
                <ShieldCheck class="size-4" />
                Protection
              </TabsTrigger>
            </TabsList>
          </div>

          <TabsContent value="experience" class="mt-0 space-y-4">
            <Card class="overflow-hidden shadow-sm">
              <CardHeader class="border-b bg-muted/20 pb-3">
                <div class="flex items-start gap-3">
                  <div class="rounded-lg border bg-background p-2">
                    <LayoutPanelTop class="size-4 text-muted-foreground" />
                  </div>
                  <div>
                    <CardTitle class="text-base">General</CardTitle>
                    <p class="mt-1 text-sm text-muted-foreground">
                      Name, order, and how sign-in starts.
                    </p>
                  </div>
                </div>
              </CardHeader>
              <CardContent class="space-y-4 pt-4">
                <div class="grid gap-4 sm:grid-cols-2">
                  <div class="space-y-2">
                    <Label for="name">Name</Label>
                    <Input id="name" v-model="form.name" placeholder="Default Login" />
                  </div>
                  <div class="space-y-2">
                    <Label for="priority">Priority</Label>
                    <Input
                      id="priority"
                      v-model.number="form.priority"
                      type="number"
                      min="0"
                      max="1000"
                    />
                    <p class="text-xs text-muted-foreground">
                      Higher priority flows are evaluated first.
                    </p>
                  </div>
                </div>

                <div class="space-y-2">
                  <Label for="strategy">Flow strategy</Label>
                  <Select v-model="form.strategy">
                    <SelectTrigger id="strategy">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="identifier_first">Identifier first</SelectItem>
                      <SelectItem value="passkey_first">Passkey first</SelectItem>
                      <SelectItem value="sso_only">SSO only</SelectItem>
                      <SelectItem value="custom">Custom</SelectItem>
                    </SelectContent>
                  </Select>
                  <p class="text-xs text-muted-foreground">
                    Controls the first step in the flow. Use Marketplace templates for full presets.
                  </p>
                </div>
              </CardContent>
            </Card>

            <Card class="overflow-hidden shadow-sm">
              <CardHeader class="border-b bg-muted/20 pb-3">
                <div class="flex items-start gap-3">
                  <div class="rounded-lg border bg-background p-2">
                    <ImageIcon class="size-4 text-muted-foreground" />
                  </div>
                  <div>
                    <CardTitle class="text-base">Branding</CardTitle>
                    <p class="mt-1 text-sm text-muted-foreground">
                      Assets for the hosted login page and live preview.
                    </p>
                  </div>
                </div>
              </CardHeader>
              <CardContent class="space-y-4 pt-4">
                <div class="grid gap-4 lg:grid-cols-2">
                  <div
                    v-for="assetField in brandingAssetFields"
                    :key="assetField.key"
                    class="rounded-xl border bg-muted/10 p-3.5"
                  >
                    <div class="flex items-start justify-between gap-3">
                      <div>
                        <p class="text-sm font-medium">{{ assetField.label }}</p>
                        <p class="mt-1 text-xs text-muted-foreground">
                          {{ assetField.description }}
                        </p>
                      </div>
                      <Button
                        v-if="form.branding[assetField.key]"
                        type="button"
                        size="sm"
                        variant="ghost"
                        :disabled="assetBusy[assetField.key]"
                        @click="removeBrandingAsset(assetField.key)"
                      >
                        Remove
                      </Button>
                    </div>

                    <div class="mt-4 space-y-3">
                      <div class="rounded-lg border border-dashed bg-background/80 p-3">
                        <div
                          v-if="form.branding[assetField.key]"
                          class="flex min-h-24 items-center justify-center rounded-md bg-muted/20 p-3"
                        >
                          <img
                            :src="form.branding[assetField.key]"
                            :alt="assetField.label"
                            class="max-h-20 max-w-full rounded object-contain"
                          />
                        </div>
                        <div
                          v-else
                          class="flex min-h-24 flex-col items-center justify-center rounded-md bg-muted/20 text-center"
                        >
                          <ImageIcon class="mb-2 size-4 text-muted-foreground" />
                          <p class="text-xs text-muted-foreground">No asset uploaded yet</p>
                        </div>
                      </div>

                      <div class="space-y-2">
                        <Label :for="`upload-${assetField.key}`">Upload file</Label>
                        <input
                          :id="`upload-${assetField.key}`"
                          type="file"
                          accept="image/*,.svg"
                          class="block w-full rounded-md border border-input bg-background px-3 py-2 text-sm file:mr-3 file:rounded-sm file:border-0 file:bg-muted file:px-2 file:py-1 file:text-xs file:font-medium"
                          :disabled="assetBusy[assetField.key]"
                          @change="onBrandingFileSelected(assetField.key, $event)"
                        />
                      </div>

                      <div class="grid gap-2 sm:grid-cols-[1fr_auto]">
                        <Input
                          v-model="assetImportUrls[assetField.key]"
                          :placeholder="assetField.placeholder"
                          :disabled="assetBusy[assetField.key]"
                        />
                        <Button
                          type="button"
                          variant="outline"
                          :disabled="assetBusy[assetField.key] || !assetImportUrls[assetField.key]"
                          @click="importBrandingAsset(assetField.key)"
                        >
                          {{ assetBusy[assetField.key] ? 'Importing…' : 'Import URL' }}
                        </Button>
                      </div>

                      <p
                        v-if="
                          assetField.key === 'cover_image' &&
                          !['split', 'card_image'].includes(form.layout)
                        "
                        class="text-xs text-muted-foreground"
                      >
                        Cover art only appears in Split and Card with image layouts.
                      </p>
                    </div>
                  </div>
                </div>

                <Separator />

                <div class="flex items-start gap-3 rounded-xl border bg-muted/10 p-4">
                  <Checkbox
                    id="hide-powered-by"
                    :checked="form.branding.hide_zitadel_branding"
                    @update:checked="
                      (value: boolean | 'indeterminate') =>
                        (form.branding.hide_zitadel_branding = value === true)
                    "
                  />
                  <div class="space-y-1">
                    <Label for="hide-powered-by" class="text-sm font-medium"
                      >Hide “Powered by Zitadel” footer</Label
                    >
                    <p class="text-xs text-muted-foreground">
                      Removes the footer in the hosted page and live preview.
                    </p>
                  </div>
                </div>
              </CardContent>
            </Card>
          </TabsContent>

          <TabsContent value="protection" class="mt-0 space-y-4">
            <Card class="overflow-hidden shadow-sm">
              <CardHeader class="border-b bg-muted/20 pb-3">
                <div class="flex items-start justify-between gap-3">
                  <div class="flex items-start gap-3">
                    <div class="rounded-lg border bg-background p-2">
                      <ShieldCheck class="size-4 text-muted-foreground" />
                    </div>
                    <div>
                      <CardTitle class="text-base">Captcha</CardTitle>
                      <p class="mt-1 text-sm text-muted-foreground">
                        Require a human check before continuing.
                      </p>
                    </div>
                  </div>
                  <Badge
                    :variant="form.captcha.mode !== 'never' ? 'default' : 'outline'"
                    class="text-xs"
                  >
                    {{ form.captcha.mode !== 'never' ? 'Active' : 'Disabled' }}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent class="space-y-4 pt-4">
                <div class="grid gap-4 lg:grid-cols-2">
                  <div class="space-y-2">
                    <Label>Provider</Label>
                    <Select v-model="form.captcha.provider">
                      <SelectTrigger><SelectValue /></SelectTrigger>
                      <SelectContent>
                        <SelectItem value="altcha">Altcha (PoW, self-hosted)</SelectItem>
                        <SelectItem value="hcaptcha">hCaptcha</SelectItem>
                        <SelectItem value="recaptcha">reCAPTCHA</SelectItem>
                        <SelectItem value="turnstile">Cloudflare Turnstile</SelectItem>
                        <SelectItem value="none">None</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>

                  <div class="space-y-2">
                    <Label>Mode</Label>
                    <Select v-model="form.captcha.mode">
                      <SelectTrigger><SelectValue /></SelectTrigger>
                      <SelectContent>
                        <SelectItem value="always">Always show</SelectItem>
                        <SelectItem value="risk_based"
                          >Risk-based (show when suspicious)</SelectItem
                        >
                        <SelectItem value="never">Disabled</SelectItem>
                      </SelectContent>
                    </Select>
                  </div>
                </div>

                <Alert
                  v-if="form.captcha.mode === 'risk_based'"
                  class="border-amber-500/30 bg-amber-500/5"
                >
                  <Sparkles class="size-4 text-amber-500" />
                  <AlertTitle class="text-sm font-medium">Adaptive challenge</AlertTitle>
                  <AlertDescription class="text-xs text-muted-foreground">
                    The login runtime evaluates local signals and only asks for a challenge when a
                    sign-in looks suspicious.
                  </AlertDescription>
                </Alert>

                <div v-if="form.captcha.provider === 'altcha'" class="space-y-2">
                  <div class="flex items-center justify-between">
                    <Label>Difficulty (1-5)</Label>
                    <span class="text-sm font-mono text-muted-foreground">{{
                      form.captcha.difficulty
                    }}</span>
                  </div>
                  <input
                    v-model.number="form.captcha.difficulty"
                    type="range"
                    min="1"
                    max="5"
                    class="w-full accent-primary"
                  />
                  <p class="text-xs text-muted-foreground">
                    Higher values increase proof-of-work cost. A value of 3 is a sensible default.
                  </p>
                </div>
              </CardContent>
            </Card>

            <div class="grid gap-4 lg:grid-cols-2">
              <Card class="overflow-hidden shadow-sm">
                <CardHeader class="border-b bg-muted/20 pb-3">
                  <div class="flex items-start justify-between gap-3">
                    <div class="flex items-start gap-3">
                      <div class="rounded-lg border bg-background p-2">
                        <Radar class="size-4 text-muted-foreground" />
                      </div>
                      <div>
                        <CardTitle class="text-base">Browser fingerprinting</CardTitle>
                        <p class="mt-1 text-sm text-muted-foreground">
                          Add a passive browser signal for returning-user detection.
                        </p>
                      </div>
                    </div>
                    <Badge
                      :variant="form.fingerprint.enabled ? 'default' : 'outline'"
                      class="text-xs"
                    >
                      {{ form.fingerprint.enabled ? 'Active' : 'Disabled' }}
                    </Badge>
                  </div>
                </CardHeader>
                <CardContent class="space-y-4 pt-4">
                  <div class="flex items-start gap-3">
                    <Checkbox
                      id="fp-on"
                      :checked="form.fingerprint.enabled"
                      @update:checked="
                        (value: boolean | 'indeterminate') =>
                          (form.fingerprint.enabled = value === true)
                      "
                    />
                    <div class="space-y-1">
                      <Label for="fp-on" class="text-sm font-medium">Enable fingerprinting</Label>
                      <p class="text-xs text-muted-foreground">
                        Adds a passive browser signal to the login flow.
                      </p>
                    </div>
                  </div>

                  <div v-if="form.fingerprint.enabled" class="space-y-4">
                    <div class="space-y-2">
                      <Label>Provider</Label>
                      <Select v-model="form.fingerprint.provider">
                        <SelectTrigger><SelectValue /></SelectTrigger>
                        <SelectContent>
                          <SelectItem value="thumbmarkjs">ThumbmarkJS (recommended)</SelectItem>
                          <SelectItem value="built_in">Built-in (canvas + WebGL)</SelectItem>
                        </SelectContent>
                      </Select>
                    </div>

                    <div class="flex items-start gap-3 rounded-xl border bg-muted/10 p-4">
                      <Checkbox
                        id="fp-persist"
                        :checked="form.fingerprint.persist"
                        @update:checked="
                          (value: boolean | 'indeterminate') =>
                            (form.fingerprint.persist = value === true)
                        "
                      />
                      <div class="space-y-1">
                        <Label for="fp-persist" class="text-sm font-medium">
                          Persist across sessions
                        </Label>
                        <p class="text-xs text-muted-foreground">
                          Reuse the signal for returning-user detection.
                        </p>
                      </div>
                    </div>
                  </div>
                </CardContent>
              </Card>

              <Card class="overflow-hidden shadow-sm">
                <CardHeader class="border-b bg-muted/20 pb-3">
                  <div class="flex items-start gap-3">
                    <div class="rounded-lg border bg-background p-2">
                      <Gauge class="size-4 text-muted-foreground" />
                    </div>
                    <div>
                      <CardTitle class="text-base">Rate limiting</CardTitle>
                      <p class="mt-1 text-sm text-muted-foreground">
                        Slow abusive retries before they become a support problem.
                      </p>
                    </div>
                  </div>
                </CardHeader>
                <CardContent class="space-y-4 pt-4">
                  <div class="grid gap-4 sm:grid-cols-2">
                    <div class="space-y-2">
                      <Label for="max-attempts">Max attempts</Label>
                      <Input
                        id="max-attempts"
                        v-model.number="form.rateLimit.maxAttempts"
                        type="number"
                        min="1"
                        max="100"
                      />
                    </div>
                    <div class="space-y-2">
                      <Label for="window">Window (seconds)</Label>
                      <Input
                        id="window"
                        v-model.number="form.rateLimit.windowSeconds"
                        type="number"
                        min="60"
                        max="3600"
                      />
                    </div>
                  </div>

                  <div class="grid gap-4 sm:grid-cols-2">
                    <div class="space-y-2">
                      <Label for="lockout">Lockout (seconds)</Label>
                      <Input
                        id="lockout"
                        v-model.number="form.rateLimit.lockoutSeconds"
                        type="number"
                        min="0"
                        max="86400"
                      />
                    </div>
                    <div class="space-y-2">
                      <Label>Scope</Label>
                      <Select v-model="form.rateLimit.scope">
                        <SelectTrigger><SelectValue /></SelectTrigger>
                        <SelectContent>
                          <SelectItem value="ip">Per IP</SelectItem>
                          <SelectItem value="identifier">Per identifier</SelectItem>
                          <SelectItem value="fingerprint">Per fingerprint</SelectItem>
                        </SelectContent>
                      </Select>
                    </div>
                  </div>
                </CardContent>
              </Card>
            </div>

            <Card class="overflow-hidden shadow-sm">
              <CardHeader class="border-b bg-muted/20 pb-3">
                <div class="flex items-start justify-between gap-3">
                  <div class="flex items-start gap-3">
                    <div class="rounded-lg border bg-background p-2">
                      <Activity class="size-4 text-muted-foreground" />
                    </div>
                    <div>
                      <CardTitle class="text-base">Telemetry</CardTitle>
                      <p class="mt-1 text-sm text-muted-foreground">
                        Collect client-side signals for debugging.
                      </p>
                    </div>
                  </div>
                  <Badge :variant="form.telemetry.enabled ? 'default' : 'outline'" class="text-xs">
                    {{ form.telemetry.enabled ? 'Active' : 'Disabled' }}
                  </Badge>
                </div>
              </CardHeader>
              <CardContent class="space-y-4 pt-4">
                <div class="flex items-start gap-3">
                  <Checkbox
                    id="tel-on"
                    :checked="form.telemetry.enabled"
                    @update:checked="
                      (value: boolean | 'indeterminate') =>
                        (form.telemetry.enabled = value === true)
                    "
                  />
                  <div class="space-y-1">
                    <Label for="tel-on" class="text-sm font-medium"
                      >Collect browser telemetry</Label
                    >
                    <p class="text-xs text-muted-foreground">
                      Useful for troubleshooting and flow quality checks.
                    </p>
                  </div>
                </div>

                <div v-if="form.telemetry.enabled" class="space-y-2">
                  <div class="flex items-center justify-between">
                    <Label>Sample rate</Label>
                    <span class="text-sm font-mono text-muted-foreground">
                      {{ Math.round(form.telemetry.sampleRate * 100) }}%
                    </span>
                  </div>
                  <input
                    v-model.number="form.telemetry.sampleRate"
                    type="range"
                    min="0"
                    max="1"
                    step="0.1"
                    class="w-full accent-primary"
                  />
                </div>
              </CardContent>
            </Card>
          </TabsContent>
        </Tabs>
      </div>

      <div class="space-y-3 xl:sticky xl:top-4">
        <Card class="overflow-hidden shadow-sm">
          <CardHeader class="border-b bg-muted/20 pb-3">
            <div class="flex items-start gap-3">
              <div class="rounded-lg border bg-background p-2">
                <BadgeCheck class="size-4 text-muted-foreground" />
              </div>
              <div>
                <CardTitle class="text-base">Live preview</CardTitle>
                <p class="mt-1 text-sm text-muted-foreground">
                  A live preview of the hosted login experience.
                </p>
              </div>
            </div>
          </CardHeader>

          <CardContent class="space-y-4 pt-4">
            <div
              class="rounded-xl border bg-[radial-gradient(circle_at_top_left,rgba(242,85,67,0.16),transparent_45%),linear-gradient(180deg,rgba(244,244,246,0.95),rgba(244,244,246,0.55))] p-3"
            >
              <div class="mb-3 flex items-center justify-between gap-3">
                <div>
                  <p class="text-sm font-medium">Layout</p>
                </div>
                <Badge variant="secondary">{{ currentLayoutLabel }}</Badge>
              </div>

              <div class="flex flex-wrap gap-1.5">
                <Button
                  v-for="layout in layouts"
                  :key="layout.id"
                  type="button"
                  size="sm"
                  :variant="form.layout === layout.id ? 'default' : 'outline'"
                  @click="form.layout = layout.id"
                >
                  {{ layout.label }}
                </Button>
              </div>
            </div>

            <div class="overflow-hidden rounded-xl border bg-muted/20 p-3">
              <LoginShell :branding="previewBranding" preview>
                <LoginNodeRenderer
                  :flow-step="previewStep"
                  :preview="true"
                  :form-data="previewFormData"
                  :confirm-passwords="previewConfirmPasswords"
                  @update:form-data="handlePreviewFormDataUpdate"
                  @update:confirm-passwords="handlePreviewConfirmPasswordsUpdate"
                />
              </LoginShell>
            </div>

            <Separator />

            <div class="grid gap-2 text-xs">
              <div class="rounded-lg border bg-background px-3 py-2">
                <span class="font-medium text-foreground">Strategy</span>
                <span class="text-muted-foreground"> · {{ strategyLabel }}</span>
              </div>
              <div v-if="templateSource" class="rounded-lg border bg-background px-3 py-2">
                <span class="font-medium text-foreground">Template source</span>
                <span class="font-mono text-muted-foreground"> · {{ templateSource }}</span>
              </div>
              <div v-if="form.telemetry.enabled" class="rounded-lg border bg-background px-3 py-2">
                <span class="font-medium text-foreground">Telemetry</span>
                <span class="text-muted-foreground">
                  · {{ Math.round(form.telemetry.sampleRate * 100) }}% sample rate</span
                >
              </div>
              <div
                v-if="form.fingerprint.enabled"
                class="rounded-lg border bg-background px-3 py-2"
              >
                <span class="font-medium text-foreground">Fingerprinting</span>
                <span class="text-muted-foreground"> · {{ form.fingerprint.provider }}</span>
              </div>
              <div class="rounded-lg border bg-background px-3 py-2">
                <span class="font-medium text-foreground">Rate limit</span>
                <span class="text-muted-foreground">
                  · {{ form.rateLimit.maxAttempts }} attempts / {{ form.rateLimit.windowSeconds }}s
                  (per {{ form.rateLimit.scope }})
                </span>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { computed, onMounted, reactive, ref } from 'vue'
  import { useRoute, useRouter } from 'vue-router'
  import { api } from '@/api/client'
  import type { FlowBranding } from '@/api/branding'
  import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
  import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
  import { Badge } from '@/components/ui/badge'
  import { Button } from '@/components/ui/button'
  import { Checkbox } from '@/components/ui/checkbox'
  import { Input } from '@/components/ui/input'
  import { Label } from '@/components/ui/label'
  import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
  } from '@/components/ui/select'
  import { Separator } from '@/components/ui/separator'
  import { Spinner } from '@/components/ui/spinner'
  import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
  import {
    Activity,
    ArrowLeft,
    ArrowUp,
    BadgeCheck,
    Gauge,
    ImageIcon,
    LayoutPanelTop,
    Palette,
    Radar,
    Shield,
    ShieldCheck,
    Sparkles,
  } from 'lucide-vue-next'
  import LoginShell from '@/login/components/LoginShell.vue'
  import LoginNodeRenderer from '@/login/components/LoginNodeRenderer.vue'
  import { buildPreviewFlowStep } from '@/login/preview'

  const route = useRoute()
  const router = useRouter()
  const activePanel = ref('experience')

  interface LoginFlow {
    id: string
    name: string
    strategy: string
    is_default: boolean
    enabled: boolean
    state: string
    priority: number
    audience: any
    auth_methods: any
    config: any
    metadata?: any
    created_at: string
    updated_at: string
  }

  type BrandingAssetField = 'logo_url' | 'logo_dark' | 'cover_image' | 'favicon'

  const flow = ref<LoginFlow | null>(null)
  const currentConfig = ref<Record<string, any>>({})
  const saving = ref(false)
  const promoting = ref(false)
  const previewFormData = reactive<Record<string, any>>({})
  const previewConfirmPasswords = reactive<Record<string, string>>({})
  const assetImportUrls = reactive<Record<string, string>>({
    logo_url: '',
    logo_dark: '',
    cover_image: '',
    favicon: '',
  })
  const assetBusy = reactive<Record<string, boolean>>({
    logo_url: false,
    logo_dark: false,
    cover_image: false,
    favicon: false,
  })

  const brandingAssetFields = [
    {
      key: 'logo_url',
      label: 'Logo',
      description: 'Shown in the card header on light layouts.',
      placeholder: 'https://example.com/logo.svg',
    },
    {
      key: 'logo_dark',
      label: 'Dark Logo',
      description: 'Shown when dark mode is active.',
      placeholder: 'https://example.com/logo-dark.svg',
    },
    {
      key: 'cover_image',
      label: 'Cover Image',
      description: 'Shown in Split and Card with image layouts.',
      placeholder: 'https://example.com/cover.jpg',
    },
    {
      key: 'favicon',
      label: 'Favicon',
      description: 'Shown in the browser tab.',
      placeholder: 'https://example.com/favicon.png',
    },
  ] as const

  const layouts = [
    { id: 'centered', label: 'Centered' },
    { id: 'split', label: 'Split' },
    { id: 'muted', label: 'Muted' },
    { id: 'card_image', label: 'Card with image' },
    { id: 'minimal', label: 'Minimal' },
  ]

  function replacePreviewRecord(target: Record<string, any>, next: Record<string, any>) {
    Object.keys(target).forEach((key) => {
      if (!(key in next)) {
        delete target[key]
      }
    })
    Object.assign(target, next)
  }

  function handlePreviewFormDataUpdate(nextValue: Record<string, any>) {
    replacePreviewRecord(previewFormData, nextValue)
  }

  function handlePreviewConfirmPasswordsUpdate(nextValue: Record<string, string>) {
    replacePreviewRecord(previewConfirmPasswords, nextValue)
  }

  const strategyLabels: Record<string, string> = {
    identifier_first: 'Identifier first',
    passkey_first: 'Passkey first',
    sso_only: 'SSO only',
    custom: 'Custom',
  }

  const form = reactive({
    name: '',
    priority: 0,
    strategy: 'identifier_first',
    layout: 'centered',
    branding: {
      logo_url: '',
      logo_dark: '',
      cover_image: '',
      favicon: '',
      hide_zitadel_branding: false,
    },
    captcha: {
      provider: 'altcha',
      mode: 'risk_based',
      difficulty: 3,
    },
    fingerprint: {
      enabled: true,
      provider: 'thumbmarkjs',
      persist: true,
    },
    rateLimit: {
      maxAttempts: 5,
      windowSeconds: 300,
      lockoutSeconds: 900,
      scope: 'ip',
    },
    telemetry: {
      enabled: true,
      sampleRate: 1.0,
    },
  })

  const templateSource = computed(() => {
    const catalog = flow.value?.metadata?._catalog || flow.value?.metadata?.catalog || null
    return catalog?.template_id || null
  })

  const currentLayoutLabel = computed(
    () => layouts.find((layout) => layout.id === form.layout)?.label || 'Centered',
  )
  const strategyLabel = computed(() => strategyLabels[form.strategy] || 'Identifier first')

  const previewBranding = computed<FlowBranding>(() => {
    const branding = currentConfig.value.branding || {}
    return {
      heading: branding.heading || 'Welcome back',
      description: branding.description || 'Sign in to your account',
      logo_url: form.branding.logo_url || '',
      org_name: branding.org_name || 'Acme Corp',
      colors: {
        primary: '#f25543',
        primary_foreground: '#ffffff',
        background: '#f4f4f6',
        surface: '#ffffff',
        text: '#0f0f11',
        muted: '#f4f4f6',
        accent: '#f25543',
        border: '#e5e5e7',
        error: '#ef4444',
        ...(branding.colors || {}),
      },
      font_family: branding.font_family || 'Arimo, Inter, system-ui, sans-serif',
      font_url: branding.font_url || '',
      texts: branding.texts || {},
      custom_css: branding.custom_css || '',
      hide_zitadel_branding: form.branding.hide_zitadel_branding,
      layout: form.layout,
      dark_mode: branding.dark_mode || 'light',
      cover_image: form.branding.cover_image || '',
      logo_dark: form.branding.logo_dark || '',
      favicon: form.branding.favicon || '',
      border_radius: branding.border_radius || 'md',
      terms_url: branding.terms_url || '',
      privacy_url: branding.privacy_url || '',
      social_position: branding.social_position || 'bottom',
      consent: branding.consent || [],
    }
  })

  const previewStep = computed(() =>
    buildPreviewFlowStep({
      strategy: form.strategy,
      branding: previewBranding.value,
      captchaEnabled: form.captcha.mode !== 'never' && form.captcha.provider !== 'none',
      captchaProvider: form.captcha.provider,
    }),
  )

  function stateVariant(state?: string): 'default' | 'secondary' | 'outline' | 'destructive' {
    switch (state) {
      case 'active':
        return 'default'
      case 'testing':
        return 'secondary'
      case 'archived':
        return 'destructive'
      default:
        return 'outline'
    }
  }

  function safeJSON(value: unknown): Record<string, any> {
    if (!value) return {}
    if (typeof value === 'string') {
      try {
        return JSON.parse(value)
      } catch {
        return {}
      }
    }
    return typeof value === 'object' ? (value as Record<string, any>) : {}
  }

  function populateForm(f: LoginFlow) {
    form.name = f.name || ''
    form.priority = f.priority || 0
    form.strategy = f.strategy || 'identifier_first'

    const config = safeJSON(f.config)
    currentConfig.value = config

    form.layout = config.branding?.layout || 'centered'
    form.branding.logo_url = config.branding?.logo_url || ''
    form.branding.logo_dark = config.branding?.logo_dark || ''
    form.branding.cover_image = config.branding?.cover_image || ''
    form.branding.favicon = config.branding?.favicon || ''
    form.branding.hide_zitadel_branding = config.branding?.hide_zitadel_branding ?? false

    if (config.captcha) {
      form.captcha.provider = config.captcha.provider || 'altcha'
      form.captcha.mode = config.captcha.mode || 'risk_based'
      form.captcha.difficulty = config.captcha.difficulty || 3
    }
    if (config.fingerprint) {
      form.fingerprint.enabled = config.fingerprint.enabled !== false
      form.fingerprint.provider = config.fingerprint.provider || 'thumbmarkjs'
      form.fingerprint.persist = config.fingerprint.persist !== false
    }
    if (config.rate_limit) {
      form.rateLimit.maxAttempts = config.rate_limit.max_attempts || 5
      form.rateLimit.windowSeconds = config.rate_limit.window_seconds || 300
      form.rateLimit.lockoutSeconds = config.rate_limit.lockout_seconds || 900
      form.rateLimit.scope = config.rate_limit.scope || 'ip'
    }
    if (config.telemetry) {
      form.telemetry.enabled = config.telemetry.enabled !== false
      form.telemetry.sampleRate = config.telemetry.sample_rate ?? 1.0
    }
  }

  async function loadFlow() {
    const id = route.params.id as string
    try {
      const resp = await api.get<LoginFlow>(`/v1/login-flows/${id}`)
      flow.value = resp
      populateForm(resp)
    } catch {
      router.push('/login-flows')
    }
  }

  async function saveFlow() {
    if (!flow.value) return
    saving.value = true
    try {
      const nextConfig = {
        ...currentConfig.value,
        captcha: {
          ...(currentConfig.value.captcha || {}),
          provider: form.captcha.provider,
          mode: form.captcha.mode,
          difficulty: form.captcha.difficulty,
          on: ['login'],
        },
        fingerprint: {
          ...(currentConfig.value.fingerprint || {}),
          enabled: form.fingerprint.enabled,
          provider: form.fingerprint.provider,
          persist: form.fingerprint.persist,
          on: ['login'],
        },
        rate_limit: {
          ...(currentConfig.value.rate_limit || {}),
          max_attempts: form.rateLimit.maxAttempts,
          window_seconds: form.rateLimit.windowSeconds,
          lockout_seconds: form.rateLimit.lockoutSeconds,
          scope: form.rateLimit.scope,
        },
        telemetry: {
          ...(currentConfig.value.telemetry || {}),
          enabled: form.telemetry.enabled,
          sample_rate: form.telemetry.sampleRate,
        },
        branding: {
          ...(currentConfig.value.branding || {}),
          layout: form.layout,
          logo_url: form.branding.logo_url,
          logo_dark: form.branding.logo_dark,
          cover_image: form.branding.cover_image,
          favicon: form.branding.favicon,
          hide_zitadel_branding: form.branding.hide_zitadel_branding,
        },
      }

      await api.patch(`/v1/login-flows/${flow.value.id}`, {
        name: form.name,
        strategy: form.strategy,
        priority: form.priority,
        is_default: flow.value.is_default,
        config: nextConfig,
      })
      await loadFlow()
    } catch (e: any) {
      console.error('Failed to save login flow:', e)
    } finally {
      saving.value = false
    }
  }

  function extractAssetId(url: string): string {
    if (!url) return ''
    try {
      const parsed = new URL(url, window.location.origin)
      const match = parsed.pathname.match(/\/assets\/login\/([^/]+)$/)
      return match?.[1] || ''
    } catch {
      return ''
    }
  }

  async function onBrandingFileSelected(field: BrandingAssetField, event: Event) {
    if (!flow.value) return
    const input = event.target as HTMLInputElement
    const file = input.files?.[0]
    if (!file) return

    const body = new FormData()
    body.append('slot', field)
    body.append('file', file)

    assetBusy[field] = true
    try {
      const resp = await api.postForm<{ url: string }>(
        `/v1/login-flows/${flow.value.id}/assets`,
        body,
      )
      form.branding[field] = resp.url
    } catch (e) {
      console.error(`Failed to upload ${field}:`, e)
    } finally {
      assetBusy[field] = false
      input.value = ''
    }
  }

  async function importBrandingAsset(field: BrandingAssetField) {
    if (!flow.value || !assetImportUrls[field]) return
    assetBusy[field] = true
    try {
      const resp = await api.post<{ url: string }>(
        `/v1/login-flows/${flow.value.id}/assets/import`,
        {
          slot: field,
          url: assetImportUrls[field],
        },
      )
      form.branding[field] = resp.url
      assetImportUrls[field] = ''
    } catch (e) {
      console.error(`Failed to import ${field}:`, e)
    } finally {
      assetBusy[field] = false
    }
  }

  async function removeBrandingAsset(field: BrandingAssetField) {
    if (!flow.value) return
    const assetID = extractAssetId(form.branding[field])
    assetBusy[field] = true
    try {
      if (assetID) {
        await api.delete(`/v1/login-flows/${flow.value.id}/assets/${assetID}`)
      }
      form.branding[field] = ''
    } catch (e) {
      console.error(`Failed to remove ${field}:`, e)
    } finally {
      assetBusy[field] = false
    }
  }

  async function promoteFlow() {
    if (!flow.value) return
    promoting.value = true
    try {
      await api.post(`/v1/login-flows/${flow.value.id}/promote`, {})
      await loadFlow()
    } catch (e: any) {
      console.error('Failed to promote flow:', e)
    } finally {
      promoting.value = false
    }
  }

  onMounted(loadFlow)
</script>
