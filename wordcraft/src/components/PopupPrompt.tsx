import { useEffect, useState } from 'react'
import * as api from '../data/api'

const SESSION_NAMES: Record<string, string> = {
  morning: '晨曦之门',
  noon: '烈日之门',
  evening: '星夜之门',
}

/**
 * 调度弹出的 360×480 提示窗。不抢焦点；用户点「开始」后才把主窗口前置。
 */
export default function PopupPrompt() {
  const [sessionType, setSessionType] = useState('morning')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    void api
      .peekPopupSession()
      .then((t) => {
        if (t) setSessionType(t)
      })
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
  }, [])

  const run = async (action: () => Promise<void>) => {
    if (busy) return
    setBusy(true)
    setError('')
    try {
      await action()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
      setBusy(false)
    }
  }

  const name = SESSION_NAMES[sessionType] ?? '训练传送门'

  return (
    <div className="min-h-screen bg-wc-bg text-wc-text flex items-center justify-center p-5">
      <div className="w-full text-center">
        <img
          src="/assets/ui/app_icon_256.png"
          alt=""
          className="w-16 h-16 mx-auto mb-4 object-contain drop-shadow-[0_0_16px_rgba(168,85,247,0.5)]"
        />
        <h1 className="text-xl font-bold font-game mb-1">传送门已开启</h1>
        <p className="text-sm text-wc-text-muted mb-6">{name} 正在等待你</p>

        {error && (
          <p className="text-xs text-wc-warning mb-4 break-words">{error}</p>
        )}

        <button
          disabled={busy}
          onClick={() => void run(() => api.acceptPopup())}
          className="w-full py-3 mb-3 bg-gradient-to-r from-wc-primary to-wc-primary-bright rounded-xl font-bold hover:opacity-90 transition btn-game disabled:opacity-40"
        >
          开始冒险
        </button>
        <button
          disabled={busy}
          onClick={() => void run(() => api.snoozePopup())}
          className="w-full py-2.5 rounded-xl border border-wc-border bg-wc-surface-2 text-sm hover:border-wc-primary/50 transition disabled:opacity-40"
        >
          稍后
        </button>
      </div>
    </div>
  )
}
