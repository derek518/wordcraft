//! 单词发音。spec F4：点击后 300ms 内出声。
//!
//! 两级策略（contracts §3.4 的 `tts_provider`）：
//! 1. **预生成缓存**——Edge-TTS 批量产出 mp3 随包分发，命中即播，延迟最低
//! 2. **系统 TTS 实时合成**——缓存未命中时的降级路径
//!
//! 当前实现第 2 级。第 1 级需要真实词库先就位（T15–T18），为 52 个占位词
//! 生成音频没有意义——词库一换就全部作废。
//!
//! ADR-3 要求平台差异由抽象隔离。此处两个平台都是**真实现**而非 stub：
//! 发音在开发机上同样需要能听见，否则无从验证。

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use tauri::{AppHandle, Manager};

/// 单词允许的字符。
///
/// 白名单而非黑名单：Windows 分支要把词交给 PowerShell，而 PowerShell 的
/// `-Command` 收的是一段脚本文本，词里若含引号即可逃逸。词库将来从公开仓库
/// 导入，"数据来自我们自己的库" 不构成跳过校验的理由。
fn validate_word(word: &str) -> Result<(), String> {
    if word.is_empty() {
        return Err("单词为空".to_string());
    }
    if word.chars().count() > 64 {
        return Err(format!("单词过长（{} 字符）", word.chars().count()));
    }
    let ok = word
        .chars()
        .all(|c| c.is_ascii_alphabetic() || matches!(c, '-' | '\'' | ' ' | '.'));
    if !ok {
        return Err(format!("单词 `{word}` 含非法字符，仅允许字母与 - ' . 空格"));
    }
    Ok(())
}

/// 预生成音频的缓存路径。
pub fn cache_path(app: &AppHandle, word: &str) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {e}"))?
        .join("audio_cache");
    Ok(dir.join(format!("{}.mp3", word.to_lowercase())))
}

// ─────────────────────────────────────────────
// 平台实现
// ─────────────────────────────────────────────

/// 启动一个朗读进程。返回后台句柄，调用方负责回收。
#[cfg(target_os = "macos")]
fn spawn_speech(word: &str) -> Result<Child, String> {
    // 必须显式指定英语音色：中文环境下默认音色会把英文按拼音读
    Command::new("/usr/bin/say")
        .args(["-v", "Samantha", "-r", "170", word])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("启动 say 失败: {e}"))
}

#[cfg(target_os = "windows")]
fn spawn_speech(word: &str) -> Result<Child, String> {
    // 词经由环境变量传入，不拼进脚本文本——PowerShell 的 -Command 收的是
    // 可执行脚本，拼接等同于把用户数据当代码执行
    const SCRIPT: &str = "\
        Add-Type -AssemblyName System.Speech; \
        $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
        $v = $s.GetInstalledVoices() | Where-Object { $_.VoiceInfo.Culture.Name -like 'en-*' } | Select-Object -First 1; \
        if ($v) { $s.SelectVoice($v.VoiceInfo.Name) }; \
        $s.Rate = 0; \
        $s.Speak($env:WORDCRAFT_TTS_WORD)";

    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .env("WORDCRAFT_TTS_WORD", word)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("启动 PowerShell 语音合成失败: {e}"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn spawn_speech(_word: &str) -> Result<Child, String> {
    Err("当前平台没有可用的语音合成后端".to_string())
}

// ─────────────────────────────────────────────
// Command
// ─────────────────────────────────────────────

/// contracts §3.4：朗读单词。
///
/// 不等待朗读结束——`spawn` 成功即返回，进程在后台回收。等待子进程结束会让
/// 调用阻塞整个朗读时长（1–2 秒），spec F4 的 300ms 预算立刻穿底。
#[tauri::command]
pub fn play_word_audio(app: AppHandle, word: String) -> Result<(), String> {
    validate_word(&word)?;

    // 预生成缓存命中时应直接播放文件（T19 剩余部分）。缓存尚未生成，
    // 此处先记录命中情况，便于后续接入时确认路径正确
    if let Ok(path) = cache_path(&app, &word) {
        if path.exists() {
            log::debug!("音频缓存命中但播放路径尚未接入: {}", path.display());
        }
    }

    let child = spawn_speech(&word)?;

    // 回收子进程并检查退出码。`spawn` 成功只说明命令存在——音色缺失、
    // 音频设备被占用都会让进程立刻非零退出，而调用方已经拿到 Ok 了。
    // 这条日志是这类静默失败的唯一线索
    tauri::async_runtime::spawn(async move {
        let mut child = child;
        match child.wait() {
            Ok(status) if !status.success() => {
                log::warn!("语音合成进程异常退出（{status}），单词 `{word}` 可能未发声");
            }
            Err(e) => log::warn!("语音合成进程回收失败: {e}"),
            _ => {}
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 合法单词通过校验() {
        for w in ["crystal", "well-known", "don't", "New York", "etc."] {
            assert!(validate_word(w).is_ok(), "`{w}` 应通过校验");
        }
    }

    #[test]
    fn 拒绝可能逃逸_powershell_脚本的字符() {
        // 这些字符若拼进 -Command 的脚本文本即可执行任意代码
        for w in [
            "word'; Remove-Item C:\\ -Recurse; '",
            "word\"",
            "word$(whoami)",
            "word`n",
            "word;calc",
            "word|more",
            "word&echo",
        ] {
            assert!(validate_word(w).is_err(), "`{w}` 应被拒绝");
        }
    }

    #[test]
    fn 拒绝空串与超长输入() {
        assert!(validate_word("").is_err());
        assert!(validate_word(&"a".repeat(65)).is_err());
        assert!(validate_word(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn 拒绝非_ascii_字母() {
        // 中文、重音字母等不属于本词库范围，放行会让 TTS 读出意外内容
        assert!(validate_word("水晶").is_err());
        assert!(validate_word("café").is_err());
    }

    /// 合成后端在本机真实可用——会实际发出声音。
    ///
    /// 默认跳过：CI 与无音频设备的机器上发声既无意义也无从断言。
    /// 手动验证用 `cargo test 语音合成后端 -- --ignored --nocapture`。
    ///
    /// 校验退出码而非只看 spawn 成功：`spawn` 只证明命令存在，音色缺失或
    /// 设备被占用时进程会立刻非零退出，而那正是「点了没反应」的真实成因。
    #[test]
    #[ignore = "会实际发声，需手动运行"]
    fn 语音合成后端在本机可用() {
        let started = std::time::Instant::now();
        let mut child = spawn_speech("crystal").expect("启动合成进程失败");
        let spawn_ms = started.elapsed().as_millis();

        let status = child.wait().expect("回收合成进程失败");
        println!("spawn 耗时 {spawn_ms}ms，退出码 {status}");

        assert!(status.success(), "合成进程异常退出：{status}");
        assert!(
            spawn_ms < 300,
            "spawn 耗时 {spawn_ms}ms 超出 spec F4 的 300ms 预算"
        );
    }
}
