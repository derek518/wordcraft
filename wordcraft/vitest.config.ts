import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    include: ['src/**/*.{test,spec}.ts'],
    environment: 'node',
    coverage: {
      provider: 'v8',
      include: ['src/core/**/*.ts'],
      thresholds: {
        // 核心纯逻辑（FSRS 适配、评级映射、状态机、干扰项）要求高覆盖，
        // 它们是产品正确性的唯一保障，且无 IO 依赖，没有难测的理由。
        lines: 80,
        functions: 80,
        branches: 75,
      },
    },
  },
})
