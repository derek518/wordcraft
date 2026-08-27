import { describe, it, expect } from 'vitest'
import { fingerprintOf } from './libraryFingerprint'

/**
 * 词库指纹。决定扩充后的词能不能到达老用户的机器。
 *
 * 先前导入只在 `onboarding_done !== 'true'` 时跑——引导一走完，词库再扩充
 * 也永远进不来。四级词加了等于没加，而界面上看不出任何异常。
 */
describe('词库指纹', () => {
  it('内容变了指纹就变', () => {
    expect(fingerprintOf('[{"word":"a"}]')).not.toBe(fingerprintOf('[{"word":"b"}]'))
  })

  it('同样多的字符换内容也要变', () => {
    // 只比词数会漏掉「数量相同、内容不同」，那是最难察觉的一种失效
    const a = fingerprintOf('abcd')
    const b = fingerprintOf('abce')
    expect(a).not.toBe(b)
  })

  it('内容不变则指纹稳定', () => {
    const text = JSON.stringify([{ word: 'apply' }, { word: 'via' }])
    expect(fingerprintOf(text)).toBe(fingerprintOf(text))
  })

  it('扩充词库必定改变指纹', () => {
    const before = JSON.stringify(Array.from({ length: 3657 }, (_, i) => ({ w: i })))
    const after = JSON.stringify(Array.from({ length: 5278 }, (_, i) => ({ w: i })))
    expect(fingerprintOf(before)).not.toBe(fingerprintOf(after))
  })
})
