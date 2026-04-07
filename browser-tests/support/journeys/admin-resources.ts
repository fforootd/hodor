import { expect, type Page } from '@playwright/test'

import { escapeRegex } from '../browser'

export async function openResourceEditTab(page: Page) {
  await page.getByRole('tab', { name: /Edit & API/i }).click()
  await expect(page.getByRole('button', { name: /Save changes/i })).toBeVisible({
    timeout: 15_000,
  })
}

export async function updateSchemaField(page: Page, label: string, value: string) {
  const field = page.getByLabel(new RegExp(`^${escapeRegex(label)}$`, 'i')).first()
  await expect(field).toBeVisible({ timeout: 15_000 })
  await field.fill(value)
}

export async function updateResourceJson(page: Page, data: Record<string, unknown>) {
  const editor = page.getByRole('textbox', { name: /Editor content/i })
  await expect(editor).toBeVisible({ timeout: 15_000 })
  await editor.click()
  await editor.press('Meta+A').catch(() => {})
  await editor.press('Control+A').catch(() => {})
  await page.keyboard.type(JSON.stringify(data, null, 2))
}

export async function saveResourceChanges(page: Page) {
  const saveButton = page.getByRole('button', { name: /Save changes/i })
  await expect(saveButton).toBeEnabled({ timeout: 15_000 })
  await saveButton.click()
}

export async function deleteCockpitResource(page: Page, singularTitle: string) {
  const deleteButtonName = new RegExp(`Delete ${escapeRegex(singularTitle)}`, 'i')
  await page.getByRole('button', { name: deleteButtonName }).first().click()

  const dialog = page.getByRole('dialog')
  await expect(dialog).toBeVisible({ timeout: 15_000 })
  await dialog.getByRole('button', { name: deleteButtonName }).click()
}

export async function openInstanceDetailFromList(page: Page, domain: string) {
  const link = page.getByRole('link', { name: domain }).first()
  await expect(link).toBeVisible({ timeout: 15_000 })
  const href = await link.getAttribute('href')
  if (!href) {
    throw new Error(`Unable to resolve instance detail link for ${domain}`)
  }
  await page.goto(href.endsWith('/settings') ? href : `${href}/settings`)
}
