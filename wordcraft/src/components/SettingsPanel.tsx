import { useState, useEffect, useCallback } from 'react'
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

/**
 * 学习范围。默认高中——产品是给备考高考的学生用的。
 *
 * 这不只是筛词：词库里 102 个虚词（the / be / I / you 这类）有 96 个标为
 * junior，选高中就一并挡掉了，不必再单独维护一份虚词名单。
 */
const LEVEL_OPTIONS = [
  { value: 'junior', label: '初中', hint: '1581 词' },
  { value: 'senior', label: '高中', hint: '2076 词' },
  { value: 'all', label: '全部', hint: '3657 词' },
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
  const [newWords, setNewWords] = useState('6')
  const [wordCount, setWordCount] = useState('20')
  const [sound, setSound] = useState(true)
  const [tts, setTts] = useState('edge')
  const [studyLevel, setStudyLevel] = useState('senior')
  const [studyDays, setStudyDays] = useState('1,2,3,4,5,6,7')
  const [remaining, setRemaining] = useState<number | null>(null)
  const [autostart, setAutostart] = useState(true)
  const [error, setError] = useState('')
  const [saved, setSaved] = useState('')
  const [exporting, setExporting] = useState(false)

  const load = useCallback(async () => {
    try {
      const [w, n, c, s, t, a, lv, sd, st] = await Promise.all([
        api.getSetting('session_windows'),
        api.getSetting('daily_new_words'),
        api.getSetting('session_word_count'),
        api.getSetting('sound_enabled'),
        api.getSetting('tts_provider'),
        api.getSetting('autostart_enabled'),
        api.getSetting('study_level'),
        api.getSetting('study_days'),
        api.getOverallStats(),
      ])
      setWindows(w ?? WINDOW_PRESETS[0].value)
      setNewWords(n ?? '6')
      setWordCount(c ?? '20')
      setSound(s !== 'false')
      setTts(t ?? 'edge')
      setAutostart(a !== 'false')
      setStudyLevel(lv ?? 'senior')
      setStudyDays(sd ?? '1,2,3,4,5,6,7')
      setRemaining(st.untouched)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  /**
   * 按当前配置走完剩余生词需要多久。
   *
   * 把这笔账摆出来，是因为它不摆出来就没人算：默认配置是按「每天都能用」
   * 调的，改成只有周末之后同样的滑块意味着一年多——而界面上看不出任何区别。
   */
  const projection = (() => {
    if (remaining === null || remaining <= 0) return null
    const days = studyDays.split(',').filter(Boolean).length
    const perWeek = days * 3 * Number(newWords || '0')
    if (perWeek <= 0) return null
    return { weeks: Math.ceil(remaining / perWeek), perWeek, remaining }
  })()

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
          hint="实际数量受强化队列大小自动调节，此处是上限。调高会让复习负担同步上升——每学 1 个新词约产生 9 次后续复习。"
          settingKey="daily_new_words"
        >
          <div className="flex items-center gap-3">
            <input
              type="range"
              min={0}
              max={20}
              value={newWords}
              onChange={(e) => setNewWords(e.target.value)}
              onMouseUp={() => save('daily_new_words', newWords, setNewWords)}
              onTouchEnd={() => save('daily_new_words', newWords, setNewWords)}
              className="flex-1 accent-wc-primary"
            />
            <span className="font-game-mono w-10 text-right text-wc-accent">{newWords}</span>
          </div>
        </Row>

        <Row
          title="单场词量"
          hint="一次训练的题目数。20 词约 3-4 分钟。"
          settingKey="session_word_count"
        >
          <div className="flex items-center gap-3">
            <input
              type="range"
              min={5}
              max={40}
              value={wordCount}
              onChange={(e) => setWordCount(e.target.value)}
              onMouseUp={() => save('session_word_count', wordCount, setWordCount)}
              onTouchEnd={() => save('session_word_count', wordCount, setWordCount)}
              className="flex-1 accent-wc-primary"
            />
            <span className="font-game-mono w-10 text-right text-wc-accent">{wordCount}</span>
          </div>
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

        <Row
          title="学习范围"
          hint="高中范围不再教 the / be / I 这类初中虚词——已经会的词不该再占用练习时间。切换后立即生效，已练过的范围外单词也不再排入。"
          settingKey="study_level"
        >
          <div className="flex gap-2">
            {LEVEL_OPTIONS.map((o) => (
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
                <div className="text-[10px] text-wc-text-dim mt-0.5">{o.hint}</div>
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
      </div>
    </div>
  )
}
