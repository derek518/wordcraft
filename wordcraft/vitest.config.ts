import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    // 组件测试需要 DOM。happy-dom 比 jsdom 快得多，
    // 而我们只用到 DOM 的基本查询与事件，用不上 jsdom 的完整度
    environment: 'happy-dom',
    coverage: {
      provider: 'v8',
      include: ['src/core/**/*.ts'],
      // 组件不纳入覆盖率门槛：那会诱使人为凑数写断言稀薄的测试。
      // 组件测试要钉的是**变异测试证明会失手**的几条不变量，不是行数
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
