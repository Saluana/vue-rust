// Types that ok-imported-props.vue and ok-imported-alias.vue import. The frontend reads them; nothing runs them.
export interface ImportedProps {
  label: string
  count: number
  enabled?: boolean
}

export type AliasProps = {
  title?: string
  size?: number
  open?: boolean
  tags?: string[]
}
