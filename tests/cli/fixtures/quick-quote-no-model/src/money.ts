// Money is held as integer cents. Display always uses two decimals, formatted explicitly with toFixed.
export interface Line {
  id: number
  description: string
  quantity: number
  unitCents: number
}

export function lineTotal(line: Line): number {
  return line.quantity * line.unitCents
}

export function formatCents(cents: number): string {
  const sign = cents < 0 ? '-' : ''
  return sign + '$' + (Math.abs(cents) / 100).toFixed(2)
}

// A whole-number quantity of 1 or more; null when the text is not one.
export function parseQuantity(text: string): number | null {
  const n = Number(text)
  return text.trim() !== '' && Number.isInteger(n) && n >= 1 ? n : null
}

// A unit price typed in dollars (0 or more), converted to whole cents; null when the text is not a price.
export function parseCents(text: string): number | null {
  const n = Number(text)
  return text.trim() !== '' && Number.isFinite(n) && n >= 0 ? Math.round(n * 100) : null
}
