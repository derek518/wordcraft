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
  const [error, setError] = useState('')
  const [saved, setSaved] = useState('')

  const load = useCallback(async () => {
    try {
      const [w, n, c, s, t] = await Promise.all([
        api.getSetting('session_windows'),
        api.getSetting('daily_new_words'),
        api.getSetting('session_word_count'),
        api.getSetting('sound_enabled'),
        api.getSetting('tts_provider'),
      ])
      setWindows(w ?? WINDOW_PRESETS[0].value)
      setNewWords(n ?? '6')
      setWordCount(c ?? '20')
      setSound(s !== 'false')
      setTts(t ?? 'edge')
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

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
          <img src="/assets/blocks/block_special.png" alt="" className="w-6 h-6 object-contain" />
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
      </div>

      <p className="text-xs text-wc-text-muted mt-4 text-center">
        开机自启在系统托盘菜单中设置
      </p>
    </div>
  )
}
