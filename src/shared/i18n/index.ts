import type { Language } from '@shared/types'
import type { Messages } from './messages'
import { en } from './en'
import { ru } from './ru'
import { zh } from './zh'

/** All languages; each one lives in its own config file next to this index. */
export const MESSAGES: Record<Language, Messages> = { en, ru, zh }

export type { Messages }
