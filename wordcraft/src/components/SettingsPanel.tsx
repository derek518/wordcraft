import { useState, useEffect, useCallback } from 'react'
import * as api from '../data/api'

interface SettingsPanelProps {
  onBack: () => void
}

/** 时段配置的常用预设。手写 `HH:MM-HH:MM,…` 容易出错，且写坏会整天不弹窗。 */
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

/**
 * 设置页。契约 §2.1 的可写键。
 *
 * 每项改动立即落库并回读——不设「保存」按钮。后端有白名单校验，
 * 非法值会被拒绝，此处把拒绝原因原样呈现，而不是悄悄回滚成旧值。
 */
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

  /**
   * 写入并回读。
   *
   * 回读不是多余的：后端白名单可能拒绝这个值，若只更新本地 state，
   * 界面会显示一个数据库里根本不存在的设置——用户以为改了，实际没有。
   */
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
      // 失败时回读真实值，让界面与数据库一致
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
    <div className="py-4 border-b border-wc-border last:border-0">
      <div className="flex items-center justify-between mb-1">
        <span className="font-medium">{title}</span>
        {saved === settingKey && <span className="text-xs text-wc-success">已保存</span>}
      </div>
      <p className="text-xs text-wc-text-muted mb-3">{hint}</p>
      {children}
    </div>
  )

  return (
    <div className="max-w-lg mx-auto">
      <div className="flex items-center gap-4 mb-6">
        <button onClick={onBack} className="text-sm text-wc-text-muted hover:text-wc-text transition">
          ← 返回
        </button>
        <h2 className="text-xl font-bold">⚙️ 设置</h2>
      </div>

      {error && (
        <div className="p-3 rounded-lg bg-wc-danger/10 border border-wc-danger/30 text-sm mb-4">
          <span className="font-bold text-wc-danger">保存失败：</span>
          <span className="text-wc-text-muted ml-1 break-words">{error}</span>
        </div>
      )}

      <div className="bg-wc-surface border border-wc-border rounded-xl px-5">
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
                className={`w-full text-left px-3 py-2 rounded-lg border text-sm transition ${
                  windows === p.value
                    ? 'border-wc-primary bg-wc-primary/10'
                    : 'border-wc-border bg-wc-surface-2 hover:border-wc-primary/50'
                }`}
              >
                <div className="font-medium">{p.label}</div>
                <div className="text-xs text-wc-text-muted font-mono">{p.value}</div>
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
            <span className="font-mono w-10 text-right">{newWords}</span>
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
            <span className="font-mono w-10 text-right">{wordCount}</span>
          </div>
        </Row>

        <Row title="音效" hint="答对、答错、升级的即时反馈音。" settingKey="sound_enabled">
          <button
            onClick={() => save('sound_enabled', String(!sound), (v) => setSound(v === 'true'))}
            className={`px-4 py-2 rounded-lg border text-sm transition ${
              sound
                ? 'border-wc-success bg-wc-success/10 text-wc-success'
                : 'border-wc-border bg-wc-surface-2 text-wc-text-muted'
            }`}
          >
            {sound ? '已开启' : '已关闭'}
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
                className={`flex-1 px-3 py-2 rounded-lg border text-xs transition ${
                  tts === o.value
                    ? 'border-wc-primary bg-wc-primary/10'
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
