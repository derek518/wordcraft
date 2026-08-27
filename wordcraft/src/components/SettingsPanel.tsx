import { useState, useEffect, useCallback, useRef } from 'react'
import * as api from '../data/api'

interface SettingsPanelProps {
  onBack: () => void
}

const WINDOW_PRESETS = [
  { label: '标准（默认）', value: '09:00-11:00,13:00-15:00,19:00-21:00' },
  { label: '早起型', value: '07:00-09:00,12:00-14:00,18:00-20:00' },
  { label: '晚睡型', value: '10:00-12:00,15:00-17:00,21:00-23:00' },
  { label: '在校日', value: '07:00-08:00,12:00-13:30,19:00-22:00' },
]

/** ISO 星期：1=周一 … 7=周日 */
const WEEKDAYS = [
  { v: 1, label: '一' }, { v: 2, label: '二' }, { v: 3, label: '三' },
  { v: 4, label: '四' }, { v: 5, label: '五' }, { v: 6, label: '六' }, { v: 7, label: '日' },
]

const TTS_OPTIONS = [
  { value: 'edge', label: '优质发音（预生成）' },
  { value: 'sapi', label: '系统发音' },
  { value: 'off', label: '关闭发音' },
]

export default function SettingsPanel({ onBack }: SettingsPanelProps) {
  const [windows, setWindows] = useState('')
  const [newWords, setNewWords] = useState(String(api.DEFAULT_DAILY_NEW))
  const [sound, setSound] = useState(true)
  const [tts, setTts] = useState('edge')
  const [studyLevel, setStudyLevel] = useState('senior')
  const [studyDays, setStudyDays] = useState('1,2,3,4,5,6,7')
  const [remaining, setRemaining] = useState<number | null>(null)
  const [levelOptions, setLevelOptions] = useState<api.StudyLevelOption[]>([])
  const [overview, setOverview] = useState<api.AbilityOverview | null>(null)
  const [autostart, setAutostart] = useState(true)
  const [error, setError] = useState('')
  const [saved, setSaved] = useState('')
  const [exporting, setExporting] = useState(false)
  /** 重置需二次确认。不可逆的操作不该一键完成 */
  const [resetArmed, setResetArmed] = useState(false)
  const [resetting, setResetting] = useState(false)
  const [resetDone, setResetDone] = useState<api.ResetSummary | null>(null)

  const load = useCallback(async () => {
    try {
      const [w, n, s, t, a, lv, sd, st, lo] = await Promise.all([
        api.getSetting('session_windows'),
        api.getSetting('daily_new_words'),
        api.getSetting('sound_enabled'),
        api.getSetting('tts_provider'),
        api.getSetting('autostart_enabled'),
        api.getSetting('study_level'),
        api.getSetting('study_days'),
        api.getOverallStats(),
        api.getStudyLevels(),
      ])
      setOverview(await api.getAbilityOverview())
      setWindows(w ?? WINDOW_PRESETS[0].value)
      setNewWords(n ?? String(api.DEFAULT_DAILY_NEW))
      setSound(s !== 'false')
      setTts(t ?? 'edge')
      setAutostart(a !== 'false')
      setStudyLevel(lv ?? 'senior')
      setStudyDays(sd ?? '1,2,3,4,5,6,7')
      setRemaining(st.untouched)
      setLevelOptions(lo)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  /**
   * 每日预算推算出的节奏。**每场新词数与每场题数都由后端算**——
   * 那些系数在 plan.rs，抄到界面上就成了一份迟早过期的副本。
   *
   * 传预算而不是读库，是为了让滑块拖动时数字即时跟着走；读库只能反映
   * 已保存的值，界面会滞后于滑块，看起来像卡住了。
   */
  const [pace, setPace] = useState<api.Pace | null>(null)
  const studyDayCount = studyDays.split(',').filter(Boolean).length

  useEffect(() => {
    let alive = true
    api
      .getPace(Number(newWords || '0'), studyDayCount)
      .then((p) => alive && setPace(p))
      .catch((e) => alive && setError(e instanceof Error ? e.message : String(e)))
    return () => {
      alive = false
    }
  }, [newWords, studyDayCount])

  /**
   * 按当前配置走完剩余生词需要多久。
   *
   * 把这笔账摆出来，是因为它不摆出来就没人算：默认配置是按「每天都能用」
   * 调的，改成只有周末之后同样的滑块意味着一年多——而界面上看不出任何区别。
   */
  const projection =
    remaining !== null && remaining > 0 && pace && pace.weekly_new > 0
      ? { weeks: Math.ceil(remaining / pace.weekly_new), perWeek: pace.weekly_new, remaining }
      : null

  /**
   * 滑块的保存。停手 400ms 后提交一次。
   *
   * 先前挂在 `onMouseUp` 上，有两个毛病：
   * - 闭包捕获的是渲染时的 state。**点击滑轨**时 mousedown→change→mouseup
   *   可能在同一批处理内完成，于是保存旧值再回写，滑块弹回原位
   * - 用方向键调滑块只触发 change、不触发 mouseup，**改动静默丢失**
   *
   * 走 change 两者都没有，也不必再区分鼠标与触摸。
   */
  const debounced = useRef<
    Record<string, { timer: ReturnType<typeof setTimeout>; flush: () => void }>
  >({})

  const saveDebounced = (key: string, value: string, apply: (v: string) => void) => {
    apply(value)
    clearTimeout(debounced.current[key]?.timer)
    const commit = () => {
      delete debounced.current[key]
      void save(key, value, apply)
    }
    debounced.current[key] = { timer: setTimeout(commit, 400), flush: commit }
  }

  // 卸载时把待提交的写入**落地**而不是丢掉——
  // 拖完滑块立刻点返回，那次调整同样该算数
  useEffect(() => {
    const pending = debounced.current
    return () => {
      Object.values(pending).forEach(({ timer, flush }) => {
        clearTimeout(timer)
        flush()
      })
    }
  }, [])

  const save = async (key: string, value: string, apply: (v: string) => void) => {
    setError('')
    try {
      await api.setSetting(key, value)
      const actual = await api.getSetting(key)
      apply(actual ?? value)
      setSaved(key)
      setTimeout(() => setSaved(''), 1500)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
      void load()
    }
  }

  const toggleAutostart = async () => {
    const next = !autostart
    setError('')
    try {
      await api.setAutostart(next)
      const actual = await api.getSetting('autostart_enabled')
      setAutostart(actual !== 'false')
      setSaved('autostart_enabled')
      setTimeout(() => setSaved(''), 1500)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
      void load()
    }
  }

  const resetData = async () => {
    setError('')
    setResetting(true)
    try {
      const summary = await api.resetLearningData()
      setResetDone(summary)
      setResetArmed(false)
      // 等级、生词数、摸底状态全变了,界面必须跟着后端重新拉一遍
      await load()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setResetting(false)
    }
  }

  const exportData = async () => {
    setError('')
    setExporting(true)
    try {
      const json = await api.exportDataJson()
      const blob = new Blob([json], { type: 'application/json' })
      const url = URL.createObjectURL(blob)
      const link = document.createElement('a')
      link.href = url
      link.download = `wordcraft-${new Date().toISOString().slice(0, 10)}.json`
      link.click()
      URL.revokeObjectURL(url)
      setSaved('export')
      setTimeout(() => setSaved(''), 1500)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setExporting(false)
    }
  }

  const Row = ({
    title,
    hint,
    settingKey,
    children,
  }: {
    title: string
    hint: string
    settingKey: string
    children: React.ReactNode
  }) => (
    <div className="py-4 border-b border-wc-border/50 last:border-0">
      <div className="flex items-center justify-between mb-1">
        <span className="font-medium font-game">{title}</span>
        {saved === settingKey && (
          <span className="text-xs text-wc-success flex items-center gap-1">
            <span className="w-1.5 h-1.5 rounded-full bg-wc-success" />
            已保存
          </span>
        )}
      </div>
      <p className="text-xs text-wc-text-muted mb-3">{hint}</p>
      {children}
    </div>
  )

  return (
    <div className="max-w-lg mx-auto">
      {/* Header */}
      <div className="flex items-center gap-4 mb-6">
        <button
          onClick={onBack}
          className="flex items-center gap-1 text-sm text-wc-text-muted hover:text-wc-text transition"
        >
          <span className="text-lg">←</span> 返回
        </button>
        <div className="flex items-center gap-2">
          <img src="/assets/blocks/block_limited.png" alt="" className="w-6 h-6 object-contain" />
          <h2 className="text-xl font-bold font-game">设置</h2>
        </div>
      </div>

      {error && (
        <div className="p-3 rounded-xl bg-wc-danger/10 border border-wc-danger/30 text-sm mb-4 hud-panel">
          <span className="font-bold text-wc-danger">保存失败：</span>
          <span className="text-wc-text-muted ml-1 break-words">{error}</span>
        </div>
      )}

      <div className="hud-panel rounded-2xl px-5 relative overflow-hidden">
        <div className="absolute top-0 left-0 right-0 h-[1px] bg-gradient-to-r from-transparent via-wc-primary/30 to-transparent" />

        <Row
          title="传送门时段"
          hint="每天三个时段自动弹出训练。改动后下次轮询即生效。"
          settingKey="session_windows"
        >
          <div className="space-y-2">
            {WINDOW_PRESETS.map((p) => (
              <button
                key={p.value}
                onClick={() => save('session_windows', p.value, setWindows)}
                className={`w-full text-left px-3 py-2.5 rounded-xl border text-sm transition-all ${
                  windows === p.value
                    ? 'border-wc-primary bg-wc-primary/10 shadow-[0_0_10px_rgba(124,58,237,0.15)]'
                    : 'border-wc-border bg-wc-surface-2 hover:border-wc-primary/50'
                }`}
              >
                <div className="font-medium">{p.label}</div>
                <div className="text-xs text-wc-text-muted font-game-mono mt-0.5">{p.value}</div>
              </button>
            ))}
          </div>
        </Row>

        <Row
          title="每日新词上限"
          hint="一天最多学几个新词。每场的新词数与题数都由它推算，不需要单独设置——两个能各自取值的旋钮会配出无法满足的组合。已经学过的新词会从当天预算里扣除，所以跳过一个时段，剩下的时段会自动补上。"
          settingKey="daily_new_words"
        >
          <div className="flex items-center gap-3">
            <input
              type="range"
              min={0}
              max={60}
              value={newWords}
              onChange={(e) => saveDebounced('daily_new_words', e.target.value, setNewWords)}
              className="flex-1 accent-wc-primary"
            />
            <span className="font-game-mono w-12 text-right text-wc-accent">{newWords}</span>
          </div>
          {pace && (
            <div className="mt-2 text-xs text-wc-text-muted font-game-mono">
              每场约 {pace.new_per_session} 个新词 · {pace.session_words} 道题
            </div>
          )}
        </Row>

        <Row title="音效" hint="答对、答错、升级的即时反馈音。" settingKey="sound_enabled">
          <button
            onClick={() => save('sound_enabled', String(!sound), (v) => setSound(v === 'true'))}
            className={`px-4 py-2 rounded-xl border text-sm transition flex items-center gap-2 ${
              sound
                ? 'border-wc-success bg-wc-success/10 text-wc-success'
                : 'border-wc-border bg-wc-surface-2 text-wc-text-muted'
            }`}
          >
            <div className={`toggle-switch ${sound ? 'on' : ''}`} style={{ pointerEvents: 'none' }} />
            <span>{sound ? '已开启' : '已关闭'}</span>
          </button>
        </Row>

        {overview && (
          <div className="py-4">
            <div className="font-medium font-game mb-1">当前水平</div>
            <p className="text-xs text-wc-text-muted mb-3 leading-relaxed">
              由每天的作答自动估算，不需要手动设置。第一次遇见的词答对答错都是一次
              观测——练得越多，估计越准，重点段也跟着走。
            </p>
            <div className="rounded-xl border border-wc-border bg-wc-surface-2 px-4 py-3">
              <div className="flex items-baseline gap-2">
                <span className="text-2xl font-game-mono text-wc-accent">
                  {overview.vocabulary.toLocaleString()}
                </span>
                <span className="text-xs text-wc-text-muted font-game-mono">
                  词 · 区间 {overview.vocabulary_low.toLocaleString()}–
                  {overview.vocabulary_high.toLocaleString()}
                </span>
              </div>
              <div className="mt-2 text-xs font-game-mono text-wc-text-muted">
                重点练习：词频第 {overview.frontier_from.toLocaleString()}–
                {overview.frontier_to.toLocaleString()} 名，
                <span className="text-wc-primary">
                  还剩 {overview.frontier_untouched.toLocaleString()} 个没学过
                </span>
              </div>
              <div className="mt-1 text-xs font-game-mono text-wc-text-muted">
                已掌握 {overview.known.toLocaleString()} · 重点{' '}
                {overview.frontier.toLocaleString()} · 暂缓{' '}
                {overview.too_hard.toLocaleString()}
              </div>
              {overview.observations === 0 ? (
                <div className="mt-2 text-xs text-wc-warning">
                  还没有作答记录，上面是初始估计。练几场之后会自动校正。
                </div>
              ) : (
                <div className="mt-2 text-xs text-wc-text-muted font-game-mono">
                  已采集 {overview.observations} 次首见作答
                </div>
              )}
            </div>
          </div>
        )}

        <Row
          title="考纲范围（可选）"
          hint="只想过一遍某本考纲时才用。难度不靠它——那由上面的能力估计负责，而考纲标签和难度基本无关：102 个高中词的常用度和 the 同级，28 个初中词比大多数四级词还生僻。默认「全部」，让系统在全库里挑。"
          settingKey="study_level"
        >
          <div className="flex gap-2">
            {levelOptions.map((o) => (
              <button
                key={o.value}
                onClick={() => save('study_level', o.value, setStudyLevel)}
                className={`flex-1 px-3 py-2.5 rounded-xl border text-xs transition-all ${
                  studyLevel === o.value
                    ? 'border-wc-primary bg-wc-primary/10 shadow-[0_0_10px_rgba(124,58,237,0.15)]'
                    : 'border-wc-border bg-wc-surface-2 hover:border-wc-primary/50'
                }`}
              >
                <div className="font-bold">{o.label}</div>
                <div className="text-[10px] text-wc-text-dim mt-0.5">{o.words} 词</div>
              </button>
            ))}
          </div>
        </Row>

        {projection && (
          <div className="rounded-xl border border-wc-border bg-wc-surface-2/60 p-3 text-xs">
            <div className="text-wc-text-dim mb-1">按当前设置估算</div>
            <div>
              还剩 <span className="font-game-mono text-wc-accent">{projection.remaining}</span> 个生词，
              每周 <span className="font-game-mono text-wc-accent">{projection.perWeek}</span> 个，
              约 <span className="font-game-mono text-wc-gold">{projection.weeks}</span> 周走完
            </div>
            {projection.weeks > 30 && (
              <div className="text-wc-warning mt-1">
                超过半年。可以增加学习日，或把「每场新词」调高——每场题量的上限是
                两分钟，别把单场撑得太长。
              </div>
            )}
          </div>
        )}

        <Row
          title="学习日"
          hint="只在选中的日子弹出训练，赛道也按这几天计分。上学期间把工作日取消掉——没弹窗的日子不算断签，赛道目标也会跟着缩小，不会留一个永远够不着的「完美一周」。"
          settingKey="study_days"
        >
          <div className="flex gap-1.5">
            {WEEKDAYS.map((d) => {
              const picked = studyDays.split(',').includes(String(d.v))
              return (
                <button
                  key={d.v}
                  onClick={() => {
                    const cur = studyDays.split(',').filter(Boolean)
                    const next = picked
                      ? cur.filter((x) => x !== String(d.v))
                      : [...cur, String(d.v)].sort()
                    // 一天都不选等于停用整个应用，后端会回落到每天——
                    // 与其让它静静回滚，不如这里就不允许
                    if (next.length === 0) return
                    save('study_days', next.join(','), setStudyDays)
                  }}
                  className={`flex-1 py-2.5 rounded-xl border text-xs transition-all ${
                    picked
                      ? 'border-wc-primary bg-wc-primary/10 shadow-[0_0_10px_rgba(124,58,237,0.15)]'
                      : 'border-wc-border bg-wc-surface-2 hover:border-wc-primary/50'
                  }`}
                >
                  {d.label}
                </button>
              )
            })}
          </div>
        </Row>

        <Row
          title="单词发音"
          hint="关闭后「听音辨词」题型会自动降级为中译英——没有声音的听力题只能靠猜。"
          settingKey="tts_provider"
        >
          <div className="flex gap-2">
            {TTS_OPTIONS.map((o) => (
              <button
                key={o.value}
                onClick={() => save('tts_provider', o.value, setTts)}
                className={`flex-1 px-3 py-2.5 rounded-xl border text-xs transition-all ${
                  tts === o.value
                    ? 'border-wc-primary bg-wc-primary/10 shadow-[0_0_10px_rgba(124,58,237,0.15)]'
                    : 'border-wc-border bg-wc-surface-2 hover:border-wc-primary/50'
                }`}
              >
                {o.label}
              </button>
            ))}
          </div>
        </Row>

        <Row
          title="开机自启"
          hint="登录系统后自动常驻托盘，到点弹出训练。关闭后只能手动打开。"
          settingKey="autostart_enabled"
        >
          <button
            aria-label="开机自启"
            onClick={() => void toggleAutostart()}
            className={`px-4 py-2 rounded-xl border text-sm transition flex items-center gap-2 ${
              autostart
                ? 'border-wc-success bg-wc-success/10 text-wc-success'
                : 'border-wc-border bg-wc-surface-2 text-wc-text-muted'
            }`}
          >
            <div className={`toggle-switch ${autostart ? 'on' : ''}`} style={{ pointerEvents: 'none' }} />
            <span>{autostart ? '已开启' : '已关闭'}</span>
          </button>
        </Row>

        <div className="py-4">
          <div className="flex items-center justify-between mb-1">
            <span className="font-medium font-game">导出学习数据</span>
            {saved === 'export' && (
              <span className="text-xs text-wc-success flex items-center gap-1">
                <span className="w-1.5 h-1.5 rounded-full bg-wc-success" />
                已导出
              </span>
            )}
          </div>
          <p className="text-xs text-wc-text-muted mb-3">
            下载一份 JSON，包含词库状态与作答记录，便于换机或备份。
          </p>
          <button
            onClick={() => void exportData()}
            disabled={exporting}
            className="px-4 py-2 rounded-xl border border-wc-border bg-wc-surface-2 text-sm hover:border-wc-primary/50 transition disabled:opacity-40"
          >
            {exporting ? '正在导出…' : '导出 JSON'}
          </button>
        </div>

        <div className="py-4 border-t border-wc-danger/25">
          <div className="font-medium font-game text-wc-danger mb-1">交给孩子前：清空试用数据</div>
          <p className="text-xs text-wc-text-muted mb-3 leading-relaxed">
            装好后自己点着试的那些作答会进入记忆算法。成年人的正确率与反应速度会把
            上百个词判成「已掌握」，孩子接手后这些词一个月内都不会再出现——而这个
            判断来自另一个人。<strong className="text-wc-danger">清空后无法恢复</strong>
            ：作答记录、等级、方块、卡牌收藏全部归零，词库与上面的配置保留。
          </p>
          {resetDone ? (
            <div className="text-xs font-game-mono text-wc-success">
              已清空 {resetDone.total_rows} 行
              {resetDone.cleared.length > 0 &&
                `（${resetDone.cleared.map(([t, n]) => `${t} ${n}`).join('、')}）`}
            </div>
          ) : resetArmed ? (
            <div className="flex items-center gap-2">
              <button
                onClick={() => void resetData()}
                disabled={resetting}
                className="px-4 py-2 rounded-xl border border-wc-danger bg-wc-danger/15 text-wc-danger text-sm hover:bg-wc-danger/25 transition disabled:opacity-40"
              >
                {resetting ? '正在清空…' : '确认清空，不可恢复'}
              </button>
              <button
                onClick={() => setResetArmed(false)}
                disabled={resetting}
                className="px-4 py-2 rounded-xl border border-wc-border bg-wc-surface-2 text-sm hover:border-wc-primary/50 transition disabled:opacity-40"
              >
                取消
              </button>
            </div>
          ) : (
            <button
              onClick={() => setResetArmed(true)}
              className="px-4 py-2 rounded-xl border border-wc-danger/50 bg-wc-surface-2 text-wc-danger text-sm hover:bg-wc-danger/10 transition"
            >
              清空学习数据
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
