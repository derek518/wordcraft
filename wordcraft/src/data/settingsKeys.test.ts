import { describe, it, expect } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { globSync } from 'node:fs'

/**
 * 前端写的每一个设置键，后端都必须允许写。
 *
 * 这个检查扫源码，不靠人工维护清单——因为人工清单已经失效两次：
 * `study_level` / `study_days` 加进设置面板时漏了后端白名单（界面上点了
 * 毫无反应），修好之后写 `library_fingerprint` 又漏了一次，启动即报错。
 *
 * 第一版的后端测试列了一张手工键表，它挡不住「我根本没想到要加进去」的那种。
 * 只有从源码扫出来的清单才会自己跟上。
 */

const root = resolve(__dirname, '../..')

function frontendKeys(): string[] {
  const files = globSync('src/**/*.{ts,tsx}', { cwd: root })
    .filter((f) => !f.endsWith('.test.ts') && !f.endsWith('.test.tsx'))
  const keys = new Set<string>()
  for (const f of files) {
    const text = readFileSync(resolve(root, f), 'utf-8')
    // 直接调用与设置面板的 save() 包装，两种写法都要覆盖
    for (const m of text.matchAll(/(?:setSetting|save)\(\s*'([a-z_]+)'/g)) {
      keys.add(m[1])
    }
  }
  return [...keys].sort()
}

function backendWritable(): string[] {
  const text = readFileSync(resolve(root, 'src-tauri/src/commands/config.rs'), 'utf-8')
  const table = text.slice(text.indexOf('const WRITABLE'), text.indexOf('#[derive(Clone, Copy)]'))
  return [...table.matchAll(/\("([a-z_]+)",/g)].map((m) => m[1]).sort()
}

describe('设置键契约', () => {
  it('前端写的键后端全部允许', () => {
    const missing = frontendKeys().filter((k) => !backendWritable().includes(k))
    expect(missing, `这些键前端会写但后端白名单里没有：${missing.join(', ')}`).toEqual([])
  })

  it('扫描确实找到了键，不是空跑', () => {
    // 正则若因重构失效，上面那条会因为「空集合 ⊆ 任何集合」而假绿
    const keys = frontendKeys()
    expect(keys.length).toBeGreaterThan(5)
    expect(keys).toContain('study_days')
    expect(backendWritable().length).toBeGreaterThan(5)
  })
})
