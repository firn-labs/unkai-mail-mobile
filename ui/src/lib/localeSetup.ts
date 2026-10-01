/**
 * Side-effect module: registers the app's language strategy
 * (`locale.ts`). Imported first in `main.ts`, so it runs before any
 * other module can format a message and fix the language too early.
 */
import { installLocaleStrategy } from './locale'

installLocaleStrategy()
