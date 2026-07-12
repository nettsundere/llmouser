import { mkdirSync, rmSync } from 'fs'
import {
  COVERAGE_ENABLED,
  MAIN_COVERAGE_DIR,
  RAW_COVERAGE_ROOT,
  RENDERER_COVERAGE_DIR
} from './coverage-paths'

export default function globalSetup(): void {
  if (!COVERAGE_ENABLED) return

  // Start each coverage run from a clean slate.
  rmSync(RAW_COVERAGE_ROOT, { recursive: true, force: true })
  mkdirSync(MAIN_COVERAGE_DIR, { recursive: true })
  mkdirSync(RENDERER_COVERAGE_DIR, { recursive: true })
}
