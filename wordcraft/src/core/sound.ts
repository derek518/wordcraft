/**
 * 音效合成。spec F6：答对/答错/升级均有即时音效，可静音，反馈延迟 <100ms。
 *
 * 用 Web Audio 振荡器现场合成，不打包任何音频文件：
 * - 素材约束要求原创或 CC0，合成音天然满足
 * - 零体积，不增加 Windows 安装包大小
 * - 延迟可控——加载 mp3 要经历 fetch → decode → play，振荡器是即时的
 */

/** 十二平均律：从 A4 = 440Hz 起算 `semitones` 个半音后的频率。 */
export function noteFreq(semitonesFromA4: number): number {
  return 440 * Math.pow(2, semitonesFromA4 / 12)
}

/** 常用音高相对 A4 的半音数。 */
export const NOTE = {
  G4: -2,
  C5: 3,
  D5: 5,
  E5: 7,
  G5: 10,
  C6: 15,
} as const

interface Tone {
  /** 相对 A4 的半音数 */
  semitone: number
  /** 起始时刻偏移（秒） */
  at: number
  /** 持续时长（秒） */
  duration: number
  /** 峰值音量 0..1 */
  gain?: number
}

/**
 * AudioContext 延迟创建。
 *
 * 浏览器要求 AudioContext 在用户手势后创建，否则会处于 suspended 状态。
 * 静音时完全不创建——不只是不发声，连上下文都不占。
 */
let context: AudioContext | null = null
let enabled = true

export function setSoundEnabled(value: boolean): void {
  enabled = value
  if (!value && context) {
    void context.close()
    context = null
  }
}

export function isSoundEnabled(): boolean {
  return enabled
}

function getContext(): AudioContext | null {
  if (!enabled) return null
  if (!context) {
    const Ctor = window.AudioContext ?? (window as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext
    if (!Ctor) return null
    context = new Ctor()
  }
  // 自动播放策略可能让上下文处于 suspended，首次交互时恢复
  if (context.state === 'suspended') void context.resume()
  return context
}

/**
 * 播放一组音。
 *
 * 每个音用独立的振荡器与增益节点，`stop()` 后浏览器自动回收——
 * 不复用节点，因为 OscillatorNode 停止后不能重启。
 */
function play(tones: Tone[]): void {
  const ctx = getContext()
  if (!ctx) return

  const now = ctx.currentTime
  for (const tone of tones) {
    const osc = ctx.createOscillator()
    const gainNode = ctx.createGain()

    // 三角波比正弦厚、比方波柔，接近 8-bit 游戏音而不刺耳
    osc.type = 'triangle'
    osc.frequency.value = noteFreq(tone.semitone)

    const start = now + tone.at
    const peak = tone.gain ?? 0.18

    // 用指数衰减而非线性——听感上更接近真实乐器的自然衰减。
    // 目标值不能为 0，指数曲线到不了零点
    gainNode.gain.setValueAtTime(0.0001, start)
    gainNode.gain.exponentialRampToValueAtTime(peak, start + 0.01)
    gainNode.gain.exponentialRampToValueAtTime(0.0001, start + tone.duration)

    osc.connect(gainNode).connect(ctx.destination)
    osc.start(start)
    osc.stop(start + tone.duration)
  }
}

// ── 音效 ──────────────────────────

/** 答错：下行二度，短促。 */
export function playIncorrect(): void {
  play([
    { semitone: NOTE.D5, at: 0, duration: 0.12 },
    { semitone: NOTE.G4, at: 0.09, duration: 0.22 },
  ])
}

/** 升级：完整上行琶音，比答对更长更亮。 */
export function playLevelUp(): void {
  play([
    { semitone: NOTE.C5, at: 0, duration: 0.14 },
    { semitone: NOTE.E5, at: 0.07, duration: 0.14 },
    { semitone: NOTE.G5, at: 0.14, duration: 0.16 },
    { semitone: NOTE.C6, at: 0.21, duration: 0.35, gain: 0.22 },
  ])
}

/** 会话完成：柔和的收束和弦。 */
export function playSessionComplete(): void {
  play([
    { semitone: NOTE.C5, at: 0, duration: 0.5, gain: 0.14 },
    { semitone: NOTE.E5, at: 0.02, duration: 0.5, gain: 0.12 },
    { semitone: NOTE.G5, at: 0.04, duration: 0.55, gain: 0.12 },
  ])
}

/** 答对音效。`combo` 是本次答对之前已连对的次数。 */
export function playCorrect(combo: number): void {
  play(correctTones(combo))
}

/** 连击移调的上限（半音）。超过纯五度会明显偏尖。 */
const MAX_COMBO_SHIFT = 7

/**
 * 构造答对音效的音符序列。
 *
 * 基础形态是 C5-E5-G5 上行琶音，整体音高随连击上移——这是玩家每答对一次
 * 都会听到的声音，一天上百次，所以移调必须封顶，否则高连击会尖到刺耳。
 *
 * 移调档位刻意对齐 `xpFor()` 的连击倍率门槛（3/5/8），让听觉反馈与 XP
 * 反馈落在同一个节奏点上——玩家能「听出」自己进了下一档。
 */
export function correctTones(combo: number): Tone[] {
  const shift = Math.min(Math.floor(combo / 3), MAX_COMBO_SHIFT)
  return [
    { semitone: NOTE.C5 + shift, at: 0, duration: 0.1 },
    { semitone: NOTE.E5 + shift, at: 0.06, duration: 0.1 },
    { semitone: NOTE.G5 + shift, at: 0.12, duration: 0.2 },
  ]
}
