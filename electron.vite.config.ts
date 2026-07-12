import { resolve } from 'path'
import { defineConfig } from 'electron-vite'

export default defineConfig({
  main: {
    build: {
      sourcemap: true,
      rollupOptions: {
        external: ['electron-store']
      }
    },
    resolve: {
      alias: { '@shared': resolve('src/shared') }
    }
  },
  preload: {
    build: { sourcemap: true },
    resolve: {
      alias: { '@shared': resolve('src/shared') }
    }
  },
  renderer: {
    build: { sourcemap: true },
    resolve: {
      alias: { '@shared': resolve('src/shared') }
    }
  }
})
