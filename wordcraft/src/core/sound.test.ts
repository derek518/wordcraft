import { describe, it, expect } from 'vitest'
import { correctTones, noteFreq, NOTE, isSoundEnabled, setSoundEnabled } from './sound'

describe('音高换算', () => {
  it('A4 为基准 440Hz', () => {
    expect(noteFreq(0)).toBeCloseTo(440)
  })

  it('上移十二个半音为八度，频率翻倍', () => {
    expect(noteFreq(12)).toBeCloseTo(880)
    expect(noteFreq(-12)).toBeCloseTo(220)
  })

  it('常用音高落在合理频段', () => {
    // C5 约 523Hz，C6 约 1047Hz
    expect(noteFreq(NOTE.C5)).toBeCloseTo(523.25, 1)
    expect(noteFreq(NOTE.C6)).toBeCloseTo(1046.5, 1)
  })
})

describe('答对音效的连击变化', () => {
  it('零连击为基础琶音', () => {
    const tones = correctTones(0)
    expect(tones.map((t) => t.semitone)).toEqual([NOTE.C5, NOTE.E5, NOTE.G5])
  })

  it('音高随连击上移', () => {
    const base = correctTones(0)[0].semitone
    expect(correctTones(3)[0].semitone).toBeGreaterThan(base)
    expect(correctTones(9)[0].semitone).toBeGreaterThan(correctTones(3)[0].semitone)
  })

  it('移调在连击倍率门槛处发生变化', () => {
    // xpFor 的门槛是 3/5/8，音效应在这些点附近可辨
    expect(correctTones(2)[0].semitone).toBe(correctTones(0)[0].semitone)
    expect(correctTones(3)[0].semitone).not.toBe(correctTones(2)[0].semitone)
  })

  it('移调封顶，高连击不会尖到失真', () => {
    const capped = correctTones(100)[0].semitone
    expect(correctTones(1000)[0].semitone).toBe(capped)
    // 封顶后最高音不超过 C7 附近
    expect(noteFreq(correctTones(1000)[2].semitone)).toBeLessThan(2100)
  })

  it('保持琶音的上行结构', () => {
    for (const combo of [0, 3, 8, 50]) {
      const tones = correctTones(combo)
      expect(tones[0].semitone).toBeLessThan(tones[1].semitone)
      expect(tones[1].semitone).toBeLessThan(tones[2].semitone)
      // 时间上依次错开
      expect(tones[0].at).toBeLessThan(tones[1].at)
      expect(tones[1].at).toBeLessThan(tones[2].at)
    }
  })

  it('总时长在答题节奏可接受范围内', () => {
    const tones = correctTones(0)
    const total = Math.max(...tones.map((t) => t.at + t.duration))
    expect(total).toBeLessThan(0.4)
  })
})

describe('静音开关', () => {
  it('默认开启', () => {
    expect(isSoundEnabled()).toBe(true)
  })

  it('可切换', () => {
    setSoundEnabled(false)
    expect(isSoundEnabled()).toBe(false)
    setSoundEnabled(true)
    expect(isSoundEnabled()).toBe(true)
  })
})
